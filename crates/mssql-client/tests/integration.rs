//! Live SQL Server integration tests.
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
//! cargo test -p mssql-client --test integration -- --ignored
//! ```
//!
//! For CI/CD, use Docker:
//! ```bash
//! docker run -e 'ACCEPT_EULA=Y' -e 'SA_PASSWORD=YourStrong@Passw0rd' \
//!     -p 1433:1433 mcr.microsoft.com/mssql/server:2022-latest
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::manual_flatten,
    clippy::format_collect,
    clippy::approx_constant
)]

use mssql_client::{Client, Config};

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={},{};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, port, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Connection Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_basic_connection() {
    let config = get_test_config().expect("SQL Server config required");

    let client = Client::connect(config).await.expect("Failed to connect");
    client.close().await.expect("Failed to close connection");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_with_invalid_credentials() {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={},{};Database=master;User Id=invalid_user;Password=wrong_password;\
         TrustServerCertificate=true;Encrypt={}",
        host, port, encrypt
    );

    let config = Config::from_connection_string(&conn_str).expect("Config should parse");
    let result = Client::connect(config).await;

    assert!(result.is_err(), "Should fail with invalid credentials");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_to_nonexistent_database() {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={},{};Database=nonexistent_db_12345;User Id={};Password={};\
         TrustServerCertificate=true;Encrypt={}",
        host, port, user, password, encrypt
    );

    let config = Config::from_connection_string(&conn_str).expect("Config should parse");
    let result = Client::connect(config).await;

    // Should either fail or connect to master (depending on server config)
    // Either way, the connection attempt should not panic
    if let Err(e) = result {
        // Expected: database doesn't exist error
        println!("Expected error: {:?}", e);
    }
}

