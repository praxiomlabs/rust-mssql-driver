//! Stored Procedure Timeout Examples
//!
//! This example demonstrates how to use timeout functionality when executing
//! stored procedures to prevent long-running queries from blocking indefinitely.
//!
//! # Running the Example
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_PASSWORD=YourPassword
//! cargo run --example stored_procedure_timeout
//! ```

// Allow unwrap in examples for code clarity and brevity
#![allow(clippy::unwrap_used)]

use mssql_client::{Client, Config};
use std::time::Duration;

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

    // Setup: Create test stored procedures
    println!("\n📝 Creating test stored procedures...");

    client
        .execute(
            "IF OBJECT_ID('dbo.sp_QuickCalc', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_QuickCalc",
            &[],
        )
        .await?;

    client
        .execute(
            "IF OBJECT_ID('dbo.sp_LongRunning', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_LongRunning",
            &[],
        )
        .await?;

    // Quick procedure (completes instantly)
    client
        .execute(
            "CREATE PROCEDURE dbo.sp_QuickCalc
            @input INT,
            @doubled INT OUTPUT
        AS
        BEGIN
            SET @doubled = @input * 2;
        END",
            &[],
        )
        .await?;

    // Long-running procedure (uses WAITFOR DELAY)
    client
        .execute(
            "CREATE PROCEDURE dbo.sp_LongRunning
            @seconds INT,
            @result INT OUTPUT
        AS
        BEGIN
            WAITFOR DELAY @seconds_clause;
            SET @result = @seconds * 1000;
        END",
            &[],
        )
        .await?;

    println!("✓ Created test procedures");

    // ========================================================================
    // EXAMPLE 1: Normal execution (no timeout)
    // ========================================================================
    println!("\n🚀 Example 1: Normal execution (no timeout)");

    let result = client
        .execute_procedure("dbo.sp_QuickCalc", &[&21i32])
        .await?;

    let doubled = result.get_output("@doubled").unwrap();
    let doubled_value = doubled.value.as_i32().unwrap();
    println!("  Input: 21");
    println!("  Output: {doubled_value}");

    // ========================================================================
    // EXAMPLE 2: Execution with sufficient timeout
    // ========================================================================
    println!("\n⏱️  Example 2: Execution with sufficient timeout");

    let result = client
        .execute_procedure_with_timeout("dbo.sp_QuickCalc", &[&15i32], Duration::from_secs(5))
        .await?;

    let doubled = result.get_output("@doubled").unwrap();
    let doubled_value = doubled.value.as_i32().unwrap();
    println!("  Input: 15");
    println!("  Output: {doubled_value}");
    println!("  ✓ Completed within 5-second timeout");

    // ========================================================================
    // EXAMPLE 3: Timeout expiration
    // ========================================================================
    println!("\n⏰ Example 3: Timeout expiration");

    // This procedure tries to wait for 3 seconds but we timeout after 100ms
    let result = client
        .execute_procedure_with_timeout("dbo.sp_LongRunning", &[&3i32], Duration::from_millis(100))
        .await;

    match result {
        Ok(_) => println!("  Unexpected: Procedure completed (should have timed out)"),
        Err(e) => {
            println!("  ✓ Expected timeout occurred: {e}");
            println!("  ✓ The 100ms timeout prevented the 3-second wait");
        }
    }

    // ========================================================================
    // EXAMPLE 4: Timeout with longer duration
    // ========================================================================
    println!("\n⏱️  Example 4: Timeout with longer duration");

    // Give it enough time to complete
    let result = client
        .execute_procedure_with_timeout("dbo.sp_LongRunning", &[&1i32], Duration::from_secs(5))
        .await;

    match result {
        Ok(r) => {
            let output = r.get_output("@result").unwrap();
            let output_value = output.value.as_i32().unwrap();
            println!("  ✓ Procedure completed within 5-second timeout");
            println!("  Result: {output_value}");
        }
        Err(e) => {
            println!("  Unexpected timeout: {e}");
        }
    }

    // ========================================================================
    // EXAMPLE 5: Timeout in transaction
    // ========================================================================
    println!("\n💼 Example 5: Timeout in transaction");

    let mut tx = client.begin_transaction().await?;

    let result = tx
        .execute_procedure_with_timeout("dbo.sp_QuickCalc", &[&99i32], Duration::from_secs(5))
        .await?;

    let doubled = result.get_output("@doubled").unwrap();
    let doubled_value = doubled.value.as_i32().unwrap();
    println!("  Input: 99");
    println!("  Output: {doubled_value}");
    println!("  ✓ Transaction procedure completed");

    let client = tx.commit().await?;
    println!("  ✓ Transaction committed");

    // ========================================================================
    // EXAMPLE 6: Practical use case - preventing runaway queries
    // ========================================================================
    println!("\n🛡️  Example 6: Practical use case - preventing runaway queries");

    println!("  Scenario: Business-critical report that must complete quickly");
    println!("  Solution: Use aggressive timeout for user-facing operations");

    let mut client = client;

    let result = client
        .execute_procedure_with_timeout("dbo.sp_QuickCalc", &[&42i32], Duration::from_secs(2))
        .await;

    match result {
        Ok(r) => {
            let doubled = r.get_output("@doubled").unwrap();
            let doubled_value = doubled.value.as_i32().unwrap();
            println!("  ✓ Report generated successfully: {doubled_value}");
        }
        Err(e) => {
            println!("  ⚠️  Report generation took too long: {e}");
            println!("  ✓ User received quick error instead of hanging");
        }
    }

    // Cleanup
    println!("\n🧹 Cleaning up...");
    client
        .execute("DROP PROCEDURE dbo.sp_QuickCalc", &[])
        .await?;
    client
        .execute("DROP PROCEDURE dbo.sp_LongRunning", &[])
        .await?;
    println!("✓ Cleanup complete");

    println!("\n✅ All timeout examples completed successfully!");
    println!("\n💡 Key Takeaways:");
    println!("  • Use timeouts for user-facing operations (2-5 seconds)");
    println!("  • Use longer timeouts for batch operations (30-60 seconds)");
    println!("  • Always handle timeout errors gracefully");
    println!("  • Test timeout behavior with both short and long durations");

    Ok(())
}
