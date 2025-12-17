//! Stress and Performance Tests
//!
//! These tests exercise the driver under heavy load to verify stability,
//! check for memory leaks, and validate performance under stress.
//!
//! Run with:
//! ```bash
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-client --test stress -- --ignored --nocapture
//! ```

use mssql_client::{Client, Config};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password =
        std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Query Load Tests
// =============================================================================

/// Test many sequential queries on a single connection.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_sequential_queries() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let query_count = 1000;
    let start = Instant::now();

    for i in 0..query_count {
        let rows = client
            .query(&format!("SELECT {} AS result", i), &[])
            .await
            .expect("Query should succeed");

        let mut found = false;
        for result in rows {
            let row = result.expect("Row should be valid");
            let val: i32 = row.get(0).expect("Should get int");
            assert_eq!(val, i);
            found = true;
        }
        assert!(found);
    }

    let elapsed = start.elapsed();
    println!(
        "Executed {} sequential queries in {:?} ({:.2} queries/sec)",
        query_count,
        elapsed,
        query_count as f64 / elapsed.as_secs_f64()
    );

    client.close().await.expect("Failed to close");
}

/// Test many concurrent queries across multiple connections.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_concurrent_queries() {
    let config = get_test_config().expect("SQL Server config required");

    let concurrency = 10;
    let queries_per_connection = 100;
    let total_queries = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    let mut handles = Vec::new();

    for conn_id in 0..concurrency {
        let config = config.clone();
        let total = total_queries.clone();

        handles.push(tokio::spawn(async move {
            let mut client = Client::connect(config)
                .await
                .expect(&format!("Connection {} failed", conn_id));

            for query_id in 0..queries_per_connection {
                let expected = conn_id * 1000 + query_id;
                let rows = client
                    .query(&format!("SELECT {} AS result", expected), &[])
                    .await
                    .expect("Query should succeed");

                let mut found = false;
                for result in rows {
                    let row = result.expect("Row should be valid");
                    let val: i32 = row.get(0).expect("Should get int");
                    assert_eq!(val, expected as i32);
                    found = true;
                }
                assert!(found);

                total.fetch_add(1, Ordering::SeqCst);
            }

            client.close().await.expect("Failed to close");
        }));
    }

    for handle in handles {
        handle.await.expect("Task should complete");
    }

    let elapsed = start.elapsed();
    let total = total_queries.load(Ordering::SeqCst);

    println!(
        "Executed {} concurrent queries ({} connections x {} queries) in {:?} ({:.2} queries/sec)",
        total,
        concurrency,
        queries_per_connection,
        elapsed,
        total as f64 / elapsed.as_secs_f64()
    );
}

/// Test rapid connection cycling (connect/disconnect).
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_connection_cycling() {
    let config = get_test_config().expect("SQL Server config required");

    let cycles = 50;
    let start = Instant::now();

    for i in 0..cycles {
        let client = Client::connect(config.clone())
            .await
            .expect(&format!("Connection {} failed", i));

        client.close().await.expect(&format!("Close {} failed", i));
    }

    let elapsed = start.elapsed();
    println!(
        "Completed {} connection cycles in {:?} ({:.2} connections/sec)",
        cycles,
        elapsed,
        cycles as f64 / elapsed.as_secs_f64()
    );
}

/// Test concurrent connection establishment.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_concurrent_connections() {
    let config = get_test_config().expect("SQL Server config required");

    let concurrency = 20;
    let start = Instant::now();

    let mut handles = Vec::new();

    for i in 0..concurrency {
        let config = config.clone();
        handles.push(tokio::spawn(async move {
            let mut client = Client::connect(config)
                .await
                .expect(&format!("Connection {} failed", i));

            // Execute a simple query
            let rows = client
                .query("SELECT 1", &[])
                .await
                .expect("Query should succeed");

            let count: usize = rows.filter_map(|r| r.ok()).count();
            assert_eq!(count, 1);

            client.close().await.expect("Failed to close");
        }));
    }

    for handle in handles {
        handle.await.expect("Task should complete");
    }

    let elapsed = start.elapsed();
    println!(
        "Established and closed {} concurrent connections in {:?}",
        concurrency, elapsed
    );
}

// =============================================================================
// Data Volume Tests
// =============================================================================

/// Test inserting and retrieving many rows.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_many_rows() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let row_count = 10_000;

    // Create temp table
    client
        .execute("CREATE TABLE #StressRows (id INT, value NVARCHAR(100))", &[])
        .await
        .expect("Failed to create table");

    // Insert in batches
    let batch_size = 100;
    let insert_start = Instant::now();

    for batch in 0..(row_count / batch_size) {
        let mut values = Vec::new();
        for i in 0..batch_size {
            let id = batch * batch_size + i;
            values.push(format!("({}, 'Value {}')", id, id));
        }
        let sql = format!("INSERT INTO #StressRows VALUES {}", values.join(","));
        client.execute(&sql, &[]).await.expect("Insert should succeed");
    }

    let insert_elapsed = insert_start.elapsed();
    println!(
        "Inserted {} rows in {:?} ({:.2} rows/sec)",
        row_count,
        insert_elapsed,
        row_count as f64 / insert_elapsed.as_secs_f64()
    );

    // Query all rows
    let query_start = Instant::now();

    let rows = client
        .query("SELECT id, value FROM #StressRows ORDER BY id", &[])
        .await
        .expect("Query should succeed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let _id: i32 = row.get(0).expect("Should get id");
        let _value: String = row.get(1).expect("Should get value");
        count += 1;
    }

    let query_elapsed = query_start.elapsed();
    assert_eq!(count, row_count, "Should retrieve all rows");

    println!(
        "Retrieved {} rows in {:?} ({:.2} rows/sec)",
        count,
        query_elapsed,
        count as f64 / query_elapsed.as_secs_f64()
    );

    client.close().await.expect("Failed to close");
}

