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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_transient_errors() {
        assert!(AuthError::Network("connection reset".into()).is_transient());
        assert!(AuthError::TokenAcquisition("timeout".into()).is_transient());
        assert!(AuthError::AzureIdentity("service unavailable".into()).is_transient());
    }

    #[test]
    fn test_terminal_errors() {
        assert!(AuthError::InvalidCredentials("bad password".into()).is_terminal());
        assert!(AuthError::UnsupportedMethod("NTLM".into()).is_terminal());
        assert!(AuthError::Certificate("expired cert".into()).is_terminal());
        assert!(AuthError::Configuration("missing field".into()).is_terminal());
    }

    #[test]
    fn test_transient_terminal_mutual_exclusion() {
        // Transient errors should not be terminal
        assert!(!AuthError::Network("err".into()).is_terminal());
        assert!(!AuthError::TokenAcquisition("err".into()).is_terminal());
        assert!(!AuthError::AzureIdentity("err".into()).is_terminal());

        // Terminal errors should not be transient
        assert!(!AuthError::InvalidCredentials("err".into()).is_transient());
        assert!(!AuthError::UnsupportedMethod("err".into()).is_transient());
        assert!(!AuthError::Certificate("err".into()).is_transient());
        assert!(!AuthError::Configuration("err".into()).is_transient());
    }

    #[test]
    fn test_ambiguous_errors_classified() {
        // Errors that are neither transient nor terminal
        // (i.e., require case-by-case handling)
        let sspi = AuthError::Sspi("negotiate failed".into());
        assert!(!sspi.is_transient());
        assert!(!sspi.is_terminal());

        let expired = AuthError::TokenExpired;
        assert!(!expired.is_transient());
        assert!(!expired.is_terminal());

        let auth_failed = AuthError::AuthenticationFailed("bad user".into());
        assert!(!auth_failed.is_transient());
        assert!(!auth_failed.is_terminal());
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            AuthError::InvalidCredentials("no password".into()).to_string(),
            "invalid credentials: no password"
        );
        assert_eq!(
            AuthError::TokenExpired.to_string(),
            "token expired or invalid"
        );
        assert_eq!(
            AuthError::Sspi("ctx init".into()).to_string(),
            "SSPI error: ctx init"
        );
        assert_eq!(
            AuthError::Configuration("missing host".into()).to_string(),
            "configuration error: missing host"
        );
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AuthError>();
    }
}
