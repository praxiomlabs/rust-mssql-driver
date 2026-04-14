//! Native Windows SSPI authentication provider.
//!
//! This module uses the actual Windows Security Support Provider Interface (SSPI)
//! via `secur32.dll` to perform authentication. Unlike the sspi-rs based provider
//! in [`super::sspi_auth`], this module delegates to the Windows kernel's credential
//! management, which supports all account types transparently:
//!
//! - **Domain accounts** (e.g., `CONTOSO\user`)
//! - **Local accounts** (e.g., `MACHINE\user`)
//! - **Microsoft Accounts** (e.g., `user@outlook.com` linked to Windows 11)
//!
//! This is the mechanism that .NET's `SqlClient` and ODBC use for
//! `Integrated Security=SSPI` on Windows.
//!
//! ## Why Native SSPI?
//!
//! The sspi-rs crate provides a pure-Rust SSPI implementation, but it cannot
//! acquire credentials from the current Windows logon session without explicit
//! username/password. The Windows SSPI subsystem (`LSASS`) has direct access to
//! cached credentials for all supported account types, including Microsoft Accounts
//! where the NTLM hash is derived from the cloud credential cache.
//!
//! ## Security
//!
//! All credential access is mediated by the Windows SSPI subsystem. This module
//! never handles raw passwords or credential material — it only receives opaque
//! security tokens from the Windows APIs.
//!
//! ## Platform
//!
//! This module is only available on Windows with the `sspi-auth` feature enabled.

use std::sync::Mutex;

use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Security::Authentication::Identity::{
    AcquireCredentialsHandleW, DeleteSecurityContext, FreeCredentialsHandle,
    InitializeSecurityContextW, SecBuffer, SecBufferDesc, ISC_REQ_CONNECTION,
    ISC_REQ_MUTUAL_AUTH, ISC_REQ_REPLAY_DETECT, ISC_REQ_SEQUENCE_DETECT,
    SECPKG_CRED_OUTBOUND,
};
use windows::Win32::Security::Credentials::SecHandle;

use crate::error::AuthError;

/// Token buffer type constant (SECBUFFER_TOKEN = 2).
const SECBUFFER_TOKEN: u32 = 2;

/// SecBufferDesc version constant (SECBUFFER_VERSION = 0).
const SECBUFFER_VERSION: u32 = 0;

/// Native data representation (SECURITY_NATIVE_DREP = 0x10).
const SECURITY_NATIVE_DREP: u32 = 0x10;

/// Maximum SSPI token size (16 KiB is more than sufficient for Negotiate/NTLM).
const MAX_TOKEN_SIZE: usize = 16_384;

/// HRESULT value for successful completion.
const SEC_E_OK: HRESULT = HRESULT(0);

/// HRESULT value indicating more data exchange is needed.
const SEC_I_CONTINUE_NEEDED: HRESULT = HRESULT(0x0009_0312_u32 as i32);

/// Native Windows SSPI authentication provider.
///
/// Uses the Windows SSPI subsystem (`secur32.dll`) for integrated authentication.
/// This supports all Windows account types including Microsoft Accounts, domain
/// accounts, and local accounts — without requiring explicit credentials.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_auth::NativeSspiAuth;
///
/// let auth = NativeSspiAuth::new("sqlserver.example.com", 1433)?;
/// let initial_token = auth.initialize()?;
/// // Send initial_token to server, receive challenge...
/// let response = auth.step(&server_challenge)?;
/// ```
pub struct NativeSspiAuth {
    /// The target service principal name (e.g., "MSSQLSvc/host:port").
    spn: String,
    /// Internal SSPI context state, protected by a mutex for thread safety.
    context: Mutex<NativeSspiContext>,
}

/// Internal state for the native SSPI handshake.
struct NativeSspiContext {
    /// Credential handle acquired from `AcquireCredentialsHandleW`.
    cred_handle: SecHandle,
    /// Security context handle from `InitializeSecurityContextW`.
    ctx_handle: SecHandle,
    /// Whether the context handle has been initialized (first ISC call made).
    has_context: bool,
    /// Whether authentication has completed successfully.
    complete: bool,
}

