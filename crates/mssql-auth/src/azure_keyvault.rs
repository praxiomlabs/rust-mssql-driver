//! Azure Key Vault Column Master Key (CMK) provider for Always Encrypted.
//!
//! This module provides integration with Azure Key Vault for secure key management
//! in SQL Server Always Encrypted scenarios.
//!
//! ## Overview
//!
//! Azure Key Vault is Microsoft's cloud-based key management service that provides
//! secure storage and access to cryptographic keys. This provider uses Azure Key Vault's
//! "unwrap" operation to decrypt Column Encryption Keys (CEKs) using Column Master Keys
//! (CMKs) stored in the vault.
//!
//! ## CMK Path Format
//!
//! The CMK path for Azure Key Vault follows this format:
//!
//! ```text
//! https://<vault-name>.vault.azure.net/keys/<key-name>/<key-version>
//! ```
//!
//! The key version is optional - if omitted, the latest version is used.
//!
//! ## Authentication
//!
//! The provider uses Azure Identity for authentication. The following methods are supported:
//!
//! - **DefaultAzureCredential**: Tries multiple authentication methods automatically
//! - **Environment variables**: Uses `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`
//! - **Managed Identity**: For Azure VMs, App Service, AKS, etc.
//! - **Azure CLI**: Uses credentials from `az login`
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_auth::azure_keyvault::AzureKeyVaultProvider;
//! use mssql_auth::ColumnEncryptionConfig;
//!
//! // Create provider with default Azure credentials
//! let provider = AzureKeyVaultProvider::new()?;
//!
//! // Or with a specific credential
//! let credential = azure_identity::DeveloperToolsCredential::new(None)?;
//! let provider = AzureKeyVaultProvider::with_credential(Arc::new(credential));
//!
//! // Register with encryption config
//! let config = ColumnEncryptionConfig::new()
//!     .with_provider(provider);
//! ```
//!
//! ## Security Considerations
//!
//! - Keys never leave Azure Key Vault; only the unwrap operation is performed
//! - Access is controlled via Azure RBAC or Key Vault access policies
//! - All communication uses TLS
//! - Audit logs are available in Azure Key Vault

use std::sync::Arc;

use azure_core::http::RequestContent;
use azure_identity::DeveloperToolsCredential;
use azure_security_keyvault_keys::KeyClient;
use azure_security_keyvault_keys::models::{EncryptionAlgorithm, KeyOperationParameters};
use tracing::{debug, instrument};
use url::Url;

use crate::encryption::{EncryptionError, KeyStoreProvider};

/// SQL Server provider name for Azure Key Vault.
const PROVIDER_NAME: &str = "AZURE_KEY_VAULT";

/// Default trusted Key Vault / Managed HSM DNS suffixes across Azure clouds.
///
/// The CMK path in Always Encrypted metadata is supplied by the server. A
/// malicious or compromised server could point it at an attacker-controlled
/// host, causing this provider to send a Key-Vault-scoped bearer token there
/// (token exfiltration / SSRF). Restricting the host to these
/// Microsoft-operated suffixes closes that vector; custom or private
/// deployments can override via
/// [`AzureKeyVaultProvider::with_trusted_endpoints`].
const DEFAULT_TRUSTED_KEY_VAULT_SUFFIXES: &[&str] = &[
    ".vault.azure.net",              // Azure public cloud
    ".vaultcore.azure.net",          // Azure public cloud (data-plane alias)
    ".managedhsm.azure.net",         // Managed HSM, public cloud
    ".vault.azure.cn",               // Azure China (21Vianet)
    ".managedhsm.azure.cn",          // Managed HSM, Azure China
    ".vault.usgovcloudapi.net",      // Azure US Government
    ".managedhsm.usgovcloudapi.net", // Managed HSM, US Government
    ".vault.microsoftazure.de",      // Azure Germany (legacy)
];

/// Azure Key Vault Column Master Key provider.
///
/// This provider implements the [`KeyStoreProvider`] trait to support
/// Always Encrypted operations using keys stored in Azure Key Vault.
///
/// ## Thread Safety
///
/// This provider is `Send + Sync` and can be safely shared across threads.
pub struct AzureKeyVaultProvider {
    /// Azure credential for authentication.
    credential: Arc<DeveloperToolsCredential>,
    /// Host suffixes a server-supplied CMK path is allowed to target.
    /// Defaults to `DEFAULT_TRUSTED_KEY_VAULT_SUFFIXES`.
    trusted_host_suffixes: Vec<String>,
}

