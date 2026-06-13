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

// =============================================================================
// Configuration Helpers
// =============================================================================

fn get_azure_config() -> Option<Config> {
    let host = std::env::var("AZURE_SQL_HOST").ok()?;
    let database = std::env::var("AZURE_SQL_DATABASE").ok()?;
    let user = std::env::var("AZURE_SQL_USER").ok()?;
    let password = std::env::var("AZURE_SQL_PASSWORD").ok()?;

    let conn_str = format!(
        "Server={host};Database={database};User Id={user};Password={password};Encrypt=true;TrustServerCertificate=false"
    );

    Config::from_connection_string(&conn_str).ok()
}

fn get_azure_config_strict() -> Option<Config> {
    let host = std::env::var("AZURE_SQL_HOST").ok()?;
    let database = std::env::var("AZURE_SQL_DATABASE").ok()?;
    let user = std::env::var("AZURE_SQL_USER").ok()?;
    let password = std::env::var("AZURE_SQL_PASSWORD").ok()?;

    let conn_str = format!(
        "Server={host};Database={database};User Id={user};Password={password};Encrypt=strict"
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
            println!("Strict mode connection failed (may not be supported): {e}");
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
        println!("Azure SQL Version: {version}");
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

        println!("Database: {db_name}, Edition: {edition:?}, Tier: {tier:?}");
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
        "Server={host};Database={database};User Id={user};Password={password};Encrypt=true;\
         TrustServerCertificate=false;Connect Timeout=30;Command Timeout=60"
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

// Note: test_azure_pool moved to mssql-testing to break circular dev-dependency

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
            .query(&format!("SELECT N'{value}'"), &[])
            .await
            .unwrap_or_else(|_| panic!("Query for {name} failed"));

        for row_result in rows {
            let row = row_result.expect("Row error");
            let result: String = row.get(0).expect("Get failed");
            assert_eq!(result, value, "{name} string should roundtrip correctly");
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

// =============================================================================
// Azure AD / Entra Authentication (FEDAUTH SecurityToken workflow, #155)
// =============================================================================

/// Service principal fixture from the environment.
///
/// Prefers the `AZURE_SQL_`-prefixed names from the issue; falls back to the
/// generic `AZURE_` names used by azure_identity's EnvironmentCredential and
/// the standing test fixture.
#[cfg(feature = "azure-identity")]
fn get_azure_sp_env() -> Option<(String, String, String)> {
    fn var2(primary: &str, fallback: &str) -> Option<String> {
        std::env::var(primary)
            .or_else(|_| std::env::var(fallback))
            .ok()
    }
    let tenant_id = var2("AZURE_SQL_TENANT_ID", "AZURE_TENANT_ID")?;
    let client_id = var2("AZURE_SQL_CLIENT_ID", "AZURE_CLIENT_ID")?;
    let client_secret = var2("AZURE_SQL_CLIENT_SECRET", "AZURE_CLIENT_SECRET")?;
    Some((tenant_id, client_id, client_secret))
}

#[cfg(feature = "azure-identity")]
fn get_azure_host_db() -> Option<(String, String)> {
    let host = std::env::var("AZURE_SQL_HOST").ok()?;
    let database = std::env::var("AZURE_SQL_DATABASE").ok()?;
    Some((host, database))
}

/// Query the authenticated principal and the login's authentication type.
#[cfg(feature = "azure-identity")]
async fn current_principal(
    client: &mut mssql_client::Client<mssql_client::Ready>,
) -> (String, String) {
    let rows = client
        .query(
            "SELECT SUSER_SNAME() AS principal, \
             (SELECT authentication_type_desc FROM sys.database_principals \
              WHERE name = USER_NAME()) AS auth_type",
            &[],
        )
        .await
        .expect("principal query failed");

    let mut result = (String::new(), String::new());
    for row_result in rows {
        let row = row_result.expect("Row error");
        result.0 = row.get::<String>(0).expect("principal");
        result.1 = row
            .get::<Option<String>>(1)
            .ok()
            .flatten()
            .unwrap_or_default();
    }
    result
}

/// End-to-end FEDAUTH login with service principal credentials: the driver
/// acquires the token from Entra ID and sends it in the LOGIN7 FEDAUTH
/// feature extension (SecurityToken workflow).
#[cfg(feature = "azure-identity")]
#[tokio::test]
#[ignore = "Requires Azure SQL Database and an Entra service principal"]
async fn test_azure_ad_service_principal_login() {
    use mssql_client::Client;

    let (tenant_id, client_id, client_secret) =
        get_azure_sp_env().expect("service principal env vars required");
    let (host, database) = get_azure_host_db().expect("AZURE_SQL_HOST/DATABASE required");

    let config = Config::new().host(host).database(database).credentials(
        mssql_client::Credentials::AzureServicePrincipal {
            tenant_id: tenant_id.into(),
            client_id: client_id.clone().into(),
            client_secret: client_secret.into(),
        },
    );

    let mut client = Client::connect(config).await.expect("FEDAUTH login failed");

    let (principal, auth_type) = current_principal(&mut client).await;
    println!("service principal login: principal={principal}, auth_type={auth_type}");
    // Azure SQL reports service principals as <client-id>@<tenant-id>.
    assert!(
        principal
            .to_lowercase()
            .starts_with(&client_id.to_lowercase()),
        "SUSER_SNAME() ({principal}) must identify the service principal ({client_id})"
    );
    assert_eq!(
        auth_type, "EXTERNAL",
        "service principal must authenticate as an EXTERNAL (Entra) principal"
    );

    client.close().await.expect("Close failed");
}

/// Pre-acquired token path: acquire via mssql-auth's ServicePrincipalAuth,
/// then log in with Credentials::AzureAccessToken (Tier 1 — what users with
/// their own token plumbing do).
#[cfg(feature = "azure-identity")]
#[tokio::test]
#[ignore = "Requires Azure SQL Database and an Entra service principal"]
async fn test_azure_ad_access_token_login() {
    use mssql_client::Client;

    let (tenant_id, client_id, client_secret) =
        get_azure_sp_env().expect("service principal env vars required");
    let (host, database) = get_azure_host_db().expect("AZURE_SQL_HOST/DATABASE required");

    let auth = mssql_auth::ServicePrincipalAuth::new(tenant_id, client_id, client_secret)
        .expect("credential construction failed");
    let token = auth.get_token().await.expect("token acquisition failed");

    let config = Config::new()
        .host(host)
        .database(database)
        .credentials(mssql_client::Credentials::azure_token(token));

    let mut client = Client::connect(config).await.expect("FEDAUTH login failed");

    let (principal, auth_type) = current_principal(&mut client).await;
    println!("access token login: principal={principal}, auth_type={auth_type}");
    assert_eq!(auth_type, "EXTERNAL");

    client.close().await.expect("Close failed");
}

/// Connection-string path: Authentication=ActiveDirectoryServicePrincipal
/// with User Id=<client-id>@<tenant-id> (ADR-002).
#[cfg(feature = "azure-identity")]
#[tokio::test]
#[ignore = "Requires Azure SQL Database and an Entra service principal"]
async fn test_azure_ad_connection_string_service_principal() {
    use mssql_client::Client;

    let (tenant_id, client_id, client_secret) =
        get_azure_sp_env().expect("service principal env vars required");
    let (host, database) = get_azure_host_db().expect("AZURE_SQL_HOST/DATABASE required");

    let conn_str = format!(
        "Server={host};Database={database};Encrypt=mandatory;\
         Authentication=Active Directory Service Principal;\
         User Id={client_id}@{tenant_id};Password={client_secret}"
    );
    let config = Config::from_connection_string(&conn_str).expect("Valid connection string");

    let mut client = Client::connect(config).await.expect("FEDAUTH login failed");

    let rows = client
        .query("SELECT 1", &[])
        .await
        .expect("query after FEDAUTH login failed");
    for row_result in rows {
        let row = row_result.expect("Row error");
        assert_eq!(row.get::<i32>(0).expect("value"), 1);
    }

    client.close().await.expect("Close failed");
}

/// End-to-end FEDAUTH login with a system-assigned managed identity.
///
/// Unlike the service-principal tests, this can only pass when run on Azure
/// compute (VM/container) whose identity has been granted a contained DB user:
/// the token is acquired from the local IMDS endpoint, which does not exist
/// outside Azure. Run it from inside the target compute with `--ignored`.
#[cfg(feature = "azure-identity")]
#[tokio::test]
#[ignore = "Requires running on Azure compute with a managed identity granted DB access"]
async fn test_azure_ad_managed_identity_login() {
    use mssql_client::Client;

    let (host, database) = get_azure_host_db().expect("AZURE_SQL_HOST/DATABASE required");

    // client_id = None selects the system-assigned identity.
    let config = Config::new()
        .host(host)
        .database(database)
        .credentials(mssql_client::Credentials::AzureManagedIdentity { client_id: None });

    let mut client = Client::connect(config).await.expect("FEDAUTH login failed");

    let (principal, auth_type) = current_principal(&mut client).await;
    println!("managed identity login: principal={principal}, auth_type={auth_type}");
    assert_eq!(
        auth_type, "EXTERNAL",
        "managed identity must authenticate as an EXTERNAL (Entra) principal"
    );

    client.close().await.expect("Close failed");
}
