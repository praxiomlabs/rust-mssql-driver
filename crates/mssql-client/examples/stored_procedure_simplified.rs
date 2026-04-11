//! Simplified Stored Procedure API Example
//!
//! This example demonstrates the simplified API for calling stored procedures
//! where OUTPUT parameters are automatically detected and do not need to be
//! provided by the user.
//!
//! # Running the Example
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_PASSWORD=YourPassword
//! cargo run --example stored_procedure_simplified
//! ```

// Allow unwrap in examples for code clarity and brevity
#![allow(clippy::unwrap_used)]

use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load connection configuration from environment
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("MSSQL_PORT").unwrap_or_else(|_| "1433".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "TestDB".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host},{port};Database={database};User Id={user};Password={password};TrustServerCertificate=true;Encrypt={encrypt}"
    );

    let config = Config::from_connection_string(&conn_str)?;
    let mut client = Client::connect(config).await?;

    println!("✓ Connected to SQL Server");

    // Setup: Create a test stored procedure
    println!("\n📝 Creating test stored procedure...");

    client.execute(
        "IF OBJECT_ID('dbo.sp_CalculateStats', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_CalculateStats",
        &[],
    ).await?;

    client
        .execute(
            "CREATE PROCEDURE dbo.sp_CalculateStats
            @input_value INT,
            @doubled INT OUTPUT,
            @tripled INT OUTPUT,
            @squared INT OUTPUT
        AS
        BEGIN
            SET @doubled = @input_value * 2;
            SET @tripled = @input_value * 3;
            SET @squared = @input_value * @input_value;
        END",
            &[],
        )
        .await?;

    println!("✓ Created stored procedure dbo.sp_CalculateStats");

    // ========================================================================
    // SIMPLIFIED API: Only provide INPUT parameters
    // ========================================================================
    println!("\n🚀 Using simplified API (only INPUT parameters)...");

    let result = client
        .execute_procedure("dbo.sp_CalculateStats", &[&7i32])
        .await?;

    // Access OUTPUT parameters
    let doubled = result.get_output("@doubled").unwrap();
    let tripled = result.get_output("@tripled").unwrap();
    let squared = result.get_output("@squared").unwrap();

    println!("  Input: 7");
    println!("  Doubled: {}", doubled.value.as_i32().unwrap());
    println!("  Tripled: {}", tripled.value.as_i32().unwrap());
    println!("  Squared: {}", squared.value.as_i32().unwrap());

    // ========================================================================
    // TRADITIONAL API: Provide all parameters explicitly
    // ========================================================================
    println!("\n🔧 Using traditional API (all parameters explicit)...");

    let result = client
        .execute_procedure(
            "dbo.sp_CalculateStats",
            &[&7i32, &None::<i32>, &None::<i32>, &None::<i32>],
        )
        .await?;

    // Access OUTPUT parameters (same as before)
    let doubled = result.get_output("@doubled").unwrap();
    let tripled = result.get_output("@tripled").unwrap();
    let squared = result.get_output("@squared").unwrap();

    println!("  Input: 7");
    println!("  Doubled: {}", doubled.value.as_i32().unwrap());
    println!("  Tripled: {}", tripled.value.as_i32().unwrap());
    println!("  Squared: {}", squared.value.as_i32().unwrap());

    // ========================================================================
    // RETURN VALUE EXAMPLE
    // ========================================================================
    println!("\n📝 Creating procedure with RETURN value...");

    client
        .execute(
            "IF OBJECT_ID('dbo.sp_GetStatus', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_GetStatus",
            &[],
        )
        .await?;

    client
        .execute(
            "CREATE PROCEDURE dbo.sp_GetStatus
            @value INT
        AS
        BEGIN
            RETURN @value * 10;
        END",
            &[],
        )
        .await?;

    println!("✓ Created stored procedure dbo.sp_GetStatus");

    println!("\n🚀 Calling procedure with RETURN value...");

    let result = client
        .execute_procedure("dbo.sp_GetStatus", &[&5i32])
        .await?;

    // Get RETURN value (always present in output_params[0])
    if let Some(return_value) = result.get_return_value() {
        let status: i32 = return_value.value.as_i32().unwrap();
        println!("  RETURN value: {status}");
    }

    // ========================================================================
    // RESULT SET + OUTPUT PARAMETERS EXAMPLE
    // ========================================================================
    println!("\n📝 Creating procedure with result set and OUTPUT parameters...");

    client
        .execute(
            "IF OBJECT_ID('dbo.sp_SearchUsers', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_SearchUsers",
            &[],
        )
        .await?;

    client
        .execute(
            "CREATE PROCEDURE dbo.sp_SearchUsers
            @min_score INT,
            @row_count INT OUTPUT
        AS
        BEGIN
            SELECT Id = 1, Name = 'Alice', Score = 95
            UNION ALL
            SELECT Id = 2, Name = 'Bob', Score = 87
            UNION ALL
            SELECT Id = 3, Name = 'Charlie', Score = 92;

            SET @row_count = @@ROWCOUNT;
        END",
            &[],
        )
        .await?;

    println!("✓ Created stored procedure dbo.sp_SearchUsers");

    println!("\n🚀 Calling procedure with result set and OUTPUT parameters...");

    let result = client
        .execute_procedure("dbo.sp_SearchUsers", &[&90i32])
        .await?;

    // Access OUTPUT parameter
    let row_count = result.get_output("@row_count").unwrap();
    println!("  Rows returned: {}", row_count.value.as_i32().unwrap());

    // Access result set
    if let Some(mut stream) = result.result_set {
        println!("  Results:");
        for row_result in stream.by_ref() {
            let row = row_result.unwrap();
            let id: i32 = row.get(0).unwrap();
            let name: String = row.get(1).unwrap();
            let score: i32 = row.get(2).unwrap();
            println!("    - {id}: {name} (score: {score})");
        }
    }

    // Cleanup
    println!("\n🧹 Cleaning up...");
    client
        .execute("DROP PROCEDURE dbo.sp_CalculateStats", &[])
        .await?;
    client
        .execute("DROP PROCEDURE dbo.sp_GetStatus", &[])
        .await?;
    client
        .execute("DROP PROCEDURE dbo.sp_SearchUsers", &[])
        .await?;
    println!("✓ Cleanup complete");

    println!("\n✅ All examples completed successfully!");

    Ok(())
}
