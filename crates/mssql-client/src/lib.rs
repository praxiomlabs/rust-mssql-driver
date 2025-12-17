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

pub mod blob;
pub mod bulk;
pub mod client;
pub mod config;
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

// Re-export commonly used types
pub use bulk::{BulkColumn, BulkInsert, BulkInsertBuilder, BulkInsertResult, BulkOptions};
pub use client::Client;
pub use config::{Config, RedirectConfig, RetryPolicy, TimeoutConfig};
pub use error::Error;
pub use from_row::{FromRow, MapRows, RowIteratorExt};
pub use mssql_auth::Credentials;
pub use mssql_types::{FromSql, SqlValue, ToSql};
pub use query::Query;
pub use row::{Column, Row};
pub use state::{Connected, ConnectionState, Disconnected, InTransaction, ProtocolState, Ready, Streaming};
pub use statement_cache::{PreparedStatement, StatementCache, StatementCacheConfig};
pub use stream::{ExecuteResult, MultiResultStream, OutputParam, QueryStream, ResultSet};
pub use to_params::{NamedParam, ParamList, ToParams};
pub use transaction::{IsolationLevel, SavePoint, Transaction};
pub use tvp::{Tvp, TvpColumn, TvpRow, TvpValue};
