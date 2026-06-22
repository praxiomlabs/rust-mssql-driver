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

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::redundant_locals)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use mssql_client::Config;
use mssql_driver_pool::{Pool, PoolError};
use tokio::time::timeout;

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host};Database={database};User Id={user};Password={password};TrustServerCertificate=true;Encrypt={encrypt}"
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

    // Warm the pool so there are idle connections to drain.
    {
        let _c1 = pool.get().await.expect("checkout 1");
        let _c2 = pool.get().await.expect("checkout 2");
    } // returned to idle here
    assert!(pool.status().available > 0, "pool should have idle conns");

    pool.close().await;
    assert!(pool.is_closed());

    // close() must actually drain the idle connections, not just flip a flag.
    assert_eq!(
        pool.status().available,
        0,
        "close() must drain idle connections"
    );

    // And new checkouts must be refused.
    assert!(
        matches!(pool.get().await, Err(PoolError::PoolClosed)),
        "checkout after close must fail with PoolClosed"
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_close_discards_in_use_connection_on_return() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .build()
        .await
        .expect("Failed to create pool");

    // Hold a connection across close(), then return it.
    let conn = pool.get().await.expect("checkout");
    pool.close().await;
    assert_eq!(pool.status().in_use, 1, "still in use during close");

    drop(conn); // returned after close — must be discarded, not re-pooled

    let status = pool.status();
    assert_eq!(status.in_use, 0, "in_use decremented on return");
    assert_eq!(
        status.available, 0,
        "connection returned after close must be discarded, not idle"
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_close_under_concurrent_churn_does_not_deadlock() {
    // Robustness smoke test, NOT a race-leak detector. The return-vs-close
    // leak window is a few microseconds inside `Drop`, too narrow to hit
    // reliably from a test — verified empirically: an `available == 0`
    // assertion here passes even against the pre-fix (racy) code, so it would
    // be false confidence. This asserts only liveness: `close()` racing many
    // concurrent checkouts/returns must not deadlock or panic. The leak
    // property is guaranteed instead by lock ordering — `close()` sets the
    // closed flag and drains the idle queue under the same lock that `Drop`
    // re-checks before re-pooling — and the non-racy path is covered by the
    // two tests above.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(4)
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..50 {
                match pool.get().await {
                    Ok(conn) => {
                        tokio::task::yield_now().await;
                        drop(conn);
                    }
                    Err(_) => break, // PoolClosed once close() lands
                }
            }
        }));
    }

    tokio::task::yield_now().await;
    pool.close().await;

    for handle in handles {
        handle.await.expect("task panicked or deadlocked");
    }
    assert!(pool.is_closed());
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_with_otel_name() {
    // Exercises the OTel metric hooks end-to-end: create, warmup, checkout,
    // checkin, close. When built without the `otel` feature this calls the
    // no-op DatabaseMetrics; with the feature it emits real gauges/counters/
    // histograms. This test only verifies the hooks don't panic on any path.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .min_connections(1)
        .max_connections(3)
        .pool_name("integration-test-pool")
        .build()
        .await
        .expect("Failed to create pool");

    // Checkout/checkin cycle — exercises record_connection_wait + record_pool_status.
    let mut conn = pool.get().await.expect("Failed to get connection");
    let rows = conn.query("SELECT 1", &[]).await.expect("Query failed");
    for _ in rows {}
    drop(conn);

    // Second checkout exercises idle reuse path.
    let conn2 = pool.get().await.expect("Failed to get connection");
    drop(conn2);

    pool.close().await;
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
    assert!(
        result.is_none(),
        "Should return None when no idle connections"
    );

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
                .query(&format!("SELECT {i} AS task_id"), &[])
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
                        let sql = format!("SELECT {i} + {j} AS sum");
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
                                eprintln!("Query {i} failed: {e:?}");
                                error_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get connection {i}: {e:?}");
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

    println!("Stress test results: {successes} successes, {errors} errors");

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
    assert!(
        metrics.connections_created >= 1,
        "Should have created at least 1 connection"
    );

    // All checkouts should be successful
    assert!(
        metrics.checkouts_successful >= 5,
        "Should have at least 5 successful checkouts"
    );
    assert_eq!(
        metrics.checkouts_failed, 0,
        "No checkouts should have failed"
    );

    // Checkout success rate should be 100%
    assert!((metrics.checkout_success_rate() - 1.0).abs() < f64::EPSILON);

    println!("Metrics: {metrics:?}");

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_status_tracking() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(5)
        .min_connections(0)
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
    assert!(
        matches!(result, Err(PoolError::Timeout { .. })),
        "Should timeout waiting for connection"
    );

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
    assert!(
        matches!(result, Err(PoolError::PoolClosed)),
        "Should error when pool is closed"
    );

    // try_get should also fail
    let result = pool.try_get();
    assert!(
        matches!(result, Err(PoolError::PoolClosed)),
        "try_get should error when pool is closed"
    );
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
        let start = start;

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
        "High throughput test: {total_queries} queries in {elapsed:?} ({qps:.2} queries/second)"
    );

    // Should be able to handle at least 100 queries per second (conservative)
    assert!(
        qps >= 100.0,
        "Should achieve at least 100 queries/second, got {qps}"
    );

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
                let sql = format!("SELECT {i} AS iteration");
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
    println!("Connection churn metrics: {metrics:?}");

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
                        let sql = format!("SELECT {worker_id} * 100 + {query_id} AS id");
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
    println!("Final pool metrics: {metrics:?}");

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
    assert_eq!(
        status.in_use, 0,
        "Detached connection should not count as in_use"
    );

    // But the client should still work
    let mut client = client;
    let rows = client
        .query("SELECT 999 AS detached", &[])
        .await
        .expect("Query should work");
    let values: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();
    assert_eq!(values, vec![999]);

    // Clean up the detached client manually
    client
        .close()
        .await
        .expect("Failed to close detached client");

    pool.close().await;
}

