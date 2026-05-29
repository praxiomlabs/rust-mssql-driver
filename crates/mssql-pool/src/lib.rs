#![doc = include_str!("../README.md")]
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
