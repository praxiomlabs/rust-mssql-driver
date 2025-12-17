//! Connection pool integration tests.
//!
//! These tests require a running SQL Server instance. They are ignored by default
//! and can be run with:
//!
//! ```bash
//! # Set connection details via environment variables
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourPassword
//! export MSSQL_ENCRYPT=false  # For development servers without TLS
//!
//! # Run integration tests
//! cargo test -p mssql-driver-pool --test integration -- --ignored
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mssql_client::Config;
use mssql_driver_pool::{Pool, PoolError};

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Basic Pool Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_create_and_close() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    assert!(!pool.is_closed());

    let status = pool.status();
    assert_eq!(status.max, 5);
    assert_eq!(status.in_use, 0);

    pool.close().await;
    assert!(pool.is_closed());
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_get_connection() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    // Get a connection
    let mut conn = pool.get().await.expect("Failed to get connection");

    // Verify pool status
    let status = pool.status();
    assert_eq!(status.in_use, 1);

    // Execute a query to verify connection works
    let rows = conn
        .query("SELECT 1 AS value", &[])
        .await
        .expect("Query failed");

    let values: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(values, vec![1]);

    // Drop connection - should return to pool
    drop(conn);

    let status = pool.status();
    assert_eq!(status.in_use, 0);
    assert_eq!(status.available, 1);

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_connection_reuse() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(2)
        .build()
        .await
        .expect("Failed to create pool");

    // Get first connection
    let conn1 = pool.get().await.expect("Failed to get connection 1");
    let id1 = conn1.metadata().id;
    drop(conn1);

    // Get another connection - should reuse the same one
    let conn2 = pool.get().await.expect("Failed to get connection 2");
    let id2 = conn2.metadata().id;

    assert_eq!(id1, id2, "Should reuse the same connection");

    drop(conn2);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_try_get_with_idle_connection() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    // First get a connection normally to create one
    let conn = pool.get().await.expect("Failed to get connection");
    drop(conn);

    // Now try_get should succeed (there's an idle connection)
    let conn = pool
        .try_get()
        .expect("try_get should succeed")
        .expect("Should get an idle connection");

    // Verify it works
    let mut conn = conn;
    let rows = conn
        .query("SELECT 42 AS answer", &[])
        .await
        .expect("Query failed");

    let values: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(values, vec![42]);

    drop(conn);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_try_get_no_idle_connections() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(1)
        .build()
        .await
        .expect("Failed to create pool");

    // Get the only connection
    let conn = pool.get().await.expect("Failed to get connection");

    // try_get should return None - no idle connections available
    let result = pool.try_get().expect("try_get should not error");
    assert!(result.is_none(), "Should return None when no idle connections");

    drop(conn);
    pool.close().await;
}

