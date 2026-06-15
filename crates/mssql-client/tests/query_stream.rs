//! Live correctness tests for the incremental streaming path
//! ([`Client::query_stream`]).
//!
//! These assert that `query_stream` yields exactly the same rows as the
//! buffered `query`, in order, and that the edge cases (empty result, server
//! error, parameters, multi-statement batch) behave. Marked `#[ignore]` — they
//! need a live SQL Server.
//!
//! ```text
//! MSSQL_HOST=localhost MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!   cargo nextest run -p mssql-client --test query_stream --run-ignored ignored-only
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

/// `query_stream` yields the same rows, in the same order, as buffered `query`
/// — including a result set large enough to span many TDS packets.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn query_stream_matches_buffered_query() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // ~5000 rows of (int, varchar) — well beyond a single packet.
    const SQL: &str = "\
        WITH n AS (SELECT 1 AS i UNION ALL SELECT i + 1 FROM n WHERE i < 5000) \
        SELECT i, CAST(CONCAT('row-', i) AS VARCHAR(32)) AS label FROM n \
        OPTION (MAXRECURSION 0)";

    // Buffered reference.
    let buffered: Vec<(i32, String)> = client
        .query(SQL, &[])
        .await
        .expect("buffered query")
        .collect_all()
        .await
        .expect("collect")
        .into_iter()
        .map(|row| {
            (
                row.get_by_name::<i32>("i").unwrap(),
                row.get_by_name::<String>("label").unwrap(),
            )
        })
        .collect();

    // Streamed.
    let mut stream = client.query_stream(SQL, &[]).await.expect("stream query");
    let mut streamed: Vec<(i32, String)> = Vec::new();
    while let Some(row) = stream.try_next().await.expect("row") {
        streamed.push((
            row.get_by_name::<i32>("i").unwrap(),
            row.get_by_name::<String>("label").unwrap(),
        ));
    }

    assert_eq!(streamed.len(), 5000, "expected 5000 streamed rows");
    assert_eq!(streamed, buffered, "streamed rows must match buffered rows");
}

/// The connection is reusable after a stream is fully drained.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn connection_reusable_after_stream_drain() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    {
        let mut stream = client
            .query_stream("SELECT TOP 3 object_id FROM sys.objects", &[])
            .await
            .expect("stream");
        let mut n = 0;
        while stream.try_next().await.expect("row").is_some() {
            n += 1;
        }
        assert_eq!(n, 3);
    }

    // A second query on the same client must succeed.
    let row = client
        .query("SELECT 42 AS answer", &[])
        .await
        .expect("second query")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(row[0].get_by_name::<i32>("answer").unwrap(), 42);
}

/// A statement that produces no result set yields an empty stream.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn query_stream_empty_result() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    let mut stream = client
        .query_stream("SELECT 1 WHERE 1 = 0", &[])
        .await
        .expect("stream");
    assert!(stream.try_next().await.expect("row").is_none());
}

/// A server error in the stream surfaces as [`Error::Server`].
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn query_stream_surfaces_server_error() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // Either query_stream() itself errors (error before any metadata) or the
    // error surfaces on the first try_next — accept both.
    let result = client
        .query_stream("SELECT * FROM no_such_table_xyz", &[])
        .await;
    let err = match result {
        Err(e) => e,
        Ok(mut stream) => stream
            .try_next()
            .await
            .expect_err("expected a server error"),
    };
    assert!(
        matches!(err, Error::Server { .. }),
        "expected Error::Server, got {err:?}"
    );
}

/// A large result set whose response spans many packets — used so that
/// stopping early genuinely leaves unsent rows on the wire.
const BIG_QUERY: &str = "\
    WITH n AS (SELECT 1 AS i UNION ALL SELECT i + 1 FROM n WHERE i < 100000) \
    SELECT i FROM n OPTION (MAXRECURSION 0)";

/// `cancel()` mid-stream leaves the connection reusable for the next request.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn cancel_mid_stream_then_reuse() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    {
        let mut stream = client.query_stream(BIG_QUERY, &[]).await.expect("stream");
        // Pull a few rows, then abandon the rest (100k rows still pending).
        for _ in 0..5 {
            stream.try_next().await.expect("row").expect("a row");
        }
        stream.cancel().await.expect("cancel must succeed");
    }

    // The connection must be clean: a fresh query returns the right answer.
    let rows = client
        .query("SELECT 7 AS v", &[])
        .await
        .expect("reuse after cancel")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 7);
}

