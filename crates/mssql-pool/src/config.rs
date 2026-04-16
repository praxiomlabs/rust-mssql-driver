//! Pool configuration.

use std::sync::Arc;
use std::time::Duration;

/// Default health check query.
pub const DEFAULT_HEALTH_CHECK_QUERY: &str = "SELECT 1";

/// Configuration for the connection pool.
///
/// This struct is marked `#[non_exhaustive]` to allow adding new fields
/// in future minor versions without breaking changes. Use the builder
/// pattern methods or [`Default::default()`] to construct instances.
#[derive(Debug, Clone)]
#[non_exhaustive]
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

    /// Whether to health-check idle connections during reaper ticks.
    ///
    /// When enabled, the background reaper task will ping idle connections
    /// using the configured health check query. Connections that fail the
    /// check are discarded, catching network disconnects (e.g., firewall
    /// timeouts, Azure idle connection drops) before checkout.
    ///
    /// Each idle connection is checked at most once per `health_check_interval`.
    /// Connections are popped one at a time, checked, and returned to the pool,
    /// so checkout availability is minimally impacted during checking.
    pub test_while_idle: bool,

    /// Whether to execute sp_reset_connection on return.
    pub sp_reset_connection: bool,

    /// Custom health check query (defaults to "SELECT 1").
    ///
    /// This query is executed to verify a connection is healthy.
    /// The query should be lightweight and return quickly.
    ///
    /// # Examples
    ///
    /// - `SELECT 1` - Simple ping (default)
    /// - `SELECT @@VERSION` - Check server version
    /// - `SELECT GETDATE()` - Check server can execute functions
    /// - `SELECT 1 FROM sys.databases WHERE name = 'mydb'` - Check database exists
    pub health_check_query: Arc<str>,
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
            test_while_idle: false,
            sp_reset_connection: true,
            health_check_query: Arc::from(DEFAULT_HEALTH_CHECK_QUERY),
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

    /// Enable or disable health checking idle connections during reaper ticks.
    ///
    /// When enabled, the reaper task pings idle connections using the
    /// configured health check query, discarding any that fail. This
    /// catches stale connections (firewall timeouts, Azure idle drops)
    /// before checkout.
    #[must_use]
    pub fn test_while_idle(mut self, enabled: bool) -> Self {
        self.test_while_idle = enabled;
        self
    }

    /// Enable or disable sp_reset_connection on return.
    #[must_use]
    pub fn sp_reset_connection(mut self, enabled: bool) -> Self {
        self.sp_reset_connection = enabled;
        self
    }

    /// Set a custom health check query.
    ///
    /// The query is executed to verify a connection is healthy.
    /// It should be lightweight and return quickly.
    ///
    /// # Arguments
    ///
    /// * `query` - The SQL query to use for health checks
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mssql_driver_pool::PoolConfig;
    ///
    /// // Use a simple ping (default)
    /// let config = PoolConfig::new();
    ///
    /// // Check database exists
    /// let config = PoolConfig::new()
    ///     .health_check_query("SELECT 1 FROM sys.databases WHERE name = 'mydb'");
    ///
    /// // Check server can execute functions
    /// let config = PoolConfig::new()
    ///     .health_check_query("SELECT GETDATE()");
    /// ```
    #[must_use]
    pub fn health_check_query(mut self, query: impl Into<Arc<str>>) -> Self {
        self.health_check_query = query.into();
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), crate::error::PoolError> {
        if self.max_connections == 0 {
            return Err(crate::error::PoolError::Configuration(
                "max_connections must be greater than 0".into(),
            ));
        }
        if self.min_connections > self.max_connections {
            return Err(crate::error::PoolError::Configuration(
                "min_connections cannot be greater than max_connections".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PoolConfig::default();
        assert_eq!(config.min_connections, 1);
        assert_eq!(config.max_connections, 10);
        assert!(config.sp_reset_connection);
        assert!(config.test_on_checkout);
        assert!(!config.test_on_checkin);
        assert!(!config.test_while_idle);
        assert_eq!(&*config.health_check_query, DEFAULT_HEALTH_CHECK_QUERY);
    }

    #[test]
    fn test_config_builder_methods() {
        let config = PoolConfig::new()
            .min_connections(5)
            .max_connections(50)
            .connection_timeout(Duration::from_secs(60))
            .idle_timeout(Duration::from_secs(120))
            .max_lifetime(Duration::from_secs(3600))
            .test_on_checkout(false)
            .test_on_checkin(true)
            .sp_reset_connection(false);

        assert_eq!(config.min_connections, 5);
        assert_eq!(config.max_connections, 50);
        assert_eq!(config.connection_timeout, Duration::from_secs(60));
        assert_eq!(config.idle_timeout, Duration::from_secs(120));
        assert_eq!(config.max_lifetime, Duration::from_secs(3600));
        assert!(!config.test_on_checkout);
        assert!(config.test_on_checkin);
        assert!(!config.sp_reset_connection);
    }

    #[test]
    fn test_custom_health_check_query() {
        let custom_query = "SELECT 1 FROM sys.databases WHERE name = 'test'";
        let config = PoolConfig::new().health_check_query(custom_query);

        assert_eq!(&*config.health_check_query, custom_query);

        // Also test with String
        let config2 = PoolConfig::new().health_check_query(String::from("SELECT @@VERSION"));
        assert_eq!(&*config2.health_check_query, "SELECT @@VERSION");
    }

    #[test]
    fn test_config_validation_success() {
        let config = PoolConfig::new().min_connections(1).max_connections(10);

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_min_greater_than_max() {
        let config = PoolConfig::new().min_connections(20).max_connections(10);

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("min_connections cannot be greater than max_connections")
        );
    }

    #[test]
    fn test_config_validation_zero_max() {
        let mut config = PoolConfig::new();
        config.max_connections = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_connections must be greater than 0")
        );
    }

    #[test]
    fn test_config_equal_min_max() {
        let config = PoolConfig::new().min_connections(5).max_connections(5);

        assert!(config.validate().is_ok());
    }
}
