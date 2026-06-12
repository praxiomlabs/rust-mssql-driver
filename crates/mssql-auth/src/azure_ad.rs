//! Azure AD / Entra ID authentication implementation.
//!
//! This module provides token handling for Azure AD federated authentication,
//! supporting both pre-acquired tokens and (with feature flags) token acquisition.
//!
//! ## Authentication Flow
//!
//! Azure AD authentication uses the TDS FEDAUTH feature extension
//! (MS-TDS §2.2.6.4). Two workflows exist:
//!
//! - **SecurityToken** (implemented, #155 Phase 1): the client acquires a
//!   token *before* login and sends it inside the LOGIN7 FEDAUTH feature
//!   extension ([`build_security_token_feature_data`]). No FEDAUTHINFO
//!   round-trip occurs.
//! - **ADAL/MSAL** (pending, #155 Phase 2): the client declares intent in
//!   LOGIN7, the server responds with FEDAUTHINFO (STS URL + SPN), and the
//!   client acquires a token and sends it in a separate FEDAUTH message.
//!
//! ## Token Sources (Tier 1 - Core)
//!
//! - Pre-acquired access token (user provides token directly)
//!
//! ## Token Sources (Tier 2 - azure-identity feature)
//!
//! These require the `azure-identity` feature flag:
//!
//! - `ManagedIdentityAuth` - Azure VM/Container identity
//! - `ServicePrincipalAuth` - Client ID + Secret
//!
//! ## Token Sources (Tier 3 - cert-auth feature)
//!
//! - `CertificateAuth` - X.509 client certificate

use std::borrow::Cow;
use std::time::{Duration, Instant};

use bytes::{BufMut, Bytes, BytesMut};

use crate::credentials::Credentials;
use crate::error::AuthError;
use crate::provider::{AuthData, AuthMethod, AuthProvider};

/// FEDAUTH library identifiers for the LOGIN7 FEDAUTH feature extension.
///
/// Per MS-TDS §2.2.6.4, `bFedAuthLibrary` is a 7-bit value that occupies the
/// high 7 bits of the feature data's Options byte (the low bit is
/// `fFedAuthEcho`). Live ID Compact Token (0x00) is legacy and not supported;
/// 0x7F is reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum FedAuthLibrary {
    /// Security token: a token acquired by the client before login and sent
    /// inside LOGIN7 (the workflow used for pre-acquired Azure AD tokens).
    SecurityToken = 0x01,
    /// ADAL: the client declares intent and acquires the token after the
    /// server's FEDAUTHINFO response. MSAL (ADAL's successor) uses this same
    /// wire identifier.
    Adal = 0x02,
}

impl FedAuthLibrary {
    /// Get the 7-bit library identifier (unshifted).
    ///
    /// In the Options byte this value is shifted left by one bit to make room
    /// for `fFedAuthEcho`: `options = (library << 1) | echo`.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Build the LOGIN7 FEDAUTH feature extension data for the SecurityToken
/// workflow (MS-TDS §2.2.6.4, `bFedAuthLibrary` = 0x01).
///
/// Layout:
///
/// ```text
/// Options       = 1 byte: (0x01 << 1) | fFedAuthEcho
/// FedAuthToken  = DWORD (LE) byte length + token as UTF-16LE
/// ```
///
/// No trailing nonce is emitted: per spec the nonce MUST be present if and
/// only if the server's PRELOGIN response carried a NONCE option, and this
/// driver does not send NONCEOPT in PRELOGIN.
///
/// `fed_auth_echo` MUST be set if and only if the server's PRELOGIN response
/// contained FEDAUTHREQUIRED with value 0x01 — the server validates this echo
/// to detect tampering.
///
/// The token must be non-empty (the spec forbids a zero-length FedAuthToken);
/// callers are expected to validate this before login.
#[must_use]
pub fn build_security_token_feature_data(token: &str, fed_auth_echo: bool) -> Bytes {
    debug_assert!(!token.is_empty(), "FedAuthToken length MUST NOT be 0");

    let token_utf16: Vec<u8> = token.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();

    let mut data = BytesMut::with_capacity(1 + 4 + token_utf16.len());
    data.put_u8((FedAuthLibrary::SecurityToken.to_byte() << 1) | u8::from(fed_auth_echo));
    data.put_u32_le(token_utf16.len() as u32);
    data.put_slice(&token_utf16);
    data.freeze()
}

/// FEDAUTH workflow types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FedAuthWorkflow {
    /// Interactive authentication with user sign-in.
    Interactive,
    /// Non-interactive with pre-acquired token.
    NonInteractive,
    /// Managed identity (system or user-assigned).
    ManagedIdentity,
    /// Service principal with secret.
    ServicePrincipal,
}

