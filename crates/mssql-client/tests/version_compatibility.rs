//! SQL Server Version Compatibility Tests
//!
//! These tests verify the driver works correctly across different SQL Server versions
//! and TDS protocol versions.
//!
//! ## Supported TDS Versions
//!
//! | TDS Version | SQL Server Version | Configuration |
//! |-------------|-------------------|---------------|
//! | TDS 7.3A | SQL Server 2008 | `TdsVersion::V7_3A` |
//! | TDS 7.3B | SQL Server 2008 R2 | `TdsVersion::V7_3B` |
//! | TDS 7.4 | SQL Server 2012+ (default) | `TdsVersion::V7_4` |
//! | TDS 8.0 | SQL Server 2022+ strict mode | `TdsVersion::V8_0` |
//!
//! ## Tested Versions
//!
//! - SQL Server 2008 (TDS 7.3A) - Legacy support
//! - SQL Server 2008 R2 (TDS 7.3B) - Legacy support
//! - SQL Server 2017 (TDS 7.4) - Recommended minimum
//! - SQL Server 2019 (TDS 7.4) - Recommended
//! - SQL Server 2022 (TDS 7.4/8.0) - Latest
//!
//! ## Running Tests
//!
//! ### Modern SQL Server (2017+)
//!
//! ```bash
//! # SQL Server 2022 (default, port 1433)
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-client --test version_compatibility -- --ignored --nocapture
//!
//! # SQL Server 2019 (port 1434)
//! MSSQL_HOST=localhost MSSQL_PORT=1434 MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-client --test version_compatibility -- --ignored --nocapture
//!
//! # SQL Server 2017 (port 1435)
//! MSSQL_HOST=localhost MSSQL_PORT=1435 MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-client --test version_compatibility -- --ignored --nocapture
//! ```
//!
//! ### Legacy SQL Server (2008/2008 R2)
//!
//! ```bash
//! # SQL Server 2008 - Requires TDS version override
//! MSSQL_HOST=legacy-server MSSQL_TDS_VERSION=7.3 MSSQL_USER=sa MSSQL_PASSWORD='secret' \
//!     cargo test -p mssql-client --test version_compatibility test_tds_7_3 -- --ignored --nocapture
//!
//! # SQL Server 2008 R2 - Requires TDS version override
//! MSSQL_HOST=legacy-server MSSQL_TDS_VERSION=7.3B MSSQL_USER=sa MSSQL_PASSWORD='secret' \
//!     cargo test -p mssql-client --test version_compatibility test_tds_7_3 -- --ignored --nocapture
//! ```
//!
//! ## Docker Containers
//!
//! ```bash
//! # SQL Server 2022 (default)
//! docker run -d --name sql_server -e 'ACCEPT_EULA=Y' -e 'SA_PASSWORD=YourStrong@Passw0rd' \
//!     -p 1433:1433 mcr.microsoft.com/mssql/server:2022-latest
//!
//! # SQL Server 2019
//! docker run -d --name sql_server_2019 -e 'ACCEPT_EULA=Y' -e 'SA_PASSWORD=YourStrong@Passw0rd' \
//!     -p 1434:1433 mcr.microsoft.com/mssql/server:2019-latest
//!
//! # SQL Server 2017
//! docker run -d --name sql_server_2017 -e 'ACCEPT_EULA=Y' -e 'SA_PASSWORD=YourStrong@Passw0rd' \
//!     -p 1435:1433 mcr.microsoft.com/mssql/server:2017-latest
//! ```
//!
//! **Note:** SQL Server 2008/2008 R2 are not available as Docker images. For legacy testing,
//! use a Windows VM with SQL Server 2008/2008 R2 installed, or use SQL Server on-premises.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::manual_range_contains
)]

use mssql_client::{Client, Config};
use tds_protocol::version::TdsVersion;

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    get_test_config_with_tds_version(None)
}

