//! Credential types for authentication.
//!
//! This module provides credential types for various SQL Server authentication methods.
//! When the `zeroize` feature is enabled, sensitive credential data is securely
//! zeroed from memory when dropped.

use std::borrow::Cow;

#[cfg(feature = "zeroize")]
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Credentials for SQL Server authentication.
///
/// This enum represents the various authentication methods supported.
/// Credentials are designed to minimize copying of sensitive data.
#[derive(Clone)]
#[non_exhaustive]
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

    /// Azure default credential chain (managed identity, then the signed-in
    /// `az` / `azd` CLI session). Maps to `Authentication=ActiveDirectoryDefault`.
    #[cfg(feature = "azure-identity")]
    AzureDefault,

    /// Integrated Windows Authentication (Kerberos/NTLM).
    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
    Integrated,

    /// Client certificate authentication (Azure AD service principal with an
    /// X.509 certificate).
    ///
    /// The certificate authenticates to Microsoft Entra, which issues the
    /// access token used for the FEDAUTH login. This is NOT TDS-level mutual
    /// TLS — SQL Server does not accept client certificates at the protocol
    /// level.
    #[cfg(feature = "cert-auth")]
    Certificate {
        /// Azure AD tenant ID.
        tenant_id: Cow<'static, str>,
        /// Application (client) ID of the service principal.
        client_id: Cow<'static, str>,
        /// Path to the certificate file: PKCS#12 (`.pfx`), or a PEM file
        /// containing both the certificate and its private key.
        cert_path: Cow<'static, str>,
        /// Optional password protecting the certificate's private key.
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

    /// Create integrated authentication credentials (Windows SSPI or Kerberos/GSSAPI).
    ///
    /// Requires the `sspi-auth` (Windows) or `integrated-auth` (Linux/macOS) feature.
    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
    #[must_use]
    pub fn integrated() -> Self {
        Self::Integrated
    }

    /// Create Azure default-credential-chain credentials
    /// (`Authentication=ActiveDirectoryDefault`): managed identity, then the
    /// signed-in `az` / `azd` CLI session.
    ///
    /// Requires the `azure-identity` feature.
    #[cfg(feature = "azure-identity")]
    #[must_use]
    pub fn azure_default() -> Self {
        Self::AzureDefault
    }

    /// Create client-certificate (Azure AD service principal) credentials.
    ///
    /// `cert_path` points at a PKCS#12 (`.pfx`) file, or a PEM file containing
    /// both the certificate and its private key. The certificate authenticates
    /// to Microsoft Entra, which issues the access token used for login.
    ///
    /// Requires the `cert-auth` feature.
    #[cfg(feature = "cert-auth")]
    pub fn certificate(
        tenant_id: impl Into<Cow<'static, str>>,
        client_id: impl Into<Cow<'static, str>>,
        cert_path: impl Into<Cow<'static, str>>,
        password: Option<Cow<'static, str>>,
    ) -> Self {
        Self::Certificate {
            tenant_id: tenant_id.into(),
            client_id: client_id.into(),
            cert_path: cert_path.into(),
            password,
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
            Self::AzureManagedIdentity { .. }
            | Self::AzureServicePrincipal { .. }
            | Self::AzureDefault => true,
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
            #[cfg(feature = "azure-identity")]
            Self::AzureDefault => "Azure Default Credential Chain",
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
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
            #[cfg(feature = "azure-identity")]
            Self::AzureDefault => f.debug_struct("AzureDefault").finish(),
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            Self::Integrated => f.debug_struct("Integrated").finish(),
            #[cfg(feature = "cert-auth")]
            Self::Certificate {
                tenant_id,
                client_id,
                cert_path,
                ..
            } => f
                .debug_struct("Certificate")
                .field("tenant_id", tenant_id)
                .field("client_id", client_id)
                .field("cert_path", cert_path)
                .field("password", &"[REDACTED]")
                .finish(),
        }
    }
}

// =============================================================================
// Secure Credentials (with zeroize feature)
// =============================================================================

/// A secret string that is securely zeroed from memory when dropped.
///
/// This type is only available when the `zeroize` feature is enabled.
/// It ensures that sensitive data like passwords and tokens are overwritten
/// with zeros when they go out of scope.
#[cfg(feature = "zeroize")]
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretString(String);

