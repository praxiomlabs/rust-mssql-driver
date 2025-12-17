//! Azure SQL Database connection example.
//!
//! This example demonstrates how to connect to Azure SQL Database,
//! including handling redirects and using Azure-specific features.
//!
//! # Running
//!
//! ```bash
//! # Azure SQL connection details
//! export AZURE_SQL_SERVER=yourserver.database.windows.net
//! export AZURE_SQL_DATABASE=yourdb
//! export AZURE_SQL_USER=yourusername
//! export AZURE_SQL_PASSWORD=yourpassword
//!
//! cargo run --example azure_sql
//! ```
//!
//! # Azure Connection String Format
//!
//! Azure SQL requires specific connection string parameters:
//! - `Encrypt=true` or `Encrypt=strict` (always encrypted)
//! - `TrustServerCertificate=false` (validate Azure certificates)
//! - Server name must include `.database.windows.net`

use mssql_client::{Client, Config, Error, Ready};
use std::time::Duration;

/// Azure-optimized retry configuration.
struct AzureRetryConfig {
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl Default for AzureRetryConfig {
    fn default() -> Self {
        Self {
            // Azure recommends more retries than on-premises
            max_retries: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
        }
    }
}

async fn connect_with_retry(config: Config, retry_config: &AzureRetryConfig) -> Result<Client<Ready>, Error> {
    let mut attempts = 0;
    let mut last_error = None;

    while attempts <= retry_config.max_retries {
        if attempts > 0 {
            let backoff = std::cmp::min(
                retry_config.initial_backoff * 2u32.pow(attempts - 1),
                retry_config.max_backoff,
            );
            // Add jitter (10-50% of backoff)
            let jitter = backoff / 5;
            let total_wait = backoff + jitter;
            println!("  Retry {}/{} after {:?}", attempts, retry_config.max_retries, total_wait);
            tokio::time::sleep(total_wait).await;
        }

        match Client::connect(config.clone()).await {
            Ok(client) => return Ok(client),
            Err(e) if e.is_transient() => {
                println!("  Transient error: {:?}", e);
                last_error = Some(e);
                attempts += 1;
            }
            Err(e) => return Err(e),
        }
    }

    Err(last_error.unwrap_or_else(|| Error::Connection("Max retries exceeded".into())))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Check for Azure credentials
    let server = std::env::var("AZURE_SQL_SERVER").ok();
    let database = std::env::var("AZURE_SQL_DATABASE").ok();
    let user = std::env::var("AZURE_SQL_USER").ok();
    let password = std::env::var("AZURE_SQL_PASSWORD").ok();

    println!("=== Azure SQL Database Connection Example ===\n");

    // If Azure credentials are not set, demonstrate with mock data
    if server.is_none() {
        println!("Note: AZURE_SQL_* environment variables not set.");
        println!("This example will demonstrate the connection pattern.\n");
        demonstrate_azure_patterns();
        return Ok(());
    }

    let server = server.unwrap();
    let database = database.unwrap_or_else(|| "master".into());
    let user = user.unwrap();
    let password = password.unwrap();

    // Build Azure-optimized connection string
    // Key differences from on-premises:
    // 1. Always use encryption
    // 2. Don't trust server certificate (use Azure's CA)
    // 3. Include database name in connection string
    let conn_str = format!(
        "Server={server};Database={database};User Id={user};Password={password};\
         Encrypt=true;TrustServerCertificate=false;Connection Timeout=30;\
         Application Name=rust-mssql-driver-example"
    );

    let config = Config::from_connection_string(&conn_str)?;

    println!("Connecting to Azure SQL: {}", server);
    println!("  Database: {}", database);
    println!("  Encryption: enabled (required for Azure)");
    println!();

    // Connect with Azure-appropriate retry logic
    let retry_config = AzureRetryConfig::default();
    let mut client = connect_with_retry(config, &retry_config).await?;

    println!("Connected successfully!\n");

    // Example 1: Check Azure SQL version and edition
    println!("1. Azure SQL Server Information:");
    let rows = client
        .query(
            "SELECT @@VERSION, SERVERPROPERTY('Edition'), SERVERPROPERTY('EngineEdition')",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;
        let version: String = row.get(0)?;
        let edition: String = row.get(1)?;
        let engine: i32 = row.get(2)?;

        println!("  Version: {}...", &version[..60.min(version.len())]);
        println!("  Edition: {}", edition);
        println!(
            "  Engine: {} ({})",
            engine,
            match engine {
                5 => "Azure SQL Database",
                6 => "Azure SQL Data Warehouse",
                8 => "Azure SQL Managed Instance",
                _ => "Unknown",
            }
        );
    }

    // Example 2: Check service tier and resource limits
    println!("\n2. Service Tier Information:");
    let rows = client
        .query(
            "SELECT \
                DATABASEPROPERTYEX(DB_NAME(), 'ServiceObjective') AS ServiceTier, \
                DATABASEPROPERTYEX(DB_NAME(), 'Edition') AS Edition",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;
        let tier: Option<String> = row.try_get(0);
        let edition: Option<String> = row.try_get(1);
        println!("  Service Tier: {:?}", tier);
        println!("  Edition: {:?}", edition);
    }

    // Example 3: Query with parameters (uses RPC for Azure efficiency)
    println!("\n3. Parameterized Query (RPC):");
    let name = "test_user";
    let rows = client
        .query("SELECT @p1 AS input, SUSER_NAME() AS current_user", &[&name])
        .await?;

    for result in rows {
        let row = result?;
        let input: String = row.get(0)?;
        let current: String = row.get(1)?;
        println!("  Input: {}, Current User: {}", input, current);
    }

    // Example 4: Handling Azure-specific scenarios
    println!("\n4. Azure-specific considerations:");
    println!("  - Redirects: Handled automatically by driver");
    println!("  - Throttling: Use exponential backoff (implemented above)");
    println!("  - Failover: Reconnect with retry on connection loss");
    println!("  - Read replicas: Use ApplicationIntent=ReadOnly for read workloads");

    client.close().await?;
    println!("\nConnection closed.");

    Ok(())
}

fn demonstrate_azure_patterns() {
    println!("Azure SQL Connection Patterns:\n");

    println!("1. Connection String Format:");
    println!("   Server=yourserver.database.windows.net;");
    println!("   Database=yourdb;");
    println!("   User Id=youruser;");
    println!("   Password=yourpassword;");
    println!("   Encrypt=true;");
    println!("   TrustServerCertificate=false;");
    println!();

    println!("2. Azure-Specific Transient Errors (retriable):");
    let azure_errors = [
        (40501, "Service is busy - retry with backoff"),
        (40613, "Database unavailable - failover in progress"),
        (49918, "Cannot process request - insufficient resources"),
        (40197, "Service error - retry"),
    ];
    for (code, desc) in azure_errors {
        println!("   Error {}: {}", code, desc);
    }
    println!();

    println!("3. Read Replica Usage:");
    println!("   // Connection string for read-only workloads:");
    println!("   Server=...;ApplicationIntent=ReadOnly;...");
    println!();

    println!("4. Retry Configuration for Azure:");
    println!("   - Max retries: 5 (more than on-premises)");
    println!("   - Initial backoff: 100ms");
    println!("   - Max backoff: 60s");
    println!("   - Always use jitter to avoid thundering herd");

    println!("\n5. Error categorization (built-in):");
    let test_errors = [
        (40501, "Service busy"),
        (40613, "Database unavailable"),
        (102, "Syntax error"),
        (2627, "Constraint violation"),
    ];
    for (code, name) in test_errors {
        let transient = Error::is_transient_server_error(code);
        let terminal = Error::is_terminal_server_error(code);
        println!(
            "   Error {}: {} - transient: {}, terminal: {}",
            code, name, transient, terminal
        );
    }
}
