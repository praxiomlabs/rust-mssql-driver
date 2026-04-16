//! Bulk Copy Protocol (BCP) example.
//!
//! This example demonstrates high-performance bulk data loading using
//! the TDS Bulk Load protocol via `Client::bulk_insert()`.
//!
//! # Running
//!
//! ```bash
//! cargo run --example bulk_insert
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::vec_init_then_push)]

use mssql_client::{BulkColumn, BulkInsertBuilder, BulkOptions, Client, Config, Error};
use mssql_types::SqlValue;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());

    let conn_str = format!(
        "Server={host};Database={database};User Id={user};Password={password};TrustServerCertificate=true"
    );

    let config = Config::from_connection_string(&conn_str)?;
    let mut client = Client::connect(config).await?;
    println!("Connected to SQL Server");

    // Create a test table using simple_query (no row count needed)
    client
        .simple_query("IF OBJECT_ID('tempdb..#BulkTest', 'U') IS NOT NULL DROP TABLE #BulkTest")
        .await?;

    client
        .simple_query(
            "CREATE TABLE #BulkTest (
                id INT NOT NULL,
                name NVARCHAR(100) NOT NULL,
                value DECIMAL(18,2),
                created_at DATETIME2 DEFAULT GETDATE()
            )",
        )
        .await?;
    println!("Test table created");

    // Define columns for bulk insert
    let columns = vec![
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("name", "NVARCHAR(100)", 1),
        BulkColumn::new("value", "DECIMAL(18,2)", 2),
    ];

    // Configure and build the bulk insert operation
    let builder = BulkInsertBuilder::new("#BulkTest")
        .with_typed_columns(columns)
        .with_options(BulkOptions {
            batch_size: 1000,
            check_constraints: true,
            fire_triggers: false,
            keep_nulls: true,
            table_lock: false,
            order_hint: None,
        });

    // Start the bulk insert — sends INSERT BULK to the server
    let mut writer = client.bulk_insert(&builder).await?;
    println!("Starting bulk insert...");

    // Generate and buffer sample data
    let num_rows = 10_000;
    for i in 0..num_rows {
        let row = vec![
            SqlValue::Int(i),
            SqlValue::String(format!("User_{i}")),
            SqlValue::Null, // Using NULL for simplicity; real code could use Decimal
        ];
        writer.send_row_values(&row)?;
    }

    // Send all buffered rows to the server and read the response
    let result = writer.finish().await?;
    println!(
        "Bulk insert complete: {} rows affected",
        result.rows_affected
    );

    // Verify the data was inserted
    let rows = client
        .query("SELECT COUNT(*) AS cnt FROM #BulkTest", &[])
        .await?;

    for result in rows {
        let row = result?;
        let count: i32 = row.get(0)?;
        println!("Verified: {count} rows in #BulkTest");
    }

    client.close().await?;
    println!("\nDone!");

    Ok(())
}

/// Example showing bulk insert with different data types
#[allow(dead_code)]
fn create_sample_rows() -> Vec<Vec<SqlValue>> {
    let mut rows = Vec::new();

    // Integer types
    rows.push(vec![
        SqlValue::Int(1),
        SqlValue::String("Integer test".into()),
        SqlValue::Null,
    ]);

    // String types
    rows.push(vec![
        SqlValue::Int(2),
        SqlValue::String("Unicode test".into()),
        SqlValue::Null,
    ]);

    // NULL handling
    rows.push(vec![
        SqlValue::Int(3),
        SqlValue::String("NULL value test".into()),
        SqlValue::Null,
    ]);

    rows
}