/// Azure AD authentication provider.
///
/// This provider supports Azure AD / Entra ID federated authentication
/// using pre-acquired access tokens (Tier 1) or token acquisition
/// with the `azure-identity` feature (Tier 2).
///
/// # Example
///
/// ```rust
/// use mssql_auth::AzureAdAuth;
///
/// // Using a pre-acquired token
/// let auth = AzureAdAuth::with_token("eyJ0eXAi...");
/// ```
#[derive(Clone)]
pub struct AzureAdAuth {
    /// The access token.
    token: Cow<'static, str>,
    /// When the token expires (if known).
    expires_at: Option<Instant>,
}

impl AzureAdAuth {
    /// Create an Azure AD authenticator with a pre-acquired token.
    ///
    /// This is the simplest form - provide a valid access token obtained
    /// from Azure AD / Entra ID via your preferred method.
    ///
    /// # Arguments
    ///
    /// * `token` - A valid JWT access token for Azure SQL Database
    pub fn with_token(token: impl Into<Cow<'static, str>>) -> Self {
        Self {
            token: token.into(),
            expires_at: None,
        }
    }

    /// Create an Azure AD authenticator with a token and expiration.
    ///
    /// Providing the expiration time allows the driver to proactively
    /// refresh tokens before they expire.
    ///
    /// # Arguments
    ///
    /// * `token` - A valid JWT access token
    /// * `expires_in` - Duration until the token expires
    pub fn with_token_expiring(token: impl Into<Cow<'static, str>>, expires_in: Duration) -> Self {
        Self {
            token: token.into(),
            expires_at: Some(Instant::now() + expires_in),
        }
    }

    /// Create from existing credentials.
    ///
    /// Returns an error if the credentials are not Azure AD credentials.
    pub fn from_credentials(credentials: &Credentials) -> Result<Self, AuthError> {
        match credentials {
            Credentials::AzureAccessToken { token } => Ok(Self::with_token(token.to_string())),
            _ => Err(AuthError::UnsupportedMethod(
                "AzureAdAuth requires Azure AD credentials".into(),
            )),
        }
    }

    /// Check if the token is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Instant::now() >= exp)
            .unwrap_or(false)
    }

    /// Check if the token is expiring soon (within the given duration).
    #[must_use]
    pub fn is_expiring_soon(&self, within: Duration) -> bool {
        self.expires_at
            .map(|exp| Instant::now() + within >= exp)
            .unwrap_or(false)
    }

    /// Build the FEDAUTH feature extension data for Login7 (SecurityToken
    /// workflow).
    ///
    /// See [`build_security_token_feature_data`] for the wire layout and the
    /// `fed_auth_echo` contract.
    #[must_use]
    pub fn build_feature_data(&self, fed_auth_echo: bool) -> Bytes {
        build_security_token_feature_data(&self.token, fed_auth_echo)
    }

    /// Build the FEDAUTH token packet data.
    ///
    /// This is the token data sent in response to FEDAUTHINFO from the server.
    #[must_use]
    pub fn build_token_data(&self) -> Bytes {
        // Token is sent as UTF-16LE
        let token_utf16: Vec<u8> = self
            .token
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();

        let mut data = Vec::with_capacity(4 + token_utf16.len());

        // Token length (4 bytes, little-endian)
        data.extend_from_slice(&(token_utf16.len() as u32).to_le_bytes());

        // Token data (UTF-16LE)
        data.extend_from_slice(&token_utf16);

        Bytes::from(data)
    }
}

impl AuthProvider for AzureAdAuth {
    fn method(&self) -> AuthMethod {
        AuthMethod::AzureAd
    }

    fn authenticate(&self) -> Result<AuthData, AuthError> {
        if self.is_expired() {
            return Err(AuthError::TokenExpired);
        }

        tracing::debug!("authenticating with Azure AD token");

        Ok(AuthData::FedAuth {
            token: self.token.to_string(),
            nonce: None,
        })
    }

    // Note: `feature_extension_data` deliberately uses the trait default
    // (`None`). The FEDAUTH feature data depends on the server's PRELOGIN
    // FEDAUTHREQUIRED response (the echo bit), which is unknowable here;
    // the login path builds it via `build_feature_data(echo)` instead.

    fn needs_refresh(&self) -> bool {
        // Refresh if token expires within 5 minutes
        self.is_expiring_soon(Duration::from_secs(300))
    }
}

