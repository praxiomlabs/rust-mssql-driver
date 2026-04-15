//! Client configuration.
//!
//! Supporting types (`RedirectConfig`, `TimeoutConfig`, `RetryPolicy`) live in
//! the `types` submodule and are re-exported here for convenience.

mod types;
pub use types::*;

use std::time::Duration;

use mssql_auth::Credentials;
#[cfg(feature = "tls")]
use mssql_tls::TlsConfig;
use tds_protocol::version::TdsVersion;

/// Configuration for connecting to SQL Server.
///
/// This struct is marked `#[non_exhaustive]` to allow adding new fields
/// in future releases without breaking semver. Use [`Config::default()`]
/// or [`Config::from_connection_string()`] to construct instances.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Config {
    /// Server hostname or IP address.
    pub host: String,

    /// Server port (default: 1433).
    pub port: u16,

    /// Database name.
    pub database: Option<String>,

    /// Authentication credentials.
    pub credentials: Credentials,

    /// TLS configuration (only available when `tls` feature is enabled).
    #[cfg(feature = "tls")]
    pub tls: TlsConfig,

    /// Application name (shown in SQL Server management tools).
    pub application_name: String,

    /// Connection timeout.
    pub connect_timeout: Duration,

    /// Command timeout.
    pub command_timeout: Duration,

    /// TDS packet size.
    pub packet_size: u16,

    /// Whether to use TDS 8.0 strict mode.
    pub strict_mode: bool,

    /// Whether to trust the server certificate.
    pub trust_server_certificate: bool,

    /// Instance name (for named instances).
    pub instance: Option<String>,

    /// Whether to enable MARS (Multiple Active Result Sets).
    pub mars: bool,

    /// Whether to require encryption (TLS).
    /// When true, the connection will use TLS even if the server doesn't require it.
    /// When false, encryption is used only if the server requires it.
    pub encrypt: bool,

    /// Disable TLS entirely and connect with plaintext.
    ///
    /// **⚠️ SECURITY WARNING:** This completely disables TLS/SSL encryption.
    /// Credentials and data will be transmitted in plaintext. Only use this
    /// for development/testing on trusted networks with legacy SQL Server
    /// instances that don't support modern TLS versions.
    ///
    /// This option exists for compatibility with legacy SQL Server versions
    /// (2008 and earlier) that may only support TLS 1.0/1.1, which modern
    /// TLS libraries (like rustls) don't support for security reasons.
    ///
    /// When `true`:
    /// - Overrides the `encrypt` setting
    /// - Sends `ENCRYPT_NOT_SUP` in PreLogin
    /// - No TLS handshake occurs
    /// - All traffic including login credentials is unencrypted
    ///
    /// **Do not use in production without understanding the security implications.**
    pub no_tls: bool,

    /// Redirect handling configuration (for Azure SQL).
    pub redirect: RedirectConfig,

    /// Retry policy for transient error handling.
    pub retry: RetryPolicy,

    /// Timeout configuration for various connection phases.
    pub timeouts: TimeoutConfig,

    /// Requested TDS protocol version.
    ///
    /// This specifies which TDS protocol version to request during connection.
    /// The server may negotiate a lower version if it doesn't support the requested version.
    ///
    /// Supported versions:
    /// - `TdsVersion::V7_3A` - SQL Server 2008
    /// - `TdsVersion::V7_3B` - SQL Server 2008 R2
    /// - `TdsVersion::V7_4` - SQL Server 2012+ (default)
    /// - `TdsVersion::V8_0` - SQL Server 2022+ strict mode (requires `strict_mode = true`)
    ///
    /// Note: When `strict_mode` is enabled, this is ignored and TDS 8.0 is used.
    pub tds_version: TdsVersion,

    /// Always Encrypted configuration.
    ///
    /// When `Some`, the client will negotiate Always Encrypted support with the
    /// server and transparently decrypt encrypted column values in result sets.
    ///
    /// Set via `Column Encryption Setting=Enabled` in connection strings, or
    /// programmatically via [`Config::with_column_encryption`].
    ///
    /// Wrapped in `Arc` because `EncryptionConfig` contains trait objects (key store
    /// providers) which cannot implement `Clone`. The `Arc` allows `Config` to remain
    /// `Clone` while sharing the encryption configuration.
    #[cfg(feature = "always-encrypted")]
    pub column_encryption: Option<std::sync::Arc<crate::encryption::EncryptionConfig>>,
}