// =============================================================================
// Deadlock Detection Tests (TEST-015)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_no_deadlock_under_contention() {
    // Test that the pool doesn't deadlock when many tasks compete for few connections.
    // This is a common failure mode for pool implementations.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(2) // Intentionally small to increase contention
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let success_count = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    // 20 tasks competing for 2 connections
    for i in 0..20 {
        let pool = pool.clone();
        let success_count = success_count.clone();

        handles.push(tokio::spawn(async move {
            // Each task does multiple operations
            for j in 0..5 {
                let mut conn = pool
                    .get()
                    .await
                    .expect("Should get connection without deadlock");

                // Simulate some work
                let sql = format!("SELECT {i} + {j} AS result");
                let _ = conn.query(&sql, &[]).await;

                // Important: Drop connection before getting next one
                drop(conn);
            }
            success_count.fetch_add(1, Ordering::Relaxed);
        }));
    }

    // Use a timeout to detect deadlocks
    let timeout_result = tokio::time::timeout(Duration::from_secs(60), async {
        for handle in handles {
            handle.await.expect("Task panicked");
        }
    })
    .await;

    assert!(
        timeout_result.is_ok(),
        "Pool deadlocked - tasks did not complete within timeout"
    );
    assert_eq!(
        success_count.load(Ordering::Relaxed),
        20,
        "All tasks should complete"
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_no_deadlock_nested_acquisition() {
    // Test that attempting nested connection acquisition (getting a second connection
    // while holding one) doesn't cause deadlock with proper timeouts.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(1) // Single connection to force contention
            .connection_timeout(Duration::from_millis(500)) // Short timeout
            .build()
            .await
            .expect("Failed to create pool"),
    );

    // Get the only connection
    let _conn1 = pool.get().await.expect("First connection should succeed");

    // Trying to get another should timeout, not deadlock
    let result = pool.get().await;
    assert!(
        matches!(result, Err(PoolError::Timeout { .. })),
        "Should timeout, not deadlock"
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_semaphore_fairness() {
    // Test that connection acquisition is fair under contention.
    // Tasks waiting longer should get connections before newer tasks.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(1)
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Start 5 tasks that will queue up
    let mut handles = Vec::new();
    for i in 0..5 {
        let pool = pool.clone();
        let order = order.clone();

        handles.push(tokio::spawn(async move {
            // Small delay so tasks start in sequence
            tokio::time::sleep(Duration::from_millis(i as u64 * 50)).await;

            let mut conn = pool.get().await.expect("Should get connection");

            // Record when this task got the connection
            order.lock().unwrap().push(i);

            // Hold connection briefly
            let _ = conn.query("SELECT 1", &[]).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }));
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let final_order = order.lock().unwrap().clone();
    println!("Connection acquisition order: {final_order:?}");

    // First task should always be first (it started first and no queue yet)
    assert_eq!(final_order[0], 0, "First task should get connection first");

    // The order should generally be sequential (FIFO) due to semaphore fairness
    // Allow some flexibility for timing variations
    let mut inversions = 0;
    for i in 1..final_order.len() {
        if final_order[i] < final_order[i - 1] {
            inversions += 1;
        }
    }

    // Tokio's semaphore is FIFO, so we expect very few inversions (ideally 0)
    assert!(
        inversions <= 1,
        "Too many order inversions ({inversions}), semaphore may not be fair"
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_rapid_acquire_release_no_deadlock() {
    // Rapidly acquire and release connections to stress the synchronization primitives.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(4)
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let iterations = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    // 8 workers doing rapid acquire/release
    for _ in 0..8 {
        let pool = pool.clone();
        let iterations = iterations.clone();

        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let conn = pool.get().await.expect("Should get connection");
                // Immediately release
                drop(conn);
                iterations.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    let timeout_result = tokio::time::timeout(Duration::from_secs(30), async {
        for handle in handles {
            handle.await.expect("Task panicked");
        }
    })
    .await;

    assert!(
        timeout_result.is_ok(),
        "Rapid acquire/release caused deadlock"
    );
    assert_eq!(
        iterations.load(Ordering::Relaxed),
        800,
        "All 800 iterations should complete"
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_mixed_operations_no_deadlock() {
    // Mix of get(), try_get(), queries, and drops to stress all code paths.
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Arc::new(
        Pool::builder()
            .client_config(client_config)
            .max_connections(3)
            .connection_timeout(Duration::from_secs(30))
            .build()
            .await
            .expect("Failed to create pool"),
    );

    let success_count = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    for i in 0..10 {
        let pool = pool.clone();
        let success_count = success_count.clone();

        handles.push(tokio::spawn(async move {
            for j in 0..20 {
                // Alternate between get() and try_get()
                if (i + j) % 3 == 0 {
                    // try_get - may return None
                    if let Ok(Some(mut conn)) = pool.try_get() {
                        let _ = conn.query("SELECT 1", &[]).await;
                    }
                } else {
                    // Regular get
                    if let Ok(mut conn) = pool.get().await {
                        let _ = conn.query("SELECT 1", &[]).await;
                    }
                }
            }
            success_count.fetch_add(1, Ordering::Relaxed);
        }));
    }

    let timeout_result = tokio::time::timeout(Duration::from_secs(60), async {
        for handle in handles {
            handle.await.expect("Task panicked");
        }
    })
    .await;

    assert!(timeout_result.is_ok(), "Mixed operations caused deadlock");
    assert_eq!(
        success_count.load(Ordering::Relaxed),
        10,
        "All workers should complete"
    );

    pool.close().await;
}

// =============================================================================
// Cancel Safety Tests
// =============================================================================

/// Test that dropping a query future mid-flight marks the connection dirty
/// so the pool discards it instead of returning it for reuse.
///
/// This validates the in_flight tracking added for cancel safety: when a
/// tokio::select! or timeout drops a query future, the partially-consumed
/// TCP stream cannot be reused. The pool must discard it and create a fresh
/// connection on the next checkout.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_cancel_safety_pool_discards_inflight_connection() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(2)
        .min_connections(0)
        .connection_timeout(Duration::from_secs(10))
        .build()
        .await
        .expect("Failed to create pool");

    // Phase 1: Start a long-running query and cancel it via timeout.
    // This drops the PooledConnection while a response is in-flight.
    {
        let mut conn = pool.get().await.expect("Failed to checkout connection");

        // WAITFOR DELAY runs for 30 seconds; we timeout after 500ms.
        // This ensures the query future is dropped mid-flight.
        let result = timeout(
            Duration::from_millis(500),
            conn.query("WAITFOR DELAY '00:00:30'; SELECT 1 AS val", &[]),
        )
        .await;

        assert!(result.is_err(), "Query should be cancelled by timeout");

        // `conn` is dropped here. Because in_flight is true, the pool
        // should discard this connection rather than returning it.
    }

    // Brief pause to let the pool process the returned connection.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Phase 2: Checkout a fresh connection and verify it works.
    // If the dirty connection were reused, this query would fail or
    // return garbage data from the partially-consumed response.
    let mut conn2 = pool
        .get()
        .await
        .expect("Failed to checkout second connection");

    let rows = conn2
        .query("SELECT 42 AS answer", &[])
        .await
        .expect("Query on fresh connection should succeed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 1);
    let answer: i32 = data[0].get(0).expect("Should get integer value");
    assert_eq!(answer, 42, "Fresh connection should return correct data");

    pool.close().await;
}

// =============================================================================
// Permit Accounting (issue #151)
// =============================================================================

/// Regression test for #151: the reaper called `add_permits` when evicting
/// idle connections, but idle connections hold no permits (permits are owned
/// by checkouts and released on checkin), so every reap permanently inflated
/// capacity past `max_connections`.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_reaper_does_not_inflate_max_connections() {
    let client_config = get_test_config().expect("SQL Server config required");

    let pool_config = mssql_driver_pool::PoolConfig::new()
        .min_connections(0)
        .max_connections(2)
        .connection_timeout(Duration::from_millis(500))
        .idle_timeout(Duration::from_millis(100))
        .test_on_checkout(false)
        .health_check_interval(Duration::from_millis(200)); // reaper tick

    let pool = Pool::builder()
        .client_config(client_config)
        .pool_config(pool_config)
        .build()
        .await
        .expect("Failed to create pool");

    // Fill the pool, then return both connections to idle.
    let c1 = pool.get().await.expect("checkout 1");
    let c2 = pool.get().await.expect("checkout 2");
    drop(c1);
    drop(c2);

    // Let the reaper evict both idle connections (idle_timeout 100ms,
    // reaper tick 200ms; generous margin for slow CI).
    tokio::time::sleep(Duration::from_millis(800)).await;
    let metrics = pool.metrics();
    assert!(
        metrics.connections_idle_expired >= 2,
        "reaper should have evicted the idle connections (evicted: {})",
        metrics.connections_idle_expired
    );

    // Capacity must still be max_connections: two checkouts succeed...
    let _h1 = pool.get().await.expect("checkout after reap 1");
    let _h2 = pool.get().await.expect("checkout after reap 2");
    // ...and a third must fail on the acquire timeout rather than exceed the
    // cap. Before the fix the reap had added two phantom permits, so this
    // third checkout incorrectly succeeded.
    let third = pool.get().await;
    assert!(
        third.is_err(),
        "third concurrent checkout must fail with max_connections=2"
    );

    pool.close().await;
}

// =============================================================================
// Scoped transaction API (#280)
// =============================================================================

/// `with_transaction` commits on `Ok` and rolls back on `Err`, and the
/// connection stays usable and returns to the pool either way.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_with_transaction_commit_and_rollback() {
    let client_config = get_test_config().expect("SQL Server config required");
    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(2)
        .build()
        .await
        .expect("Failed to create pool");

    let mut conn = pool.get().await.expect("get connection");

    // A connection-scoped temp table created OUTSIDE the transaction, so a
    // rollback undoes the in-transaction INSERT but not the table itself.
    conn.execute("CREATE TABLE #t280 (v INT)", &[])
        .await
        .expect("create temp table");

    // Commit path: the inserted row survives.
    conn.with_transaction(async |tx| {
        tx.execute("INSERT INTO #t280 (v) VALUES (1)", &[]).await?;
        Ok(())
    })
    .await
    .expect("committed transaction");

    // Rollback path: the closure errors, so its INSERT is undone.
    let rolled_back = conn
        .with_transaction(async |tx| {
            tx.execute("INSERT INTO #t280 (v) VALUES (2)", &[]).await?;
            Err::<(), _>(PoolError::ConnectionCreation(
                "intentional abort".to_string(),
            ))
        })
        .await;
    assert!(rolled_back.is_err(), "closure error must surface");

    // The connection is still usable (returned to `self` after each tx): only
    // the committed row remains.
    let rows = conn
        .query("SELECT COUNT(*) AS n FROM #t280", &[])
        .await
        .expect("count query");
    let counts: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|r| r.get(0).unwrap())
        .collect();
    assert_eq!(counts, vec![1], "only the committed row should remain");

    // Returns to the pool on drop.
    drop(conn);
    let status = pool.status();
    assert_eq!(status.in_use, 0);
    assert_eq!(status.available, 1);

    pool.close().await;
}

/// A connection used via `with_transaction` is the same one reused from the
/// pool afterward (it was not detached).
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_with_transaction_keeps_connection_poolable() {
    let client_config = get_test_config().expect("SQL Server config required");
    let pool = Pool::builder()
        .client_config(client_config)
        .max_connections(1)
        .build()
        .await
        .expect("Failed to create pool");

    let mut conn = pool.get().await.expect("get connection");
    let id_before = conn.metadata().id;
    let value = conn
        .with_transaction(async |tx| {
            let rows = tx.query("SELECT 7 AS v", &[]).await?;
            let v: Vec<i32> = rows
                .filter_map(|r| r.ok())
                .map(|r| r.get(0).unwrap())
                .collect();
            Ok(v[0])
        })
        .await
        .expect("transaction");
    assert_eq!(value, 7);
    drop(conn);

    // With max_connections=1, the next checkout must reuse the same connection,
    // proving `with_transaction` returned it to the pool rather than detaching.
    let conn2 = pool.get().await.expect("reuse connection");
    assert_eq!(conn2.metadata().id, id_before, "connection must be reused");
    drop(conn2);

    pool.close().await;
}
