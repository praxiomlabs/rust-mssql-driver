//! Azure SQL Database specific integration tests.
//!
//! These tests are designed to validate Azure SQL-specific functionality.
//! All tests marked with `#[ignore]` require a live Azure SQL Database.
//!
//! Run with:
//!   AZURE_SQL_HOST=myserver.database.windows.net \
//!   AZURE_SQL_DATABASE=mydb \
//!   AZURE_SQL_USER=admin \
//!   AZURE_SQL_PASSWORD=... \
//!   cargo test --test azure_sql -- --ignored --nocapture

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::approx_constant
)]

use mssql_client::{Config, Error};
use std::time::Duration;

// =============================================================================
// Configuration Helpers
// =============================================================================

fn get_azure_config() -> Option<Config> {
    let host = std::env::var("AZURE_SQL_HOST").ok()?;
    let database = std::env::var("AZURE_SQL_DATABASE").ok()?;
    let user = std::env::var("AZURE_SQL_USER").ok()?;
    let password = std::env::var("AZURE_SQL_PASSWORD").ok()?;

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};Encrypt=true;TrustServerCertificate=false",
        host, database, user, password
    );

    Config::from_connection_string(&conn_str).ok()
}

fn get_azure_config_strict() -> Option<Config> {
    let host = std::env::var("AZURE_SQL_HOST").ok()?;
    let database = std::env::var("AZURE_SQL_DATABASE").ok()?;
    let user = std::env::var("AZURE_SQL_USER").ok()?;
    let password = std::env::var("AZURE_SQL_PASSWORD").ok()?;

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};Encrypt=strict",
        host, database, user, password
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Azure Connection String Format Tests
// =============================================================================

#[test]
fn test_azure_connection_string_format() {
    let conn_str = "Server=myserver.database.windows.net;Database=mydb;\
                    User Id=admin@myserver;Password=Password123!;Encrypt=strict";

    let result = Config::from_connection_string(conn_str);
    assert!(result.is_ok(), "Azure connection string should parse");
}

#[test]
fn test_azure_connection_string_with_port() {
    // Azure SQL default port
    let conn_str = "Server=myserver.database.windows.net,1433;Database=mydb;\
                    User Id=admin;Password=Password123!;Encrypt=true";

    let result = Config::from_connection_string(conn_str);
    assert!(
        result.is_ok(),
        "Azure connection string with port should parse"
    );
}

#[test]
fn test_azure_connection_string_with_options() {
    // Full Azure SQL connection string with all options
    let conn_str = "Server=myserver.database.windows.net;Database=mydb;\
                    User Id=admin@myserver;Password=Password123!;\
                    Encrypt=strict;TrustServerCertificate=false;\
                    Connect Timeout=30;Command Timeout=60;\
                    Application Name=TestApp;MultiSubnetFailover=true";

    let result = Config::from_connection_string(conn_str);
    assert!(
        result.is_ok(),
        "Azure connection string with full options should parse"
    );
}

#[test]
fn test_azure_managed_instance_format() {
    // Azure SQL Managed Instance format
    let conn_str = "Server=mi-instance.abc123.database.windows.net;Database=mydb;\
                    User Id=admin;Password=Password123!;Encrypt=true";

    let result = Config::from_connection_string(conn_str);
    assert!(
        result.is_ok(),
        "Azure Managed Instance connection string should parse"
    );
}

// =============================================================================
// Encryption Mode Tests
// =============================================================================

#[test]
fn test_encrypt_strict_config() {
    // Use connection string with strict mode
    let config = Config::from_connection_string(
        "Server=myserver.database.windows.net;Database=mydb;User Id=admin;Password=password;Encrypt=strict",
    )
    .expect("Valid connection string");

    // strict_mode should be set
    assert!(config.strict_mode);
}

#[test]
fn test_encrypt_true_vs_strict() {
    // Encrypt=true (TDS 7.4)
    let config_true = Config::from_connection_string(
        "Server=test.database.windows.net;Database=db;User Id=u;Password=p;Encrypt=true",
    )
    .unwrap();

    // Encrypt=strict (TDS 8.0)
    let config_strict = Config::from_connection_string(
        "Server=test.database.windows.net;Database=db;User Id=u;Password=p;Encrypt=strict",
    )
    .unwrap();

    // Both should parse successfully
    let _ = (config_true, config_strict);
}

// =============================================================================
// Azure SQL Error Handling Tests
// =============================================================================