#[cfg(feature = "zeroize")]
impl SecretString {
    /// Create a new secret string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Get the secret value.
    ///
    /// # Security
    ///
    /// Be careful with the returned reference - avoid logging or
    /// copying the value unnecessarily.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "zeroize")]
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

#[cfg(feature = "zeroize")]
impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[cfg(feature = "zeroize")]
impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Secure credentials with automatic zeroization on drop.
///
/// This type is only available when the `zeroize` feature is enabled.
/// All sensitive fields are securely zeroed from memory when the
/// credentials are dropped.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_auth::SecureCredentials;
///
/// let creds = SecureCredentials::sql_server("user", "password");
/// // When `creds` goes out of scope, the password is securely zeroed
/// ```
#[cfg(feature = "zeroize")]
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureCredentials {
    kind: SecureCredentialKind,
}

#[cfg(feature = "zeroize")]
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
enum SecureCredentialKind {
    SqlServer {
        username: String,
        password: SecretString,
    },
    AzureAccessToken {
        token: SecretString,
    },
    #[cfg(feature = "azure-identity")]
    AzureManagedIdentity {
        client_id: Option<String>,
    },
    #[cfg(feature = "azure-identity")]
    AzureServicePrincipal {
        tenant_id: String,
        client_id: String,
        client_secret: SecretString,
    },
    #[cfg(feature = "azure-identity")]
    AzureDefault,
    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
    Integrated,
    #[cfg(feature = "cert-auth")]
    Certificate {
        tenant_id: String,
        client_id: String,
        cert_path: String,
        password: Option<SecretString>,
    },
}

#[cfg(feature = "zeroize")]
impl SecureCredentials {
    /// Create SQL Server credentials with secure password handling.
    pub fn sql_server(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            kind: SecureCredentialKind::SqlServer {
                username: username.into(),
                password: SecretString::new(password),
            },
        }
    }

    /// Create Azure access token credentials with secure token handling.
    pub fn azure_token(token: impl Into<String>) -> Self {
        Self {
            kind: SecureCredentialKind::AzureAccessToken {
                token: SecretString::new(token),
            },
        }
    }

    /// Check if these credentials use SQL authentication.
    #[must_use]
    pub fn is_sql_auth(&self) -> bool {
        matches!(self.kind, SecureCredentialKind::SqlServer { .. })
    }

    /// Check if these credentials use Azure AD.
    #[must_use]
    pub fn is_azure_ad(&self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match &self.kind {
            SecureCredentialKind::AzureAccessToken { .. } => true,
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureManagedIdentity { .. }
            | SecureCredentialKind::AzureServicePrincipal { .. } => true,
            _ => false,
        }
    }

    /// Get the authentication method name.
    #[must_use]
    pub fn method_name(&self) -> &'static str {
        match &self.kind {
            SecureCredentialKind::SqlServer { .. } => "SQL Server Authentication",
            SecureCredentialKind::AzureAccessToken { .. } => "Azure AD Access Token",
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureManagedIdentity { .. } => "Azure Managed Identity",
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureServicePrincipal { .. } => "Azure Service Principal",
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureDefault => "Azure Default Credential Chain",
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            SecureCredentialKind::Integrated => "Integrated Authentication",
            #[cfg(feature = "cert-auth")]
            SecureCredentialKind::Certificate { .. } => "Certificate Authentication",
        }
    }

    /// Get the username for SQL Server authentication.
    ///
    /// Returns `None` for non-SQL authentication methods.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        match &self.kind {
            SecureCredentialKind::SqlServer { username, .. } => Some(username),
            _ => None,
        }
    }

    /// Get the password for SQL Server authentication.
    ///
    /// Returns `None` for non-SQL authentication methods.
    ///
    /// # Security
    ///
    /// Be careful with the returned reference - avoid logging or
    /// copying the value unnecessarily.
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        match &self.kind {
            SecureCredentialKind::SqlServer { password, .. } => Some(password.expose_secret()),
            _ => None,
        }
    }

    /// Get the token for Azure AD authentication.
    ///
    /// Returns `None` for non-Azure AD authentication methods.
    ///
    /// # Security
    ///
    /// Be careful with the returned reference - avoid logging or
    /// copying the value unnecessarily.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        match &self.kind {
            SecureCredentialKind::AzureAccessToken { token } => Some(token.expose_secret()),
            _ => None,
        }
    }
}