// SAFETY: SecHandle is a pair of pointer-sized integers (dwLower, dwUpper)
// that act as opaque handles. The Windows SSPI subsystem manages thread safety
// for the underlying resources. We protect access with a Mutex.
unsafe impl Send for NativeSspiContext {}

impl NativeSspiAuth {
    /// Create a new native SSPI authentication provider for integrated auth.
    ///
    /// Uses the current Windows user's credentials (from the logon session).
    /// This works for domain accounts, local accounts, and Microsoft Accounts.
    ///
    /// # Arguments
    ///
    /// * `hostname` - The SQL Server hostname
    /// * `port` - The SQL Server port (typically 1433)
    ///
    /// # Errors
    ///
    /// Returns an error if credential acquisition fails.
    pub fn new(hostname: &str, port: u16) -> Result<Self, AuthError> {
        let spn = format!("MSSQLSvc/{hostname}:{port}");

        let mut cred_handle = SecHandle::default();
        let mut expiry: i64 = 0;

        // Encode package name as null-terminated wide string
        let package: Vec<u16> = "Negotiate\0".encode_utf16().collect();

        // SAFETY: AcquireCredentialsHandleW is called with:
        // - pszprincipal: NULL (current user)
        // - pszpackage: "Negotiate" (well-known SSP name, null-terminated wide string)
        // - fCredentialUse: OUTBOUND (client-side)
        // - pvLogonId: NULL (current logon session)
        // - pAuthData: NULL (use current user's cached credentials)
        // - pGetKeyFn/pvGetKeyArgument: NULL (not used)
        // - phCredential: valid mutable pointer to our handle
        // - ptsExpiry: valid mutable pointer
        //
        // The `package` Vec lives for the duration of this call.
        // The output cred_handle is an opaque handle that must be freed
        // with FreeCredentialsHandle, which we do in Drop.
        let result = unsafe {
            AcquireCredentialsHandleW(
                None,                            // principal: current user
                PCWSTR(package.as_ptr()),         // package: "Negotiate"
                SECPKG_CRED_OUTBOUND,            // credential use: client-side
                None,                            // logon id: current session
                None,                            // auth data: current user creds
                None,                            // get key fn: not used
                None,                            // get key arg: not used
                &mut cred_handle,                // output: credential handle
                Some(&mut expiry),               // output: expiry time
            )
        };

        if let Err(e) = result {
            return Err(AuthError::Sspi(format!(
                "Failed to acquire Windows credentials: {e}"
            )));
        }

        tracing::debug!(
            spn = %spn,
            "Acquired native Windows SSPI credentials for current user"
        );

        Ok(Self {
            spn,
            context: Mutex::new(NativeSspiContext {
                cred_handle,
                ctx_handle: SecHandle::default(),
                has_context: false,
                complete: false,
            }),
        })
    }

    /// Initialize the SSPI context and generate the first authentication token.
    ///
    /// This token should be included in the Login7 packet's SSPI data field.
    ///
    /// # Errors
    ///
    /// Returns an error if context initialization fails.
    pub fn initialize(&self) -> Result<Vec<u8>, AuthError> {
        let mut ctx = self
            .context
            .lock()
            .map_err(|_| AuthError::Sspi("Failed to acquire context lock".into()))?;

        // Encode SPN as wide string (null-terminated)
        let spn_wide: Vec<u16> = format!("{}\0", self.spn).encode_utf16().collect();

        // Set up output buffer
        let mut out_buf = vec![0u8; MAX_TOKEN_SIZE];
        let mut out_sec_buf = SecBuffer {
            cbBuffer: out_buf.len() as u32,
            BufferType: SECBUFFER_TOKEN,
            pvBuffer: out_buf.as_mut_ptr().cast(),
        };
        let mut out_buf_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut out_sec_buf,
        };

        let mut context_attrs: u32 = 0;
        let mut expiry: i64 = 0;

        let context_req = ISC_REQ_MUTUAL_AUTH
            | ISC_REQ_REPLAY_DETECT
            | ISC_REQ_SEQUENCE_DETECT
            | ISC_REQ_CONNECTION;

