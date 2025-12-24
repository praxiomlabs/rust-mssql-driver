//! Azure AD / Entra ID authentication implementation.
//!
//! This module provides Azure AD federated authentication for SQL Server,
//! supporting both pre-acquired tokens and (with feature flags) token acquisition.
//!
//! ## Authentication Flow
//!
//! Azure AD authentication uses the TDS FEDAUTH feature extension:
//!
//! 1. Client includes FEDAUTH feature in Login7 packet
//! 2. Server responds with FEDAUTHINFO containing STS URL and SPN
//! 3. Client acquires token (or uses pre-acquired token)
//! 4. Client sends FEDAUTH token packet
//! 5. Server validates token and completes authentication
//!
//! ## Token Sources (Tier 1 - Core) ✅ Implemented
//!
//! - Pre-acquired access token (user provides token directly)
//!
//! ## Token Sources (Tier 2 - azure-identity feature) ✅ Implemented
//!
//! These require the `azure-identity` feature flag:
//!
//! - `ManagedIdentityAuth` - Azure VM/Container identity
//! - `ServicePrincipalAuth` - Client ID + Secret
//!
//! ## Token Sources (Tier 3 - cert-auth feature) ✅ Implemented
//!
//! - `CertificateAuth` - X.509 client certificate

use std::borrow::Cow;
use std::time::{Duration, Instant};

use bytes::Bytes;

use crate::credentials::Credentials;
use crate::error::AuthError;
use crate::provider::{AuthData, AuthMethod, AuthProvider};

/// FEDAUTH library options for Login7.
///
/// These values indicate to the server which token library the client uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FedAuthLibrary {
    /// ADAL (Azure Active Directory Authentication Library) - legacy.
    Adal = 0x01,
    /// Security token (raw JWT).
    SecurityToken = 0x02,
    /// MSAL (Microsoft Authentication Library) - current.
    Msal = 0x03,
}

impl FedAuthLibrary {
    /// Get the byte value for the FEDAUTH feature extension.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// FEDAUTH workflow types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// The library type to report to the server.
    library: FedAuthLibrary,
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
            library: FedAuthLibrary::SecurityToken,
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
            library: FedAuthLibrary::SecurityToken,
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

    /// Set the library type to report to the server.
    #[must_use]
    pub fn with_library(mut self, library: FedAuthLibrary) -> Self {
        self.library = library;
        self
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

    /// Build the FEDAUTH feature extension data for Login7.
    ///
    /// Format:
    /// - 1 byte: Library type (ADAL=1, SecurityToken=2, MSAL=3)
    /// - 1 byte: Workflow (0x00 for pre-acquired token)
    /// - 4 bytes: FedAuth token length (big-endian)
    /// - N bytes: FedAuth token (UTF-16LE encoded)
    #[must_use]
    pub fn build_feature_data(&self) -> Bytes {
        let mut data = Vec::with_capacity(6);

        // Library type (1 byte)
        data.push(self.library.to_byte());

        // Workflow - 0x00 for non-interactive/pre-acquired token
        data.push(0x00);

        // For FEDAUTH, the actual token is sent in a separate FEDAUTH token packet,
        // not in the Login7 feature extension. The feature extension just indicates
        // that we want to use FEDAUTH.

        Bytes::from(data)
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

    fn feature_extension_data(&self) -> Option<Bytes> {
        Some(self.build_feature_data())
    }

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
            .field("library", &self.library)
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

    #[test]
    fn test_azure_ad_feature_data() {
        let auth = AzureAdAuth::with_token("test_token");
        let data = auth.build_feature_data();

        assert!(!data.is_empty());
        assert_eq!(data[0], FedAuthLibrary::SecurityToken.to_byte());
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
        match data {
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
        let debug = format!("{:?}", auth);
        assert!(!debug.contains("secret_token"));
        assert!(debug.contains("[REDACTED]"));
    }
}
