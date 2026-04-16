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
        "Server={host};Database=master;User Id={user};Password={password};TrustServerCertificate=true;Encrypt=true"
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

/// Verify VARCHAR RPC param round-trip when the server's default collation
/// is NOT the hardcoded Latin1_General_CI_AS fallback.
///
/// Regression pin for item 3.9: when `SendStringParametersAsUnicode=false` is
/// active, the driver must encode VARCHAR parameters via the collation captured
/// from the SqlCollation ENVCHANGE during login, not the hardcoded
/// Latin1_General_CI_AS / Windows-1252 default. If the fix regresses, Chinese
/// input chars get Windows-1252-encoded with '?' replacements and the readback
/// contains "????" instead of the original characters.
///
/// This test: (1) creates a fresh database with `COLLATE Chinese_PRC_CI_AS`
/// (LCID 0x0804 → GB18030 / CP936), (2) connects to it with
/// `SendStringParametersAsUnicode=false`, (3) sends a VARCHAR RPC param
/// containing simplified Chinese characters, (4) reads it back, (5) asserts
/// bit-exact round-trip. The setup/teardown uses the `sa` login and cleans up
/// whether or not the main assertions pass.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_varchar_param_chinese_prc_collation_round_trip() -> Result<(), Error> {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());

    // Unique DB name per run so parallel or interrupted runs don't collide.
    let db_name = format!(
        "mssql_driver_test_chinese_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    // Build a setup client on master so we can CREATE / DROP DATABASE.
    let setup_conn = format!(
        "Server={host};Database=master;User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt=true"
    );
    let setup_config = Config::from_connection_string(&setup_conn)?;

    // Create the DB with Chinese_PRC_CI_AS (LCID 0x0804 → GB18030 / CP936).
    {
        let mut setup = Client::connect(setup_config.clone()).await?;
        setup
            .execute(
                &format!("CREATE DATABASE {db_name} COLLATE Chinese_PRC_CI_AS"),
                &[],
            )
            .await?;
        setup.close().await?;
    }

    // Run the main scenario inside a closure so we always get to the cleanup
    // block, even on failure.
    let run = async {
        let conn = format!(
            "Server={host};Database={db_name};User Id={user};Password={password};\
             SendStringParametersAsUnicode=false;\
             TrustServerCertificate=true;Encrypt=true"
        );
        let config = Config::from_connection_string(&conn)?;
        assert!(
            !config.send_string_parameters_as_unicode,
            "SendStringParametersAsUnicode=false must parse correctly"
        );

        let mut client = Client::connect(config).await?;

        // VARCHAR column inherits the DB's default collation (Chinese_PRC_CI_AS).
        client
            .execute(
                "CREATE TABLE dbo.chinese_round_trip (id INT, txt VARCHAR(100))",
                &[],
            )
            .await?;

        // "Hello world" in Simplified Chinese — four CJK code points.
        // UTF-8: 12 bytes, GB18030: 8 bytes, Windows-1252 with '?' fallback: 4 bytes.
        let chinese = "你好世界";

        // Send via RPC parameter: under SendStringParametersAsUnicode=false the
        // driver must route through VARCHAR + the captured server collation.
        // If it regresses to the hardcoded Latin1_General_CI_AS default, these
        // characters get transcoded to '?' and the round-trip compares "????"
        // against "你好世界".
        client
            .execute(
                "INSERT INTO dbo.chinese_round_trip (id, txt) VALUES (@p1, @p2)",
                &[&1i32, &chinese],
            )
            .await?;

        // Read back via SELECT — the decode path uses the column collation, which
        // is the same Chinese_PRC_CI_AS we wrote with, so a clean round-trip.
        let rows = client
            .query(
                "SELECT txt, DATALENGTH(txt) FROM dbo.chinese_round_trip WHERE id = @p1",
                &[&1i32],
            )
            .await?;

        let mut iter = rows.into_iter();
        let row = iter.next().expect("expected one row")?;
        let txt: String = row.get(0)?;
        let byte_len: i32 = row.get(1)?;

        assert_eq!(
            txt, chinese,
            "VARCHAR param with non-Latin collation must round-trip verbatim; \
             got {txt:?} — if this is \"????\", the driver regressed to the \
             hardcoded Latin1 collation instead of using the captured server collation"
        );
        // GB18030 encodes each of these four code points as exactly 2 bytes.
        assert_eq!(
            byte_len, 8,
            "Chinese_PRC_CI_AS column should store {chinese:?} as 8 bytes (GB18030 / CP936)"
        );

        // Also exercise the query_named path — it shares `convert_named_params` /
        // `sql_value_to_rpc_param` with the positional path.
        use mssql_client::NamedParam;
        use mssql_types::SqlValue;
        let extra = "数据";
        client
            .execute_named(
                "INSERT INTO dbo.chinese_round_trip (id, txt) VALUES (@id, @txt)",
                &[
                    NamedParam::new("id", SqlValue::Int(2)),
                    NamedParam::new("txt", SqlValue::String(extra.into())),
                ],
            )
            .await?;

        let rows = client
            .query(
                "SELECT txt FROM dbo.chinese_round_trip WHERE id = 2",
                &[],
            )
            .await?;
        let mut iter = rows.into_iter();
        let row = iter.next().expect("expected one row from named insert")?;
        let got: String = row.get(0)?;
        assert_eq!(got, extra);

        client.close().await?;
        Ok::<_, Error>(())
    }
    .await;

    // Cleanup: always drop the test DB regardless of assertion outcome.
    {
        let mut cleanup = Client::connect(setup_config).await?;
        // Put DB in single-user to kick any lingering connections, then drop.
        let _ = cleanup
            .execute(
                &format!(
                    "IF DB_ID('{db_name}') IS NOT NULL BEGIN \
                        ALTER DATABASE {db_name} SET SINGLE_USER WITH ROLLBACK IMMEDIATE; \
                        DROP DATABASE {db_name}; \
                     END"
                ),
                &[],
            )
            .await;
        cleanup.close().await?;
    }

    run
}