/// Dropping a stream mid-result leaves the connection in-flight; a directly
/// reused client recovers it (Attention/drain) on the next request.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn drop_mid_stream_then_reuse() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    {
        let mut stream = client.query_stream(BIG_QUERY, &[]).await.expect("stream");
        for _ in 0..5 {
            stream.try_next().await.expect("row").expect("a row");
        }
        // Drop without cancel/drain — 100k rows still pending on the wire.
    }

    // The next request must recover the abandoned response, not read its bytes.
    let rows = client
        .query("SELECT 11 AS v", &[])
        .await
        .expect("reuse after drop")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 11);
}

/// `cancel()` on a fully drained stream is a no-op.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn cancel_after_full_drain_is_noop() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    let mut stream = client
        .query_stream("SELECT TOP 2 object_id FROM sys.objects", &[])
        .await
        .expect("stream");
    while stream.try_next().await.expect("row").is_some() {}
    stream
        .cancel()
        .await
        .expect("cancel after drain is a no-op");

    let rows = client
        .query("SELECT 1 AS v", &[])
        .await
        .expect("reuse")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 1);
}

/// A server error raised *after* rows have already been yielded surfaces on
/// `try_next`, and the connection is left clean for the next request.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn error_mid_stream_then_reuse() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    // First statement yields a row; the second raises a mid-stream error.
    const SQL: &str = "SELECT 1 AS n; RAISERROR('boom', 16, 1);";

    {
        let mut stream = client.query_stream(SQL, &[]).await.expect("stream");
        // The first row comes through fine.
        let row = stream.try_next().await.expect("first row").expect("a row");
        assert_eq!(row.get_by_name::<i32>("n").unwrap(), 1);
        // A later pull hits the server error.
        let mut found: Option<Error> = None;
        loop {
            match stream.try_next().await {
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(e) => {
                    found = Some(e);
                    break;
                }
            }
        }
        let err = found.expect("expected a server error, got end of stream");
        assert!(
            matches!(err, Error::Server { .. }),
            "expected Error::Server, got {err:?}"
        );
    }

    // The connection must be reusable after surfacing the mid-stream error.
    let rows = client
        .query("SELECT 13 AS v", &[])
        .await
        .expect("reuse after mid-stream error")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 13);
}

/// `query_stream` works on a `Client<InTransaction>`: it reads uncommitted rows
/// written earlier in the same transaction, and the transaction can be
/// committed/rolled back once the stream is dropped.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn query_stream_within_transaction() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let client = Client::connect(cfg).await.expect("connect");

    let mut tx = client.begin_transaction().await.expect("begin");
    tx.execute("CREATE TABLE #stream_tx (n INT)", &[])
        .await
        .expect("create temp table");
    tx.execute("INSERT INTO #stream_tx VALUES (1), (2), (3)", &[])
        .await
        .expect("insert");

    let mut got: Vec<i32> = Vec::new();
    {
        // Stream the uncommitted rows from within the transaction.
        let mut stream = tx
            .query_stream("SELECT n FROM #stream_tx ORDER BY n", &[])
            .await
            .expect("stream in transaction");
        while let Some(row) = stream.try_next().await.expect("row") {
            got.push(row.get_by_name::<i32>("n").unwrap());
        }
    }
    assert_eq!(got, vec![1, 2, 3], "streamed the in-transaction rows");

    // The borrow has ended, so the transaction can now be rolled back.
    let mut client = tx.rollback().await.expect("rollback");
    let rows = client
        .query("SELECT 17 AS v", &[])
        .await
        .expect("reuse after rollback")
        .collect_all()
        .await
        .expect("collect");
    assert_eq!(rows[0].get_by_name::<i32>("v").unwrap(), 17);
}

/// Parameterized streaming works (sp_executesql path).
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn query_stream_with_parameters() {
    let Some(cfg) = get_test_config() else {
        return;
    };
    let mut client = Client::connect(cfg).await.expect("connect");

    let mut stream = client
        .query_stream("SELECT @p1 + @p2 AS total", &[&10i32, &32i32])
        .await
        .expect("stream");
    let row = stream.try_next().await.expect("row").expect("one row");
    assert_eq!(row.get_by_name::<i32>("total").unwrap(), 42);
    assert!(stream.try_next().await.expect("row").is_none());
}
