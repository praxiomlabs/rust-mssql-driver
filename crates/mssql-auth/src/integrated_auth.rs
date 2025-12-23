//! Integrated authentication (Kerberos/SPNEGO) provider.
//!
//! This module provides Kerberos authentication using GSSAPI (Generic Security
//! Services Application Program Interface) for SQL Server connections on Linux
//! and macOS. Windows uses SSPI natively, but GSSAPI is compatible on the wire.
//!
//! ## Prerequisites
//!
//! - **Kerberos libraries**: libkrb5-dev (Debian/Ubuntu) or krb5-devel (RHEL/Fedora)
//! - **Valid Kerberos ticket**: Run `kinit user@REALM` before connecting
//! - **DNS/SPN configuration**: SQL Server SPN must be registered in AD
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_auth::IntegratedAuth;
//!
//! // Create authenticator for SQL Server
//! let auth = IntegratedAuth::new("sqlserver.example.com", 1433)?;
//!
//! // Start authentication (returns initial SPNEGO token)
//! let initial_token = auth.initialize()?;
//!
//! // Process server's response and get next token (if needed)
//! let response_token = auth.step(&server_token)?;
//! ```
//!
//! ## How It Works
//!
//! SQL Server integrated authentication uses SPNEGO (Simple and Protected
//! GSS-API Negotiation) as specified in [RFC 4178](https://tools.ietf.org/html/rfc4178).
//! The TDS protocol carries SSPI tokens in packet type 0x11.
//!
//! 1. Client sends Login7 with integrated auth flag
//! 2. Server responds with SSPI challenge
//! 3. Client processes challenge via GSSAPI, sends response
//! 4. Server validates and completes authentication

use std::sync::Mutex;

use libgssapi::{
    context::{ClientCtx, CtxFlags},
    credential::{Cred, CredUsage},
    name::Name,
    oid::{GSS_MECH_KRB5, GSS_NT_HOSTBASED_SERVICE, OidSet},
};

use crate::error::AuthError;
use crate::provider::{AuthData, AuthMethod, AuthProvider};

/// SPNEGO mechanism OID for negotiating authentication.
///
/// SPNEGO allows the client and server to negotiate which underlying
/// mechanism to use (typically Kerberos 5).
const GSS_MECH_SPNEGO: libgssapi::oid::Oid = libgssapi::oid::Oid::from_slice(&[
    0x2b, 0x06, 0x01, 0x05, 0x05, 0x02, // 1.3.6.1.5.5.2
]);

/// Integrated authentication provider using Kerberos/SPNEGO.
///
/// This provider implements GSSAPI-based authentication for SQL Server,
/// compatible with Windows integrated authentication (SSPI).
///
/// # Thread Safety
///
/// The GSSAPI context is wrapped in a Mutex for thread safety, though
/// authentication is typically single-threaded per connection.
pub struct IntegratedAuth {
    /// The target service principal name string (e.g., "MSSQLSvc/host:port").
    spn: String,
    /// GSSAPI client context, wrapped for interior mutability.
    context: Mutex<Option<ClientCtx>>,
    /// Whether authentication has completed.
    complete: Mutex<bool>,
}

impl IntegratedAuth {
    /// Create a new integrated authentication provider.
    ///
    /// # Arguments
    ///
    /// * `hostname` - The SQL Server hostname (must match SPN in Active Directory)
    /// * `port` - The SQL Server port (typically 1433)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let auth = IntegratedAuth::new("sqlserver.contoso.com", 1433);
    /// ```
    #[must_use]
    pub fn new(hostname: &str, port: u16) -> Self {
        // SQL Server service principal format: MSSQLSvc/hostname:port
        let spn = format!("MSSQLSvc/{}:{}", hostname, port);

        Self {
            spn,
            context: Mutex::new(None),
            complete: Mutex::new(false),
        }
    }

    /// Create with a custom service principal name.
    ///
    /// Use this when the SPN doesn't follow the standard format,
    /// such as when using a SQL Server alias or cluster name.
    ///
    /// # Arguments
    ///
    /// * `spn` - The full service principal name
    #[must_use]
    pub fn with_spn(spn: impl Into<String>) -> Self {
        Self {
            spn: spn.into(),
            context: Mutex::new(None),
            complete: Mutex::new(false),
        }
    }

    /// Create a GSSAPI Name from the stored SPN.
    fn create_service_name(&self) -> Result<Name, AuthError> {
        Name::new(self.spn.as_bytes(), Some(&GSS_NT_HOSTBASED_SERVICE))
            .map_err(|e| AuthError::Sspi(format!("Failed to create service name: {}", e)))
    }

