//! SQL Server Version Compatibility Tests
//!
//! These tests verify the driver works correctly across different SQL Server versions.
//! The driver uses TDS 7.4 protocol which is compatible with SQL Server 2012+.
//!
//! Tested versions:
//! - SQL Server 2017 (TDS 7.4)
//! - SQL Server 2019 (TDS 7.4)
//! - SQL Server 2022 (TDS 8.0 with fallback to 7.4)
//!
//! Run with specific version:
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
//! To run Docker containers for all versions:
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

use mssql_client::{Client, Config};

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password =
        std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={},{};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, port, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str).ok()
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

    // Check for known version patterns
    let is_known_version = version_string.contains("2017")
        || version_string.contains("2019")
        || version_string.contains("2022");
    assert!(is_known_version, "Should be a known SQL Server version");

    client.close().await.expect("Failed to close");
}

/// Test product version number detection.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_product_version() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Use CAST to avoid SQL_VARIANT type issues
    let rows = client
        .query(
            "SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128)) AS Version, \
                    CAST(SERVERPROPERTY('ProductMajorVersion') AS NVARCHAR(10)) AS Major",
            &[],
        )
        .await
        .expect("Query should succeed");

    for result in rows {
        let row = result.expect("Row should be valid");
        let version: String = row.get(0).expect("Should get version");
        let major: String = row.get(1).expect("Should get major version");

        println!("Product Version: {}, Major: {}", version, major);

        // Major version should be 14 (2017), 15 (2019), or 16 (2022)
        let major_num: i32 = major.parse().expect("Major should be numeric");
        assert!(
            major_num >= 14,
            "Should be SQL Server 2017 or later (major >= 14)"
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
        .query("SELECT CAST('2024-06-15 14:30:00' AS DATETIME2) AS dt2", &[])
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
        .execute("CREATE TABLE #VersionTest (id INT, value NVARCHAR(50))", &[])
        .await
        .expect("Create should succeed");

    // Test commit
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin should succeed");

    tx.execute(
        "INSERT INTO #VersionTest VALUES (1, 'committed')",
        &[],
    )
    .await
    .expect("Insert should succeed");

    client = tx.commit().await.expect("Commit should succeed");

    // Verify committed
    let rows = client
        .query("SELECT COUNT(*) FROM #VersionTest WHERE value = 'committed'", &[])
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

    tx.execute(
        "INSERT INTO #VersionTest VALUES (2, 'rolled_back')",
        &[],
    )
    .await
    .expect("Insert should succeed");

    client = tx.rollback().await.expect("Rollback should succeed");

    // Verify rolled back
    let rows = client
        .query("SELECT COUNT(*) FROM #VersionTest WHERE value = 'rolled_back'", &[])
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
        client.execute(&sql, &[]).await.expect("Insert should succeed");
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

    // Check if we're on 2019+
    let rows = client
        .query(
            "SELECT CAST(SERVERPROPERTY('ProductMajorVersion') AS NVARCHAR(10))",
            &[],
        )
        .await
        .expect("Version query should succeed");

    let mut major_version = 0;
    for result in rows {
        let row = result.expect("Row should be valid");
        let major: String = row.get(0).expect("Should get version");
        major_version = major.parse().unwrap_or(0);
    }

    if major_version < 15 {
        println!("Skipping SQL Server 2019 features test (running on version {})", major_version);
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
    client.execute(&sql, &[]).await.expect("Insert should succeed");

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

use chrono::Datelike;
