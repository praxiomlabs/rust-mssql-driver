//! Client error types.

use std::sync::Arc;

use thiserror::Error;

/// Errors that can occur during client operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Connection failed.
    #[error("connection failed: {0}")]
    Connection(String),

    /// Connection closed unexpectedly.
    #[error("connection closed")]
    ConnectionClosed,

    /// Authentication failed.
    #[error("authentication failed: {0}")]
    Authentication(#[from] mssql_auth::AuthError),

    /// TLS error.
    #[cfg(feature = "tls")]
    #[error("TLS error: {0}")]
    Tls(#[from] mssql_tls::TlsError),

    /// TLS error (when TLS feature is disabled, stores the message).
    #[cfg(not(feature = "tls"))]
    #[error("TLS error: {0}")]
    Tls(String),

    /// Protocol error from the TDS layer (preserves the source error chain).
    #[error("protocol error: {0}")]
    ProtocolError(#[from] tds_protocol::ProtocolError),

    /// Protocol violation with a descriptive message.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Codec error.
    #[error("codec error: {0}")]
    Codec(#[from] mssql_codec::CodecError),

    /// Type conversion error.
    #[error("type error: {0}")]
    Type(#[from] mssql_types::TypeError),

    /// Query execution error.
    #[error("query error: {0}")]
    Query(String),

    /// Server returned an error.
    #[error("server error {number} (severity {class}, state {state}): {message}{}", format_server_location(.server, .procedure, .line))]
    Server {
        /// Error number.
        number: i32,
        /// Error class/severity (0-25).
        class: u8,
        /// Error state.
        state: u8,
        /// Error message.
        message: String,
        /// Server name where error occurred.
        server: Option<String>,
        /// Stored procedure name (if applicable).
        procedure: Option<String>,
        /// Line number in the SQL batch or procedure.
        line: u32,
    },

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// TCP connection timeout occurred.
    #[error("TCP connection timed out connecting to {host}:{port}")]
    ConnectTimeout {
        /// Target host.
        host: String,
        /// Target port.
        port: u16,
    },

    /// TLS handshake timeout occurred.
    #[error("TLS handshake timed out with {host}:{port}")]
    TlsTimeout {
        /// Target host.
        host: String,
        /// Target port.
        port: u16,
    },

    /// Login/authentication response timeout occurred.
    #[error("login timed out for {host}:{port}")]
    LoginTimeout {
        /// Target host.
        host: String,
        /// Target port.
        port: u16,
    },

    /// Command execution timeout occurred.
    #[error("command timed out")]
    CommandTimeout,

    /// Connection routing required (Azure SQL).
    #[error("routing required to {host}:{port}")]
    Routing {
        /// Target host.
        host: String,
        /// Target port.
        port: u16,
    },

    /// Too many redirects during connection.
    #[error("too many redirects (max {max})")]
    TooManyRedirects {
        /// Maximum redirects allowed.
        max: u8,
    },

    /// IO error (wrapped in Arc for Clone support).
    #[error("IO error: {0}")]
    Io(#[source] SharedIoError),

    /// Invalid identifier (potential SQL injection attempt).
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),

    /// Connection pool exhausted.
    #[error("connection pool exhausted")]
    PoolExhausted,

    /// Query cancellation error.
    #[error("query cancellation failed: {0}")]
    Cancel(String),

    /// Query was cancelled by user request.
    #[error("query cancelled")]
    Cancelled,

    /// SQL Browser service instance resolution failed.
    #[error("SQL Browser resolution failed for instance '{instance}': {reason}")]
    BrowserResolution {
        /// The instance name that was being resolved.
        instance: String,
        /// Description of what went wrong.
        reason: String,
    },

    /// FILESTREAM operation failed.
    ///
    /// This error occurs when opening or accessing a FILESTREAM BLOB fails,
    /// typically due to a missing driver DLL, invalid path, or permission issue.
    #[cfg(all(windows, feature = "filestream"))]
    #[error("FILESTREAM error: {0}")]
    FileStream(String),

    /// Always Encrypted operation failed.
    ///
    /// This error occurs during CEK decryption, column value decryption, or
    /// parameter encryption. Key material is never included in the error message.
    #[cfg(feature = "always-encrypted")]
    #[error("encryption error: {0}")]
    Encryption(String),
}

// Note: From<mssql_tls::TlsError> and From<tds_protocol::ProtocolError> are
// derived via #[from] on the enum variants above, preserving the full error chain.

/// A cloneable wrapper around `std::io::Error` that preserves the error source chain.
///
/// `Arc<io::Error>` does not implement `std::error::Error`, which breaks
/// `source()` chain traversal used by libraries like `anyhow` and `eyre`.
/// This newtype bridges the gap.
#[derive(Debug, Clone)]
pub struct SharedIoError(Arc<std::io::Error>);

impl std::fmt::Display for SharedIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for SharedIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(SharedIoError(Arc::new(e)))
    }
}

#[cfg(feature = "always-encrypted")]
impl From<mssql_auth::EncryptionError> for Error {
    fn from(e: mssql_auth::EncryptionError) -> Self {
        // SECURITY: Do NOT include key material in the error message.
        // EncryptionError::Display does not log keys, but we convert to
        // String to ensure no internal state leaks.
        Error::Encryption(e.to_string())
    }
}

impl Error {
    /// Check if this error is transient and may succeed on retry.
    ///
    /// Transient errors include timeouts, connection issues, and
    /// certain server errors that may resolve themselves.
    ///
    /// Per ADR-009, the following server error codes are considered transient:
    /// - 1205: Deadlock victim
    /// - -2: Timeout
    /// - 10928, 10929: Resource limit (Azure)
    /// - 40197: Service error (Azure)
    /// - 40501: Service busy (Azure)
    /// - 40613: Database unavailable (Azure)
    /// - 49918, 49919, 49920: Cannot process request (Azure)
    /// - 4060: Cannot open database (may be transient during failover)
    /// - 18456: Login failed (may be transient in Azure during failover)
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::ConnectTimeout { .. }
            | Self::TlsTimeout { .. }
            | Self::LoginTimeout { .. }
            | Self::CommandTimeout
            | Self::ConnectionClosed
            | Self::Connection(_)
            | Self::Routing { .. }
            | Self::PoolExhausted
            | Self::Io(_) => true,
            Self::Server { number, .. } => Self::is_transient_server_error(*number),
            _ => false,
        }
    }

    /// Check if a server error number is transient (may succeed on retry).
    ///
    /// This follows the error codes specified in ADR-009.
    ///
    /// # Extending with custom error codes
    ///
    /// Applications with domain-specific transient error codes can compose
    /// this method with their own logic:
    ///
    /// ```rust
    /// use mssql_client::Error;
    ///
    /// fn is_transient_for_my_app(err: &Error) -> bool {
    ///     // Check built-in transient codes first
    ///     if err.is_transient() {
    ///         return true;
    ///     }
    ///     // Add application-specific transient server errors
    ///     if let Error::Server { number, .. } = err {
    ///         matches!(number, 50001 | 50002) // custom app error codes
    ///     } else {
    ///         false
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn is_transient_server_error(number: i32) -> bool {
        matches!(
            number,
            1205 |      // Deadlock victim
            -2 |        // Timeout
            10928 |     // Resource limit (Azure)
            10929 |     // Resource limit (Azure)
            40197 |     // Service error (Azure)
            40501 |     // Service busy (Azure)
            40613 |     // Database unavailable (Azure)
            49918 |     // Cannot process request (Azure)
            49919 |     // Cannot process create/update (Azure)
            49920 |     // Cannot process request (Azure)
            4060 |      // Cannot open database
            18456 // Login failed (may be transient in Azure)
        )
    }

    /// Check if this is a terminal error that will never succeed on retry.
    ///
    /// Terminal errors include syntax errors, constraint violations, and
    /// other errors that indicate programmer error or data issues.
    ///
    /// Per ADR-009, the following server error codes are terminal:
    /// - 102: Syntax error
    /// - 207: Invalid column
    /// - 208: Invalid object
    /// - 547: Constraint violation
    /// - 2627: Unique constraint violation
    /// - 2601: Duplicate key
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::Config(_)
            | Self::InvalidIdentifier(_)
            | Self::Protocol(_)
            | Self::ProtocolError(_)
            | Self::Tls(_)
            | Self::Authentication(_)
            | Self::Cancel(_) => true,
            Self::Server { number, .. } => Self::is_terminal_server_error(*number),
            _ => false,
        }
    }

    /// Check if a server error number is terminal (will never succeed on retry).
    ///
    /// This follows the error codes specified in ADR-009.
    #[must_use]
    pub fn is_terminal_server_error(number: i32) -> bool {
        matches!(
            number,
            102 |       // Syntax error
            207 |       // Invalid column
            208 |       // Invalid object
            547 |       // Constraint violation
            2627 |      // Unique constraint violation
            2601 // Duplicate key
        )
    }

    /// Check if this error indicates a protocol/driver bug.
    ///
    /// Protocol errors typically indicate a bug in the driver implementation
    /// rather than a user error or server issue. These are always terminal.
    #[must_use]
    pub fn is_protocol_error(&self) -> bool {
        matches!(self, Self::Protocol(_) | Self::ProtocolError(_))
    }

    /// Check if this is a TLS/encryption error.
    ///
    /// TLS errors indicate certificate, handshake, or encryption failures.
    /// These are terminal — TLS timeouts are reported as [`Error::TlsTimeout`] instead.
    #[must_use]
    pub fn is_tls_error(&self) -> bool {
        matches!(self, Self::Tls(_) | Self::TlsTimeout { .. })
    }

    /// Check if this is an authentication error.
    #[must_use]
    pub fn is_authentication_error(&self) -> bool {
        matches!(self, Self::Authentication(_))
    }

    /// Check if this is a configuration error.
    ///
    /// Configuration errors are always terminal — they indicate invalid
    /// settings that cannot be resolved by retrying.
    #[must_use]
    pub fn is_config_error(&self) -> bool {
        matches!(self, Self::Config(_))
    }

    /// Check if this is a server error with a specific number.
    #[must_use]
    pub fn is_server_error(&self, number: i32) -> bool {
        matches!(self, Self::Server { number: n, .. } if *n == number)
    }

    /// Get the error class/severity if this is a server error.
    ///
    /// SQL Server error classes range from 0-25:
    /// - 0-10: Informational
    /// - 11-16: User errors
    /// - 17-19: Resource/hardware errors
    /// - 20-25: System errors (connection terminating)
    #[must_use]
    pub fn class(&self) -> Option<u8> {
        match self {
            Self::Server { class, .. } => Some(*class),
            _ => None,
        }
    }

    /// Alias for `class()` - returns error severity.
    #[must_use]
    pub fn severity(&self) -> Option<u8> {
        self.class()
    }
}

