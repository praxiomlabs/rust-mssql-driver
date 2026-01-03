//! Stress tests for mssql-client.
//!
//! These tests are designed to be run under memory analysis tools like
//! Valgrind or DHAT to detect memory leaks and allocation patterns.
//!
//! Run with: cargo test --test stress_tests -- --ignored --nocapture
//! With Valgrind: cargo valgrind test --test stress_tests -- --ignored
//!
//! For live SQL Server stress tests:
//!   MSSQL_TEST_HOST=localhost MSSQL_TEST_PASSWORD=... cargo test --test stress_tests -- --ignored

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::approx_constant
)]

use mssql_client::Config;
use std::time::Duration;

// =============================================================================
// Config Stress Tests (No SQL Server Required)
// =============================================================================

#[test]
fn stress_config_parsing_repeated() {
    // Parse connection strings repeatedly to check for memory leaks
    let conn_str = "Server=localhost,1433;Database=test;User Id=sa;Password=secret;\
                    Encrypt=true;TrustServerCertificate=true;Connect Timeout=30;\
                    Application Name=StressTest;Command Timeout=60";

    for _ in 0..10_000 {
        let config = Config::from_connection_string(conn_str);
        assert!(config.is_ok());
        drop(config);
    }
}

#[test]
fn stress_config_builder_repeated() {
    // Build configs repeatedly to check for memory leaks using connection strings
    for i in 0..10_000 {
        let conn_str = format!(
            "Server=localhost,1433;Database=db_{};User Id=sa;Password=password;\
             Application Name=app_{};Connect Timeout=30;TrustServerCertificate=true",
            i, i
        );
        let config = Config::from_connection_string(&conn_str);
        assert!(config.is_ok());
        drop(config);
    }
}

#[test]
fn stress_error_creation() {
    use mssql_client::Error;
    use std::sync::Arc;

    // Create many errors to check for memory leaks
    for i in 0i32..10_000 {
        let errors: Vec<Error> = vec![
            Error::Connection(format!("connection error {}", i)),
            Error::ConnectionClosed,
            Error::Tls(format!("tls error {}", i)),
            Error::Protocol(format!("protocol error {}", i)),
            Error::Query(format!("query error {}", i)),
            Error::Server {
                number: i,
                class: 16,
                state: 1,
                message: format!("server error {}", i),
                server: Some("testserver".into()),
                procedure: Some("sp_test".into()),
                line: i as u32,
            },
            Error::Transaction(format!("tx error {}", i)),
            Error::Config(format!("config error {}", i)),
            Error::ConnectTimeout,
            Error::Routing {
                host: format!("host_{}", i),
                port: (i % 65535) as u16,
            },
            Error::Io(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("io error {}", i),
            ))),
        ];

        // Force evaluation
        for err in &errors {
            let _ = err.to_string();
            let _ = err.is_transient();
            let _ = err.is_terminal();
        }

        drop(errors);
    }
}

// =============================================================================
// Type Conversion Stress Tests
// =============================================================================

#[test]
fn stress_type_conversions() {
    use mssql_types::{FromSql, SqlValue};

    for i in 0..10_000 {
        // Create various SQL values
        let values = vec![
            SqlValue::Int(i),
            SqlValue::BigInt(i as i64 * 1_000_000),
            SqlValue::String(format!("string value number {}", i)),
            SqlValue::Double(i as f64 / 1000.0),
            SqlValue::Bool(i % 2 == 0),
            SqlValue::Null,
        ];

        // Convert them
        for value in &values {
            match value {
                SqlValue::Int(_) => {
                    let _: Option<i32> = Option::<i32>::from_sql(value).ok().flatten();
                }
                SqlValue::BigInt(_) => {
                    let _: Option<i64> = Option::<i64>::from_sql(value).ok().flatten();
                }
                SqlValue::String(_) => {
                    let _: Option<String> = Option::<String>::from_sql(value).ok().flatten();
                }
                SqlValue::Double(_) => {
                    let _: Option<f64> = Option::<f64>::from_sql(value).ok().flatten();
                }
                SqlValue::Bool(_) => {
                    let _: Option<bool> = Option::<bool>::from_sql(value).ok().flatten();
                }
                SqlValue::Null => {
                    let _: Option<i32> = Option::<i32>::from_sql(value).ok().flatten();
                }
                _ => {}
            }
        }

        drop(values);
    }
}

