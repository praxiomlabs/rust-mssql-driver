//! Pool error types.

use thiserror::Error;

/// Errors that can occur during pool operations.
#[derive(Debug, Error)]
pub enum PoolError {
    /// Failed to acquire a connection within the timeout.
    #[error("connection acquisition timeout after {0:?}")]
    AcquisitionTimeout(std::time::Duration),

    /// Pool is closed.
    #[error("pool is closed")]
    PoolClosed,

    /// Connection creation failed.
    #[error("failed to create connection: {0}")]
    ConnectionCreation(String),

    /// Connection is unhealthy.
    #[error("connection health check failed: {0}")]
    UnhealthyConnection(String),

    /// Connection reset failed.
    #[error("connection reset failed: {0}")]
    ResetFailed(String),

    /// Pool configuration error.
    #[error("pool configuration error: {0}")]
    Configuration(String),

    /// Maximum connections reached.
    #[error("maximum connections ({max}) reached")]
    MaxConnectionsReached {
        /// Maximum allowed connections.
        max: u32,
    },

    /// Connection validation failed.
    #[error("connection validation failed: {0}")]
    ValidationFailed(String),
}
