//! Windows FILESTREAM integration tests.
//!
//! These tests require:
//! - Windows machine
//! - SQL Server with FILESTREAM enabled (access level 2)
//! - Windows Authentication (FILESTREAM requires it)
//! - Microsoft OLE DB Driver for SQL Server installed (msoledbsql.dll)
//! - The `FilestreamTest` database created with test fixtures
//!
//! ## SQL Server FILESTREAM setup
//!
//! 1. Enable FILESTREAM via SQL Server Configuration Manager
//!    (SQL Server Services → right-click instance → Properties → FILESTREAM tab)
//! 2. Run: `EXEC sp_configure 'filestream access level', 2; RECONFIGURE;`
//! 3. Create the test database:
//!
//! ```sql
//! CREATE DATABASE FilestreamTest
//! ON PRIMARY (
//!     NAME = FilestreamTest_data,
//!     FILENAME = 'C:\SQLData\FilestreamTest.mdf'  -- adjust path
//! ),
//! FILEGROUP FilestreamFG CONTAINS FILESTREAM (
//!     NAME = FilestreamTest_fs,
//!     FILENAME = 'C:\SQLData\FilestreamTest_fs'    -- adjust path
//! )
//! LOG ON (
//!     NAME = FilestreamTest_log,
//!     FILENAME = 'C:\SQLData\FilestreamTest_log.ldf'
//! );
//!
//! USE FilestreamTest;
//! CREATE TABLE dbo.Documents (
//!     Id UNIQUEIDENTIFIER ROWGUIDCOL NOT NULL DEFAULT NEWID() PRIMARY KEY,
//!     Name NVARCHAR(256),
//!     Content VARBINARY(MAX) FILESTREAM NULL
//! );
//!
//! INSERT INTO dbo.Documents (Name, Content)
//! VALUES ('test.txt', CAST('Hello FILESTREAM from Rust!' AS VARBINARY(MAX)));
//! ```
//!
//! ## Running
//!
//! ```bash
//! cargo test -p mssql-client --test windows_filestream --features "sspi-auth,filestream" -- --ignored
//! ```

#![cfg(all(windows, feature = "filestream", feature = "sspi-auth"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config, FileStreamAccess};
use tokio::io::AsyncReadExt;

fn get_filestream_config() -> Config {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);

    let conn_str = format!(
        "Server={host},{port};Database=FilestreamTest;Integrated Security=true;TrustServerCertificate=true"
    );

    Config::from_connection_string(&conn_str).expect("Failed to parse connection string")
}

#[tokio::test]
#[ignore = "Requires Windows with FILESTREAM-enabled SQL Server"]
async fn test_filestream_read() {
    let config = get_filestream_config();
    let client = Client::connect(config).await.expect("Connection failed");

    // Begin transaction (required for FILESTREAM)
    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin transaction failed");

    // Get the FILESTREAM path
    let rows = tx
        .query(
            "SELECT Content.PathName() FROM dbo.Documents WHERE Name = 'test.txt'",
            &[],
        )
        .await
        .expect("PathName query failed");

    let mut path: Option<String> = None;
    for result in rows {
        let row = result.expect("Row error");
        path = Some(row.get::<String>(0).expect("Get path failed"));
    }
    let path = path.expect("No FILESTREAM path returned — is the test data inserted?");

    assert!(
        path.starts_with("\\\\"),
        "FILESTREAM path should be a UNC path, got: {path}"
    );

    // Open the FILESTREAM BLOB for reading
    let mut stream = tx
        .open_filestream(&path, FileStreamAccess::Read)
        .await
        .expect("open_filestream failed");

    // Read the content
    let mut data = Vec::new();
    stream.read_to_end(&mut data).await.expect("Read failed");

    // Verify content matches what we inserted
    let content = String::from_utf8(data).expect("Content is not valid UTF-8");
    assert_eq!(content, "Hello FILESTREAM from Rust!", "Content mismatch");

    // Drop the stream before committing
    drop(stream);

    tx.rollback().await.expect("Rollback failed");
}

#[tokio::test]
#[ignore = "Requires Windows with FILESTREAM-enabled SQL Server"]
async fn test_filestream_write() {
    let config = get_filestream_config();
    let client = Client::connect(config).await.expect("Connection failed");

    let mut tx = client
        .begin_transaction()
        .await
        .expect("Begin transaction failed");

    // Insert a new row with NULL content (creates the FILESTREAM placeholder)
    tx.execute(
        "INSERT INTO dbo.Documents (Name, Content) VALUES ('write_test.txt', CAST('' AS VARBINARY(MAX)))",
        &[],
    )
    .await
    .expect("Insert failed");

    // Get the FILESTREAM path for the new row
    let rows = tx
        .query(
            "SELECT Content.PathName() FROM dbo.Documents WHERE Name = 'write_test.txt'",
            &[],
        )
        .await
        .expect("PathName query failed");

    let mut path: Option<String> = None;
    for result in rows {
        let row = result.expect("Row error");
        path = Some(row.get::<String>(0).expect("Get path failed"));
    }
    let path = path.expect("No FILESTREAM path returned");

    // Write data to the FILESTREAM BLOB
    {
        use tokio::io::AsyncWriteExt;
        let mut stream = tx
            .open_filestream(&path, FileStreamAccess::Write)
            .await
            .expect("open_filestream for write failed");

        stream
            .write_all(b"Written from Rust FILESTREAM!")
            .await
            .expect("Write failed");

        stream.shutdown().await.expect("Shutdown failed");
    }

    // Read it back to verify
    let rows = tx
        .query(
            "SELECT CAST(Content AS VARCHAR(MAX)) FROM dbo.Documents WHERE Name = 'write_test.txt'",
            &[],
        )
        .await
        .expect("Readback query failed");

    let mut content: Option<String> = None;
    for result in rows {
        let row = result.expect("Row error");
        content = Some(row.get::<String>(0).expect("Get content failed"));
    }
    assert_eq!(
        content.as_deref(),
        Some("Written from Rust FILESTREAM!"),
        "Written content doesn't match"
    );

    // Rollback so we don't pollute the test database
    tx.rollback().await.expect("Rollback failed");
}

#[tokio::test]
#[ignore = "Requires Windows with FILESTREAM-enabled SQL Server"]
async fn test_filestream_dll_loading() {
    // Verify the DLL loads successfully (doesn't need a database connection)
    // This test passes if msoledbsql.dll or sqlncli11.dll is installed
    let result = mssql_client::filestream::FileStream::open(
        "\\\\nonexistent\\path",
        FileStreamAccess::Read,
        &[0u8; 16], // dummy transaction context
    );

    // Should fail with an OpenSqlFilestream error (not a DLL loading error)
    let err = result.expect_err("Should fail with invalid path");
    let msg = format!("{err}");
    assert!(
        msg.contains("OpenSqlFilestream failed"),
        "Expected OpenSqlFilestream error, not DLL loading error. Got: {msg}"
    );
}
