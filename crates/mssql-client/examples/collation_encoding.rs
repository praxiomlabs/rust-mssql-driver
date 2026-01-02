//! Collation-aware VARCHAR encoding/decoding example.
//!
//! This example demonstrates how the driver handles VARCHAR columns with
//! locale-specific character encodings (collations) such as Japanese Shift_JIS,
//! Chinese GB18030/Big5, Korean EUC-KR, and various Windows code pages.
//!
//! # Background
//!
//! SQL Server VARCHAR columns store data in single-byte or multi-byte character
//! encodings determined by the column's collation. Unlike NVARCHAR (which is
//! always UTF-16), VARCHAR data must be decoded using the correct code page:
//!
//! | Collation | Code Page | Encoding |
//! |-----------|-----------|----------|
//! | Japanese_CI_AS | 932 | Shift_JIS |
//! | Chinese_PRC_CI_AS | 936 | GBK/GB18030 |
//! | Korean_Wansung_CI_AS | 949 | EUC-KR |
//! | Chinese_Taiwan_Stroke_CI_AS | 950 | Big5 |
//! | SQL_Latin1_General_CP1_CI_AS | 1252 | Windows-1252 |
//! | Latin1_General_100_CI_AS_SC_UTF8 | 65001 | UTF-8 |
//!
//! When the `encoding` feature is enabled (default), the driver automatically
//! decodes VARCHAR data using the column's collation information.
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
//! cargo run --example collation_encoding
//! ```
//!
//! # Feature Flag
//!
//! The collation-aware decoding requires the `encoding` feature:
//!
//! ```toml
//! [dependencies]
//! mssql-client = { version = "0.4", features = ["encoding"] }
//! ```
//!
//! Without this feature, the driver falls back to UTF-16LE decoding for
//! non-UTF-8 data, which may produce incorrect results for VARCHAR columns.

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config, Error, Ready};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing for logging (shows collation fallback warnings)
    tracing_subscriber::fmt::init();

    // Build configuration from environment
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "true".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, database, user, password, encrypt
    );

    let config = Config::from_connection_string(&conn_str)?;

    println!("Connecting to SQL Server at {}...", host);
    let mut client = Client::connect(config).await?;
    println!("Connected successfully!\n");

    // Demonstrate collation-aware decoding with different character sets
    demonstrate_collation_metadata(&mut client).await?;
    demonstrate_nvarchar_unicode(&mut client).await?;

    // Only run VARCHAR tests if the server supports the required collations
    // (Some SQL Server installations may not have all collations available)
    if let Err(e) = demonstrate_varchar_with_collations(&mut client).await {
        println!(
            "\nNote: VARCHAR collation tests skipped or partially failed: {}",
            e
        );
        println!(
            "This is expected if the SQL Server instance doesn't have the required collations."
        );
    }

    // Close the connection gracefully
    client.close().await?;
    println!("\nConnection closed.");

    Ok(())
}

/// Demonstrates how to inspect column collation metadata.
async fn demonstrate_collation_metadata(client: &mut Client<Ready>) -> Result<(), Error> {
    println!("=== Column Collation Metadata ===\n");

    // Query columns with different collations
    let rows = client
        .query(
            "SELECT
                CAST('hello' AS VARCHAR(50)) AS varchar_col,
                N'hello' AS nvarchar_col,
                CAST('test' AS CHAR(10)) AS char_col",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;

        // Access column metadata to see collation information
        println!("Column metadata:");
        for (i, col) in row.columns().iter().enumerate() {
            println!("  [{}] {} ({})", i, col.name, col.type_name,);

            // Show encoding information when available
            #[cfg(feature = "encoding")]
            {
                println!("       Encoding: {}", col.encoding_name());
                println!("       Is UTF-8: {}", col.is_utf8_collation());
            }
        }
        println!();
    }

    Ok(())
}

/// Demonstrates NVARCHAR (always UTF-16, no collation decoding needed).
async fn demonstrate_nvarchar_unicode(client: &mut Client<Ready>) -> Result<(), Error> {
    println!("=== NVARCHAR Unicode Handling ===\n");

    // NVARCHAR uses UTF-16LE encoding, so Unicode text works directly
    let rows = client
        .query(
            "SELECT
                N'Hello, 世界!' AS chinese,
                N'こんにちは' AS japanese,
                N'안녕하세요' AS korean,
                N'Привет мир' AS russian,
                N'مرحبا بالعالم' AS arabic",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;
        println!("NVARCHAR results (always Unicode):");
        println!("  Chinese:  {}", row.get::<String>(0)?);
        println!("  Japanese: {}", row.get::<String>(1)?);
        println!("  Korean:   {}", row.get::<String>(2)?);
        println!("  Russian:  {}", row.get::<String>(3)?);
        println!("  Arabic:   {}", row.get::<String>(4)?);
    }
    println!();

    Ok(())
}

/// Demonstrates VARCHAR with various collations.
///
/// This requires the SQL Server to have the appropriate collations installed.
async fn demonstrate_varchar_with_collations(client: &mut Client<Ready>) -> Result<(), Error> {
    println!("=== VARCHAR Collation-Aware Decoding ===\n");

    // Create a temporary table to test different collations
    client
        .execute(
            "IF OBJECT_ID('tempdb..#collation_test') IS NOT NULL
                DROP TABLE #collation_test",
            &[],
        )
        .await?;

    // Try to create table with various collations
    // Note: Not all collations may be available on all SQL Server installations
    let create_result = client
        .execute(
            "CREATE TABLE #collation_test (
                id INT IDENTITY(1,1),
                -- Latin1 (Western European)
                latin1_col VARCHAR(100) COLLATE SQL_Latin1_General_CP1_CI_AS,
                -- We use NVARCHAR for CJK to avoid collation availability issues
                unicode_col NVARCHAR(100)
            )",
            &[],
        )
        .await;

    if let Err(e) = create_result {
        println!("Could not create test table: {}", e);
        return Ok(());
    }

    // Insert test data
    client
        .execute(
            "INSERT INTO #collation_test (latin1_col, unicode_col) VALUES
                ('Hello World', N'Hello World'),
                ('Café résumé', N'Café résumé'),
                ('Müller Böse', N'Müller Böse')",
            &[],
        )
        .await?;

    // Query and display results
    let rows = client.query("SELECT * FROM #collation_test", &[]).await?;

    println!("Test data with Latin1 (Windows-1252) collation:");
    for result in rows {
        let row = result?;
        let id: i32 = row.get(0)?;
        let latin1: String = row.get(1)?;
        let unicode: String = row.get(2)?;

        println!("  Row {}: latin1='{}' unicode='{}'", id, latin1, unicode);

        // Verify that both columns decode correctly
        if latin1 == unicode {
            println!("         ✓ Encoding preserved correctly");
        } else {
            println!("         ⚠ Encoding mismatch (may indicate decoding issue)");
        }
    }
    println!();

    // Show how collation affects byte representation
    println!("Understanding VARCHAR encoding:");
    println!("  - VARCHAR stores data in the collation's code page");
    println!("  - Windows-1252 (Latin1): 'é' = 0xE9 (single byte)");
    println!("  - Shift_JIS (Japanese): '日' = 0x93FA (two bytes)");
    println!("  - UTF-16 (NVARCHAR): '日' = 0x65E5 (two bytes, different value)");
    println!();

    // Clean up
    client.execute("DROP TABLE #collation_test", &[]).await?;

    Ok(())
}
