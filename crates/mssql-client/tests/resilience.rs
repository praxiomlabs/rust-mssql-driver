//! SQL Server Resilience and Recovery Tests
//!
//! These tests validate driver behavior when connections are disrupted,
//! including server restarts, network interruptions, and connection termination.
//!
//! Run with:
//! ```bash
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-client --test resilience -- --ignored
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::expect_fun_call,
    clippy::panic
)]

use mssql_client::{Client, Config};
use std::time::Duration;
use tokio::time::timeout;

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host};Database={database};User Id={user};Password={password};TrustServerCertificate=true;Encrypt={encrypt}"
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Connection State Detection Tests
// =============================================================================

/// Test that the driver detects when a connection has been killed server-side.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_detect_killed_connection() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config.clone())
        .await
        .expect("Failed to connect");

    // Get current session ID
    let rows = client
        .query("SELECT @@SPID AS spid", &[])
        .await
        .expect("Query failed");

    let mut spid: i16 = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        spid = row.get(0).expect("Failed to get SPID");
    }

    assert!(spid > 0, "Should have valid SPID");

    // Create another connection to kill the first one
    let mut admin_client = Client::connect(config)
        .await
        .expect("Failed to connect admin");

    // Kill the first session
    admin_client
        .execute(&format!("KILL {spid}"), &[])
        .await
        .expect("Failed to kill session");

    admin_client.close().await.expect("Failed to close admin");

    // Give the server a moment to process the kill
    tokio::time::sleep(Duration::from_millis(100)).await;

    // The next query on the killed connection should fail
    let result = client.query("SELECT 1", &[]).await;
    assert!(result.is_err(), "Query should fail on killed connection");
}

/// Test that the driver handles connection timeout gracefully.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_with_timeout() {
    let config = get_test_config().expect("SQL Server config required");

    // Connection should complete within 30 seconds
    let result = timeout(Duration::from_secs(30), Client::connect(config)).await;

    match result {
        Ok(Ok(client)) => {
            client.close().await.expect("Failed to close");
        }
        Ok(Err(e)) => {
            panic!("Connection failed with error: {e:?}");
        }
        Err(_) => {
            panic!("Connection timed out after 30 seconds");
        }
    }
}

/// Test that the driver handles query timeout gracefully.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_timeout_handling() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute a quick query that should complete immediately
    let result = timeout(
        Duration::from_secs(5),
        client.query("SELECT 1 AS quick_result", &[]),
    )
    .await;

    assert!(result.is_ok(), "Quick query should not timeout");
    let rows = result.unwrap().expect("Query should succeed");
    let count: usize = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Recovery After Errors Tests
// =============================================================================

/// Test that a connection can recover after a query error.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_recovery_after_syntax_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute invalid SQL
    let result = client.query("SELEKT * FROM nonexistent", &[]).await;
    assert!(result.is_err(), "Invalid SQL should fail");

    // Connection should still be usable
    let rows = client
        .query("SELECT 1 AS recovered", &[])
        .await
        .expect("Recovery query should succeed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: i32 = row.get(0).expect("Failed to get value");
        assert_eq!(val, 1);
        found = true;
    }
    assert!(found, "Should have received result");

    client.close().await.expect("Failed to close");
}

