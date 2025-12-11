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
#[derive(Debug, Clone)]
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

#[cfg(test)]
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
}
