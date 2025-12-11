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

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod credentials;
pub mod error;
pub mod sql_auth;

pub use credentials::Credentials;
pub use error::AuthError;
pub use sql_auth::SqlAuthenticator;