#[cfg(feature = "zeroize")]
impl std::fmt::Debug for SecureCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            SecureCredentialKind::SqlServer { username, .. } => f
                .debug_struct("SecureCredentials::SqlServer")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            SecureCredentialKind::AzureAccessToken { .. } => f
                .debug_struct("SecureCredentials::AzureAccessToken")
                .field("token", &"[REDACTED]")
                .finish(),
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureManagedIdentity { client_id } => f
                .debug_struct("SecureCredentials::AzureManagedIdentity")
                .field("client_id", client_id)
                .finish(),
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureServicePrincipal {
                tenant_id,
                client_id,
                ..
            } => f
                .debug_struct("SecureCredentials::AzureServicePrincipal")
                .field("tenant_id", tenant_id)
                .field("client_id", client_id)
                .field("client_secret", &"[REDACTED]")
                .finish(),
            #[cfg(feature = "azure-identity")]
            SecureCredentialKind::AzureDefault => {
                f.debug_struct("SecureCredentials::AzureDefault").finish()
            }
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            SecureCredentialKind::Integrated => {
                f.debug_struct("SecureCredentials::Integrated").finish()
            }
            #[cfg(feature = "cert-auth")]
            SecureCredentialKind::Certificate {
                tenant_id,
                client_id,
                cert_path,
                ..
            } => f
                .debug_struct("SecureCredentials::Certificate")
                .field("tenant_id", tenant_id)
                .field("client_id", client_id)
                .field("cert_path", cert_path)
                .field("password", &"[REDACTED]")
                .finish(),
        }
    }
}

