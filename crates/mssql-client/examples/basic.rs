//! Basic connection and query example.
//!
//! This example demonstrates how to connect to SQL Server and execute
//! simple queries with parameters.
//!
//! # Running
//!
//! ```bash
//! # Set connection details via environment variables
//! export MSSQL_HOST=localhost
//! export MSSQL_DATABASE=testdb
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourStrong@Passw0rd
//!
//! cargo run --example basic
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Build configuration using connection string
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());
    // Set MSSQL_ENCRYPT=false for development servers without TLS configured
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "true".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, database, user, password, encrypt
    );

    let config = Config::from_connection_string(&conn_str)?;

    println!("Connecting to SQL Server at {}...", host);

    // Connect to the database
    let mut client = Client::connect(config).await?;

    println!("Connected successfully!");

    // Execute a simple query
    let rows = client.query("SELECT @@VERSION AS version", &[]).await?;

    // Process the results - QueryStream yields Result<Row, Error>
    for result in rows {
        let row = result?;
        let version: String = row.get(0)?;
        println!("SQL Server Version: {}", version);
    }

    // Execute a statement (returns row count directly as u64)
    println!("\nExecuting parameterized statement...");

    let user_id = 1i32;
    let rows_affected = client
        .execute(
            "SELECT @p1 AS input_value, GETDATE() AS query_time",
            &[&user_id],
        )
        .await?;

    println!("Rows affected: {}", rows_affected);

    // Query with multiple parameters
    let name = "test";
    let count = 42i32;

    let rows = client
        .query("SELECT @p1 AS name, @p2 AS count", &[&name, &count])
        .await?;

    for result in rows {
        let row = result?;
        let n: String = row.get(0)?;
        let c: i32 = row.get(1)?;
        println!("Name: {}, Count: {}", n, c);
    }

    // Close the connection gracefully
    client.close().await?;

    println!("\nConnection closed.");

    Ok(())
}