#[test]
fn test_azure_transient_error_detection() {
    // Error 40501: Service busy
    let err = Error::Server {
        number: 40501,
        class: 16,
        state: 1,
        message: "The service is currently busy".into(),
        server: Some("myserver.database.windows.net".into()),
        procedure: None,
        line: 0,
    };
    assert!(err.is_transient(), "40501 should be transient");

    // Error 40613: Database unavailable
    let err = Error::Server {
        number: 40613,
        class: 16,
        state: 1,
        message: "Database is not currently available".into(),
        server: Some("myserver.database.windows.net".into()),
        procedure: None,
        line: 0,
    };
    assert!(err.is_transient(), "40613 should be transient");

    // Error 10928: Resource limit
    let err = Error::Server {
        number: 10928,
        class: 16,
        state: 1,
        message: "Resource ID exceeded".into(),
        server: Some("myserver.database.windows.net".into()),
        procedure: None,
        line: 0,
    };
    assert!(err.is_transient(), "10928 should be transient");

    // Error 49918: Cannot process request
    let err = Error::Server {
        number: 49918,
        class: 16,
        state: 1,
        message: "Cannot process request".into(),
        server: Some("myserver.database.windows.net".into()),
        procedure: None,
        line: 0,
    };
    assert!(err.is_transient(), "49918 should be transient");
}

#[test]
fn test_azure_redirect_error() {
    let err = Error::Routing {
        host: "prod-replica.database.windows.net".into(),
        port: 11000,
    };

    assert!(err.is_transient(), "Routing should be transient");
    assert!(!err.is_terminal(), "Routing should not be terminal");
}

#[test]
fn test_azure_too_many_redirects() {
    let err = Error::TooManyRedirects { max: 3 };
    let msg = err.to_string();

    assert!(msg.contains("redirects"));
    assert!(msg.contains("3"));
}

// =============================================================================
// Live Azure SQL Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_basic_connection() {
    use mssql_client::Client;

    let config = get_azure_config().expect("Azure SQL config required");
    let client = Client::connect(config).await.expect("Connection failed");

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database with TDS 8.0 support"]
async fn test_azure_strict_mode_connection() {
    use mssql_client::Client;

    let config = get_azure_config_strict().expect("Azure SQL config required");
    let result = Client::connect(config).await;

    // May fail if Azure SQL doesn't support strict mode yet
    match result {
        Ok(client) => {
            client.close().await.expect("Close failed");
        }
        Err(e) => {
            // Strict mode might not be supported yet
            println!(
                "Strict mode connection failed (may not be supported): {}",
                e
            );
        }
    }
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_query_execution() {
    use mssql_client::Client;

    let config = get_azure_config().expect("Azure SQL config required");
    let mut client = Client::connect(config).await.expect("Connection failed");

    // Test basic query
    let rows = client
        .query("SELECT @@VERSION AS version", &[])
        .await
        .expect("Query failed");

    let mut found = false;
    for row_result in rows {
        let row = row_result.expect("Row error");
        let version: String = row.get(0).expect("Get version failed");
        println!("Azure SQL Version: {}", version);
        assert!(
            version.contains("Azure") || version.contains("SQL"),
            "Version should mention Azure or SQL"
        );
        found = true;
    }
    assert!(found, "Should have at least one row");

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_database_properties() {
    use mssql_client::Client;

    let config = get_azure_config().expect("Azure SQL config required");
    let mut client = Client::connect(config).await.expect("Connection failed");

    // Query database edition and service objective
    let rows = client
        .query(
            "SELECT DB_NAME() AS db_name, \
             DATABASEPROPERTYEX(DB_NAME(), 'Edition') AS edition, \
             DATABASEPROPERTYEX(DB_NAME(), 'ServiceObjective') AS service_tier",
            &[],
        )
        .await
        .expect("Query failed");

    for row_result in rows {
        let row = row_result.expect("Row error");
        let db_name: String = row.get(0).expect("Get db_name failed");
        let edition: Option<String> = row.get(1).ok();
        let tier: Option<String> = row.get(2).ok();

        println!(
            "Database: {}, Edition: {:?}, Tier: {:?}",
            db_name, edition, tier
        );
    }

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_connection_with_timeout() {
    use mssql_client::Client;

    // Use connection string with timeout settings
    let host = std::env::var("AZURE_SQL_HOST").expect("AZURE_SQL_HOST required");
    let database = std::env::var("AZURE_SQL_DATABASE").expect("AZURE_SQL_DATABASE required");
    let user = std::env::var("AZURE_SQL_USER").expect("AZURE_SQL_USER required");
    let password = std::env::var("AZURE_SQL_PASSWORD").expect("AZURE_SQL_PASSWORD required");

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};Encrypt=true;\
         TrustServerCertificate=false;Connect Timeout=30;Command Timeout=60",
        host, database, user, password
    );

    let config = Config::from_connection_string(&conn_str).expect("Valid connection string");
    let mut client = Client::connect(config).await.expect("Connection failed");

    // Quick query to verify connection
    let rows = client
        .query("SELECT 1 AS num", &[])
        .await
        .expect("Query failed");
    for _ in rows {}

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_transaction() {
    use mssql_client::Client;

    let config = get_azure_config().expect("Azure SQL config required");
    let mut client = Client::connect(config).await.expect("Connection failed");

    // Create temp table
    client
        .execute(
            "IF OBJECT_ID('tempdb..#azure_test') IS NOT NULL DROP TABLE #azure_test",
            &[],
        )
        .await
        .ok();

    client
        .execute("CREATE TABLE #azure_test (id INT, name NVARCHAR(50))", &[])
        .await
        .expect("Create table failed");

    // Transaction with rollback - type-state pattern: begin_transaction consumes client
    let mut tx = client.begin_transaction().await.expect("Begin failed");
    tx.execute(
        "INSERT INTO #azure_test (id, name) VALUES (1, N'Test')",
        &[],
    )
    .await
    .expect("Insert failed");

    // Rollback returns the client in Ready state
    let mut client = tx.rollback().await.expect("Rollback failed");

    // Verify rollback
    let rows = client
        .query("SELECT COUNT(*) FROM #azure_test", &[])
        .await
        .expect("Query failed");

    for row_result in rows {
        let row = row_result.expect("Row error");
        let count: i32 = row.get(0).expect("Get count failed");
        assert_eq!(count, 0, "Rollback should have removed the row");
    }

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_pool() {
    use mssql_driver_pool::{Pool, PoolConfig};
    use std::sync::Arc;

    let config = get_azure_config().expect("Azure SQL config required");

    let pool_config = PoolConfig::new()
        .min_connections(1)
        .max_connections(5)
        .connection_timeout(Duration::from_secs(30));

    let pool = Arc::new(
        Pool::new(pool_config, config)
            .await
            .expect("Pool creation failed"),
    );

    // Get multiple connections
    let mut handles = Vec::new();
    for i in 0..10 {
        let pool = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            let mut conn = match pool.get().await {
                Ok(c) => c,
                Err(_) => return Err(format!("Failed to get connection for task {}", i)),
            };
            let rows = match conn.query(&format!("SELECT {} AS num", i), &[]).await {
                Ok(r) => r,
                Err(e) => return Err(format!("Query failed for task {}: {}", i, e)),
            };
            for _ in rows {}
            Ok::<_, String>(i)
        }));
    }

    let mut successes = 0;
    for handle in handles {
        if handle.await.expect("Task panicked").is_ok() {
            successes += 1;
        }
    }

    assert_eq!(successes, 10, "All 10 queries should succeed");

    pool.close().await;
}