fn default_trusted_suffixes() -> Vec<String> {
    DEFAULT_TRUSTED_KEY_VAULT_SUFFIXES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

impl AzureKeyVaultProvider {
    /// Create a new Azure Key Vault provider with default credentials.
    ///
    /// This uses [`DeveloperToolsCredential`] which tries multiple authentication
    /// methods in order:
    ///
    /// 1. Azure CLI credentials (`az login`)
    /// 2. Other developer tools (Visual Studio Code, etc.)
    ///
    /// For production environments, use [`Self::with_credential`] with a specific
    /// credential type such as managed identity or service principal.
    ///
    /// # Errors
    ///
    /// Returns an error if credential initialization fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let provider = AzureKeyVaultProvider::new()?;
    /// ```
    pub fn new() -> Result<Self, EncryptionError> {
        let credential = DeveloperToolsCredential::new(None).map_err(|e| {
            EncryptionError::ConfigurationError(format!("Failed to create Azure credential: {e}"))
        })?;
        Ok(Self {
            credential,
            trusted_host_suffixes: default_trusted_suffixes(),
        })
    }

    /// Create a new Azure Key Vault provider with an existing credential.
    ///
    /// Use this when you need to share a credential across multiple providers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use azure_identity::DeveloperToolsCredential;
    ///
    /// let credential = Arc::new(DeveloperToolsCredential::new(None)?);
    /// let provider = AzureKeyVaultProvider::with_credential(credential);
    /// ```
    #[must_use]
    pub fn with_credential(credential: Arc<DeveloperToolsCredential>) -> Self {
        Self {
            credential,
            trusted_host_suffixes: default_trusted_suffixes(),
        }
    }

    /// Override the set of host suffixes a server-supplied CMK path may target.
    ///
    /// By default only Microsoft-operated Key Vault / Managed HSM endpoints
    /// (`.vault.azure.net`, `.managedhsm.azure.net`, and the China / US-Gov /
    /// legacy-Germany variants) are accepted, so a malicious server cannot
    /// redirect key operations to an attacker-controlled host.
    /// Use this only for private or sovereign deployments with custom DNS, and
    /// pass suffixes you fully control (e.g. `".vault.contoso.example"`).
    /// Replacing the list with an over-broad suffix re-opens the SSRF /
    /// token-exfiltration vector this guard closes.
    #[must_use]
    pub fn with_trusted_endpoints<I, S>(mut self, suffixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.trusted_host_suffixes = suffixes.into_iter().map(Into::into).collect();
        self
    }

    /// Parse a CMK path into vault URL, key name, and optional version.
    ///
    /// Expected format: `https://<vault>.vault.azure.net/keys/<key-name>[/<version>]`
    ///
    /// The scheme must be `https` and the host must match one of
    /// `trusted_suffixes`; otherwise an error is returned, since the path
    /// originates from the (untrusted) server.
    fn parse_cmk_path(
        cmk_path: &str,
        trusted_suffixes: &[String],
    ) -> Result<(String, String, Option<String>), EncryptionError> {
        let url = Url::parse(cmk_path).map_err(|e| {
            EncryptionError::CmkError(format!("Invalid CMK path '{cmk_path}': {e}"))
        })?;

        if url.scheme() != "https" {
            return Err(EncryptionError::CmkError(format!(
                "CMK path must use https, got scheme '{}' in '{cmk_path}'",
                url.scheme()
            )));
        }

        let host = url
            .host_str()
            .ok_or_else(|| EncryptionError::CmkError("CMK path missing host".into()))?;
        let host_lc = host.to_ascii_lowercase();
        let trusted = trusted_suffixes
            .iter()
            .any(|suffix| host_lc.ends_with(&suffix.to_ascii_lowercase()));
        if !trusted {
            return Err(EncryptionError::CmkError(format!(
                "CMK host '{host}' is not a trusted Key Vault endpoint. The CMK path is \
                 supplied by the server; allowing an arbitrary host would let a malicious \
                 server redirect key operations and exfiltrate access tokens. Trusted \
                 suffixes: {trusted_suffixes:?}. For custom deployments use \
                 AzureKeyVaultProvider::with_trusted_endpoints."
            )));
        }

        // Extract vault URL (scheme + host)
        let vault_url = format!("{}://{host}", url.scheme());

        // Parse path segments: /keys/<name>[/<version>]
        let segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();

        if segments.len() < 2 || segments[0] != "keys" {
            return Err(EncryptionError::CmkError(format!(
                "Invalid CMK path format: expected /keys/<name>[/<version>], got '{}'",
                url.path()
            )));
        }

        let key_name = segments[1].to_string();
        let key_version = if segments.len() >= 3 && !segments[2].is_empty() {
            Some(segments[2].to_string())
        } else {
            None
        };

        Ok((vault_url, key_name, key_version))
    }

    /// Create a Key Vault client for a specific vault.
    fn create_client(&self, vault_url: &str) -> Result<KeyClient, EncryptionError> {
        KeyClient::new(vault_url, self.credential.clone(), None).map_err(|e| {
            EncryptionError::CmkError(format!("Failed to create Key Vault client: {e}"))
        })
    }
}

impl std::fmt::Debug for AzureKeyVaultProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureKeyVaultProvider")
            .field("provider_name", &PROVIDER_NAME)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl KeyStoreProvider for AzureKeyVaultProvider {
    fn provider_name(&self) -> &str {
        PROVIDER_NAME
    }

    #[instrument(skip(self, encrypted_cek), fields(cmk_path = %cmk_path, algorithm = %algorithm))]
    async fn decrypt_cek(
        &self,
        cmk_path: &str,
        algorithm: &str,
        encrypted_cek: &[u8],
    ) -> Result<Vec<u8>, EncryptionError> {
        debug!("Decrypting CEK using Azure Key Vault");

        // Parse the CMK path
        let (vault_url, key_name, key_version) =
            Self::parse_cmk_path(cmk_path, &self.trusted_host_suffixes)?;

        // Create client for this vault
        let client = self.create_client(&vault_url)?;

        // Map algorithm name to Azure Key Vault algorithm
        let kv_algorithm = map_algorithm(algorithm)?;

        // Parse the canonical encrypted-CEK envelope
        let envelope = crate::cek_envelope::parse(encrypted_cek)?;

        // Verify the envelope signature against the CMK (mandatory, matching
        // the reference implementation) before asking the vault to unwrap.
        let digest = envelope.signed_digest();
        let valid = self
            .verify_signature(cmk_path, &digest, envelope.signature)
            .await?;
        if !valid {
            return Err(EncryptionError::CekDecryptionFailed(
                "CEK envelope signature verification failed".into(),
            ));
        }

        // Build unwrap parameters
        let parameters = KeyOperationParameters {
            algorithm: Some(kv_algorithm),
            value: Some(envelope.ciphertext.to_vec()),
            ..Default::default()
        };

        // key_version is required by the Azure SDK 0.13+ API
        let version = key_version.ok_or_else(|| {
            EncryptionError::CmkError(
                "CMK path must include key version (e.g., /keys/<name>/<version>)".into(),
            )
        })?;

        // Convert parameters to RequestContent
        let request_content: RequestContent<KeyOperationParameters> =
            parameters.try_into().map_err(|e| {
                EncryptionError::CekDecryptionFailed(format!("Failed to create request: {e}"))
            })?;

        // Call Key Vault unwrap operation
        let result = client
            .unwrap_key(&key_name, &version, request_content, None)
            .await
            .map_err(|e| {
                EncryptionError::CekDecryptionFailed(format!("Key Vault unwrap failed: {e}"))
            })?
            .into_model()
            .map_err(|e| {
                EncryptionError::CekDecryptionFailed(format!("Failed to parse response: {e}"))
            })?;

        // Extract the decrypted CEK from response
        let decrypted = result.result.ok_or_else(|| {
            EncryptionError::CekDecryptionFailed("Key Vault unwrap returned no result".into())
        })?;

        debug!("Successfully decrypted CEK using Azure Key Vault");
        Ok(decrypted)
    }

    #[instrument(skip(self, data), fields(cmk_path = %cmk_path))]
    async fn sign_data(&self, cmk_path: &str, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        debug!("Signing data using Azure Key Vault");

        // Parse the CMK path
        let (vault_url, key_name, key_version) =
            Self::parse_cmk_path(cmk_path, &self.trusted_host_suffixes)?;

        // Create client for this vault
        let client = self.create_client(&vault_url)?;

        // Build sign parameters - use RS256 (RSA-SHA256) by default
        use azure_security_keyvault_keys::models::{
            KeyClientSignOptions, SignParameters, SignatureAlgorithm,
        };

        let parameters = SignParameters {
            algorithm: Some(SignatureAlgorithm::Rs256),
            value: Some(data.to_vec()),
        };

        // key_version is required for this operation. As of azure_security_keyvault_keys
        // 1.0, sign() takes the key version via options.key_version rather than as a
        // positional argument (verify()/unwrap_key() still take it positionally).
        let version = key_version.ok_or_else(|| {
            EncryptionError::CmkError("CMK path must include key version for sign operation".into())
        })?;

        let request_content: RequestContent<SignParameters> = parameters
            .try_into()
            .map_err(|e| EncryptionError::CmkError(format!("Failed to create request: {e}")))?;

        let sign_options = KeyClientSignOptions {
            key_version: Some(version),
            ..Default::default()
        };

        // Call Key Vault sign operation
        let result = client
            .sign(&key_name, request_content, Some(sign_options))
            .await
            .map_err(|e| EncryptionError::CmkError(format!("Key Vault sign failed: {e}")))?
            .into_model()
            .map_err(|e| EncryptionError::CmkError(format!("Failed to parse response: {e}")))?;

        // Extract the signature from response
        let signature = result
            .result
            .ok_or_else(|| EncryptionError::CmkError("Key Vault sign returned no result".into()))?;

        debug!("Successfully signed data using Azure Key Vault");
        Ok(signature)
    }

    #[instrument(skip(self, data, signature), fields(cmk_path = %cmk_path))]
    async fn verify_signature(
        &self,
        cmk_path: &str,
        data: &[u8],
        signature: &[u8],
    ) -> Result<bool, EncryptionError> {
        debug!("Verifying signature using Azure Key Vault");

        // Parse the CMK path
        let (vault_url, key_name, key_version) =
            Self::parse_cmk_path(cmk_path, &self.trusted_host_suffixes)?;

        // Create client for this vault
        let client = self.create_client(&vault_url)?;

        // Build verify parameters
        use azure_security_keyvault_keys::models::{SignatureAlgorithm, VerifyParameters};

        let parameters = VerifyParameters {
            algorithm: Some(SignatureAlgorithm::Rs256),
            digest: Some(data.to_vec()),
            signature: Some(signature.to_vec()),
        };

        // key_version is required by the Azure SDK 0.13+ API
        let version = key_version.ok_or_else(|| {
            EncryptionError::CmkError(
                "CMK path must include key version for verify operation".into(),
            )
        })?;

        let request_content: RequestContent<VerifyParameters> = parameters
            .try_into()
            .map_err(|e| EncryptionError::CmkError(format!("Failed to create request: {e}")))?;

        // Call Key Vault verify operation
        let result = client
            .verify(&key_name, &version, request_content, None)
            .await
            .map_err(|e| EncryptionError::CmkError(format!("Key Vault verify failed: {e}")))?
            .into_model()
            .map_err(|e| EncryptionError::CmkError(format!("Failed to parse response: {e}")))?;

        // Extract the verification result
        // KeyVerifyResult has a `value` field of type Option<bool>
        let is_valid = result.value.unwrap_or(false);

        debug!("Signature verification result: {}", is_valid);
        Ok(is_valid)
    }
}

/// Map SQL Server algorithm name to Azure Key Vault algorithm.
fn map_algorithm(algorithm: &str) -> Result<EncryptionAlgorithm, EncryptionError> {
    match algorithm.to_uppercase().as_str() {
        "RSA_OAEP" | "RSA-OAEP" => Ok(EncryptionAlgorithm::RsaOaep),
        "RSA_OAEP_256" | "RSA-OAEP-256" => Ok(EncryptionAlgorithm::RsaOaep256),
        "RSA1_5" | "RSA-1_5" => Ok(EncryptionAlgorithm::Rsa1_5),
        _ => Err(EncryptionError::ConfigurationError(format!(
            "Unsupported key encryption algorithm: {algorithm}. Expected RSA_OAEP, RSA_OAEP_256, or RSA1_5"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn trusted() -> Vec<String> {
        default_trusted_suffixes()
    }

    #[test]
    fn test_parse_cmk_path() {
        // Full path with version
        let (vault, name, version) = AzureKeyVaultProvider::parse_cmk_path(
            "https://myvault.vault.azure.net/keys/mykey/abc123",
            &trusted(),
        )
        .expect("valid CMK path with version should parse");
        assert_eq!(vault, "https://myvault.vault.azure.net");
        assert_eq!(name, "mykey");
        assert_eq!(version, Some("abc123".to_string()));

        // Path without version
        let (vault, name, version) = AzureKeyVaultProvider::parse_cmk_path(
            "https://myvault.vault.azure.net/keys/mykey",
            &trusted(),
        )
        .expect("valid CMK path without version should parse");
        assert_eq!(vault, "https://myvault.vault.azure.net");
        assert_eq!(name, "mykey");
        assert_eq!(version, None);

        // Path with trailing slash (no version)
        let (vault, name, version) = AzureKeyVaultProvider::parse_cmk_path(
            "https://myvault.vault.azure.net/keys/mykey/",
            &trusted(),
        )
        .expect("valid CMK path with trailing slash should parse");
        assert_eq!(vault, "https://myvault.vault.azure.net");
        assert_eq!(name, "mykey");
        assert_eq!(version, None);

        // Managed HSM and a sovereign-cloud host are also trusted by default.
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://myhsm.managedhsm.azure.net/keys/mykey",
                &trusted(),
            )
            .is_ok()
        );
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://myvault.vault.usgovcloudapi.net/keys/mykey",
                &trusted(),
            )
            .is_ok()
        );
    }

    #[test]
    fn test_parse_cmk_path_invalid() {
        // Not a URL
        assert!(AzureKeyVaultProvider::parse_cmk_path("not-a-url", &trusted()).is_err());

        // Wrong path format (host is trusted so we reach the path check)
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://myvault.vault.azure.net/secrets/mysecret",
                &trusted(),
            )
            .is_err()
        );

        // Missing key name
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://myvault.vault.azure.net/keys",
                &trusted(),
            )
            .is_err()
        );
    }

    /// Issue #162: a server-supplied CMK path pointing at an untrusted host
    /// must be rejected — otherwise a malicious server could redirect key
    /// operations and exfiltrate the Key-Vault-scoped access token.
    #[test]
    fn test_parse_cmk_path_rejects_untrusted_host() {
        // Attacker-controlled host
        let err = AzureKeyVaultProvider::parse_cmk_path(
            "https://attacker.example.com/keys/mykey",
            &trusted(),
        )
        .expect_err("untrusted host must be rejected");
        assert!(err.to_string().contains("not a trusted Key Vault endpoint"));

        // Suffix-spoofing: trusted suffix appears as a non-suffix label.
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://vault.azure.net.attacker.com/keys/mykey",
                &trusted(),
            )
            .is_err()
        );

        // http downgrade is rejected even for a trusted host.
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "http://myvault.vault.azure.net/keys/mykey",
                &trusted(),
            )
            .is_err()
        );
    }

    /// Issue #162: a custom trusted-endpoint list permits private deployments
    /// while still rejecting everything outside it.
    #[test]
    fn test_with_trusted_endpoints_override() {
        let custom = vec![".vault.contoso.example".to_string()];
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://kv1.vault.contoso.example/keys/mykey",
                &custom,
            )
            .is_ok()
        );
        // The Azure default is no longer trusted under the override.
        assert!(
            AzureKeyVaultProvider::parse_cmk_path(
                "https://myvault.vault.azure.net/keys/mykey",
                &custom,
            )
            .is_err()
        );
    }

    #[test]
    fn test_map_algorithm() {
        assert!(matches!(
            map_algorithm("RSA_OAEP").expect("RSA_OAEP should be a valid algorithm"),
            EncryptionAlgorithm::RsaOaep
        ));
        assert!(matches!(
            map_algorithm("RSA-OAEP").expect("RSA-OAEP should be a valid algorithm"),
            EncryptionAlgorithm::RsaOaep
        ));
        assert!(matches!(
            map_algorithm("RSA_OAEP_256").expect("RSA_OAEP_256 should be a valid algorithm"),
            EncryptionAlgorithm::RsaOaep256
        ));
        // Case insensitive
        assert!(matches!(
            map_algorithm("rsa_oaep").expect("lowercase rsa_oaep should be valid"),
            EncryptionAlgorithm::RsaOaep
        ));
        assert!(map_algorithm("UNKNOWN").is_err());
    }
}
