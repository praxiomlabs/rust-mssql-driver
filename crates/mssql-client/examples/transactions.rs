//! Transaction handling with savepoints example.
//!
//! This example demonstrates transaction management including:
//! - Beginning and committing transactions
//! - Rolling back transactions
//! - Creating and using savepoints
//! - Isolation levels
//!
//! # Running
//!
//! ```bash
//! cargo run --example transactions
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config, Error, IsolationLevel, Ready};

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
    let client = Client::connect(config).await?;
    println!("Connected to SQL Server");

    // Example 1: Basic transaction with commit
    println!("\n--- Example 1: Basic Transaction ---");
    basic_transaction_example(client).await?;

    Ok(())
}

async fn basic_transaction_example(client: Client<Ready>) -> Result<(), Error> {
    // Begin a transaction
    // Note: begin_transaction() consumes `client` and returns a transaction wrapper
    let mut tx = client.begin_transaction().await?;
    println!("Transaction started");

    // Execute statements within the transaction (returns rows affected as u64)
    tx.execute("CREATE TABLE #TempUsers (id INT, name NVARCHAR(100))", &[])
        .await?;
    println!("Temporary table created");

    tx.execute(
        "INSERT INTO #TempUsers (id, name) VALUES (@p1, @p2)",
        &[&1i32, &"Alice"],
    )
    .await?;
    println!("Inserted Alice");

    tx.execute(
        "INSERT INTO #TempUsers (id, name) VALUES (@p1, @p2)",
        &[&2i32, &"Bob"],
    )
    .await?;
    println!("Inserted Bob");

    // Create a savepoint before potentially risky operation
    let savepoint = tx.save_point("before_charlie").await?;
    println!("Savepoint 'before_charlie' created");

    tx.execute(
        "INSERT INTO #TempUsers (id, name) VALUES (@p1, @p2)",
        &[&3i32, &"Charlie"],
    )
    .await?;
    println!("Inserted Charlie");

    // Decide to rollback to savepoint (simulating an error condition)
    let simulate_error = std::env::var("SIMULATE_ERROR").is_ok();

    if simulate_error {
        println!("Simulating error - rolling back to savepoint...");
        tx.rollback_to(&savepoint).await?;
        println!("Rolled back to savepoint (Charlie's insert undone)");
    }

    // Query the data
    let rows = tx
        .query("SELECT id, name FROM #TempUsers ORDER BY id", &[])
        .await?;

    println!("\nUsers in transaction:");
    for result in rows {
        let row = result?;
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        println!("  {} - {}", id, name);
    }

    // Commit the transaction
    // This consumes the transaction and returns the client in Ready state
    let client = tx.commit().await?;
    println!("\nTransaction committed");

    // The client is now back in Ready state and can be used for more operations
    client.close().await?;

    Ok(())
}

/// Example demonstrating isolation levels
#[allow(dead_code)]
async fn isolation_level_example(mut client: Client<Ready>) -> Result<(), Error> {
    // You can specify isolation level when beginning a transaction
    let isolation = IsolationLevel::ReadCommitted;
    println!("Using isolation level: {:?}", isolation);

    // Begin transaction with specific isolation level
    // Use as_sql() which returns the full SET TRANSACTION ISOLATION LEVEL statement
    // Or use name() which returns just the level name
    client.simple_query(isolation.as_sql()).await?;

    let tx = client.begin_transaction().await?;
    println!("Transaction started with {:?}", isolation);

    // ... perform operations ...

    let client = tx.commit().await?;
    client.close().await?;

    Ok(())
}