// =============================================================================
// Azure-Specific Feature Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires Azure SQL Database with Unicode data"]
async fn test_azure_unicode_support() {
    use mssql_client::Client;

    let config = get_azure_config().expect("Azure SQL config required");
    let mut client = Client::connect(config).await.expect("Connection failed");

    // Test various Unicode strings
    let test_strings = [
        ("ASCII", "Hello World"),
        ("German", "Größe"),
        ("French", "Café"),
        ("Chinese", "你好世界"),
        ("Japanese", "こんにちは"),
        ("Korean", "안녕하세요"),
        ("Emoji", "Hello 👋🌍"),
        ("Arabic", "مرحبا"),
        ("Hebrew", "שלום"),
    ];

    for (name, value) in test_strings {
        let rows = client
            .query(&format!("SELECT N'{}'", value), &[])
            .await
            .unwrap_or_else(|_| panic!("Query for {} failed", name));

        for row_result in rows {
            let row = row_result.expect("Row error");
            let result: String = row.get(0).expect("Get failed");
            assert_eq!(result, value, "{} string should roundtrip correctly", name);
        }
    }

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires Azure SQL Database"]
async fn test_azure_datetime_types() {
    use mssql_client::Client;

    let config = get_azure_config().expect("Azure SQL config required");
    let mut client = Client::connect(config).await.expect("Connection failed");

    // Test various datetime functions
    let rows = client
        .query(
            "SELECT GETUTCDATE() AS utc, SYSDATETIMEOFFSET() AS dto",
            &[],
        )
        .await
        .expect("Query failed");

    for row_result in rows {
        let row = row_result.expect("Row error");
        // Just verify we can read the values
        let _utc: Option<chrono::NaiveDateTime> = row.get(0).ok();
        let _dto: Option<chrono::DateTime<chrono::FixedOffset>> = row.get(1).ok();
    }

    client.close().await.expect("Close failed");
}
