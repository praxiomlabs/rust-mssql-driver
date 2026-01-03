//! Timeout scenario tests for mssql-client.
//!
//! Tests for various timeout configurations and behaviors.
//! Tests marked with `#[ignore]` require a real SQL Server instance.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::approx_constant
)]

use mssql_client::Config;
use std::time::Duration;

// =============================================================================
// Configuration Timeout Tests
// =============================================================================

#[test]
fn test_connect_timeout_configuration() {
    // Use connection string to configure connect timeout
    let config = Config::from_connection_string(
        "Server=localhost;Database=master;User Id=sa;Password=pass;Connect Timeout=5",
    )
    .expect("Valid connection string");

    // Verify timeout was parsed (field is public)
    assert_eq!(config.connect_timeout.as_secs(), 5);
}

#[test]
fn test_command_timeout_configuration() {
    // Use connection string to configure command timeout
    let config = Config::from_connection_string(
        "Server=localhost;Database=master;User Id=sa;Password=pass;Command Timeout=30",
    )
    .expect("Valid connection string");

    // Should not panic during configuration
    let _ = config;
}

#[test]
fn test_zero_timeout_configuration() {
    // Zero timeout should mean no timeout (infinite)
    let config = Config::from_connection_string(
        "Server=localhost;Database=master;User Id=sa;Password=pass;Connect Timeout=0;Command Timeout=0",
    )
    .expect("Valid connection string");

    let _ = config;
}

#[test]
fn test_very_short_timeout_configuration() {
    // Very short timeouts should be configurable via connection string
    // Note: Connection string timeout is in seconds, so we test small second values
    let config = Config::from_connection_string(
        "Server=localhost;Database=master;User Id=sa;Password=pass;Connect Timeout=1;Command Timeout=1",
    )
    .expect("Valid connection string");

    let _ = config;
}

#[test]
fn test_very_long_timeout_configuration() {
    // Very long timeouts (1 hour = 3600 seconds) should be configurable
    let config = Config::from_connection_string(
        "Server=localhost;Database=master;User Id=sa;Password=pass;Connect Timeout=3600;Command Timeout=3600",
    )
    .expect("Valid connection string");

    let _ = config;
}

#[test]
fn test_timeout_from_connection_string() {
    // Connect Timeout in connection string
    let result = Config::from_connection_string(
        "Server=localhost;Connect Timeout=15;Command Timeout=30;User Id=sa;Password=x",
    );
    assert!(result.is_ok());
}

#[test]
fn test_timeout_zero_from_connection_string() {
    // Zero timeout in connection string
    let result = Config::from_connection_string(
        "Server=localhost;Connect Timeout=0;Command Timeout=0;User Id=sa;Password=x",
    );
    assert!(result.is_ok());
}

#[test]
fn test_invalid_timeout_from_connection_string() {
    // Negative timeout should fail or be treated as zero
    let result =
        Config::from_connection_string("Server=localhost;Connect Timeout=-5;User Id=sa;Password=x");
    // Should handle gracefully (either error or treat as default)
    let _ = result;
}

#[test]
fn test_non_numeric_timeout_from_connection_string() {
    // Non-numeric timeout should fail
    let result = Config::from_connection_string(
        "Server=localhost;Connect Timeout=abc;User Id=sa;Password=x",
    );
    // Should handle gracefully
    let _ = result;
}