/// Test that a connection can handle multiple consecutive errors.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multiple_consecutive_errors() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Multiple error-producing queries
    for i in 0..5 {
        let result = client
            .query(&format!("RAISERROR('Test error {i}', 16, 1)"), &[])
            .await;
        assert!(result.is_err(), "RAISERROR should produce an error");
    }

    // Connection should still be usable
    let rows = client
        .query("SELECT 'still alive' AS status", &[])
        .await
        .expect("Recovery query should succeed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let status: String = row.get(0).expect("Failed to get status");
        assert_eq!(status, "still alive");
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

/// Test recovery after deadlock simulation.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_recovery_after_deadlock_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Simulate a deadlock error (can't create real deadlock in single connection)
    // Instead, test that we recover from error 1205 (deadlock) if it occurred
    let _result = client
        .query("RAISERROR('Simulated deadlock', 13, 1) WITH NOWAIT", &[])
        .await;

    // Depending on severity, this may or may not error
    // The point is to ensure we can still use the connection

    let rows = client
        .query("SELECT 'recovered' AS result", &[])
        .await
        .expect("Should recover after deadlock-like error");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let _result: String = row.get(0).expect("Failed to get result");
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Transaction Resilience Tests
// =============================================================================

/// Test that transaction state is properly tracked after errors.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_state_after_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table
    client
        .execute("CREATE TABLE #TxErrorTest (id INT PRIMARY KEY)", &[])
        .await
        .expect("Failed to create table");

    // Begin transaction
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    // Insert valid data
    tx.execute("INSERT INTO #TxErrorTest VALUES (1)", &[])
        .await
        .expect("Insert should succeed");

    // Try to insert duplicate (will fail)
    let result = tx.execute("INSERT INTO #TxErrorTest VALUES (1)", &[]).await;
    assert!(result.is_err(), "Duplicate insert should fail");

    // Transaction should still be active and rollbackable
    let mut client = tx.rollback().await.expect("Rollback should succeed");

    // Verify nothing was committed
    let rows = client
        .query("SELECT COUNT(*) FROM #TxErrorTest", &[])
        .await
        .expect("Count query should succeed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        count = row.get::<i32>(0).expect("Failed to get count");
    }
    assert_eq!(count, 0, "Table should be empty after rollback");

    client.close().await.expect("Failed to close");
}

/// Test nested savepoint behavior on error.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_savepoint_error_recovery() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table
    client
        .execute("CREATE TABLE #SavepointTest (id INT)", &[])
        .await
        .expect("Failed to create table");

    // Begin transaction
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    // Insert first row
    tx.execute("INSERT INTO #SavepointTest VALUES (1)", &[])
        .await
        .expect("First insert should succeed");

    // Create savepoint (using raw SQL since API may not expose savepoints)
    tx.execute("SAVE TRANSACTION sp1", &[])
        .await
        .expect("Savepoint should succeed");

    // Insert second row
    tx.execute("INSERT INTO #SavepointTest VALUES (2)", &[])
        .await
        .expect("Second insert should succeed");

    // Rollback to savepoint
    tx.execute("ROLLBACK TRANSACTION sp1", &[])
        .await
        .expect("Rollback to savepoint should succeed");

    // Insert different row
    tx.execute("INSERT INTO #SavepointTest VALUES (3)", &[])
        .await
        .expect("Third insert should succeed");

    // Commit
    let mut client = tx.commit().await.expect("Commit should succeed");

    // Verify results: should have 1 and 3, but not 2
    let rows = client
        .query("SELECT id FROM #SavepointTest ORDER BY id", &[])
        .await
        .expect("Query should succeed");

    let mut ids: Vec<i32> = Vec::new();
    for result in rows {
        let row = result.expect("Row should be valid");
        ids.push(row.get(0).expect("Failed to get id"));
    }

    assert_eq!(ids, vec![1, 3], "Should have 1 and 3, not 2");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Connection Pool Resilience Tests (if pool is available)
// =============================================================================

/// Test that multiple connections can be established successfully.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multiple_concurrent_connections() {
    let config = get_test_config().expect("SQL Server config required");

    // Create multiple connections concurrently
    let mut handles = Vec::new();
    for i in 0..5 {
        let config = config.clone();
        handles.push(tokio::spawn(async move {
            let mut client = Client::connect(config).await.expect("Failed to connect");

            // Execute a query to verify connection is working
            let rows = client
                .query(&format!("SELECT {i} AS conn_id"), &[])
                .await
                .expect("Query failed");

            let mut found = false;
            for result in rows {
                let row = result.expect("Row should be valid");
                let id: i32 = row.get(0).expect("Failed to get id");
                assert_eq!(id, i);
                found = true;
            }
            assert!(found);

            client.close().await.expect("Failed to close");
        }));
    }

    // Wait for all connections to complete
    for handle in handles {
        handle.await.expect("Task should complete");
    }
}