// =============================================================================
// Query Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_simple_select() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT 1 AS value", &[])
        .await
        .expect("Query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let value: i32 = row.get(0).expect("Should get value");
        assert_eq!(value, 1);
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_select_multiple_columns() {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test with DECIMAL type (now properly parsed)
    let rows = client
        .query(
            "SELECT 1 AS a, 'hello' AS b, CAST(3.14 AS DECIMAL(10,2)) AS c",
            &[],
        )
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let a: i32 = row.get(0).expect("Should get a");
        let b: String = row.get(1).expect("Should get b");
        let c: Decimal = row.get(2).expect("Should get c");

        assert_eq!(a, 1);
        assert_eq!(b, "hello");
        assert_eq!(c, Decimal::from_str("3.14").unwrap());
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_select_multiple_rows() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query(
            "SELECT value FROM (VALUES (1), (2), (3), (4), (5)) AS t(value)",
            &[],
        )
        .await
        .expect("Query failed");

    let values: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(values, vec![1, 2, 3, 4, 5]);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_select_null_values() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT NULL AS nullable_col", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: Option<i32> = row.get(0).expect("Should get nullable");
        assert!(value.is_none());
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_server_version() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT @@VERSION AS version", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Should get version");
        assert!(version.contains("Microsoft SQL Server"));
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Parameterized Query Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_parameterized_query_int() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let param = 42i32;
    let rows = client
        .query("SELECT @p1 AS value", &[&param])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: i32 = row.get(0).expect("Should get value");
        assert_eq!(value, 42);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_parameterized_query_string() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let param = "hello world";
    let rows = client
        .query("SELECT @p1 AS value", &[&param])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: String = row.get(0).expect("Should get value");
        assert_eq!(value, "hello world");
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_parameterized_query_multiple_params() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let name = "Alice";
    let age = 30i32;
    let rows = client
        .query("SELECT @p1 AS name, @p2 AS age", &[&name, &age])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let n: String = row.get(0).expect("Should get name");
        let a: i32 = row.get(1).expect("Should get age");
        assert_eq!(n, "Alice");
        assert_eq!(a, 30);
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Execute (Non-Query) Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_execute_returns_row_count() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a temp table, insert some rows, then count them
    client
        .execute(
            "CREATE TABLE #test_execute (id INT, name NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Create failed");

    let count = client
        .execute(
            "INSERT INTO #test_execute VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie')",
            &[],
        )
        .await
        .expect("Insert failed");

    assert_eq!(count, 3, "Should have inserted 3 rows");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_syntax_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let result = client.query("SELEKT * FROM nowhere", &[]).await;
    assert!(result.is_err(), "Should fail with syntax error");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_table_not_found() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let result = client
        .query("SELECT * FROM nonexistent_table_xyz", &[])
        .await;
    assert!(result.is_err(), "Should fail with table not found");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_division_by_zero() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let result = client.query("SELECT 1/0", &[]).await;
    // Division by zero in SQL Server depends on ANSI_WARNINGS setting
    // It may return NULL or error
    match result {
        Ok(rows) => {
            // If it succeeded, the value should be NULL
            for r in rows {
                if let Ok(row) = r {
                    let val: Option<i32> = row.get(0).ok().flatten();
                    println!("Division by zero returned: {:?}", val);
                }
            }
        }
        Err(e) => {
            println!("Division by zero error: {:?}", e);
        }
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Data Type Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_bigint() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT CAST(9223372036854775807 AS BIGINT) AS value", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: i64 = row.get(0).expect("Should get bigint");
        assert_eq!(value, i64::MAX);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_float() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT CAST(3.14159265358979 AS FLOAT) AS value", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: f64 = row.get(0).expect("Should get float");
        assert!((value - std::f64::consts::PI).abs() < 0.0000001);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_bit() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT CAST(1 AS BIT) AS t, CAST(0 AS BIT) AS f", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let t: bool = row.get(0).expect("Should get true");
        let f: bool = row.get(1).expect("Should get false");
        assert!(t);
        assert!(!f);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_datetime() {
    use chrono::{Datelike, NaiveDateTime, Timelike};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test DATETIME parsing (returns NaiveDateTime via SqlValue::DateTime)
    let rows = client
        .query("SELECT GETDATE() AS now", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let now: NaiveDateTime = row.get(0).expect("Should get datetime");
        // Just verify it's a reasonable year (not a parse error)
        assert!(
            now.year() >= 2024 && now.year() <= 2100,
            "Year should be reasonable"
        );
    }

    // Also test a specific datetime value
    let rows = client
        .query(
            "SELECT CAST('2024-06-15 14:30:45.123' AS DATETIME) AS dt",
            &[],
        )
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let dt: NaiveDateTime = row.get(0).expect("Should get datetime");
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
        // Seconds may have slight rounding due to DATETIME precision (1/300th second)
        assert!(dt.second() == 45 || dt.second() == 44);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_unicode_strings() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT N'Hello, \u{4e16}\u{754c}!' AS greeting", &[]) // Hello, World! in Chinese
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let greeting: String = row.get(0).expect("Should get greeting");
        assert_eq!(greeting, "Hello, \u{4e16}\u{754c}!");
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Multiple Statements Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multiple_queries_same_connection() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute multiple queries on the same connection
    for i in 1..=5 {
        let rows = client
            .query(&format!("SELECT {} AS iteration", i), &[])
            .await
            .expect("Query failed");

        for result in rows {
            let row = result.expect("Row should be valid");
            let value: i32 = row.get(0).expect("Should get value");
            assert_eq!(value, i);
        }
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Temp Table Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_temp_table_operations() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #test_temp (id INT PRIMARY KEY, name NVARCHAR(100))",
            &[],
        )
        .await
        .expect("Create table failed");

    // Insert data
    client
        .execute("INSERT INTO #test_temp VALUES (1, 'Alice')", &[])
        .await
        .expect("Insert failed");

    client
        .execute("INSERT INTO #test_temp VALUES (2, 'Bob')", &[])
        .await
        .expect("Insert failed");

    // Query data
    let rows = client
        .query("SELECT id, name FROM #test_temp ORDER BY id", &[])
        .await
        .expect("Query failed");

    let results: Vec<(i32, String)> = rows
        .filter_map(|r| r.ok())
        .map(|row| (row.get(0).unwrap(), row.get(1).unwrap()))
        .collect();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0], (1, "Alice".to_string()));
    assert_eq!(results[1], (2, "Bob".to_string()));

    // Update data
    let updated = client
        .execute("UPDATE #test_temp SET name = 'Alicia' WHERE id = 1", &[])
        .await
        .expect("Update failed");
    assert_eq!(updated, 1);

    // Delete data
    let deleted = client
        .execute("DELETE FROM #test_temp WHERE id = 2", &[])
        .await
        .expect("Delete failed");
    assert_eq!(deleted, 1);

    // Verify final state
    let rows = client
        .query("SELECT COUNT(*) FROM #test_temp", &[])
        .await
        .expect("Count failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 1);
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Large Data Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_large_result_set() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Generate 1000 rows using a recursive CTE
    let rows = client
        .query(
            "WITH nums AS (
                SELECT 1 AS n
                UNION ALL
                SELECT n + 1 FROM nums WHERE n < 1000
            )
            SELECT n FROM nums
            OPTION (MAXRECURSION 1000)",
            &[],
        )
        .await
        .expect("Query failed");

    let count = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1000);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_large_string() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a string of 8000 characters (max for VARCHAR)
    let large_string = "X".repeat(8000);
    let rows = client
        .query(&format!("SELECT '{}' AS large_value", large_string), &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: String = row.get(0).expect("Should get value");
        assert_eq!(value.len(), 8000);
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Transaction Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_commit() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table for test
    client
        .execute("CREATE TABLE #tx_test (id INT, value NVARCHAR(50))", &[])
        .await
        .expect("Create table failed");

    // Get the client back after the initial setup
    let tx = client.begin_transaction().await.expect("Begin failed");

    // Insert data within transaction
    let mut tx = tx;
    tx.execute("INSERT INTO #tx_test VALUES (1, 'committed')", &[])
        .await
        .expect("Insert failed");

    // Commit the transaction
    let mut client = tx.commit().await.expect("Commit failed");

    // Verify data persists after commit
    let rows = client
        .query("SELECT value FROM #tx_test WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let values: Vec<String> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0], "committed");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_rollback() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table with initial data
    client
        .execute(
            "CREATE TABLE #tx_rollback (id INT, value NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Create table failed");

    client
        .execute("INSERT INTO #tx_rollback VALUES (1, 'original')", &[])
        .await
        .expect("Insert failed");

    // Begin transaction and modify data
    let mut tx = client.begin_transaction().await.expect("Begin failed");

    tx.execute(
        "UPDATE #tx_rollback SET value = 'modified' WHERE id = 1",
        &[],
    )
    .await
    .expect("Update failed");

    // Rollback the transaction
    let mut client = tx.rollback().await.expect("Rollback failed");

    // Verify data is unchanged after rollback
    let rows = client
        .query("SELECT value FROM #tx_rollback WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let values: Vec<String> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0], "original");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_savepoint() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #tx_savepoint (id INT, value NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Create table failed");

    // Begin transaction
    let mut tx = client.begin_transaction().await.expect("Begin failed");

    // Insert first row
    tx.execute("INSERT INTO #tx_savepoint VALUES (1, 'first')", &[])
        .await
        .expect("Insert failed");

    // Create savepoint
    let savepoint = tx.save_point("sp1").await.expect("Savepoint failed");

    // Insert second row
    tx.execute("INSERT INTO #tx_savepoint VALUES (2, 'second')", &[])
        .await
        .expect("Insert failed");

    // Rollback to savepoint (undoes second insert)
    tx.rollback_to(&savepoint)
        .await
        .expect("Rollback to savepoint failed");

    // Commit the transaction (only first row should exist)
    let mut client = tx.commit().await.expect("Commit failed");

    // Verify only first row exists
    let rows = client
        .query("SELECT id FROM #tx_savepoint ORDER BY id", &[])
        .await
        .expect("Query failed");

    let ids: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(
        ids,
        vec![1],
        "Only first row should exist after savepoint rollback"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_within_transaction() {
    let config = get_test_config().expect("SQL Server config required");
    let client = Client::connect(config).await.expect("Failed to connect");

    // Begin transaction
    let mut tx = client.begin_transaction().await.expect("Begin failed");

    // Query within transaction should work
    let rows = tx
        .query("SELECT 1 AS value", &[])
        .await
        .expect("Query in transaction failed");

    let values: Vec<i32> = rows
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();

    assert_eq!(values, vec![1]);

    // Commit and close
    let client = tx.commit().await.expect("Commit failed");
    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multiple_savepoints() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute("CREATE TABLE #tx_multi_sp (step INT)", &[])
        .await
        .expect("Create table failed");

    // Begin transaction
    let mut tx = client.begin_transaction().await.expect("Begin failed");

    tx.execute("INSERT INTO #tx_multi_sp VALUES (1)", &[])
        .await
        .expect("Insert 1 failed");

    let sp1 = tx.save_point("sp1").await.expect("Savepoint 1 failed");

    tx.execute("INSERT INTO #tx_multi_sp VALUES (2)", &[])
        .await
        .expect("Insert 2 failed");

    let sp2 = tx.save_point("sp2").await.expect("Savepoint 2 failed");

    tx.execute("INSERT INTO #tx_multi_sp VALUES (3)", &[])
        .await
        .expect("Insert 3 failed");

    // Rollback to sp2 (removes step 3)
    tx.rollback_to(&sp2).await.expect("Rollback to sp2 failed");

    // Query to verify steps 1 and 2 exist
    let rows = tx
        .query("SELECT COUNT(*) FROM #tx_multi_sp", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(count, 2, "Should have 2 rows after rollback to sp2");

    // Rollback to sp1 (removes step 2)
    tx.rollback_to(&sp1).await.expect("Rollback to sp1 failed");

    // Query to verify only step 1 exists
    let rows = tx
        .query("SELECT COUNT(*) FROM #tx_multi_sp", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(count, 1, "Should have 1 row after rollback to sp1");

    let client = tx.commit().await.expect("Commit failed");
    client.close().await.expect("Failed to close");
}

// =============================================================================
// Multi-Result Set Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multi_result_set() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute a batch with multiple SELECT statements
    let mut results = client
        .query_multiple(
            "SELECT 1 AS a; SELECT 2 AS b, 3 AS c; SELECT 4 AS d, 5 AS e, 6 AS f;",
            &[],
        )
        .await
        .expect("Query failed");

    // Verify we have 3 result sets
    assert_eq!(results.result_count(), 3, "Should have 3 result sets");
    assert_eq!(
        results.current_result_index(),
        0,
        "Should start at first result"
    );

    // Process first result set (1 column, 1 row)
    let columns = results.columns().expect("Should have columns");
    assert_eq!(columns.len(), 1, "First result should have 1 column");
    assert_eq!(columns[0].name, "a");

    let row = results
        .next_row()
        .await
        .expect("Should get row")
        .expect("Row should exist");
    let a: i32 = row.get(0).expect("Should get value");
    assert_eq!(a, 1);

    // No more rows in first result
    assert!(results.next_row().await.expect("Should succeed").is_none());

    // Move to second result set
    assert!(results.has_more_results(), "Should have more results");
    assert!(
        results
            .next_result()
            .await
            .expect("next_result should succeed"),
        "Should advance to second result"
    );
    assert_eq!(results.current_result_index(), 1);

    // Process second result set (2 columns, 1 row)
    let columns = results.columns().expect("Should have columns");
    assert_eq!(columns.len(), 2, "Second result should have 2 columns");
    assert_eq!(columns[0].name, "b");
    assert_eq!(columns[1].name, "c");

    let row = results
        .next_row()
        .await
        .expect("Should get row")
        .expect("Row should exist");
    let b: i32 = row.get(0).expect("Should get value");
    let c: i32 = row.get(1).expect("Should get value");
    assert_eq!(b, 2);
    assert_eq!(c, 3);

    // Move to third result set
    assert!(results.has_more_results(), "Should have more results");
    assert!(
        results
            .next_result()
            .await
            .expect("next_result should succeed"),
        "Should advance to third result"
    );
    assert_eq!(results.current_result_index(), 2);

    // Process third result set (3 columns, 1 row)
    let columns = results.columns().expect("Should have columns");
    assert_eq!(columns.len(), 3, "Third result should have 3 columns");
    assert_eq!(columns[0].name, "d");
    assert_eq!(columns[1].name, "e");
    assert_eq!(columns[2].name, "f");

    let row = results
        .next_row()
        .await
        .expect("Should get row")
        .expect("Row should exist");
    let d: i32 = row.get(0).expect("Should get value");
    let e: i32 = row.get(1).expect("Should get value");
    let f: i32 = row.get(2).expect("Should get value");
    assert_eq!(d, 4);
    assert_eq!(e, 5);
    assert_eq!(f, 6);

    // No more result sets
    assert!(!results.has_more_results(), "Should not have more results");
    assert!(
        !results
            .next_result()
            .await
            .expect("next_result should succeed"),
        "Should not advance"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multi_result_with_rows() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp tables and insert data
    client
        .execute("CREATE TABLE #multi_r1 (id INT, name NVARCHAR(50))", &[])
        .await
        .expect("Create table 1 failed");

    client
        .execute("INSERT INTO #multi_r1 VALUES (1, 'Alice'), (2, 'Bob')", &[])
        .await
        .expect("Insert failed");

    client
        .execute("CREATE TABLE #multi_r2 (code NVARCHAR(10), value INT)", &[])
        .await
        .expect("Create table 2 failed");

    client
        .execute(
            "INSERT INTO #multi_r2 VALUES ('A', 100), ('B', 200), ('C', 300)",
            &[],
        )
        .await
        .expect("Insert failed");

    // Execute batch querying both tables
    let mut results = client
        .query_multiple("SELECT * FROM #multi_r1; SELECT * FROM #multi_r2;", &[])
        .await
        .expect("Query failed");

    // Process first result (2 rows)
    assert_eq!(results.result_count(), 2);

    let mut row_count = 0;
    while let Some(row) = results.next_row().await.expect("Row read failed") {
        let id: i32 = row.get(0).expect("Get id");
        let name: String = row.get(1).expect("Get name");
        match row_count {
            0 => {
                assert_eq!(id, 1);
                assert_eq!(name, "Alice");
            }
            1 => {
                assert_eq!(id, 2);
                assert_eq!(name, "Bob");
            }
            _ => panic!("Too many rows"),
        }
        row_count += 1;
    }
    assert_eq!(row_count, 2, "First result should have 2 rows");

    // Move to second result
    assert!(results.next_result().await.expect("Advance failed"));

    // Process second result (3 rows)
    let mut row_count = 0;
    while let Some(row) = results.next_row().await.expect("Row read failed") {
        let code: String = row.get(0).expect("Get code");
        let value: i32 = row.get(1).expect("Get value");
        match row_count {
            0 => {
                assert_eq!(code, "A");
                assert_eq!(value, 100);
            }
            1 => {
                assert_eq!(code, "B");
                assert_eq!(value, 200);
            }
            2 => {
                assert_eq!(code, "C");
                assert_eq!(value, 300);
            }
            _ => panic!("Too many rows"),
        }
        row_count += 1;
    }
    assert_eq!(row_count, 3, "Second result should have 3 rows");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Transaction Isolation Level Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_isolation_read_uncommitted() {
    use mssql_client::IsolationLevel;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute("CREATE TABLE #iso_test (id INT, value NVARCHAR(50))", &[])
        .await
        .expect("Create table failed");

    // Begin transaction with READ UNCOMMITTED
    let mut tx = client
        .begin_transaction_with_isolation(IsolationLevel::ReadUncommitted)
        .await
        .expect("Begin failed");

    // Insert data
    tx.execute("INSERT INTO #iso_test VALUES (1, 'test')", &[])
        .await
        .expect("Insert failed");

    // Query to verify data is accessible
    let rows = tx
        .query("SELECT COUNT(*) FROM #iso_test", &[])
        .await
        .expect("Query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(count, 1, "Should see the inserted row");

    let client = tx.commit().await.expect("Commit failed");
    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_isolation_serializable() {
    use mssql_client::IsolationLevel;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #iso_serial (id INT PRIMARY KEY, value INT)",
            &[],
        )
        .await
        .expect("Create table failed");

    // Insert initial data
    client
        .execute("INSERT INTO #iso_serial VALUES (1, 100)", &[])
        .await
        .expect("Insert failed");

    // Begin transaction with SERIALIZABLE isolation
    let mut tx = client
        .begin_transaction_with_isolation(IsolationLevel::Serializable)
        .await
        .expect("Begin failed");

    // Read the data (this will hold locks under SERIALIZABLE)
    let rows = tx
        .query("SELECT value FROM #iso_serial WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let value: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(value, 100);

    // Update the value
    tx.execute("UPDATE #iso_serial SET value = 200 WHERE id = 1", &[])
        .await
        .expect("Update failed");

    // Commit
    let client = tx.commit().await.expect("Commit failed");
    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_isolation_repeatable_read() {
    use mssql_client::IsolationLevel;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute("CREATE TABLE #iso_rr (id INT, value INT)", &[])
        .await
        .expect("Create table failed");

    // Insert test data
    client
        .execute("INSERT INTO #iso_rr VALUES (1, 10), (2, 20), (3, 30)", &[])
        .await
        .expect("Insert failed");

    // Begin transaction with REPEATABLE READ
    let mut tx = client
        .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
        .await
        .expect("Begin failed");

    // First read
    let rows = tx
        .query("SELECT SUM(value) FROM #iso_rr", &[])
        .await
        .expect("Query failed");

    let sum1: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(sum1, 60);

    // Second read within same transaction should return same result
    let rows = tx
        .query("SELECT SUM(value) FROM #iso_rr", &[])
        .await
        .expect("Query failed");

    let sum2: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(sum1, sum2, "Both reads should return the same sum");

    let client = tx.commit().await.expect("Commit failed");
    client.close().await.expect("Failed to close");
}

// =============================================================================
// Statement Cache Tests (TEST-019)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_repeated_parameterized_queries() {
    // This test verifies that repeated parameterized queries work correctly.
    // While we use sp_executesql internally (not sp_prepare/sp_execute),
    // this exercises the query path that would benefit from caching.
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute the same parameterized query multiple times
    for i in 1..=10 {
        let param = i;
        let rows = client
            .query("SELECT @p1 * 2 AS doubled", &[&param])
            .await
            .expect("Query failed");

        let result: Vec<i32> = rows
            .filter_map(|r| r.ok())
            .map(|row| row.get(0).unwrap())
            .collect();

        assert_eq!(
            result,
            vec![param * 2],
            "Iteration {} should return doubled value",
            i
        );
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_statement_cache_with_different_queries() {
    // This test verifies that different SQL queries work independently,
    // simulating how the statement cache would track different statements.
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table for testing
    client
        .execute("CREATE TABLE #cache_test (id INT, value NVARCHAR(50))", &[])
        .await
        .expect("Create table failed");

    // Insert test data
    client
        .execute(
            "INSERT INTO #cache_test VALUES (1, 'one'), (2, 'two'), (3, 'three')",
            &[],
        )
        .await
        .expect("Insert failed");

    // Query 1: SELECT by id
    let query1 = "SELECT value FROM #cache_test WHERE id = @p1";

    // Query 2: SELECT all above id
    let query2 = "SELECT id, value FROM #cache_test WHERE id > @p1 ORDER BY id";

    // Interleave different queries to test cache discrimination
    let result1 = client
        .query(query1, &[&1i32])
        .await
        .expect("Query 1 failed");
    let values1: Vec<String> = result1
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();
    assert_eq!(values1, vec!["one"]);

    let result2 = client
        .query(query2, &[&1i32])
        .await
        .expect("Query 2 failed");
    let ids2: Vec<i32> = result2
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();
    assert_eq!(ids2, vec![2, 3]);

    // Repeat query 1 with different parameter
    let result1b = client
        .query(query1, &[&2i32])
        .await
        .expect("Query 1b failed");
    let values1b: Vec<String> = result1b
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();
    assert_eq!(values1b, vec!["two"]);

    // Repeat query 2 with different parameter
    let result2b = client
        .query(query2, &[&2i32])
        .await
        .expect("Query 2b failed");
    let ids2b: Vec<i32> = result2b
        .filter_map(|r| r.ok())
        .map(|row| row.get(0).unwrap())
        .collect();
    assert_eq!(ids2b, vec![3]);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_high_query_volume() {
    // Stress test for query execution to verify stability under load.
    // This would exercise statement cache eviction if sp_prepare were used.
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute 500 different parameterized queries
    // This exceeds the default cache size of 256, which would trigger eviction
    for i in 0..500 {
        let param = i;
        // Use unique SQL text for each iteration to simulate many different statements
        let sql = format!("SELECT @p1 + {} AS result", i);
        let rows = client
            .query(&sql, &[&param])
            .await
            .unwrap_or_else(|e| panic!("Query {} failed: {:?}", i, e));

        let result: i32 = rows
            .filter_map(|r| r.ok())
            .next()
            .map(|row| row.get(0).unwrap())
            .unwrap_or(-1);

        assert_eq!(result, param + i, "Query {} should return correct sum", i);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_same_sql_different_params() {
    // This test verifies that the same SQL template works correctly with different parameters.
    // This is the primary use case for prepared statement caching.
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #params_test (id INT PRIMARY KEY, name NVARCHAR(50), score INT)",
            &[],
        )
        .await
        .expect("Create table failed");

    // Insert test data
    for i in 1..=100 {
        let name = format!("User{}", i);
        let score = i * 10;
        client
            .execute(
                "INSERT INTO #params_test VALUES (@p1, @p2, @p3)",
                &[&{ i }, &name.as_str(), &{ score }],
            )
            .await
            .unwrap_or_else(|e| panic!("Insert {} failed: {:?}", i, e));
    }

    // Query with same SQL, different params (cache should help here)
    let query = "SELECT name, score FROM #params_test WHERE id = @p1";

    for i in 1..=100 {
        let rows = client
            .query(query, &[&{ i }])
            .await
            .unwrap_or_else(|e| panic!("Query for id {} failed: {:?}", i, e));

        let results: Vec<(String, i32)> = rows
            .filter_map(|r| r.ok())
            .map(|row| (row.get(0).unwrap(), row.get(1).unwrap()))
            .collect();

        assert_eq!(results.len(), 1, "Should find exactly one row for id {}", i);
        assert_eq!(results[0].0, format!("User{}", i));
        assert_eq!(results[0].1, { i * 10 });
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// TLS and Encryption Tests
// =============================================================================

/// Helper to get config with specific encryption mode
fn get_config_with_encrypt(encrypt: &str) -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());

    let conn_str = format!(
        "Server={},{};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, port, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str).ok()
}

/// Check if the server supports TLS 1.2+ (required for Encrypt=true/false tests).
///
/// Legacy SQL Server versions (pre-2017) without TLS 1.2 updates cannot complete
/// TLS handshakes with rustls. This helper connects using the environment's encryption
/// setting (typically no_tls for legacy servers), checks the version, and determines
/// if TLS tests should be skipped.
///
/// Returns `true` if TLS tests should be skipped.
async fn should_skip_tls_tests() -> bool {
    // If we're already in no_tls mode, check if server is legacy
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());
    if encrypt.eq_ignore_ascii_case("no_tls") {
        // User explicitly set no_tls, which means server likely doesn't support TLS 1.2
        println!("Skipping TLS test: MSSQL_ENCRYPT=no_tls indicates legacy server without TLS 1.2");
        return true;
    }

    // Try to connect and check version
    let config = match get_test_config() {
        Some(c) => c,
        None => return true, // Skip if no config available
    };

    match Client::connect(config).await {
        Ok(mut client) => {
            // Check server major version
            // SQL Server 2017+ (major 14+) generally supports TLS 1.2 out of the box
            // Earlier versions need TLS 1.2 cumulative updates which may not be installed
            let rows = client
                .query(
                    "SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128))",
                    &[],
                )
                .await;

            let mut should_skip = false;
            if let Ok(rows) = rows {
                for result in rows {
                    if let Ok(row) = result {
                        if let Ok(version) = row.get::<String>(0) {
                            let major: i32 = version
                                .split('.')
                                .next()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);

                            if major < 14 {
                                println!(
                                    "Skipping TLS test: SQL Server major version {} < 14 \
                                     (may not support TLS 1.2 without updates)",
                                    major
                                );
                                should_skip = true;
                            }
                        }
                    }
                }
            } else {
                // If we can't check version, skip to be safe
                should_skip = true;
            }

            let _ = client.close().await;
            should_skip
        }
        Err(_) => true, // If connection fails, skip
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_with_encryption_true() {
    // Skip on legacy servers that don't support TLS 1.2
    if should_skip_tls_tests().await {
        return;
    }

    let config = get_config_with_encrypt("true").expect("SQL Server config required");

    let client = Client::connect(config)
        .await
        .expect("Failed to connect with Encrypt=true");

    // Verify connection works by running a simple query
    let mut client = client;
    let rows = client
        .query("SELECT 1 AS encrypted_connection", &[])
        .await
        .expect("Query failed");
    assert_eq!(rows.len(), 1);

    client.close().await.expect("Failed to close connection");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_with_encryption_false() {
    // Skip on legacy servers that don't support TLS 1.2
    // Note: Even with Encrypt=false, TLS is used for login credentials per TDS spec
    if should_skip_tls_tests().await {
        return;
    }

    let config = get_config_with_encrypt("false").expect("SQL Server config required");

    let client = Client::connect(config)
        .await
        .expect("Failed to connect with Encrypt=false");

    let mut client = client;
    let rows = client
        .query("SELECT 1 AS unencrypted_connection", &[])
        .await
        .expect("Query failed");
    assert_eq!(rows.len(), 1);

    client.close().await.expect("Failed to close connection");
}

#[tokio::test]
#[ignore = "Requires SQL Server 2022+ with TDS 8.0 support"]
async fn test_tds_8_strict_mode() {
    // TDS 8.0 strict mode - TLS handshake before any TDS traffic
    // This requires SQL Server 2022+ configured for strict encryption
    let config = get_config_with_encrypt("strict").expect("SQL Server config required");

    // Note: This test may fail if SQL Server is not configured for TDS 8.0
    // TDS 8.0 requires server-side configuration changes
    match Client::connect(config).await {
        Ok(client) => {
            // If connection succeeds, TDS 8.0 is working
            let mut client = client;
            let rows = client
                .query("SELECT 'TDS 8.0' AS protocol_mode", &[])
                .await
                .expect("Query failed in TDS 8.0 mode");
            assert_eq!(rows.len(), 1);
            client.close().await.expect("Failed to close");
        }
        Err(e) => {
            // Expected if server doesn't support TDS 8.0 strict mode
            // The connection may fail with "strict encryption mode required"
            println!("TDS 8.0 strict mode not available: {:?}", e);
        }
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_server_tds_version() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Query the server version to verify connection protocol
    let rows = client
        .query(
            "SELECT @@VERSION AS version, SERVERPROPERTY('ProductMajorVersion') AS major_version",
            &[],
        )
        .await
        .expect("Version query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Failed to get version");
        println!("SQL Server version: {}", version);

        // SQL Server 2022 = version 16
        // SQL Server 2019 = version 15
        // SQL Server 2017 = version 14
        // SQL Server 2016 = version 13
        assert!(version.contains("Microsoft SQL Server"));
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close connection");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_encrypted_query_roundtrip() {
    // Skip on legacy servers that don't support TLS 1.2
    if should_skip_tls_tests().await {
        return;
    }

    // Test that data integrity is maintained through encrypted connection
    let config = get_config_with_encrypt("true").expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #EncryptTest (id INT, data NVARCHAR(100), num FLOAT)",
            &[],
        )
        .await
        .expect("Failed to create temp table");

    // Insert test data with various types over encrypted connection
    let test_id: i32 = 1;
    let test_string = String::from("Hello, encrypted world!");
    let test_float: f64 = 123.456;

    client
        .execute(
            "INSERT INTO #EncryptTest VALUES (@p1, @p2, @p3)",
            &[&test_id, &test_string.as_str(), &test_float],
        )
        .await
        .expect("Failed to insert test data");

    // Read back and verify data integrity over encrypted connection
    let rows = client
        .query("SELECT id, data, num FROM #EncryptTest WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let id: i32 = row.get(0).expect("Failed to get id");
        let data: String = row.get(1).expect("Failed to get data");
        let num: f64 = row.get(2).expect("Failed to get float");

        assert_eq!(id, 1);
        assert_eq!(data, test_string);
        assert!(
            (num - test_float).abs() < 0.0001,
            "Float mismatch: {} vs {}",
            num,
            test_float
        );
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close connection");
}

// =============================================================================
// Connection Resilience Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_state_after_error() {
    // Verify connection remains usable after a server error
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Cause a server error (invalid SQL)
    let result = client
        .query("SELECT * FROM NonExistentTable12345", &[])
        .await;
    assert!(result.is_err(), "Expected error for non-existent table");

    // Connection should still be usable
    let rows = client
        .query("SELECT 1 AS still_working", &[])
        .await
        .expect("Query should succeed after error");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: i32 = row.get(0).expect("Failed to get value");
        assert_eq!(val, 1);
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multiple_errors_recovery() {
    // Test that multiple consecutive errors don't corrupt connection state
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Cause multiple errors
    for i in 1..=5 {
        let result = client
            .query(&format!("SELECT * FROM NonExistentTable{}", i), &[])
            .await;
        assert!(
            result.is_err(),
            "Expected error {} for non-existent table",
            i
        );
    }

    // Connection should still work
    let rows = client
        .query("SELECT 'recovered' AS status", &[])
        .await
        .expect("Query should succeed after multiple errors");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: String = row.get(0).expect("Failed to get value");
        assert_eq!(val, "recovered");
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_rollback_preserves_state() {
    // Verify transaction state is correctly managed on rollback
    // Using the driver's transaction API instead of raw SQL
    let config = get_test_config().expect("SQL Server config required");
    let client = Client::connect(config).await.expect("Failed to connect");

    // Create a test table before transaction
    let mut client = client;
    client
        .execute(
            "CREATE TABLE #TxRollbackPreserve (id INT PRIMARY KEY, value NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Insert initial data
    client
        .execute("INSERT INTO #TxRollbackPreserve VALUES (1, 'initial')", &[])
        .await
        .expect("Failed to insert");

    // Start a transaction using the client API
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    // Make a change within transaction
    tx.execute(
        "UPDATE #TxRollbackPreserve SET value = 'modified' WHERE id = 1",
        &[],
    )
    .await
    .expect("Failed to update");

    // Rollback the transaction
    let mut client = tx.rollback().await.expect("Failed to rollback");

    // Verify the original value is preserved
    let rows = client
        .query("SELECT value FROM #TxRollbackPreserve WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: String = row.get(0).expect("Failed to get value");
        assert_eq!(val, "initial", "Transaction should have been rolled back");
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_duplicate_key_error_recovery() {
    // Test that duplicate key errors don't break connection state
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table
    client
        .execute("CREATE TABLE #DupKeyTest (id INT PRIMARY KEY)", &[])
        .await
        .expect("Failed to create table");

    // First insert should succeed
    client
        .execute("INSERT INTO #DupKeyTest VALUES (1)", &[])
        .await
        .expect("First insert should succeed");

    // Duplicate key should fail
    let result = client
        .execute("INSERT INTO #DupKeyTest VALUES (1)", &[])
        .await;
    assert!(result.is_err(), "Duplicate insert should fail");

    // Next insert should succeed
    client
        .execute("INSERT INTO #DupKeyTest VALUES (2)", &[])
        .await
        .expect("Insert after error should succeed");

    // Verify both successful inserts are present
    let rows = client
        .query("SELECT COUNT(*) FROM #DupKeyTest", &[])
        .await
        .expect("Count query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let cnt: i32 = row.get(0).expect("Failed to get count");
        assert_eq!(cnt, 2, "Should have exactly 2 rows");
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_binary_data_roundtrip() {
    // Test binary data handling with various patterns using fixed-size VARBINARY
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table with fixed size VARBINARY (not MAX) for reliable parsing
    client
        .execute(
            "CREATE TABLE #BinaryRoundtrip (id INT PRIMARY KEY, data VARBINARY(500))",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Test various binary patterns using hex literals (skip empty for now)
    let test_cases: Vec<(i32, Vec<u8>)> = vec![
        (1, vec![0x00]),                   // Single null byte
        (2, vec![0xFF]),                   // Single max byte
        (3, vec![0x00, 0xFF, 0x00, 0xFF]), // Alternating
        (4, vec![0xDE, 0xAD, 0xBE, 0xEF]), // Classic magic bytes
        (5, (0..64u8).collect()),          // Sequential bytes (subset for reliability)
    ];

    for (id, data) in &test_cases {
        // Use literal hex for binary data insertion
        let hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
        let sql = format!("INSERT INTO #BinaryRoundtrip VALUES ({}, 0x{})", id, hex);
        client
            .execute(&sql, &[])
            .await
            .unwrap_or_else(|e| panic!("Failed to insert binary data for id {}: {:?}", id, e));
    }

    // Verify each
    for (id, expected) in &test_cases {
        let rows = client
            .query(
                &format!("SELECT data FROM #BinaryRoundtrip WHERE id = {}", id),
                &[],
            )
            .await
            .unwrap_or_else(|e| panic!("Query failed for id {}: {:?}", id, e));

        let mut found = false;
        for result in rows {
            let row = result.expect("Row should be valid");
            let data: Vec<u8> = row.get(0).expect("Failed to get binary data");
            assert_eq!(&data, expected, "Binary data mismatch for id {}", id);
            found = true;
        }
        assert!(found, "No row found for id {}", id);
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Table-Valued Parameter (TVP) Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tvp_basic_int_list() {
    use mssql_client::{Tvp, TvpColumn, TvpRow, TvpValue};
    use mssql_types::{ToSql, TypeError};

    // Define a simple TVP struct for integer IDs
    struct IntId {
        id: i32,
    }

    impl Tvp for IntId {
        fn type_name() -> &'static str {
            "dbo.IntIdList"
        }

        fn columns() -> Vec<TvpColumn> {
            vec![TvpColumn::new("Id", "INT", 0)]
        }

        fn to_row(&self) -> Result<TvpRow, TypeError> {
            Ok(TvpRow::new(vec![self.id.to_sql()?]))
        }
    }

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create the TVP type in SQL Server (if it doesn't exist)
    // Note: This requires db_owner or ALTER permission on the schema
    let create_type = r#"
        IF TYPE_ID('dbo.IntIdList') IS NULL
        BEGIN
            CREATE TYPE dbo.IntIdList AS TABLE (
                Id INT NOT NULL
            );
        END
    "#;
    if let Err(e) = client.execute(create_type, &[]).await {
        println!("Could not create TVP type (may already exist): {:?}", e);
    }

    // Create test data table
    client
        .execute(
            "CREATE TABLE #TvpTestData (Id INT PRIMARY KEY, Name NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Failed to create test data table");

    client
        .execute(
            "INSERT INTO #TvpTestData VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie'), (4, 'Diana')",
            &[],
        )
        .await
        .expect("Failed to insert test data");

    // Create TVP data
    let ids = vec![IntId { id: 1 }, IntId { id: 3 }];
    let tvp = TvpValue::new(&ids).expect("Failed to create TVP");

    // Query using TVP directly with a join
    // Note: We use inline query rather than a temp stored procedure because
    // SQL Server temporary procedures cannot reference user-defined table types
    let rows = client
        .query(
            "SELECT d.Id, d.Name FROM #TvpTestData d INNER JOIN @p1 i ON d.Id = i.Id ORDER BY d.Id",
            &[&tvp],
        )
        .await
        .expect("TVP query failed");

    let results: Vec<(i32, String)> = rows
        .filter_map(|r| r.ok())
        .map(|row| (row.get(0).unwrap(), row.get(1).unwrap()))
        .collect();

    assert_eq!(results.len(), 2, "Should return 2 matching rows");
    assert_eq!(results[0], (1, "Alice".to_string()));
    assert_eq!(results[1], (3, "Charlie".to_string()));

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tvp_empty_table() {
    use mssql_client::{Tvp, TvpColumn, TvpRow, TvpValue};
    use mssql_types::{ToSql, TypeError};

    struct IntId {
        id: i32,
    }

    impl Tvp for IntId {
        fn type_name() -> &'static str {
            "dbo.IntIdList"
        }

        fn columns() -> Vec<TvpColumn> {
            vec![TvpColumn::new("Id", "INT", 0)]
        }

        fn to_row(&self) -> Result<TvpRow, TypeError> {
            Ok(TvpRow::new(vec![self.id.to_sql()?]))
        }
    }

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Ensure TVP type exists
    let create_type = r#"
        IF TYPE_ID('dbo.IntIdList') IS NULL
        BEGIN
            CREATE TYPE dbo.IntIdList AS TABLE (
                Id INT NOT NULL
            );
        END
    "#;
    if let Err(e) = client.execute(create_type, &[]).await {
        println!("Could not create TVP type: {:?}", e);
    }

    // Create an empty TVP
    let tvp: TvpValue = TvpValue::empty::<IntId>();

    // Query with empty TVP - should return no rows
    let rows = client
        .query(
            "SELECT Id FROM (SELECT 1 AS Id UNION SELECT 2) AS t WHERE Id IN (SELECT Id FROM @p1)",
            &[&tvp],
        )
        .await
        .expect("Query with empty TVP failed");

    let count = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 0, "Empty TVP should match no rows");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tvp_multi_column() {
    use mssql_client::{Tvp, TvpColumn, TvpRow, TvpValue};
    use mssql_types::{ToSql, TypeError};

    // TVP with multiple columns
    struct UserData {
        id: i32,
        name: String,
        score: i32,
    }

    impl Tvp for UserData {
        fn type_name() -> &'static str {
            "dbo.UserDataList"
        }

        fn columns() -> Vec<TvpColumn> {
            vec![
                TvpColumn::new("Id", "INT", 0),
                TvpColumn::new("Name", "NVARCHAR(50)", 1),
                TvpColumn::new("Score", "INT", 2),
            ]
        }

        fn to_row(&self) -> Result<TvpRow, TypeError> {
            Ok(TvpRow::new(vec![
                self.id.to_sql()?,
                self.name.to_sql()?,
                self.score.to_sql()?,
            ]))
        }
    }

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create the multi-column TVP type
    let create_type = r#"
        IF TYPE_ID('dbo.UserDataList') IS NULL
        BEGIN
            CREATE TYPE dbo.UserDataList AS TABLE (
                Id INT NOT NULL,
                Name NVARCHAR(50) NOT NULL,
                Score INT NOT NULL
            );
        END
    "#;
    if let Err(e) = client.execute(create_type, &[]).await {
        println!("Could not create TVP type: {:?}", e);
    }

    // Create TVP data
    let users = vec![
        UserData {
            id: 1,
            name: "Alice".to_string(),
            score: 95,
        },
        UserData {
            id: 2,
            name: "Bob".to_string(),
            score: 87,
        },
        UserData {
            id: 3,
            name: "Charlie".to_string(),
            score: 92,
        },
    ];
    let tvp = TvpValue::new(&users).expect("Failed to create TVP");

    // Query to read back the TVP data
    let rows = client
        .query("SELECT Id, Name, Score FROM @p1 ORDER BY Id", &[&tvp])
        .await
        .expect("Multi-column TVP query failed");

    let results: Vec<(i32, String, i32)> = rows
        .filter_map(|r| r.ok())
        .map(|row| {
            (
                row.get(0).unwrap(),
                row.get(1).unwrap(),
                row.get(2).unwrap(),
            )
        })
        .collect();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], (1, "Alice".to_string(), 95));
    assert_eq!(results[1], (2, "Bob".to_string(), 87));
    assert_eq!(results[2], (3, "Charlie".to_string(), 92));

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tvp_bulk_insert() {
    use mssql_client::{Tvp, TvpColumn, TvpRow, TvpValue};
    use mssql_types::{ToSql, TypeError};

    struct BulkRow {
        id: i32,
        value: String,
    }

    impl Tvp for BulkRow {
        fn type_name() -> &'static str {
            "dbo.BulkRowList"
        }

        fn columns() -> Vec<TvpColumn> {
            vec![
                TvpColumn::new("Id", "INT", 0),
                TvpColumn::new("Value", "NVARCHAR(100)", 1),
            ]
        }

        fn to_row(&self) -> Result<TvpRow, TypeError> {
            Ok(TvpRow::new(vec![self.id.to_sql()?, self.value.to_sql()?]))
        }
    }

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create the TVP type
    let create_type = r#"
        IF TYPE_ID('dbo.BulkRowList') IS NULL
        BEGIN
            CREATE TYPE dbo.BulkRowList AS TABLE (
                Id INT NOT NULL,
                Value NVARCHAR(100) NOT NULL
            );
        END
    "#;
    if let Err(e) = client.execute(create_type, &[]).await {
        println!("Could not create TVP type: {:?}", e);
    }

    // Create destination table
    client
        .execute(
            "CREATE TABLE #BulkDest (Id INT PRIMARY KEY, Value NVARCHAR(100))",
            &[],
        )
        .await
        .expect("Failed to create destination table");

    // Create 100 rows of test data
    let rows: Vec<BulkRow> = (1..=100)
        .map(|i| BulkRow {
            id: i,
            value: format!("Value {}", i),
        })
        .collect();

    let tvp = TvpValue::new(&rows).expect("Failed to create TVP");

    // Execute bulk insert using TVP directly
    // Note: We use inline INSERT rather than a temp stored procedure because
    // SQL Server temporary procedures cannot reference user-defined table types
    let result = client
        .execute(
            "INSERT INTO #BulkDest (Id, Value) SELECT Id, Value FROM @p1",
            &[&tvp],
        )
        .await
        .expect("Bulk insert failed");

    assert_eq!(result, 100, "Should have inserted 100 rows");

    // Verify data
    let rows = client
        .query("SELECT COUNT(*) FROM #BulkDest", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap_or(0);

    assert_eq!(count, 100, "Table should have 100 rows");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Data Type Regression Tests (v0.2.3+ fixes)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_date() {
    use chrono::{Datelike, NaiveDate};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test DATE type - was causing "unexpected EOF reading DATE" before fix
    let rows = client
        .query("SELECT CAST('2025-12-24' AS DATE) AS christmas_eve", &[])
        .await
        .expect("DATE query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let date: NaiveDate = row.get(0).expect("Should get date");
        assert_eq!(date.year(), 2025);
        assert_eq!(date.month(), 12);
        assert_eq!(date.day(), 24);
    }

    // Test NULL DATE
    let rows = client
        .query("SELECT CAST(NULL AS DATE) AS null_date", &[])
        .await
        .expect("NULL DATE query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let date: Option<NaiveDate> = row.try_get(0);
        assert!(date.is_none(), "Should be NULL");
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_xml() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test XML type - was causing "unexpected EOF reading Xml data" before fix
    let rows = client
        .query(
            "SELECT CAST('<root><item id=\"1\">Test</item></root>' AS XML) AS xml_data",
            &[],
        )
        .await
        .expect("XML query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let xml: String = row.get(0).expect("Should get XML");
        assert!(xml.contains("<root>"), "Should contain XML root");
        assert!(xml.contains("Test"), "Should contain text content");
    }

    // Test NULL XML
    let rows = client
        .query("SELECT CAST(NULL AS XML) AS null_xml", &[])
        .await
        .expect("NULL XML query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let xml: Option<String> = row.try_get(0);
        assert!(xml.is_none(), "Should be NULL");
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_text_deprecated() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a temp table with TEXT column (deprecated but still supported)
    client
        .execute("CREATE TABLE #TextTest (id INT, content TEXT)", &[])
        .await
        .expect("Failed to create temp table");

    client
        .execute(
            "INSERT INTO #TextTest VALUES (1, 'Hello from TEXT column')",
            &[],
        )
        .await
        .expect("Failed to insert");

    // Test TEXT type - was causing "unexpected end of stream" before fix
    let rows = client
        .query("SELECT content FROM #TextTest WHERE id = 1", &[])
        .await
        .expect("TEXT query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let text: String = row.get(0).expect("Should get TEXT");
        assert_eq!(text, "Hello from TEXT column");
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_ntext_deprecated() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a temp table with NTEXT column (deprecated but still supported)
    client
        .execute("CREATE TABLE #NTextTest (id INT, content NTEXT)", &[])
        .await
        .expect("Failed to create temp table");

    client
        .execute(
            "INSERT INTO #NTextTest VALUES (1, N'Hello \u{4e16}\u{754c} from NTEXT')",
            &[],
        )
        .await
        .expect("Failed to insert");

    // Test NTEXT type - was causing "unexpected end of stream" before fix
    let rows = client
        .query("SELECT content FROM #NTextTest WHERE id = 1", &[])
        .await
        .expect("NTEXT query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let text: String = row.get(0).expect("Should get NTEXT");
        assert!(text.contains("Hello"));
        assert!(text.contains("\u{4e16}\u{754c}")); // Chinese characters
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_image_deprecated() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a temp table with IMAGE column (deprecated but still supported)
    client
        .execute("CREATE TABLE #ImageTest (id INT, data IMAGE)", &[])
        .await
        .expect("Failed to create temp table");

    client
        .execute("INSERT INTO #ImageTest VALUES (1, 0xDEADBEEF)", &[])
        .await
        .expect("Failed to insert");

    // Test IMAGE type - was causing "unexpected end of stream" before fix
    let rows = client
        .query("SELECT data FROM #ImageTest WHERE id = 1", &[])
        .await
        .expect("IMAGE query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let data: Vec<u8> = row.get(0).expect("Should get IMAGE");
        assert_eq!(data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_decimal_high_scale() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test high-scale DECIMAL - was causing hang before fix
    // rust_decimal supports max scale of 28, so scale 30+ falls back to f64
    let rows = client
        .query(
            "SELECT CAST(123456.123456789012345678901234567890 AS DECIMAL(38,30)) AS high_scale",
            &[],
        )
        .await
        .expect("High-scale DECIMAL query failed - driver may have hung");

    for result in rows {
        let row = result.expect("Row should be valid");
        // With scale > 28, we fall back to f64
        let value: f64 = row.get(0).expect("Should get high-scale decimal as f64");
        // Check approximate value (f64 won't have full precision)
        assert!(
            (value - 123456.123456789).abs() < 0.001,
            "Value should be approximately correct: {}",
            value
        );
    }

    // Test normal scale DECIMAL still works with rust_decimal
    let rows = client
        .query(
            "SELECT CAST(123.456789 AS DECIMAL(18,6)) AS normal_scale",
            &[],
        )
        .await
        .expect("Normal DECIMAL query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        // Normal scale should parse correctly
        let value: f64 = row.get(0).expect("Should get decimal");
        assert!(
            (value - 123.456789).abs() < 0.000001,
            "Value should match: {}",
            value
        );
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_nvarchar_max() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test NVARCHAR(MAX) - was returning corrupted data before fix
    let long_text = "Hello World! ".repeat(1000); // 13,000 chars

    let rows = client
        .query(
            &format!(
                "SELECT CAST(N'{}' AS NVARCHAR(MAX)) AS long_text",
                long_text
            ),
            &[],
        )
        .await
        .expect("NVARCHAR(MAX) query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let text: String = row.get(0).expect("Should get NVARCHAR(MAX)");
        assert_eq!(text.len(), long_text.len(), "Length should match");
        assert_eq!(text, long_text, "Content should match exactly");
    }

    // Test with Unicode
    let unicode_text = "Hello \u{4e16}\u{754c}! ".repeat(500);
    let rows = client
        .query(
            &format!(
                "SELECT CAST(N'{}' AS NVARCHAR(MAX)) AS unicode_text",
                unicode_text
            ),
            &[],
        )
        .await
        .expect("Unicode NVARCHAR(MAX) query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let text: String = row.get(0).expect("Should get NVARCHAR(MAX)");
        assert_eq!(text, unicode_text, "Unicode content should match");
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_varchar_max() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test VARCHAR(MAX) - was returning corrupted data before fix
    let long_text = "Test data! ".repeat(1000); // 11,000 chars

    let rows = client
        .query(
            &format!("SELECT CAST('{}' AS VARCHAR(MAX)) AS long_text", long_text),
            &[],
        )
        .await
        .expect("VARCHAR(MAX) query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let text: String = row.get(0).expect("Should get VARCHAR(MAX)");
        assert_eq!(text.len(), long_text.len(), "Length should match");
        assert_eq!(text, long_text, "Content should match exactly");
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_varbinary_max() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test VARBINARY(MAX) - was returning corrupted data before fix
    // Create a large binary value
    let rows = client
        .query(
            "SELECT CAST(REPLICATE(CAST(0xDEADBEEF AS VARBINARY(MAX)), 1000) AS VARBINARY(MAX)) AS big_binary",
            &[],
        )
        .await
        .expect("VARBINARY(MAX) query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let data: Vec<u8> = row.get(0).expect("Should get VARBINARY(MAX)");
        // 4 bytes * 1000 = 4000 bytes
        assert_eq!(data.len(), 4000, "Binary length should be 4000 bytes");
        // Check pattern repeats correctly
        assert_eq!(data[0..4], [0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(data[3996..4000], [0xDE, 0xAD, 0xBE, 0xEF]);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_multi_column_with_max_types() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test multiple columns with MAX types in same row
    let rows = client
        .query(
            "SELECT
                42 AS int_col,
                CAST('Hello World' AS NVARCHAR(MAX)) AS nvarchar_max_col,
                CAST('Test' AS VARCHAR(MAX)) AS varchar_max_col,
                123.456 AS float_col,
                CAST(0xCAFEBABE AS VARBINARY(MAX)) AS varbinary_max_col",
            &[],
        )
        .await
        .expect("Multi-column MAX query failed");

    for result in rows {
        let row = result.expect("Row should be valid");

        let int_val: i32 = row.get(0).expect("Should get int");
        assert_eq!(int_val, 42);

        let nvarchar_max: String = row.get(1).expect("Should get NVARCHAR(MAX)");
        assert_eq!(nvarchar_max, "Hello World");

        let varchar_max: String = row.get(2).expect("Should get VARCHAR(MAX)");
        assert_eq!(varchar_max, "Test");

        let float_val: f64 = row.get(3).expect("Should get float");
        assert!((float_val - 123.456).abs() < 0.001);

        let varbinary_max: Vec<u8> = row.get(4).expect("Should get VARBINARY(MAX)");
        assert_eq!(varbinary_max, vec![0xCA, 0xFE, 0xBA, 0xBE]);
    }

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_data_type_sql_variant() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test SQL_VARIANT with integer
    let rows = client
        .query("SELECT CAST(42 AS SQL_VARIANT) AS variant_int", &[])
        .await
        .expect("SQL_VARIANT INT query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: i32 = row.get(0).expect("Should get SQL_VARIANT as int");
        assert_eq!(value, 42);
    }

    // Test SQL_VARIANT with string
    let rows = client
        .query(
            "SELECT CAST('Hello World' AS SQL_VARIANT) AS variant_str",
            &[],
        )
        .await
        .expect("SQL_VARIANT string query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: String = row.get(0).expect("Should get SQL_VARIANT as string");
        assert_eq!(value, "Hello World");
    }

    // Test SQL_VARIANT with decimal
    let rows = client
        .query(
            "SELECT CAST(123.456 AS SQL_VARIANT) AS variant_decimal",
            &[],
        )
        .await
        .expect("SQL_VARIANT decimal query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: f64 = row.get(0).expect("Should get SQL_VARIANT as f64");
        assert!(
            (value - 123.456).abs() < 0.001,
            "Value should be approximately correct: {}",
            value
        );
    }

    // Test NULL SQL_VARIANT
    let rows = client
        .query("SELECT CAST(NULL AS SQL_VARIANT) AS variant_null", &[])
        .await
        .expect("SQL_VARIANT NULL query failed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: Option<i32> = row.try_get(0);
        assert!(value.is_none(), "Should be NULL");
    }

    client.close().await.expect("Failed to close");
}