        // SAFETY: InitializeSecurityContextW is called with:
        // - phCredential: valid credential handle from AcquireCredentialsHandleW
        // - phContext: NULL (first call, no existing context)
        // - pszTargetName: null-terminated wide SPN string
        // - fContextReq: standard flags for SQL Server auth
        // - Reserved1/Reserved2: 0
        // - TargetDataRep: SECURITY_NATIVE_DREP
        // - pInput: NULL (first call, no input token)
        // - phNewContext: valid mutable pointer for output context
        // - pOutput: valid SecBufferDesc with pre-allocated buffer
        // - pfContextAttr: valid mutable pointer
        // - ptsExpiry: valid mutable pointer
        //
        // On success, ctx_handle receives the new security context handle
        // and out_sec_buf.cbBuffer is updated with the actual token size.
        let hr = unsafe {
            InitializeSecurityContextW(
                Some(&ctx.cred_handle),          // credential handle
                None,                            // no existing context (first call)
                Some(PCWSTR(spn_wide.as_ptr()).as_ptr()), // target SPN
                context_req,                     // context requirements
                0,                               // reserved
                SECURITY_NATIVE_DREP,            // data representation
                None,                            // no input (first call)
                0,                               // reserved
                Some(&mut ctx.ctx_handle),       // output context handle
                Some(&mut out_buf_desc),         // output buffer
                &mut context_attrs,              // output context attributes
                Some(&mut expiry),               // output expiry
            )
        };