/// Test that the driver handles rapid connect/disconnect cycles.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rapid_connect_disconnect() {
    let config = get_test_config().expect("SQL Server config required");

    // Rapidly connect and disconnect multiple times
    for i in 0..10 {
        let client = Client::connect(config.clone())
            .await
            .expect(&format!("Failed to connect on iteration {i}"));

        client
            .close()
            .await
            .expect(&format!("Failed to close on iteration {i}"));
    }
}

// =============================================================================
// Long-Running Query Tests
// =============================================================================

/// Test handling of a moderately long-running query.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_long_running_query() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute a query that takes a few seconds (using WAITFOR)
    // Note: Use execute for the delay, then query for the result
    let start = std::time::Instant::now();

    client
        .execute("WAITFOR DELAY '00:00:02'", &[])
        .await
        .expect("WAITFOR should succeed");

    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_secs(2),
        "Query should take at least 2 seconds"
    );

    // Verify connection still works after long delay
    let rows = client
        .query("SELECT 'done' AS result", &[])
        .await
        .expect("Query after wait should succeed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let result: String = row.get(0).expect("Failed to get result");
        assert_eq!(result, "done");
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

/// Issue #156 regression: `query_with_timeout` must cancel via Attention and
/// leave the connection usable, not desync it by dropping the read future.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_with_timeout_leaves_connection_usable() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // A 10s server-side delay against a 1s command timeout must fail fast...
    let start = std::time::Instant::now();
    let result = client
        .query_with_timeout(
            "WAITFOR DELAY '00:00:10'; SELECT 1 AS n",
            &[],
            Duration::from_secs(1),
        )
        .await;
    let elapsed = start.elapsed();
    match result {
        Err(mssql_client::Error::CommandTimeout) => {}
        Err(other) => panic!("expected CommandTimeout, got {other:?}"),
        Ok(_) => panic!("expected CommandTimeout, query unexpectedly succeeded"),
    }
    assert!(
        elapsed < Duration::from_secs(5),
        "timeout must fire near 1s, not wait out the 10s query; took {elapsed:?}"
    );

    // ...and the very next query on the SAME client must succeed with the
    // correct result — proving the connection was not left desynced.
    let rows = client
        .query("SELECT 42 AS answer", &[])
        .await
        .expect("connection must be usable after a command timeout");
    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 1);
    let answer: i32 = data[0].get(0).expect("must read the fresh result");
    assert_eq!(answer, 42, "stale response would yield wrong data");

    client.close().await.expect("Failed to close");
}

/// Issue #156 regression: the default `command_timeout` is enforced on a
/// plain `query()` (no explicit timeout), and the connection stays usable.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_default_command_timeout_enforced() {
    let mut config = get_test_config().expect("SQL Server config required");
    config.command_timeout = Duration::from_secs(1);
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let start = std::time::Instant::now();
    let result = client.query("WAITFOR DELAY '00:00:10'", &[]).await;
    let elapsed = start.elapsed();
    match result {
        Err(mssql_client::Error::CommandTimeout) => {}
        Err(other) => panic!("default command_timeout must cancel; got {other:?}"),
        Ok(_) => panic!("default command_timeout must cancel; query succeeded"),
    }
    assert!(
        elapsed < Duration::from_secs(5),
        "default timeout must fire near 1s; took {elapsed:?}"
    );

    let rows = client
        .query("SELECT 7 AS n", &[])
        .await
        .expect("connection must be usable after a default-timeout cancel");
    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    let n: i32 = data[0].get(0).expect("must read fresh result");
    assert_eq!(n, 7);

    client.close().await.expect("Failed to close");
}

