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

/// Parse a boolean value from a connection string keyword.
///
/// Per the ADO.NET specification, boolean keywords accept:
/// `true`, `false`, `yes`, `no`, `1`, `0` (case-insensitive).
/// Returns an error for any other value, preventing silent misconfiguration.
fn parse_conn_bool(key: &str, value: &str) -> Result<bool, crate::error::Error> {
    match value.to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(crate::error::Error::Config(format!(
            "invalid boolean value for '{key}': '{value}' (expected true/false/yes/no/1/0)"
        ))),
    }
}

/// Split a connection string into key-value pairs, respecting quoted values.
///
/// Per the ADO.NET specification:
/// - Values containing semicolons must be enclosed in double (`"`) or single (`'`) quotes
/// - Doubled quotes inside are escapes: `""` → `"`, `''` → `'`
/// - Leading/trailing whitespace around values is trimmed (but preserved inside quotes)
///
/// Returns pairs of `(key, value)` where the value has quotes stripped and escapes resolved.
fn split_connection_string(conn_str: &str) -> Result<Vec<(String, String)>, crate::error::Error> {
    let mut pairs = Vec::new();
    let chars: Vec<char> = conn_str.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace and semicolons between pairs
        while i < len && (chars[i] == ';' || chars[i].is_whitespace()) {
            i += 1;
        }
        if i >= len {
            break;
        }

        // Read key (up to '=')
        let key_start = i;
        while i < len && chars[i] != '=' {
            i += 1;
        }
        if i >= len {
            // Trailing text with no '=' — skip it (could be trailing whitespace)
            let remaining = chars[key_start..].iter().collect::<String>();
            if remaining.trim().is_empty() {
                break;
            }
            return Err(crate::error::Error::Config(format!(
                "invalid key-value pair (missing '='): '{remaining}'"
            )));
        }
        let key: String = chars[key_start..i].iter().collect();
        i += 1; // skip '='

        // Read value — may be quoted or unquoted
        // Skip leading whitespace in value
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }

        let value = if i < len && (chars[i] == '"' || chars[i] == '\'') {
            // Quoted value: read until matching unescaped closing quote
            let quote_char = chars[i];
            i += 1; // skip opening quote
            let mut val = String::new();
            loop {
                if i >= len {
                    return Err(crate::error::Error::Config(format!(
                        "unterminated quoted value for key '{}'",
                        key.trim()
                    )));
                }
                if chars[i] == quote_char {
                    // Check for escaped quote (doubled: "" or '')
                    if i + 1 < len && chars[i + 1] == quote_char {
                        val.push(quote_char);
                        i += 2;
                    } else {
                        i += 1; // skip closing quote
                        break;
                    }
                } else {
                    val.push(chars[i]);
                    i += 1;
                }
            }
            // Skip to next semicolon or end
            while i < len && chars[i] != ';' {
                i += 1;
            }
            val
        } else {
            // Unquoted value: read until semicolon or end
            let val_start = i;
            while i < len && chars[i] != ';' {
                i += 1;
            }
            chars[val_start..i].iter().collect::<String>()
        };

        let key_trimmed = key.trim().to_string();
        if !key_trimmed.is_empty() {
            pairs.push((key_trimmed, value));
        }
    }

    Ok(pairs)
}

