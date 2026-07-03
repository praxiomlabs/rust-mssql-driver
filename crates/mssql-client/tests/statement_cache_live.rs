//! Live integration tests for the opt-in prepared-statement cache (#205).
//!
//! Require a running SQL Server instance; ignored by default. Run with:
//!
//! ```bash
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!   cargo test -p mssql-client --test statement_cache_live -- --ignored
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config};

/// Base test configuration from environment variables (cache off).
fn base_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host},{port};Database={database};User Id={user};Password={password};TrustServerCertificate=true;Encrypt={encrypt}"
    );
    Config::from_connection_string(&conn_str).ok()
}

/// Read the single i32 column of a one-row result set.
async fn query_one_i32(client: &mut Client<mssql_client::Ready>, param: i32) -> i32 {
    let rows = client
        .query("SELECT @p1 AS value", &[&param])
        .await
        .expect("query failed");
    let mut value = None;
    for result in rows {
        let row = result.expect("row should be valid");
        value = Some(row.get::<i32>(0).expect("should get value"));
    }
    value.expect("expected one row")
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn statement_cache_prepares_once_then_reuses_handle() {
    let config = base_config()
        .expect("SQL Server config required")
        .with_statement_cache(true);
    let mut client = Client::connect(config).await.expect("connect");

    // First execution prepares; the next three reuse the cached handle. Correct
    // results across all four prove the sp_execute-with-cached-handle path.
    for v in [10, 20, 30, 40] {
        assert_eq!(query_one_i32(&mut client, v).await, v);
    }

    let stats = client.statement_cache_stats();
    assert_eq!(stats.misses, 1, "the statement is prepared exactly once");
    assert_eq!(stats.hits, 3, "the next three executions reuse the handle");
    assert_eq!(stats.entries, 1, "one distinct statement cached");

    client.close().await.expect("close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn statement_cache_keeps_distinct_statements_separate() {
    let config = base_config()
        .expect("SQL Server config required")
        .with_statement_cache(true);
    let mut client = Client::connect(config).await.expect("connect");

    assert_eq!(query_one_i32(&mut client, 7).await, 7);
    // A different SQL text must prepare its own handle, not reuse the first.
    let rows = client
        .query("SELECT @p1 + 1 AS value", &[&7i32])
        .await
        .expect("query failed");
    let mut value = None;
    for result in rows {
        value = Some(result.expect("row").get::<i32>(0).expect("value"));
    }
    assert_eq!(value, Some(8));

    let stats = client.statement_cache_stats();
    assert_eq!(stats.misses, 2, "two distinct statements prepared");
    assert_eq!(stats.entries, 2);

    client.close().await.expect("close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn statement_cache_off_never_consults_cache() {
    // Default config has the cache off: the path stays on sp_executesql and the
    // cache is never touched.
    let config = base_config().expect("SQL Server config required");
    assert!(!config.statement_cache, "cache is off by default");
    let mut client = Client::connect(config).await.expect("connect");

    for v in [1, 2, 3] {
        assert_eq!(query_one_i32(&mut client, v).await, v);
    }

    let stats = client.statement_cache_stats();
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.entries, 0);

    client.close().await.expect("close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn statement_cache_cleared_on_connection_reset() {
    let config = base_config()
        .expect("SQL Server config required")
        .with_statement_cache(true);
    let mut client = Client::connect(config).await.expect("connect");

    // Populate: one distinct statement, prepared once.
    assert_eq!(query_one_i32(&mut client, 5).await, 5);
    let stats = client.statement_cache_stats();
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits, 0);

    // Simulate a pool return: the next request carries RESETCONNECTION, which
    // invalidates every server-side prepared handle. The client MUST drop the
    // cache so it never sp_execute()s an invalidated handle.
    client.mark_needs_reset();

    // Same SQL again. If the cache was cleared it MISSES (re-prepares). If it
    // was NOT cleared this would be a HIT on a now-invalid handle. clear()
    // preserves the cumulative counters, so a cleared cache yields hits==0,
    // misses==2, entries==1.
    assert_eq!(query_one_i32(&mut client, 6).await, 6);
    let stats = client.statement_cache_stats();
    assert_eq!(
        stats.hits, 0,
        "cache must be cleared on reset — no reuse of an invalidated handle"
    );
    assert_eq!(
        stats.misses, 2,
        "the reset forces a re-prepare of the same statement"
    );
    assert_eq!(stats.entries, 1, "the re-prepared statement is re-cached");

    client.close().await.expect("close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn statement_cache_evicts_when_full_and_stays_usable() {
    let config = base_config()
        .expect("SQL Server config required")
        .with_statement_cache(true);
    let mut client = Client::connect(config).await.expect("connect");

    // Exceed the 256-entry default cap with distinct SQL texts. Each eviction
    // fires a best-effort sp_unprepare for the evicted handle; a broken
    // unprepare-on-eviction path would error here or corrupt the connection.
    for i in 0..300i32 {
        let sql = format!("SELECT @p1 + {i} AS value");
        let rows = client.query(&sql, &[&i]).await.expect("query failed");
        let mut got = None;
        for result in rows {
            got = Some(result.expect("row").get::<i32>(0).expect("value"));
        }
        assert_eq!(got, Some(i + i));
    }

    let stats = client.statement_cache_stats();
    assert_eq!(
        stats.entries, 256,
        "cache is capped at the default max; eviction occurred"
    );

    // The connection survived the evictions (each an sp_unprepare) and is still
    // usable for a fresh query.
    assert_eq!(query_one_i32(&mut client, 99).await, 99);

    client.close().await.expect("close");
}