/// Issue #185 regression: `command_timeout` must cover the stored-procedure,
/// named-parameter, and multi-result paths — previously only `query()` and
/// `execute()` ran under the deadline.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_command_timeout_covers_all_command_paths() {
    let mut config = get_test_config().expect("SQL Server config required");
    config.command_timeout = Duration::from_secs(1);
    let mut client = Client::connect(config).await.expect("Failed to connect");

    const SLOW: &str = "WAITFOR DELAY '00:00:10'";

    client
        .execute(
            "CREATE OR ALTER PROCEDURE dbo.slow_proc_185 AS WAITFOR DELAY '00:00:10'",
            &[],
        )
        .await
        .expect("creating the slow procedure must succeed");

    // Each closure result is checked the same way: CommandTimeout near 1s,
    // and the connection must answer a fresh query afterwards.
    fn expect_timeout<T: std::fmt::Debug>(
        path: &str,
        start: std::time::Instant,
        result: Result<T, mssql_client::Error>,
    ) {
        match result {
            Err(mssql_client::Error::CommandTimeout) => {}
            Err(other) => panic!("{path}: expected CommandTimeout, got {other:?}"),
            Ok(v) => panic!("{path}: expected CommandTimeout, got Ok({v:?})"),
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(5),
            "{path}: timeout must fire near 1s, not wait out the 10s delay; took {elapsed:?}"
        );
    }

    let start = std::time::Instant::now();
    let result = client.call_procedure("dbo.slow_proc_185", &[]).await;
    expect_timeout("call_procedure", start, result);

    let start = std::time::Instant::now();
    let result = client
        .procedure("dbo.slow_proc_185")
        .expect("valid procedure name")
        .execute()
        .await;
    expect_timeout("ProcedureBuilder::execute", start, result);

    let start = std::time::Instant::now();
    let result = client.execute_named(SLOW, &[]).await;
    expect_timeout("execute_named", start, result);

    let start = std::time::Instant::now();
    let result = client
        .query_multiple(&format!("{SLOW}; SELECT 1 AS n"), &[])
        .await
        .map(|_| ());
    expect_timeout("query_multiple", start, result);

    // After four cancels in a row the connection must still be clean.
    let rows = client
        .query("SELECT 99 AS n", &[])
        .await
        .expect("connection must be usable after timeouts on every path");
    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    let n: i32 = data[0].get(0).expect("must read fresh result");
    assert_eq!(n, 99);

    client
        .execute("DROP PROCEDURE dbo.slow_proc_185", &[])
        .await
        .expect("cleanup must succeed");
    client.close().await.expect("Failed to close");
}

/// Test that short timeout interrupts long-running query.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_cancelled_by_timeout() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Try to execute a 10-second query with a 1-second timeout
    let result = timeout(
        Duration::from_secs(1),
        client.query("WAITFOR DELAY '00:00:10'; SELECT 1", &[]),
    )
    .await;

    // Should timeout, not complete
    assert!(result.is_err(), "Query should be cancelled by timeout");

    // Note: After timeout, connection may be in an undefined state
    // Depending on driver implementation, it may or may not be reusable
}

