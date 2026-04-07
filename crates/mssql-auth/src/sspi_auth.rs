//! Windows SSPI authentication provider.
//!
//! This module provides Windows-native Security Support Provider Interface (SSPI)
//! authentication for SQL Server connections. It supports both Windows integrated
//! authentication (current user) and explicit Windows credentials.
//!
//! ## Features
//!
//! - **Negotiate protocol**: Automatically selects between Kerberos and NTLM
//! - **Integrated auth**: Use current Windows login credentials
//! - **Explicit credentials**: Supply username/password for different account
//! - **Cross-platform**: Uses sspi-rs which works on Windows and emulates SSPI on Unix
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_auth::SspiAuth;
//!
//! // Use current Windows login (integrated auth)
//! let auth = SspiAuth::new("sqlserver.example.com", 1433)?;
//!
//! // Or with explicit credentials
//! let auth = SspiAuth::with_credentials(
//!     "sqlserver.example.com",
//!     1433,
//!     "DOMAIN\\username",
//!     "password",
//! )?;
//!
//! // Start authentication
//! let initial_token = auth.initialize()?;
//!
//! // Process server response
//! let response_token = auth.step(&server_token)?;
//! ```
//!
//! ## Wire Protocol
//!
//! SSPI tokens are exchanged using the TDS SSPI packet type (0x11).
//! The authentication follows the SPNEGO/Negotiate protocol:
//!
//! 1. Client sends Login7 packet with integrated auth flag
//! 2. Server responds with SSPI challenge token
//! 3. Client processes challenge, sends response token
//! 4. Server validates and completes authentication

use std::sync::Mutex;

use sspi::{
    AuthIdentity, BufferType, ClientRequestFlags, CredentialUse, Credentials, CredentialsBuffers,
    DataRepresentation, Negotiate, NegotiateConfig, SecurityBuffer, SecurityStatus, Sspi, SspiImpl,
    Username, ntlm::NtlmConfig,
};

use crate::error::AuthError;
use crate::provider::{AuthData, AuthMethod, AuthProvider};

/// Windows SSPI authentication provider.
///
/// This provider implements SSPI-based authentication for SQL Server,
/// supporting both integrated (current user) and explicit credential modes.
///
/// # Thread Safety
///
/// The SSPI context is wrapped in a Mutex for thread safety, though
/// authentication is typically single-threaded per connection.
pub struct SspiAuth {
    /// The target service principal name (e.g., "MSSQLSvc/host:port").
    spn: String,
    /// Optional explicit credentials (domain\user, password).
    credentials: Option<(String, String)>,
    /// The SSPI context state.
    context: Mutex<SspiContext>,
}

/// Internal SSPI context state.
struct SspiContext {
    /// The Negotiate SSP instance.
    negotiate: Negotiate,
    /// Acquired credentials handle.
    creds_handle: Option<CredentialsBuffers>,
    /// Whether authentication has completed.
    complete: bool,
}

/// Create a default Negotiate configuration using NTLM.
fn create_negotiate_config() -> NegotiateConfig {
    NegotiateConfig::new(
        Box::new(NtlmConfig::default()),
        // Allow Kerberos and NTLM, but not PKU2U
        Some("kerberos,ntlm".to_string()),
        // Client computer name (not critical for SQL Server auth)
        String::new(),
    )
}

impl SspiAuth {
    /// Create a new SSPI authentication provider for integrated auth.
    ///
    /// Uses the current Windows user's credentials.
    ///
    /// # Arguments
    ///
    /// * `hostname` - The SQL Server hostname
    /// * `port` - The SQL Server port (typically 1433)
    ///
    /// # Errors
    ///
    /// Returns an error if the Negotiate context cannot be created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let auth = SspiAuth::new("sqlserver.contoso.com", 1433)?;
    /// ```
    pub fn new(hostname: &str, port: u16) -> Result<Self, AuthError> {
        // SQL Server SPN format: MSSQLSvc/hostname:port
        let spn = format!("MSSQLSvc/{hostname}:{port}");

        let negotiate = Negotiate::new_client(create_negotiate_config())
            .map_err(|e| AuthError::Sspi(format!("Failed to create Negotiate context: {e}")))?;

        Ok(Self {
            spn,
            credentials: None,
            context: Mutex::new(SspiContext {
                negotiate,
                creds_handle: None,
                complete: false,
            }),
        })
    }

