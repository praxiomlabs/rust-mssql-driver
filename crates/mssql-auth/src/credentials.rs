//! Credential types for authentication.

use std::borrow::Cow;

/// Credentials for SQL Server authentication.
///
/// This enum represents the various authentication methods supported.
/// Credentials are designed to minimize copying of sensitive data.
#[derive(Clone)]
pub enum Credentials {
    /// SQL Server authentication with username and password.
    SqlServer {
        /// Username.
        username: Cow<'static, str>,
        /// Password.
        password: Cow<'static, str>,
    },

    /// Azure Active Directory / Entra ID access token.
    AzureAccessToken {
        /// The access token string.
        token: Cow<'static, str>,
    },

    /// Azure Managed Identity (for VMs and containers).
    #[cfg(feature = "azure-identity")]
    AzureManagedIdentity {
        /// Optional client ID for user-assigned identity.
        client_id: Option<Cow<'static, str>>,
    },

    /// Azure Service Principal.
    #[cfg(feature = "azure-identity")]
    AzureServicePrincipal {
        /// Tenant ID.
        tenant_id: Cow<'static, str>,
        /// Client ID.
        client_id: Cow<'static, str>,
        /// Client secret.
        client_secret: Cow<'static, str>,
    },

    /// Integrated Windows Authentication (Kerberos/NTLM).
    #[cfg(feature = "integrated-auth")]
    Integrated,

    /// Client certificate authentication.
    #[cfg(feature = "cert-auth")]
    Certificate {
        /// Path to certificate file.
        cert_path: Cow<'static, str>,
        /// Optional password for encrypted certificates.
        password: Option<Cow<'static, str>>,
    },
}

impl Credentials {
    /// Create SQL Server credentials.
    pub fn sql_server(
        username: impl Into<Cow<'static, str>>,
        password: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::SqlServer {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Create Azure access token credentials.
    pub fn azure_token(token: impl Into<Cow<'static, str>>) -> Self {
        Self::AzureAccessToken {
            token: token.into(),
        }
    }

    /// Check if these credentials use SQL authentication.
    #[must_use]
    pub fn is_sql_auth(&self) -> bool {
        matches!(self, Self::SqlServer { .. })
    }

    /// Check if these credentials use Azure AD.
    #[must_use]
    pub fn is_azure_ad(&self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match self {
            Self::AzureAccessToken { .. } => true,
            #[cfg(feature = "azure-identity")]
            Self::AzureManagedIdentity { .. } | Self::AzureServicePrincipal { .. } => true,
            _ => false,
        }
    }

    /// Get the authentication method name.
    #[must_use]
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::SqlServer { .. } => "SQL Server Authentication",
            Self::AzureAccessToken { .. } => "Azure AD Access Token",
            #[cfg(feature = "azure-identity")]
            Self::AzureManagedIdentity { .. } => "Azure Managed Identity",
            #[cfg(feature = "azure-identity")]
            Self::AzureServicePrincipal { .. } => "Azure Service Principal",
            #[cfg(feature = "integrated-auth")]
            Self::Integrated => "Integrated Authentication",
            #[cfg(feature = "cert-auth")]
            Self::Certificate { .. } => "Certificate Authentication",
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never expose sensitive data in debug output
        match self {
            Self::SqlServer { username, .. } => f
                .debug_struct("SqlServer")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::AzureAccessToken { .. } => f
                .debug_struct("AzureAccessToken")
                .field("token", &"[REDACTED]")
                .finish(),
            #[cfg(feature = "azure-identity")]
            Self::AzureManagedIdentity { client_id } => f
                .debug_struct("AzureManagedIdentity")
                .field("client_id", client_id)
                .finish(),
            #[cfg(feature = "azure-identity")]
            Self::AzureServicePrincipal {
                tenant_id,
                client_id,
                ..
            } => f
                .debug_struct("AzureServicePrincipal")
                .field("tenant_id", tenant_id)
                .field("client_id", client_id)
                .field("client_secret", &"[REDACTED]")
                .finish(),
            #[cfg(feature = "integrated-auth")]
            Self::Integrated => f.debug_struct("Integrated").finish(),
            #[cfg(feature = "cert-auth")]
            Self::Certificate { cert_path, .. } => f
                .debug_struct("Certificate")
                .field("cert_path", cert_path)
                .field("password", &"[REDACTED]")
                .finish(),
        }
    }
}

// Note: For proper zeroization of sensitive data, the `zeroize` crate should
// be used in production. The Drop implementation has been omitted here because:
// 1. Safe zeroization requires the `zeroize` crate
// 2. The `unsafe_code` lint is denied in this crate
// 3. Cow<'static, str> with Borrowed variants cannot be zeroized anyway
//
// TODO: Add optional `zeroize` feature with proper secret handling