/// Test with large data values.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_large_values() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create table with large columns
    client
        .execute("CREATE TABLE #LargeValues (id INT, big_text NVARCHAR(MAX))", &[])
        .await
        .expect("Failed to create table");

    // Insert rows with increasingly large text
    let sizes = [100, 1_000, 10_000, 50_000, 100_000];

    for (i, &size) in sizes.iter().enumerate() {
        let big_text: String = "A".repeat(size);
        let id = i as i32;

        client
            .execute(
                "INSERT INTO #LargeValues VALUES (@p1, @p2)",
                &[&id, &big_text.as_str()],
            )
            .await
            .expect(&format!("Insert size {} should succeed", size));
    }

    println!("Inserted values of sizes: {:?}", sizes);

    // Retrieve and verify
    let rows = client
        .query("SELECT id, LEN(big_text) AS len FROM #LargeValues ORDER BY id", &[])
        .await
        .expect("Query should succeed");

    let mut idx = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let len: i64 = row.get(1).expect("Should get len"); // LEN() returns BIGINT
        assert_eq!(len, sizes[idx] as i64, "Length should match");
        idx += 1;
    }

    assert_eq!(idx, sizes.len());

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Transaction Stress Tests
// =============================================================================

/// Test many sequential transactions.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_sequential_transactions() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table
    client
        .execute("CREATE TABLE #TxStress (id INT PRIMARY KEY, value INT)", &[])
        .await
        .expect("Failed to create table");

    let tx_count = 100;
    let start = Instant::now();

    for i in 0..tx_count {
        let mut tx = client
            .begin_transaction()
            .await
            .expect("Begin should succeed");

        tx.execute(
            "INSERT INTO #TxStress VALUES (@p1, @p2)",
            &[&i, &(i * 10)],
        )
        .await
        .expect("Insert should succeed");

        client = tx.commit().await.expect("Commit should succeed");
    }

    let elapsed = start.elapsed();

    // Verify all rows exist
    let rows = client
        .query("SELECT COUNT(*) FROM #TxStress", &[])
        .await
        .expect("Count query should succeed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        count = row.get::<i32>(0).expect("Should get count");
    }

    assert_eq!(count, tx_count, "All transactions should have committed");

    println!(
        "Committed {} transactions in {:?} ({:.2} tx/sec)",
        tx_count,
        elapsed,
        tx_count as f64 / elapsed.as_secs_f64()
    );

    client.close().await.expect("Failed to close");
}

/// Test transaction with mixed commit/rollback.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_mixed_transactions() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table
    client
        .execute("CREATE TABLE #MixedTx (id INT PRIMARY KEY)", &[])
        .await
        .expect("Failed to create table");

    let tx_count = 100;
    let mut committed = 0;
    let start = Instant::now();

    for i in 0..tx_count {
        let mut tx = client
            .begin_transaction()
            .await
            .expect("Begin should succeed");

        tx.execute("INSERT INTO #MixedTx VALUES (@p1)", &[&i])
            .await
            .expect("Insert should succeed");

        // Commit even, rollback odd
        if i % 2 == 0 {
            client = tx.commit().await.expect("Commit should succeed");
            committed += 1;
        } else {
            client = tx.rollback().await.expect("Rollback should succeed");
        }
    }

    let elapsed = start.elapsed();

    // Verify correct count
    let rows = client
        .query("SELECT COUNT(*) FROM #MixedTx", &[])
        .await
        .expect("Count query should succeed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        count = row.get::<i32>(0).expect("Should get count");
    }

    assert_eq!(count, committed, "Only committed transactions should exist");

    println!(
        "Processed {} transactions ({} committed, {} rolled back) in {:?}",
        tx_count,
        committed,
        tx_count - committed,
        elapsed
    );

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Stability Tests
// =============================================================================

/// Test connection remains stable over time with periodic queries.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_long_lived_connection() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let duration = Duration::from_secs(10);
    let interval = Duration::from_millis(100);
    let start = Instant::now();
    let mut query_count = 0;

    while start.elapsed() < duration {
        let rows = client
            .query("SELECT GETDATE()", &[])
            .await
            .expect("Query should succeed");

        let count: usize = rows.filter_map(|r| r.ok()).count();
        assert_eq!(count, 1);
        query_count += 1;

        tokio::time::sleep(interval).await;
    }

    let elapsed = start.elapsed();
    println!(
        "Maintained connection for {:?} with {} queries",
        elapsed, query_count
    );

    client.close().await.expect("Failed to close");
}

/// Test error recovery under stress.
#[tokio::test]
#[ignore = "Requires SQL Server - stress test"]
async fn test_stress_error_recovery() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let iterations = 100;
    let mut errors = 0;
    let mut successes = 0;

    for i in 0..iterations {
        // Alternate between valid and invalid queries
        if i % 2 == 0 {
            let result = client.query("SELECT 1", &[]).await;
            if result.is_ok() {
                let rows = result.unwrap();
                let _ = rows.filter_map(|r| r.ok()).count();
                successes += 1;
            }
        } else {
            let result = client.query("SELEKT 1", &[]).await;
            if result.is_err() {
                errors += 1;
            }
        }
    }

    assert_eq!(successes, iterations / 2, "All valid queries should succeed");
    assert_eq!(errors, iterations / 2, "All invalid queries should fail");

    // Connection should still be usable
    let rows = client
        .query("SELECT 'still alive'", &[])
        .await
        .expect("Final query should succeed");

    let count: usize = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1);

    println!(
        "Completed {} iterations ({} successes, {} errors), connection still viable",
        iterations, successes, errors
    );

    client.close().await.expect("Failed to close");
}
