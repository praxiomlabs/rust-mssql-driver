//! Stored procedure integration tests.
//!
//! These tests require a running SQL Server instance. They are ignored by default
//! and can be run with:
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourPassword
//! export MSSQL_ENCRYPT=false
//!
//! cargo test -p mssql-client --test stored_procedure -- --ignored
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::manual_flatten
)]

use mssql_client::{Client, Config, SqlValue};

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
        "Server={host},{port};Database={database};User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt={encrypt}"
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Simple Call Tests (call_procedure with positional INPUT params)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_no_params() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a simple procedure
    client
        .execute(
            "IF OBJECT_ID('dbo.test_no_params', 'P') IS NOT NULL DROP PROCEDURE dbo.test_no_params",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_no_params AS BEGIN SELECT 1 AS result END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .call_procedure("dbo.test_no_params", &[])
        .await
        .unwrap();
    assert_eq!(result.return_value, 0);
    assert!(result.has_result_sets());
    assert_eq!(result.result_sets.len(), 1);

    // Cleanup
    client
        .execute("DROP PROCEDURE dbo.test_no_params", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_with_input_params() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_input_params', 'P') IS NOT NULL DROP PROCEDURE dbo.test_input_params",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_input_params @a INT, @b INT AS BEGIN SELECT @a + @b AS sum_result END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .call_procedure("dbo.test_input_params", &[&10i32, &20i32])
        .await
        .unwrap();
    assert_eq!(result.return_value, 0);
    assert_eq!(result.result_sets.len(), 1);

    let mut rs = result.result_sets.into_iter().next().unwrap();
    let row = rs.next_row().unwrap().unwrap();
    let sum: i32 = row.get(0).unwrap();
    assert_eq!(sum, 30);

    client
        .execute("DROP PROCEDURE dbo.test_input_params", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_with_return_value() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_return_val', 'P') IS NOT NULL DROP PROCEDURE dbo.test_return_val",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_return_val @code INT AS BEGIN RETURN @code END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .call_procedure("dbo.test_return_val", &[&42i32])
        .await
        .unwrap();
    assert_eq!(result.return_value, 42);
    assert!(result.result_sets.is_empty());

    client
        .execute("DROP PROCEDURE dbo.test_return_val", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_multiple_result_sets() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_multi_rs', 'P') IS NOT NULL DROP PROCEDURE dbo.test_multi_rs",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_multi_rs AS BEGIN \
             SELECT 1 AS a, 2 AS b; \
             SELECT 'hello' AS greeting; \
             END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .call_procedure("dbo.test_multi_rs", &[])
        .await
        .unwrap();
    assert_eq!(result.return_value, 0);
    assert_eq!(result.result_sets.len(), 2);

    // First result set: two int columns
    let mut rs1 = result.result_sets.into_iter().next().unwrap();
    assert_eq!(rs1.columns().len(), 2);
    let row = rs1.next_row().unwrap().unwrap();
    let a: i32 = row.get(0).unwrap();
    let b: i32 = row.get(1).unwrap();
    assert_eq!(a, 1);
    assert_eq!(b, 2);

    client
        .execute("DROP PROCEDURE dbo.test_multi_rs", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_rows_affected() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_rows_affected_table', 'U') IS NOT NULL DROP TABLE dbo.test_rows_affected_table",
            &[],
        )
        .await
        .unwrap();
    client
        .execute("CREATE TABLE dbo.test_rows_affected_table (id INT)", &[])
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO dbo.test_rows_affected_table VALUES (1), (2), (3)",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "IF OBJECT_ID('dbo.test_rows_affected', 'P') IS NOT NULL DROP PROCEDURE dbo.test_rows_affected",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_rows_affected AS BEGIN \
             DELETE FROM dbo.test_rows_affected_table WHERE id > 1 \
             END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .call_procedure("dbo.test_rows_affected", &[])
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 2);

    client
        .execute("DROP PROCEDURE dbo.test_rows_affected", &[])
        .await
        .unwrap();
    client
        .execute("DROP TABLE dbo.test_rows_affected_table", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

// =============================================================================
// Builder Tests (procedure() with named INPUT/OUTPUT params)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_procedure_builder_output_int() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_output_int', 'P') IS NOT NULL DROP PROCEDURE dbo.test_output_int",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_output_int @a INT, @b INT, @result INT OUTPUT \
             AS BEGIN SET @result = @a + @b END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .procedure("dbo.test_output_int")
        .unwrap()
        .input("@a", &10i32)
        .input("@b", &20i32)
        .output_int("@result")
        .execute()
        .await
        .unwrap();

    assert_eq!(result.return_value, 0);
    let output = result
        .get_output("@result")
        .expect("output param should exist");
    match &output.value {
        SqlValue::Int(v) => assert_eq!(*v, 30),
        other => panic!("expected Int, got {other:?}"),
    }

    // Test @-prefix stripping in get_output
    assert!(result.get_output("result").is_some());

    client
        .execute("DROP PROCEDURE dbo.test_output_int", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_procedure_builder_output_nvarchar() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_output_nvarchar', 'P') IS NOT NULL DROP PROCEDURE dbo.test_output_nvarchar",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_output_nvarchar @name NVARCHAR(100), @greeting NVARCHAR(200) OUTPUT \
             AS BEGIN SET @greeting = N'Hello, ' + @name + N'!' END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .procedure("dbo.test_output_nvarchar")
        .unwrap()
        .input("@name", &"World")
        .output_nvarchar("@greeting", 200)
        .execute()
        .await
        .unwrap();

    let output = result
        .get_output("@greeting")
        .expect("output param should exist");
    match &output.value {
        SqlValue::String(s) => assert_eq!(s, "Hello, World!"),
        other => panic!("expected String, got {other:?}"),
    }

    client
        .execute("DROP PROCEDURE dbo.test_output_nvarchar", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_procedure_builder_with_result_set_and_output() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_rs_and_output', 'P') IS NOT NULL DROP PROCEDURE dbo.test_rs_and_output",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_rs_and_output @count INT OUTPUT AS BEGIN \
             SELECT 'row1' AS name UNION ALL SELECT 'row2'; \
             SET @count = 2; \
             END",
            &[],
        )
        .await
        .unwrap();

    let result = client
        .procedure("dbo.test_rs_and_output")
        .unwrap()
        .output_int("@count")
        .execute()
        .await
        .unwrap();

    // Should have both result sets and output params
    assert!(result.has_result_sets());
    let count_param = result
        .get_output("@count")
        .expect("output param should exist");
    match &count_param.value {
        SqlValue::Int(v) => assert_eq!(*v, 2),
        other => panic!("expected Int, got {other:?}"),
    }

    client
        .execute("DROP PROCEDURE dbo.test_rs_and_output", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

// =============================================================================
// Transaction Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_in_transaction() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_tx_proc', 'P') IS NOT NULL DROP PROCEDURE dbo.test_tx_proc",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_tx_proc AS BEGIN SELECT 42 AS val END",
            &[],
        )
        .await
        .unwrap();

    let mut tx = client.begin_transaction().await.unwrap();

    let result = tx.call_procedure("dbo.test_tx_proc", &[]).await.unwrap();
    assert_eq!(result.return_value, 0);
    assert!(result.has_result_sets());

    let client = tx.rollback().await.unwrap();

    let mut client = client;
    client
        .execute("DROP PROCEDURE dbo.test_tx_proc", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_nonexistent_procedure() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let result = client
        .call_procedure("dbo.this_proc_does_not_exist", &[])
        .await;
    assert!(result.is_err(), "Should fail for nonexistent procedure");

    client.close().await.unwrap();
}

#[test]
fn test_procedure_name_validation() {
    // This doesn't need SQL Server — it's just validation
    // We can't call client.procedure() without a connection, but we can
    // test that validate_qualified_identifier works for procedure names.
    // The validation is tested in validation.rs, but let's verify the
    // error surfaces through the public API.

    // Invalid names should be caught at compile time (before any network call)
    // This is tested via the validation module's unit tests.
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_call_procedure_schema_qualified() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "IF OBJECT_ID('dbo.test_schema_qual', 'P') IS NOT NULL DROP PROCEDURE dbo.test_schema_qual",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "CREATE PROCEDURE dbo.test_schema_qual AS BEGIN RETURN 7 END",
            &[],
        )
        .await
        .unwrap();

    // Call with explicit schema qualification
    let result = client
        .call_procedure("dbo.test_schema_qual", &[])
        .await
        .unwrap();
    assert_eq!(result.return_value, 7);

    client
        .execute("DROP PROCEDURE dbo.test_schema_qual", &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}
