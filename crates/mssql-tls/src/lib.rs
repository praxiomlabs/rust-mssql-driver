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

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod config;
pub mod connector;
pub mod error;

pub use config::TlsConfig;
pub use connector::TlsConnector;
pub use error::TlsError;