impl Default for Config {
    fn default() -> Self {
        let timeouts = TimeoutConfig::default();
        Self {
            host: "localhost".to_string(),
            port: 1433,
            database: None,
            credentials: Credentials::sql_server("", ""),
            #[cfg(feature = "tls")]
            tls: TlsConfig::default(),
            application_name: "mssql-client".to_string(),
            connect_timeout: timeouts.connect_timeout,
            command_timeout: timeouts.command_timeout,
            packet_size: 4096,
            strict_mode: false,
            trust_server_certificate: false,
            instance: None,
            mars: false,
            encrypt: true, // Default to encrypted for security
            no_tls: false, // Never plaintext by default
            redirect: RedirectConfig::default(),
            retry: RetryPolicy::default(),
            timeouts,
            tds_version: TdsVersion::V7_4, // Default to TDS 7.4 for broad compatibility
            #[cfg(feature = "always-encrypted")]
            column_encryption: None,
        }
    }
}

impl Config {
    /// Create a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a connection string into configuration.
    ///
    /// Supports ADO.NET-style connection strings:
    /// ```text
    /// Server=localhost;Database=mydb;User Id=sa;Password=secret;
    /// ```
    pub fn from_connection_string(conn_str: &str) -> Result<Self, crate::error::Error> {
        let mut config = Self::default();

        for part in conn_str.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| crate::error::Error::Config(format!("invalid key-value: {part}")))?;

            let key = key.trim().to_lowercase();
            let value = value.trim();

