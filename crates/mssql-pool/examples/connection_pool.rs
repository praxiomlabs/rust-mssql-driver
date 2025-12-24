//! Connection pooling with metrics example.
//!
//! This example demonstrates how to use the built-in connection pool
//! and monitor pool health through metrics.
//!
//! # Running
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourStrong@Passw0rd
//!
//! cargo run --example connection_pool
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::Config;
use mssql_driver_pool::{Pool, PoolConfig, PoolError};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());

    let conn_str = format!(
        "Server={};Database=master;User Id={};Password={};TrustServerCertificate=true",
        host, user, password
    );

    let config = Config::from_connection_string(&conn_str)?;

    println!("=== Connection Pool with Metrics Example ===\n");

    // Configure the pool
    let pool_config = PoolConfig::new()
        .min_connections(2)
        .max_connections(10)
        .connection_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .sp_reset_connection(true); // Use sp_reset_connection for state cleanup

    println!("Pool configuration:");
    println!("  Min connections: {}", pool_config.min_connections);
    println!("  Max connections: {}", pool_config.max_connections);
    println!("  Idle timeout: {:?}", pool_config.idle_timeout);
    println!();

    // Create the pool using Pool::new()
    let pool: Arc<Pool> = Arc::new(Pool::new(pool_config, config).await?);

    println!("Pool created, waiting for minimum connections...\n");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Print initial pool status
    print_pool_status(&pool);

    // Example 1: Basic pool usage
    println!("\n1. Basic pool usage:");
    {
        let mut conn = pool.get().await?;
        let rows = conn.query("SELECT @@VERSION", &[]).await?;
        for result in rows {
            let row = result?;
            let version: String = row.get(0)?;
            println!("  Connected to: {}...", &version[..50.min(version.len())]);
        }
        // Connection is automatically returned to pool when dropped
    }

    // Example 2: Concurrent usage
    println!("\n2. Concurrent pool usage (10 parallel queries):");
    let start = Instant::now();
    let mut handles = vec![];

    for i in 0..10 {
        let pool_clone: Arc<Pool> = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            let mut conn = pool_clone.get().await?;
            let _ = conn.execute("SELECT @p1, GETDATE()", &[&i]).await?;
            Ok::<_, PoolError>(i)
        }));
    }

    let mut completed = 0;
    for handle in handles {
        if handle.await?.is_ok() {
            completed += 1;
        }
    }

    println!("  Completed {} queries in {:?}", completed, start.elapsed());

    // Print metrics after concurrent usage
    print_pool_metrics(&pool);

    // Example 3: Monitor pool health
    println!("\n3. Pool health monitoring:");
    let status = pool.status();

    let utilization = status.utilization();
    let health_status = if utilization < 70.0 {
        "HEALTHY"
    } else if utilization < 90.0 {
        "WARNING"
    } else {
        "CRITICAL"
    };

    println!("  Pool health: {}", health_status);
    println!("  Utilization: {:.1}%", utilization);

    // Example 4: Simulate load and show metrics over time
    println!("\n4. Pool under load (20 concurrent queries):");
    let pool_for_load: Arc<Pool> = Arc::clone(&pool);

    let load_test = tokio::spawn(async move {
        let mut handles = vec![];
        for i in 0..20 {
            let p: Arc<Pool> = Arc::clone(&pool_for_load);
            handles.push(tokio::spawn(async move {
                let mut conn = p.get().await?;
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(100)).await;
                conn.execute("SELECT @p1", &[&i]).await?;
                Ok::<_, PoolError>(())
            }));
        }
        for h in handles {
            let _ = h.await;
        }
    });

    // Monitor while load test runs
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        print_pool_status(&pool);
    }

    let _ = load_test.await;

    // Final metrics
    println!("\n5. Final pool metrics:");
    print_pool_metrics(&pool);
    print_pool_status(&pool);

    // Graceful shutdown
    println!("\n6. Graceful shutdown:");
    println!("  Closing pool...");
    // Pool connections are cleaned up when the pool is dropped
    drop(pool);
    println!("  Pool closed.");

    Ok(())
}

fn print_pool_status(pool: &Pool) {
    let status = pool.status();
    println!(
        "  Status: {}/{} connections ({:.1}% utilization)",
        status.in_use,
        status.total,
        status.utilization()
    );
}

fn print_pool_metrics(pool: &Pool) {
    let metrics = pool.metrics();
    println!("  Metrics:");
    println!("    Connections created: {}", metrics.connections_created);
    println!("    Connections closed: {}", metrics.connections_closed);
    println!(
        "    Checkout success rate: {:.2}%",
        metrics.checkout_success_rate() * 100.0
    );
    println!(
        "    Health checks: {} performed, {} failed",
        metrics.health_checks_performed, metrics.health_checks_failed
    );
    println!(
        "    Resets: {} performed, {} failed",
        metrics.resets_performed, metrics.resets_failed
    );
}