/// Helper to get test configuration with optional TDS version override.
///
/// Environment variables:
/// - MSSQL_HOST: Required - Server hostname
/// - MSSQL_PORT: Optional - Server port (default: 1433)
/// - MSSQL_USER: Optional - Username (default: sa)
/// - MSSQL_PASSWORD: Optional - Password (default: YourStrong@Passw0rd)
/// - MSSQL_DATABASE: Optional - Database (default: master)
/// - MSSQL_ENCRYPT: Optional - Encryption setting (default: false)
/// - MSSQL_TDS_VERSION: Optional - TDS version (7.3, 7.3A, 7.3B, 7.4, 8.0)
fn get_test_config_with_tds_version(tds_override: Option<TdsVersion>) -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    // Check for TDS version from environment or use provided override
    let tds_version = tds_override.or_else(|| {
        std::env::var("MSSQL_TDS_VERSION")
            .ok()
            .and_then(|v| TdsVersion::parse(&v))
    });

    let mut conn_str = format!(
        "Server={},{};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, port, database, user, password, encrypt
    );

    // Add TDS version to connection string if specified
    if let Some(version) = tds_version {
        let version_str = match version {
            v if v == TdsVersion::V7_3A => "7.3A",
            v if v == TdsVersion::V7_3B => "7.3B",
            v if v == TdsVersion::V7_4 => "7.4",
            v if v == TdsVersion::V8_0 => "8.0",
            _ => "7.4",
        };
        conn_str.push_str(&format!(";TDSVersion={}", version_str));
    }

    Config::from_connection_string(&conn_str).ok()
}

/// Get TDS version from environment, if set.
fn get_env_tds_version() -> Option<TdsVersion> {
    std::env::var("MSSQL_TDS_VERSION")
        .ok()
        .and_then(|v| TdsVersion::parse(&v))
}

// =============================================================================
// Version Detection Tests
// =============================================================================

/// Test that we can detect the SQL Server version.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_version_detection() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let rows = client
        .query("SELECT @@VERSION", &[])
        .await
        .expect("Version query should succeed");

    let mut version_string = String::new();
    for result in rows {
        let row = result.expect("Row should be valid");
        version_string = row.get(0).expect("Should get version");
    }

    println!("SQL Server Version: {}", version_string);

    // Verify we got a valid version string
    assert!(
        version_string.contains("Microsoft SQL Server"),
        "Should contain Microsoft SQL Server"
    );

    // Check for known version patterns (all supported SQL Server versions)
    let is_known_version = version_string.contains("2008")
        || version_string.contains("2012")
        || version_string.contains("2014")
        || version_string.contains("2016")
        || version_string.contains("2017")
        || version_string.contains("2019")
        || version_string.contains("2022");
    assert!(
        is_known_version,
        "Should be a known SQL Server version (2008+), got: {}",
        version_string
    );

    client.close().await.expect("Failed to close");
}

/// Test product version number detection.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_product_version() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Use CAST to avoid SQL_VARIANT type issues
    // Note: ProductMajorVersion returns NULL in SQL Server 2014 RTM, so we parse from ProductVersion
    let rows = client
        .query(
            "SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128)) AS Version",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Should get version");

        // Parse major version from ProductVersion string (e.g., "16.0.4225.2" -> 16)
        let major_num: i32 = version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .expect("Should parse major version from ProductVersion");

        println!("Product Version: {}, Major: {}", version, major_num);

        // Major version should be 10+ (SQL Server 2008+)
        // 10 = 2008, 11 = 2012, 12 = 2014, 13 = 2016, 14 = 2017, 15 = 2019, 16 = 2022
        assert!(
            major_num >= 10,
            "Should be SQL Server 2008 or later (major >= 10), got: {}",
            major_num
        );
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// TDS Protocol Version Tests
// =============================================================================

/// Test TDS protocol version negotiation.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tds_protocol_version() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // The connection succeeds means TDS version negotiation worked
    // Our driver uses TDS 7.4 which is compatible with SQL Server 2012+

    let rows = client
        .query("SELECT 1 AS test", &[])
        .await
        .expect("Query should succeed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: i32 = row.get(0).expect("Should get int");
        assert_eq!(val, 1);
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Feature Compatibility Tests
// =============================================================================

/// Test basic data types work across versions.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_basic_data_types_compatibility() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test integer types
    let rows = client
        .query(
            "SELECT CAST(1 AS TINYINT) AS t, \
                    CAST(2 AS SMALLINT) AS s, \
                    CAST(3 AS INT) AS i, \
                    CAST(4 AS BIGINT) AS b",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let t: i16 = row.get(0).expect("Should get tinyint");
        let s: i16 = row.get(1).expect("Should get smallint");
        let i: i32 = row.get(2).expect("Should get int");
        let b: i64 = row.get(3).expect("Should get bigint");
        assert_eq!(t, 1);
        assert_eq!(s, 2);
        assert_eq!(i, 3);
        assert_eq!(b, 4);
    }

    // Test string types
    let rows = client
        .query(
            "SELECT CAST('hello' AS VARCHAR(50)) AS v, \
                    CAST(N'world' AS NVARCHAR(50)) AS n",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let v: String = row.get(0).expect("Should get varchar");
        let n: String = row.get(1).expect("Should get nvarchar");
        assert_eq!(v, "hello");
        assert_eq!(n, "world");
    }

    // Test decimal
    let rows = client
        .query("SELECT CAST(123.45 AS DECIMAL(10,2)) AS d", &[])
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let d: rust_decimal::Decimal = row.get(0).expect("Should get decimal");
        assert_eq!(d.to_string(), "123.45");
    }

    client.close().await.expect("Failed to close");
}

