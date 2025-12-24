//! Benchmarks for mssql-client row access patterns and configuration.
//!
//! Tests the performance of the `Arc<Bytes>` pattern from ADR-004.

#![allow(missing_docs, clippy::unwrap_used, clippy::approx_constant)]

use bytes::Bytes;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mssql_client::Config;
use std::hint::black_box;
use std::sync::Arc;

// Re-export internal types for benchmarking
// Note: These would need to be exposed for benchmarking, or we use the public API
use mssql_types::{FromSql, SqlValue};

/// Benchmark connection string parsing - a common hot path in application startup.
fn bench_connection_string_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_string");

    // Simple connection string
    let simple = "Server=localhost;Database=test;User Id=sa;Password=secret;";
    group.throughput(Throughput::Bytes(simple.len() as u64));
    group.bench_function("simple", |b| {
        b.iter(|| {
            let config = Config::from_connection_string(black_box(simple));
            black_box(config)
        })
    });

    // Connection string with port
    let with_port = "Server=localhost,1434;Database=test;User Id=sa;Password=secret;";
    group.throughput(Throughput::Bytes(with_port.len() as u64));
    group.bench_function("with_port", |b| {
        b.iter(|| {
            let config = Config::from_connection_string(black_box(with_port));
            black_box(config)
        })
    });

    // Connection string with named instance
    let with_instance = "Server=localhost\\SQLEXPRESS;Database=test;User Id=sa;Password=secret;";
    group.throughput(Throughput::Bytes(with_instance.len() as u64));
    group.bench_function("with_instance", |b| {
        b.iter(|| {
            let config = Config::from_connection_string(black_box(with_instance));
            black_box(config)
        })
    });

    // Full Azure-style connection string
    let azure = "Server=myserver.database.windows.net;Database=mydb;\
                 User Id=admin@myserver;Password=VeryStrongP@ssw0rd!;\
                 Encrypt=strict;TrustServerCertificate=false;\
                 Connect Timeout=30;Application Name=MyApp;";
    group.throughput(Throughput::Bytes(azure.len() as u64));
    group.bench_function("azure_full", |b| {
        b.iter(|| {
            let config = Config::from_connection_string(black_box(azure));
            black_box(config)
        })
    });

    group.finish();
}

/// Benchmark FromSql trait conversions - common operations when reading rows.
fn bench_from_sql_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("from_sql");

    // Integer conversion
    let int_value = SqlValue::Int(42);
    group.bench_function("i32_from_int", |b| {
        b.iter(|| {
            let result: Result<i32, _> = i32::from_sql(black_box(&int_value));
            black_box(result)
        })
    });

    // BigInt conversion
    let bigint_value = SqlValue::BigInt(9_876_543_210);
    group.bench_function("i64_from_bigint", |b| {
        b.iter(|| {
            let result: Result<i64, _> = i64::from_sql(black_box(&bigint_value));
            black_box(result)
        })
    });

    // String conversion
    let string_value = SqlValue::String("Hello, World! This is a test string.".to_string());
    group.bench_function("string_from_string", |b| {
        b.iter(|| {
            let result: Result<String, _> = String::from_sql(black_box(&string_value));
            black_box(result)
        })
    });

    // Option<i32> from non-null
    group.bench_function("option_i32_some", |b| {
        b.iter(|| {
            let result: Result<Option<i32>, _> = Option::<i32>::from_sql(black_box(&int_value));
            black_box(result)
        })
    });

    // Option<i32> from null
    let null_value = SqlValue::Null;
    group.bench_function("option_i32_none", |b| {
        b.iter(|| {
            let result: Result<Option<i32>, _> = Option::<i32>::from_sql(black_box(&null_value));
            black_box(result)
        })
    });

    // Float conversion
    let float_value = SqlValue::Double(3.14159265358979);
    group.bench_function("f64_from_double", |b| {
        b.iter(|| {
            let result: Result<f64, _> = f64::from_sql(black_box(&float_value));
            black_box(result)
        })
    });

    // Bool conversion
    let bool_value = SqlValue::Bool(true);
    group.bench_function("bool_from_bool", |b| {
        b.iter(|| {
            let result: Result<bool, _> = bool::from_sql(black_box(&bool_value));
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark Arc<Bytes> buffer operations - the zero-copy pattern from ADR-004.
fn bench_arc_bytes_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_bytes");

    // Small buffer (typical single column)
    let small_data = b"Hello World";
    let small_arc: Arc<Bytes> = Arc::new(Bytes::from_static(small_data));
    group.bench_function("clone_small", |b| {
        b.iter(|| {
            let cloned = Arc::clone(black_box(&small_arc));
            black_box(cloned)
        })
    });

    // Medium buffer (typical row)
    let medium_data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let medium_arc: Arc<Bytes> = Arc::new(Bytes::from(medium_data));
    group.bench_function("clone_medium", |b| {
        b.iter(|| {
            let cloned = Arc::clone(black_box(&medium_arc));
            black_box(cloned)
        })
    });

    // Large buffer (batch result)
    let large_data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    let large_arc: Arc<Bytes> = Arc::new(Bytes::from(large_data));
    group.bench_function("clone_large", |b| {
        b.iter(|| {
            let cloned = Arc::clone(black_box(&large_arc));
            black_box(cloned)
        })
    });

    // Slice access (zero-copy)
    group.bench_function("slice_medium", |b| {
        b.iter(|| {
            let slice = &medium_arc[100..200];
            black_box(slice)
        })
    });

    group.finish();
}

/// Benchmark config builder pattern - used during connection setup.
fn bench_config_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_builder");

    // Minimal config
    group.bench_function("minimal", |b| {
        b.iter(|| {
            let config = Config::new().host("localhost").database("test");
            black_box(config)
        })
    });

    // Full config with all options
    group.bench_function("full", |b| {
        b.iter(|| {
            use std::time::Duration;
            let config = Config::new()
                .host("myserver.database.windows.net")
                .port(1433)
                .database("mydb")
                .application_name("benchmark")
                .connect_timeout(Duration::from_secs(30))
                .trust_server_certificate(false)
                .strict_mode(true)
                .max_redirects(3)
                .max_retries(5);
            black_box(config)
        })
    });

    group.finish();
}

/// Benchmark SqlValue creation and matching - common in row processing.
fn bench_sql_value_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_value");

    // Creating various SqlValue types
    group.bench_function("create_int", |b| b.iter(|| black_box(SqlValue::Int(42))));

    group.bench_function("create_bigint", |b| {
        b.iter(|| black_box(SqlValue::BigInt(9_876_543_210)))
    });

    group.bench_function("create_string", |b| {
        b.iter(|| black_box(SqlValue::String("test value".to_string())))
    });

    group.bench_function("create_null", |b| b.iter(|| black_box(SqlValue::Null)));

    // Pattern matching for null checks
    let values = [
        SqlValue::Int(1),
        SqlValue::Null,
        SqlValue::String("test".to_string()),
        SqlValue::Null,
        SqlValue::BigInt(100),
    ];

    group.bench_function("null_check_iter", |b| {
        b.iter(|| {
            let count: usize = values
                .iter()
                .filter(|v| matches!(v, SqlValue::Null))
                .count();
            black_box(count)
        })
    });

    // is_null method check
    let value = SqlValue::Int(42);
    group.bench_function("is_null_check", |b| {
        b.iter(|| {
            let is_null = black_box(&value).is_null();
            black_box(is_null)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_connection_string_parsing,
    bench_from_sql_conversions,
    bench_arc_bytes_operations,
    bench_config_builder,
    bench_sql_value_operations,
);

criterion_main!(benches);
