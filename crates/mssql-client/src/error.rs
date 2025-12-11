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
        /// Error severity.
        severity: u8,
        /// Error state.
        state: u8,
        /// Error message.
        message: String,
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
}

impl Error {
    /// Check if this error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionTimeout
                | Self::CommandTimeout
                | Self::ConnectionClosed
                | Self::Routing { .. }
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

    /// Get the error severity if this is a server error.
    #[must_use]
    pub fn severity(&self) -> Option<u8> {
        match self {
            Self::Server { severity, .. } => Some(*severity),
            _ => None,
        }
    }
}

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, Error>;
