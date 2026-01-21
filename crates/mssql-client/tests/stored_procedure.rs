//! Stored procedure execution tests with output parameters.
//!
//! These tests require a running SQL Server instance. They are ignored by default
//! and can be run with:
//!
//! ```bash
//! # Run stored procedure tests (uses hardcoded connection: localhost/ABC/sa/1354)
//! cargo test -p mssql-client --test stored_procedure -- --ignored
//! ```
//!
//! Connection details (hardcoded for testing):
//! - Server: localhost,1433
//! - Database: ABC
//! - User: sa
//! - Password: 1354
//! - TrustServerCertificate: true
//! - Encrypt: false
//!
//! **First-time setup:** The tests will automatically create all required stored procedures.
//! No manual database setup is required - just ensure the database exists.
//!
//! **Compatibility:** Works with SQL Server 2008 and later versions.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::manual_flatten
)]

use mssql_client::{Client, Config, Ready};
use tds_protocol::rpc::{RpcParam, TypeInfo};

/// Helper to get test configuration.
/// Uses hardcoded connection details for testing: localhost/ABC/sa/1354
fn get_test_config() -> Config {
    let conn_str = "Server=localhost,1433;Database=ABC;User Id=sa;Password=1354;TrustServerCertificate=true;Encrypt=false";

    Config::from_connection_string(conn_str).unwrap()
}