/// Format the server/procedure/line suffix for server error Display.
fn format_server_location(
    server: &Option<String>,
    procedure: &Option<String>,
    line: &u32,
) -> String {
    let mut parts = Vec::new();
    if let Some(srv) = server {
        if !srv.is_empty() {
            parts.push(format!("server: {srv}"));
        }
    }
    if let Some(proc) = procedure {
        if !proc.is_empty() {
            parts.push(format!("procedure: {proc}"));
        }
    }
    if *line > 0 {
        parts.push(format!("line: {line}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join(", "))
    }
}

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_server_error(number: i32) -> Error {
        Error::Server {
            number,
            class: 16,
            state: 1,
            message: "Test error".to_string(),
            server: None,
            procedure: None,
            line: 1,
        }
    }

    #[test]
    fn test_is_transient_connection_errors() {
        assert!(
            Error::ConnectTimeout {
                host: "test".into(),
                port: 1433
            }
            .is_transient()
        );
        assert!(
            Error::TlsTimeout {
                host: "test".into(),
                port: 1433
            }
            .is_transient()
        );
        assert!(
            Error::LoginTimeout {
                host: "test".into(),
                port: 1433
            }
            .is_transient()
        );
        assert!(Error::CommandTimeout.is_transient());
        assert!(Error::ConnectionClosed.is_transient());
        assert!(Error::PoolExhausted.is_transient());
        assert!(
            Error::Routing {
                host: "test".into(),
                port: 1433,
            }
            .is_transient()
        );
    }

    #[test]
    fn test_is_transient_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        assert!(Error::Io(SharedIoError(Arc::new(io_err))).is_transient());
    }

    #[test]
    fn test_is_transient_server_errors_deadlock() {
        // 1205 - Deadlock victim
        assert!(make_server_error(1205).is_transient());
    }

    #[test]
    fn test_is_transient_server_errors_timeout() {
        // -2 - Timeout
        assert!(make_server_error(-2).is_transient());
    }

    #[test]
    fn test_is_transient_server_errors_azure() {
        // Azure-specific transient errors
        assert!(make_server_error(10928).is_transient()); // Resource limit
        assert!(make_server_error(10929).is_transient()); // Resource limit
        assert!(make_server_error(40197).is_transient()); // Service error
        assert!(make_server_error(40501).is_transient()); // Service busy
        assert!(make_server_error(40613).is_transient()); // Database unavailable
        assert!(make_server_error(49918).is_transient()); // Cannot process request
        assert!(make_server_error(49919).is_transient()); // Cannot process create/update
        assert!(make_server_error(49920).is_transient()); // Cannot process request
    }

    #[test]
    fn test_is_transient_server_errors_other() {
        // Other transient errors
        assert!(make_server_error(4060).is_transient()); // Cannot open database
        assert!(make_server_error(18456).is_transient()); // Login failed (Azure failover)
    }

    #[test]
    fn test_is_not_transient() {
        // Non-transient errors
        assert!(!Error::Config("bad config".into()).is_transient());
        assert!(!Error::Query("syntax error".into()).is_transient());
        assert!(!Error::InvalidIdentifier("bad id".into()).is_transient());
        assert!(!make_server_error(102).is_transient()); // Syntax error
    }

    #[test]
    fn test_is_terminal_server_errors() {
        // Terminal SQL errors per ADR-009
        assert!(make_server_error(102).is_terminal()); // Syntax error
        assert!(make_server_error(207).is_terminal()); // Invalid column
        assert!(make_server_error(208).is_terminal()); // Invalid object
        assert!(make_server_error(547).is_terminal()); // Constraint violation
        assert!(make_server_error(2627).is_terminal()); // Unique constraint violation
        assert!(make_server_error(2601).is_terminal()); // Duplicate key
    }

    #[test]
    fn test_is_terminal_config_errors() {
        assert!(Error::Config("bad config".into()).is_terminal());
        assert!(Error::InvalidIdentifier("bad id".into()).is_terminal());
    }

    #[test]
    fn test_is_not_terminal() {
        // Non-terminal errors (may be transient or other)
        assert!(
            !Error::ConnectTimeout {
                host: "test".into(),
                port: 1433
            }
            .is_terminal()
        );
        assert!(!make_server_error(1205).is_terminal()); // Deadlock - transient, not terminal
        assert!(!make_server_error(40501).is_terminal()); // Service busy - transient
    }

    #[test]
    fn test_transient_server_error_static() {
        // Test the static helper function
        assert!(Error::is_transient_server_error(1205));
        assert!(Error::is_transient_server_error(40501));
        assert!(!Error::is_transient_server_error(102));
    }

    #[test]
    fn test_terminal_server_error_static() {
        // Test the static helper function
        assert!(Error::is_terminal_server_error(102));
        assert!(Error::is_terminal_server_error(2627));
        assert!(!Error::is_terminal_server_error(1205));
    }

    #[test]
    fn test_error_class() {
        let err = make_server_error(102);
        assert_eq!(err.class(), Some(16));
        assert_eq!(err.severity(), Some(16));

        assert_eq!(
            Error::ConnectTimeout {
                host: "test".into(),
                port: 1433
            }
            .class(),
            None
        );
    }

    #[test]
    fn test_is_server_error() {
        let err = make_server_error(102);
        assert!(err.is_server_error(102));
        assert!(!err.is_server_error(103));

        assert!(
            !Error::ConnectTimeout {
                host: "test".into(),
                port: 1433
            }
            .is_server_error(102)
        );
    }
}
