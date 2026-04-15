//! Windows SSPI integrated authentication tests.
//!
//! These tests require:
//! - Windows machine
//! - SQL Server with Windows Authentication enabled
//! - Current Windows user has a SQL Server login
//! - TCP/IP enabled on the SQL Server instance
//!
//! Run with:
//! ```bash
//! cargo test -p mssql-client --test windows_sspi --features sspi-auth -- --ignored
//! ```
//!
//! By default, connects to `localhost:1433`. Override with environment variables:
//! ```bash
//! export MSSQL_HOST=myserver
//! export MSSQL_PORT=1433
//! ```

#![cfg(all(windows, feature = "sspi-auth"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config};

fn get_integrated_config() -> Config {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);

    let conn_str = format!(
        "Server={host},{port};Database=master;Integrated Security=true;TrustServerCertificate=true"
    );

    Config::from_connection_string(&conn_str).expect("Failed to parse connection string")
}

#[tokio::test]
#[ignore = "Requires Windows with SQL Server and Windows Authentication"]
async fn test_sspi_connect_and_query() {
    let config = get_integrated_config();
    let mut client = Client::connect(config)
        .await
        .expect("SSPI connection failed");

    // Verify we're connected and authenticated
    let rows = client
        .query("SELECT SYSTEM_USER AS [user], auth_scheme FROM sys.dm_exec_connections WHERE session_id = @@SPID", &[])
        .await
        .expect("Query failed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row error");
        let user: String = row.get(0).expect("Get user failed");
        let scheme: String = row.get(1).expect("Get scheme failed");

        assert!(!user.is_empty(), "SYSTEM_USER should not be empty");
        assert!(
            scheme == "NTLM" || scheme == "KERBEROS",
            "Expected NTLM or KERBEROS auth scheme, got: {scheme}"
        );
        found = true;
    }
    assert!(found, "Expected at least one row");

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Windows with SQL Server and Windows Authentication"]
async fn test_sspi_transaction() {
    let config = get_integrated_config();
    let client = Client::connect(config)
        .await
        .expect("SSPI connection failed");

    // Begin a transaction via SSPI-authenticated connection
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin transaction failed");

    tx.execute("SELECT 1", &[])
        .await
        .expect("Execute in transaction failed");

    // Rollback (we didn't change anything, just verifying it works)
    let _client = tx.rollback().await.expect("Rollback failed");
}

#[tokio::test]
#[ignore = "Requires Windows with SQL Server and Windows Authentication"]
async fn test_sspi_server_version() {
    let config = get_integrated_config();
    let mut client = Client::connect(config)
        .await
        .expect("SSPI connection failed");

    let rows = client
        .query("SELECT @@VERSION", &[])
        .await
        .expect("Query failed");

    for result in rows {
        let row = result.expect("Row error");
        let version: String = row.get(0).expect("Get version failed");
        assert!(
            version.contains("Microsoft SQL Server"),
            "Expected SQL Server version string, got: {version}"
        );
    }

    client.close().await.expect("Close failed");
}
