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

/// Dropping the stream after reading only part of a large blob leaves the
/// connection in-flight; a directly reused client recovers it on the next
/// request rather than reading the abandoned blob's bytes.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_drop_mid_blob_then_reuse() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // A 2 MB blob — large enough that a few chunks leaves most of it on the wire.
    const SQL: &str = "SELECT 1 AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 2000000) AS VARBINARY(MAX)) AS doc";

    {
        let mut stream = client.query_stream_blob(SQL, &[]).await.expect("stream");
        let _ = stream.next().await.expect("next").expect("one row");
        // Read just one chunk, then drop the stream with the blob half-read.
        let chunk = stream.read_chunk().await.expect("chunk");
        assert!(chunk.is_some(), "expected at least one blob chunk");
    }

    // The next request must recover the abandoned response, not its bytes.
    let rows = client
        .query("SELECT 23 AS v", &[])
        .await
        .expect("reuse after mid-blob drop")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 23);
}

/// `query_stream_blob` works on a `Client<InTransaction>`.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn blob_stream_within_transaction() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let client = Client::connect(cfg).await.expect("connect");
    let mut tx = client.begin_transaction().await.expect("begin");

    const SQL: &str = "SELECT 1 AS id, \
        CAST(REPLICATE(CAST('B' AS VARCHAR(MAX)), 80000) AS VARBINARY(MAX)) AS doc";

    {
        let mut stream = tx.query_stream_blob(SQL, &[]).await.expect("stream in tx");
        let row = stream.next().await.expect("next").expect("one row");
        assert_eq!(row.get_by_name::<i32>("id").unwrap(), 1);
        let mut sink: Vec<u8> = Vec::new();
        let n = stream.copy_blob_to(&mut sink).await.expect("copy blob");
        assert_eq!(n, 80_000);
        assert!(sink.iter().all(|&b| b == 0x42), "all bytes must be 'B'");
        assert!(stream.next().await.expect("next").is_none());
    }

    let mut client = tx.commit().await.expect("commit");
    let rows = client
        .query("SELECT 29 AS v", &[])
        .await
        .expect("reuse after commit")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 29);
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

// ---------------------------------------------------------------------------
// query_stream_rows: multiple trailing MAX columns per row (#258)
// ---------------------------------------------------------------------------

/// Two trailing VARBINARY(MAX) columns, both streamed per row via `next_blob`.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn stream_rows_two_trailing_blobs() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // doc1 = 30000 bytes of 'A', doc2 = 50000 bytes of 'B'.
    const SQL: &str = "SELECT 7 AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 30000) AS VARBINARY(MAX)) AS doc1, \
        CAST(REPLICATE(CAST('B' AS VARCHAR(MAX)), 50000) AS VARBINARY(MAX)) AS doc2";

    let mut stream = client.query_stream_rows(SQL, &[]).await.expect("stream");
    // The trailing MAX columns are reported in wire order.
    let blob_names: Vec<String> = stream
        .blob_columns()
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(blob_names, vec!["doc1".to_string(), "doc2".to_string()]);

    let row = stream.next().await.expect("next").expect("one row");
    assert_eq!(row.get_by_name::<i32>("id").unwrap(), 7);

    let mut collected: Vec<(String, Vec<u8>)> = Vec::new();
    while stream.next_blob().await.expect("next_blob") {
        let name = stream
            .current_blob_column()
            .expect("blob column")
            .name
            .clone();
        let mut sink: Vec<u8> = Vec::new();
        stream.copy_blob_to(&mut sink).await.expect("copy blob");
        collected.push((name, sink));
    }

    assert_eq!(collected.len(), 2);
    assert_eq!(collected[0].0, "doc1");
    assert_eq!(collected[0].1.len(), 30_000);
    assert!(collected[0].1.iter().all(|&b| b == 0x41));
    assert_eq!(collected[1].0, "doc2");
    assert_eq!(collected[1].1.len(), 50_000);
    assert!(collected[1].1.iter().all(|&b| b == 0x42));

    assert!(stream.next().await.expect("next").is_none());
}

