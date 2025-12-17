//! Connection lifecycle management.
//!
//! This module defines traits and types for managing connection lifecycle
//! in the pool, including health checks and connection reset operations.

use crate::error::PoolError;

/// Trait for connection lifecycle management.
///
/// This trait defines the operations needed to manage connection health
/// and state within the pool. Implementations handle SQL Server-specific
/// lifecycle requirements like `sp_reset_connection`.
///
/// # Native Async Traits
///
/// Per ARCHITECTURE.md §4.1, this uses native async traits (Rust 2024 Edition)
/// for zero overhead. For trait object usage, see `DynConnectionLifecycle`.
#[allow(async_fn_in_trait)]
pub trait ConnectionLifecycle: Send + Sync {
    /// Check if the connection is healthy.
    ///
    /// Typically executes `SELECT 1` to verify the connection is alive
    /// and responsive.
    async fn health_check(&self) -> Result<(), PoolError>;

    /// Reset connection state for pool return.
    ///
    /// Executes `sp_reset_connection` to clean up server-side state:
    /// - Closes open cursors
    /// - Releases temp tables
    /// - Resets SET options to defaults
    /// - Clears transaction state
    /// - Invalidates prepared statement handles
    async fn reset(&mut self) -> Result<(), PoolError>;

    /// Check if the connection is still valid for use.
    ///
    /// This is a lighter-weight check than `health_check`, typically
    /// just checking if the underlying TCP connection is still open.
    fn is_valid(&self) -> bool;
}

/// Async trait for connection lifecycle with trait object compatibility.
///
/// Use this when you need `dyn ConnectionLifecycle` (e.g., for dynamic dispatch).
/// Per ARCHITECTURE.md, `#[async_trait]` is required for object safety.
#[async_trait::async_trait]
pub trait DynConnectionLifecycle: Send + Sync {
    /// Check if the connection is healthy.
    async fn health_check(&self) -> Result<(), PoolError>;

    /// Reset connection state for pool return.
    async fn reset(&mut self) -> Result<(), PoolError>;

    /// Check if the connection is still valid for use.
    fn is_valid(&self) -> bool;
}

/// Health check result with timing information.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Whether the health check passed.
    pub healthy: bool,
    /// Time taken to complete the health check.
    pub latency: std::time::Duration,
    /// Error message if unhealthy.
    pub error: Option<String>,
}

impl HealthCheckResult {
    /// Create a successful health check result.
    pub fn healthy(latency: std::time::Duration) -> Self {
        Self {
            healthy: true,
            latency,
            error: None,
        }
    }

    /// Create a failed health check result.
    pub fn unhealthy(latency: std::time::Duration, error: impl Into<String>) -> Self {
        Self {
            healthy: false,
            latency,
            error: Some(error.into()),
        }
    }
}

/// Connection state tracked by the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is idle and available for use.
    Idle,
    /// Connection is currently in use.
    InUse,
    /// Connection is being health-checked.
    Checking,
    /// Connection is being reset.
    Resetting,
    /// Connection is being closed.
    Closing,
    /// Connection is closed and should be removed.
    Closed,
    /// Connection is in an error state.
    Error,
}

impl ConnectionState {
    /// Check if the connection is available for checkout.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if the connection is currently busy.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::InUse | Self::Checking | Self::Resetting)
    }

    /// Check if the connection should be removed from the pool.
    #[must_use]
    pub fn should_remove(&self) -> bool {
        matches!(self, Self::Closed | Self::Error)
    }
}

/// Metadata about a pooled connection.
#[derive(Debug, Clone)]
pub struct ConnectionMetadata {
    /// Unique identifier for this connection.
    pub id: u64,
    /// When the connection was created.
    pub created_at: std::time::Instant,
    /// When the connection was last used.
    pub last_used_at: std::time::Instant,
    /// When the connection was last health-checked.
    pub last_checked_at: Option<std::time::Instant>,
    /// Number of times the connection has been checked out.
    pub checkout_count: u64,
    /// Current state of the connection.
    pub state: ConnectionState,
}

impl ConnectionMetadata {
    /// Create metadata for a new connection.
    pub fn new(id: u64) -> Self {
        let now = std::time::Instant::now();
        Self {
            id,
            created_at: now,
            last_used_at: now,
            last_checked_at: None,
            checkout_count: 0,
            state: ConnectionState::Idle,
        }
    }

    /// Check if the connection has exceeded its maximum lifetime.
    #[must_use]
    pub fn is_expired(&self, max_lifetime: std::time::Duration) -> bool {
        self.created_at.elapsed() > max_lifetime
    }

    /// Check if the connection has been idle too long.
    #[must_use]
    pub fn is_idle_expired(&self, idle_timeout: std::time::Duration) -> bool {
        self.last_used_at.elapsed() > idle_timeout
    }

    /// Check if a health check is due.
    #[must_use]
    pub fn needs_health_check(&self, check_interval: std::time::Duration) -> bool {
        match self.last_checked_at {
            Some(last) => last.elapsed() > check_interval,
            None => true,
        }
    }

    /// Mark the connection as checked out.
    pub fn mark_checkout(&mut self) {
        self.last_used_at = std::time::Instant::now();
        self.checkout_count += 1;
        self.state = ConnectionState::InUse;
    }

    /// Mark the connection as returned to idle.
    pub fn mark_checkin(&mut self) {
        self.last_used_at = std::time::Instant::now();
        self.state = ConnectionState::Idle;
    }

    /// Mark the connection as health-checked.
    pub fn mark_health_check(&mut self) {
        self.last_checked_at = Some(std::time::Instant::now());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_connection_state_availability() {
        assert!(ConnectionState::Idle.is_available());
        assert!(!ConnectionState::InUse.is_available());
        assert!(!ConnectionState::Checking.is_available());
    }

    #[test]
    fn test_connection_state_busy() {
        assert!(!ConnectionState::Idle.is_busy());
        assert!(ConnectionState::InUse.is_busy());
        assert!(ConnectionState::Checking.is_busy());
        assert!(ConnectionState::Resetting.is_busy());
    }

    #[test]
    fn test_connection_state_should_remove() {
        assert!(!ConnectionState::Idle.should_remove());
        assert!(!ConnectionState::InUse.should_remove());
        assert!(ConnectionState::Closed.should_remove());
        assert!(ConnectionState::Error.should_remove());
    }

    #[test]
    fn test_connection_metadata_new() {
        let meta = ConnectionMetadata::new(1);
        assert_eq!(meta.id, 1);
        assert_eq!(meta.checkout_count, 0);
        assert_eq!(meta.state, ConnectionState::Idle);
    }

    #[test]
    fn test_connection_metadata_checkout() {
        let mut meta = ConnectionMetadata::new(1);
        meta.mark_checkout();

        assert_eq!(meta.checkout_count, 1);
        assert_eq!(meta.state, ConnectionState::InUse);
    }

    #[test]
    fn test_connection_metadata_checkin() {
        let mut meta = ConnectionMetadata::new(1);
        meta.mark_checkout();
        meta.mark_checkin();

        assert_eq!(meta.state, ConnectionState::Idle);
    }

    #[test]
    fn test_health_check_result_healthy() {
        let result = HealthCheckResult::healthy(Duration::from_millis(5));
        assert!(result.healthy);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_health_check_result_unhealthy() {
        let result = HealthCheckResult::unhealthy(Duration::from_millis(1000), "timeout");
        assert!(!result.healthy);
        assert_eq!(result.error.as_deref(), Some("timeout"));
    }
}
