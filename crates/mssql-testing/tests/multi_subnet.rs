//! Behavior tests for `MultiSubnetFailover` parallel TCP connect.
//!
//! The driver claims (README, CLAUDE.md) that `MultiSubnetFailover=true`
//! races parallel TCP connects to *all* resolved addresses and uses the
//! first to succeed. These tests prove the fan-out is real by binding mock
//! servers on both loopback stacks (`127.0.0.1` and `[::1]`) under the same
//! port and counting how many TCP connections each listener receives when
//! the client connects to `localhost`.
//!
//! These run in normal CI; no live SQL Server required. They self-skip when
//! `localhost` does not resolve dual-stack or IPv6 loopback is unavailable.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::time::Duration;

use mssql_client::{Client, Config};
use mssql_testing::mock_server::MockTdsServer;

/// Two mock servers on the same port: one on the IPv4 loopback, one on the
/// IPv6 loopback. `None` when the environment cannot support the scenario.
async fn dual_stack_mocks() -> Option<(MockTdsServer, MockTdsServer)> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host("localhost:1").await.ok()?.collect();
    let has_v4 = addrs.iter().any(SocketAddr::is_ipv4);
    let has_v6 = addrs.iter().any(SocketAddr::is_ipv6);
    if !(has_v4 && has_v6) {
        return None;
    }

    let v4 = MockTdsServer::builder().build().await.ok()?;
    let port = v4.port();
    let v6 = MockTdsServer::builder()
        .with_bind_addr(format!("[::1]:{port}"))
        .build()
        .await
        .ok()?;
    Some((v4, v6))
}

fn localhost_config(port: u16, multi_subnet: bool) -> Config {
    Config::from_connection_string(&format!(
        "Server=localhost,{port};User Id=sa;Password=test;Encrypt=no_tls;\
         ConnectRetryCount=0;MultiSubnetFailover={multi_subnet}"
    ))
    .expect("config parses")
}

/// `MultiSubnetFailover=true` must open a TCP connection to *every* resolved
/// address (the race), not walk them sequentially.
#[tokio::test]
async fn test_multi_subnet_failover_races_all_resolved_addresses() {
    let Some((v4, v6)) = dual_stack_mocks().await else {
        eprintln!("skipping: localhost is not dual-stack in this environment");
        return;
    };

    let client = Client::connect(localhost_config(v4.port(), true))
        .await
        .expect("parallel connect must succeed via the first winner");

    // Both listeners must have seen a connection attempt. The losing socket
    // is aborted by the client after the winner completes, but its TCP
    // handshake already reached the listener.
    let mut v4_total = 0;
    let mut v6_total = 0;
    for _ in 0..20 {
        v4_total = v4.total_connection_count().await;
        v6_total = v6.total_connection_count().await;
        if v4_total + v6_total >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        (v4_total, v6_total),
        (1, 1),
        "MultiSubnetFailover must race one TCP connect per resolved address"
    );

    let _ = client.close().await;
    v4.stop();
    v6.stop();
}

/// Without `MultiSubnetFailover`, the client connects to a single address —
/// no speculative connections to the other stack.
#[tokio::test]
async fn test_sequential_connect_uses_single_address() {
    let Some((v4, v6)) = dual_stack_mocks().await else {
        eprintln!("skipping: localhost is not dual-stack in this environment");
        return;
    };

    let client = Client::connect(localhost_config(v4.port(), false))
        .await
        .expect("sequential connect succeeds");

    // Give any stray speculative connections time to land before counting.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let total = v4.total_connection_count().await + v6.total_connection_count().await;
    assert_eq!(
        total, 1,
        "without MultiSubnetFailover exactly one address must be contacted"
    );

    let _ = client.close().await;
    v4.stop();
    v6.stop();
}
