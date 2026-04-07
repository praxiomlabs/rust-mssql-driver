//! Authentication error types.

use thiserror::Error;

/// Errors that can occur during authentication.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuthError {
    /// Invalid credentials provided.
    #[error("invalid credentials: {0}")]
    InvalidCredentials(String),

    /// Authentication failed on server.
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Token expired or invalid.
    #[error("token expired or invalid")]
    TokenExpired,

    /// Token acquisition failed.
    #[error("failed to acquire token: {0}")]
    TokenAcquisition(String),

    /// Unsupported authentication method.
    #[error("unsupported authentication method: {0}")]
    UnsupportedMethod(String),

    /// SSPI/GSSAPI error.
    #[error("SSPI error: {0}")]
    Sspi(String),

    /// Certificate error.
    #[error("certificate error: {0}")]
    Certificate(String),

    /// Network error during authentication.
    #[error("network error: {0}")]
    Network(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Configuration(String),

    /// Azure identity error.
    #[error("Azure identity error: {0}")]
    AzureIdentity(String),
}

impl AuthError {
    /// Check if this error is transient and may succeed on retry.
    ///
    /// Network errors, token acquisition failures (may be temporary service
    /// issues), and Azure identity errors are potentially transient. Invalid
    /// credentials and unsupported methods are terminal.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::TokenAcquisition(_) | Self::AzureIdentity(_)
        )
    }

    /// Check if this error is terminal and will never succeed on retry.
    ///
    /// Invalid credentials, unsupported methods, certificate errors, and
    /// configuration errors are permanent.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::InvalidCredentials(_)
                | Self::UnsupportedMethod(_)
                | Self::Certificate(_)
                | Self::Configuration(_)
        )
    }
}
