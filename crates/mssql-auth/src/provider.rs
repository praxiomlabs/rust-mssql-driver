//! Authentication provider traits.
//!
//! This module defines the `AuthProvider` trait for implementing
//! authentication strategies, as specified in ARCHITECTURE.md.

use bytes::Bytes;

use crate::error::AuthError;

/// Authentication method enumeration.
///
/// This indicates which authentication flow to use during connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthMethod {
    /// SQL Server authentication (username/password in Login7).
    SqlServer,
    /// Azure AD / Entra ID federated authentication.
    AzureAd,
    /// Integrated Windows authentication (SSPI/Kerberos).
    Integrated,
    /// Certificate-based authentication.
    Certificate,
}

impl AuthMethod {
    /// Check if this method uses federated authentication.
    #[must_use]
    pub fn is_federated(&self) -> bool {
        matches!(self, Self::AzureAd)
    }

    /// Check if this method uses SSPI.
    #[must_use]
    pub fn is_sspi(&self) -> bool {
        matches!(self, Self::Integrated)
    }

    /// Check if this method uses Login7 credentials.
    #[must_use]
    pub fn uses_login7_credentials(&self) -> bool {
        matches!(self, Self::SqlServer)
    }
}

/// Authentication data produced by an auth provider.
///
/// This contains the data needed to authenticate with SQL Server,
/// depending on the authentication method being used.
///
/// Sensitive fields (password bytes, tokens, SSPI blobs) are securely zeroized
/// on drop when the `zeroize` feature is enabled.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthData {
    /// SQL Server credentials for Login7 packet.
    SqlServer {
        /// Username.
        username: String,
        /// Obfuscated password bytes (XOR + bit rotation).
        password_bytes: Vec<u8>,
    },
    /// Federated authentication token for FEDAUTH feature.
    FedAuth {
        /// The access token.
        token: String,
        /// Token nonce (optional, for certain flows).
        nonce: Option<Bytes>,
    },
    /// SSPI blob for integrated authentication.
    Sspi {
        /// The SSPI authentication blob.
        blob: Vec<u8>,
    },
    /// No additional authentication data needed.
    None,
}

/// Trait for authentication providers.
///
/// Authentication providers are responsible for producing the authentication
/// data needed for the TDS connection. Different providers support different
/// authentication methods (SQL auth, Azure AD, integrated, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use mssql_auth::{AuthProvider, SqlServerAuth};
///
/// let provider = SqlServerAuth::new("username", "password");
/// let auth_data = provider.authenticate().await?;
/// ```
pub trait AuthProvider: Send + Sync {
    /// Get the authentication method this provider uses.
    fn method(&self) -> AuthMethod;

    /// Authenticate and produce authentication data.
    ///
    /// This may involve network calls (e.g., for Azure AD token acquisition)
    /// so it returns a future in async implementations.
    fn authenticate(&self) -> Result<AuthData, AuthError>;

    /// Get additional feature extension data for Login7.
    ///
    /// Some authentication methods (like Azure AD) require feature extensions
    /// in the Login7 packet. This returns the raw feature data if needed.
    fn feature_extension_data(&self) -> Option<Bytes> {
        None
    }

    /// Check if this provider needs to refresh its authentication.
    ///
    /// For token-based authentication, this can check if the token is expired
    /// or about to expire.
    fn needs_refresh(&self) -> bool {
        false
    }
}

/// Async authentication provider trait.
///
/// This is for authentication methods that require async operations,
/// such as acquiring tokens from Azure AD endpoints.
#[allow(async_fn_in_trait)]
pub trait AsyncAuthProvider: Send + Sync {
    /// Get the authentication method this provider uses.
    fn method(&self) -> AuthMethod;

    /// Authenticate asynchronously and produce authentication data.
    async fn authenticate_async(&self) -> Result<AuthData, AuthError>;

    /// Get additional feature extension data for Login7.
    fn feature_extension_data(&self) -> Option<Bytes> {
        None
    }

    /// Check if this provider needs to refresh its authentication.
    fn needs_refresh(&self) -> bool {
        false
    }
}

// Implement AuthProvider for any AsyncAuthProvider by blocking
// (for use in synchronous contexts when needed)
impl<T: AsyncAuthProvider> AuthProvider for T {
    fn method(&self) -> AuthMethod {
        <T as AsyncAuthProvider>::method(self)
    }

    fn authenticate(&self) -> Result<AuthData, AuthError> {
        // This is a fallback - in practice, async providers should be used
        // with authenticate_async(). This implementation is for compatibility.
        Err(AuthError::Configuration(
            "Async auth provider must use authenticate_async()".into(),
        ))
    }

    fn feature_extension_data(&self) -> Option<Bytes> {
        <T as AsyncAuthProvider>::feature_extension_data(self)
    }