impl std::fmt::Debug for AzureAdAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureAdAuth")
            .field("token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_ad_with_token() {
        let auth = AzureAdAuth::with_token("test_token");
        assert_eq!(auth.method(), AuthMethod::AzureAd);
        assert!(!auth.is_expired());
    }

    #[test]
    fn test_azure_ad_with_expiring_token() {
        let auth = AzureAdAuth::with_token_expiring("test_token", Duration::from_secs(3600));
        assert!(!auth.is_expired());
        assert!(!auth.is_expiring_soon(Duration::from_secs(60)));
    }

    #[test]
    fn test_azure_ad_expired_token() {
        let auth = AzureAdAuth::with_token_expiring("test_token", Duration::from_secs(0));
        // Token with 0 duration should be expired immediately (or very soon)
        std::thread::sleep(Duration::from_millis(10));
        assert!(auth.is_expired());

        let result = auth.authenticate();
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    /// Wire-exact encoding of the SecurityToken FEDAUTH feature data per
    /// MS-TDS §2.2.6.4: Options byte = (0x01 << 1) | echo, then a
    /// little-endian DWORD byte length, then the token as UTF-16LE. No nonce.
    #[test]
    fn test_security_token_feature_data_wire_exact() {
        // "AB" -> UTF-16LE 41 00 42 00, length 4.
        let no_echo = build_security_token_feature_data("AB", false);
        assert_eq!(
            no_echo.as_ref(),
            &[0x02, 0x04, 0x00, 0x00, 0x00, 0x41, 0x00, 0x42, 0x00],
            "echo clear: options must be 0x02 (SecurityToken << 1)"
        );

        let echo = build_security_token_feature_data("AB", true);
        assert_eq!(
            echo.as_ref(),
            &[0x03, 0x04, 0x00, 0x00, 0x00, 0x41, 0x00, 0x42, 0x00],
            "echo set: fFedAuthEcho is the low bit of the options byte"
        );
    }

    /// Non-BMP characters must encode as UTF-16 surrogate pairs and the DWORD
    /// length must count bytes (not code units or chars).
    #[test]
    fn test_security_token_feature_data_surrogate_pair() {
        // U+1F600 -> surrogates D83D DE00 -> LE bytes 3D D8 00 DE.
        let data = build_security_token_feature_data("\u{1F600}", false);
        assert_eq!(
            data.as_ref(),
            &[0x02, 0x04, 0x00, 0x00, 0x00, 0x3D, 0xD8, 0x00, 0xDE]
        );
    }

    /// The method form delegates to the free function with the same token.
    #[test]
    fn test_azure_ad_feature_data() {
        let auth = AzureAdAuth::with_token("test_token");
        let data = auth.build_feature_data(true);

        assert_eq!(data, build_security_token_feature_data("test_token", true));
        // Library bits: options >> 1 must be the SecurityToken identifier.
        assert_eq!(data[0] >> 1, FedAuthLibrary::SecurityToken.to_byte());
        assert_eq!(data[0] & 1, 1);
    }

    /// The wire identifiers come from MS-TDS §2.2.6.4 and must never drift:
    /// SecurityToken = 0x01, ADAL (also used by MSAL) = 0x02.
    #[test]
    fn test_fed_auth_library_wire_values() {
        assert_eq!(FedAuthLibrary::SecurityToken.to_byte(), 0x01);
        assert_eq!(FedAuthLibrary::Adal.to_byte(), 0x02);
    }

    #[test]
    fn test_azure_ad_token_data() {
        let auth = AzureAdAuth::with_token("AB");
        let data = auth.build_token_data();

        // Length (4 bytes) + "AB" in UTF-16LE (4 bytes)
        assert_eq!(data.len(), 8);
        // Length is 4 (2 UTF-16 code units * 2 bytes each)
        assert_eq!(&data[0..4], &[4, 0, 0, 0]);
    }

    #[test]
    fn test_from_credentials() {
        let creds = Credentials::azure_token("my_token");
        let auth = AzureAdAuth::from_credentials(&creds).unwrap();

        let data = auth.authenticate().unwrap();
        match &data {
            AuthData::FedAuth { token, .. } => {
                assert_eq!(token, "my_token");
            }
            _ => panic!("Expected FedAuth data"),
        }
    }

    #[test]
    fn test_from_credentials_wrong_type() {
        let creds = Credentials::sql_server("user", "pass");
        let result = AzureAdAuth::from_credentials(&creds);
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_redacts_token() {
        let auth = AzureAdAuth::with_token("secret_token");
        let debug = format!("{auth:?}");
        assert!(!debug.contains("secret_token"));
        assert!(debug.contains("[REDACTED]"));
    }
}
