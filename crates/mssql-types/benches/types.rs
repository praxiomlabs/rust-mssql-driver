//! Benchmarks for SQL Server type encoding and decoding.

#![allow(clippy::unwrap_used, clippy::approx_constant, missing_docs)]

use bytes::BytesMut;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mssql_types::{FromSql, SqlValue, ToSql, decode_utf16_string, encode_utf16_string};
use std::hint::black_box;

/// Helper to encode a UTF-16 string and return the bytes.
fn encode_utf16_to_bytes(s: &str) -> Vec<u8> {
    let mut buf = BytesMut::new();
    encode_utf16_string(s, &mut buf);
    buf.to_vec()
}

/// Benchmark UTF-16 string encoding (Rust String → SQL NVARCHAR format).
fn bench_utf16_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("utf16_encode");

    // Short string
    let short = "Hello";
    group.throughput(Throughput::Bytes(short.len() as u64));
    group.bench_function("short", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(64);
            encode_utf16_string(black_box(short), &mut buf);
            black_box(buf)
        })
    });

    // Medium string (typical column value)
    let medium = "This is a typical database column value with some content";
    group.throughput(Throughput::Bytes(medium.len() as u64));
    group.bench_function("medium", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(256);
            encode_utf16_string(black_box(medium), &mut buf);
            black_box(buf)
        })
    });

    // Long string (text field)
    let long = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";
    group.throughput(Throughput::Bytes(long.len() as u64));
    group.bench_function("long", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(1024);
            encode_utf16_string(black_box(long), &mut buf);
            black_box(buf)
        })
    });

    // Unicode string (non-ASCII)
    let unicode = "日本語テスト文字列 émoji et accénts";
    group.throughput(Throughput::Bytes(unicode.len() as u64));
    group.bench_function("unicode", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(256);
            encode_utf16_string(black_box(unicode), &mut buf);
            black_box(buf)
        })
    });

    group.finish();
}

/// Benchmark UTF-16 string decoding (SQL NVARCHAR format → Rust String).
fn bench_utf16_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("utf16_decode");

    // Short string - skip 2-byte length prefix
    let short_encoded = encode_utf16_to_bytes("Hello");
    let short_data = &short_encoded[2..]; // Skip length prefix
    group.throughput(Throughput::Bytes(short_data.len() as u64));
    group.bench_function("short", |b| {
        b.iter(|| {
            let decoded = decode_utf16_string(black_box(short_data));
            black_box(decoded)
        })
    });

    // Medium string
    let medium_encoded =
        encode_utf16_to_bytes("This is a typical database column value with some content");
    let medium_data = &medium_encoded[2..];
    group.throughput(Throughput::Bytes(medium_data.len() as u64));
    group.bench_function("medium", |b| {
        b.iter(|| {
            let decoded = decode_utf16_string(black_box(medium_data));
            black_box(decoded)
        })
    });

    // Long string
    let long = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";
    let long_encoded = encode_utf16_to_bytes(long);
    let long_data = &long_encoded[2..];
    group.throughput(Throughput::Bytes(long_data.len() as u64));
    group.bench_function("long", |b| {
        b.iter(|| {
            let decoded = decode_utf16_string(black_box(long_data));
            black_box(decoded)
        })
    });

    group.finish();
}

