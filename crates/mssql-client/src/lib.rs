//! # mssql-client
//!
//! High-level async SQL Server client with type-state connection management.
//!
//! This is the primary public API surface for the rust-mssql-driver project.
//! It provides a type-safe, ergonomic interface for working with SQL Server
//! databases.
//!
//! ## Features
//!
//! - **Type-state pattern**: Compile-time enforcement of connection states
//! - **Async/await**: Built on Tokio for efficient async I/O
//! - **Prepared statements**: Automatic caching with LRU eviction
//! - **Transactions**: Full transaction support with savepoints
//! - **Azure support**: Automatic routing and failover handling
//! - **Streaming results**: Memory-efficient processing of large result sets
//!
//! ## Type-State Connection Management
//!
//! The client uses a compile-time type-state pattern that ensures invalid
//! operations are caught at compile time rather than runtime:
//!
//! ```text
//! Disconnected -> Ready (via connect())
//! Ready -> InTransaction (via begin_transaction())
//! Ready -> Streaming (via query that returns a stream)
//! InTransaction -> Ready (via commit() or rollback())
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_client::{Client, Config};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::from_connection_string(
//!         "Server=localhost;Database=test;User Id=sa;Password=Password123;"
//!     )?;
//!
//!     let mut client = Client::connect(config).await?;
//!
//!     // Execute a query with parameters
//!     let rows = client
//!         .query("SELECT * FROM users WHERE id = @p1", &[&1])
//!         .await?;
//!
//!     for row in rows {
//!         let name: String = row.get(0)?;
//!         println!("User: {}", name);
//!     }
//!
//!     // Transactions with savepoint support
//!     let mut tx = client.begin_transaction().await?;
//!     tx.execute("INSERT INTO users (name) VALUES (@p1)", &[&"Alice"]).await?;
//!
//!     // Create a savepoint for partial rollback
//!     let sp = tx.savepoint("before_update").await?;
//!     tx.execute("UPDATE users SET active = 1", &[]).await?;
//!
//!     // Rollback to savepoint if needed
//!     // tx.rollback_to(&sp).await?;
//!
//!     tx.commit().await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

// Module dependency graph (acyclic):
//
//   client ──→ config, state, error, stream, transaction, statement_cache
//     ├── connect.rs ──→ config, state, instrumentation, mssql_tls, mssql_codec, tds_protocol
//     ├── params.rs  ──→ mssql_types, tds_protocol
//     └── response.rs ──→ error, mssql_codec, tds_protocol
//   stream ──→ error, row
//   row ──→ blob, error, mssql_types
//   config ──→ mssql_auth, mssql_tls, tds_protocol
//   bulk ──→ error, mssql_types, tds_protocol
//   cancel ──→ error, mssql_codec, mssql_tls
//   encryption ──→ mssql_auth, tds_protocol
//   column_parser ──→ error, mssql_types, tds_protocol

pub mod blob;
pub mod bulk;
pub mod cancel;
pub mod change_tracking;
pub mod client;
pub(crate) mod column_parser;
pub mod config;
pub mod encryption;
pub mod error;
pub mod from_row;
pub mod instrumentation;
pub mod query;
pub mod row;
pub mod state;
pub mod statement_cache;
pub mod stream;
pub mod to_params;
pub mod transaction;
pub mod tvp;
pub(crate) mod validation;

// Re-export commonly used types
pub use bulk::{BulkColumn, BulkInsert, BulkInsertBuilder, BulkInsertResult, BulkOptions};
pub use cancel::CancelHandle;
pub use client::Client;
pub use config::{Config, RedirectConfig, RetryPolicy, TimeoutConfig};
pub use error::{Error, SharedIoError};

// Re-export TDS version for configuration
pub use from_row::{FromRow, MapRows, RowIteratorExt};
pub use mssql_auth::Credentials;
pub use tds_protocol::version::TdsVersion;

// Secure credential types (with zeroize feature)
#[cfg(feature = "zeroize")]
pub use mssql_auth::{SecretString, SecureCredentials};
pub use mssql_types::{FromSql, SqlValue, ToSql};
pub use query::Query;
pub use row::{Column, Row};
pub use state::{
    Connected, ConnectionState, Disconnected, InTransaction, ProtocolState, Ready, Streaming,
};
pub use statement_cache::{PreparedStatement, StatementCache, StatementCacheConfig};
pub use stream::{ExecuteResult, MultiResultStream, OutputParam, QueryStream, ResultSet};
pub use to_params::{NamedParam, ParamList, ToParams};
pub use transaction::{IsolationLevel, SavePoint, Transaction};
pub use tvp::{Tvp, TvpColumn, TvpRow, TvpValue};

// Always Encrypted types
#[cfg(feature = "always-encrypted")]
pub use encryption::EncryptionContext;
pub use encryption::{
    EncryptionConfig, ParameterCryptoInfo, ParameterEncryptionInfo, ResultSetEncryptionInfo,
};

// OpenTelemetry instrumentation (available whether or not otel feature is enabled)
pub use instrumentation::{
    DatabaseMetrics, OperationTimer, SanitizationConfig, attributes, metric_names, span_names,
};

// Change Tracking support
pub use change_tracking::{
    ChangeMetadata, ChangeOperation, ChangeTracking, ChangeTrackingQuery, SyncVersionStatus,
};

#[cfg(test)]
mod auto_trait_tests {
    //! Compile-time assertions that key async types are Send + Sync.
    //!
    //! These tests catch regressions where a type accidentally becomes
    //! !Send or !Sync due to interior changes (e.g., adding an Rc, Cell,
    //! or non-Send future). They cost nothing at runtime.

    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    // --- Type-state Client variants ---
    #[test]
    fn client_ready_is_send_sync() {
        assert_send::<Client<Ready>>();
        assert_sync::<Client<Ready>>();
    }

    #[test]
    fn client_in_transaction_is_send_sync() {
        assert_send::<Client<InTransaction>>();
        assert_sync::<Client<InTransaction>>();
    }

    #[test]
    fn client_disconnected_is_send_sync() {
        assert_send::<Client<Disconnected>>();
        assert_sync::<Client<Disconnected>>();
    }

    #[test]
    fn client_connected_is_send_sync() {
        assert_send::<Client<Connected>>();
        assert_sync::<Client<Connected>>();
    }

    #[test]
    fn client_streaming_is_send_sync() {
        assert_send::<Client<Streaming>>();
        assert_sync::<Client<Streaming>>();
    }

    // --- Configuration ---
    #[test]
    fn config_is_send_sync() {
        assert_send::<Config>();
        assert_sync::<Config>();
    }

    // --- Streaming types ---
    #[test]
    fn query_stream_is_send_sync() {
        assert_send::<QueryStream<'_>>();
        assert_sync::<QueryStream<'_>>();
    }

    #[test]
    fn multi_result_stream_is_send_sync() {
        assert_send::<MultiResultStream<'_>>();
        assert_sync::<MultiResultStream<'_>>();
    }

    #[test]
    fn result_set_is_send_sync() {
        assert_send::<ResultSet>();
        assert_sync::<ResultSet>();
    }

    #[test]
    fn execute_result_is_send_sync() {
        assert_send::<ExecuteResult>();
        assert_sync::<ExecuteResult>();
    }

    // --- Bulk insert types ---
    #[test]
    fn bulk_insert_is_send_sync() {
        assert_send::<BulkInsert>();
        assert_sync::<BulkInsert>();
    }

    #[test]
    fn bulk_insert_builder_is_send_sync() {
        assert_send::<BulkInsertBuilder>();
        assert_sync::<BulkInsertBuilder>();
    }

    #[test]
    fn bulk_options_is_send_sync() {
        assert_send::<BulkOptions>();
        assert_sync::<BulkOptions>();
    }

    // --- Cancel handle ---
    #[test]
    fn cancel_handle_is_send_sync() {
        assert_send::<CancelHandle>();
        assert_sync::<CancelHandle>();
    }

    // --- Row and column types ---
    #[test]
    fn row_is_send_sync() {
        assert_send::<Row>();
        assert_sync::<Row>();
    }

    #[test]
    fn column_is_send_sync() {
        assert_send::<Column>();
        assert_sync::<Column>();
    }

    // --- Statement cache ---
    #[test]
    fn statement_cache_is_send_sync() {
        assert_send::<StatementCache>();
        assert_sync::<StatementCache>();
    }

    // --- Error type ---
    #[test]
    fn error_is_send_sync() {
        assert_send::<Error>();
        assert_sync::<Error>();
    }
}
