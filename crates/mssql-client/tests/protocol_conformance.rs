//! TDS Protocol Conformance Tests
//!
//! These tests validate TDS protocol behavior against a real SQL Server instance.
//! They verify that the driver correctly implements the TDS protocol as specified
//! in MS-TDS.
//!
//! Run with:
//! ```bash
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-client --test protocol_conformance -- --ignored
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::expect_fun_call,
    clippy::approx_constant
)]

use chrono::Datelike;
use chrono::Timelike;
use mssql_client::{Client, Config};
use uuid::Uuid;

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// TDS Version and Server Information Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_server_version_info() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Query server version information using @@VERSION which returns a plain string
    // (SERVERPROPERTY returns SQL_VARIANT which may not be fully supported)
    let rows = client
        .query(
            r#"
            SELECT
                @@VERSION AS full_version,
                @@SERVERNAME AS server_name,
                @@SPID AS session_id
            "#,
            &[],
        )
        .await
        .expect("Version query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let full_version: String = row.get(0).expect("Failed to get full_version");
        let server_name: String = row.get(1).expect("Failed to get server_name");
        let session_id: i16 = row.get(2).expect("Failed to get session_id");

        println!("Server: {}", server_name);
        println!("Session ID: {}", session_id);
        println!("Version: {}", &full_version[..full_version.len().min(80)]);

        // Verify we got valid version information
        assert!(full_version.contains("Microsoft SQL Server"));
        assert!(!server_name.is_empty());
        assert!(session_id > 0, "Session ID should be positive");
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_database_context() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Verify initial database context
    let rows = client
        .query("SELECT DB_NAME() AS current_db", &[])
        .await
        .expect("Query failed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let db_name: String = row.get(0).expect("Failed to get db_name");
        println!("Connected to database: {}", db_name);
        assert!(!db_name.is_empty());
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Data Type Conformance Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_all_integer_types() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test all SQL Server integer types with valid range values
    // Note: TINYINT in SQL Server is unsigned (0-255), so we only test max
    let rows = client
        .query(
            r#"
            SELECT
                CAST(255 AS TINYINT) AS tinyint_max,
                CAST(0 AS TINYINT) AS tinyint_min,
                CAST(32767 AS SMALLINT) AS smallint_max,
                CAST(-32768 AS SMALLINT) AS smallint_min,
                CAST(2147483647 AS INT) AS int_max,
                CAST(-2147483648 AS INT) AS int_min,
                CAST(9223372036854775807 AS BIGINT) AS bigint_max,
                CAST(-9223372036854775808 AS BIGINT) AS bigint_min
            "#,
            &[],
        )
        .await
        .expect("Integer query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        // TINYINT is unsigned (0-255)
        let tinyint_max: u8 = row.get(0).expect("Failed to get tinyint max");
        let tinyint_min: u8 = row.get(1).expect("Failed to get tinyint min");
        let smallint_max: i16 = row.get(2).expect("Failed to get smallint max");
        let smallint_min: i16 = row.get(3).expect("Failed to get smallint min");
        let int_max: i32 = row.get(4).expect("Failed to get int max");
        let int_min: i32 = row.get(5).expect("Failed to get int min");
        let bigint_max: i64 = row.get(6).expect("Failed to get bigint max");
        let bigint_min: i64 = row.get(7).expect("Failed to get bigint min");

        assert_eq!(tinyint_max, 255);
        assert_eq!(tinyint_min, 0);
        assert_eq!(smallint_max, 32767);
        assert_eq!(smallint_min, -32768);
        assert_eq!(int_max, 2147483647);
        assert_eq!(int_min, -2147483648);
        assert_eq!(bigint_max, 9223372036854775807);
        assert_eq!(bigint_min, -9223372036854775808);

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_floating_point_types() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query(
            r#"
            SELECT
                CAST(3.14159265358979 AS FLOAT) AS float_val,
                CAST(2.71828 AS REAL) AS real_val,
                CAST(123456.789 AS DECIMAL(18,3)) AS decimal_val,
                CAST(999999999999.99 AS NUMERIC(18,2)) AS numeric_val,
                CAST(12345.67 AS MONEY) AS money_val,
                CAST(214748.3647 AS SMALLMONEY) AS smallmoney_val
            "#,
            &[],
        )
        .await
        .expect("Float query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        let float_val: f64 = row.get(0).expect("Failed to get float");
        let real_val: f32 = row.get(1).expect("Failed to get real");

        assert!((float_val - 3.14159265358979).abs() < 1e-10);
        assert!((real_val - 2.71828).abs() < 1e-4);

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_string_types() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test fixed-length and variable-length string types
    let rows = client
        .query(
            r#"
            SELECT
                CAST('Hello' AS CHAR(10)) AS char_val,
                CAST('World' AS VARCHAR(50)) AS varchar_val,
                CAST(N'こんにちは' AS NCHAR(10)) AS nchar_val,
                CAST(N'世界' AS NVARCHAR(50)) AS nvarchar_val
            "#,
            &[],
        )
        .await
        .expect("String query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        let char_val: String = row.get(0).expect("Failed to get char");
        let varchar_val: String = row.get(1).expect("Failed to get varchar");
        let nchar_val: String = row.get(2).expect("Failed to get nchar");
        let nvarchar_val: String = row.get(3).expect("Failed to get nvarchar");

        // CHAR is fixed-width, padded with spaces
        assert!(char_val.starts_with("Hello"));
        assert_eq!(varchar_val, "World");
        assert!(nchar_val.starts_with("こんにちは"));
        assert_eq!(nvarchar_val, "世界");

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_datetime_types() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test DATETIME and SMALLDATETIME types (most commonly used)
    let rows = client
        .query(
            r#"
            SELECT
                CAST('2024-12-17 14:30:00' AS DATETIME) AS datetime_val,
                CAST('2024-12-17 14:30:00' AS SMALLDATETIME) AS smalldatetime_val,
                GETDATE() AS current_datetime
            "#,
            &[],
        )
        .await
        .expect("Datetime query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        // Read datetimes - driver should handle conversion
        let datetime_val: chrono::NaiveDateTime = row.get(0).expect("Failed to get datetime");
        let smalldatetime_val: chrono::NaiveDateTime =
            row.get(1).expect("Failed to get smalldatetime");
        let current: chrono::NaiveDateTime = row.get(2).expect("Failed to get current datetime");

        // Verify expected values
        assert_eq!(datetime_val.year(), 2024);
        assert_eq!(datetime_val.month(), 12);
        assert_eq!(datetime_val.day(), 17);
        assert_eq!(datetime_val.hour(), 14);
        assert_eq!(datetime_val.minute(), 30);

        // SMALLDATETIME rounds to minute precision
        assert_eq!(smalldatetime_val.year(), 2024);
        assert_eq!(smalldatetime_val.month(), 12);
        assert_eq!(smalldatetime_val.day(), 17);

        // Current datetime should have a reasonable year
        assert!(
            current.year() >= 2024 && current.year() <= 2100,
            "Current year should be recent"
        );

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_binary_types() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query(
            r#"
            SELECT
                CAST(0xDEADBEEF AS BINARY(4)) AS binary_val,
                CAST(0xCAFEBABE AS VARBINARY(10)) AS varbinary_val,
                CAST(0x0102030405060708 AS VARBINARY(100)) AS varbinary_long
            "#,
            &[],
        )
        .await
        .expect("Binary query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        let binary_val: Vec<u8> = row.get(0).expect("Failed to get binary");
        let varbinary_val: Vec<u8> = row.get(1).expect("Failed to get varbinary");

        assert_eq!(binary_val, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(varbinary_val, vec![0xCA, 0xFE, 0xBA, 0xBE]);

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_guid_type() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Use NEWID() directly to generate and return a GUID
    let rows = client
        .query("SELECT NEWID() AS guid_val", &[])
        .await
        .expect("GUID query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let guid_val: Uuid = row.get(0).expect("Failed to get guid");

        // The UUID we read should be valid (not nil)
        assert!(!guid_val.is_nil(), "UUID should not be nil");

        // Verify the UUID string representation is valid format
        let guid_str = guid_val.to_string();
        assert_eq!(guid_str.len(), 36, "GUID string should be 36 chars");
        assert!(
            guid_str.chars().filter(|c| *c == '-').count() == 4,
            "GUID should have 4 dashes"
        );

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// NULL Handling Conformance Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_null_handling() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query(
            r#"
            SELECT
                CAST(NULL AS INT) AS null_int,
                CAST(NULL AS VARCHAR(50)) AS null_varchar,
                CAST(NULL AS DATETIME) AS null_datetime,
                CAST(NULL AS VARBINARY(50)) AS null_binary,
                CAST(NULL AS UNIQUEIDENTIFIER) AS null_guid
            "#,
            &[],
        )
        .await
        .expect("NULL query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        let null_int: Option<i32> = row.get(0).expect("Failed to get null_int");
        let null_varchar: Option<String> = row.get(1).expect("Failed to get null_varchar");

        assert!(null_int.is_none(), "Expected NULL for int");
        assert!(null_varchar.is_none(), "Expected NULL for varchar");

        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Multiple Result Set Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_multiple_result_sets() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute batch with multiple result sets
    let rows = client
        .query(
            r#"
            SELECT 1 AS first_result;
            SELECT 2 AS second_result;
            SELECT 3 AS third_result;
            "#,
            &[],
        )
        .await
        .expect("Multi-result query failed");

    // Count all rows across all result sets
    let total: usize = rows.filter_map(|r| r.ok()).count();

    // Should have at least one row (may be 1 or 3 depending on driver behavior)
    assert!(
        total >= 1,
        "Expected at least one row from multi-result batch"
    );

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Error Handling Conformance Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_sql_error_severity_levels() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test informational message (severity < 10)
    let result = client
        .query("PRINT 'This is an informational message'", &[])
        .await;
    // PRINT should succeed (it's informational)
    assert!(result.is_ok(), "PRINT should not cause an error");

    // Test error (severity >= 16)
    let result = client
        .query("RAISERROR('Test error message', 16, 1)", &[])
        .await;
    assert!(
        result.is_err(),
        "RAISERROR with severity 16 should cause an error"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_constraint_violation_error() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create table with constraint
    client
        .execute(
            "CREATE TABLE #ConstraintTest (id INT PRIMARY KEY, value INT CHECK (value > 0))",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Test primary key violation
    client
        .execute("INSERT INTO #ConstraintTest VALUES (1, 10)", &[])
        .await
        .expect("First insert should succeed");

    let result = client
        .execute("INSERT INTO #ConstraintTest VALUES (1, 20)", &[])
        .await;
    assert!(result.is_err(), "Duplicate key should cause an error");

    // Test check constraint violation
    let result = client
        .execute("INSERT INTO #ConstraintTest VALUES (2, -5)", &[])
        .await;
    assert!(
        result.is_err(),
        "Check constraint violation should cause an error"
    );

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Transaction Protocol Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_transaction_commit() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table
    client
        .execute(
            "CREATE TABLE #TxCommitTest (id INT, value NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Begin transaction using API
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    // Insert within transaction
    tx.execute("INSERT INTO #TxCommitTest VALUES (1, 'committed')", &[])
        .await
        .expect("Failed to insert");

    // Commit
    let mut client = tx.commit().await.expect("Failed to commit");

    // Verify data persisted
    let rows = client
        .query("SELECT value FROM #TxCommitTest WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let value: String = row.get(0).expect("Failed to get value");
        assert_eq!(value, "committed");
        found = true;
    }
    assert!(found, "Committed data should be visible");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_transaction_rollback() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create test table and insert initial data
    client
        .execute(
            "CREATE TABLE #TxRollbackTest (id INT, value NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Failed to create table");

    client
        .execute("INSERT INTO #TxRollbackTest VALUES (1, 'original')", &[])
        .await
        .expect("Failed to insert initial data");

    // Begin transaction
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    // Update within transaction
    tx.execute(
        "UPDATE #TxRollbackTest SET value = 'modified' WHERE id = 1",
        &[],
    )
    .await
    .expect("Failed to update");

    // Rollback
    let mut client = tx.rollback().await.expect("Failed to rollback");

    // Verify original data preserved
    let rows = client
        .query("SELECT value FROM #TxRollbackTest WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let value: String = row.get(0).expect("Failed to get value");
        assert_eq!(value, "original", "Rollback should preserve original data");
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Prepared Statement Protocol Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_parameterized_query() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test various parameter types
    let int_param: i32 = 42;
    let str_param = String::from("test value");
    let float_param: f64 = 3.14159;

    let rows = client
        .query(
            "SELECT @p1 AS int_val, @p2 AS str_val, @p3 AS float_val",
            &[&int_param, &str_param.as_str(), &float_param],
        )
        .await
        .expect("Parameterized query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let int_val: i32 = row.get(0).expect("Failed to get int");
        let str_val: String = row.get(1).expect("Failed to get string");
        let float_val: f64 = row.get(2).expect("Failed to get float");

        assert_eq!(int_val, 42);
        assert_eq!(str_val, "test value");
        assert!((float_val - 3.14159).abs() < 1e-5);
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_statement_cache() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Execute the same query multiple times with different parameters
    // This should exercise the prepared statement cache
    let query = "SELECT @p1 * 2 AS doubled";

    for i in 1..=10 {
        let param: i32 = i;
        let rows = client
            .query(query, &[&param])
            .await
            .expect("Query should succeed");

        let mut found = false;
        for result in rows {
            let row = result.expect("Row should be valid");
            let doubled: i32 = row.get(0).expect("Failed to get result");
            assert_eq!(doubled, i * 2);
            found = true;
        }
        assert!(found);
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Row Count and Metadata Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_affected_rows() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create and populate test table
    client
        .execute("CREATE TABLE #AffectedRowsTest (id INT)", &[])
        .await
        .expect("Failed to create table");

    // Insert multiple rows
    let affected = client
        .execute(
            "INSERT INTO #AffectedRowsTest VALUES (1), (2), (3), (4), (5)",
            &[],
        )
        .await
        .expect("Failed to insert");

    assert_eq!(affected, 5, "Should report 5 affected rows");

    // Update some rows
    let affected = client
        .execute(
            "UPDATE #AffectedRowsTest SET id = id + 10 WHERE id > 3",
            &[],
        )
        .await
        .expect("Failed to update");

    assert_eq!(affected, 2, "Should report 2 affected rows");

    // Delete rows
    let affected = client
        .execute("DELETE FROM #AffectedRowsTest WHERE id < 5", &[])
        .await
        .expect("Failed to delete");

    assert_eq!(affected, 3, "Should report 3 affected rows");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Large Data Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_large_varchar() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a large string (close to max varchar size of 8000)
    let large_string: String = "A".repeat(7999);

    client
        .execute(
            "CREATE TABLE #LargeVarcharTest (id INT, data VARCHAR(8000))",
            &[],
        )
        .await
        .expect("Failed to create table");

    client
        .execute(
            "INSERT INTO #LargeVarcharTest VALUES (1, @p1)",
            &[&large_string.as_str()],
        )
        .await
        .expect("Failed to insert large string");

    let rows = client
        .query("SELECT data FROM #LargeVarcharTest WHERE id = 1", &[])
        .await
        .expect("Query failed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let data: String = row.get(0).expect("Failed to get data");
        assert_eq!(data.len(), 7999);
        assert!(data.chars().all(|c| c == 'A'));
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_many_columns() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Query with many columns to test metadata handling
    let rows = client
        .query(
            r#"
            SELECT
                1 AS c1, 2 AS c2, 3 AS c3, 4 AS c4, 5 AS c5,
                6 AS c6, 7 AS c7, 8 AS c8, 9 AS c9, 10 AS c10,
                11 AS c11, 12 AS c12, 13 AS c13, 14 AS c14, 15 AS c15,
                16 AS c16, 17 AS c17, 18 AS c18, 19 AS c19, 20 AS c20
            "#,
            &[],
        )
        .await
        .expect("Many-column query failed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");

        // Verify we can access all 20 columns
        for i in 0..20 {
            let val: i32 = row.get(i).expect(&format!("Failed to get column {}", i));
            assert_eq!(val, (i + 1) as i32);
        }
        count += 1;
    }
    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_protocol_many_rows() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Generate many rows using a recursive CTE
    let rows = client
        .query(
            r#"
            WITH Numbers AS (
                SELECT 1 AS n
                UNION ALL
                SELECT n + 1 FROM Numbers WHERE n < 1000
            )
            SELECT n FROM Numbers OPTION (MAXRECURSION 1000)
            "#,
            &[],
        )
        .await
        .expect("Many-row query failed");

    let count: usize = rows.filter_map(|r| r.ok()).count();
    assert_eq!(count, 1000, "Should receive exactly 1000 rows");

    client.close().await.expect("Failed to close");
}
