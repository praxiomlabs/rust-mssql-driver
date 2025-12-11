//! Client error types.

use thiserror::Error;

/// Errors that can occur during client operations.
#[derive(Debug, Error)]
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
    #[error("TLS error: {0}")]
    Tls(#[from] mssql_tls::TlsError),

    /// Protocol error.
    #[error("protocol error: {0}")]
    Protocol(#[from] tds_protocol::ProtocolError),

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
    #[error("server error {number}: {message}")]
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

    /// Transaction error.
    #[error("transaction error: {0}")]
    Transaction(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Connection timeout occurred.
    #[error("connection timed out")]
    ConnectionTimeout,

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

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid identifier (potential SQL injection attempt).
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),

    /// Connection pool exhausted.
    #[error("connection pool exhausted")]
    PoolExhausted,
}

impl Error {
    /// Check if this error is transient and may succeed on retry.
    ///
    /// Transient errors include timeouts, connection issues, and
    /// certain server errors that may resolve themselves.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::ConnectionTimeout
                | Self::CommandTimeout
                | Self::ConnectionClosed
                | Self::Routing { .. }
                | Self::PoolExhausted
                | Self::Io(_)
        )
    }

    /// Check if this error indicates a protocol/driver bug.
    ///
    /// Protocol errors typically indicate a bug in the driver implementation
    /// rather than a user error or server issue.
    #[must_use]
    pub fn is_protocol_error(&self) -> bool {
        matches!(self, Self::Protocol(_))
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

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, Error>;
