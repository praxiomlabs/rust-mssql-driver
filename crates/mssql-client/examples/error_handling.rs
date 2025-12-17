//! Error handling and recovery patterns example.
//!
//! This example demonstrates how to handle various error types from the driver
//! and implement retry logic for transient errors.
//!
//! # Running
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourStrong@Passw0rd
//!
//! cargo run --example error_handling
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::never_loop)]

use mssql_client::{Client, Config, Error, Ready};
use std::time::Duration;

/// Execute a query with automatic retry for transient errors.
async fn query_with_retry<T, F>(
    client: &mut Client<Ready>,
    sql: &str,
    params: &[&(dyn mssql_client::ToSql + Sync)],
    max_retries: u32,
    process: F,
) -> Result<T, Error>
where
    F: Fn(&mssql_client::Row) -> Result<T, Error>,
{
    let mut attempts = 0;
    let mut last_error = None;

    while attempts <= max_retries {
        if attempts > 0 {
            // Exponential backoff with jitter
            let base_delay = Duration::from_millis(100 * 2u64.pow(attempts - 1));
            let jitter = Duration::from_millis(rand_jitter(50));
            let delay = base_delay + jitter;
            println!(
                "  Retry attempt {}/{} after {:?}",
                attempts, max_retries, delay
            );
            tokio::time::sleep(delay).await;
        }

        match client.query(sql, params).await {
            Ok(rows) => {
                // Try to process the first row
                for result in rows {
                    match result {
                        Ok(row) => return process(&row),
                        Err(e) => return Err(e),
                    }
                }
                // No rows returned - this may be expected
                return Err(Error::Query("No rows returned".into()));
            }
            Err(e) if e.is_transient() => {
                println!("  Transient error: {:?}", e);
                last_error = Some(e);
                attempts += 1;
            }
            Err(e) => {
                // Non-transient error - fail immediately
                return Err(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| Error::Query("Max retries exceeded".into())))
}

/// Simple pseudo-random jitter (not cryptographically secure).
fn rand_jitter(max_ms: u64) -> u64 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_nanos() as u64 % max_ms
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());

    let conn_str = format!(
        "Server={};Database=master;User Id={};Password={};TrustServerCertificate=true",
        host, user, password
    );

    let config = Config::from_connection_string(&conn_str)?;

    println!("=== Error Handling Examples ===\n");

    // Example 1: Basic error handling
    println!("1. Basic error handling:");
    let mut client = Client::connect(config.clone()).await?;

    match client.query("SELECT 1/0", &[]).await {
        Ok(rows) => {
            for result in rows {
                match result {
                    Ok(row) => println!("  Result: {:?}", row),
                    Err(e) => println!("  Row error: {:?}", e),
                }
            }
        }
        Err(Error::Server {
            number,
            message,
            class,
            ..
        }) => {
            println!(
                "  SQL Server Error #{}: {} (severity: {})",
                number, message, class
            );
        }
        Err(e) => println!("  Other error: {:?}", e),
    }

    // Example 2: Handling authentication errors
    println!("\n2. Authentication error (expected to fail):");
    let bad_conn_str = format!(
        "Server={};Database=master;User Id=invalid;Password=wrong;TrustServerCertificate=true;Connect Timeout=5",
        host
    );
    let bad_config = Config::from_connection_string(&bad_conn_str)?;

    match Client::connect(bad_config).await {
        Ok(_) => println!("  Unexpectedly connected!"),
        Err(Error::Authentication(auth_err)) => {
            println!("  Authentication failed: {:?}", auth_err);
            println!("  This is expected - do not retry auth failures");
        }
        Err(e) => println!("  Other error: {:?}", e),
    }

    // Example 3: Query with retry for transient errors
    println!("\n3. Query with automatic retry:");
    let version: String = query_with_retry(&mut client, "SELECT @@VERSION", &[], 3, |row| {
        row.get(0).map_err(Error::Type)
    })
    .await?;
    println!("  Server version: {}...", &version[..50.min(version.len())]);

    // Example 4: Handling constraint violations (non-transient)
    println!("\n4. Constraint violation (non-retriable):");

    // Create a temp table with a constraint
    client
        .execute(
            "CREATE TABLE #test_constraints (id INT PRIMARY KEY, name VARCHAR(50))",
            &[],
        )
        .await?;
    client
        .execute("INSERT INTO #test_constraints VALUES (1, 'first')", &[])
        .await?;

    // Try to violate the primary key constraint
    match client
        .execute("INSERT INTO #test_constraints VALUES (1, 'duplicate')", &[])
        .await
    {
        Ok(_) => println!("  Unexpectedly succeeded!"),
        Err(
            ref e @ Error::Server {
                number,
                ref message,
                ..
            },
        ) if number == 2627 => {
            println!("  Primary key violation (error {}): {}", number, message);
            println!("  is_transient: {}", e.is_transient());
            println!("  is_terminal: {}", e.is_terminal());
            println!("  This is NOT transient - fix your data, don't retry");
        }
        Err(e) => println!("  Other error: {:?}", e),
    }

    // Example 5: Using built-in error categorization
    println!("\n5. Built-in error categorization:");
    demonstrate_error_categorization();

    client.close().await?;
    println!("\nAll error handling examples completed.");

    Ok(())
}

fn demonstrate_error_categorization() {
    // Create sample errors to demonstrate categorization
    let errors: Vec<(&str, Error)> = vec![
        ("Connection timeout", Error::ConnectTimeout),
        ("Invalid config", Error::Config("Bad value".into())),
        (
            "Deadlock",
            Error::Server {
                number: 1205,
                class: 13,
                state: 1,
                message: "Transaction was deadlocked".into(),
                server: None,
                procedure: None,
                line: 0,
            },
        ),
        (
            "Syntax error",
            Error::Server {
                number: 102,
                class: 15,
                state: 1,
                message: "Incorrect syntax".into(),
                server: None,
                procedure: None,
                line: 1,
            },
        ),
    ];

    for (name, error) in errors {
        let transient = if error.is_transient() { "YES" } else { "NO" };
        let terminal = if error.is_terminal() { "YES" } else { "NO" };
        let action = if error.is_transient() {
            "Retry with backoff"
        } else if error.is_terminal() {
            "Fix code/data, redeploy"
        } else {
            "Investigate"
        };
        println!(
            "  {} -> transient: {}, terminal: {} -> {}",
            name, transient, terminal, action
        );
    }
}