/// Capability audit: explicit cancellation must actually kill the query
/// **server-side**, not just abandon the client-side future.
///
/// The claims under test (cancel.rs rustdoc): `CancelHandle::cancel()` sends
/// Attention, the current query returns an error, the response is drained
/// (DONE_ATTN), and the same connection remains usable. This proves all four
/// plus the part no other test covers: the request disappears from
/// `sys.dm_exec_requests`, observed from a second connection.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_explicit_cancel_terminates_query_server_side() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config.clone()).await.expect("connect A");
    let mut observer = Client::connect(config).await.expect("connect B");

    // Identify connection A's session so the observer can watch its request.
    let rows = client
        .query("SELECT CAST(@@SPID AS INT)", &[])
        .await
        .expect("spid query");
    let spid: i32 = rows
        .into_iter()
        .next()
        .expect("spid row")
        .expect("spid row ok")
        .get(0)
        .expect("spid value");

    let canceller = client.cancel_handle();

    // While A is stuck in WAITFOR: positive control (the request must be
    // visible server-side), then cancel.
    let watcher = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        let rows = observer
            .query(
                "SELECT COUNT(*) FROM sys.dm_exec_requests \
                 WHERE session_id = @p1 AND command = 'WAITFOR'",
                &[&spid],
            )
            .await
            .expect("observer query");
        let running: i32 = rows
            .into_iter()
            .next()
            .expect("count row")
            .expect("count row ok")
            .get(0)
            .expect("count value");

        canceller.cancel().await.expect("cancel send");
        (observer, running)
    });

    let started = std::time::Instant::now();
    let result = client.query("WAITFOR DELAY '00:00:30'", &[]).await;
    let elapsed = started.elapsed();

    let err = result.err().expect("cancelled query must return an error");
    assert!(
        matches!(err, mssql_client::Error::Cancelled),
        "cancelled query must return Error::Cancelled, got {err:?}"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "cancel must interrupt the 30s WAITFOR promptly; took {elapsed:?}"
    );

    let (mut observer, running_before) = watcher.await.expect("watcher task");
    assert_eq!(
        running_before, 1,
        "positive control: the WAITFOR request must be visible in \
         sys.dm_exec_requests before cancellation"
    );

    // Server side: the request must be gone shortly after the cancel.
    let mut still_running = i32::MAX;
    for _ in 0..30 {
        let rows = observer
            .query(
                "SELECT COUNT(*) FROM sys.dm_exec_requests \
                 WHERE session_id = @p1 AND command = 'WAITFOR'",
                &[&spid],
            )
            .await
            .expect("observer recheck");
        still_running = rows
            .into_iter()
            .next()
            .expect("recheck row")
            .expect("recheck row ok")
            .get(0)
            .expect("recheck value");
        if still_running == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        still_running, 0,
        "the WAITFOR request must terminate server-side after Attention"
    );

    // Same connection must be clean and reusable (DONE_ATTN drained).
    let rows = client
        .query("SELECT 42", &[])
        .await
        .expect("query on the same connection after cancel must succeed");
    let value: i32 = rows
        .into_iter()
        .next()
        .expect("reuse row")
        .expect("reuse row ok")
        .get(0)
        .expect("reuse value");
    assert_eq!(value, 42);

    observer.close().await.expect("close observer");
    client.close().await.expect("close client");
}

// =============================================================================
// Error Boundary Tests
// =============================================================================

/// Test that arithmetic overflow is handled correctly.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_arithmetic_overflow_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // This should cause an arithmetic overflow
    let result = client
        .query("SELECT CAST(99999999999999999999 AS INT)", &[])
        .await;

    assert!(
        result.is_err(),
        "Arithmetic overflow should produce an error"
    );

    // Connection should still work
    let rows = client
        .query("SELECT 1 AS still_works", &[])
        .await
        .expect("Recovery should succeed");

    let count: usize = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

/// Test that divide by zero is handled correctly.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_divide_by_zero_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Enable ANSI_WARNINGS to make divide by zero an error
    client
        .execute("SET ARITHABORT ON", &[])
        .await
        .expect("Failed to set ARITHABORT");

    let result = client.query("SELECT 1/0", &[]).await;

    assert!(result.is_err(), "Divide by zero should produce an error");

    // Connection should still work
    let rows = client
        .query("SELECT 1 AS still_works", &[])
        .await
        .expect("Recovery should succeed");

    let count: usize = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

/// Test that string truncation is handled correctly.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_string_truncation_handling() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create table with small column
    client
        .execute("CREATE TABLE #TruncTest (val CHAR(5))", &[])
        .await
        .expect("Failed to create table");

    // Enable strict mode
    client
        .execute("SET ANSI_WARNINGS ON", &[])
        .await
        .expect("Failed to set ANSI_WARNINGS");

    // Try to insert string that's too long
    let _result = client
        .execute(
            "INSERT INTO #TruncTest VALUES ('This is way too long for the column')",
            &[],
        )
        .await;

    // Depending on SQL Server settings, this may truncate or error
    // The driver should handle either case gracefully

    // Connection should still work
    let rows = client
        .query("SELECT 1 AS still_works", &[])
        .await
        .expect("Recovery should succeed");

    let count: usize = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}
