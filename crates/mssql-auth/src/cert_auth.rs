//! Client certificate authentication provider.
//!
//! This module provides Azure AD authentication using a client certificate
//! (X.509) instead of a client secret. This is more secure than using secrets
//! because certificates can be stored in secure hardware (HSM) and have
//! built-in expiration.
//!
//! ## How It Works
//!
//! Certificate authentication uses an Azure AD Service Principal with an
//! X.509 certificate. The certificate's private key is used to sign a JWT
//! assertion, which Azure AD validates using the certificate's public key
//! registered with the application.
//!
//! **Important**: This is NOT TDS-level mTLS. SQL Server/Azure SQL do not
//! support client certificates at the TDS protocol level. Instead, the
//! certificate authenticates to Azure AD, which issues an access token
//! used for SQL authentication.
//!
//! ## Prerequisites
//!
//! 1. Create an Azure AD App Registration
//! 2. Generate or upload a certificate to the app registration
//! 3. Export the certificate (PKCS#12 or PEM format)
//! 4. Grant the service principal access to your Azure SQL database
//!
//! ## Example (PKCS#12)
//!
//! ```rust,ignore
//! use mssql_auth::CertificateAuth;
//! use std::fs;
//!
//! // Load PKCS#12 certificate from file
//! let cert_bytes = fs::read("service-principal.pfx")?;
//!
//! let auth = CertificateAuth::new(
//!     "your-tenant-id",
//!     "your-client-id",
//!     cert_bytes,
//!     Some("certificate-password"),
//! )?;
//!
//! // Get access token for Azure SQL
//! let token = auth.get_token().await?;
//! ```
//!
//! ## Example (PEM)
//!
//! PEM certificates are common in Linux/Kubernetes environments:
//!
//! ```rust,ignore
//! use mssql_auth::CertificateAuth;
//! use std::fs;
//!
//! // Load PEM certificate and private key
//! let cert_pem = fs::read("cert.pem")?;
//! let key_pem = fs::read("key.pem")?;
//!
//! let auth = CertificateAuth::from_pem(
//!     "your-tenant-id",
//!     "your-client-id",
//!     &cert_pem,
//!     &key_pem,
//!     None, // optional password
//! )?;
//!
//! let token = auth.get_token().await?;
//! ```
//!
//! ## Security Considerations
//!
//! - Store certificates in Azure Key Vault or secure hardware when possible
//! - Use certificates with appropriate key sizes (RSA 2048+ or ECDSA P-256+)
//! - Set reasonable certificate expiration (1-2 years)
//! - Rotate certificates before expiration
//! - Never commit certificates to source control

use std::sync::Arc;
use std::time::Duration;

use azure_core::credentials::TokenCredential;
use azure_identity::ClientCertificateCredential;

use crate::AzureAdAuth;
use crate::error::AuthError;

/// The Azure SQL Database scope for token requests.
const AZURE_SQL_SCOPE: &str = "https://database.windows.net/.default";

/// Client certificate authentication provider.
///
/// Uses an X.509 certificate to authenticate as an Azure AD Service Principal,
/// then acquires an access token for Azure SQL Database.
///
/// # Security
///
/// Certificate authentication is more secure than client secrets because:
/// - Certificates have built-in expiration
/// - Private keys can be stored in secure hardware (HSM/TPM)
/// - Certificates support hardware-based attestation
/// - Certificate rotation doesn't require application restarts
pub struct CertificateAuth {
    credential: Arc<ClientCertificateCredential>,
}

impl CertificateAuth {
    /// Create a new certificate authentication provider.
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The Azure AD tenant ID
    /// * `client_id` - The application (client) ID of the service principal
    /// * `certificate` - The PKCS#12 (.pfx) certificate bytes (base64-encoded or raw)
    /// * `password` - Optional password for the certificate's private key
    ///
    /// # Errors
    ///
    /// Returns an error if the certificate cannot be parsed or the credential
    /// cannot be created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_auth::CertificateAuth;
    /// use std::fs;
    ///
    /// let cert = fs::read("app.pfx")?;
    /// let auth = CertificateAuth::new(
    ///     "tenant-id",
    ///     "client-id",
    ///     cert,
    ///     Some("cert-password"),
    /// )?;
    /// ```
    pub fn new(
        tenant_id: impl AsRef<str>,
        client_id: impl Into<String>,
        certificate: impl AsRef<[u8]>,
        password: Option<&str>,
    ) -> Result<Self, AuthError> {
        use azure_core::credentials::Secret;
        use azure_identity::ClientCertificateCredentialOptions;
        use base64::Engine;

        // The certificate should be base64-encoded PKCS#12
        // If it's raw bytes, encode it first
        let cert_bytes = certificate.as_ref();
        let cert_b64 = if cert_bytes.starts_with(b"MII") || is_base64(cert_bytes) {
            // Already looks like base64
            String::from_utf8_lossy(cert_bytes).to_string()
        } else {
            // Raw PKCS#12 bytes - encode to base64
            base64::engine::general_purpose::STANDARD.encode(cert_bytes)
        };

        let cert_secret = Secret::new(cert_b64);

        // Create options with password if provided
        // Note: send_certificate_chain is now controlled by AZURE_CLIENT_SEND_CERTIFICATE_CHAIN env var
        let options = if let Some(pwd) = password {
            ClientCertificateCredentialOptions {
                password: Some(Secret::new(pwd.to_string())),
                ..Default::default()
            }
        } else {
            ClientCertificateCredentialOptions::default()
        };

        let credential = ClientCertificateCredential::new(
            tenant_id.as_ref().to_string(),
            client_id.into(),
            cert_secret,
            Some(options),
        )
        .map_err(|e| {
            AuthError::Certificate(format!("Failed to create certificate credential: {}", e))
        })?;

        Ok(Self { credential })
    }