/// Helper to create test stored procedures.
async fn setup_test_procedures(client: &mut Client<Ready>) {
    // Drop existing procedures if they exist (compatible with SQL Server 2008+)
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_SimpleOutput', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_SimpleOutput",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestOutputParams', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestOutputParams",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestResultSetAndOutputs', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestResultSetAndOutputs",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestReturnStatement', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestReturnStatement",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestMultipleOutputs', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestMultipleOutputs",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestTransaction', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestTransaction",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestNullOutput', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestNullOutput",
        &[]
    ).await;
    let _ = client.execute(
        "IF OBJECT_ID('dbo.sp_TestStringOutput', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_TestStringOutput",
        &[]
    ).await;

    // Now create all stored procedures
    // Note: CREATE PROCEDURE must be the first statement in a batch

    // Simple test procedure - just returns constant value
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_SimpleOutput
            @result INT OUTPUT
        AS
        BEGIN
            SET @result = 42;
        END",
            &[],
        )
        .await;

    // Create test procedure: output parameters only
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestOutputParams
            @a INT,
            @b INT,
            @sum INT OUTPUT,
            @product INT OUTPUT
        AS
        BEGIN
            SET @sum = @a + @b;
            SET @product = @a * @b;
        END",
            &[],
        )
        .await;

    // Create test procedure: result set + output parameters
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestResultSetAndOutputs
            @min_id INT,
            @row_count INT OUTPUT,
            @max_id INT OUTPUT
        AS
        BEGIN
            SELECT Id = 1, Name = 'Alice', Score = 95
            UNION ALL
            SELECT Id = 2, Name = 'Bob', Score = 87
            UNION ALL
            SELECT Id = 3, Name = 'Charlie', Score = 92;

            SET @row_count = @@ROWCOUNT;
            SET @max_id = 3;
        END",
            &[],
        )
        .await;

    // Create test procedure: RETURN statement
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestReturnStatement
            @value INT
        AS
        BEGIN
            RETURN @value;
        END",
            &[],
        )
        .await;

    // Create test procedure: multiple output parameters
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestMultipleOutputs
            @input INT,
            @doubled INT OUTPUT,
            @tripled INT OUTPUT,
            @squared INT OUTPUT
        AS
        BEGIN
            SET @doubled = @input * 2;
            SET @tripled = @input * 3;
            SET @squared = @input * @input;
        END",
            &[],
        )
        .await;

    // Create test procedure for transaction testing
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestTransaction
            @user_id INT,
            @new_balance INT OUTPUT
        AS
        BEGIN
            -- Simulate updating a balance
            SET @new_balance = 1000;
        END",
            &[],
        )
        .await;

    // Create test procedure with NULL output
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestNullOutput
            @should_be_null BIT = 0,
            @result INT OUTPUT
        AS
        BEGIN
            IF @should_be_null = 1
                SET @result = NULL;
            ELSE
                SET @result = 42;
        END",
            &[],
        )
        .await;

    // Create test procedure with string output
    let _ = client
        .execute(
            "CREATE PROCEDURE dbo.sp_TestStringOutput
            @name NVARCHAR(100),
            @result NVARCHAR(200) OUTPUT
        AS
        BEGIN
            SET @result = 'Hello, ' + @name + '!';
        END",
            &[],
        )
        .await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_output_params() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Verify stored procedure exists
    let check_proc = client.execute(
        "SELECT name, type_desc FROM sys.objects WHERE type = 'P' AND name = 'sp_TestOutputParams'",
        &[]
    ).await.expect("Failed to query");
    println!("Found {} objects", check_proc);

    // Try calling stored procedure with direct SQL
    let mut sql_result = client
        .query(
            "EXEC dbo.sp_TestOutputParams @a=10, @b=5, @sum=NULL, @product=NULL",
            &[],
        )
        .await
        .expect("Failed to execute with SQL");
    println!("Direct SQL execution:");
    while let Some(Ok(_row)) = sql_result.next() {
        println!("  Row returned (should be none for output params only)");
    }
    println!("End Direct SQL execution\n");

    // Test simple output parameter
    println!("Testing sp_SimpleOutput:");
    let simple_result = client
        .execute_procedure(
            "dbo.sp_SimpleOutput",
            vec![RpcParam::null("@result", TypeInfo::int()).as_output()],
        )
        .await
        .expect("Failed to execute sp_SimpleOutput");
    println!(
        "  Received {} output parameters",
        simple_result.output_params.len()
    );
    for (i, output) in simple_result.output_params.iter().enumerate() {
        println!("    [{}] value={:?}", i, output.value);
    }

    // Test basic output parameters
    let sum_param = RpcParam::null("@sum", TypeInfo::int()).as_output();
    let product_param = RpcParam::null("@product", TypeInfo::int()).as_output();

    let result = client
        .execute_procedure(
            "dbo.sp_TestOutputParams",
            vec![
                RpcParam::int("@a", 10),
                RpcParam::int("@b", 5),
                sum_param,
                product_param,
            ],
        )
        .await
        .expect("Failed to execute procedure");

    // Debug: print all outputs
    println!("Received {} output parameters:", result.output_params.len());
    for (i, output) in result.output_params.iter().enumerate() {
        println!("  [{}] name='{}', value={:?}", i, output.name, output.value);
    }

    // Verify no result set or affected rows
    assert!(!result.has_result_set(), "Should not have result set");
    assert_eq!(result.rows_affected, 0, "Should not have affected rows");

    // Verify output parameters
    assert_eq!(
        result.output_params.len(),
        2,
        "Should have 2 output parameters"
    );

    // Note: SQL Server may return empty parameter names, so we use index-based access
    // The outputs are returned in the same order as declared in the stored procedure
    let sum_value: i32 = result.output_params[0]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(sum_value, 15, "10 + 5 = 15");

    let product_value: i32 = result.output_params[1]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(product_value, 50, "10 * 5 = 50");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_result_set_and_outputs() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Test result set + output parameters
    let row_count_param = RpcParam::null("@row_count", TypeInfo::int()).as_output();
    let max_id_param = RpcParam::null("@max_id", TypeInfo::int()).as_output();

    let mut result = client
        .execute_procedure(
            "dbo.sp_TestResultSetAndOutputs",
            vec![RpcParam::int("@min_id", 1), row_count_param, max_id_param],
        )
        .await
        .expect("Failed to execute procedure");

    // Verify we have a result set
    assert!(result.has_result_set(), "Should have result set");

    // Verify no affected rows for SELECT
    assert_eq!(
        result.rows_affected, 0,
        "SELECT should have 0 affected rows"
    );

    // Process result set
    let mut count = 0;
    let mut max_id_in_result = 0;

    if let Some(mut rows) = result.take_result_set() {
        for row_result in rows.by_ref() {
            let row = row_result.expect("Row should be valid");
            let id: i32 = row.get(0).expect("Should get Id");
            let name: String = row.get(1).expect("Should get Name");
            let score: i32 = row.get(2).expect("Should get Score");

            max_id_in_result = max_id_in_result.max(id);
            count += 1;

            println!("Row {}: id={}, name={}, score={}", count, id, name, score);
        }
    }

    assert_eq!(count, 3, "Should have 3 rows");
    assert_eq!(max_id_in_result, 3, "Max ID should be 3");

    // Verify output parameters
    assert_eq!(
        result.output_params.len(),
        2,
        "Should have 2 output parameters"
    );

    let row_count_output = result
        .output_params
        .iter()
        .find(|p| p.name == "row_count")
        .expect("Should find row_count output");
    let row_count_value: i32 = row_count_output.value.as_i32().expect("Should be i32");
    assert_eq!(row_count_value, 3, "Should have 3 rows");

    let max_id_output = result
        .output_params
        .iter()
        .find(|p| p.name == "max_id")
        .expect("Should find max_id output");
    let max_id_value: i32 = max_id_output.value.as_i32().expect("Should be i32");
    assert_eq!(max_id_value, 3, "Max ID should be 3");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_return_statement() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Test RETURN statement
    let result = client
        .execute_procedure(
            "dbo.sp_TestReturnStatement",
            vec![RpcParam::int("@value", 42)],
        )
        .await
        .expect("Failed to execute procedure");

    // Verify no result set or affected rows
    assert!(!result.has_result_set(), "Should not have result set");
    assert_eq!(result.rows_affected, 0, "Should not have affected rows");

    // Verify RETURN value (comes as output with empty name)
    assert_eq!(
        result.output_params.len(),
        1,
        "Should have 1 output parameter (RETURN value)"
    );

    let return_output = &result.output_params[0];
    assert!(
        return_output.name.is_empty() || return_output.name == "@RETURN_VALUE",
        "RETURN value should have empty name or @RETURN_VALUE"
    );

    let return_value: i32 = return_output.value.as_i32().expect("Should be i32");
    assert_eq!(return_value, 42, "RETURN value should be 42");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_multiple_outputs() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Test multiple output parameters
    let doubled_param = RpcParam::null("@doubled", TypeInfo::int()).as_output();
    let tripled_param = RpcParam::null("@tripled", TypeInfo::int()).as_output();
    let squared_param = RpcParam::null("@squared", TypeInfo::int()).as_output();

    let result = client
        .execute_procedure(
            "dbo.sp_TestMultipleOutputs",
            vec![
                RpcParam::int("@input", 7),
                doubled_param,
                tripled_param,
                squared_param,
            ],
        )
        .await
        .expect("Failed to execute procedure");

    assert_eq!(
        result.output_params.len(),
        3,
        "Should have 3 output parameters"
    );

    let doubled: i32 = result.output_params[0]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(doubled, 14, "7 * 2 = 14");

    let tripled: i32 = result.output_params[1]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(tripled, 21, "7 * 3 = 21");

    let squared: i32 = result.output_params[2]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(squared, 49, "7 * 7 = 49");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_in_transaction() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Test stored procedure execution within transaction
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Failed to begin transaction");

    let balance_param = RpcParam::null("@new_balance", TypeInfo::int()).as_output();

    let result = tx
        .execute_procedure(
            "dbo.sp_TestTransaction",
            vec![RpcParam::int("@user_id", 123), balance_param],
        )
        .await
        .expect("Failed to execute procedure in transaction");

    assert_eq!(
        result.output_params.len(),
        1,
        "Should have 1 output parameter"
    );

    let balance: i32 = result.output_params[0]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(balance, 1000, "New balance should be 1000");

    // Commit transaction
    tx.commit().await.expect("Failed to commit transaction");
    // Note: client is consumed by begin_transaction, so we can't close it here
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_null_output_param() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Test NULL output parameter (pass 1 to indicate result should be null)
    let output_param = RpcParam::null("@result", TypeInfo::int()).as_output();

    let result = client
        .execute_procedure(
            "dbo.sp_TestNullOutput",
            vec![RpcParam::int("@should_be_null", 1), output_param],
        )
        .await
        .expect("Failed to execute procedure");

    assert_eq!(
        result.output_params.len(),
        1,
        "Should have 1 output parameter"
    );
    assert!(
        result.output_params[0].value.is_null(),
        "Output value should be NULL when @should_be_null=1"
    );

    // Test non-NULL output (pass 0 to indicate result should not be null)
    let output_param2 = RpcParam::null("@result", TypeInfo::int()).as_output();

    let result2 = client
        .execute_procedure(
            "dbo.sp_TestNullOutput",
            vec![RpcParam::int("@should_be_null", 0), output_param2],
        )
        .await
        .expect("Failed to execute procedure");

    assert_eq!(
        result2.output_params.len(),
        1,
        "Should have 1 output parameter"
    );
    let value: i32 = result2.output_params[0]
        .value
        .as_i32()
        .expect("Should be i32");
    assert_eq!(
        value, 42,
        "Output value should be 42 when @should_be_null=0"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_stored_procedure_string_output_param() {
    let config = get_test_config();
    let mut client = Client::connect(config).await.expect("Failed to connect");

    setup_test_procedures(&mut client).await;

    // Test string output parameter
    let output_param = RpcParam::null("@result", TypeInfo::nvarchar(200)).as_output();

    let result = client
        .execute_procedure(
            "dbo.sp_TestStringOutput",
            vec![RpcParam::nvarchar("@name", "World"), output_param],
        )
        .await
        .expect("Failed to execute procedure");

    assert_eq!(
        result.output_params.len(),
        1,
        "Should have 1 output parameter"
    );

    let greeting: &str = result.output_params[0]
        .value
        .as_str()
        .expect("Should be string");
    assert_eq!(greeting, "Hello, World!", "Greeting should match");

    client.close().await.expect("Failed to close");
}
