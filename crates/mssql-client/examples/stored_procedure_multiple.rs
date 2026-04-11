//! Stored Procedure Multiple Result Sets Example
//!
//! This example demonstrates how to execute stored procedures that return
//! multiple result sets using the `execute_procedure_multiple` method.
//!
//! # Running the Example
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_PASSWORD=YourPassword
//! cargo run --example stored_procedure_multiple
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

    // Setup: Create test stored procedures
    println!("\n📝 Creating test stored procedures...");

    client.execute(
        "IF OBJECT_ID('dbo.sp_GetUserReports', 'P') IS NOT NULL DROP PROCEDURE dbo.sp_GetUserReports",
        &[],
    ).await?;

    // Create procedure that returns multiple result sets
    client
        .execute(
            "CREATE PROCEDURE dbo.sp_GetUserReports
            @min_score INT
        AS
        BEGIN
            -- First result set: User summary
            SELECT
                Id,
                Name,
                Score,
                'Summary' AS ReportType
            FROM (SELECT 1 AS Id, 'Alice' AS Name, 95 AS Score
                  UNION ALL
                  SELECT 2 AS Id, 'Bob' AS Name, 87 AS Score
                  UNION ALL
                  SELECT 3 AS Id, 'Charlie' AS Name, 92 AS Score) AS Users
            WHERE Score >= @min_score
            ORDER BY Score DESC;

            -- Second result set: Detailed scores
            SELECT
                Id,
                Name,
                Score,
                'Details' AS ReportType
            FROM (SELECT 1 AS Id, 'Alice' AS Name, 95 AS Score
                  UNION ALL
                  SELECT 2 AS Id, 'Bob' AS Name, 87 AS Score
                  UNION ALL
                  SELECT 3 AS Id, 'Charlie' AS Name, 92 AS Score) AS UserScores
            WHERE Score >= @min_score
            ORDER BY Score DESC;
        END",
            &[],
        )
        .await?;

    println!("✓ Created test procedures");

    // ========================================================================
    // EXAMPLE 1: Basic multiple result sets
    // ========================================================================
    println!("\n🚀 Example 1: Basic multiple result sets");

    let mut result = client
        .execute_procedure_multiple("dbo.sp_GetUserReports", &[&85i32])
        .await?;

    // Access RETURN value
    if let Some(return_value) = result.get_return_value() {
        let status: i32 = return_value.value.as_i32().unwrap_or(0);
        println!("  Procedure status: {status}");
    }

    println!("  Result set 1 ({} total):", result.result_count());

    // Process first result set
    let mut count = 0;
    while let Some(row) = result.next_row().await? {
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        let score: i32 = row.get(2)?;
        let report_type: String = row.get(3)?;

        println!("    - {id}: {name} - {score} ({report_type})");
        count += 1;
        if count >= 2 {
            println!("    ... (showing first 2 rows)");
            break;
        }
    }

    // Move to second result set
    if result.next_result().await? {
        println!("  Result set 2:");

        let mut count = 0;
        while let Some(row) = result.next_row().await? {
            let id: i32 = row.get(0)?;
            let name: String = row.get(1)?;
            let score: i32 = row.get(2)?;
            let report_type: String = row.get(3)?;

            println!("    - {id}: {name} - {score} ({report_type})");
            count += 1;
            if count >= 2 {
                println!("    ... (showing first 2 rows)");
                break;
            }
        }
    }

    println!("  Total result sets: {}", result.result_count());

    // ========================================================================
    // EXAMPLE 2: Transaction with multiple result sets
    // ========================================================================
    println!("\n💼 Example 2: Transaction with multiple result sets");

    let mut tx = client.begin_transaction().await?;

    let mut result = tx
        .execute_procedure_multiple("dbo.sp_GetUserReports", &[&90i32])
        .await?;

    println!("  Transaction: Processing multiple result sets");

    // Process first result set in transaction
    let mut count = 0;
    while let Some(row) = result.next_row().await? {
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        println!("    - {id}: {name} (score >= 90)");
        count += 1;
        if count >= 2 {
            println!("    ... (showing first 2 rows)");
            break;
        }
    }

    // Check if there are more results
    if result.has_more_results() {
        println!("  ✓ Additional result sets available");
    }

    let mut client = tx.commit().await?;
    println!("  ✓ Transaction committed");

    // ========================================================================
    // EXAMPLE 3: Error handling with multiple result sets
    // ========================================================================
    println!("\n🛡️  Example 3: Error handling with multiple result sets");

    match client
        .execute_procedure("dbo.sp_NonExistent", &[&1i32])
        .await
    {
        Ok(_) => {
            println!("  Unexpected: Procedure executed");
        }
        Err(e) => {
            println!("  ✓ Expected error caught: {e}");
            println!("  ✓ Error handling works correctly");
        }
    }

    // ========================================================================
    // EXAMPLE 4: Checking result set availability
    // ========================================================================
    println!("\n📊 Example 4: Checking result set availability");

    let result = client
        .execute_procedure_multiple("dbo.sp_GetUserReports", &[&80i32])
        .await?;

    println!("  Has result sets: {}", result.has_result_set());
    println!(
        "  Current result set index: {}",
        result.current_result_index()
    );
    println!("  Total result sets: {}", result.result_count());
    println!("  Has more results: {}", result.has_more_results());

    if let Some(columns) = result.columns() {
        println!("  Current result set has {} columns", columns.len());
        for (i, col) in columns.iter().enumerate() {
            if i < 3 {
                println!("    - Column {}: {}", i + 1, col.name);
            }
        }
        if columns.len() > 3 {
            println!("    ... (showing first 3 columns)");
        }
    }

    // Cleanup
    println!("\n🧹 Cleaning up...");
    client
        .execute("DROP PROCEDURE dbo.sp_GetUserReports", &[])
        .await?;
    println!("✓ Cleanup complete");

    println!("\n✅ All multiple result set examples completed successfully!");
    println!("\n💡 Key Takeaways:");
    println!("  • Use `execute_procedure_multiple` for procedures with multiple SELECTs");
    println!("  • Navigate between result sets using `next_result()`");
    println!("  • Process rows within a result set using `next_row()`");
    println!("  • Access OUTPUT parameters alongside result sets");
    println!("  • Works seamlessly with transactions");
    println!("  • Full error handling and type safety support");

    Ok(())
}