    /// Initialize the GSSAPI context and get the initial token.
    ///
    /// This must be called first to start the authentication handshake.
    /// The returned token should be sent to the server.
    ///
    /// # Errors
    ///
    /// Returns an error if credential acquisition or context initialization fails.
    pub fn initialize(&self) -> Result<Vec<u8>, AuthError> {
        // Create service name from stored SPN
        let service_name = self.create_service_name()?;

        // Acquire default credentials from the Kerberos ticket cache
        let mut mechs = OidSet::new()
            .map_err(|e| AuthError::Sspi(format!("Failed to create OID set: {}", e)))?;

        // Add SPNEGO mechanism for negotiation
        mechs
            .add(&GSS_MECH_SPNEGO)
            .map_err(|e| AuthError::Sspi(format!("Failed to add SPNEGO mechanism: {}", e)))?;

        // Also add Kerberos as fallback
        mechs
            .add(&GSS_MECH_KRB5)
            .map_err(|e| AuthError::Sspi(format!("Failed to add Kerberos mechanism: {}", e)))?;

        let cred = Cred::acquire(None, None, CredUsage::Initiate, Some(&mechs))
            .map_err(|e| AuthError::Sspi(format!("Failed to acquire credentials: {}", e)))?;

        // Create client context with mutual authentication flag
        let mut ctx = ClientCtx::new(
            Some(cred),
            service_name,
            CtxFlags::GSS_C_MUTUAL_FLAG | CtxFlags::GSS_C_REPLAY_FLAG,
            Some(&GSS_MECH_SPNEGO),
        );

        // Get initial token
        let token = ctx
            .step(None, None)
            .map_err(|e| AuthError::Sspi(format!("Failed to initialize context: {}", e)))?
            .ok_or_else(|| {
                AuthError::Sspi("No initial token generated (context already complete?)".into())
            })?;

        // Store the context for subsequent steps
        let mut context_guard = self
            .context
            .lock()
            .map_err(|_| AuthError::Sspi("Failed to acquire context lock".into()))?;
        *context_guard = Some(ctx);

        Ok(token.to_vec())
    }

    /// Process a server token and generate a response.
    ///
    /// Call this method each time the server sends an SSPI token.
    /// If the return value is `None`, authentication is complete.
    ///
    /// # Arguments
    ///
    /// * `server_token` - The SSPI token received from the server
    ///
    /// # Errors
    ///
    /// Returns an error if the context step fails or the context
    /// hasn't been initialized.
    pub fn step(&self, server_token: &[u8]) -> Result<Option<Vec<u8>>, AuthError> {
        let mut context_guard = self
            .context
            .lock()
            .map_err(|_| AuthError::Sspi("Failed to acquire context lock".into()))?;

        let ctx = context_guard.as_mut().ok_or_else(|| {
            AuthError::Sspi("Context not initialized - call initialize() first".into())
        })?;

        match ctx.step(Some(server_token), None) {
            Ok(Some(token)) => Ok(Some(token.to_vec())),
            Ok(None) => {
                // Authentication complete
                let mut complete_guard = self
                    .complete
                    .lock()
                    .map_err(|_| AuthError::Sspi("Failed to acquire complete lock".into()))?;
                *complete_guard = true;
                Ok(None)
            }
            Err(e) => Err(AuthError::Sspi(format!("GSSAPI step failed: {}", e))),
        }
    }

    /// Check if authentication has completed successfully.
    pub fn is_complete(&self) -> bool {
        self.complete.lock().map(|guard| *guard).unwrap_or(false)
    }

    /// Get the negotiated mechanism OID (after authentication completes).
    ///
    /// This indicates which mechanism (Kerberos, NTLM, etc.) was actually used.
    pub fn negotiated_mechanism(&self) -> Option<String> {
        self.context.lock().ok().and_then(|guard| {
            guard.as_ref().map(|_ctx| {
                // Note: libgssapi doesn't expose mech_type directly
                // after negotiation in a convenient way
                "SPNEGO/Kerberos".to_string()
            })
        })
    }
}

impl std::fmt::Debug for IntegratedAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegratedAuth")
            .field("complete", &self.is_complete())
            .finish_non_exhaustive()
    }
}

impl AuthProvider for IntegratedAuth {
    fn method(&self) -> AuthMethod {
        AuthMethod::Integrated
    }

    fn authenticate(&self) -> Result<AuthData, AuthError> {
        // Generate initial SSPI blob
        let blob = self.initialize()?;
        Ok(AuthData::Sspi { blob })
    }
}

// Note: IntegratedAuth is not Clone because GSSAPI contexts are stateful
// and cannot be cloned. Each connection needs its own authenticator.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name_format() {
        // This test verifies the SPN format
        let auth = IntegratedAuth::new("sqlserver.example.com", 1433);
        assert_eq!(auth.spn, "MSSQLSvc/sqlserver.example.com:1433");
    }

    #[test]
    fn test_custom_spn() {
        let auth = IntegratedAuth::with_spn("MSSQLSvc/cluster.example.com:1433");
        assert_eq!(auth.spn, "MSSQLSvc/cluster.example.com:1433");
    }

    #[test]
    fn test_debug_output() {
        let auth = IntegratedAuth::new("test.example.com", 1433);
        let debug = format!("{:?}", auth);
        assert!(debug.contains("IntegratedAuth"));
    }

    #[test]
    fn test_is_complete_initially_false() {
        let auth = IntegratedAuth::new("test.example.com", 1433);
        assert!(!auth.is_complete());
    }
}
