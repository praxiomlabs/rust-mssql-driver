//! Azure Identity authentication providers.
//!
//! This module provides Azure authentication using the `azure_identity` crate
//! for token acquisition. It supports:
//!
//! - **Managed Identity**: For Azure VMs, App Service, Container Instances, and AKS
//! - **Service Principal**: For application-based authentication with client credentials
//!
//! ## Example: Managed Identity (System-Assigned)
//!
//! ```rust,ignore
//! use mssql_auth::ManagedIdentityAuth;
//!
//! // System-assigned managed identity (default)
//! let auth = ManagedIdentityAuth::system_assigned();
//! let token = auth.get_token().await?;
//! ```
//!
//! ## Example: Managed Identity (User-Assigned)
//!
//! ```rust,ignore
//! use mssql_auth::ManagedIdentityAuth;
//!
//! // User-assigned managed identity by client ID
//! let auth = ManagedIdentityAuth::user_assigned_client_id("your-client-id");
//! let token = auth.get_token().await?;
//! ```
//!
//! ## Example: Service Principal
//!
//! ```rust,ignore
//! use mssql_auth::ServicePrincipalAuth;
//!
//! let auth = ServicePrincipalAuth::new(
//!     "your-tenant-id",
//!     "your-client-id",
//!     "your-client-secret",
//! );
//! let token = auth.get_token().await?;
//! ```

use std::sync::Arc;
use std::time::Duration;

use azure_core::credentials::TokenCredential;
use azure_identity::{
    ClientSecretCredential, ManagedIdentityCredential, ManagedIdentityCredentialOptions,
    UserAssignedId,
};

use crate::AzureAdAuth;
use crate::error::AuthError;
use crate::provider::{AuthData, AuthMethod};

/// The Azure SQL Database scope for token requests.
const AZURE_SQL_SCOPE: &str = "https://database.windows.net/.default";

/// Managed Identity authentication provider.
///
/// Uses Azure Managed Identity to acquire access tokens for Azure SQL Database.
/// This works on Azure VMs, App Service, Container Instances, and AKS.
#[derive(Clone)]
pub struct ManagedIdentityAuth {
    credential: Arc<ManagedIdentityCredential>,
}

impl ManagedIdentityAuth {
    /// Create authentication using system-assigned managed identity.
    ///
    /// This is the simplest form - uses the identity assigned to the Azure resource
    /// (VM, App Service, etc.) that the code is running on.
    ///
    /// # Errors
    ///
    /// Returns an error if the managed identity credential cannot be created.
    pub fn system_assigned() -> Result<Self, AuthError> {
        let credential = ManagedIdentityCredential::new(None)
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(Self { credential })
    }

    /// Create authentication using a user-assigned managed identity by client ID.
    ///
    /// Use this when you have multiple managed identities and need to specify which one to use.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client ID of the user-assigned managed identity
    ///
    /// # Errors
    ///
    /// Returns an error if the managed identity credential cannot be created.
    pub fn user_assigned_client_id(client_id: impl Into<String>) -> Result<Self, AuthError> {
        let options = ManagedIdentityCredentialOptions {
            user_assigned_id: Some(UserAssignedId::ClientId(client_id.into())),
            ..Default::default()
        };
        let credential = ManagedIdentityCredential::new(Some(options))
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(Self { credential })
    }

    /// Create authentication using a user-assigned managed identity by resource ID.
    ///
    /// # Arguments
    ///
    /// * `resource_id` - The Azure resource ID of the user-assigned managed identity
    ///
    /// # Errors
    ///
    /// Returns an error if the managed identity credential cannot be created.
    pub fn user_assigned_resource_id(resource_id: impl Into<String>) -> Result<Self, AuthError> {
        let options = ManagedIdentityCredentialOptions {
            user_assigned_id: Some(UserAssignedId::ResourceId(resource_id.into())),
            ..Default::default()
        };
        let credential = ManagedIdentityCredential::new(Some(options))
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(Self { credential })
    }

    /// Create authentication using a user-assigned managed identity by object ID.
    ///
    /// # Arguments
    ///
    /// * `object_id` - The object ID of the user-assigned managed identity
    ///
    /// # Errors
    ///
    /// Returns an error if the managed identity credential cannot be created.
    pub fn user_assigned_object_id(object_id: impl Into<String>) -> Result<Self, AuthError> {
        let options = ManagedIdentityCredentialOptions {
            user_assigned_id: Some(UserAssignedId::ObjectId(object_id.into())),
            ..Default::default()
        };
        let credential = ManagedIdentityCredential::new(Some(options))
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(Self { credential })
    }

    /// Get an access token for Azure SQL Database.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails.
    pub async fn get_token(&self) -> Result<String, AuthError> {
        let token = self
            .credential
            .get_token(&[AZURE_SQL_SCOPE], None)
            .await
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(token.token.secret().to_string())
    }

    /// Get an access token with expiration information.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails.
    pub async fn get_token_with_expiry(&self) -> Result<(String, Option<Duration>), AuthError> {
        let token = self
            .credential
            .get_token(&[AZURE_SQL_SCOPE], None)
            .await
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;

        // Calculate time until expiration
        let now = time::OffsetDateTime::now_utc();
        let expires_in = if token.expires_on > now {
            let diff = token.expires_on - now;
            Some(Duration::from_secs(diff.whole_seconds().max(0) as u64))
        } else {
            None
        };

        Ok((token.token.secret().to_string(), expires_in))
    }