    /// Create a new SSPI authentication provider with explicit credentials.
    ///
    /// Use this when authenticating as a different user than the current
    /// Windows login.
    ///
    /// # Arguments
    ///
    /// * `hostname` - The SQL Server hostname
    /// * `port` - The SQL Server port
    /// * `username` - Username in "DOMAIN\\user" or "user@domain" format
    /// * `password` - Password for the user
    ///
    /// # Errors
    ///
    /// Returns an error if the Negotiate context cannot be created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let auth = SspiAuth::with_credentials(
    ///     "sqlserver.contoso.com",
    ///     1433,
    ///     "CONTOSO\\sqluser",
    ///     "MyP@ssw0rd",
    /// )?;
    /// ```
    pub fn with_credentials(
        hostname: &str,
        port: u16,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Self, AuthError> {
        let spn = format!("MSSQLSvc/{hostname}:{port}");

        let negotiate = Negotiate::new_client(create_negotiate_config())
            .map_err(|e| AuthError::Sspi(format!("Failed to create Negotiate context: {e}")))?;

        Ok(Self {
            spn,
            credentials: Some((username.into(), password.into())),
            context: Mutex::new(SspiContext {
                negotiate,
                creds_handle: None,
                complete: false,
            }),
        })
    }

    /// Create with a custom service principal name.
    ///
    /// Use when the SPN doesn't follow the standard format,
    /// such as when using a SQL Server alias or cluster name.
    ///
    /// # Arguments
    ///
    /// * `spn` - The full service principal name
    ///
    /// # Errors
    ///
    /// Returns an error if the Negotiate context cannot be created.
    pub fn with_spn(spn: impl Into<String>) -> Result<Self, AuthError> {
        let negotiate = Negotiate::new_client(create_negotiate_config())
            .map_err(|e| AuthError::Sspi(format!("Failed to create Negotiate context: {e}")))?;

        Ok(Self {
            spn: spn.into(),
            credentials: None,
            context: Mutex::new(SspiContext {
                negotiate,
                creds_handle: None,
                complete: false,
            }),
        })
    }

    /// Initialize the SSPI context and get the initial token.
    ///
    /// This must be called first to start the authentication handshake.
    /// The returned token should be sent to the server.
    ///
    /// # Errors
    ///
    /// Returns an error if credential acquisition or context initialization fails.
    pub fn initialize(&self) -> Result<Vec<u8>, AuthError> {
        let mut ctx = self
            .context
            .lock()
            .map_err(|_| AuthError::Sspi("Failed to acquire context lock".into()))?;

        // Acquire credentials
        let credentials = if let Some((ref username, ref password)) = self.credentials {
            // Parse username into domain and user parts
            let parsed_user = Username::parse(username)
                .map_err(|e| AuthError::Sspi(format!("Invalid username format: {e}")))?;

            let identity = AuthIdentity {
                username: parsed_user,
                password: password.clone().into(),
            };

            // Convert to Credentials enum
            Some(Credentials::from(identity))
        } else {
            None
        };

        let creds_result = {
            let mut builder = ctx
                .negotiate
                .acquire_credentials_handle()
                .with_credential_use(CredentialUse::Outbound);

            // Only add auth data if we have explicit credentials
            if let Some(ref creds) = credentials {
                builder = builder.with_auth_data(creds);
            }

            builder
                .execute(&mut ctx.negotiate)
                .map_err(|e| AuthError::Sspi(format!("Failed to acquire credentials: {e}")))?
        };

        // Store credentials handle (may be None for integrated auth)
        ctx.creds_handle = creds_result.credentials_handle;

        // Initialize security context
        // Take credentials handle temporarily to avoid overlapping mutable borrows
        let mut creds = ctx.creds_handle.take();
        let mut output_buffer = vec![SecurityBuffer::new(Vec::new(), BufferType::Token)];
        let spn = self.spn.clone();

        let mut builder = ctx
            .negotiate
            .initialize_security_context()
            .with_credentials_handle(&mut creds)
            .with_context_requirements(
                ClientRequestFlags::MUTUAL_AUTH
                    | ClientRequestFlags::REPLAY_DETECT
                    | ClientRequestFlags::SEQUENCE_DETECT,
            )
            .with_target_data_representation(DataRepresentation::Native)
            .with_target_name(&spn)
            .with_output(&mut output_buffer);

        let init_result = ctx
            .negotiate
            .initialize_security_context_impl(&mut builder)
            .map_err(|e| AuthError::Sspi(format!("Failed to initialize context: {e}")))?
            .resolve_to_result()
            .map_err(|e| AuthError::Sspi(format!("Failed to resolve context: {e}")))?;

        // Put credentials handle back
        ctx.creds_handle = creds;

        // Check result status
        match init_result.status {
            SecurityStatus::Ok | SecurityStatus::ContinueNeeded => {
                if init_result.status == SecurityStatus::Ok {
                    ctx.complete = true;
                }

                // Return the output token
                let token = output_buffer
                    .into_iter()
                    .find(|b| b.buffer_type.buffer_type == BufferType::Token)
                    .map(|b| b.buffer)
                    .unwrap_or_default();

                Ok(token)
            }
            status => Err(AuthError::Sspi(format!(
                "Unexpected status during initialization: {status:?}"
            ))),
        }
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
        let mut ctx = self
            .context
            .lock()
            .map_err(|_| AuthError::Sspi("Failed to acquire context lock".into()))?;

        if ctx.complete {
            return Ok(None);
        }

        if ctx.creds_handle.is_none() {
            return Err(AuthError::Sspi(
                "Context not initialized - call initialize() first".into(),
            ));
        }

        // Set up input and output buffers
        let mut input_buffer = vec![SecurityBuffer::new(
            server_token.to_vec(),
            BufferType::Token,
        )];
        let mut output_buffer = vec![SecurityBuffer::new(Vec::new(), BufferType::Token)];
        let spn = self.spn.clone();

        // Take credentials handle temporarily to avoid overlapping mutable borrows
        let mut creds = ctx.creds_handle.take();

        let mut builder = ctx
            .negotiate
            .initialize_security_context()
            .with_credentials_handle(&mut creds)
            .with_context_requirements(
                ClientRequestFlags::MUTUAL_AUTH
                    | ClientRequestFlags::REPLAY_DETECT
                    | ClientRequestFlags::SEQUENCE_DETECT,
            )
            .with_target_data_representation(DataRepresentation::Native)
            .with_target_name(&spn)
            .with_input(&mut input_buffer)
            .with_output(&mut output_buffer);

        let result = ctx
            .negotiate
            .initialize_security_context_impl(&mut builder)
            .map_err(|e| AuthError::Sspi(format!("SSPI step failed: {e}")))?
            .resolve_to_result()
            .map_err(|e| AuthError::Sspi(format!("Failed to resolve step result: {e}")))?;

        // Put credentials handle back
        ctx.creds_handle = creds;

        match result.status {
            SecurityStatus::Ok => {
                ctx.complete = true;
                // Return final token if there is one
                let token = output_buffer
                    .into_iter()
                    .find(|b| {
                        b.buffer_type.buffer_type == BufferType::Token && !b.buffer.is_empty()
                    })
                    .map(|b| b.buffer);
                Ok(token)
            }
            SecurityStatus::ContinueNeeded => {
                let token = output_buffer
                    .into_iter()
                    .find(|b| b.buffer_type.buffer_type == BufferType::Token)
                    .map(|b| b.buffer)
                    .unwrap_or_default();
                Ok(Some(token))
            }
            status => Err(AuthError::Sspi(format!(
                "Unexpected status during step: {status:?}"
            ))),
        }
    }

    /// Check if authentication has completed successfully.
    pub fn is_complete(&self) -> bool {
        self.context.lock().map(|ctx| ctx.complete).unwrap_or(false)
    }

    /// Get the target SPN.
    #[must_use]
    pub fn spn(&self) -> &str {
        &self.spn
    }
}

impl std::fmt::Debug for SspiAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SspiAuth")
            .field("spn", &self.spn)
            .field("has_explicit_credentials", &self.credentials.is_some())
            .field("complete", &self.is_complete())
            .finish()
    }
}

