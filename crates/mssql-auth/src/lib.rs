//! # mssql-auth
//!
//! Authentication strategies for SQL Server connections.
//!
//! This crate provides various authentication methods, isolated from
//! connection logic for better modularity and testing.
//!
//! ## Supported Authentication Methods
//!
//! | Method | Feature Flag | Status | Description |
//! |--------|--------------|--------|-------------|
//! | SQL Authentication | default | ✅ Implemented | Username/password |
//! | Azure AD Token | default | ✅ Implemented | Pre-obtained access token |
//! | Azure Managed Identity | `azure-identity` | ✅ Implemented | VM/container identity |
//! | Service Principal | `azure-identity` | ✅ Implemented | App credentials |
//! | Integrated (Kerberos) | `integrated-auth` | ✅ Implemented | GSSAPI/Kerberos (Linux/macOS) |
//! | Windows SSPI | `sspi-auth` | ✅ Implemented | Native Windows SSPI |
//! | Certificate | `cert-auth` | ✅ Implemented | Entra service principal w/ X.509 cert |
//!
//! `CertificateAuth` acquires a token from Microsoft Entra using an X.509
//! client certificate; `mssql-client` wires `Credentials::Certificate` through
//! the FEDAUTH SecurityToken login. This authenticates to Entra — it is NOT
//! TDS-level mutual TLS (SQL Server does not accept client certificates at the
//! protocol level).
//!
//! The Azure AD methods use the FEDAUTH SecurityToken workflow: the token is
//! acquired client-side and sent in the LOGIN7 FEDAUTH feature extension
//! (see [`azure_ad::build_security_token_feature_data`]). The ADAL/MSAL
//! workflow (server-directed acquisition via FEDAUTHINFO) is #155 Phase 2.
//!
//! ## Authentication Tiers
//!
//! Per ARCHITECTURE.md, authentication is tiered:
//!
//! ### Tier 1 (Core - Pure Rust, Default)
//!
//! - [`SqlServerAuth`] - Username/password via Login7 ✅ Implemented
//! - [`AzureAdAuth`] - Pre-acquired access token ✅ Implemented
//!
//! ### Tier 2 (Azure Native - `azure-identity` feature) ✅ Implemented
//!
//! - `ManagedIdentityAuth` - Azure VM/Container identity
//! - `ServicePrincipalAuth` - Client ID + Secret
//!
//! ### Tier 3 (Enterprise - `integrated-auth` or `sspi-auth` feature) ✅ Implemented
//!
//! - `IntegratedAuth` - Kerberos (Linux/macOS via GSSAPI)
//! - `SspiAuth` - Windows SSPI (native Windows, cross-platform via sspi-rs)
//!
//! ### Tier 4 (Certificate - `cert-auth` feature) ✅ Implemented
//!
//! - `CertificateAuth` - Entra service principal authentication with an X.509
//!   certificate (authenticates to Entra, not TDS-level mTLS)
//!
//! ## Secure Credential Handling
//!
//! Enable the `zeroize` feature for secure credential handling:
//!
//! ```toml
//! mssql-auth = { version = "0.1", features = ["zeroize"] }
//! ```
//!
//! This enables secure credential handling that automatically zeroes
//! sensitive data from memory when dropped.
//!
//! ## Example
//!
//! ```rust
//! use mssql_auth::{SqlServerAuth, AzureAdAuth, AuthProvider};
//!
//! // SQL Server authentication
//! let sql_auth = SqlServerAuth::new("sa", "Password123!");
//! let auth_data = sql_auth.authenticate().unwrap();
//!
//! // Azure AD authentication with pre-acquired token
//! let azure_auth = AzureAdAuth::with_token("eyJ0eXAi...");
//! ```

#![warn(missing_docs)]
// Unsafe code is denied globally but allowed in the Windows CNG FFI module.
// See windows_certstore.rs for detailed SAFETY comments on each unsafe block.
#![deny(unsafe_code)]

pub mod azure_ad;
#[cfg(feature = "azure-identity")]
pub mod azure_identity_auth;
#[cfg(feature = "cert-auth")]
pub mod cert_auth;
pub mod credentials;
pub mod encryption;
pub mod error;
#[cfg(feature = "integrated-auth")]
pub mod integrated_auth;
#[cfg(all(windows, feature = "sspi-auth"))]
#[allow(unsafe_code)] // Windows SSPI FFI; see SAFETY comments in each unsafe block
pub mod native_sspi;
#[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
pub mod negotiator;
pub mod provider;
pub mod sql_auth;
#[cfg(feature = "sspi-auth")]
pub mod sspi_auth;

// Always Encrypted cryptography
#[cfg(feature = "always-encrypted")]
pub mod aead;
#[cfg(feature = "always-encrypted")]
pub mod cek_envelope;
#[cfg(feature = "always-encrypted")]
pub mod key_store;
#[cfg(feature = "always-encrypted")]
pub mod key_unwrap;

// Always Encrypted key providers
#[cfg(feature = "azure-keyvault")]
pub mod azure_keyvault;
#[cfg(all(windows, feature = "windows-certstore"))]
#[allow(unsafe_code)] // Windows CNG FFI; see SAFETY comments in each unsafe block
pub mod windows_certstore;

// Core types
pub use credentials::Credentials;
pub use error::AuthError;
pub use provider::{AsyncAuthProvider, AuthData, AuthMethod, AuthProvider};

// Authentication providers
pub use azure_ad::{AzureAdAuth, FedAuthLibrary};
pub use sql_auth::SqlServerAuth;

// Secure credential types (with zeroize feature)
#[cfg(feature = "zeroize")]
pub use credentials::{SecretString, SecureCredentials};

// Azure Identity authentication (with azure-identity feature)
#[cfg(feature = "azure-identity")]
pub use azure_identity_auth::{ManagedIdentityAuth, ServicePrincipalAuth};

// Integrated authentication (Kerberos/GSSAPI - with integrated-auth feature)
#[cfg(feature = "integrated-auth")]
pub use integrated_auth::IntegratedAuth;

// Certificate authentication (Azure AD with X.509 certificate - with cert-auth feature)
#[cfg(feature = "cert-auth")]
pub use cert_auth::CertificateAuth;

// Native Windows SSPI authentication (with sspi-auth feature, Windows only)
#[cfg(all(windows, feature = "sspi-auth"))]
pub use native_sspi::NativeSspiAuth;

// Windows SSPI authentication via sspi-rs (with sspi-auth feature)
#[cfg(feature = "sspi-auth")]
pub use sspi_auth::SspiAuth;

// SSPI/GSSAPI negotiator trait (with integrated-auth or sspi-auth feature)
#[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
pub use negotiator::SspiNegotiator;

// Always Encrypted infrastructure
pub use encryption::{
    CekMetadata, ColumnEncryptionConfig, ColumnEncryptionInfo, EncryptedValue, EncryptionError,
    EncryptionType, KeyStoreProvider,
};

// Always Encrypted cryptography (with always-encrypted feature)
#[cfg(feature = "always-encrypted")]
pub use aead::AeadEncryptor;
#[cfg(feature = "always-encrypted")]
pub use key_store::{CekCache, CekCacheKey, InMemoryKeyStore};
#[cfg(feature = "always-encrypted")]
pub use key_unwrap::RsaKeyUnwrapper;

// Always Encrypted key providers
#[cfg(feature = "azure-keyvault")]
pub use azure_keyvault::AzureKeyVaultProvider;
#[cfg(all(windows, feature = "windows-certstore"))]
pub use windows_certstore::WindowsCertStoreProvider;
