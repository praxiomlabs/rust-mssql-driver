//! Pool error types.

use thiserror::Error;

/// Errors that can occur during pool operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PoolError {
    /// Failed to acquire a connection within the timeout.
    #[error("connection acquisition timeout after {0:?}")]
    AcquisitionTimeout(std::time::Duration),

    /// Timeout waiting for a connection.
    ///
    /// Includes pool state at the time of timeout for diagnostics.
    #[error(
        "timeout waiting for connection (capacity: {capacity}, in_use: {in_use}, idle: {idle}, waiters: {waiters})"
    )]
    Timeout {
        /// Maximum pool capacity.
        capacity: u32,
        /// Number of connections currently checked out.
        in_use: u32,
        /// Number of idle connections available.
        idle: u32,
        /// Number of tasks waiting for a connection (including this one).
        waiters: u32,
    },

    /// Pool is closed.
    #[error("pool is closed")]
    PoolClosed,

    /// Connection error (preserves the underlying client error chain).
    #[error("connection error: {0}")]
    Connection(#[from] mssql_client::Error),

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

impl PoolError {
    /// Check if this error is transient and may succeed on retry.
    ///
    /// Timeouts, unhealthy connections, and transient connection errors
    /// may resolve on retry. Pool closure, configuration errors, and
    /// terminal connection errors are permanent.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::AcquisitionTimeout(_)
            | Self::Timeout { .. }
            | Self::UnhealthyConnection(_)
            | Self::ResetFailed(_)
            | Self::ValidationFailed(_)
            | Self::MaxConnectionsReached { .. } => true,
            Self::Connection(e) => e.is_transient(),
            Self::ConnectionCreation(_) => true, // creation failures are often transient
            _ => false,
        }
    }

    /// Check if this error is terminal and will never succeed on retry.
    ///
    /// Pool closure and configuration errors are always terminal.
    /// Connection errors delegate to the underlying client error.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::PoolClosed | Self::Configuration(_) => true,
            Self::Connection(e) => e.is_terminal(),
            _ => false,
        }
    }
}
