//! Pool configuration.

use std::time::Duration;

/// Configuration for the connection pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain.
    pub min_connections: u32,

    /// Maximum number of connections allowed.
    pub max_connections: u32,

    /// Time to wait for a connection before timing out.
    pub connection_timeout: Duration,

    /// Time a connection can be idle before being closed.
    pub idle_timeout: Duration,

    /// Maximum lifetime of a connection.
    pub max_lifetime: Duration,

    /// Whether to test connections on checkout.
    pub test_on_checkout: bool,

    /// Whether to test connections on checkin.
    pub test_on_checkin: bool,

    /// Interval between health checks for idle connections.
    pub health_check_interval: Duration,

    /// Whether to reset connection state on return.
    pub reset_on_return: bool,

    /// Whether to run sp_reset_connection on return.
    pub sp_reset_connection: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
            test_on_checkout: true,
            test_on_checkin: false,
            health_check_interval: Duration::from_secs(30),
            reset_on_return: true,
            sp_reset_connection: true,
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the minimum number of connections.
    #[must_use]
    pub fn min_connections(mut self, count: u32) -> Self {
        self.min_connections = count;
        self
    }

    /// Set the maximum number of connections.
    #[must_use]
    pub fn max_connections(mut self, count: u32) -> Self {
        self.max_connections = count;
        self
    }

    /// Set the connection acquisition timeout.
    #[must_use]
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set the idle connection timeout.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set the maximum connection lifetime.
    #[must_use]
    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = lifetime;
        self
    }

    /// Enable or disable testing connections on checkout.
    #[must_use]
    pub fn test_on_checkout(mut self, enabled: bool) -> Self {
        self.test_on_checkout = enabled;
        self
    }

    /// Enable or disable testing connections on checkin.
    #[must_use]
    pub fn test_on_checkin(mut self, enabled: bool) -> Self {
        self.test_on_checkin = enabled;
        self
    }

    /// Set the health check interval.
    #[must_use]
    pub fn health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// Enable or disable sp_reset_connection on return.
    #[must_use]
    pub fn sp_reset_connection(mut self, enabled: bool) -> Self {
        self.sp_reset_connection = enabled;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), crate::error::PoolError> {
        if self.min_connections > self.max_connections {
            return Err(crate::error::PoolError::Configuration(
                "min_connections cannot be greater than max_connections".into(),
            ));
        }
        if self.max_connections == 0 {
            return Err(crate::error::PoolError::Configuration(
                "max_connections must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}
