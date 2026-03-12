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
//! - Configurable health checks (default: `SELECT 1`)
//! - Configurable min/max pool sizes
//! - Connection timeout, idle timeout, and max lifetime
//! - Background reaper task for expired connection cleanup
//! - Comprehensive metrics (wait queue depth, acquisition time, etc.)
//! - Per-connection prepared statement cache management
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_driver_pool::{Pool, PoolConfig};
//! use std::time::Duration;
//!
//! // Using the builder pattern
//! let pool = Pool::builder()
//!     .min_connections(5)
//!     .max_connections(20)
//!     .idle_timeout(Duration::from_secs(300))
//!     .sp_reset_connection(true)
//!     .build()
//!     .await?;
//!
//! // Or using PoolConfig directly with custom health check
//! let config = PoolConfig::new()
//!     .min_connections(5)
//!     .max_connections(20)
//!     .health_check_query("SELECT 1 FROM sys.databases WHERE name = 'mydb'");
//!
//! let pool = Pool::new(config).await?;
//!
//! // Get a connection from the pool
//! let conn = pool.get().await?;
//! // Use connection...
//! // Connection automatically returned to pool on drop
//!
//! // Check pool status (includes wait queue depth)
//! let status = pool.status();
//! println!("Pool utilization: {:.1}%", status.utilization());
//! println!("Wait queue depth: {}", status.wait_queue_depth);
//!
//! // Get metrics (includes acquisition time, expiration stats, etc.)
//! let metrics = pool.metrics();
//! println!("Checkout success rate: {:.2}", metrics.checkout_success_rate());
//! println!("Avg acquisition time: {:?}", metrics.avg_acquisition_time());
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod config;
pub mod error;
pub mod lifecycle;
pub mod pool;

// Configuration
pub use config::{DEFAULT_HEALTH_CHECK_QUERY, PoolConfig};

// Error types
pub use error::PoolError;

// Pool types
pub use pool::{Pool, PoolBuilder, PoolMetrics, PoolStatus, PooledConnection};

// Lifecycle management
pub use lifecycle::{
    ConnectionLifecycle, ConnectionMetadata, ConnectionState, DynConnectionLifecycle,
    HealthCheckResult,
};

#[cfg(test)]
mod auto_trait_tests {
    //! Compile-time assertions that key pool types are Send + Sync.
    //!
    //! These tests catch regressions where a type accidentally becomes
    //! !Send or !Sync due to interior changes. They cost nothing at runtime.

    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn pool_is_send_sync() {
        assert_send::<Pool>();
        assert_sync::<Pool>();
    }

    #[test]
    fn pooled_connection_is_send_sync() {
        assert_send::<PooledConnection>();
        assert_sync::<PooledConnection>();
    }

    #[test]
    fn pool_config_is_send_sync() {
        assert_send::<PoolConfig>();
        assert_sync::<PoolConfig>();
    }

    #[test]
    fn pool_builder_is_send_sync() {
        assert_send::<PoolBuilder>();
        assert_sync::<PoolBuilder>();
    }

    #[test]
    fn pool_status_is_send_sync() {
        assert_send::<PoolStatus>();
        assert_sync::<PoolStatus>();
    }

    #[test]
    fn pool_metrics_is_send_sync() {
        assert_send::<PoolMetrics>();
        assert_sync::<PoolMetrics>();
    }

    #[test]
    fn pool_error_is_send_sync() {
        assert_send::<PoolError>();
        assert_sync::<PoolError>();
    }
}