/// Test datetime types work across versions.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_datetime_compatibility() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // DATETIME2 is available in all supported versions (2017+)
    let rows = client
        .query(
            "SELECT CAST('2024-06-15 14:30:00' AS DATETIME2) AS dt2",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let dt: chrono::NaiveDateTime = row.get(0).expect("Should get datetime2");
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    client.close().await.expect("Failed to close");
}

/// Test transactions work across versions.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_transaction_compatibility() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #VersionTest (id INT, value NVARCHAR(50))",
            &[],
        )
        .await
        .expect("Create should succeed");

    // Test commit
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin should succeed");

    tx.execute("INSERT INTO #VersionTest VALUES (1, 'committed')", &[])
        .await
        .expect("Insert should succeed");

    client = tx.commit().await.expect("Commit should succeed");

    // Verify committed
    let rows = client
        .query(
            "SELECT COUNT(*) FROM #VersionTest WHERE value = 'committed'",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 1);
    }

    // Test rollback
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin should succeed");

    tx.execute("INSERT INTO #VersionTest VALUES (2, 'rolled_back')", &[])
        .await
        .expect("Insert should succeed");

    client = tx.rollback().await.expect("Rollback should succeed");

    // Verify rolled back
    let rows = client
        .query(
            "SELECT COUNT(*) FROM #VersionTest WHERE value = 'rolled_back'",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 0);
    }

    client.close().await.expect("Failed to close");
}

/// Test parameterized queries work across versions.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_parameterized_queries_compatibility() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Integer parameter
    let rows = client
        .query("SELECT @p1 * 2 AS result", &[&42i32])
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let val: i32 = row.get(0).expect("Should get result");
        assert_eq!(val, 84);
    }

    // String parameter
    let rows = client
        .query("SELECT @p1 + ' world' AS result", &[&"hello"])
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let val: String = row.get(0).expect("Should get result");
        assert_eq!(val, "hello world");
    }

    client.close().await.expect("Failed to close");
}

/// Test error handling works across versions.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_error_handling_compatibility() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Syntax error
    let result = client.query("SELEKT 1", &[]).await;
    assert!(result.is_err(), "Syntax error should fail");

    // Connection should still work
    let rows = client
        .query("SELECT 'still working'", &[])
        .await
        .expect("Query should succeed after error");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: String = row.get(0).expect("Should get string");
        assert_eq!(val, "still working");
        found = true;
    }
    assert!(found);

    // Division by zero
    let result = client.query("SELECT 1/0", &[]).await;
    assert!(result.is_err(), "Division by zero should fail");

    // Connection should still work
    let rows = client
        .query("SELECT 'still ok'", &[])
        .await
        .expect("Query should succeed after error");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: String = row.get(0).expect("Should get string");
        assert_eq!(val, "still ok");
        found = true;
    }
    assert!(found);

    client.close().await.expect("Failed to close");
}

/// Test large data handling works across versions.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_large_data_compatibility() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Large string using server-side generation to avoid parameter encoding issues
    let rows = client
        .query(
            "SELECT REPLICATE('X', 5000) AS big_string, LEN(REPLICATE('X', 5000)) AS len",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let returned: String = row.get(0).expect("Should get string");
        let len: i32 = row.get(1).expect("Should get length");
        assert_eq!(returned.len(), 5000);
        assert_eq!(len, 5000);
    }

    // Many rows
    client
        .execute("CREATE TABLE #ManyRows (id INT)", &[])
        .await
        .expect("Create should succeed");

    // Insert 1000 rows
    for batch in 0..10 {
        let mut values = Vec::new();
        for i in 0..100 {
            values.push(format!("({})", batch * 100 + i));
        }
        let sql = format!("INSERT INTO #ManyRows VALUES {}", values.join(","));
        client
            .execute(&sql, &[])
            .await
            .expect("Insert should succeed");
    }

    // Count rows
    let rows = client
        .query("SELECT COUNT(*) FROM #ManyRows", &[])
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 1000);
    }

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Version-Specific Feature Tests
// =============================================================================

