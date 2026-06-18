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
