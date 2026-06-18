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
pub mod blob_stream;
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
pub(crate) mod plp;
pub mod procedure;
pub mod query;
pub mod row;
// Sans-IO incremental token decoder driving the streaming read path.
pub(crate) mod row_source;
pub mod row_stream;
pub mod state;
// Not yet wired into the query path (queries use sp_executesql); kept
// crate-private until the cache is actually used, so the public API does not
// expose types for an unshipped feature. Re-export when it lands.
pub(crate) mod statement_cache;
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
// Sub-error types carried by `Error` variants and the `FromSql`/`ToSql` trait
// return type. Re-exported so downstream crates can name them (e.g. match on
// `Error::Type(e)`, or write `fn from_sql(..) -> Result<Self, TypeError>`)
// without depending on the internal crates directly. `EncryptionError` is
// intentionally NOT here: `Error` stringifies it (see `error.rs`) so key
// material cannot leak, and it appears in no other public signature.
pub use mssql_auth::AuthError;
pub use mssql_codec::CodecError;
#[cfg(feature = "tls")]
pub use mssql_tls::TlsError;
pub use mssql_types::TypeError;
pub use tds_protocol::ProtocolError;

// TLS configuration: re-export so the `Config::tls` field is usable (custom
// root certificates, client auth) without a direct `mssql-tls` dependency.
// `CertificateDer` is needed to add a root certificate.
#[cfg(feature = "tls")]
pub use mssql_tls::{CertificateDer, TlsConfig};

// `KeyStoreProvider` extension trait: users implement it for custom Always
// Encrypted key stores (per the encryption-module docs) without a direct
// `mssql-auth` dependency.
#[cfg(feature = "always-encrypted")]
pub use mssql_auth::KeyStoreProvider;

// `Collation` appears on `Column::collation` and the `with_collation` builders
// (Column, BulkColumn); re-export so those are usable without a direct
// `tds-protocol` dependency.
pub use tds_protocol::token::Collation;

// Derive macros, re-exported under the `derive` feature so users need only a
// single `mssql-client` dependency (the macros' generated code resolves all
// its paths through `mssql_client`, including `__private` below). The macro
// names intentionally match the trait names — they live in the macro
// namespace, so `#[derive(FromRow)]` and `impl FromRow` coexist (as with
// serde's `Serialize`).
#[cfg(feature = "derive")]
pub use mssql_derive::{FromRow, ToParams, Tvp};

/// Items the derive macros' generated code references. Not public API: hidden
/// from docs and exempt from stability guarantees. Centralizing them here
/// keeps the proc-macro crate decoupled from internal restructuring.
#[doc(hidden)]
pub mod __private {
    pub use mssql_types::{ToSql, TypeError};
}

// Re-export TDS version for configuration
pub use from_row::{FromRow, MapRows, RowIteratorExt};
pub use mssql_auth::Credentials;
pub use tds_protocol::version::TdsVersion;

// Secure credential types (with zeroize feature)
#[cfg(feature = "zeroize")]
pub use mssql_auth::{SecretString, SecureCredentials};
pub use mssql_types::{
    Binary, Char, EncryptedParamType, FromSql, NChar, SqlTyped, SqlValue, ToSql, TypedNull, binary,
    char, nchar, null,
};
#[cfg(feature = "chrono")]
pub use mssql_types::{
    DateTime2, DateTimeLegacy, DateTimeOffset, SmallDateTime, Time, datetime, datetime2,
    datetimeoffset, time,
};
#[cfg(feature = "decimal")]
pub use mssql_types::{Money, Numeric, SmallMoney, numeric};
pub use procedure::ProcedureBuilder;
pub use query::in_params;
pub use row::{Column, Row};
pub use state::{Connected, ConnectionState, Disconnected, InTransaction, ProtocolState, Ready};

/// Internal entry points for the fuzzing harness in `fuzz/`.
///
/// Enabled only by the `fuzzing` feature; not public API and exempt from
/// all stability guarantees.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod __fuzzing {
    pub use crate::column_parser::parse_column_value;
}

/// Internal entry point for the allocation benchmark in `benches/`.
///
/// Enabled only by the `bench` feature; not public API and exempt from all
/// stability guarantees.
#[cfg(feature = "bench")]
#[doc(hidden)]
#[allow(clippy::expect_used)]
pub mod __bench {
    use bytes::Bytes;
    use tds_protocol::token::{Token, TokenParser};

    use crate::Ready;
    use crate::client::Client;
    use crate::row::Row;

    /// Decode a complete buffered query response (one ColMetaData followed by
    /// ROW tokens) into materialized `Row`s, with no async or socket IO.
    ///
    /// Mirrors the buffered read path over the same functions production uses:
    /// Stage A drives `TokenParser` (as in `read_query_response`) and Stage B
    /// runs `build_columns` + `convert_raw_row` (as in `QueryStream`). It
    /// exists only to give the allocation benchmark a deterministic, in-memory
    /// seam.
    #[must_use]
    pub fn decode_buffered_response(bytes: Bytes) -> Vec<Row> {
        let mut parser = TokenParser::new(bytes);
        let mut row_meta = std::sync::Arc::new(crate::row::ColMetaData::new(Vec::new()));
        let mut meta = None;
        let mut rows = Vec::new();
        while let Some(token) = parser
            .next_token_with_metadata(meta.as_ref())
            .expect("benchmark fixture must decode")
        {
            match token {
                Token::ColMetaData(m) => {
                    row_meta = std::sync::Arc::new(crate::row::ColMetaData::new(
                        Client::<Ready>::build_columns(&m),
                    ));
                    meta = Some(m);
                }
                Token::Row(raw) => {
                    let m = meta.as_ref().expect("ColMetaData precedes ROW tokens");
                    rows.push(
                        crate::column_parser::convert_raw_row(&raw, m, &row_meta)
                            .expect("benchmark row must convert"),
                    );
                }
                _ => {}
            }
        }
        rows
    }
}
pub use blob_stream::BlobStream;
pub use row_stream::RowStream;
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
pub use encryption::EncryptionConfig;

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
    fn row_stream_is_send_sync() {
        assert_send::<RowStream<'_>>();
        assert_sync::<RowStream<'_>>();
    }

    #[test]
    fn blob_stream_is_send_sync() {
        assert_send::<BlobStream<'_>>();
        assert_sync::<BlobStream<'_>>();
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

    // --- Statement cache (crate-private until wired) ---
    #[test]
    fn statement_cache_is_send_sync() {
        use crate::statement_cache::StatementCache;
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
