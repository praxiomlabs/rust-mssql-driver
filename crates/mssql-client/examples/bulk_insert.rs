//! Bulk Copy Protocol (BCP) example.
//!
//! This example demonstrates high-performance bulk data loading using
//! the TDS Bulk Load protocol.
//!
//! # Running
//!
//! ```bash
//! cargo run --example bulk_insert
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::vec_init_then_push)]

use mssql_client::{BulkColumn, BulkInsert, BulkInsertBuilder, BulkOptions, Client, Config, Error};
use mssql_types::SqlValue;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true",
        host, database, user, password
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

    // Configure bulk insert options
    let options = BulkOptions {
        batch_size: 1000, // Commit every 1000 rows
        check_constraints: true,
        fire_triggers: false,
        keep_nulls: true,
        table_lock: false, // Use row-level locking
        order_hint: None,
        max_errors: 0, // Abort on first error
    };

    // Define columns for bulk insert
    // Note: Precision/scale for DECIMAL comes from the SQL type string
    let columns = vec![
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("name", "NVARCHAR(100)", 1),
        BulkColumn::new("value", "DECIMAL(18,2)", 2),
    ];

    // Create the bulk insert builder to generate the INSERT BULK statement
    let builder = BulkInsertBuilder::new("#BulkTest")
        .with_options(options.clone())
        .with_typed_columns(columns.clone());

    let insert_bulk_sql = builder.build_insert_bulk_statement();
    println!("INSERT BULK statement: {}", insert_bulk_sql);

    println!("Starting bulk insert...");

    // Create the bulk insert operation with columns and batch size
    let mut bulk = BulkInsert::new(columns, options.batch_size);

    // Generate and send sample data
    let num_rows = 10_000;
    for i in 0..num_rows {
        let row = vec![
            SqlValue::Int(i),
            SqlValue::String(format!("User_{}", i)),
            SqlValue::Null, // Using NULL for simplicity; real code could use Decimal
        ];

        bulk.send_row_values(&row)?;

        // Check if we should flush the batch
        if bulk.should_flush() {
            let packets = bulk.take_packets();
            // In a real implementation, these packets would be sent to the server
            println!(
                "  Flushed batch at {} rows, {} packets generated",
                bulk.total_rows(),
                packets.len()
            );
        }
    }

    // Finish the bulk operation
    let final_packets = bulk.finish_packets();
    println!(
        "  Final flush: {} total rows, {} packets",
        bulk.total_rows(),
        final_packets.len()
    );

    let result = bulk.result();
    println!(
        "\nBulk insert packet generation complete: {} rows prepared, {} batches",
        result.rows_affected, result.batches_committed
    );

    // Note: In a full implementation, the packets would be sent to the server
    // and then we'd verify the data. This example demonstrates packet generation.

    // Query to show that we're still connected
    let rows = client
        .query("SELECT 'Bulk insert packets generated' AS status", &[])
        .await?;

    for result in rows {
        let row = result?;
        let status: String = row.get(0)?;
        println!("Status: {}", status);
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
