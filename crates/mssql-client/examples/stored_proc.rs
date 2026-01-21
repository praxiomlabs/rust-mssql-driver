//! Example demonstrating stored procedure execution with output parameters.
//!
//! This example shows how to:
//! - Execute stored procedures with output parameters
//! - Retrieve output parameter values
//! - Handle stored procedures that return result sets
//! - Use RETURN statement values
//!
//! # Prerequisites
//!
//! Run the following SQL to set up the test stored procedures:
//!
//! ```sql
//! -- Stored procedure with output parameters only
//! CREATE PROCEDURE dbo.CalculateSum
//!     @a INT,
//!     @b INT,
//!     @result INT OUTPUT
//! AS
//! BEGIN
//!     SET @result = @a + @b;
//! END
//! GO
//!
//! -- Stored procedure with result set and output parameters
//! CREATE PROCEDURE dbo.GetUserOrders
//!     @userId INT,
//!     @totalCount INT OUTPUT
//! AS
//! BEGIN
//!     SELECT OrderId, OrderDate, Total FROM Orders WHERE UserId = @userId;
//!     SET @totalCount = @@ROWCOUNT;
//! END
//! GO
//!
//! -- Stored procedure with RETURN statement
//! CREATE PROCEDURE dbo.CheckUserExists
//!     @userId INT
//! AS
//! BEGIN
//!     IF EXISTS (SELECT 1 FROM Users WHERE Id = @userId)
//!         RETURN 1;
//!     RETURN 0;
//! END
//! GO
//! ```

use mssql_client::Client;
use tds_protocol::rpc::{RpcParam, TypeInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to SQL Server
    let config = mssql_client::Config::from_connection_string(
        "Server=localhost;Database=TestDb;User Id=sa;Password=YourPassword!;TrustServerCertificate=true",
    )?;

    let mut client = Client::connect(config).await?;
    println!("✅ Connected to SQL Server\n");

    // Example 1: Stored procedure with output parameters only
    println!("📊 Example 1: Output Parameters");
    println!("─────────────────────────────────");

    let result_param = RpcParam::null("@result", TypeInfo::int()).as_output();

    let result = client.execute_procedure(
        "dbo.CalculateSum",
        vec![
            RpcParam::int("@a", 10),
            RpcParam::int("@b", 25),
            result_param,
        ],
    ).await?;

    // Get the output parameter value
    if let Some(output) = result.get_output("result") {
        let sum: i32 = output.value.as_i32().expect("expected i32");
        println!("✅ CalculateSum(10, 25) = {}", sum);
    }

    println!("  Result set: {}", result.has_result_set());
    println!("  Affected rows: {}\n", result.rows_affected);

    // Example 2: Stored procedure with result set AND output parameters
    println!("📊 Example 2: Result Set + Output Parameters");
    println!("──────────────────────────────────────────────");

    let count_param = RpcParam::null("@totalCount", TypeInfo::int()).as_output();

    let mut result = client.execute_procedure(
        "dbo.GetUserOrders",
        vec![
            RpcParam::int("@userId", 123),
            count_param,
        ],
    ).await?;

    let Some(mut rows) = result.take_result_set() else {
        return Err("Expected result set".into());
    };

    // Process the result set
    let mut order_count = 0;
    while let Some(Ok(row)) = rows.next() {
        let order_id: i32 = row.get(0)?;
        let total: f64 = row.get(2)?;
        println!("  Order #{}: ${:.2}", order_id, total);
        order_count += 1;
    }

    // Get the output parameter (should match the row count)
    if let Some(output) = result.get_output("totalCount") {
        let total_count: i32 = output.value.as_i32().expect("expected i32");
        println!("✅ Total orders (from output parameter): {}", total_count);
        assert_eq!(order_count, total_count);
    }
    println!();

    // Example 3: Stored procedure with RETURN statement
    println!("📊 Example 3: RETURN Statement");
    println!("──────────────────────────────");

    let result = client.execute_procedure(
        "dbo.CheckUserExists",
        vec![RpcParam::int("@userId", 123)],
    ).await?;

    // RETURN value comes as an output parameter with empty name
    let return_value = result.output_params.first().expect("expected return value");
    let exists: i32 = return_value.value.as_i32().expect("expected i32");
    println!("✅ User exists (RETURN value): {}", exists == 1);
    println!();

    client.close().await?;
    println!("✅ Connection closed");

    Ok(())
}
