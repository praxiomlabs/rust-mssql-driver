//! Connection pool implementation.
//!
//! This module provides a purpose-built connection pool for SQL Server
//! with SQL Server-specific lifecycle management including `sp_reset_connection`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use mssql_client::{Client, Config as ClientConfig, Ready};
use parking_lot::Mutex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use crate::config::PoolConfig;
use crate::error::PoolError;
use crate::lifecycle::ConnectionMetadata;

/// A connection pool for SQL Server.
///
/// The pool manages a set of database connections, providing automatic
/// connection reuse, health checking, and lifecycle management.
///
/// # Features
///
/// - `sp_reset_connection` execution on connection return
/// - Health checks via `SELECT 1`
/// - Configurable min/max pool sizes
/// - Connection timeout and idle timeout
/// - Automatic reconnection on transient failures
///
/// # Example
///
/// ```rust,ignore
/// use mssql_driver_pool::{Pool, PoolConfig};
/// use mssql_client::Config;
///
/// let pool_config = PoolConfig::new()
///     .min_connections(5)
///     .max_connections(20);
///
/// let pool = Pool::builder()
///     .connection_config(client_config)
///     .pool_config(pool_config)
///     .build()
///     .await?;
///
/// let conn = pool.get().await?;
/// // Use connection...
/// ```
pub struct Pool {
    config: PoolConfig,
    client_config: ClientConfig,
    inner: Arc<PoolInner>,
}

/// A pooled connection entry.
struct PooledEntry {
    /// The actual client connection.
    client: Client<Ready>,
    /// Connection metadata.
    metadata: ConnectionMetadata,
}

struct PoolInner {
    /// Pool configuration.
    #[allow(dead_code)] // Used for idle timeout, health checks, etc.
    config: PoolConfig,

    /// Whether the pool is closed.
    closed: AtomicBool,

    /// Counter for generating connection IDs.
    next_connection_id: AtomicU64,

    /// When the pool was created.
    created_at: Instant,

    /// Pool metrics.
    metrics: Mutex<PoolMetricsInner>,

    /// Idle connections ready for use.
    idle_connections: Mutex<VecDeque<PooledEntry>>,

    /// Semaphore to limit total connections (wrapped in Arc for owned permits).
    semaphore: Arc<Semaphore>,

    /// Number of connections currently in use.
    in_use_count: AtomicU64,

    /// Total connections created.
    total_connections: AtomicU64,
}

/// Internal metrics tracking.
#[derive(Debug, Default)]
struct PoolMetricsInner {
    /// Total connections created.
    connections_created: u64,
    /// Total connections closed.
    connections_closed: u64,
    /// Total successful checkouts.
    checkouts_successful: u64,
    /// Total failed checkouts (timeouts, errors).
    checkouts_failed: u64,
    /// Total health checks performed.
    health_checks_performed: u64,
    /// Total health check failures.
    health_checks_failed: u64,
    /// Total resets performed.
    resets_performed: u64,
    /// Total reset failures.
    resets_failed: u64,
}

impl Pool {
    /// Create a new pool builder.
    ///
    /// Use the builder to configure the pool before creating it.
    #[must_use]
    pub fn builder() -> PoolBuilder {
        PoolBuilder::new()
    }

    /// Create a new pool with the given configuration and client configuration.
    ///
    /// For more control over pool creation, use [`Pool::builder()`].
    pub async fn new(config: PoolConfig, client_config: ClientConfig) -> Result<Self, PoolError> {
        config.validate()?;

        let inner = Arc::new(PoolInner {
            config: config.clone(),
            closed: AtomicBool::new(false),
            next_connection_id: AtomicU64::new(1),
            created_at: Instant::now(),
            metrics: Mutex::new(PoolMetricsInner::default()),
            idle_connections: Mutex::new(VecDeque::with_capacity(config.max_connections as usize)),
            semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
            in_use_count: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
        });

        tracing::info!(
            min = config.min_connections,
            max = config.max_connections,
            "connection pool created"
        );

        Ok(Self {
            config,
            client_config,
            inner,
        })
    }

