//! # mssql-tls
//!
//! TLS negotiation layer for SQL Server connections.
//!
//! This crate handles the complexity of TLS negotiation for both TDS 7.x
//! (pre-login encryption negotiation) and TDS 8.0 (strict TLS-first mode).
//!
//! ## TDS Version Differences
//!
//! ### TDS 7.x (SQL Server 2019 and earlier)
//! ```text
//! TCP Connect → PreLogin (cleartext) → TLS Handshake → Login7 (encrypted)
//! ```
//!
//! ### TDS 8.0 (SQL Server 2022+ strict mode)
//! ```text
//! TCP Connect → TLS Handshake → PreLogin (encrypted) → Login7 (encrypted)
//! ```
//!
//! ## Features
//!
//! - TLS 1.2 and TLS 1.3 support via rustls
//! - Server certificate validation
//! - Hostname verification
//! - Custom certificate authority support
//! - Client certificate authentication (TDS 8.0)
//!
//! ## Security
//!
//! By default, this crate validates server certificates using the Mozilla
//! root certificate store. The `TrustServerCertificate` option disables
//! validation but logs a warning - this should only be used for development.
//!
//! ```rust,ignore
//! use mssql_tls::{TlsConfig, TlsConnector, default_tls_config};
//!
//! // Secure default configuration
//! let config = default_tls_config()?;
//!
//! // Or use the builder pattern
//! let tls_config = TlsConfig::new()
//!     .strict_mode(true)  // TDS 8.0
//!     .min_protocol_version(TlsVersion::Tls13);
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod config;
pub mod connector;
pub mod error;

pub use config::{ClientAuth, TlsConfig, TlsVersion};
pub use connector::{default_tls_config, TlsConnector};
pub use error::TlsError;

// Re-export tokio-rustls stream type for convenience
pub use tokio_rustls::client::TlsStream;

/// TDS TLS negotiation mode.
///
/// This determines when TLS handshake occurs relative to TDS protocol messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TlsNegotiationMode {
    /// TDS 7.x style: TLS handshake occurs after PreLogin exchange.
    ///
    /// ```text
    /// TCP Connect → PreLogin (cleartext) → TLS Handshake → Login7 (encrypted)
    /// ```
    ///
    /// This is the default for SQL Server 2019 and earlier, and for
    /// SQL Server 2022+ when `Encrypt=true` (not strict).
    PostPreLogin,

    /// TDS 8.0 strict mode: TLS handshake occurs immediately after TCP connect.
    ///
    /// ```text
    /// TCP Connect → TLS Handshake → PreLogin (encrypted) → Login7 (encrypted)
    /// ```
    ///
    /// This is required for SQL Server 2022+ when `Encrypt=strict` is set.
    /// All TDS traffic is encrypted, including the PreLogin packet.
    Strict,
}

impl TlsNegotiationMode {
    /// Check if this mode requires TLS before any TDS traffic.
    #[must_use]
    pub fn is_tls_first(&self) -> bool {
        matches!(self, Self::Strict)
    }

    /// Check if PreLogin is sent in cleartext.
    #[must_use]
    pub fn prelogin_encrypted(&self) -> bool {
        matches!(self, Self::Strict)
    }

    /// Get the mode from encryption settings.
    ///
    /// # Arguments
    ///
    /// * `encrypt_strict` - Whether `Encrypt=strict` is set (TDS 8.0)
    #[must_use]
    pub fn from_encrypt_mode(encrypt_strict: bool) -> Self {
        if encrypt_strict {
            Self::Strict
        } else {
            Self::PostPreLogin
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiation_mode_strict() {
        let mode = TlsNegotiationMode::Strict;
        assert!(mode.is_tls_first());
        assert!(mode.prelogin_encrypted());
    }

    #[test]
    fn test_negotiation_mode_post_prelogin() {
        let mode = TlsNegotiationMode::PostPreLogin;
        assert!(!mode.is_tls_first());
        assert!(!mode.prelogin_encrypted());
    }

    #[test]
    fn test_from_encrypt_mode() {
        assert_eq!(
            TlsNegotiationMode::from_encrypt_mode(true),
            TlsNegotiationMode::Strict
        );
        assert_eq!(
            TlsNegotiationMode::from_encrypt_mode(false),
            TlsNegotiationMode::PostPreLogin
        );
    }
}
