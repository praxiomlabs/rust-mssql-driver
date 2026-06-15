//! Live correctness tests for BLOB sub-streaming
//! ([`Client::query_stream_blob`]).
//!
//! ```text
//! MSSQL_HOST=localhost MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!   cargo nextest run -p mssql-client --test blob_stream --run-ignored ignored-only
//! ```

#![allow(clippy::expect_used, clippy::unwrap_used)]

use mssql_client::{Client, Config, Error};

fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT").unwrap_or_else(|_| "1433".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let conn_str = format!(
        "Server={host},{port};Database=master;User Id={user};Password={password};\
         TrustServerCertificate=true"
    );
    Config::from_connection_string(&conn_str).ok()
}

/// One row: scalar column + a 100 KB VARBINARY(MAX) blob streamed to a sink.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_single_varbinary_max() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // 100_000 bytes of 0x41 ('A') as VARBINARY(MAX).
    const SQL: &str = "SELECT 1 AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 100000) AS VARBINARY(MAX)) AS doc";

    let mut stream = client.query_stream_blob(SQL, &[]).await.expect("stream");
    let row = stream.next().await.expect("next").expect("one row");
    assert_eq!(row.get_by_name::<i32>("id").unwrap(), 1);

    let mut sink: Vec<u8> = Vec::new();
    let n = stream.copy_blob_to(&mut sink).await.expect("copy blob");
    assert_eq!(n, 100_000);
    assert_eq!(sink.len(), 100_000);
    assert!(sink.iter().all(|&b| b == 0x41), "all bytes must be 'A'");

    assert!(stream.next().await.expect("next").is_none());
}

/// Multiple rows, each with its own streamed blob, read via read_chunk.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_multiple_rows() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // Three rows; row n has a blob of n*10000 bytes of value n.
    const SQL: &str = "SELECT n AS id, \
        CAST(REPLICATE(CAST(CHAR(64 + n) AS VARCHAR(MAX)), n * 10000) AS VARBINARY(MAX)) AS doc \
        FROM (VALUES (1), (2), (3)) v(n)";

    let mut stream = client.query_stream_blob(SQL, &[]).await.expect("stream");
    let mut seen = Vec::new();
    while let Some(row) = stream.next().await.expect("next") {
        let id: i32 = row.get_by_name("id").unwrap();
        let mut buf = Vec::new();
        while let Some(chunk) = stream.read_chunk().await.expect("chunk") {
            buf.extend_from_slice(&chunk);
        }
        let expected_byte = 0x40 + id as u8; // 'A' for 1, 'B' for 2, ...
        assert_eq!(buf.len(), (id as usize) * 10000);
        assert!(buf.iter().all(|&b| b == expected_byte));
        seen.push(id);
    }
    assert_eq!(seen, vec![1, 2, 3]);
}

/// Advancing without reading a blob auto-drains it; the next row is intact.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_auto_drain_on_advance() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    const SQL: &str = "SELECT n AS id, \
        CAST(REPLICATE(CAST('Z' AS VARCHAR(MAX)), 50000) AS VARBINARY(MAX)) AS doc \
        FROM (VALUES (1), (2), (3)) v(n)";

    let mut stream = client.query_stream_blob(SQL, &[]).await.expect("stream");
    let mut ids = Vec::new();
    // Never read any blob — each next() must auto-drain the previous one.
    while let Some(row) = stream.next().await.expect("next") {
        ids.push(row.get_by_name::<i32>("id").unwrap());
    }
    assert_eq!(ids, vec![1, 2, 3]);

    // Connection must be clean afterwards.
    let rows = client
        .query("SELECT 99 AS v", &[])
        .await
        .expect("reuse")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 99);
}

/// A NULL blob (exercises the NBCROW path) yields no chunks.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_null_blob() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    const SQL: &str = "SELECT 1 AS id, CAST(NULL AS VARBINARY(MAX)) AS doc";

    let mut stream = client.query_stream_blob(SQL, &[]).await.expect("stream");
    let row = stream.next().await.expect("next").expect("one row");
    assert_eq!(row.get_by_name::<i32>("id").unwrap(), 1);
    assert!(stream.blob_is_null());
    assert!(stream.read_chunk().await.expect("chunk").is_none());
    assert!(stream.next().await.expect("next").is_none());
}

/// An NVARCHAR(MAX) text blob streams its UTF-16LE bytes.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_nvarchar_max() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    const SQL: &str = "SELECT 1 AS id, \
        REPLICATE(CAST(N'A' AS NVARCHAR(MAX)), 50000) AS doc";

    let mut stream = client.query_stream_blob(SQL, &[]).await.expect("stream");
    let _ = stream.next().await.expect("next").expect("one row");
    let mut buf = Vec::new();
    while let Some(chunk) = stream.read_chunk().await.expect("chunk") {
        buf.extend_from_slice(&chunk);
    }
    // 50_000 'A' chars in UTF-16LE = 100_000 bytes of (0x41, 0x00).
    assert_eq!(buf.len(), 100_000);
    assert!(buf.chunks(2).all(|c| c == [0x41, 0x00]));
}

/// Validation: a result set with no MAX column is rejected.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_rejects_no_max_column() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");
    let result = client
        .query_stream_blob("SELECT 1 AS id, 2 AS j", &[])
        .await;
    assert!(
        matches!(result, Err(Error::Protocol(_))),
        "expected Protocol error for no MAX column"
    );
}

/// Validation: the MAX column must be last.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_rejects_non_trailing_max() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");
    let result = client
        .query_stream_blob("SELECT CAST('x' AS VARCHAR(MAX)) AS doc, 1 AS id", &[])
        .await;
    assert!(
        matches!(result, Err(Error::Protocol(_))),
        "expected Protocol error for non-trailing MAX column"
    );
}