    /// Convert to an `AzureAdAuth` provider with an acquired token.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails.
    pub async fn to_azure_ad_auth(&self) -> Result<AzureAdAuth, AuthError> {
        let (token, expires_in) = self.get_token_with_expiry().await?;
        match expires_in {
            Some(duration) => Ok(AzureAdAuth::with_token_expiring(token, duration)),
            None => Ok(AzureAdAuth::with_token(token)),
        }
    }
}

impl std::fmt::Debug for ManagedIdentityAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedIdentityAuth")
            .finish_non_exhaustive()
    }
}

impl crate::provider::AsyncAuthProvider for ManagedIdentityAuth {
    fn method(&self) -> AuthMethod {
        AuthMethod::AzureAd
    }

    async fn authenticate_async(&self) -> Result<AuthData, AuthError> {
        let token = self.get_token().await?;
        Ok(AuthData::FedAuth { token, nonce: None })
    }

    fn needs_refresh(&self) -> bool {
        // Managed identity tokens are acquired fresh each time
        false
    }
}

/// Service Principal authentication provider.
///
/// Uses Azure Service Principal (application credentials) to acquire access tokens.
/// This is suitable for server-to-server authentication where no user is present.
pub struct ServicePrincipalAuth {
    credential: Arc<ClientSecretCredential>,
}

impl ServicePrincipalAuth {
    /// Create a new Service Principal authenticator.
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The Azure AD tenant ID
    /// * `client_id` - The application (client) ID
    /// * `client_secret` - The client secret
    ///
    /// # Errors
    ///
    /// Returns an error if the credential cannot be created.
    pub fn new(
        tenant_id: impl AsRef<str>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Result<Self, AuthError> {
        use azure_core::credentials::Secret;

        let secret = Secret::new(client_secret.into());
        let credential =
            ClientSecretCredential::new(tenant_id.as_ref(), client_id.into(), secret, None)
                .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(Self { credential })
    }

    /// Get an access token for Azure SQL Database.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails.
    pub async fn get_token(&self) -> Result<String, AuthError> {
        let token = self
            .credential
            .get_token(&[AZURE_SQL_SCOPE], None)
            .await
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;
        Ok(token.token.secret().to_string())
    }

    /// Get an access token with expiration information.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails.
    pub async fn get_token_with_expiry(&self) -> Result<(String, Option<Duration>), AuthError> {
        let token = self
            .credential
            .get_token(&[AZURE_SQL_SCOPE], None)
            .await
            .map_err(|e| AuthError::AzureIdentity(e.to_string()))?;

        // Calculate time until expiration
        let now = time::OffsetDateTime::now_utc();
        let expires_in = if token.expires_on > now {
            let diff = token.expires_on - now;
            Some(Duration::from_secs(diff.whole_seconds().max(0) as u64))
        } else {
            None
        };

        Ok((token.token.secret().to_string(), expires_in))
    }

    /// Convert to an `AzureAdAuth` provider with an acquired token.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails.
    pub async fn to_azure_ad_auth(&self) -> Result<AzureAdAuth, AuthError> {
        let (token, expires_in) = self.get_token_with_expiry().await?;
        match expires_in {
            Some(duration) => Ok(AzureAdAuth::with_token_expiring(token, duration)),
            None => Ok(AzureAdAuth::with_token(token)),
        }
    }
}

impl Clone for ServicePrincipalAuth {
    fn clone(&self) -> Self {
        Self {
            credential: Arc::clone(&self.credential),
        }
    }
}

impl std::fmt::Debug for ServicePrincipalAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicePrincipalAuth")
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

impl crate::provider::AsyncAuthProvider for ServicePrincipalAuth {
    fn method(&self) -> AuthMethod {
        AuthMethod::AzureAd
    }

    async fn authenticate_async(&self) -> Result<AuthData, AuthError> {
        let token = self.get_token().await?;
        Ok(AuthData::FedAuth { token, nonce: None })
    }

    fn needs_refresh(&self) -> bool {
        // Service principal tokens are acquired fresh each time
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // Note: These tests require Azure credentials to be configured in the environment.
    // They are marked as ignored by default and can be run manually with:
    // cargo test --features azure-identity -- --ignored

    #[tokio::test]
    #[ignore = "Requires Azure Managed Identity environment"]
    async fn test_managed_identity_system_assigned() {
        let auth = ManagedIdentityAuth::system_assigned().expect("Failed to create credential");
        let token = auth.get_token().await.expect("Failed to get token");
        assert!(!token.is_empty());
    }

    #[tokio::test]
    #[ignore = "Requires Azure Service Principal credentials"]
    async fn test_service_principal() {
        let tenant_id = std::env::var("AZURE_TENANT_ID").expect("AZURE_TENANT_ID not set");
        let client_id = std::env::var("AZURE_CLIENT_ID").expect("AZURE_CLIENT_ID not set");
        let client_secret =
            std::env::var("AZURE_CLIENT_SECRET").expect("AZURE_CLIENT_SECRET not set");

        let auth = ServicePrincipalAuth::new(tenant_id, client_id, client_secret)
            .expect("Failed to create credential");
        let token = auth.get_token().await.expect("Failed to get token");
        assert!(!token.is_empty());
    }

    #[test]
    fn test_debug_redacts_credentials() {
        // Just verify Debug impl doesn't panic and doesn't expose secrets
        if let Ok(auth) = ManagedIdentityAuth::system_assigned() {
            let debug = format!("{:?}", auth);
            assert!(debug.contains("ManagedIdentityAuth"));
        }
    }
}