    /// Get a connection from the pool.
    ///
    /// This will either return an existing idle connection or create a new one
    /// if the pool is not at capacity. If all connections are in use and the
    /// pool is at capacity, this will wait until a connection becomes available
    /// or the timeout is reached.
    pub async fn get(&self) -> Result<PooledConnection, PoolError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(PoolError::PoolClosed);
        }

        tracing::trace!("acquiring connection from pool");

        // Try to acquire semaphore permit with timeout
        let permit = match timeout(
            self.config.connection_timeout,
            Arc::clone(&self.inner.semaphore).acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                // Semaphore was closed (pool shut down)
                self.inner.metrics.lock().checkouts_failed += 1;
                return Err(PoolError::PoolClosed);
            }
            Err(_) => {
                // Timeout waiting for semaphore
                self.inner.metrics.lock().checkouts_failed += 1;
                return Err(PoolError::Timeout);
            }
        };

        // Try to get an idle connection first
        let entry = {
            let mut idle = self.inner.idle_connections.lock();
            idle.pop_front()
        };

        let (client, mut metadata) = match entry {
            Some(entry) => {
                tracing::trace!(connection_id = entry.metadata.id, "reusing idle connection");
                (entry.client, entry.metadata)
            }
            None => {
                // No idle connection, create a new one
                let id = self.next_connection_id();
                tracing::debug!(connection_id = id, "creating new connection");

                match Client::connect(self.client_config.clone()).await {
                    Ok(client) => {
                        self.inner.total_connections.fetch_add(1, Ordering::Relaxed);
                        self.inner.metrics.lock().connections_created += 1;
                        (client, ConnectionMetadata::new(id))
                    }
                    Err(e) => {
                        // Return the permit since we failed to create connection
                        drop(permit);
                        self.inner.metrics.lock().checkouts_failed += 1;
                        return Err(PoolError::Connection(e.to_string()));
                    }
                }
            }
        };

        // Mark as in use
        metadata.mark_checkout();
        self.inner.in_use_count.fetch_add(1, Ordering::Relaxed);
        self.inner.metrics.lock().checkouts_successful += 1;

        Ok(PooledConnection {
            client: Some(client),
            metadata,
            pool: self.inner.clone(),
            client_config: self.client_config.clone(),
            _permit: permit,
        })
    }

    /// Try to get a connection without waiting.
    ///
    /// Returns `None` if no connections are immediately available.
    /// This is non-blocking and will not create new connections.
    pub fn try_get(&self) -> Result<Option<PooledConnection>, PoolError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(PoolError::PoolClosed);
        }

        // Try to acquire a permit without waiting
        let permit = match self.inner.semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                // No permits available (pool at capacity with all connections in use)
                return Ok(None);
            }
        };

        // Try to get an idle connection (non-blocking)
        let entry = {
            let mut idle = self.inner.idle_connections.lock();
            idle.pop_front()
        };

        match entry {
            Some(entry) => {
                let mut metadata = entry.metadata;
                metadata.mark_checkout();
                self.inner.in_use_count.fetch_add(1, Ordering::Relaxed);
                self.inner.metrics.lock().checkouts_successful += 1;

                tracing::trace!(
                    connection_id = metadata.id,
                    "try_get: reusing idle connection"
                );

                Ok(Some(PooledConnection {
                    client: Some(entry.client),
                    metadata,
                    pool: self.inner.clone(),
                    client_config: self.client_config.clone(),
                    _permit: permit,
                }))
            }
            None => {
                // No idle connections available, return the permit
                // (don't create a new connection - that would block)
                drop(permit);
                Ok(None)
            }
        }
    }

    /// Get the current pool status.
    #[must_use]
    pub fn status(&self) -> PoolStatus {
        let idle = self.inner.idle_connections.lock().len() as u32;
        let in_use = self.inner.in_use_count.load(Ordering::Relaxed) as u32;
        PoolStatus {
            available: idle,
            in_use,
            total: idle + in_use,
            max: self.config.max_connections,
        }
    }

    /// Get pool metrics.
    #[must_use]
    pub fn metrics(&self) -> PoolMetrics {
        let inner = self.inner.metrics.lock();
        PoolMetrics {
            connections_created: inner.connections_created,
            connections_closed: inner.connections_closed,
            checkouts_successful: inner.checkouts_successful,
            checkouts_failed: inner.checkouts_failed,
            health_checks_performed: inner.health_checks_performed,
            health_checks_failed: inner.health_checks_failed,
            resets_performed: inner.resets_performed,
            resets_failed: inner.resets_failed,
            uptime: self.inner.created_at.elapsed(),
        }
    }

    /// Close the pool, dropping all connections.
    pub async fn close(&self) {
        self.inner.closed.store(true, Ordering::Release);
        tracing::info!("connection pool closed");
    }

    /// Check if the pool is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }

    /// Get the pool configuration.
    #[must_use]
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }

    /// Generate a new unique connection ID.
    #[allow(dead_code)] // Used when connection creation is implemented
    fn next_connection_id(&self) -> u64 {
        self.inner
            .next_connection_id
            .fetch_add(1, Ordering::Relaxed)
    }
}