/// Convert a connection string value to `Option<String>`, treating empty strings as `None`.
///
/// In ADO.NET, specifying a keyword with an empty value (e.g., `Database=;`) resets it
/// to its default. We represent this as `None` for optional fields.
fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

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

    /// Application workload intent for AlwaysOn Availability Group routing.
    ///
    /// When set to [`ApplicationIntent::ReadOnly`], SQL Server routes the
    /// connection to a readable secondary replica. Sent in LOGIN7 TypeFlags
    /// as the `READONLY_INTENT` bit.
    pub application_intent: ApplicationIntent,

    /// Client workstation name sent to SQL Server in the LOGIN7 HostName field.
    ///
    /// Used for auditing via `sys.dm_exec_sessions.host_name`.
    /// When `None`, the driver sends the machine hostname (from the `COMPUTERNAME`
    /// or `HOSTNAME` environment variable). Set via `Workstation ID` or `WSID`
    /// in connection strings.
    pub workstation_id: Option<String>,

    /// Session language for server warning/error messages.
    ///
    /// When set, sent in LOGIN7's Language field. The language name can be
    /// up to 128 characters. Set via `Language` or `Current Language` in
    /// connection strings.
    pub language: Option<String>,

    /// Enable MultiSubnetFailover for AlwaysOn Availability Group listeners.
    ///
    /// When `true`, the driver resolves the server hostname to all IP addresses
    /// and attempts parallel TCP connections simultaneously. The first successful
    /// connection wins and all others are cancelled. This reduces connection time
    /// when the AG listener spans multiple subnets.
    ///
    /// Set via `MultiSubnetFailover=True` in connection strings.
    ///
    /// Default: `false`
    pub multi_subnet_failover: bool,

    /// Whether to send `String`/`&str` parameters as NVARCHAR (Unicode).
    ///
    /// When `true` (default), string parameters are sent as NVARCHAR using
    /// UTF-16LE encoding. This is safe for all character sets but prevents
    /// SQL Server from using index seeks on VARCHAR columns (due to implicit
    /// NVARCHAR→VARCHAR conversion).
    ///
    /// When `false`, string parameters are sent as VARCHAR using Windows-1252
    /// encoding. This allows index seeks on VARCHAR columns but may lose data
    /// for characters outside the Windows-1252 range.
    ///
    /// Set via `SendStringParametersAsUnicode=false` in connection strings.
    ///
    /// Default: `true`
    pub send_string_parameters_as_unicode: bool,

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
            application_intent: ApplicationIntent::default(),
            workstation_id: None,
            language: None,
            multi_subnet_failover: false,
            send_string_parameters_as_unicode: true,
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
    /// Supports ADO.NET-style connection strings with full quoting support:
    /// ```text
    /// Server=localhost;Database=mydb;User Id=sa;Password="complex;pass";
    /// ```
    ///
    /// Values containing semicolons can be enclosed in double or single quotes
    /// per the ADO.NET specification. The `tcp:` prefix from Azure Portal
    /// connection strings is automatically stripped.
    pub fn from_connection_string(conn_str: &str) -> Result<Self, crate::error::Error> {
        let mut config = Self::default();
        let pairs = split_connection_string(conn_str)?;

        for (key, value) in &pairs {
            let key = key.trim().to_lowercase();
            let value = value.trim();

            match key.as_str() {
                // --- Server / Data Source (ADO.NET aliases: Addr, Address, Network Address) ---
                "server" | "data source" | "addr" | "address" | "network address" | "host" => {
                    // Strip tcp: prefix (common in Azure Portal connection strings).
                    // Reject np: (Named Pipes) and lpc: (Shared Memory) — not supported.
                    // All prefix checks are case-insensitive per ADO.NET conventions.
                    let lower_value = value.to_lowercase();
                    let server_value = if lower_value.starts_with("tcp:") {
                        &value[4..]
                    } else if lower_value.starts_with("np:") {
                        return Err(crate::error::Error::Config(
                            "Named Pipes connections (np:) are not supported. Use TCP connections instead."
                                .into(),
                        ));
                    } else if lower_value.starts_with("lpc:") {
                        return Err(crate::error::Error::Config(
                            "Shared Memory connections (lpc:) are not supported. Use TCP connections instead."
                                .into(),
                        ));
                    } else {
                        value
                    };

                    // Handle host,port or host\instance format
                    if let Some((host, port_or_instance)) = server_value.split_once(',') {
                        config.host = host.to_string();
                        config.port = port_or_instance.trim().parse().map_err(|_| {
                            crate::error::Error::Config(format!("invalid port: {port_or_instance}"))
                        })?;
                    } else if let Some((host, instance)) = server_value.split_once('\\') {
                        config.host = host.to_string();
                        config.instance = non_empty(instance);
                    } else {
                        config.host = server_value.to_string();
                    }
                }
                "port" => {
                    config.port = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid port: {value}"))
                    })?;
                }
                // --- Database ---
                "database" | "initial catalog" => {
                    config.database = non_empty(value);
                }
                // --- Credentials ---
                "user id" | "uid" | "user" => {
                    if let Credentials::SqlServer { password, .. } = &config.credentials {
                        config.credentials =
                            Credentials::sql_server(value.to_string(), password.clone());
                    }
                }
                "password" | "pwd" => {
                    if let Credentials::SqlServer { username, .. } = &config.credentials {
                        config.credentials =
                            Credentials::sql_server(username.clone(), value.to_string());
                    }
                }
                // --- Application ---
                "application name" | "app" => {
                    config.application_name = value.to_string();
                }
                "applicationintent" | "application intent" => {
                    config.application_intent = match value.to_lowercase().as_str() {
                        "readonly" => ApplicationIntent::ReadOnly,
                        "readwrite" => ApplicationIntent::ReadWrite,
                        _ => {
                            return Err(crate::error::Error::Config(format!(
                                "invalid ApplicationIntent: '{value}' (expected ReadOnly or ReadWrite)"
                            )));
                        }
                    };
                }
                "workstation id" | "wsid" => {
                    config.workstation_id = non_empty(value);
                }
                "current language" | "language" => {
                    config.language = non_empty(value);
                }
                // --- Timeouts (ADO.NET alias: Timeout) ---
                "connect timeout" | "connection timeout" | "timeout" => {
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
                // --- Security ---
                "trustservercertificate" | "trust server certificate" => {
                    config.trust_server_certificate = parse_conn_bool(&key, value)?;
                }
                "encrypt" => {
                    // Encrypt supports several non-boolean values beyond true/false:
                    // - "strict" = TDS 8.0 strict mode (always encrypted transport)
                    // - "mandatory" / "true" / "yes" / "1" = require TLS
                    // - "optional" / "false" / "no" / "0" = TLS only if server requires
                    // - "no_tls" = Tiberius-compatible plaintext mode for legacy servers
                    //
                    // "mandatory" and "optional" are Microsoft.Data.SqlClient v5+ aliases.
                    if value.eq_ignore_ascii_case("strict") {
                        config.strict_mode = true;
                        config.encrypt = true;
                        config.no_tls = false;
                    } else if value.eq_ignore_ascii_case("mandatory") {
                        config.encrypt = true;
                        config.no_tls = false;
                    } else if value.eq_ignore_ascii_case("optional") {
                        config.encrypt = false;
                        config.no_tls = false;
                    } else if value.eq_ignore_ascii_case("no_tls") {
                        config.no_tls = true;
                        config.encrypt = false;
                    } else {
                        // Standard boolean values (true/false/yes/no/1/0)
                        let enabled = parse_conn_bool(&key, value)?;
                        config.encrypt = enabled;
                        config.no_tls = false;
                    }
                }
                "integrated security" | "trusted_connection" => {
                    // Accepts standard booleans + "sspi" (ADO.NET strongly-recommended value)
                    let enabled =
                        value.eq_ignore_ascii_case("sspi") || parse_conn_bool(&key, value)?;
                    if enabled {
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
                // --- Always Encrypted ---
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
                // --- Protocol ---
                "multipleactiveresultsets" | "mars" => {
                    config.mars = parse_conn_bool(&key, value)?;
                }
                "packet size" => {
                    config.packet_size = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid packet size: {value}"))
                    })?;
                }
                "tdsversion" | "tds version" | "protocolversion" | "protocol version" => {
                    config.tds_version = TdsVersion::parse(value).ok_or_else(|| {
                        crate::error::Error::Config(format!(
                            "invalid TDS version: {value}. Supported values: 7.3, 7.3A, 7.3B, 7.4, 8.0"
                        ))
                    })?;
                    if config.tds_version.is_tds_8() {
                        config.strict_mode = true;
                    }
                }
                // --- Connection resiliency ---
                "connectretrycount" | "connect retry count" => {
                    config.retry.max_retries = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!("invalid ConnectRetryCount: '{value}'"))
                    })?;
                }
                "connectretryinterval" | "connect retry interval" => {
                    let secs: u64 = value.parse().map_err(|_| {
                        crate::error::Error::Config(format!(
                            "invalid ConnectRetryInterval: '{value}'"
                        ))
                    })?;
                    config.retry.initial_backoff = Duration::from_secs(secs);
                }
                // --- Pool keywords: recognized but must be set via PoolConfig ---
                "max pool size"
                | "min pool size"
                | "pooling"
                | "connection lifetime"
                | "load balance timeout" => {
                    tracing::info!(
                        key = key.as_str(),
                        value = value,
                        "connection string keyword '{}' is recognized but pool settings \
                         must be configured via PoolConfig, not the connection string",
                        key,
                    );
                }
                // --- MultiSubnetFailover ---
                "multisubnetfailover" | "multi subnet failover" => {
                    config.multi_subnet_failover = parse_conn_bool(&key, value)?;
                }
                // --- String parameter encoding ---
                "sendstringparametersasunicode" | "send string parameters as unicode" => {
                    config.send_string_parameters_as_unicode = parse_conn_bool(&key, value)?;
                }
                // --- Known ADO.NET keywords not supported by this driver ---
                "failover partner"
                | "persist security info"
                | "persistsecurityinfo"
                | "enlist"
                | "replication"
                | "transaction binding"
                | "type system version"
                | "user instance"
                | "attachdbfilename"
                | "extended properties"
                | "initial file name"
                | "context connection"
                | "network library"
                | "network"
                | "net"
                | "asynchronous processing"
                | "async"
                | "transparentnetworkipresolution"
                | "poolblockingperiod"
                | "authentication"
                | "hostnameincertificate"
                | "servercertificate" => {
                    tracing::info!(
                        key = key.as_str(),
                        value = value,
                        "connection string keyword '{}' is recognized but not supported by this driver",
                        key,
                    );
                }
                _ => {
                    tracing::debug!(
                        key = key.as_str(),
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

    /// Set the application workload intent for AlwaysOn AG routing.
    #[must_use]
    pub fn application_intent(mut self, intent: ApplicationIntent) -> Self {
        self.application_intent = intent;
        self
    }

    /// Set the client workstation name sent to SQL Server in LOGIN7.
    ///
    /// This appears in `sys.dm_exec_sessions.host_name` for auditing.
    /// When not set, the driver sends the machine hostname automatically.
    #[must_use]
    pub fn workstation_id(mut self, id: impl Into<String>) -> Self {
        self.workstation_id = Some(id.into());
        self
    }

    /// Set the session language for server messages.
    ///
    /// The language name can be up to 128 characters (e.g., `"us_english"`).
    #[must_use]
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// Enable MultiSubnetFailover for AlwaysOn Availability Group listeners.
    ///
    /// When enabled, the driver resolves the server hostname to all IP addresses
    /// and races parallel TCP connections. The first successful connection wins.
    #[must_use]
    pub fn multi_subnet_failover(mut self, enabled: bool) -> Self {
        self.multi_subnet_failover = enabled;
        self
    }

    /// Control whether string parameters are sent as NVARCHAR (Unicode) or VARCHAR.
    ///
    /// When `false`, `String`/`&str` parameters are sent as VARCHAR using
    /// Windows-1252 encoding, which allows SQL Server to use index seeks on
    /// VARCHAR columns.
    ///
    /// Default: `true` (NVARCHAR)
    #[must_use]
    pub fn send_string_parameters_as_unicode(mut self, enabled: bool) -> Self {
        self.send_string_parameters_as_unicode = enabled;
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
    fn test_connection_string_dot_instance() {
        // "." is a standard ADO.NET alias for localhost
        let config = Config::from_connection_string("Server=.\\SQLEXPRESS;Database=test;").unwrap();

        assert_eq!(config.host, ".");
        assert_eq!(config.instance, Some("SQLEXPRESS".to_string()));
    }

    #[test]
    fn test_connection_string_local_instance() {
        // "(local)" is a standard ADO.NET alias for localhost
        let config =
            Config::from_connection_string("Server=(local)\\SQLEXPRESS;Database=test;").unwrap();

        assert_eq!(config.host, "(local)");
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

        // Encrypt=mandatory (Microsoft.Data.SqlClient v5+ alias for true)
        let config = Config::from_connection_string("Server=localhost;Encrypt=mandatory;").unwrap();
        assert!(config.encrypt);
        assert!(!config.no_tls);

        // Encrypt=optional (Microsoft.Data.SqlClient v5+ alias for false)
        let config = Config::from_connection_string("Server=localhost;Encrypt=optional;").unwrap();
        assert!(!config.encrypt);
        assert!(!config.no_tls);
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

    // =======================================================================
    // ADO.NET conformance tests (quoted values, aliases, boolean validation)
    // =======================================================================

    #[test]
    fn test_parse_conn_bool_all_values() {
        assert!(parse_conn_bool("test", "true").unwrap());
        assert!(parse_conn_bool("test", "True").unwrap());
        assert!(parse_conn_bool("test", "TRUE").unwrap());
        assert!(parse_conn_bool("test", "yes").unwrap());
        assert!(parse_conn_bool("test", "Yes").unwrap());
        assert!(parse_conn_bool("test", "1").unwrap());

        assert!(!parse_conn_bool("test", "false").unwrap());
        assert!(!parse_conn_bool("test", "False").unwrap());
        assert!(!parse_conn_bool("test", "FALSE").unwrap());
        assert!(!parse_conn_bool("test", "no").unwrap());
        assert!(!parse_conn_bool("test", "No").unwrap());
        assert!(!parse_conn_bool("test", "0").unwrap());

        // Invalid values should error
        assert!(parse_conn_bool("test", "banana").is_err());
        assert!(parse_conn_bool("test", "tru").is_err());
        assert!(parse_conn_bool("test", "").is_err());
    }

    #[test]
    fn test_boolean_validation_trust_server_certificate() {
        // Valid boolean → ok
        let config =
            Config::from_connection_string("Server=localhost;TrustServerCertificate=true;")
                .unwrap();
        assert!(config.trust_server_certificate);

        let config =
            Config::from_connection_string("Server=localhost;TrustServerCertificate=no;").unwrap();
        assert!(!config.trust_server_certificate);

        // Invalid boolean → error (previously silently set to false!)
        let result =
            Config::from_connection_string("Server=localhost;TrustServerCertificate=banana;");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid boolean"));
    }

    #[test]
    fn test_boolean_validation_mars() {
        let config = Config::from_connection_string("Server=localhost;MARS=true;").unwrap();
        assert!(config.mars);

        // Typo → error instead of silent false
        let result = Config::from_connection_string("Server=localhost;MARS=tru;");
        assert!(result.is_err());
    }

    #[test]
    fn test_quoted_value_semicolon() {
        // Password with semicolons — must be quoted per ADO.NET spec
        let config = Config::from_connection_string(
            r#"Server=localhost;User Id=sa;Password="my;complex;pass";"#,
        )
        .unwrap();
        if let mssql_auth::Credentials::SqlServer { password, .. } = &config.credentials {
            assert_eq!(password.as_ref(), "my;complex;pass");
        } else {
            unreachable!("expected SqlServer credentials");
        }
    }

    #[test]
    fn test_quoted_value_single_quotes() {
        let config =
            Config::from_connection_string("Server=localhost;User Id=sa;Password='my;pass';")
                .unwrap();
        if let mssql_auth::Credentials::SqlServer { password, .. } = &config.credentials {
            assert_eq!(password.as_ref(), "my;pass");
        } else {
            unreachable!("expected SqlServer credentials");
        }
    }

    #[test]
    fn test_quoted_value_escaped_double_quotes() {
        // Doubled quotes → single quote per ADO.NET spec
        let config = Config::from_connection_string(
            r#"Server=localhost;User Id=sa;Password="has ""quotes""";"#,
        )
        .unwrap();
        if let mssql_auth::Credentials::SqlServer { password, .. } = &config.credentials {
            assert_eq!(password.as_ref(), r#"has "quotes""#);
        } else {
            unreachable!("expected SqlServer credentials");
        }
    }

    #[test]
    fn test_quoted_value_escaped_single_quotes() {
        let config =
            Config::from_connection_string("Server=localhost;User Id=sa;Password='it''s complex';")
                .unwrap();
        if let mssql_auth::Credentials::SqlServer { password, .. } = &config.credentials {
            assert_eq!(password.as_ref(), "it's complex");
        } else {
            unreachable!("expected SqlServer credentials");
        }
    }

    #[test]
    fn test_quoted_value_unterminated() {
        let result = Config::from_connection_string(r#"Server=localhost;Password="unterminated;"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unterminated"));
    }

    #[test]
    fn test_tcp_prefix_stripped() {
        // Azure Portal format: tcp:hostname,port
        let config = Config::from_connection_string(
            "Server=tcp:myserver.database.windows.net,1433;Database=mydb;",
        )
        .unwrap();
        assert_eq!(config.host, "myserver.database.windows.net");
        assert_eq!(config.port, 1433);
    }

    #[test]
    fn test_tcp_prefix_mixed_case() {
        // Protocol prefixes are case-insensitive per ADO.NET
        let config = Config::from_connection_string("Server=Tcp:myhost,1433;").unwrap();
        assert_eq!(config.host, "myhost");

        let config = Config::from_connection_string("Server=TCP:myhost,1433;").unwrap();
        assert_eq!(config.host, "myhost");
    }

    #[test]
    fn test_tcp_prefix_with_instance() {
        let config =
            Config::from_connection_string("Server=tcp:myhost\\INST;Database=test;").unwrap();
        assert_eq!(config.host, "myhost");
        assert_eq!(config.instance, Some("INST".to_string()));
    }

    #[test]
    fn test_np_prefix_rejected() {
        let result =
            Config::from_connection_string(r"Server=np:\\myhost\pipe\sql\query;Database=test;");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Named Pipes"));

        // Case-insensitive rejection
        let result =
            Config::from_connection_string(r"Server=NP:\\myhost\pipe\sql\query;Database=test;");
        assert!(result.is_err());
    }

    #[test]
    fn test_lpc_prefix_rejected() {
        let result = Config::from_connection_string("Server=lpc:myhost;Database=test;");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Shared Memory"));
    }

    #[test]
    fn test_server_alias_addr() {
        let config = Config::from_connection_string("Addr=myhost;").unwrap();
        assert_eq!(config.host, "myhost");
    }

    #[test]
    fn test_server_alias_address() {
        let config = Config::from_connection_string("Address=myhost,1434;").unwrap();
        assert_eq!(config.host, "myhost");
        assert_eq!(config.port, 1434);
    }

    #[test]
    fn test_server_alias_network_address() {
        let config = Config::from_connection_string("Network Address=myhost;").unwrap();
        assert_eq!(config.host, "myhost");
    }

    #[test]
    fn test_timeout_alias() {
        let config = Config::from_connection_string("Server=localhost;Timeout=30;").unwrap();
        assert_eq!(config.connect_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_application_intent_readonly() {
        let config =
            Config::from_connection_string("Server=localhost;ApplicationIntent=ReadOnly;").unwrap();
        assert_eq!(config.application_intent, ApplicationIntent::ReadOnly);
    }

    #[test]
    fn test_application_intent_readwrite() {
        let config =
            Config::from_connection_string("Server=localhost;Application Intent=ReadWrite;")
                .unwrap();
        assert_eq!(config.application_intent, ApplicationIntent::ReadWrite);
    }

    #[test]
    fn test_application_intent_invalid() {
        let result = Config::from_connection_string("Server=localhost;ApplicationIntent=banana;");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("ApplicationIntent")
        );
    }

    #[test]
    fn test_workstation_id() {
        let config =
            Config::from_connection_string("Server=localhost;Workstation ID=MYPC;").unwrap();
        assert_eq!(config.workstation_id, Some("MYPC".to_string()));
    }

    #[test]
    fn test_wsid_alias() {
        let config =
            Config::from_connection_string("Server=localhost;WSID=MYWORKSTATION;").unwrap();
        assert_eq!(config.workstation_id, Some("MYWORKSTATION".to_string()));
    }

    #[test]
    fn test_language() {
        let config =
            Config::from_connection_string("Server=localhost;Language=us_english;").unwrap();
        assert_eq!(config.language, Some("us_english".to_string()));
    }

    #[test]
    fn test_current_language_alias() {
        let config =
            Config::from_connection_string("Server=localhost;Current Language=Deutsch;").unwrap();
        assert_eq!(config.language, Some("Deutsch".to_string()));
    }

    #[test]
    fn test_connect_retry_count() {
        let config =
            Config::from_connection_string("Server=localhost;ConnectRetryCount=5;").unwrap();
        assert_eq!(config.retry.max_retries, 5);
    }

    #[test]
    fn test_connect_retry_interval() {
        let config =
            Config::from_connection_string("Server=localhost;ConnectRetryInterval=15;").unwrap();
        assert_eq!(config.retry.initial_backoff, Duration::from_secs(15));
    }

    #[test]
    fn test_pool_keywords_accepted_without_error() {
        // Pool keywords should be recognized (not error) but not affect Config
        let result = Config::from_connection_string(
            "Server=localhost;Max Pool Size=10;Min Pool Size=2;Pooling=true;",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_known_unsupported_keywords_accepted() {
        // Known ADO.NET keywords we don't support should not error
        let result = Config::from_connection_string(
            "Server=localhost;Failover Partner=backup;Persist Security Info=false;",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_multi_subnet_failover_connection_string() {
        let config =
            Config::from_connection_string("Server=ag-listener;MultiSubnetFailover=true;").unwrap();
        assert!(config.multi_subnet_failover);

        // Space-separated variant
        let config =
            Config::from_connection_string("Server=ag-listener;Multi Subnet Failover=true;")
                .unwrap();
        assert!(config.multi_subnet_failover);

        // Disabled
        let config =
            Config::from_connection_string("Server=ag-listener;MultiSubnetFailover=false;")
                .unwrap();
        assert!(!config.multi_subnet_failover);

        // Default is false
        let config = Config::from_connection_string("Server=localhost;").unwrap();
        assert!(!config.multi_subnet_failover);
    }

    #[test]
    fn test_multi_subnet_failover_builder() {
        let config = Config::new().multi_subnet_failover(true);
        assert!(config.multi_subnet_failover);

        let config = Config::new().multi_subnet_failover(false);
        assert!(!config.multi_subnet_failover);
    }

    #[test]
    fn test_multi_subnet_failover_invalid_value() {
        let result = Config::from_connection_string("Server=localhost;MultiSubnetFailover=banana;");
        assert!(result.is_err());
    }

    #[test]
    fn test_application_intent_builder() {
        let config = Config::new().application_intent(ApplicationIntent::ReadOnly);
        assert_eq!(config.application_intent, ApplicationIntent::ReadOnly);
    }

    #[test]
    fn test_workstation_id_builder() {
        let config = Config::new().workstation_id("MY-PC");
        assert_eq!(config.workstation_id, Some("MY-PC".to_string()));
    }

    #[test]
    fn test_language_builder() {
        let config = Config::new().language("us_english");
        assert_eq!(config.language, Some("us_english".to_string()));
    }

    #[test]
    fn test_send_string_parameters_as_unicode_connection_string() {
        let config =
            Config::from_connection_string("Server=localhost;SendStringParametersAsUnicode=false;")
                .unwrap();
        assert!(!config.send_string_parameters_as_unicode);

        // Space-separated variant
        let config = Config::from_connection_string(
            "Server=localhost;Send String Parameters As Unicode=false;",
        )
        .unwrap();
        assert!(!config.send_string_parameters_as_unicode);

        // Enabled explicitly
        let config =
            Config::from_connection_string("Server=localhost;SendStringParametersAsUnicode=true;")
                .unwrap();
        assert!(config.send_string_parameters_as_unicode);

        // Default is true
        let config = Config::from_connection_string("Server=localhost;").unwrap();
        assert!(config.send_string_parameters_as_unicode);
    }

    #[test]
    fn test_send_string_parameters_as_unicode_builder() {
        let config = Config::new().send_string_parameters_as_unicode(false);
        assert!(!config.send_string_parameters_as_unicode);

        let config = Config::new().send_string_parameters_as_unicode(true);
        assert!(config.send_string_parameters_as_unicode);
    }

    #[test]
    fn test_send_string_parameters_as_unicode_invalid_value() {
        let result = Config::from_connection_string(
            "Server=localhost;SendStringParametersAsUnicode=banana;",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_values_become_none() {
        // Per ADO.NET, empty values reset optional fields to default (None)
        let config =
            Config::from_connection_string("Server=localhost;Database=;Language=;").unwrap();
        assert_eq!(config.database, None);
        assert_eq!(config.language, None);
    }
}
