//! # mssql-driver-pool
//!
//! Purpose-built connection pool for SQL Server with lifecycle management.
//!
//! Unlike generic connection pools, this implementation understands SQL Server
//! specifics like `sp_reset_connection` for proper connection state cleanup.
//!
//! ## Features
//!
//! - `sp_reset_connection` execution on connection return
//! - Health checks via `SELECT 1`
//! - Configurable min/max pool sizes
//! - Connection timeout and idle timeout
//! - Automatic reconnection on transient failures
//! - Per-connection prepared statement cache management
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_driver_pool::{Pool, PoolConfig};
//!
//! let config = PoolConfig::new()
//!     .min_connections(5)
//!     .max_connections(20)
//!     .idle_timeout(Duration::from_secs(300));
//!
//! let pool = Pool::new(connection_config, config).await?;
//! let conn = pool.get().await?;
//! // Use connection...
//! // Connection automatically returned to pool on drop
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod config;
pub mod error;
pub mod pool;

pub use config::PoolConfig;
pub use error::PoolError;
pub use pool::Pool;