// =============================================================================
// Concurrent Access Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_concurrent_access() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(5)
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let success_count = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    // Spawn 10 concurrent tasks
    for i in 0..10 {
        let pool = pool.clone();
        let success_count = success_count.clone();

        handles.push(tokio::spawn(async move {
            let mut conn = pool.get().await.expect("Failed to get connection");

            // Execute a query
            let rows = conn
                .query(&format!("SELECT {} AS task_id", i), &[])
                .await
                .expect("Query failed");

            let values: Vec<i32> = rows
                .filter_map(|r| r.ok())
                .map(|row| row.get(0).unwrap())
                .collect();

            assert_eq!(values, vec![i]);
            success_count.fetch_add(1, Ordering::Relaxed);
        }));
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    assert_eq!(success_count.load(Ordering::Relaxed), 10);

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_concurrent_stress_test() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(10)
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let success_count = Arc::new(AtomicU32::new(0));
    let error_count = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    // Spawn 50 concurrent tasks with 10 connections
    for i in 0..50 {
        let pool = pool.clone();
        let success_count = success_count.clone();
        let error_count = error_count.clone();

        handles.push(tokio::spawn(async move {
            match pool.get().await {
                Ok(mut conn) => {
                    // Execute multiple queries per connection
                    for j in 0..3 {
                        let sql = format!("SELECT {} + {} AS sum", i, j);
                        match conn.query(&sql, &[]).await {
                            Ok(rows) => {
                                let values: Vec<i32> = rows
                                    .filter_map(|r| r.ok())
                                    .map(|row| row.get(0).unwrap())
                                    .collect();

                                assert_eq!(values, vec![i + j]);
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => {
                                eprintln!("Query {} failed: {:?}", i, e);
                                error_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get connection {}: {:?}", i, e);
                    error_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let successes = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);

    println!("Stress test results: {} successes, {} errors", successes, errors);

    // We expect all 150 queries (50 tasks * 3 queries each) to succeed
    assert_eq!(successes, 150, "All queries should succeed");
    assert_eq!(errors, 0, "No errors should occur");

    pool.close().await;
}

// =============================================================================
// Pool Metrics Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_metrics() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    // Get and return a few connections
    for _ in 0..5 {
        let conn = pool.get().await.expect("Failed to get connection");
        drop(conn);
    }

    let metrics = pool.metrics();

    // Should have at least 1 connection created (may reuse for subsequent gets)
    assert!(metrics.connections_created >= 1, "Should have created at least 1 connection");

    // All checkouts should be successful
    assert!(metrics.checkouts_successful >= 5, "Should have at least 5 successful checkouts");
    assert_eq!(metrics.checkouts_failed, 0, "No checkouts should have failed");

    // Checkout success rate should be 100%
    assert!((metrics.checkout_success_rate() - 1.0).abs() < f64::EPSILON);

    println!("Metrics: {:?}", metrics);

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_status_tracking() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    // Initial status
    let status = pool.status();
    assert_eq!(status.in_use, 0);
    assert_eq!(status.available, 0);
    assert_eq!(status.total, 0);

    // Get a connection
    let conn1 = pool.get().await.expect("Failed to get connection");
    let status = pool.status();
    assert_eq!(status.in_use, 1);
    assert_eq!(status.total, 1);

    // Get another connection
    let conn2 = pool.get().await.expect("Failed to get connection");
    let status = pool.status();
    assert_eq!(status.in_use, 2);

    // Return first connection
    drop(conn1);
    let status = pool.status();
    assert_eq!(status.in_use, 1);
    assert_eq!(status.available, 1);

    // Return second connection
    drop(conn2);
    let status = pool.status();
    assert_eq!(status.in_use, 0);
    assert_eq!(status.available, 2);

    pool.close().await;
}

// =============================================================================
// Timeout and Error Handling Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_connection_timeout() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(1)
        .connection_timeout(Duration::from_millis(100))
        .build()
        .await
        .expect("Failed to create pool");

    // Hold the only connection
    let _conn = pool.get().await.expect("Failed to get connection");

    // Try to get another connection - should timeout
    let result = pool.get().await;
    assert!(matches!(result, Err(PoolError::Timeout)), "Should timeout waiting for connection");

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_closed_error() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    // Close the pool
    pool.close().await;

    // Try to get a connection - should fail
    let result = pool.get().await;
    assert!(matches!(result, Err(PoolError::PoolClosed)), "Should error when pool is closed");

    // try_get should also fail
    let result = pool.try_get();
    assert!(matches!(result, Err(PoolError::PoolClosed)), "try_get should error when pool is closed");
}

// =============================================================================
// Load/Stress Tests (TEST-010)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_high_throughput() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(10)
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let query_count = Arc::new(AtomicU32::new(0));
    let start = std::time::Instant::now();

    // Run for 5 seconds with as many queries as possible
    let runtime = Duration::from_secs(5);
    let mut handles = Vec::new();

    for _ in 0..20 {
        let pool = pool.clone();
        let query_count = query_count.clone();
        let start = start.clone();

        handles.push(tokio::spawn(async move {
            while start.elapsed() < runtime {
                if let Ok(mut conn) = pool.get().await {
                    if conn.query("SELECT 1", &[]).await.is_ok() {
                        query_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    // Wait for all workers
    for handle in handles {
        let _ = handle.await;
    }

    let total_queries = query_count.load(Ordering::Relaxed);
    let elapsed = start.elapsed();
    let qps = total_queries as f64 / elapsed.as_secs_f64();

    println!(
        "High throughput test: {} queries in {:?} ({:.2} queries/second)",
        total_queries, elapsed, qps
    );

    // Should be able to handle at least 100 queries per second (conservative)
    assert!(qps >= 100.0, "Should achieve at least 100 queries/second, got {}", qps);

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_connection_churn() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(5)
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let success_count = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    // Rapid connection checkout/checkin pattern
    for i in 0..100 {
        let pool = pool.clone();
        let success_count = success_count.clone();

        handles.push(tokio::spawn(async move {
            // Get connection, execute query, return
            if let Ok(mut conn) = pool.get().await {
                let sql = format!("SELECT {} AS iteration", i);
                if conn.query(&sql, &[]).await.is_ok() {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let successes = success_count.load(Ordering::Relaxed);
    assert_eq!(successes, 100, "All 100 iterations should succeed");

    // Verify metrics after churn
    let metrics = pool.metrics();
    println!("Connection churn metrics: {:?}", metrics);

    // Should have reused connections heavily (not created 100 connections)
    assert!(
        metrics.connections_created <= 5,
        "Should reuse connections, not create 100. Created: {}",
        metrics.connections_created
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_sustained_load() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(8)
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let success_count = Arc::new(AtomicU32::new(0));
    let error_count = Arc::new(AtomicU32::new(0));

    // Run sustained load: 16 concurrent workers, each doing 50 queries
    let mut handles = Vec::new();

    for worker_id in 0..16 {
        let pool = pool.clone();
        let success_count = success_count.clone();
        let error_count = error_count.clone();

        handles.push(tokio::spawn(async move {
            for query_id in 0..50 {
                match pool.get().await {
                    Ok(mut conn) => {
                        let sql = format!(
                            "SELECT {} * 100 + {} AS id",
                            worker_id, query_id
                        );
                        match conn.query(&sql, &[]).await {
                            Ok(rows) => {
                                let values: Vec<i32> = rows
                                    .filter_map(|r| r.ok())
                                    .map(|row| row.get(0).unwrap())
                                    .collect();

                                let expected = worker_id * 100 + query_id;
                                if values == vec![expected] {
                                    success_count.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    error_count.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Err(_) => {
                                error_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Worker panicked");
    }

    let successes = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);

    println!(
        "Sustained load test: {} successes, {} errors (total: {})",
        successes,
        errors,
        successes + errors
    );

    // All 800 queries (16 workers * 50 queries) should succeed
    assert_eq!(successes, 800, "All 800 queries should succeed");
    assert_eq!(errors, 0, "No errors should occur");

    let metrics = pool.metrics();
    println!("Final pool metrics: {:?}", metrics);

    pool.close().await;
}

// =============================================================================
// Connection Detach Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_detach_connection() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(2)
        .build()
        .await
        .expect("Failed to create pool");

    // Get and detach a connection
    let conn = pool.get().await.expect("Failed to get connection");
    let client = conn.detach().expect("Should detach client");

    // Connection should not be in pool anymore
    let status = pool.status();
    assert_eq!(status.in_use, 0, "Detached connection should not count as in_use");

    // But the client should still work
    let mut client = client;
    let rows = client.query("SELECT 999 AS detached", &[]).await.expect("Query should work");
    let values: Vec<i32> = rows.filter_map(|r| r.ok()).map(|row| row.get(0).unwrap()).collect();
    assert_eq!(values, vec![999]);

    // Clean up the detached client manually
    client.close().await.expect("Failed to close detached client");

    pool.close().await;
}
