//! # mssql-auth
//!
//! Authentication strategies for SQL Server connections.
//!
//! This crate provides various authentication methods, isolated from
//! connection logic for better modularity and testing.
//!
//! ## Supported Authentication Methods
//!
//! | Method | Feature Flag | Description |
//! |--------|--------------|-------------|
//! | SQL Authentication | default | Username/password |
//! | Azure AD Token | default | Pre-obtained access token |
//! | Azure Managed Identity | `azure-identity` | VM/container identity |
//! | Service Principal | `azure-identity` | App credentials |
//! | Integrated (Kerberos) | `integrated-auth` | GSSAPI/Kerberos |
//! | Certificate | `cert-auth` | Client certificate |
//!
//! ## Authentication Tiers
//!
//! Per ARCHITECTURE.md, authentication is tiered:
//!
//! ### Tier 1 (Core - Pure Rust, Default)
//!
//! - [`SqlServerAuth`] - Username/password via Login7
//! - [`AzureAdAuth`] - Pre-acquired access token
//!
//! ### Tier 2 (Azure Native - `azure-identity` feature)
//!
//! - Managed Identity (Azure VM/Container)
//! - Service Principal (Client ID + Secret)
//!
//! ### Tier 3 (Enterprise/Legacy - `integrated-auth` feature)
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
pub mod credentials;
pub mod error;
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