/// A NULL blob between two non-NULL blobs (NBCROW path): the null one yields no
/// chunks and does not desynchronize the following blob.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn stream_rows_null_blob_among_blobs() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    const SQL: &str = "SELECT 1 AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 20000) AS VARBINARY(MAX)) AS doc1, \
        CAST(NULL AS VARBINARY(MAX)) AS doc2, \
        CAST(REPLICATE(CAST('C' AS VARCHAR(MAX)), 20000) AS VARBINARY(MAX)) AS doc3";

    let mut stream = client.query_stream_rows(SQL, &[]).await.expect("stream");
    let _ = stream.next().await.expect("next").expect("one row");

    // doc1: non-null
    assert!(stream.next_blob().await.expect("next_blob"));
    assert!(!stream.blob_is_null());
    let mut b1 = Vec::new();
    while let Some(c) = stream.read_chunk().await.expect("chunk") {
        b1.extend_from_slice(&c);
    }
    assert_eq!(b1.len(), 20_000);
    assert!(b1.iter().all(|&b| b == 0x41));

    // doc2: NULL
    assert!(stream.next_blob().await.expect("next_blob"));
    assert!(stream.blob_is_null());
    assert!(stream.read_chunk().await.expect("chunk").is_none());

    // doc3: non-null, must be intact after the NULL blob
    assert!(stream.next_blob().await.expect("next_blob"));
    assert!(!stream.blob_is_null());
    let mut b3 = Vec::new();
    while let Some(c) = stream.read_chunk().await.expect("chunk") {
        b3.extend_from_slice(&c);
    }
    assert_eq!(b3.len(), 20_000);
    assert!(b3.iter().all(|&b| b == 0x43));

    // No more blobs, no more rows.
    assert!(!stream.next_blob().await.expect("next_blob"));
    assert!(stream.next().await.expect("next").is_none());
}

/// Advancing rows without reading the trailing blobs auto-drains all of them;
/// subsequent rows and the connection stay intact.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn stream_rows_auto_drain_unread_blobs() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    const SQL: &str = "SELECT n AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 40000) AS VARBINARY(MAX)) AS doc1, \
        CAST(REPLICATE(CAST('B' AS VARCHAR(MAX)), 40000) AS VARBINARY(MAX)) AS doc2 \
        FROM (VALUES (1), (2), (3)) v(n)";

    let mut stream = client.query_stream_rows(SQL, &[]).await.expect("stream");
    let mut ids = Vec::new();
    // Read no blobs at all — next() must drain both trailing blobs per row.
    while let Some(row) = stream.next().await.expect("next") {
        ids.push(row.get_by_name::<i32>("id").unwrap());
    }
    assert_eq!(ids, vec![1, 2, 3]);

    // Partially-read case: read only the first blob of a fresh stream, then advance.
    let mut stream = client.query_stream_rows(SQL, &[]).await.expect("stream");
    let _ = stream.next().await.expect("next").expect("row 1");
    assert!(stream.next_blob().await.expect("next_blob"));
    let _ = stream.read_chunk().await.expect("chunk"); // one chunk of doc1, leave doc2 untouched
    // Advancing must drain the rest of doc1 AND all of doc2.
    let mut rest = Vec::new();
    while let Some(row) = stream.next().await.expect("next") {
        rest.push(row.get_by_name::<i32>("id").unwrap());
    }
    assert_eq!(rest, vec![2, 3]);

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

/// `query_stream_rows` rejects an interleaved layout (a scalar column after a
/// MAX column).
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn stream_rows_rejects_scalar_after_blob() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");
    let result = client
        .query_stream_rows("SELECT CAST('x' AS VARCHAR(MAX)) AS doc, 1 AS id", &[])
        .await;
    assert!(
        matches!(result, Err(Error::Protocol(_))),
        "expected Protocol error for a scalar column after a MAX column"
    );
}

/// `query_stream_rows` works on a `Client<InTransaction>`.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn stream_rows_within_transaction() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let client = Client::connect(cfg).await.expect("connect");
    let mut tx = client.begin_transaction().await.expect("begin");

    const SQL: &str = "SELECT 1 AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 10000) AS VARBINARY(MAX)) AS doc1, \
        CAST(REPLICATE(CAST('B' AS VARCHAR(MAX)), 10000) AS VARBINARY(MAX)) AS doc2";

    {
        let mut stream = tx.query_stream_rows(SQL, &[]).await.expect("stream in tx");
        let _ = stream.next().await.expect("next").expect("one row");
        let mut total = 0u64;
        while stream.next_blob().await.expect("next_blob") {
            let mut sink: Vec<u8> = Vec::new();
            total += stream.copy_blob_to(&mut sink).await.expect("copy blob");
        }
        assert_eq!(total, 20_000);
        assert!(stream.next().await.expect("next").is_none());
    }

    let mut client = tx.commit().await.expect("commit");
    let rows = client
        .query("SELECT 29 AS v", &[])
        .await
        .expect("reuse after commit")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 29);
}
