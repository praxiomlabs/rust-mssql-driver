//! Allocation measurement for the buffered row-decode path (#300).
//!
//! The buffered read path (`query` -> `QueryStream`) is allocation-bound: a
//! callgrind profile of a 1000-row decode spends ~48% of instructions in
//! malloc/free/realloc. This binary gives that path a deterministic, in-memory
//! measurement basis (no live server, no valgrind) so each optimization PR can
//! report a reproducible allocation delta.
//!
//! It installs a counting global allocator (this binary only, the same pattern
//! as `tests/streaming_memory.rs`), decodes a synthesized 64-column x 1000-row
//! response (cycling INT4 / NVARCHAR / BigVarBinary so the fixed-, ushort-
//! string-, and ushort-binary decode arms are all exercised), and prints the
//! number of heap allocations and bytes attributable to the decode alone (the
//! fixture is built before the measurement window).
//!
//! Run with: `cargo bench -p mssql-client --bench buffered_decode --features bench`

#![allow(
    missing_docs,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;
use mssql_client::__bench::decode_buffered_response;

/// Number of heap allocations and total bytes requested, since process start.
static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

/// A pass-through allocator that counts allocation calls and bytes.
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: forwarding an unchanged layout to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarding the pointer/layout pair we returned from `alloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const NUM_COLS: usize = 64;
const NUM_ROWS: usize = 1000;

/// Column shape cycled across the row, by TDS `TypeId`.
#[derive(Clone, Copy)]
enum ColKind {
    Int4,
    NVarChar,
    VarBinary,
}

fn col_kind(i: usize) -> ColKind {
    match i % 3 {
        0 => ColKind::Int4,
        1 => ColKind::NVarChar,
        _ => ColKind::VarBinary,
    }
}

/// Append a UTF-16LE column name (`c<i>`) prefixed by its char count.
fn push_name(v: &mut Vec<u8>, i: usize) {
    let name: Vec<u16> = format!("c{i}").encode_utf16().collect();
    v.push(name.len() as u8);
    for u in name {
        v.extend_from_slice(&u.to_le_bytes());
    }
}

/// Build a COLMETADATA token (0x81) for `NUM_COLS` columns.
fn colmetadata() -> Vec<u8> {
    let mut v = vec![0x81];
    v.extend_from_slice(&(NUM_COLS as u16).to_le_bytes());
    for i in 0..NUM_COLS {
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user_type
        v.extend_from_slice(&[0x01, 0x00]); // flags (nullable)
        match col_kind(i) {
            ColKind::Int4 => {
                v.push(0x38); // TypeId::Int4 (fixed, 4 bytes)
            }
            ColKind::NVarChar => {
                v.push(0xE7); // TypeId::NVarChar
                v.extend_from_slice(&[0x64, 0x00]); // max_length = 100
                v.extend_from_slice(&[0x09, 0x04, 0xD0, 0x00, 0x34]); // collation
            }
            ColKind::VarBinary => {
                v.push(0xA5); // TypeId::BigVarBinary
                v.extend_from_slice(&[0x00, 0x01]); // max_length = 256
            }
        }
        push_name(&mut v, i);
    }
    v
}

/// Build one ROW token (0xD1) with representative, mostly-non-null values.
fn row(r: usize) -> Vec<u8> {
    let mut v = vec![0xD1];
    for i in 0..NUM_COLS {
        match col_kind(i) {
            ColKind::Int4 => {
                v.extend_from_slice(&((r * 31 + i) as i32).to_le_bytes());
            }
            ColKind::NVarChar => {
                // Every 8th cell NULL, to mix the NULL path in.
                if (r + i) % 8 == 0 {
                    v.extend_from_slice(&[0xFF, 0xFF]);
                } else {
                    let s: Vec<u8> = format!("val_{r}_{i}")
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect();
                    v.extend_from_slice(&(s.len() as u16).to_le_bytes());
                    v.extend_from_slice(&s);
                }
            }
            ColKind::VarBinary => {
                let bytes = [(r as u8), (i as u8), 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45];
                v.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
                v.extend_from_slice(&bytes);
            }
        }
    }
    v
}

/// DONE token (0xFD): status + curcmd + 8-byte row count.
fn done(row_count: u64) -> Vec<u8> {
    let mut v = vec![0xFD];
    v.extend_from_slice(&[0x10, 0x00]); // DONE_COUNT
    v.extend_from_slice(&[0xC1, 0x00]); // curcmd
    v.extend_from_slice(&row_count.to_le_bytes());
    v
}

/// Assemble the full response buffer (built before the measurement window).
fn build_fixture() -> Bytes {
    let mut v = colmetadata();
    for r in 0..NUM_ROWS {
        v.extend_from_slice(&row(r));
    }
    v.extend_from_slice(&done(NUM_ROWS as u64));
    Bytes::from(v)
}

fn main() {
    // Build the fixture BEFORE sampling the counters, so only the decode is
    // attributed.
    let fixture = build_fixture();

    let calls_before = ALLOC_CALLS.load(Ordering::Relaxed);
    let bytes_before = ALLOC_BYTES.load(Ordering::Relaxed);

    let rows = decode_buffered_response(fixture);

    let calls = ALLOC_CALLS.load(Ordering::Relaxed) - calls_before;
    let bytes = ALLOC_BYTES.load(Ordering::Relaxed) - bytes_before;

    // Correctness guard + keep `rows` live across the measurement.
    assert_eq!(rows.len(), NUM_ROWS, "decoded the wrong number of rows");

    println!("buffered_decode {NUM_COLS}x{NUM_ROWS}:");
    println!("  {calls} allocations, {bytes} bytes ({} rows)", rows.len());
    println!(
        "  per row: {:.2} allocations, {:.0} bytes",
        calls as f64 / NUM_ROWS as f64,
        bytes as f64 / NUM_ROWS as f64,
    );
}