/// Convert from non-secure credentials to secure credentials.
#[cfg(feature = "zeroize")]
impl From<Credentials> for SecureCredentials {
    fn from(creds: Credentials) -> Self {
        match creds {
            Credentials::SqlServer { username, password } => {
                SecureCredentials::sql_server(username.into_owned(), password.into_owned())
            }
            Credentials::AzureAccessToken { token } => {
                SecureCredentials::azure_token(token.into_owned())
            }
            #[cfg(feature = "azure-identity")]
            Credentials::AzureManagedIdentity { client_id } => SecureCredentials {
                kind: SecureCredentialKind::AzureManagedIdentity {
                    client_id: client_id.map(|c| c.into_owned()),
                },
            },
            #[cfg(feature = "azure-identity")]
            Credentials::AzureServicePrincipal {
                tenant_id,
                client_id,
                client_secret,
            } => SecureCredentials {
                kind: SecureCredentialKind::AzureServicePrincipal {
                    tenant_id: tenant_id.into_owned(),
                    client_id: client_id.into_owned(),
                    client_secret: SecretString::new(client_secret.into_owned()),
                },
            },
            #[cfg(feature = "azure-identity")]
            Credentials::AzureDefault => SecureCredentials {
                kind: SecureCredentialKind::AzureDefault,
            },
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            Credentials::Integrated => SecureCredentials {
                kind: SecureCredentialKind::Integrated,
            },
            #[cfg(feature = "cert-auth")]
            Credentials::Certificate {
                tenant_id,
                client_id,
                cert_path,
                password,
            } => SecureCredentials {
                kind: SecureCredentialKind::Certificate {
                    tenant_id: tenant_id.into_owned(),
                    client_id: client_id.into_owned(),
                    cert_path: cert_path.into_owned(),
                    password: password.map(|p| SecretString::new(p.into_owned())),
                },
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_sql_server() {
        let creds = Credentials::sql_server("user", "password");
        assert!(creds.is_sql_auth());
        assert!(!creds.is_azure_ad());
        match creds {
            Credentials::SqlServer { username, password } => {
                assert_eq!(username.as_ref(), "user");
                assert_eq!(password.as_ref(), "password");
            }
            _ => panic!("Expected SqlServer variant"),
        }
    }

    #[test]
    fn test_credentials_azure_token() {
        let creds = Credentials::azure_token("my-token");
        assert!(!creds.is_sql_auth());
        assert!(creds.is_azure_ad());
        match creds {
            Credentials::AzureAccessToken { token } => {
                assert_eq!(token.as_ref(), "my-token");
            }
            _ => panic!("Expected AzureAccessToken variant"),
        }
    }

    #[test]
    fn test_credentials_debug_redacts_password() {
        let creds = Credentials::sql_server("user", "supersecret");
        let debug = format!("{creds:?}");
        assert!(debug.contains("user"));
        assert!(!debug.contains("supersecret"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn test_credentials_debug_redacts_token() {
        let creds = Credentials::azure_token("supersecrettoken");
        let debug = format!("{creds:?}");
        assert!(!debug.contains("supersecrettoken"));
        assert!(debug.contains("REDACTED"));
    }

    #[cfg(feature = "cert-auth")]
    #[test]
    fn test_credentials_certificate_constructor_and_debug() {
        let creds =
            Credentials::certificate("tenant-1", "client-1", "/path/app.pfx", Some("pw".into()));
        assert!(!creds.is_sql_auth());
        // Certificate auth is Entra-backed but `is_azure_ad()` reports the
        // pre-acquired/MI/SP token variants only; cert is handled explicitly
        // in the client's FEDAUTH validation.
        assert!(!creds.is_azure_ad());
        assert_eq!(creds.method_name(), "Certificate Authentication");
        match &creds {
            Credentials::Certificate {
                tenant_id,
                client_id,
                cert_path,
                ..
            } => {
                assert_eq!(tenant_id.as_ref(), "tenant-1");
                assert_eq!(client_id.as_ref(), "client-1");
                assert_eq!(cert_path.as_ref(), "/path/app.pfx");
            }
            _ => panic!("Expected Certificate variant"),
        }
        let debug = format!("{creds:?}");
        assert!(debug.contains("tenant-1"));
        assert!(debug.contains("/path/app.pfx"));
        assert!(!debug.contains("pw"));
        assert!(debug.contains("REDACTED"));
    }

    #[cfg(feature = "azure-identity")]
    #[test]
    fn test_credentials_azure_default() {
        let creds = Credentials::azure_default();
        assert!(creds.is_azure_ad());
        assert!(!creds.is_sql_auth());
        assert!(matches!(creds, Credentials::AzureDefault));
        assert_eq!(creds.method_name(), "Azure Default Credential Chain");
        assert_eq!(format!("{creds:?}"), "AzureDefault");
    }

    #[cfg(feature = "zeroize")]
    mod zeroize_tests {
        use super::*;

        #[test]
        fn test_secret_string_creation() {
            let secret = SecretString::new("my-password");
            assert_eq!(secret.expose_secret(), "my-password");
        }

        #[test]
        fn test_secret_string_zeroize_clears_value() {
            let mut secret = SecretString::new("super-secret");
            secret.zeroize();
            // `zeroize()` is what `ZeroizeOnDrop` runs on drop; it must scrub
            // the secret. Reading the buffer after the value is dropped would
            // be UB, so this asserts the scrubbing operation directly.
            assert!(secret.expose_secret().is_empty());
        }

        #[test]
        fn test_secret_string_is_zeroize_on_drop() {
            // Compile-time guarantee that `SecretString` is scrubbed on drop.
            fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
            assert_zeroize_on_drop::<SecretString>();
        }

        #[test]
        fn test_secret_string_from_string() {
            let secret: SecretString = String::from("password").into();
            assert_eq!(secret.expose_secret(), "password");
        }

        #[test]
        fn test_secret_string_from_str() {
            let secret: SecretString = "password".into();
            assert_eq!(secret.expose_secret(), "password");
        }

        #[test]
        fn test_secret_string_debug_redacted() {
            let secret = SecretString::new("supersecret");
            let debug = format!("{secret:?}");
            assert!(!debug.contains("supersecret"));
            assert!(debug.contains("REDACTED"));
        }

        #[test]
        fn test_secret_string_clone() {
            let secret = SecretString::new("password");
            let cloned = secret.clone();
            assert_eq!(cloned.expose_secret(), "password");
        }

        #[test]
        fn test_secure_credentials_sql_server() {
            let creds = SecureCredentials::sql_server("user", "password");
            assert_eq!(creds.username(), Some("user"));
            assert_eq!(creds.password(), Some("password"));
            assert!(creds.token().is_none());
        }

        #[test]
        fn test_secure_credentials_azure_token() {
            let creds = SecureCredentials::azure_token("my-token");
            assert!(creds.username().is_none());
            assert!(creds.password().is_none());
            assert_eq!(creds.token(), Some("my-token"));
        }

        #[test]
        fn test_secure_credentials_debug_redacts_password() {
            let creds = SecureCredentials::sql_server("user", "supersecret");
            let debug = format!("{creds:?}");
            assert!(debug.contains("user"));
            assert!(!debug.contains("supersecret"));
            assert!(debug.contains("REDACTED"));
        }

        #[test]
        fn test_secure_credentials_debug_redacts_token() {
            let creds = SecureCredentials::azure_token("supersecrettoken");
            let debug = format!("{creds:?}");
            assert!(!debug.contains("supersecrettoken"));
            assert!(debug.contains("REDACTED"));
        }

        #[test]
        fn test_secure_credentials_from_credentials() {
            let creds = Credentials::sql_server("user", "password");
            let secure: SecureCredentials = creds.into();
            assert_eq!(secure.username(), Some("user"));
            assert_eq!(secure.password(), Some("password"));
        }

        #[test]
        fn test_secure_credentials_clone() {
            let creds = SecureCredentials::sql_server("user", "password");
            let cloned = creds.clone();
            assert_eq!(cloned.username(), Some("user"));
            assert_eq!(cloned.password(), Some("password"));
        }
    }
}
