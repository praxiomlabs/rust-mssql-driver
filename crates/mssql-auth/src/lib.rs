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
//! | Azure Managed Identity | `azure-identity` | ⏳ Planned v0.2 | VM/container identity |
//! | Service Principal | `azure-identity` | ⏳ Planned v0.2 | App credentials |
//! | Integrated (Kerberos) | `integrated-auth` | ⏳ Planned v0.2 | GSSAPI/Kerberos |
//! | Certificate | `cert-auth` | ⏳ Planned | Client certificate |
//!
//! ## Authentication Tiers
//!
//! Per ARCHITECTURE.md, authentication is tiered:
//!
//! ### Tier 1 (Core - Pure Rust, Default) ✅ Implemented
//!
//! - [`SqlServerAuth`] - Username/password via Login7
//! - [`AzureAdAuth`] - Pre-acquired access token
//!
//! ### Tier 2 (Azure Native - `azure-identity` feature) ⏳ Planned for v0.2.0
//!
//! - Managed Identity (Azure VM/Container)
//! - Service Principal (Client ID + Secret)
//!
//! ### Tier 3 (Enterprise/Legacy - `integrated-auth` feature) ⏳ Planned for v0.2.0
//!
//! - Kerberos (Linux/macOS via GSSAPI)
//! - NTLM/Kerberos (Windows via SSPI)
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
#![deny(unsafe_code)]

pub mod azure_ad;
#[cfg(feature = "azure-identity")]
pub mod azure_identity_auth;
pub mod credentials;
pub mod error;
#[cfg(feature = "integrated-auth")]
pub mod integrated_auth;
pub mod provider;
pub mod sql_auth;

// Core types
pub use credentials::Credentials;
pub use error::AuthError;
pub use provider::{AsyncAuthProvider, AuthData, AuthMethod, AuthProvider};

// Authentication providers
pub use azure_ad::{AzureAdAuth, FedAuthLibrary};
pub use sql_auth::SqlServerAuth;

// Legacy API (deprecated)
#[allow(deprecated)]
pub use sql_auth::SqlAuthenticator;

// Secure credential types (with zeroize feature)
#[cfg(feature = "zeroize")]
pub use credentials::{SecretString, SecureCredentials};

// Azure Identity authentication (with azure-identity feature)
#[cfg(feature = "azure-identity")]
pub use azure_identity_auth::{ManagedIdentityAuth, ServicePrincipalAuth};

// Integrated authentication (Kerberos/GSSAPI - with integrated-auth feature)
#[cfg(feature = "integrated-auth")]
pub use integrated_auth::IntegratedAuth;
