//! # mssql-testing
//!
//! Test infrastructure for SQL Server driver development.
//!
//! This crate provides utilities for integration testing against
//! SQL Server instances, including testcontainers support and a mock TDS server.
//!
//! ## Features
//!
//! - SQL Server container management via testcontainers
//! - Mock TDS server for unit tests (no Docker required)
//! - Packet recording and replay for regression tests
//! - Test fixture utilities
//! - Connection helpers for tests
//!
//! ## Mock Server Example
//!
//! ```rust,ignore
//! use mssql_testing::mock_server::{MockTdsServer, MockResponse, MockColumn, ScalarValue};
//!
//! #[tokio::test]
//! async fn test_with_mock_server() {
//!     // Create a mock server with pre-configured responses
//!     let server = MockTdsServer::builder()
//!         .with_response(
//!             "SELECT * FROM users WHERE id = 1",
//!             MockResponse::rows(
//!                 vec![MockColumn::int("id"), MockColumn::nvarchar("name", 50)],
//!                 vec![vec![ScalarValue::Int(1), ScalarValue::String("Alice".into())]],
//!             ),
//!         )
//!         .build()
//!         .await
//!         .unwrap();
//!
//!     // Connect your client to server.addr()
//!     let addr = server.addr();
//!     // ...
//! }
//! ```
//!
//! ## Container Example
//!
//! ```rust,ignore
//! use mssql_testing::SqlServerContainer;
//! use testcontainers::clients::Cli;
//!
//! #[tokio::test]
//! async fn test_with_real_server() {
//!     let docker = Cli::default();
//!     let container = docker.run(SqlServerContainer::default());
//!     let port = container.get_host_port_ipv4(1433);
//!     // Connect to localhost:port...
//! }
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod container;
pub mod fixtures;
pub mod mock_server;

pub use container::SqlServerContainer;
pub use mock_server::{
    MockColumn, MockResponse, MockServerBuilder, MockServerConfig, MockServerError, MockTdsServer,
    PacketRecorder, RecordedPacket, ScalarValue,
};