// =============================================================================
// Arc<Bytes> Pattern Stress Tests
// =============================================================================

#[test]
fn stress_arc_bytes_clone() {
    use bytes::Bytes;
    use std::sync::Arc;

    // Simulate the Arc<Bytes> pattern used for row data
    let data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
    let arc_bytes: Arc<Bytes> = Arc::new(Bytes::from(data));

    // Clone many times (simulates sharing row data across tasks)
    let mut clones = Vec::with_capacity(1000);
    for _ in 0..1000 {
        clones.push(Arc::clone(&arc_bytes));
    }

    // Access slices
    for (i, clone) in clones.iter().enumerate() {
        let start = i % 9000;
        let end = start + 100;
        let _slice = &clone[start..end];
    }

    drop(clones);
    drop(arc_bytes);
}

#[test]
fn stress_arc_bytes_varying_sizes() {
    use bytes::Bytes;
    use std::sync::Arc;

    for size_exp in 0..15 {
        let size = 1 << size_exp; // 1, 2, 4, ..., 16384
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let arc: Arc<Bytes> = Arc::new(Bytes::from(data));

        // Clone and access
        for _ in 0..100 {
            let clone = Arc::clone(&arc);
            let _len = clone.len();
            drop(clone);
        }

        drop(arc);
    }
}

// =============================================================================
// Pool Config Stress Tests
// =============================================================================

#[test]
fn stress_pool_config_creation() {
    use mssql_driver_pool::PoolConfig;

    for i in 0..10_000 {
        let config = PoolConfig::new()
            .min_connections((i % 10) as u32)
            .max_connections(((i % 90) + 10) as u32)
            .connection_timeout(Duration::from_millis((i % 30000).max(1) as u64))
            .idle_timeout(Duration::from_secs((i % 3600) as u64))
            .max_lifetime(Duration::from_secs((i % 7200) as u64));

        drop(config);
    }
}

// =============================================================================
// Live Database Stress Tests (Require SQL Server)
// =============================================================================

fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_TEST_HOST").ok()?;
    let port = std::env::var("MSSQL_TEST_PORT").unwrap_or_else(|_| "1433".into());
    let user = std::env::var("MSSQL_TEST_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_TEST_PASSWORD").ok()?;

    let conn_str = format!(
        "Server={},{};Database=master;User Id={};Password={};TrustServerCertificate=true",
        host, port, user, password
    );

    Config::from_connection_string(&conn_str).ok()
}