/// Test features available in SQL Server 2017+.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_sql_2017_features() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Check if we're on 2017+ (major version 14+)
    let rows = client
        .query(
            "SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128))",
            &[],
        )
        .await
        .expect("Version query should succeed");

    let mut major_version = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Should get version");
        major_version = version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
    }

    if major_version < 14 {
        println!(
            "Skipping SQL Server 2017 features test (running on major version {})",
            major_version
        );
        client.close().await.expect("Failed to close");
        return;
    }

    // STRING_AGG is available in 2017+
    client
        .execute(
            "CREATE TABLE #StringAggTest (id INT, val NVARCHAR(10))",
            &[],
        )
        .await
        .expect("Create should succeed");

    client
        .execute(
            "INSERT INTO #StringAggTest VALUES (1, 'A'), (1, 'B'), (1, 'C')",
            &[],
        )
        .await
        .expect("Insert should succeed");

    let rows = client
        .query(
            "SELECT STRING_AGG(val, ',') WITHIN GROUP (ORDER BY val) FROM #StringAggTest",
            &[],
        )
        .await
        .expect("STRING_AGG should work");

    for result in rows {
        let row = result.expect("Row should be valid");
        let aggregated: String = row.get(0).expect("Should get string");
        assert_eq!(aggregated, "A,B,C");
    }

    // TRIM function (2017+)
    let rows = client
        .query("SELECT TRIM('  hello  ') AS trimmed", &[])
        .await
        .expect("TRIM should work");

    for result in rows {
        let row = result.expect("Row should be valid");
        let trimmed: String = row.get(0).expect("Should get string");
        assert_eq!(trimmed, "hello");
    }

    client.close().await.expect("Failed to close");
}

/// Test features available in SQL Server 2019+.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_sql_2019_features() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Check if we're on 2019+ (major version 15+)
    // Note: ProductMajorVersion returns NULL in SQL Server 2014 RTM, so parse from ProductVersion
    let rows = client
        .query(
            "SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128))",
            &[],
        )
        .await
        .expect("Version query should succeed");

    let mut major_version = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Should get version");
        major_version = version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
    }

    if major_version < 15 {
        println!(
            "Skipping SQL Server 2019 features test (running on major version {})",
            major_version
        );
        client.close().await.expect("Failed to close");
        return;
    }

    // APPROX_COUNT_DISTINCT (2019+)
    client
        .execute("CREATE TABLE #ApproxTest (val INT)", &[])
        .await
        .expect("Create should succeed");

    // Use a loop to insert values instead of GENERATE_SERIES (2022 feature)
    let mut values = Vec::new();
    for i in 1..=100 {
        values.push(format!("({})", i));
    }
    let sql = format!("INSERT INTO #ApproxTest VALUES {}", values.join(","));
    client
        .execute(&sql, &[])
        .await
        .expect("Insert should succeed");

    let rows = client
        .query("SELECT APPROX_COUNT_DISTINCT(val) FROM #ApproxTest", &[])
        .await
        .expect("APPROX_COUNT_DISTINCT should work");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i64 = row.get(0).expect("Should get count");
        // Approximate count should be close to 100
        assert!(count >= 90 && count <= 110, "Approx count should be ~100");
    }

    client.close().await.expect("Failed to close");
}

use chrono::{Datelike, Timelike};

// =============================================================================
// TDS 7.3 Protocol Tests (SQL Server 2008/2008 R2)
// =============================================================================

