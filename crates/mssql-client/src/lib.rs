#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

// Module dependency graph (acyclic):
//
//   client ──→ config, state, error, stream, transaction, statement_cache
//     ├── connect.rs ──→ config, state, instrumentation, mssql_tls, mssql_codec, tds_protocol
//     ├── params.rs  ──→ mssql_types, tds_protocol
//     └── response.rs ──→ error, mssql_codec, tds_protocol
//   procedure ──→ client, error, state, stream, tds_protocol
//   stream ──→ error, row
//   row ──→ blob, error, mssql_types
//   config ──→ mssql_auth, mssql_tls, tds_protocol
//   bulk ──→ error, mssql_types, tds_protocol
//   cancel ──→ error, mssql_codec, mssql_tls
//   encryption ──→ mssql_auth, tds_protocol
//   column_parser ──→ error, mssql_types, tds_protocol

pub mod blob;
pub(crate) mod browser;
pub mod bulk;
pub mod cancel;
pub mod change_tracking;
pub mod client;
#[cfg(feature = "always-encrypted")]
pub(crate) mod column_decryptor;
pub(crate) mod column_parser;
pub mod config;
pub mod encryption;
pub mod error;
#[cfg(all(windows, feature = "filestream"))]
#[allow(unsafe_code)] // Win32 FFI for OpenSqlFilestream; see SAFETY comments in each unsafe block
pub mod filestream;
pub mod from_row;
pub mod instrumentation;
pub mod procedure;
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
pub use bulk::{
    BulkColumn, BulkInsert, BulkInsertBuilder, BulkInsertResult, BulkOptions, BulkWriter,
};
pub use cancel::CancelHandle;
pub use client::Client;
pub use config::{ApplicationIntent, Config, RedirectConfig, RetryPolicy, TimeoutConfig};
pub use error::{Error, SharedIoError};

// Re-export TDS version for configuration
pub use from_row::{FromRow, MapRows, RowIteratorExt};
pub use mssql_auth::Credentials;
pub use tds_protocol::version::TdsVersion;

// Secure credential types (with zeroize feature)
#[cfg(feature = "zeroize")]
pub use mssql_auth::{SecretString, SecureCredentials};
#[cfg(feature = "chrono")]
pub use mssql_types::SmallDateTime;
pub use mssql_types::{FromSql, SqlValue, ToSql};
#[cfg(feature = "decimal")]
pub use mssql_types::{Money, SmallMoney};
pub use procedure::ProcedureBuilder;
pub use query::{Query, in_params};
pub use row::{Column, Row};
pub use state::{
    Connected, ConnectionState, Disconnected, InTransaction, ProtocolState, Ready, Streaming,
};
pub use statement_cache::{PreparedStatement, StatementCache, StatementCacheConfig};

/// Internal entry points for the fuzzing harness in `fuzz/`.
///
/// Enabled only by the `fuzzing` feature; not public API and exempt from
/// all stability guarantees.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod __fuzzing {
    pub use crate::column_parser::parse_column_value;
}
pub use stream::{
    ExecuteResult, MultiResultStream, OutputParam, ProcedureResult, QueryStream, ResultSet,
};
pub use to_params::{NamedParam, ParamList, ToParams};
pub use transaction::{IsolationLevel, SavePoint, Transaction};
pub use tvp::{Tvp, TvpColumn, TvpRow, TvpValue};

// FILESTREAM support (Windows only)
#[cfg(all(windows, feature = "filestream"))]
pub use filestream::{FileStream, FileStreamAccess, open_options as filestream_options};

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

    #[test]
    fn procedure_result_is_send_sync() {
        assert_send::<ProcedureResult>();
        assert_sync::<ProcedureResult>();
    }

    #[test]
    fn procedure_builder_is_send_sync() {
        assert_send::<ProcedureBuilder<'_, Ready>>();
        assert_sync::<ProcedureBuilder<'_, Ready>>();
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