impl crate::negotiator::SspiNegotiator for SspiAuth {
    fn initialize(&self) -> Result<Vec<u8>, AuthError> {
        SspiAuth::initialize(self)
    }

    fn step(&self, server_token: &[u8]) -> Result<Option<Vec<u8>>, AuthError> {
        SspiAuth::step(self, server_token)
    }

    fn is_complete(&self) -> bool {
        SspiAuth::is_complete(self)
    }
}

impl AuthProvider for SspiAuth {
    fn method(&self) -> AuthMethod {
        AuthMethod::Integrated
    }

    fn authenticate(&self) -> Result<AuthData, AuthError> {
        // Generate initial SSPI blob
        let blob = self.initialize()?;
        Ok(AuthData::Sspi { blob })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_spn_format() {
        let auth = SspiAuth::new("sqlserver.example.com", 1433).unwrap();
        assert_eq!(auth.spn(), "MSSQLSvc/sqlserver.example.com:1433");
    }

    #[test]
    fn test_custom_spn() {
        let auth = SspiAuth::with_spn("MSSQLSvc/cluster.example.com:1433").unwrap();
        assert_eq!(auth.spn(), "MSSQLSvc/cluster.example.com:1433");
    }

    #[test]
    fn test_debug_output() {
        let auth = SspiAuth::new("test.example.com", 1433).unwrap();
        let debug = format!("{auth:?}");
        assert!(debug.contains("SspiAuth"));
        assert!(debug.contains("test.example.com"));
    }

    #[test]
    fn test_is_complete_initially_false() {
        let auth = SspiAuth::new("test.example.com", 1433).unwrap();
        assert!(!auth.is_complete());
    }

    #[test]
    fn test_with_credentials() {
        let auth = SspiAuth::with_credentials("test.example.com", 1433, "DOMAIN\\user", "password")
            .unwrap();
        let debug = format!("{auth:?}");
        assert!(debug.contains("has_explicit_credentials: true"));
    }
}