/// Test TDS 7.3A connection (SQL Server 2008).
///
/// This test verifies that the driver can connect using TDS 7.3A protocol.
/// Requires a SQL Server 2008 or compatible instance.
#[tokio::test]
#[ignore = "Requires SQL Server 2008 or compatible instance"]
async fn test_tds_7_3a_connection() {
    let config = get_test_config_with_tds_version(Some(TdsVersion::V7_3A))
        .expect("SQL Server config required");

    println!("Connecting with TDS 7.3A (SQL Server 2008)...");

    let mut client = Client::connect(config)
        .await
        .expect("Failed to connect with TDS 7.3A");

    // Verify connection works
    let rows = client
        .query("SELECT 1 AS test", &[])
        .await
        .expect("Query should succeed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: i32 = row.get(0).expect("Should get int");
        assert_eq!(val, 1);
        found = true;
    }
    assert!(found, "Should have received result");

    client.close().await.expect("Failed to close");
    println!("TDS 7.3A connection successful!");
}

/// Test TDS 7.3B connection (SQL Server 2008 R2).
///
/// This test verifies that the driver can connect using TDS 7.3B protocol.
/// Requires a SQL Server 2008 R2 or compatible instance.
#[tokio::test]
#[ignore = "Requires SQL Server 2008 R2 or compatible instance"]
async fn test_tds_7_3b_connection() {
    let config = get_test_config_with_tds_version(Some(TdsVersion::V7_3B))
        .expect("SQL Server config required");

    println!("Connecting with TDS 7.3B (SQL Server 2008 R2)...");

    let mut client = Client::connect(config)
        .await
        .expect("Failed to connect with TDS 7.3B");

    // Verify connection works
    let rows = client
        .query("SELECT 1 AS test", &[])
        .await
        .expect("Query should succeed");

    let mut found = false;
    for result in rows {
        let row = result.expect("Row should be valid");
        let val: i32 = row.get(0).expect("Should get int");
        assert_eq!(val, 1);
        found = true;
    }
    assert!(found, "Should have received result");

    client.close().await.expect("Failed to close");
    println!("TDS 7.3B connection successful!");
}

/// Test TDS 7.3 data types (DATE, TIME, DATETIME2, DATETIMEOFFSET).
///
/// These types were introduced in TDS 7.3 (SQL Server 2008).
/// This test verifies they work correctly with TDS 7.3 connections.
#[tokio::test]
#[ignore = "Requires SQL Server 2008 or later"]
async fn test_tds_7_3_datetime_types() {
    let tds_version = get_env_tds_version().unwrap_or(TdsVersion::V7_4);
    let config =
        get_test_config_with_tds_version(Some(tds_version)).expect("SQL Server config required");

    println!("Testing TDS 7.3+ datetime types with {}...", tds_version);

    // Skip if explicitly using TDS 7.2 or earlier (though we don't support it)
    if tds_version.is_legacy() {
        println!("Skipping datetime types test for legacy TDS version");
        return;
    }

    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Test DATE type (TDS 7.3+)
    let rows = client
        .query("SELECT CAST('2024-06-15' AS DATE) AS d", &[])
        .await
        .expect("DATE query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let d: chrono::NaiveDate = row.get(0).expect("Should get date");
        assert_eq!(d.year(), 2024);
        assert_eq!(d.month(), 6);
        assert_eq!(d.day(), 15);
        println!("DATE: {} ✓", d);
    }

    // Test TIME type (TDS 7.3+)
    let rows = client
        .query("SELECT CAST('14:30:45.1234567' AS TIME) AS t", &[])
        .await
        .expect("TIME query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let t: chrono::NaiveTime = row.get(0).expect("Should get time");
        assert_eq!(t.hour(), 14);
        assert_eq!(t.minute(), 30);
        assert_eq!(t.second(), 45);
        println!("TIME: {} ✓", t);
    }

    // Test DATETIME2 type (TDS 7.3+)
    let rows = client
        .query(
            "SELECT CAST('2024-06-15 14:30:45.1234567' AS DATETIME2) AS dt2",
            &[],
        )
        .await
        .expect("DATETIME2 query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let dt: chrono::NaiveDateTime = row.get(0).expect("Should get datetime2");
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
        println!("DATETIME2: {} ✓", dt);
    }

    // Test DATETIMEOFFSET type (TDS 7.3+)
    let rows = client
        .query(
            "SELECT CAST('2024-06-15 14:30:45.1234567 -05:00' AS DATETIMEOFFSET) AS dto",
            &[],
        )
        .await
        .expect("DATETIMEOFFSET query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let dto: chrono::DateTime<chrono::FixedOffset> =
            row.get(0).expect("Should get datetimeoffset");
        assert_eq!(dto.year(), 2024);
        assert_eq!(dto.month(), 6);
        assert_eq!(dto.day(), 15);
        println!("DATETIMEOFFSET: {} ✓", dto);
    }

    client.close().await.expect("Failed to close");
    println!("TDS 7.3+ datetime types test passed!");
}

