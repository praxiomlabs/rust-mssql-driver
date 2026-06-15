//! The streaming-redesign memory proof and regression guard.
//!
//! This is the *definition of done* for true result streaming. It runs a query
//! whose response is ~10 MB and asserts that peak heap allocation during
//! consumption stays bounded to roughly one row — not the whole response. A
//! custom counting global allocator (this binary only) makes the measurement
//! deterministic, unlike RSS.
//!
//! It consumes via [`Client::query_stream`] (the incremental path added in the
//! streaming redesign), which reads TDS packets on demand. Against the buffered
//! [`Client::query`] this assertion would FAIL (peak ≈ the full response,
//! measured at ~40 MB for this query); against `query_stream` peak stays at
//! ~one packet plus one row, so it passes.
//!
//! Run it against a live server (it is `#[ignore]`d so it never runs without
//! one):
//! ```text
//! MSSQL_HOST=localhost MSSQL_PORT=1433 MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!   cargo nextest run -p mssql-client --test streaming_memory --run-ignored ignored-only
//! ```

#![allow(clippy::expect_used)]
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use mssql_client::{Client, Config};

/// A pass-through allocator that tracks live and peak allocated bytes.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: delegates every allocation to the system allocator unchanged; the
// atomics only observe sizes and never affect the returned pointers.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT").unwrap_or_else(|_| "1433".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let conn_str = format!(
        "Server={host},{port};Database=master;User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt=true"
    );
    Config::from_connection_string(&conn_str).ok()
}

/// ~100k rows of `(int, char(100))` ≈ a 10 MB response. Peak heap while reading
/// it must stay far below that if rows stream rather than buffer.
const LARGE_QUERY: &str = "\
    WITH n AS (SELECT 1 AS i UNION ALL SELECT i + 1 FROM n WHERE i < 100000) \
    SELECT i, CAST(REPLICATE('x', 100) AS CHAR(100)) AS pad FROM n OPTION (MAXRECURSION 0)";

#[tokio::test]
#[ignore = "Requires a live SQL Server"]
async fn streaming_query_bounds_peak_memory() {
    let Some(cfg) = config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // Baseline the live bytes, then measure the peak reached while the query
    // runs and the rows are consumed incrementally.
    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);

    let mut stream = client.query_stream(LARGE_QUERY, &[]).await.expect("query");
    let mut count = 0usize;
    while let Some(row) = stream.try_next().await.expect("row") {
        let _ = row;
        count += 1;
    }

    let peak_delta = PEAK.load(Ordering::Relaxed).saturating_sub(baseline);
    eprintln!("rows={count} peak_delta={peak_delta} bytes");

    assert_eq!(count, 100_000, "expected 100k rows");
    // ~one row is ~100 bytes; one packet is ~8 KB. A 2 MB bound is far below the
    // ~10 MB response (the buffered path peaks at ~40 MB), so it can only pass
    // if rows stream rather than buffer.
    assert!(
        peak_delta < 2_000_000,
        "peak heap delta was {peak_delta} bytes — streaming should bound this to \
         roughly one row, not the whole ~10 MB response"
    );
}
