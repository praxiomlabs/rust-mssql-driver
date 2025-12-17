//! Mock TDS Server Fidelity Tests
//!
//! These tests validate that the mock TDS server is properly structured and
//! produces responses that should be compatible with TDS clients.
//!
//! NOTE: Full client connectivity tests are ignored because the mock server
//! doesn't implement TLS support. The client requires TLS negotiation even with
//! Encrypt=false in most scenarios. Future work should add mock TLS support.
//!
//! Run structural tests (no SQL Server required):
//! ```bash
//! cargo test -p mssql-testing --test mock_fidelity
//! ```
//!
//! Run comparison tests against real SQL Server:
//! ```bash
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!     cargo test -p mssql-testing --test mock_fidelity -- --ignored
//! ```

use mssql_testing::mock_server::{MockColumn, MockResponse, MockTdsServer, ScalarValue};
use tds_protocol::types::TypeId;

// =============================================================================
// Mock Server Structure Tests (no client connectivity required)
// =============================================================================

#[tokio::test]
async fn test_mock_server_starts_and_listens() {
    let server = MockTdsServer::builder()
        .with_server_name("FidelityTest")
        .with_database("testdb")
        .build()
        .await
        .expect("Server should start");

    assert!(server.port() > 0, "Should have valid port");
    assert_eq!(server.host(), "127.0.0.1", "Should listen on localhost");
    assert_eq!(server.connection_count().await, 0, "Should start with no connections");

    server.stop();
}

#[tokio::test]
async fn test_mock_server_builder_configuration() {
    let server = MockTdsServer::builder()
        .with_server_name("CustomServer")
        .with_database("customdb")
        .with_response("SELECT 1", MockResponse::scalar_int(1))
        .with_response("SELECT 2", MockResponse::scalar_int(2))
        .with_default_response(MockResponse::empty())
        .build()
        .await
        .expect("Server should start");

    assert!(server.port() > 0);
    server.stop();
}

#[tokio::test]
async fn test_mock_response_types() {
    // Test scalar int
    let response = MockResponse::scalar_int(42);
    match response {
        MockResponse::Scalar(ScalarValue::Int(v)) => assert_eq!(v, 42),
        _ => panic!("Expected scalar int"),
    }

    // Test scalar string
    let response = MockResponse::scalar_string("hello");
    match response {
        MockResponse::Scalar(ScalarValue::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("Expected scalar string"),
    }

    // Test rows affected
    let response = MockResponse::affected(5);
    match response {
        MockResponse::RowsAffected(n) => assert_eq!(n, 5),
        _ => panic!("Expected rows affected"),
    }

    // Test error
    let response = MockResponse::error(50000, "Test error");
    match response {
        MockResponse::Error { number, message, severity } => {
            assert_eq!(number, 50000);
            assert_eq!(message, "Test error");
            assert_eq!(severity, 16);
        }
        _ => panic!("Expected error"),
    }

    // Test empty
    let response = MockResponse::empty();
    match response {
        MockResponse::RowsAffected(0) => {}
        _ => panic!("Expected empty (rows affected 0)"),
    }
}

#[tokio::test]
async fn test_mock_column_constructors() {
    // Test int column
    let col = MockColumn::int("id");
    assert_eq!(col.name, "id");
    assert_eq!(col.type_id, TypeId::IntN);
    assert_eq!(col.max_length, Some(4));
    assert!(col.nullable);

    // Test bigint column
    let col = MockColumn::bigint("big_id");
    assert_eq!(col.name, "big_id");
    assert_eq!(col.type_id, TypeId::IntN);
    assert_eq!(col.max_length, Some(8));

    // Test nvarchar column
    let col = MockColumn::nvarchar("name", 50);
    assert_eq!(col.name, "name");
    assert_eq!(col.type_id, TypeId::NVarChar);
    assert_eq!(col.max_length, Some(100)); // 50 chars * 2 bytes

    // Test with_nullable
    let col = MockColumn::int("required").with_nullable(false);
    assert!(!col.nullable);
}

#[tokio::test]
async fn test_mock_rows_response() {
    let columns = vec![
        MockColumn::int("id"),
        MockColumn::nvarchar("name", 50),
    ];

    let rows = vec![
        vec![ScalarValue::Int(1), ScalarValue::String("Alice".into())],
        vec![ScalarValue::Int(2), ScalarValue::String("Bob".into())],
    ];

    let response = MockResponse::rows(columns.clone(), rows.clone());
    match response {
        MockResponse::Rows { columns: cols, rows: data } => {
            assert_eq!(cols.len(), 2);
            assert_eq!(data.len(), 2);
            assert_eq!(cols[0].name, "id");
            assert_eq!(cols[1].name, "name");
        }
        _ => panic!("Expected rows response"),
    }
}

#[tokio::test]
async fn test_scalar_value_types() {
    // Test all scalar value types
    let null = ScalarValue::Null;
    let bool_val = ScalarValue::Bool(true);
    let int_val = ScalarValue::Int(42);
    let bigint_val = ScalarValue::BigInt(9223372036854775807);
    let float_val = ScalarValue::Float(3.14);
    let double_val = ScalarValue::Double(2.71828);
    let string_val = ScalarValue::String("hello".into());
    let binary_val = ScalarValue::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]);

    // Verify types can be cloned and debug-printed
    let _ = format!("{:?}", null.clone());
    let _ = format!("{:?}", bool_val.clone());
    let _ = format!("{:?}", int_val.clone());
    let _ = format!("{:?}", bigint_val.clone());
    let _ = format!("{:?}", float_val.clone());
    let _ = format!("{:?}", double_val.clone());
    let _ = format!("{:?}", string_val.clone());
    let _ = format!("{:?}", binary_val.clone());
}

#[tokio::test]
async fn test_multiple_mock_servers() {
    // Can run multiple mock servers simultaneously
    let server1 = MockTdsServer::builder()
        .with_server_name("Server1")
        .build()
        .await
        .expect("Server 1 should start");

    let server2 = MockTdsServer::builder()
        .with_server_name("Server2")
        .build()
        .await
        .expect("Server 2 should start");

    assert_ne!(server1.port(), server2.port(), "Should use different ports");

    server1.stop();
    server2.stop();
}

#[tokio::test]
async fn test_mock_server_stop() {
    let server = MockTdsServer::builder()
        .build()
        .await
        .expect("Server should start");

    let port = server.port();
    assert!(port > 0);

    // Stopping should not panic
    server.stop();

    // Can call stop multiple times safely
    server.stop();
}

// =============================================================================
// The following tests require mock TLS support and are currently ignored
// =============================================================================

#[tokio::test]
#[ignore = "Mock server needs TLS support for client connectivity"]
async fn test_mock_server_basic_connection() {
    // This test would verify full client connectivity
    // Currently blocked by lack of TLS support in mock server
}

#[tokio::test]
#[ignore = "Mock server needs TLS support for client connectivity"]
async fn test_mock_server_query_response() {
    // This test would verify query execution
    // Currently blocked by lack of TLS support in mock server
}

// =============================================================================
// Comparison Tests Against Real SQL Server (require live SQL Server)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server for comparison"]
async fn test_compare_with_real_server() {
    // This test would compare mock and real SQL Server responses
    // Requires both mock TLS support and a running SQL Server
}