/// Builder for creating a connection pool.
///
/// # Example
///
/// ```rust,ignore
/// let pool = Pool::builder()
///     .client_config(client_config)
///     .pool_config(pool_config)
///     .build()
///     .await?;
/// ```
pub struct PoolBuilder {
    pool_config: PoolConfig,
    client_config: Option<ClientConfig>,
}

impl PoolBuilder {
    /// Create a new pool builder with default settings.
    pub fn new() -> Self {
        Self {
            pool_config: PoolConfig::default(),
            client_config: None,
        }
    }

    /// Set the client configuration (required).
    #[must_use]
    pub fn client_config(mut self, config: ClientConfig) -> Self {
        self.client_config = Some(config);
        self
    }

    /// Set the pool configuration.
    #[must_use]
    pub fn pool_config(mut self, config: PoolConfig) -> Self {
        self.pool_config = config;
        self
    }

    /// Set the minimum number of connections.
    #[must_use]
    pub fn min_connections(mut self, count: u32) -> Self {
        self.pool_config.min_connections = count;
        self
    }

    /// Set the maximum number of connections.
    #[must_use]
    pub fn max_connections(mut self, count: u32) -> Self {
        self.pool_config.max_connections = count;
        self
    }

    /// Set the connection acquisition timeout.
    #[must_use]
    pub fn connection_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.pool_config.connection_timeout = timeout;
        self
    }

    /// Set the idle connection timeout.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.pool_config.idle_timeout = timeout;
        self
    }

    /// Enable or disable `sp_reset_connection` on return.
    #[must_use]
    pub fn sp_reset_connection(mut self, enabled: bool) -> Self {
        self.pool_config.sp_reset_connection = enabled;
        self
    }

    /// Build the pool.
    ///
    /// # Errors
    ///
    /// Returns an error if `client_config` was not set.
    pub async fn build(self) -> Result<Pool, PoolError> {
        let client_config = self
            .client_config
            .ok_or_else(|| PoolError::Configuration("client_config is required".to_string()))?;
        Pool::new(self.pool_config, client_config).await
    }
}

impl Default for PoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Status information about the pool.
#[derive(Debug, Clone, Copy)]
pub struct PoolStatus {
    /// Number of idle connections available.
    pub available: u32,
    /// Number of connections currently in use.
    pub in_use: u32,
    /// Total number of connections.
    pub total: u32,
    /// Maximum allowed connections.
    pub max: u32,
}

impl PoolStatus {
    /// Calculate the utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max == 0 {
            return 0.0;
        }
        (self.in_use as f64 / self.max as f64) * 100.0
    }

    /// Check if the pool is at capacity.
    #[must_use]
    pub fn is_at_capacity(&self) -> bool {
        self.total >= self.max
    }
}

/// Metrics collected from the pool.
#[derive(Debug, Clone)]
pub struct PoolMetrics {
    /// Total connections created since pool start.
    pub connections_created: u64,
    /// Total connections closed since pool start.
    pub connections_closed: u64,
    /// Successful connection checkouts.
    pub checkouts_successful: u64,
    /// Failed connection checkouts (timeouts, pool closed, etc.).
    pub checkouts_failed: u64,
    /// Health checks performed.
    pub health_checks_performed: u64,
    /// Health checks that failed.
    pub health_checks_failed: u64,
    /// Connection resets performed.
    pub resets_performed: u64,
    /// Connection resets that failed.
    pub resets_failed: u64,
    /// Time since pool creation.
    pub uptime: std::time::Duration,
}

impl PoolMetrics {
    /// Calculate checkout success rate (0.0 to 1.0).
    #[must_use]
    pub fn checkout_success_rate(&self) -> f64 {
        let total = self.checkouts_successful + self.checkouts_failed;
        if total == 0 {
            return 1.0;
        }
        self.checkouts_successful as f64 / total as f64
    }

    /// Calculate health check success rate (0.0 to 1.0).
    #[must_use]
    pub fn health_check_success_rate(&self) -> f64 {
        if self.health_checks_performed == 0 {
            return 1.0;
        }
        let successful = self.health_checks_performed - self.health_checks_failed;
        successful as f64 / self.health_checks_performed as f64
    }
}

/// A connection retrieved from the pool.
///
/// When dropped, the connection is automatically returned to the pool.
/// Use [`detach()`](PooledConnection::detach) to prevent automatic return.
pub struct PooledConnection {
    /// The actual client connection (Option to allow taking on drop).
    client: Option<Client<Ready>>,
    /// Connection metadata.
    metadata: ConnectionMetadata,
    /// Reference to the pool for returning the connection.
    pool: Arc<PoolInner>,
    /// Client config for reconnection if needed.
    #[allow(dead_code)] // Will be used for reconnection logic
    client_config: ClientConfig,
    /// Semaphore permit (released when connection returns to pool).
    _permit: OwnedSemaphorePermit,
}