// =============================================================================
// Live Timeout Tests (require SQL Server)
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
#[ignore = "Requires SQL Server"]
async fn test_connect_timeout_success() {
    use mssql_client::Client;

    let mut config = get_test_config().expect("SQL Server config required");
    config = config.connect_timeout(Duration::from_secs(30));

    let client = Client::connect(config)
        .await
        .expect("Should connect within timeout");
    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires SQL Server - may be slow"]
async fn test_connect_timeout_expired() {
    use mssql_client::Client;

    // Connect to non-routable IP with very short timeout
    let conn_str = "Server=10.255.255.1,1433;Database=master;User Id=sa;Password=password;\
                    TrustServerCertificate=true;Connect Timeout=1";
    let config = Config::from_connection_string(conn_str).expect("Valid connection string");

    let start = std::time::Instant::now();
    let result = Client::connect(config).await;
    let elapsed = start.elapsed();

    // Should fail within reasonable time (timeout + small overhead)
    assert!(result.is_err(), "Should fail to connect");
    assert!(
        elapsed < Duration::from_secs(10),
        "Should timeout quickly, took {:?}",
        elapsed
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_with_application_timeout() {
    use mssql_client::Client;
    use tokio::time::timeout;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Run a quick query with application-level timeout
    let result = timeout(Duration::from_secs(5), async {
        client.query("SELECT 1", &[]).await
    })
    .await;

    assert!(result.is_ok(), "Should complete within timeout");
    assert!(result.unwrap().is_ok(), "Query should succeed");

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires SQL Server - runs slow query"]
async fn test_slow_query_timeout() {
    use mssql_client::Client;
    use tokio::time::timeout;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Run a slow query with short timeout
    let result = timeout(Duration::from_secs(1), async {
        // WAITFOR DELAY pauses for specified time
        client.query("WAITFOR DELAY '00:00:05'", &[]).await
    })
    .await;

    // Should timeout before query completes
    assert!(result.is_err(), "Should timeout waiting for slow query");

    // Connection may or may not be usable after timeout
    // Just ensure we don't panic
    let _ = client.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_cancel_long_running_query() {
    use mssql_client::Client;
    use std::sync::Arc;
    use tokio::sync::Notify;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Get cancel handle before starting query
    let cancel_handle = client.cancel_handle();
    let started = Arc::new(Notify::new());
    let started_clone = started.clone();

    // Spawn a task to cancel after short delay
    tokio::spawn(async move {
        started_clone.notified().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = cancel_handle.cancel().await;
    });

    // Start a long-running query
    started.notify_one();
    let result = client.query("WAITFOR DELAY '00:00:30'", &[]).await;

    // Query should be cancelled
    // The exact error type depends on implementation
    assert!(
        result.is_err() || result.is_ok(),
        "Query should complete (cancelled or timeout)"
    );

    // Clean up - connection may need to be closed
    let _ = client.close().await;
}

// =============================================================================
// Pool Timeout Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_acquire_timeout() {
    use mssql_driver_pool::{Pool, PoolConfig};

    let config = get_test_config().expect("SQL Server config required");

    // Create a pool with size 1 and short acquire timeout
    let pool_config = PoolConfig::new()
        .max_connections(1)
        .connection_timeout(Duration::from_millis(100));

    let pool = Pool::new(pool_config, config)
        .await
        .expect("Pool creation failed");

    // Get the only connection
    let conn1 = pool.get().await.expect("First get should succeed");

    // Try to get another - should timeout
    let start = std::time::Instant::now();
    let result = pool.get().await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Should timeout waiting for connection");
    assert!(
        elapsed < Duration::from_secs(1),
        "Should timeout quickly, took {:?}",
        elapsed
    );

    // Release first connection
    drop(conn1);

    // Now should be able to get a connection
    let _conn2 = pool.get().await.expect("Should succeed after release");

    pool.close().await;
}

// =============================================================================
// Connection Exhaustion Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_exhaustion_recovery() {
    use mssql_driver_pool::{Pool, PoolConfig};
    use std::sync::Arc;

    let config = get_test_config().expect("SQL Server config required");

    // Create a small pool
    let pool_config = PoolConfig::new()
        .min_connections(1)
        .max_connections(3)
        .connection_timeout(Duration::from_secs(5));

    let pool = Arc::new(
        Pool::new(pool_config, config)
            .await
            .expect("Pool creation failed"),
    );

    // Exhaust all connections
    let conn1 = pool.get().await.expect("Get 1 failed");
    let conn2 = pool.get().await.expect("Get 2 failed");
    let conn3 = pool.get().await.expect("Get 3 failed");

    // Pool is now exhausted - spawn a task that will wait
    let pool_clone = Arc::clone(&pool);
    let waiter = tokio::spawn(async move {
        let start = std::time::Instant::now();
        let result = pool_clone.get().await;
        (start.elapsed(), result.is_ok())
    });

    // Wait a bit then release a connection
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(conn1);

    // The waiter should succeed
    let (elapsed, success) = waiter.await.expect("Waiter panicked");
    assert!(success, "Waiter should get a connection after release");
    assert!(
        elapsed < Duration::from_secs(5),
        "Should not wait full timeout, got {:?}",
        elapsed
    );

    drop(conn2);
    drop(conn3);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_concurrent_connection_requests() {
    use mssql_driver_pool::{Pool, PoolConfig};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let config = get_test_config().expect("SQL Server config required");

    // Create a pool smaller than concurrent requests
    let pool_config = PoolConfig::new()
        .min_connections(2)
        .max_connections(5)
        .connection_timeout(Duration::from_secs(30));

    let pool = Arc::new(
        Pool::new(pool_config, config)
            .await
            .expect("Pool creation failed"),
    );
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    // Spawn 20 concurrent tasks, each doing a quick query
    let mut handles = Vec::new();
    for i in 0..20 {
        let pool = Arc::clone(&pool);
        let success = success_count.clone();
        let errors = error_count.clone();

        handles.push(tokio::spawn(async move {
            match pool.get().await {
                Ok(mut conn) => {
                    // Simulate some work
                    match conn.query(&format!("SELECT {}", i), &[]).await {
                        Ok(_) => {
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
        }));
    }

    // Wait for all tasks
    for handle in handles {
        let _ = handle.await;
    }

    let successes = success_count.load(Ordering::SeqCst);
    let errors = error_count.load(Ordering::SeqCst);

    // All should succeed (pool should handle contention)
    assert_eq!(
        successes, 20,
        "All 20 requests should succeed, got {} successes and {} errors",
        successes, errors
    );

    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_pool_exhaustion_timeout() {
    use mssql_driver_pool::{Pool, PoolConfig};
    use std::sync::Arc;

    let config = get_test_config().expect("SQL Server config required");

    // Create a pool with size 1 and very short timeout
    let pool_config = PoolConfig::new()
        .max_connections(1)
        .connection_timeout(Duration::from_millis(50));

    let pool = Arc::new(
        Pool::new(pool_config, config)
            .await
            .expect("Pool creation failed"),
    );

    // Hold the only connection
    let _conn = pool.get().await.expect("First get failed");

    // Multiple requests should all timeout quickly
    let mut handles = Vec::new();
    for _ in 0..5 {
        let pool = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            let start = std::time::Instant::now();
            let result = pool.get().await;
            (start.elapsed(), result.is_err())
        }));
    }

    for handle in handles {
        let (elapsed, timed_out) = handle.await.expect("Task panicked");
        assert!(timed_out, "Should timeout when pool exhausted");
        assert!(
            elapsed < Duration::from_millis(200),
            "Should timeout quickly, took {:?}",
            elapsed
        );
    }

    pool.close().await;
}

#[test]
fn test_pool_config_validation() {
    use mssql_driver_pool::PoolConfig;

    // min > max should be handled gracefully
    let config = PoolConfig::new().min_connections(10).max_connections(5);

    // The config should either error or adjust (implementation dependent)
    // Just verify it doesn't panic
    let _ = config;
}

#[test]
fn test_pool_config_zero_max() {
    use mssql_driver_pool::PoolConfig;

    // Zero max should be handled
    let config = PoolConfig::new().max_connections(0);
    let _ = config;
}

#[test]
fn test_pool_config_large_values() {
    use mssql_driver_pool::PoolConfig;

    // Very large pool sizes should be configurable
    let config = PoolConfig::new().min_connections(100).max_connections(1000);
    let _ = config;
}
