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
//! - Optional OS/platform trust store (`native-certs` feature)
//!
//! ## Security
//!
//! By default, this crate validates server certificates using the Mozilla
//! root certificate store. The `TrustServerCertificate` option disables
//! validation but logs a warning - this should only be used for development.
//!
//! Enterprise deployments often issue server certificates from an internal CA
//! that lives in the operating system trust store rather than Mozilla's list.
//! Enable the `native-certs` feature to delegate validation to the OS/platform
//! verifier (Windows CryptoAPI, macOS SecTrust, Linux native certs), which
//! honors those internal CAs along with OS revocation and policy. Explicit
//! [`TlsConfig::add_root_certificate`] roots always take precedence over the
//! OS store.
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
//!
//! ## Encryption modes
//!
//! The driver's `Encrypt` connection-string setting maps onto this crate as:
//!
//! | `Encrypt` | TLS | Negotiation | Notes |
//! |-----------|-----|-------------|-------|
//! | `strict` | Yes | [`TlsNegotiationMode::Strict`] (TDS 8.0) | All traffic encrypted, including PreLogin. SQL Server 2022+ only. |
//! | `true` / `mandatory` | Yes | [`TlsNegotiationMode::PostPreLogin`] | PreLogin is cleartext; Login7 onward is encrypted. |
//! | `no_tls` | No | — | No TLS at all; credentials travel in plaintext. |
//!
//! `no_tls` exists only for legacy SQL Server (2008-2016) that cannot negotiate
//! TLS 1.2+. rustls does not support TLS 1.0/1.1, so those servers cannot use
//! TLS through this driver — use `no_tls` only on a trusted network.
//!
//! ## SQL Server version requirements
//!
//! | Version | TLS support | TDS 8.0 strict |
//! |---------|-------------|----------------|
//! | 2008-2016 | TLS 1.0/1.1 by default — use `no_tls`, or configure the server for 1.2 | No |
//! | 2017-2019 | TLS 1.2 | No |
//! | 2022+ | TLS 1.2 / 1.3 | Yes |
//! | Azure SQL | TLS 1.2 minimum | Varies |
//!
//! ## Certificate validation
//!
//! By default the server certificate is validated against the Mozilla root CA
//! store, must be unexpired, and must match the server hostname.
//!
//! ```rust,no_run
//! use mssql_tls::{TlsConfig, TlsVersion};
//!
//! let _config = TlsConfig::new()
//!     .min_protocol_version(TlsVersion::Tls12)
//!     .max_protocol_version(TlsVersion::Tls13)
//!     .strict_mode(true); // TDS 8.0
//! ```
//!
//! For an internal CA or self-signed certificate, add the CA with
//! [`TlsConfig::add_root_certificate`]; override the verified hostname with
//! [`TlsConfig::with_server_name`]. [`TlsConfig::trust_server_certificate`]
//! disables validation entirely and is for development only — it logs a warning
//! and leaves the connection open to man-in-the-middle attacks.
//!
//! ## Troubleshooting
//!
//! - **`certificate verify failed`** — self-signed/internal cert (add the CA),
//!   expired cert, or hostname mismatch (set [`TlsConfig::with_server_name`]).
//! - **TLS handshake times out** — a firewall on port 1433, or the server is
//!   not configured for encryption.
//! - **`handshake failure` / no shared protocol** — the server requires a TLS
//!   version outside the configured range; widen it with
//!   [`TlsConfig::min_protocol_version`] / [`TlsConfig::max_protocol_version`].
//! - **Strict mode rejected** — `Encrypt=strict` requires SQL Server 2022+; use
//!   `Encrypt=true` for older servers.
//!
//! ## Security recommendations
//!
//! For production: `Encrypt=true` or `strict`, `TrustServerCertificate=false`,
//! TLS 1.2 minimum, and a server certificate from a trusted CA whose name
//! matches the host. PCI DSS, HIPAA, SOC 2, and FedRAMP all require TLS 1.2+
//! (FedRAMP additionally requires FIPS 140-2 validated cryptography).

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod config;
pub mod connector;
pub mod error;
pub mod prelogin_wrapper;

pub use config::{ClientAuth, TlsConfig, TlsVersion};
pub use connector::{TlsConnector, default_tls_config};
pub use error::TlsError;
pub use prelogin_wrapper::TlsPreloginWrapper;

// Re-export tokio-rustls stream type for convenience
pub use tokio_rustls::client::TlsStream;

// Re-export rustls PKI types so users can construct TLS configs without adding
// a direct dependency on the `rustls` crate. Changing these re-exports is a
// semver-breaking change (this crate is coupled to rustls 0.23.x).
pub use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// TDS TLS negotiation mode.
///
/// This determines when TLS handshake occurs relative to TDS protocol messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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