impl PooledConnection {
    /// Get the connection metadata.
    #[must_use]
    pub fn metadata(&self) -> &ConnectionMetadata {
        &self.metadata
    }

    /// Get a reference to the underlying client.
    #[must_use]
    pub fn client(&self) -> Option<&Client<Ready>> {
        self.client.as_ref()
    }

    /// Get a mutable reference to the underlying client.
    #[must_use]
    pub fn client_mut(&mut self) -> Option<&mut Client<Ready>> {
        self.client.as_mut()
    }

    /// Detach the connection from the pool.
    ///
    /// Returns the underlying client. The connection will not be returned
    /// to the pool when this `PooledConnection` is dropped.
    pub fn detach(mut self) -> Option<Client<Ready>> {
        self.client.take()
    }

    /// Execute a query on this pooled connection.
    pub async fn query<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn mssql_client::ToSql + Sync)],
    ) -> Result<mssql_client::QueryStream<'a>, PoolError> {
        let client = self.client.as_mut().ok_or(PoolError::Connection(
            "connection detached or invalid".to_string(),
        ))?;
        client
            .query(sql, params)
            .await
            .map_err(|e| PoolError::Connection(e.to_string()))
    }

    /// Execute a statement on this pooled connection.
    pub async fn execute(
        &mut self,
        sql: &str,
        params: &[&(dyn mssql_client::ToSql + Sync)],
    ) -> Result<u64, PoolError> {
        let client = self.client.as_mut().ok_or(PoolError::Connection(
            "connection detached or invalid".to_string(),
        ))?;
        client
            .execute(sql, params)
            .await
            .map_err(|e| PoolError::Connection(e.to_string()))
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // Always decrement in_use_count since it was incremented during checkout.
        // This handles both normal returns and detached connections.
        self.pool.in_use_count.fetch_sub(1, Ordering::Relaxed);

        if let Some(client) = self.client.take() {
            // Check if connection is in a transaction started via raw SQL.
            // If so, we cannot safely return it to the pool because:
            // 1. Drop is sync, so we can't execute ROLLBACK
            // 2. The next user would get a connection mid-transaction
            // Instead, we discard the connection entirely.
            if client.is_in_transaction() {
                tracing::warn!(
                    connection_id = self.metadata.id,
                    "connection returned to pool with active transaction - discarding"
                );
                // Connection is dropped here, not returned to pool
                return;
            }

            tracing::trace!(
                connection_id = self.metadata.id,
                "returning connection to pool"
            );

            // Update metadata for checkin
            self.metadata.mark_checkin();

            // Return connection to idle queue
            let entry = PooledEntry {
                client,
                metadata: self.metadata.clone(),
            };

            self.pool.idle_connections.lock().push_back(entry);
        } else {
            tracing::trace!(
                connection_id = self.metadata.id,
                "connection detached, not returning to pool"
            );
        }
        // Note: the semaphore permit is automatically released when _permit is dropped
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_status_utilization() {
        let status = PoolStatus {
            available: 5,
            in_use: 5,
            total: 10,
            max: 20,
        };
        assert!((status.utilization() - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pool_status_at_capacity() {
        let status = PoolStatus {
            available: 0,
            in_use: 10,
            total: 10,
            max: 10,
        };
        assert!(status.is_at_capacity());

        let status2 = PoolStatus {
            available: 5,
            in_use: 5,
            total: 10,
            max: 20,
        };
        assert!(!status2.is_at_capacity());
    }

    #[test]
    fn test_pool_metrics_success_rates() {
        let metrics = PoolMetrics {
            connections_created: 10,
            connections_closed: 2,
            checkouts_successful: 90,
            checkouts_failed: 10,
            health_checks_performed: 100,
            health_checks_failed: 5,
            resets_performed: 80,
            resets_failed: 2,
            uptime: std::time::Duration::from_secs(3600),
        };

        assert!((metrics.checkout_success_rate() - 0.9).abs() < f64::EPSILON);
        assert!((metrics.health_check_success_rate() - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_default() {
        let builder = PoolBuilder::new();
        assert_eq!(builder.pool_config.min_connections, 1);
        assert_eq!(builder.pool_config.max_connections, 10);
    }

    #[test]
    fn test_builder_fluent() {
        let builder = Pool::builder()
            .min_connections(5)
            .max_connections(50)
            .sp_reset_connection(false);

        assert_eq!(builder.pool_config.min_connections, 5);
        assert_eq!(builder.pool_config.max_connections, 50);
        assert!(!builder.pool_config.sp_reset_connection);
    }
}
