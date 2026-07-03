//! Behavior tests for the connect-retry loop (ConnectRetryCount / Interval).
//!
//! The driver documents automatic retry of transient connection failures
//! (`RetryPolicy`, wired from `ConnectRetryCount`/`ConnectRetryInterval`).
//! These tests prove the reconnect loop against a mock TDS server that drops
//! the first connection (a transient failure) then serves the next normally,
//! so the whole path (transient classification -> backoff -> reconnect) runs
//! without a live SQL Server. The backoff math itself is unit-tested; this is
//! the missing end-to-end proof that the loop actually reconnects.
//!
//! These run in normal CI; no live SQL Server required.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config};
use mssql_testing::mock_server::MockTdsServer;

fn mock_config(port: u16, retry_count: u32) -> Config {
    // Plaintext-only mock (Encrypt=no_tls). ConnectRetryInterval=1 keeps the
    // backoff to ~1s so the test stays fast.
    Config::from_connection_string(&format!(
        "Server=127.0.0.1,{port};User Id=sa;Password=test;Encrypt=no_tls;\
         ConnectRetryCount={retry_count};ConnectRetryInterval=1"
    ))
    .expect("config parses")
}

/// With retries enabled, a transient connection failure (the server dropping
/// the first socket before any handshake) must be retried, and the reconnect
/// must succeed on the next attempt.
#[tokio::test]
async fn transient_failure_is_retried_and_reconnect_succeeds() {
    let server = MockTdsServer::builder()
        .fail_first_connections(1)
        .build()
        .await
        .expect("server starts");

    Client::connect(mock_config(server.port(), 1))
        .await
        .expect("connect must retry past the transient failure and succeed");

    // The server saw exactly two attempts: the dropped one plus the successful
    // reconnect.
    assert_eq!(
        server.total_connection_count().await,
        2,
        "one dropped attempt + one successful reconnect"
    );
}

/// Control: with retries disabled, the SAME transient failure must surface as
/// an error. This proves the retry loop — not something else — is what makes
/// the test above succeed (the green only means something because this goes
/// red).
#[tokio::test]
async fn no_retry_surfaces_the_transient_failure() {
    let server = MockTdsServer::builder()
        .fail_first_connections(1)
        .build()
        .await
        .expect("server starts");

    let result = Client::connect(mock_config(server.port(), 0)).await;

    assert!(
        result.is_err(),
        "with ConnectRetryCount=0 the transient failure must not be retried"
    );
    assert_eq!(
        server.total_connection_count().await,
        1,
        "exactly one attempt, no reconnect"
    );
}