        if hr == SEC_E_OK || hr == SEC_I_CONTINUE_NEEDED {
            ctx.has_context = true;

            if hr == SEC_E_OK {
                ctx.complete = true;
            }

            // Extract the token from the output buffer
            let token_len = out_sec_buf.cbBuffer as usize;
            let token = out_buf[..token_len].to_vec();

            tracing::debug!(
                token_len,
                continue_needed = (hr == SEC_I_CONTINUE_NEEDED),
                "SSPI initialization produced token"
            );

            Ok(token)
        } else {
            Err(AuthError::Sspi(format!(
                "InitializeSecurityContext failed: HRESULT 0x{:08X}",
                hr.0 as u32
            )))
        }
    }

    /// Process a server challenge token and generate a response.
    ///
    /// Returns `Some(token)` if more data needs to be sent to the server,
    /// or `None` if authentication is complete.
    ///
    /// # Arguments
    ///
    /// * `server_token` - The SSPI challenge token received from the server
    ///
    /// # Errors
    ///
    /// Returns an error if the negotiation step fails.
    pub fn step(&self, server_token: &[u8]) -> Result<Option<Vec<u8>>, AuthError> {
        let mut ctx = self
            .context
            .lock()
            .map_err(|_| AuthError::Sspi("Failed to acquire context lock".into()))?;

        if ctx.complete {
            return Ok(None);
        }

        if !ctx.has_context {
            return Err(AuthError::Sspi(
                "Context not initialized - call initialize() first".into(),
            ));
        }

        // Encode SPN as wide string (null-terminated)
        let spn_wide: Vec<u16> = format!("{}\0", self.spn).encode_utf16().collect();

        // Set up input buffer with server's challenge token
        let mut in_buf = server_token.to_vec();
        let mut in_sec_buf = SecBuffer {
            cbBuffer: in_buf.len() as u32,
            BufferType: SECBUFFER_TOKEN,
            pvBuffer: in_buf.as_mut_ptr().cast(),
        };
        let in_buf_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut in_sec_buf,
        };

        // Set up output buffer
        let mut out_buf = vec![0u8; MAX_TOKEN_SIZE];
        let mut out_sec_buf = SecBuffer {
            cbBuffer: out_buf.len() as u32,
            BufferType: SECBUFFER_TOKEN,
            pvBuffer: out_buf.as_mut_ptr().cast(),
        };
        let mut out_buf_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut out_sec_buf,
        };

        let mut context_attrs: u32 = 0;
        let mut expiry: i64 = 0;

        let context_req = ISC_REQ_MUTUAL_AUTH
            | ISC_REQ_REPLAY_DETECT
            | ISC_REQ_SEQUENCE_DETECT
            | ISC_REQ_CONNECTION;

        // SAFETY: InitializeSecurityContextW is called with:
        // - phCredential: valid credential handle
        // - phContext: existing context from previous call
        // - pszTargetName: null-terminated wide SPN string
        // - pInput: SecBufferDesc containing the server's challenge token
        // - phNewContext: same context handle (updated in place)
        // - pOutput: SecBufferDesc with pre-allocated buffer for response
        //
        // The context handle is updated in-place. The output buffer receives
        // the response token to send to the server.
        let hr = unsafe {
            InitializeSecurityContextW(
                Some(&ctx.cred_handle),          // credential handle
                Some(&ctx.ctx_handle),           // existing context
                Some(PCWSTR(spn_wide.as_ptr()).as_ptr()), // target SPN
                context_req,                     // context requirements
                0,                               // reserved
                SECURITY_NATIVE_DREP,            // data representation
                Some(&in_buf_desc),              // input from server
                0,                               // reserved
                Some(&mut ctx.ctx_handle),       // context handle (updated)
                Some(&mut out_buf_desc),         // output buffer
                &mut context_attrs,              // output context attributes
                Some(&mut expiry),               // output expiry
            )
        };

        match hr {
            hr if hr == SEC_E_OK => {
                ctx.complete = true;
                let token_len = out_sec_buf.cbBuffer as usize;
                if token_len > 0 {
                    let token = out_buf[..token_len].to_vec();
                    tracing::debug!(token_len, "SSPI step complete with final token");
                    Ok(Some(token))
                } else {
                    tracing::debug!("SSPI step complete, no final token");
                    Ok(None)
                }
            }
            hr if hr == SEC_I_CONTINUE_NEEDED => {
                let token_len = out_sec_buf.cbBuffer as usize;
                let token = out_buf[..token_len].to_vec();
                tracing::debug!(token_len, "SSPI step needs continuation");
                Ok(Some(token))
            }
            _ => Err(AuthError::Sspi(format!(
                "SSPI step failed: HRESULT 0x{:08X}",
                hr.0 as u32
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

impl std::fmt::Debug for NativeSspiAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSspiAuth")
            .field("spn", &self.spn)
            .field("complete", &self.is_complete())
            .finish()
    }
}

impl Drop for NativeSspiContext {
    fn drop(&mut self) {
        // SAFETY: We free the security context handle if it was initialized,
        // and the credential handle which was always initialized in new().
        // Both handles were obtained from valid SSPI API calls.
        // Double-free is prevented by the has_context flag and because
        // Drop runs exactly once.
        unsafe {
            if self.has_context {
                let _ = DeleteSecurityContext(&self.ctx_handle);
            }
            let _ = FreeCredentialsHandle(&self.cred_handle);
        }
    }
}

impl crate::negotiator::SspiNegotiator for NativeSspiAuth {
    fn initialize(&self) -> Result<Vec<u8>, AuthError> {
        NativeSspiAuth::initialize(self)
    }

    fn step(&self, server_token: &[u8]) -> Result<Option<Vec<u8>>, AuthError> {
        NativeSspiAuth::step(self, server_token)
    }

    fn is_complete(&self) -> bool {
        NativeSspiAuth::is_complete(self)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_spn_format() {
        let auth = NativeSspiAuth::new("sqlserver.example.com", 1433).unwrap();
        assert_eq!(auth.spn(), "MSSQLSvc/sqlserver.example.com:1433");
    }

    #[test]
    fn test_debug_output() {
        let auth = NativeSspiAuth::new("test.example.com", 1433).unwrap();
        let debug = format!("{auth:?}");
        assert!(debug.contains("NativeSspiAuth"));
        assert!(debug.contains("test.example.com"));
    }

    #[test]
    fn test_is_complete_initially_false() {
        let auth = NativeSspiAuth::new("test.example.com", 1433).unwrap();
        assert!(!auth.is_complete());
    }

    #[test]
    fn test_initialize_produces_token() {
        // This test verifies that Windows SSPI can acquire credentials
        // and produce an initial Negotiate token for the current user.
        let auth = NativeSspiAuth::new("localhost", 1433).unwrap();
        let token = auth.initialize().unwrap();
        assert!(!token.is_empty(), "Initial SSPI token should not be empty");
        // Negotiate tokens start with the SPNEGO OID wrapped in an
        // APPLICATION [0] ASN.1 tag (0x60)
        assert_eq!(
            token[0], 0x60,
            "Token should start with SPNEGO APPLICATION tag"
        );
    }
}
