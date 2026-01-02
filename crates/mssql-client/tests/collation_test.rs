//! Integration tests for collation-aware VARCHAR decoding.
//!
//! These tests verify that VARCHAR columns with various collations are
//! correctly decoded using the appropriate character encoding.

#![allow(clippy::expect_used)]

use mssql_client::{Client, Config, Error};

/// Helper to get test configuration from environment
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());

    let conn_str = format!(
        "Server={};Database=master;User Id={};Password={};TrustServerCertificate=true;Encrypt=true",
        host, user, password
    );

    Config::from_connection_string(&conn_str).ok()
}

/// Test Latin1 (Windows-1252) collation with extended ASCII characters
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_latin1_varchar_decoding() -> Result<(), Error> {
    let config = get_test_config().expect("Could not create config");
    let mut client = Client::connect(config).await?;

    // Create temp table with Latin1 collation
    client
        .execute(
            "CREATE TABLE #latin1_test (
                id INT,
                text_col VARCHAR(100) COLLATE SQL_Latin1_General_CP1_CI_AS
            )",
            &[],
        )
        .await?;

    // Insert text with extended ASCII (é, ü, ñ, etc.)
    client
        .execute(
            "INSERT INTO #latin1_test VALUES
                (1, 'Café'),
                (2, 'Müller'),
                (3, 'España'),
                (4, 'naïve')",
            &[],
        )
        .await?;

    // Query and verify
    let rows = client
        .query("SELECT id, text_col FROM #latin1_test ORDER BY id", &[])
        .await?;

    let mut results = Vec::new();
    for result in rows {
        let row = result?;

        // Debug: print column metadata
        let columns = row.columns();
        if results.is_empty() {
            println!("Column 1 (text_col):");
            println!("  Name: {}", columns[1].name);
            println!("  Type: {}", columns[1].type_name);
            println!("  Collation: {:?}", columns[1].collation);
            #[cfg(feature = "encoding")]
            println!("  Encoding: {}", columns[1].encoding_name());
        }

        let id: i32 = row.get(0)?;
        let text: String = row.get(1)?;
        results.push((id, text));
    }

    assert_eq!(results.len(), 4);
    assert_eq!(results[0], (1, "Café".to_string()));
    assert_eq!(results[1], (2, "Müller".to_string()));
    assert_eq!(results[2], (3, "España".to_string()));
    assert_eq!(results[3], (4, "naïve".to_string()));

    client.close().await?;
    Ok(())
}

/// Test UTF-8 collation (SQL Server 2019+)
#[tokio::test]
#[ignore = "Requires SQL Server 2019+"]
async fn test_utf8_varchar_decoding() -> Result<(), Error> {
    let config = get_test_config().expect("Could not create config");
    let mut client = Client::connect(config).await?;

    // Create temp table with UTF-8 collation
    client
        .execute(
            "CREATE TABLE #utf8_test (
                id INT,
                text_col VARCHAR(100) COLLATE Latin1_General_100_CI_AS_SC_UTF8
            )",
            &[],
        )
        .await?;

    // Insert multi-language text
    // Note: Use N'...' prefix for Unicode literals to ensure proper encoding
    client
        .execute(
            "INSERT INTO #utf8_test VALUES
                (1, N'Hello'),
                (2, N'Café résumé'),
                (3, N'日本語'),
                (4, N'中文'),
                (5, N'한국어'),
                (6, N'Привет')",
            &[],
        )
        .await?;

    // Query and verify
    let rows = client
        .query("SELECT id, text_col FROM #utf8_test ORDER BY id", &[])
        .await?;

    let mut results = Vec::new();
    for result in rows {
        let row = result?;
        let id: i32 = row.get(0)?;
        let text: String = row.get(1)?;
        results.push((id, text));
    }

    assert_eq!(results.len(), 6);
    assert_eq!(results[0], (1, "Hello".to_string()));
    assert_eq!(results[1], (2, "Café résumé".to_string()));
    assert_eq!(results[2], (3, "日本語".to_string()));
    assert_eq!(results[3], (4, "中文".to_string()));
    assert_eq!(results[4], (5, "한국어".to_string()));
    assert_eq!(results[5], (6, "Привет".to_string()));

    client.close().await?;
    Ok(())
}

/// Test column metadata includes collation information
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_column_collation_metadata() -> Result<(), Error> {
    let config = get_test_config().expect("Could not create config");
    let mut client = Client::connect(config).await?;

    let rows = client
        .query(
            "SELECT
                CAST('test' AS VARCHAR(50)) COLLATE SQL_Latin1_General_CP1_CI_AS as latin1_col,
                N'test' as nvarchar_col",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;

        // Check that VARCHAR column has collation metadata
        let columns = row.columns();
        assert_eq!(columns.len(), 2);

        // First column should be VARCHAR with collation
        let latin1_col = &columns[0];
        assert_eq!(latin1_col.name, "latin1_col");
        assert!(latin1_col.collation.is_some());

        #[cfg(feature = "encoding")]
        {
            // Should report encoding
            assert!(!latin1_col.encoding_name().is_empty());
            assert!(!latin1_col.is_utf8_collation());
        }
    }

    client.close().await?;
    Ok(())
}

/// Test NVARCHAR still works correctly (UTF-16, no collation decoding needed)
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_nvarchar_unicode() -> Result<(), Error> {
    let config = get_test_config().expect("Could not create config");
    let mut client = Client::connect(config).await?;

    let rows = client
        .query(
            "SELECT
                N'Hello, 世界!' as chinese,
                N'こんにちは' as japanese,
                N'안녕하세요' as korean,
                N'Привет мир' as russian",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;
        assert_eq!(row.get::<String>(0)?, "Hello, 世界!");
        assert_eq!(row.get::<String>(1)?, "こんにちは");
        assert_eq!(row.get::<String>(2)?, "안녕하세요");
        assert_eq!(row.get::<String>(3)?, "Привет мир");
    }

    client.close().await?;
    Ok(())
}