    fn needs_refresh(&self) -> bool {
        <T as AsyncAuthProvider>::needs_refresh(self)
    }
}

// Secure zeroization of sensitive authentication data when `zeroize` feature is enabled.
#[cfg(feature = "zeroize")]
impl Drop for AuthData {
    fn drop(&mut self) {
        use zeroize::Zeroize;

        match self {
            AuthData::SqlServer { password_bytes, .. } => {
                password_bytes.zeroize();
            }
            AuthData::FedAuth { token, .. } => {
                token.zeroize();
            }
            AuthData::Sspi { blob } => {
                blob.zeroize();
            }
            AuthData::None => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_method_properties() {
        assert!(AuthMethod::AzureAd.is_federated());
        assert!(!AuthMethod::SqlServer.is_federated());

        assert!(AuthMethod::Integrated.is_sspi());
        assert!(!AuthMethod::SqlServer.is_sspi());

        assert!(AuthMethod::SqlServer.uses_login7_credentials());
        assert!(!AuthMethod::AzureAd.uses_login7_credentials());
    }

    #[test]
    fn test_auth_method_all_variants_classified() {
        // Every auth method should have exactly one primary category
        let methods = [
            AuthMethod::SqlServer,
            AuthMethod::AzureAd,
            AuthMethod::Integrated,
            AuthMethod::Certificate,
        ];

        for method in &methods {
            let categories = [
                method.uses_login7_credentials(),
                method.is_federated(),
                method.is_sspi(),
            ];
            // At most one category should be true (Certificate has none)
            let count = categories.iter().filter(|&&b| b).count();
            assert!(
                count <= 1,
                "{method:?} has {count} categories, expected 0 or 1"
            );
        }
    }

    #[test]
    fn test_auth_method_certificate() {
        let cert = AuthMethod::Certificate;
        assert!(!cert.is_federated());
        assert!(!cert.is_sspi());
        assert!(!cert.uses_login7_credentials());
    }

    #[test]
    fn test_auth_data_sql_server() {
        let data = AuthData::SqlServer {
            username: "sa".to_string(),
            password_bytes: vec![0xA5, 0xB6],
        };
        // Verify it's the right variant via pattern match
        match &data {
            AuthData::SqlServer {
                username,
                password_bytes,
            } => {
                assert_eq!(username, "sa");
                assert_eq!(password_bytes.len(), 2);
            }
            _ => panic!("Expected SqlServer variant"),
        }
    }

    #[test]
    fn test_auth_data_fed_auth() {
        let data = AuthData::FedAuth {
            token: "eyJhbGciOiJSUzI1NiJ9.test".to_string(),
            nonce: None,
        };
        match &data {
            AuthData::FedAuth { token, nonce } => {
                assert!(token.starts_with("eyJ"));
                assert!(nonce.is_none());
            }
            _ => panic!("Expected FedAuth variant"),
        }
    }

    #[test]
    fn test_auth_data_sspi() {
        let data = AuthData::Sspi {
            blob: vec![0x4E, 0x54, 0x4C, 0x4D], // "NTLM" bytes
        };
        match &data {
            AuthData::Sspi { blob } => {
                assert_eq!(blob.len(), 4);
            }
            _ => panic!("Expected Sspi variant"),
        }
    }

    #[test]
    fn test_auth_data_none() {
        let data = AuthData::None;
        assert!(matches!(data, AuthData::None));
    }

    #[test]
    fn test_auth_data_debug_output() {
        // Verify Debug impl doesn't panic on any variant
        let variants: Vec<AuthData> = vec![
            AuthData::SqlServer {
                username: "test".into(),
                password_bytes: vec![1, 2, 3],
            },
            AuthData::FedAuth {
                token: "tok".into(),
                nonce: Some(Bytes::from_static(b"nonce")),
            },
            AuthData::Sspi {
                blob: vec![0x01, 0x02],
            },
            AuthData::None,
        ];

        for v in &variants {
            let _ = format!("{v:?}");
        }
    }

    /// A mock auth provider for testing the trait interface.
    struct MockProvider {
        method: AuthMethod,
    }

    impl AuthProvider for MockProvider {
        fn method(&self) -> AuthMethod {
            self.method
        }

        fn authenticate(&self) -> Result<AuthData, crate::error::AuthError> {
            Ok(AuthData::None)
        }
    }

    #[test]
    fn test_auth_provider_trait_defaults() {
        let provider = MockProvider {
            method: AuthMethod::SqlServer,
        };

        assert_eq!(provider.method(), AuthMethod::SqlServer);
        assert!(provider.feature_extension_data().is_none());
        assert!(!provider.needs_refresh());

        let data = provider.authenticate().unwrap();
        assert!(matches!(data, AuthData::None));
    }
}