/// Benchmark ToSql conversions (Rust → SqlValue).
fn bench_to_sql(c: &mut Criterion) {
    let mut group = c.benchmark_group("to_sql");

    // Integer
    let int_val: i32 = 12345;
    group.bench_function("i32", |b| {
        b.iter(|| {
            let sql_val = black_box(&int_val).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // Big integer
    let bigint_val: i64 = 9876543210i64;
    group.bench_function("i64", |b| {
        b.iter(|| {
            let sql_val = black_box(&bigint_val).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // Float
    let float_val: f64 = 3.14159265358979;
    group.bench_function("f64", |b| {
        b.iter(|| {
            let sql_val = black_box(&float_val).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // Bool
    let bool_val = true;
    group.bench_function("bool", |b| {
        b.iter(|| {
            let sql_val = black_box(&bool_val).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // String
    let string_val = "test string value".to_string();
    group.bench_function("String", |b| {
        b.iter(|| {
            let sql_val = black_box(&string_val).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // String slice
    let str_val: &str = "test string value";
    group.bench_function("str", |b| {
        b.iter(|| {
            let sql_val = black_box(&str_val).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // Option<i32> - Some
    let opt_some: Option<i32> = Some(42);
    group.bench_function("Option_i32_Some", |b| {
        b.iter(|| {
            let sql_val = black_box(&opt_some).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    // Option<i32> - None
    let opt_none: Option<i32> = None;
    group.bench_function("Option_i32_None", |b| {
        b.iter(|| {
            let sql_val = black_box(&opt_none).to_sql().unwrap();
            black_box(sql_val)
        })
    });

    group.finish();
}

/// Benchmark FromSql conversions (SqlValue → Rust).
fn bench_from_sql(c: &mut Criterion) {
    let mut group = c.benchmark_group("from_sql");

    // Integer
    let int_sql = SqlValue::Int(12345);
    group.bench_function("i32", |b| {
        b.iter(|| {
            let val: i32 = i32::from_sql(black_box(&int_sql)).unwrap();
            black_box(val)
        })
    });

    // Big integer
    let bigint_sql = SqlValue::BigInt(9876543210i64);
    group.bench_function("i64", |b| {
        b.iter(|| {
            let val: i64 = i64::from_sql(black_box(&bigint_sql)).unwrap();
            black_box(val)
        })
    });

    // Float
    let float_sql = SqlValue::Double(3.14159265358979);
    group.bench_function("f64", |b| {
        b.iter(|| {
            let val: f64 = f64::from_sql(black_box(&float_sql)).unwrap();
            black_box(val)
        })
    });

    // Bool
    let bool_sql = SqlValue::Bool(true);
    group.bench_function("bool", |b| {
        b.iter(|| {
            let val: bool = bool::from_sql(black_box(&bool_sql)).unwrap();
            black_box(val)
        })
    });

    // String
    let string_sql = SqlValue::String("test string value".to_string());
    group.bench_function("String", |b| {
        b.iter(|| {
            let val: String = String::from_sql(black_box(&string_sql)).unwrap();
            black_box(val)
        })
    });

    // Option<i32> - from non-null
    let opt_sql = SqlValue::Int(42);
    group.bench_function("Option_i32_Some", |b| {
        b.iter(|| {
            let val: Option<i32> = Option::<i32>::from_sql(black_box(&opt_sql)).unwrap();
            black_box(val)
        })
    });

    // Option<i32> - from null
    let null_sql = SqlValue::Null;
    group.bench_function("Option_i32_None", |b| {
        b.iter(|| {
            let val: Option<i32> = Option::<i32>::from_sql(black_box(&null_sql)).unwrap();
            black_box(val)
        })
    });

    group.finish();
}

/// Benchmark SqlValue creation and pattern matching.
fn bench_sql_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_value");

    // Creating various SqlValue types
    group.bench_function("create_int", |b| b.iter(|| black_box(SqlValue::Int(12345))));

    group.bench_function("create_string", |b| {
        b.iter(|| black_box(SqlValue::String("test".to_string())))
    });

    group.bench_function("create_null", |b| b.iter(|| black_box(SqlValue::Null)));

    // Pattern matching
    let values = [
        SqlValue::Int(1),
        SqlValue::String("test".to_string()),
        SqlValue::Bool(true),
        SqlValue::Null,
        SqlValue::BigInt(12345),
    ];

    group.bench_function("is_null_check", |b| {
        b.iter(|| {
            let count: usize = values
                .iter()
                .filter(|v| matches!(v, SqlValue::Null))
                .count();
            black_box(count)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_utf16_encode,
    bench_utf16_decode,
    bench_to_sql,
    bench_from_sql,
    bench_sql_value,
);

criterion_main!(benches);