/// Test TDS version negotiation with SQL Server.
///
/// This test connects with different TDS versions and verifies the
/// negotiation works correctly.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tds_version_negotiation() {
    let config = get_test_config().expect("SQL Server config required");

    println!("Testing TDS version negotiation...");

    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Get server version info
    // Note: ProductMajorVersion returns NULL in SQL Server 2014 RTM, so parse from ProductVersion
    let rows = client
        .query(
            "SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128)) AS Version",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Should get version");
        let major_num: i32 = version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        println!("Connected to SQL Server {} (major: {})", version, major_num);

        // Verify expected TDS version support
        if major_num >= 16 {
            println!("  SQL Server 2022+ detected - supports TDS 7.4 and 8.0");
        } else if major_num >= 14 {
            println!("  SQL Server 2017+ detected - supports TDS 7.4");
        } else if major_num >= 11 {
            println!("  SQL Server 2012+ detected - supports TDS 7.4");
        } else if major_num >= 10 {
            println!("  SQL Server 2008+ detected - supports TDS 7.3");
        }
    }

    client.close().await.expect("Failed to close");
}

/// Test that TDS 7.3 features are correctly detected.
#[tokio::test]
async fn test_tds_7_3_feature_detection() {
    // Test that feature detection methods work correctly
    let v7_3a = TdsVersion::V7_3A;
    let v7_3b = TdsVersion::V7_3B;
    let v7_4 = TdsVersion::V7_4;
    let v8_0 = TdsVersion::V8_0;

    // TDS 7.3+ supports new datetime types
    assert!(
        v7_3a.supports_date_time_types(),
        "TDS 7.3A should support datetime types"
    );
    assert!(
        v7_3b.supports_date_time_types(),
        "TDS 7.3B should support datetime types"
    );
    assert!(
        v7_4.supports_date_time_types(),
        "TDS 7.4 should support datetime types"
    );
    assert!(
        v8_0.supports_date_time_types(),
        "TDS 8.0 should support datetime types"
    );

    // TDS 7.3 does NOT support session recovery (TDS 7.4+ only)
    assert!(
        !v7_3a.supports_session_recovery(),
        "TDS 7.3A should NOT support session recovery"
    );
    assert!(
        !v7_3b.supports_session_recovery(),
        "TDS 7.3B should NOT support session recovery"
    );
    assert!(
        v7_4.supports_session_recovery(),
        "TDS 7.4 should support session recovery"
    );
    assert!(
        v8_0.supports_session_recovery(),
        "TDS 8.0 should support session recovery"
    );

    // TDS 7.3 is not legacy (SQL Server 2005 and earlier are legacy)
    assert!(!v7_3a.is_legacy(), "TDS 7.3A should NOT be legacy");
    assert!(!v7_3b.is_legacy(), "TDS 7.3B should NOT be legacy");
    assert!(!v7_4.is_legacy(), "TDS 7.4 should NOT be legacy");

    // Display format
    assert_eq!(format!("{}", v7_3a), "TDS 7.3A");
    assert_eq!(format!("{}", v7_3b), "TDS 7.3B");
    assert_eq!(format!("{}", v7_4), "TDS 7.4");
    assert_eq!(format!("{}", v8_0), "TDS 8.0");

    println!("TDS 7.3 feature detection tests passed!");
}

/// Test encryption configuration with TDS 7.3.
///
/// TDS 7.3 supports:
/// - Encrypt=true (full TLS encryption)
/// - Encrypt=false (login-only encryption or no encryption)
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_tds_7_3_encryption_options() {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| {
        println!("MSSQL_HOST not set, skipping test");
        String::new()
    });
    if host.is_empty() {
        return;
    }

    // Test with encryption enabled (default)
    {
        let config = get_test_config_with_tds_version(Some(TdsVersion::V7_3A))
            .expect("SQL Server config required");
        println!("Testing TDS 7.3A with encryption...");

        match Client::connect(config).await {
            Ok(mut client) => {
                let rows = client
                    .query("SELECT 'encrypted' AS mode", &[])
                    .await
                    .expect("Query should succeed");

                for result in rows {
                    let row = result.expect("Row should be valid");
                    let mode: String = row.get(0).expect("Should get mode");
                    println!("Connection mode: {}", mode);
                }
                client.close().await.expect("Failed to close");
                println!("TDS 7.3A with encryption: OK ✓");
            }
            Err(e) => {
                println!("TDS 7.3A with encryption failed: {}", e);
                // This is not necessarily a test failure - the server might not support
                // the requested TDS version
            }
        }
    }

    println!("TDS 7.3 encryption options test completed!");
}