            match key.as_str() {
                "server" | "data source" | "host" => {
                    // Handle host:port or host\instance format
                    if let Some((host, port_or_instance)) = value.split_once(',') {
                        config.host = host.to_string();
                        config.port = port_or_instance.parse().map_err(|_| {
                            crate::error::Error::Config(format!("invalid port: {port_or_instance}"))
                        })?;
                    } else if let Some((host, instance)) = value.split_once('\\') {
                        config.host = host.to_string();
                        config.instance = Some(instance.to_string());
                    } else {
                        config.host = value.to_string();
                    }
                }
                "port" => {
                    config.port = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid port: {value}"))
                    })?;
                }
                "database" | "initial catalog" => {
                    config.database = Some(value.to_string());
                }
                "user id" | "uid" | "user" => {
                    // Update credentials with new username
                    if let Credentials::SqlServer { password, .. } = &config.credentials {
                        config.credentials =
                            Credentials::sql_server(value.to_string(), password.clone());
                    }
                }
                "password" | "pwd" => {
                    // Update credentials with new password
                    if let Credentials::SqlServer { username, .. } = &config.credentials {
                        config.credentials =
                            Credentials::sql_server(username.clone(), value.to_string());
                    }
                }
                "application name" | "app" => {
                    config.application_name = value.to_string();
                }
                "connect timeout" | "connection timeout" => {
                    let secs: u64 = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid timeout: {value}"))
                    })?;
                    config.connect_timeout = Duration::from_secs(secs);
                }
                "command timeout" => {
                    let secs: u64 = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid timeout: {value}"))
                    })?;
                    config.command_timeout = Duration::from_secs(secs);
                }
                "trustservercertificate" | "trust server certificate" => {
                    config.trust_server_certificate = value.eq_ignore_ascii_case("true")
                        || value.eq_ignore_ascii_case("yes")
                        || value == "1";
                }
                "encrypt" => {
                    // Handle encryption levels: strict, true, false, yes, no, 1, 0, no_tls
                    if value.eq_ignore_ascii_case("strict") {
                        config.strict_mode = true;
                        config.encrypt = true;
                        config.no_tls = false;
                    } else if value.eq_ignore_ascii_case("no_tls") {
                        // Tiberius-compatible option for truly unencrypted connections.
                        // This is for legacy SQL Server instances that don't support TLS 1.2+.
                        config.no_tls = true;
                        config.encrypt = false;
                    } else if value.eq_ignore_ascii_case("true")
                        || value.eq_ignore_ascii_case("yes")
                        || value == "1"
                    {
                        config.encrypt = true;
                        config.no_tls = false;
                    } else if value.eq_ignore_ascii_case("false")
                        || value.eq_ignore_ascii_case("no")
                        || value == "0"
                    {
                        config.encrypt = false;
                        config.no_tls = false;
                    }
                }
                "column encryption setting" | "columnencryptionsetting" => {
                    #[cfg(feature = "always-encrypted")]
                    if value.eq_ignore_ascii_case("enabled") {
                        config.column_encryption = Some(std::sync::Arc::new(
                            crate::encryption::EncryptionConfig::new(),
                        ));
                    }
                    #[cfg(not(feature = "always-encrypted"))]
                    if value.eq_ignore_ascii_case("enabled") {
                        return Err(crate::error::Error::Config(
                            "Column Encryption Setting=Enabled requires the 'always-encrypted' feature. \
                             Enable it in your Cargo.toml: mssql-client = { features = [\"always-encrypted\"] }"
                                .to_string(),
                        ));
                    }
                }
                "multipleactiveresultsets" | "mars" => {
                    config.mars = value.eq_ignore_ascii_case("true")
                        || value.eq_ignore_ascii_case("yes")
                        || value == "1";
                }
                "packet size" => {
                    config.packet_size = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid packet size: {value}"))
                    })?;
                }
                "tdsversion" | "tds version" | "protocolversion" | "protocol version" => {
                    // Parse TDS version from connection string
                    // Supports: "7.3", "7.3A", "7.3B", "7.4", "8.0"
                    config.tds_version = TdsVersion::parse(value).ok_or_else(|| {
                        crate::error::Error::Config(format!(
                            "invalid TDS version: {value}. Supported values: 7.3, 7.3A, 7.3B, 7.4, 8.0"
                        ))
                    })?;
                    // If TDS 8.0 is requested, enable strict mode
                    if config.tds_version.is_tds_8() {
                        config.strict_mode = true;
                    }
                }
                "integrated security" | "trusted_connection" => {
                    if value.eq_ignore_ascii_case("true")
                        || value.eq_ignore_ascii_case("yes")
                        || value.eq_ignore_ascii_case("sspi")
                        || value == "1"
                    {
                        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                        {
                            config.credentials = Credentials::Integrated;
                        }
                        #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
                        {
                            return Err(crate::error::Error::Config(
                                "Integrated Security requires the 'integrated-auth' (Linux/macOS) \
                                 or 'sspi-auth' (Windows) feature to be enabled"
                                    .into(),
                            ));
                        }
                    }
                }
                _ => {
                    // Ignore unknown options for forward compatibility
                    tracing::debug!(
                        key = key,
                        value = value,
                        "ignoring unknown connection string option"
                    );
                }
            }
        }

        Ok(config)
    }

    /// Set the server host.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the server port.
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the database name.
    #[must_use]
    pub fn database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// Set the credentials.
    #[must_use]
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Set the application name.
    #[must_use]
    pub fn application_name(mut self, name: impl Into<String>) -> Self {
        self.application_name = name.into();
        self
    }

    /// Set the connect timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set trust server certificate option.
    #[must_use]
    pub fn trust_server_certificate(mut self, trust: bool) -> Self {
        self.trust_server_certificate = trust;
        #[cfg(feature = "tls")]
        {
            self.tls = self.tls.trust_server_certificate(trust);
        }
        self
    }

    /// Enable TDS 8.0 strict mode.
    #[must_use]
    pub fn strict_mode(mut self, enabled: bool) -> Self {
        self.strict_mode = enabled;
        #[cfg(feature = "tls")]
        {
            self.tls = self.tls.strict_mode(enabled);
        }
        if enabled {
            self.tds_version = TdsVersion::V8_0;
        }
        self
    }

    /// Set the TDS protocol version.
    ///
    /// This specifies which TDS protocol version to request during connection.
    /// The server may negotiate a lower version if it doesn't support the requested version.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mssql_client::Config;
    /// use tds_protocol::version::TdsVersion;
    ///
    /// // Connect to SQL Server 2008
    /// let config = Config::new()
    ///     .host("legacy-server")
    ///     .tds_version(TdsVersion::V7_3A);
    ///
    /// // Connect to SQL Server 2008 R2
    /// let config = Config::new()
    ///     .host("legacy-server")
    ///     .tds_version(TdsVersion::V7_3B);
    /// ```
    ///
    /// Note: When `strict_mode` is enabled, this is ignored and TDS 8.0 is used.
    #[must_use]
    pub fn tds_version(mut self, version: TdsVersion) -> Self {
        self.tds_version = version;
        // If TDS 8.0 is requested, automatically enable strict mode
        if version.is_tds_8() {
            self.strict_mode = true;
            #[cfg(feature = "tls")]
            {
                self.tls = self.tls.strict_mode(true);
            }
        }
        self
    }

    /// Enable or disable TLS encryption.
    ///
    /// When `true` (default), the connection will use TLS encryption.
    /// When `false`, encryption is used only if the server requires it.
    ///
    /// **Warning:** Disabling encryption is insecure and should only be
    /// used for development/testing on trusted networks.
    #[must_use]
    pub fn encrypt(mut self, enabled: bool) -> Self {
        self.encrypt = enabled;
        self
    }

    /// Disable TLS entirely and connect with plaintext (Tiberius-compatible).
    ///
    /// **⚠️ SECURITY WARNING:** This completely disables TLS/SSL encryption.
    /// Credentials and all data will be transmitted in plaintext over the network.
    ///
    /// # When to use this
    ///
    /// This option exists for compatibility with legacy SQL Server versions
    /// (2008 and earlier) that may only support TLS 1.0/1.1. Modern TLS libraries
    /// like rustls require TLS 1.2 or higher for security reasons, making it
    /// impossible to establish encrypted connections to these older servers.
    ///
    /// # Security implications
    ///
    /// When enabled:
    /// - Login credentials are sent in plaintext
    /// - All query data is transmitted without encryption
    /// - Network traffic can be intercepted and read by attackers
    ///
    /// **Only use this for development/testing on isolated, trusted networks.**
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Connection string (Tiberius-compatible)
    /// let config = Config::from_connection_string(
    ///     "Server=legacy-server;User Id=sa;Password=secret;Encrypt=no_tls"
    /// )?;
    ///
    /// // Builder API
    /// let config = Config::new()
    ///     .host("legacy-server")
    ///     .no_tls(true);
    /// ```
    #[must_use]
    pub fn no_tls(mut self, enabled: bool) -> Self {
        self.no_tls = enabled;
        if enabled {
            self.encrypt = false;
        }
        self
    }

    /// Enable Always Encrypted with the given encryption configuration.
    ///
    /// When enabled, the client will negotiate Always Encrypted support during
    /// connection and transparently decrypt encrypted column values.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::{Config, EncryptionConfig};
    /// use mssql_auth::InMemoryKeyStore;
    ///
    /// let config = Config::new()
    ///     .with_column_encryption(
    ///         EncryptionConfig::new().with_provider(key_store)
    ///     );
    /// ```
    #[cfg(feature = "always-encrypted")]
    #[must_use]
    pub fn with_column_encryption(mut self, config: crate::encryption::EncryptionConfig) -> Self {
        self.column_encryption = Some(std::sync::Arc::new(config));
        self
    }

    /// Create a new configuration with a different host (for routing).
    #[must_use]
    pub fn with_host(mut self, host: &str) -> Self {
        self.host = host.to_string();
        self
    }

    /// Create a new configuration with a different port (for routing).
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the redirect handling configuration.
    #[must_use]
    pub fn redirect(mut self, redirect: RedirectConfig) -> Self {
        self.redirect = redirect;
        self
    }

    /// Set the maximum number of redirect attempts.
    #[must_use]
    pub fn max_redirects(mut self, max: u8) -> Self {
        self.redirect.max_redirects = max;
        self
    }

    /// Set the retry policy for transient error handling.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Set the maximum number of retry attempts.
    #[must_use]
    pub fn max_retries(mut self, max: u32) -> Self {
        self.retry.max_retries = max;
        self
    }

    /// Set the timeout configuration.
    #[must_use]
    pub fn timeouts(mut self, timeouts: TimeoutConfig) -> Self {
        // Sync the legacy fields for backward compatibility first
        self.connect_timeout = timeouts.connect_timeout;
        self.command_timeout = timeouts.command_timeout;
        self.timeouts = timeouts;
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_string_parsing() {
        let config = Config::from_connection_string(
            "Server=localhost;Database=test;User Id=sa;Password=secret;",
        )
        .unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.database, Some("test".to_string()));
    }

    #[test]
    fn test_connection_string_with_port() {
        let config =
            Config::from_connection_string("Server=localhost,1434;Database=test;").unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 1434);
    }

    #[test]
    fn test_connection_string_with_instance() {
        let config =
            Config::from_connection_string("Server=localhost\\SQLEXPRESS;Database=test;").unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.instance, Some("SQLEXPRESS".to_string()));
    }

    #[test]
    fn test_redirect_config_defaults() {
        let config = RedirectConfig::default();
        assert_eq!(config.max_redirects, 2);
        assert!(config.follow_redirects);
    }

    #[test]
    fn test_redirect_config_builder() {
        let config = RedirectConfig::new()
            .max_redirects(5)
            .follow_redirects(false);
        assert_eq!(config.max_redirects, 5);
        assert!(!config.follow_redirects);
    }

    #[test]
    fn test_redirect_config_no_follow() {
        let config = RedirectConfig::no_follow();
        assert_eq!(config.max_redirects, 0);
        assert!(!config.follow_redirects);
    }

    #[test]
    fn test_config_redirect_builder() {
        let config = Config::new().max_redirects(3);
        assert_eq!(config.redirect.max_redirects, 3);

        let config2 = Config::new().redirect(RedirectConfig::no_follow());
        assert!(!config2.redirect.follow_redirects);
    }

    #[test]
    fn test_retry_policy_defaults() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.initial_backoff, Duration::from_millis(100));
        assert_eq!(policy.max_backoff, Duration::from_secs(30));
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!(policy.jitter);
    }

    #[test]
    fn test_retry_policy_builder() {
        let policy = RetryPolicy::new()
            .max_retries(5)
            .initial_backoff(Duration::from_millis(200))
            .max_backoff(Duration::from_secs(60))
            .backoff_multiplier(3.0)
            .jitter(false);

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_backoff, Duration::from_millis(200));
        assert_eq!(policy.max_backoff, Duration::from_secs(60));
        assert!((policy.backoff_multiplier - 3.0).abs() < f64::EPSILON);
        assert!(!policy.jitter);
    }

    #[test]
    fn test_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        assert_eq!(policy.max_retries, 0);
        assert!(!policy.should_retry(0));
    }

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::new().max_retries(3);
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    #[test]
    fn test_retry_policy_backoff_calculation() {
        let policy = RetryPolicy::new()
            .initial_backoff(Duration::from_millis(100))
            .backoff_multiplier(2.0)
            .max_backoff(Duration::from_secs(10))
            .jitter(false);

        assert_eq!(policy.backoff_for_attempt(0), Duration::ZERO);
        assert_eq!(policy.backoff_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.backoff_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_backoff_capped() {
        let policy = RetryPolicy::new()
            .initial_backoff(Duration::from_secs(1))
            .backoff_multiplier(10.0)
            .max_backoff(Duration::from_secs(5))
            .jitter(false);

        // Attempt 3 would be 1s * 10^2 = 100s, but capped at 5s
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(5));
    }

    #[test]
    fn test_config_retry_builder() {
        let config = Config::new().max_retries(5);
        assert_eq!(config.retry.max_retries, 5);

        let config2 = Config::new().retry(RetryPolicy::no_retry());
        assert_eq!(config2.retry.max_retries, 0);
    }

    #[test]
    fn test_timeout_config_defaults() {
        let config = TimeoutConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(15));
        assert_eq!(config.tls_timeout, Duration::from_secs(10));
        assert_eq!(config.login_timeout, Duration::from_secs(30));
        assert_eq!(config.command_timeout, Duration::from_secs(30));
        assert_eq!(config.idle_timeout, Duration::from_secs(300));
        assert_eq!(config.keepalive_interval, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_timeout_config_builder() {
        let config = TimeoutConfig::new()
            .connect_timeout(Duration::from_secs(5))
            .tls_timeout(Duration::from_secs(3))
            .login_timeout(Duration::from_secs(10))
            .command_timeout(Duration::from_secs(60))
            .idle_timeout(Duration::from_secs(600))
            .keepalive_interval(Some(Duration::from_secs(60)));

        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.tls_timeout, Duration::from_secs(3));
        assert_eq!(config.login_timeout, Duration::from_secs(10));
        assert_eq!(config.command_timeout, Duration::from_secs(60));
        assert_eq!(config.idle_timeout, Duration::from_secs(600));
        assert_eq!(config.keepalive_interval, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_timeout_config_no_keepalive() {
        let config = TimeoutConfig::new().no_keepalive();
        assert_eq!(config.keepalive_interval, None);
    }

    #[test]
    fn test_timeout_config_total_connect() {
        let config = TimeoutConfig::new()
            .connect_timeout(Duration::from_secs(5))
            .tls_timeout(Duration::from_secs(3))
            .login_timeout(Duration::from_secs(10));

        // 5 + 3 + 10 = 18 seconds
        assert_eq!(config.total_connect_timeout(), Duration::from_secs(18));
    }

    #[test]
    fn test_config_timeouts_builder() {
        let timeouts = TimeoutConfig::new()
            .connect_timeout(Duration::from_secs(5))
            .command_timeout(Duration::from_secs(60));

        let config = Config::new().timeouts(timeouts);
        assert_eq!(config.timeouts.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.timeouts.command_timeout, Duration::from_secs(60));
        // Check that legacy fields are synced
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.command_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_tds_version_default() {
        let config = Config::default();
        assert_eq!(config.tds_version, TdsVersion::V7_4);
        assert!(!config.strict_mode);
    }

    #[test]
    fn test_tds_version_builder() {
        let config = Config::new().tds_version(TdsVersion::V7_3A);
        assert_eq!(config.tds_version, TdsVersion::V7_3A);
        assert!(!config.strict_mode);

        let config = Config::new().tds_version(TdsVersion::V7_3B);
        assert_eq!(config.tds_version, TdsVersion::V7_3B);
        assert!(!config.strict_mode);

        // TDS 8.0 should automatically enable strict mode
        let config = Config::new().tds_version(TdsVersion::V8_0);
        assert_eq!(config.tds_version, TdsVersion::V8_0);
        assert!(config.strict_mode);
    }

    #[test]
    fn test_strict_mode_sets_tds_8() {
        let config = Config::new().strict_mode(true);
        assert!(config.strict_mode);
        assert_eq!(config.tds_version, TdsVersion::V8_0);
    }

    #[test]
    fn test_connection_string_tds_version() {
        // Test TDS 7.3
        let config = Config::from_connection_string("Server=localhost;TDSVersion=7.3;").unwrap();
        assert_eq!(config.tds_version, TdsVersion::V7_3A);

        // Test TDS 7.3A explicitly
        let config = Config::from_connection_string("Server=localhost;TDSVersion=7.3A;").unwrap();
        assert_eq!(config.tds_version, TdsVersion::V7_3A);

        // Test TDS 7.3B
        let config = Config::from_connection_string("Server=localhost;TDSVersion=7.3B;").unwrap();
        assert_eq!(config.tds_version, TdsVersion::V7_3B);

        // Test TDS 7.4
        let config = Config::from_connection_string("Server=localhost;TDSVersion=7.4;").unwrap();
        assert_eq!(config.tds_version, TdsVersion::V7_4);

        // Test TDS 8.0 enables strict mode
        let config = Config::from_connection_string("Server=localhost;TDSVersion=8.0;").unwrap();
        assert_eq!(config.tds_version, TdsVersion::V8_0);
        assert!(config.strict_mode);

        // Test alternative key names
        let config =
            Config::from_connection_string("Server=localhost;ProtocolVersion=7.3;").unwrap();
        assert_eq!(config.tds_version, TdsVersion::V7_3A);
    }

    #[test]
    fn test_connection_string_invalid_tds_version() {
        let result = Config::from_connection_string("Server=localhost;TDSVersion=invalid;");
        assert!(result.is_err());

        let result = Config::from_connection_string("Server=localhost;TDSVersion=9.0;");
        assert!(result.is_err());
    }

    #[test]
    fn test_connection_string_no_tls() {
        // no_tls should disable TLS entirely
        let config = Config::from_connection_string("Server=legacy;Encrypt=no_tls;").unwrap();
        assert!(config.no_tls);
        assert!(!config.encrypt);
        assert!(!config.strict_mode);

        // Case insensitive
        let config = Config::from_connection_string("Server=legacy;Encrypt=no_tls;").unwrap();
        assert!(config.no_tls);

        // Encrypt=true should disable no_tls
        let config = Config::from_connection_string("Server=localhost;Encrypt=true;").unwrap();
        assert!(!config.no_tls);
        assert!(config.encrypt);

        // Encrypt=strict should disable no_tls
        let config = Config::from_connection_string("Server=localhost;Encrypt=strict;").unwrap();
        assert!(!config.no_tls);
        assert!(config.encrypt);
        assert!(config.strict_mode);
    }

    #[test]
    fn test_no_tls_builder() {
        // Builder method
        let config = Config::new().no_tls(true);
        assert!(config.no_tls);
        assert!(!config.encrypt);

        // Disable
        let config = Config::new().no_tls(true).no_tls(false);
        assert!(!config.no_tls);
    }

    #[test]
    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
    fn test_connection_string_integrated_security() {
        // "Integrated Security=true" should set Credentials::Integrated
        let config =
            Config::from_connection_string("Server=localhost;Integrated Security=true;").unwrap();
        assert_eq!(
            config.credentials.method_name(),
            "Integrated Authentication"
        );

        // "yes" variant
        let config =
            Config::from_connection_string("Server=localhost;Integrated Security=yes;").unwrap();
        assert_eq!(
            config.credentials.method_name(),
            "Integrated Authentication"
        );

        // "sspi" variant
        let config =
            Config::from_connection_string("Server=localhost;Integrated Security=sspi;").unwrap();
        assert_eq!(
            config.credentials.method_name(),
            "Integrated Authentication"
        );

        // "1" variant
        let config =
            Config::from_connection_string("Server=localhost;Integrated Security=1;").unwrap();
        assert_eq!(
            config.credentials.method_name(),
            "Integrated Authentication"
        );

        // Trusted_Connection synonym
        let config =
            Config::from_connection_string("Server=localhost;Trusted_Connection=true;").unwrap();
        assert_eq!(
            config.credentials.method_name(),
            "Integrated Authentication"
        );
    }

    #[test]
    #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
    fn test_connection_string_integrated_security_without_feature() {
        // Should return an error when the feature is not enabled
        let result = Config::from_connection_string("Server=localhost;Integrated Security=true;");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("integrated-auth"));
    }
}
