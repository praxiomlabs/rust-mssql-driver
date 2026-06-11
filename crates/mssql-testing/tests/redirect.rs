//! Behavior tests for ENVCHANGE Routing (Azure SQL Gateway redirect) handling.
//!
//! The driver documents automatic redirect following (`ARCHITECTURE.md` §
//! "Azure SQL Redirect Handling"). These tests prove the behavior against a
//! mock TDS server that answers LOGIN7 with a spec-faithful Routing token —
//! including the zero-length OldValue that real gateways send — so the whole
//! chain (token parse → `Error::Routing` → reconnect loop → redirect cap) is
//! exercised without an Azure subscription.
//!
//! These run in normal CI; no live SQL Server required.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config, Error};
use mssql_testing::mock_server::{MockResponse, MockTdsServer};

fn mock_config(port: u16) -> Config {
    // The mock is plaintext-only: NotSupported encryption keeps the entire
    // exchange (including login) off TLS. Retries are disabled so connection
    // counts below are deterministic.
    Config::from_connection_string(&format!(
        "Server=127.0.0.1,{port};User Id=sa;Password=test;Encrypt=no_tls;ConnectRetryCount=0"
    ))
    .expect("config parses")
}

/// A login-time routing token must be followed to the target server, and the
/// session must end up on the target.
#[tokio::test]
async fn test_login_redirect_followed_to_target() {
    let target = MockTdsServer::builder()
        .with_server_name("RedirectTarget")
        .with_response("SELECT 42", MockResponse::scalar_int(42))
        .build()
        .await
        .expect("target starts");

    let gateway = MockTdsServer::builder()
        .with_server_name("Gateway")
        .with_login_routing(target.host(), target.port())
        .build()
        .await
        .expect("gateway starts");

    let mut client = Client::connect(mock_config(gateway.port()))
        .await
        .expect("connect must follow the routing redirect to the target");

    // The query must be served by the target, not the gateway.
    let rows = client.query("SELECT 42", &[]).await.expect("query");
    let row = rows.into_iter().next().expect("row").expect("row ok");
    let value: i32 = row.get(0).expect("value");
    assert_eq!(value, 42);

    assert_eq!(
        gateway.total_connection_count().await,
        1,
        "gateway sees exactly the initial login attempt"
    );
    assert_eq!(
        target.total_connection_count().await,
        1,
        "target serves the redirected session"
    );

    let _ = client.close().await;
    gateway.stop();
    target.stop();
}

/// A routing loop must stop after `max_redirects` follows with
/// `Error::TooManyRedirects` — exactly 1 + max_redirects connection attempts.
#[tokio::test]
async fn test_redirect_loop_stops_at_max_redirects() {
    // Every login answers "go to me again".
    let looper = MockTdsServer::builder()
        .with_server_name("Looper")
        .with_login_routing_to_self()
        .build()
        .await
        .expect("looper starts");

    let err = Client::connect(mock_config(looper.port()))
        .await
        .expect_err("a self-routing server must not be followed forever");

    match err {
        Error::TooManyRedirects { max } => assert_eq!(max, 2),
        other => panic!("expected TooManyRedirects, got {other:?}"),
    }

    assert_eq!(
        looper.total_connection_count().await,
        3,
        "default max_redirects=2 allows the original attempt plus exactly \
         two followed redirects"
    );

    looper.stop();
}

/// With `follow_redirects = false` the routing token surfaces as
/// `Error::Routing` instead of being followed.
#[tokio::test]
async fn test_redirect_surfaced_when_following_disabled() {
    let gateway = MockTdsServer::builder()
        .with_login_routing("compute.example.invalid", 11001)
        .build()
        .await
        .expect("gateway starts");

    let mut config = mock_config(gateway.port());
    config.redirect.follow_redirects = false;

    let err = Client::connect(config)
        .await
        .expect_err("routing must surface as an error when following is off");

    match err {
        Error::Routing { host, port } => {
            assert_eq!(host, "compute.example.invalid");
            assert_eq!(port, 11001);
        }
        other => panic!("expected Error::Routing, got {other:?}"),
    }

    gateway.stop();
}
