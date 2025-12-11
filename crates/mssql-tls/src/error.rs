//! TLS-related error types.

use thiserror::Error;

/// Errors that can occur during TLS operations.
#[derive(Debug, Error)]
pub enum TlsError {
    /// TLS handshake failed.
    #[error("TLS handshake failed: {0}")]
    HandshakeFailed(String),

    /// Certificate validation failed.
    #[error("certificate validation failed: {0}")]
    CertificateValidation(String),

    /// Hostname verification failed.
    #[error("hostname verification failed: expected {expected}, got {actual}")]
    HostnameVerification {
        /// Expected hostname.
        expected: String,
        /// Actual hostname from certificate.
        actual: String,
    },

    /// Invalid certificate format.
    #[error("invalid certificate: {0}")]
    InvalidCertificate(String),

    /// Invalid private key format.
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),

    /// TLS configuration error.
    #[error("TLS configuration error: {0}")]
    Configuration(String),

    /// IO error during TLS operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Rustls error.
    #[error("rustls error: {0}")]
    Rustls(#[from] rustls::Error),

    /// Server requires encryption but client disabled it.
    #[error("server requires encryption")]
    EncryptionRequired,

    /// Client requires encryption but server doesn't support it.
    #[error("server does not support encryption")]
    EncryptionNotSupported,

    /// TDS 8.0 strict mode is required but not supported.
    #[error("TDS 8.0 strict mode required")]
    StrictModeRequired,

    /// Connection closed during TLS negotiation.
    #[error("connection closed during TLS negotiation")]
    ConnectionClosed,
}