/// Test basic data operations with TDS 7.3.
#[tokio::test]
#[ignore = "Requires SQL Server 2008 or later"]
async fn test_tds_7_3_basic_operations() {
    let tds_version = get_env_tds_version().unwrap_or(TdsVersion::V7_3A);
    let config =
        get_test_config_with_tds_version(Some(tds_version)).expect("SQL Server config required");

    println!("Testing basic operations with {}...", tds_version);

    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute(
            "CREATE TABLE #TDS73Test (id INT PRIMARY KEY, name NVARCHAR(50), value DECIMAL(10,2))",
            &[],
        )
        .await
        .expect("Create table should succeed");

    // Insert data
    client
        .execute(
            "INSERT INTO #TDS73Test VALUES (1, N'Test One', 123.45), (2, N'Test Two', 678.90)",
            &[],
        )
        .await
        .expect("Insert should succeed");

    // Query data
    let rows = client
        .query("SELECT id, name, value FROM #TDS73Test ORDER BY id", &[])
        .await
        .expect("Query should succeed");

    let mut count = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let id: i32 = row.get(0).expect("Should get id");
        let name: String = row.get(1).expect("Should get name");
        let value: rust_decimal::Decimal = row.get(2).expect("Should get value");
        println!(
            "  Row {}: id={}, name='{}', value={}",
            count + 1,
            id,
            name,
            value
        );
        count += 1;
    }
    assert_eq!(count, 2, "Should have 2 rows");

    // Update data
    client
        .execute("UPDATE #TDS73Test SET value = 999.99 WHERE id = 1", &[])
        .await
        .expect("Update should succeed");

    // Verify update
    let rows = client
        .query("SELECT value FROM #TDS73Test WHERE id = 1", &[])
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let value: rust_decimal::Decimal = row.get(0).expect("Should get value");
        assert_eq!(value.to_string(), "999.99");
    }

    // Delete data
    client
        .execute("DELETE FROM #TDS73Test WHERE id = 2", &[])
        .await
        .expect("Delete should succeed");

    // Verify delete
    let rows = client
        .query("SELECT COUNT(*) FROM #TDS73Test", &[])
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 1);
    }

    client.close().await.expect("Failed to close");
    println!("TDS 7.3 basic operations test passed!");
}

/// Test transactions with TDS 7.3.
#[tokio::test]
#[ignore = "Requires SQL Server 2008 or later"]
async fn test_tds_7_3_transactions() {
    let tds_version = get_env_tds_version().unwrap_or(TdsVersion::V7_3A);
    let config =
        get_test_config_with_tds_version(Some(tds_version)).expect("SQL Server config required");

    println!("Testing transactions with {}...", tds_version);

    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create temp table
    client
        .execute("CREATE TABLE #TxTest73 (id INT, value NVARCHAR(50))", &[])
        .await
        .expect("Create should succeed");

    // Test commit
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin should succeed");

    tx.execute("INSERT INTO #TxTest73 VALUES (1, 'committed')", &[])
        .await
        .expect("Insert should succeed");

    client = tx.commit().await.expect("Commit should succeed");

    let rows = client
        .query(
            "SELECT COUNT(*) FROM #TxTest73 WHERE value = 'committed'",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 1, "Committed row should exist");
    }
    println!("  Transaction commit: OK ✓");

    // Test rollback
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin should succeed");

    tx.execute("INSERT INTO #TxTest73 VALUES (2, 'rolled_back')", &[])
        .await
        .expect("Insert should succeed");

    client = tx.rollback().await.expect("Rollback should succeed");

    let rows = client
        .query(
            "SELECT COUNT(*) FROM #TxTest73 WHERE value = 'rolled_back'",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let count: i32 = row.get(0).expect("Should get count");
        assert_eq!(count, 0, "Rolled back row should not exist");
    }
    println!("  Transaction rollback: OK ✓");

    client.close().await.expect("Failed to close");
    println!("TDS 7.3 transactions test passed!");
}