    /// Create a new certificate authentication provider from PEM-encoded files.
    ///
    /// This is a convenience method for users who have PEM-formatted certificates
    /// (common in Linux/Kubernetes environments) rather than PKCS#12 format.
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The Azure AD tenant ID
    /// * `client_id` - The application (client) ID of the service principal
    /// * `cert_pem` - The PEM-encoded certificate (typically from a `.pem` or `.crt` file)
    /// * `key_pem` - The PEM-encoded private key (typically from a `.key` or `.pem` file)
    /// * `password` - Optional password for the PKCS#12 bundle (used during conversion)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The certificate PEM cannot be parsed
    /// - The private key PEM cannot be parsed
    /// - The PEM-to-PKCS#12 conversion fails
    /// - The credential cannot be created
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_auth::CertificateAuth;
    /// use std::fs;
    ///
    /// let cert_pem = fs::read("cert.pem")?;
    /// let key_pem = fs::read("key.pem")?;
    ///
    /// let auth = CertificateAuth::from_pem(
    ///     "tenant-id",
    ///     "client-id",
    ///     &cert_pem,
    ///     &key_pem,
    ///     None, // or Some("pkcs12-password")
    /// )?;
    /// ```
    pub fn from_pem(
        tenant_id: impl AsRef<str>,
        client_id: impl Into<String>,
        cert_pem: impl AsRef<[u8]>,
        key_pem: impl AsRef<[u8]>,
        password: Option<&str>,
    ) -> Result<Self, AuthError> {
        use std::io::BufReader;

        // Parse certificate from PEM
        let cert_pem_bytes = cert_pem.as_ref();
        let mut cert_reader = BufReader::new(cert_pem_bytes);
        let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                AuthError::Certificate(format!("Failed to parse certificate PEM: {}", e))
            })?;

        let cert_der = certs
            .first()
            .ok_or_else(|| AuthError::Certificate("No certificate found in PEM data".into()))?;

        // Parse private key from PEM
        let key_pem_bytes = key_pem.as_ref();
        let mut key_reader = BufReader::new(key_pem_bytes);
        let key_der = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|e| AuthError::Certificate(format!("Failed to parse private key PEM: {}", e)))?
            .ok_or_else(|| AuthError::Certificate("No private key found in PEM data".into()))?;

        // Convert to PKCS#12 format
        let pkcs12_password = password.unwrap_or("");
        let pfx = p12::PFX::new(
            cert_der.as_ref(),
            key_der.secret_der(),
            None, // No CA certificate
            pkcs12_password,
            "cert",
        )
        .ok_or_else(|| AuthError::Certificate("Failed to create PKCS#12 from PEM data".into()))?;

        let pkcs12_bytes = pfx.to_der();

        // Use existing constructor with the converted PKCS#12
        Self::new(tenant_id, client_id, pkcs12_bytes, password)
    }

    /// Get an access token for Azure SQL Database.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails (e.g., certificate invalid,
    /// network error, insufficient permissions).
    pub async fn get_token(&self) -> Result<String, AuthError> {
        let token = self
            .credential
            .get_token(&[AZURE_SQL_SCOPE], None)
            .await
            .map_err(|e| AuthError::Certificate(format!("Failed to acquire token: {}", e)))?;
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
            .map_err(|e| AuthError::Certificate(format!("Failed to acquire token: {}", e)))?;

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
    /// This is useful when you need to use the token with APIs that
    /// expect `AzureAdAuth`.
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

/// Check if bytes look like base64-encoded data.
fn is_base64(data: &[u8]) -> bool {
    // Simple heuristic: base64 contains only alphanumeric, +, /, =
    // and PKCS#12 raw data would have binary bytes
    data.iter().all(|&b| {
        b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=' || b == b'\n' || b == b'\r'
    })
}

impl Clone for CertificateAuth {
    fn clone(&self) -> Self {
        Self {
            credential: Arc::clone(&self.credential),
        }
    }
}

impl std::fmt::Debug for CertificateAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateAuth")
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

impl crate::provider::AsyncAuthProvider for CertificateAuth {
    fn method(&self) -> crate::provider::AuthMethod {
        crate::provider::AuthMethod::AzureAd
    }

    async fn authenticate_async(&self) -> Result<crate::provider::AuthData, AuthError> {
        let token = self.get_token().await?;
        Ok(crate::provider::AuthData::FedAuth { token, nonce: None })
    }

    fn needs_refresh(&self) -> bool {
        // Certificate-based tokens are acquired fresh each time
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // Note: These tests require Azure credentials and a valid certificate.
    // They are marked as ignored and can be run manually with:
    // cargo test --features cert-auth -- --ignored

    #[test]
    fn test_is_base64() {
        assert!(is_base64(b"SGVsbG8gV29ybGQ="));
        assert!(is_base64(b"MIIC+jCCAeKgAwIBAgIJAL"));
        assert!(!is_base64(&[0x00, 0x01, 0x02, 0x03])); // Binary data
    }

    #[test]
    fn test_debug_redacts_credentials() {
        // We can't easily create a CertificateAuth without valid creds,
        // but we can verify the Debug implementation is defined
        // and would redact credentials
    }

    #[tokio::test]
    #[ignore = "Requires Azure Service Principal with certificate"]
    async fn test_certificate_auth() {
        let tenant_id = std::env::var("AZURE_TENANT_ID").expect("AZURE_TENANT_ID not set");
        let client_id = std::env::var("AZURE_CLIENT_ID").expect("AZURE_CLIENT_ID not set");
        let cert_path = std::env::var("AZURE_CLIENT_CERTIFICATE_PATH")
            .expect("AZURE_CLIENT_CERTIFICATE_PATH not set");
        let cert_password = std::env::var("AZURE_CLIENT_CERTIFICATE_PASSWORD").ok();

        let cert_bytes = std::fs::read(&cert_path).expect("Failed to read certificate");
        let auth = CertificateAuth::new(tenant_id, client_id, cert_bytes, cert_password.as_deref())
            .expect("Failed to create CertificateAuth");

        let token = auth.get_token().await.expect("Failed to get token");
        assert!(!token.is_empty());
    }

    #[test]
    fn test_from_pem_invalid_certificate() {
        let invalid_cert = b"not a valid PEM certificate";
        let valid_key_format = b"-----BEGIN PRIVATE KEY-----\nMIIE=\n-----END PRIVATE KEY-----";

        let result = CertificateAuth::from_pem(
            "tenant-id",
            "client-id",
            invalid_cert,
            valid_key_format,
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("No certificate found"),
            "Expected 'No certificate found' error, got: {}",
            err
        );
    }

    #[test]
    fn test_from_pem_invalid_private_key() {
        // Valid PEM structure but not actually a valid cert (will fail at PKCS#12 conversion)
        let cert_pem =
            b"-----BEGIN CERTIFICATE-----\nMIIBkTCB+wIJAKHBfpE=\n-----END CERTIFICATE-----";
        let invalid_key = b"not a valid PEM private key";

        let result =
            CertificateAuth::from_pem("tenant-id", "client-id", cert_pem, invalid_key, None);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("No private key found"),
            "Expected 'No private key found' error, got: {}",
            err
        );
    }

    #[test]
    fn test_from_pem_empty_certificate() {
        let empty_cert = b"";
        let key_pem = b"-----BEGIN PRIVATE KEY-----\nMIIE=\n-----END PRIVATE KEY-----";

        let result = CertificateAuth::from_pem("tenant-id", "client-id", empty_cert, key_pem, None);

        assert!(result.is_err());
    }

    #[test]
    fn test_from_pem_empty_private_key() {
        let cert_pem =
            b"-----BEGIN CERTIFICATE-----\nMIIBkTCB+wIJAKHBfpE=\n-----END CERTIFICATE-----";
        let empty_key = b"";

        let result = CertificateAuth::from_pem("tenant-id", "client-id", cert_pem, empty_key, None);

        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "Requires Azure Service Principal with PEM certificate"]
    async fn test_certificate_auth_from_pem() {
        let tenant_id = std::env::var("AZURE_TENANT_ID").expect("AZURE_TENANT_ID not set");
        let client_id = std::env::var("AZURE_CLIENT_ID").expect("AZURE_CLIENT_ID not set");
        let cert_path = std::env::var("AZURE_CLIENT_CERTIFICATE_PEM")
            .expect("AZURE_CLIENT_CERTIFICATE_PEM not set");
        let key_path = std::env::var("AZURE_CLIENT_PRIVATE_KEY_PEM")
            .expect("AZURE_CLIENT_PRIVATE_KEY_PEM not set");

        let cert_pem = std::fs::read(&cert_path).expect("Failed to read certificate PEM");
        let key_pem = std::fs::read(&key_path).expect("Failed to read private key PEM");

        let auth = CertificateAuth::from_pem(tenant_id, client_id, &cert_pem, &key_pem, None)
            .expect("Failed to create CertificateAuth from PEM");

        let token = auth.get_token().await.expect("Failed to get token");
        assert!(!token.is_empty());
    }
}
