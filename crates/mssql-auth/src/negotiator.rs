//! SSPI/GSSAPI negotiation trait for integrated authentication.
//!
//! This trait provides a common interface for both Windows SSPI (`SspiAuth`)
//! and Unix GSSAPI (`IntegratedAuth`) authentication providers, enabling
//! the client login flow to handle SSPI token exchange generically.

use crate::error::AuthError;

/// Trait for SSPI/GSSAPI token negotiation during login.
///
/// Both `SspiAuth` (Windows SSPI) and `IntegratedAuth` (Unix GSSAPI)
/// implement this trait, allowing the client connection code to drive
/// the multi-step authentication handshake without knowing which
/// underlying mechanism is in use.
pub trait SspiNegotiator: Send + Sync {
    /// Generate the initial authentication token.
    ///
    /// This token is included in the Login7 packet's SSPI data field.
    ///
    /// # Errors
    ///
    /// Returns an error if credential acquisition or context initialization fails.
    fn initialize(&self) -> Result<Vec<u8>, AuthError>;

    /// Process a server challenge token and generate a response.
    ///
    /// Returns `Some(token)` if more data needs to be sent, or `None`
    /// if authentication is complete.
    ///
    /// # Errors
    ///
    /// Returns an error if the negotiation step fails.
    fn step(&self, server_token: &[u8]) -> Result<Option<Vec<u8>>, AuthError>;

    /// Check whether the authentication handshake has completed.
    fn is_complete(&self) -> bool;
}