#[tokio::test]
#[ignore = "Requires SQL Server - Long running stress test"]
async fn stress_connection_cycle() {
    use mssql_client::Client;

    let base_config = get_test_config().expect("SQL Server config required");

    // Connect and disconnect repeatedly
    for i in 0..100 {
        let config = base_config.clone();
        match Client::connect(config).await {
            Ok(client) => {
                let _ = client.close().await;
            }
            Err(e) => {
                panic!("Connection {} failed: {}", i, e);
            }
        }
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server - Long running stress test"]
async fn stress_query_cycle() {
    use mssql_client::Client;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Run many queries
    for i in 0..1000 {
        let result = client
            .query(
                &format!("SELECT {} AS num, 'iteration {}' AS txt", i, i),
                &[],
            )
            .await;

        match result {
            Ok(rows) => {
                // Consume rows
                for row_result in rows {
                    let _row = row_result.expect("Row error");
                }
            }
            Err(e) => {
                panic!("Query {} failed: {}", i, e);
            }
        }
    }

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires SQL Server - Long running stress test"]
async fn stress_pool_acquire_release() {
    use mssql_driver_pool::{Pool, PoolConfig};

    let config = get_test_config().expect("SQL Server config required");

    let pool_config = PoolConfig::new()
        .min_connections(2)
        .max_connections(5)
        .connection_timeout(Duration::from_secs(30));

    let pool = Pool::new(pool_config, config)
        .await
        .expect("Pool creation failed");

    // Acquire and release many times
    for i in 0..500 {
        match pool.get().await {
            Ok(mut conn) => {
                // Quick query
                let _ = conn.query(&format!("SELECT {}", i), &[]).await;
                // Connection returned on drop
            }
            Err(e) => {
                panic!("Pool get {} failed: {}", i, e);
            }
        }
    }

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server - Long running stress test"]
async fn stress_concurrent_pool_usage() {
    use mssql_driver_pool::{Pool, PoolConfig};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let config = get_test_config().expect("SQL Server config required");

    let pool_config = PoolConfig::new()
        .min_connections(2)
        .max_connections(10)
        .connection_timeout(Duration::from_secs(60));

    let pool: Arc<Pool> = Arc::new(
        Pool::new(pool_config, config)
            .await
            .expect("Pool creation failed"),
    );

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    // Spawn many concurrent tasks
    let mut handles = Vec::new();
    for task_id in 0..50 {
        let pool: Arc<Pool> = Arc::clone(&pool);
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);

        handles.push(tokio::spawn(async move {
            for query_id in 0..20 {
                match pool.get().await {
                    Ok(mut conn) => {
                        let query = format!("SELECT {} + {}", task_id, query_id);
                        match conn.query(&query, &[]).await {
                            Ok(rows) => {
                                // Consume all rows
                                for _ in rows {}
                                success.fetch_add(1, Ordering::SeqCst);
                            }
                            Err(_) => {
                                errors.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        }));
    }

    // Wait for all tasks
    for handle in handles {
        let _ = handle.await;
    }

    let successes = success_count.load(Ordering::SeqCst);
    let errors = error_count.load(Ordering::SeqCst);

    println!(
        "Concurrent pool stress: {} successes, {} errors",
        successes, errors
    );

    // Most should succeed
    assert!(
        successes > 900,
        "Expected > 900 successes, got {}",
        successes
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server - Long running stress test"]
async fn stress_transaction_cycle() {
    use mssql_driver_pool::{Pool, PoolConfig};

    let config = get_test_config().expect("SQL Server config required");

    // Use a pool for transaction stress testing to handle type-state properly
    let pool_config = PoolConfig::new()
        .min_connections(1)
        .max_connections(5)
        .connection_timeout(Duration::from_secs(30));

    let pool = Pool::new(pool_config, config)
        .await
        .expect("Pool creation failed");

    // Create test table using a connection from the pool
    {
        let mut conn = pool.get().await.expect("Get connection failed");
        let _ = conn
            .execute(
                "IF OBJECT_ID('tempdb..#stress_test') IS NOT NULL DROP TABLE #stress_test",
                &[],
            )
            .await;
        conn.execute(
            "CREATE TABLE #stress_test (id INT, value NVARCHAR(100))",
            &[],
        )
        .await
        .expect("Create table failed");
    }

    // Run many transactions using pool connections
    for i in 0..100 {
        let mut conn = pool.get().await.expect("Get connection failed");

        // Execute transaction using explicit SQL commands
        // This avoids the type-state complexity for stress testing
        conn.execute("BEGIN TRANSACTION", &[])
            .await
            .expect("Begin failed");

        // Insert
        conn.execute(
            &format!(
                "INSERT INTO #stress_test (id, value) VALUES ({}, 'value_{}')",
                i, i
            ),
            &[],
        )
        .await
        .expect("Insert failed");

        // Commit or rollback
        if i % 3 == 0 {
            conn.execute("ROLLBACK TRANSACTION", &[])
                .await
                .expect("Rollback failed");
        } else {
            conn.execute("COMMIT TRANSACTION", &[])
                .await
                .expect("Commit failed");
        }
    }

    pool.close().await;
}
