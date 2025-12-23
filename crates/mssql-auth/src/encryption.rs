//! Always Encrypted infrastructure for SQL Server.
//!
//! This module provides the foundational types and interfaces for implementing
//! SQL Server's Always Encrypted feature, which provides client-side encryption
//! for sensitive database columns.
//!
//! ## Architecture Overview
//!
//! Always Encrypted uses a two-tier key hierarchy:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        Key Hierarchy                            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   Column Master Key (CMK)                                       │
//! │   ├── Stored externally (KeyVault, CertStore, HSM)              │
//! │   ├── Never sent to SQL Server                                  │
//! │   └── Used to encrypt/decrypt CEKs                              │
//! │            │                                                    │
//! │            ▼                                                    │
//! │   Column Encryption Key (CEK)                                   │
//! │   ├── Stored in database (encrypted by CMK)                     │
//! │   ├── Decrypted on client side                                  │
//! │   └── Used for actual data encryption (AES-256)                 │
//! │            │                                                    │
//! │            ▼                                                    │
//! │   Encrypted Column Data                                         │
//! │   ├── Deterministic: Same input → same ciphertext               │
//! │   └── Randomized: Same input → different ciphertext             │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Security Model
//!
//! - **Client-only decryption**: The SQL Server never sees plaintext data
//! - **DBA protection**: Even database administrators cannot read encrypted data
//! - **Key separation**: CMK stays in secure key store, never transmitted
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_auth::encryption::{ColumnEncryptionConfig, KeyStoreProvider};
//!
//! // Create encryption configuration
//! let config = ColumnEncryptionConfig::new()
//!     .with_key_store(azure_key_vault_provider)
//!     .build();
//!
//! // Use with connection
//! let client = Client::connect(config.with_encryption(encryption_config)).await?;
//! ```
//!
//! ## Implementation Status
//!
//! This module provides the **infrastructure and interfaces** for Always Encrypted.
//! Full implementation requires:
//!
//! - [ ] Key store provider implementations (Azure KeyVault, Windows CertStore)
//! - [ ] AES-256 encryption/decryption routines
//! - [ ] RSA-OAEP key unwrapping
//! - [ ] Metadata fetching from sys.columns
//! - [ ] Parameter encryption hooks
//! - [ ] Result decryption hooks
//!
//! Tracked as CRYPTO-001 in the project roadmap.

use std::fmt;

/// Encryption type for Always Encrypted columns.
///
/// Determines how data is encrypted and what operations are supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionType {
    /// Deterministic encryption: same plaintext → same ciphertext.
    ///
    /// Supports:
    /// - Equality comparisons (`WHERE col = @param`)
    /// - JOIN operations
    /// - GROUP BY
    /// - DISTINCT
    /// - Indexing
    ///
    /// **Security note**: Reveals data patterns; less secure than randomized.
    Deterministic,

    /// Randomized encryption: same plaintext → different ciphertext each time.
    ///
    /// Maximum security but does NOT support:
    /// - Any comparisons (equality, range, etc.)
    /// - JOIN operations on encrypted column
    /// - GROUP BY or DISTINCT
    /// - Indexing
    Randomized,
}

impl EncryptionType {
    /// Returns the algorithm identifier used in metadata.
    #[must_use]
    pub fn algorithm_name(&self) -> &'static str {
        match self {
            EncryptionType::Deterministic => "AEAD_AES_256_CBC_HMAC_SHA_256_DETERMINISTIC",
            EncryptionType::Randomized => "AEAD_AES_256_CBC_HMAC_SHA_256_RANDOMIZED",
        }
    }

    /// Parse from the numeric value stored in sys.columns.
    #[must_use]
    pub fn from_sys_columns_value(value: i32) -> Option<Self> {
        match value {
            1 => Some(EncryptionType::Deterministic),
            2 => Some(EncryptionType::Randomized),
            _ => None,
        }
    }
}

/// Metadata about a Column Encryption Key (CEK).
///
/// This metadata is retrieved from SQL Server's `sys.column_encryption_keys`
/// and related system views.
#[derive(Debug, Clone)]
pub struct CekMetadata {
    /// Database-level identifier for this CEK.
    pub database_id: u32,
    /// CEK identifier within the database.
    pub cek_id: u32,
    /// Version of the CEK (for key rotation).
    pub cek_version: u32,
    /// Metadata version (changes with any metadata update).
    pub cek_md_version: u64,
    /// The encrypted CEK value (encrypted by CMK).
    pub encrypted_value: Vec<u8>,
    /// Name of the key store provider (e.g., "AZURE_KEY_VAULT").
    pub key_store_provider_name: String,
    /// Path to the Column Master Key in the key store.
    pub cmk_path: String,
    /// Asymmetric algorithm used to encrypt the CEK (e.g., "RSA_OAEP").
    pub encryption_algorithm: String,
}

/// Encryption information for a specific database column.
#[derive(Debug, Clone)]
pub struct ColumnEncryptionInfo {
    /// The column name.
    pub column_name: String,
    /// The ordinal position (1-based).
    pub column_ordinal: u16,
    /// Whether this column is encrypted.
    pub is_encrypted: bool,
    /// The encryption type (if encrypted).
    pub encryption_type: Option<EncryptionType>,
    /// The encryption algorithm name.
    pub encryption_algorithm: Option<String>,
    /// CEK metadata (if encrypted).
    pub cek_metadata: Option<CekMetadata>,
}

impl ColumnEncryptionInfo {
    /// Create info for a non-encrypted column.
    #[must_use]
    pub fn unencrypted(column_name: impl Into<String>, column_ordinal: u16) -> Self {
        Self {
            column_name: column_name.into(),
            column_ordinal,
            is_encrypted: false,
            encryption_type: None,
            encryption_algorithm: None,
            cek_metadata: None,
        }
    }

    /// Create info for an encrypted column.
    #[must_use]
    pub fn encrypted(
        column_name: impl Into<String>,
        column_ordinal: u16,
        encryption_type: EncryptionType,
        cek_metadata: CekMetadata,
    ) -> Self {
        Self {
            column_name: column_name.into(),
            column_ordinal,
            is_encrypted: true,
            encryption_type: Some(encryption_type),
            encryption_algorithm: Some(encryption_type.algorithm_name().to_string()),
            cek_metadata: Some(cek_metadata),
        }
    }
}

/// Error types for Always Encrypted operations.
#[derive(Debug)]
pub enum EncryptionError {
    /// The requested key store provider is not registered.
    KeyStoreNotFound(String),
    /// Failed to retrieve or unwrap the Column Master Key.
    CmkError(String),
    /// Failed to decrypt the Column Encryption Key.
    CekDecryptionFailed(String),
    /// Failed to encrypt data.
    EncryptionFailed(String),
    /// Failed to decrypt data.
    DecryptionFailed(String),
    /// The column's encryption metadata is not available.
    MetadataNotAvailable(String),
    /// The requested operation is not supported with this encryption type.
    UnsupportedOperation(String),
    /// Configuration error.
    ConfigurationError(String),
}

impl std::error::Error for EncryptionError {}

impl fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncryptionError::KeyStoreNotFound(name) => {
                write!(f, "Key store provider not found: {}", name)
            }
            EncryptionError::CmkError(msg) => {
                write!(f, "Column Master Key error: {}", msg)
            }
            EncryptionError::CekDecryptionFailed(msg) => {
                write!(f, "Failed to decrypt Column Encryption Key: {}", msg)
            }
            EncryptionError::EncryptionFailed(msg) => {
                write!(f, "Encryption failed: {}", msg)
            }
            EncryptionError::DecryptionFailed(msg) => {
                write!(f, "Decryption failed: {}", msg)
            }
            EncryptionError::MetadataNotAvailable(msg) => {
                write!(f, "Encryption metadata not available: {}", msg)
            }
            EncryptionError::UnsupportedOperation(msg) => {
                write!(f, "Unsupported operation with encryption: {}", msg)
            }
            EncryptionError::ConfigurationError(msg) => {
                write!(f, "Encryption configuration error: {}", msg)
            }
        }
    }
}

/// Trait for Column Master Key (CMK) providers.
///
/// Implementations of this trait provide access to CMKs stored in various
/// key stores (Azure Key Vault, Windows Certificate Store, HSMs, etc.).
///
/// # Security
///
/// Implementations must ensure:
/// - Keys are never logged or exposed in error messages
/// - Keys are zeroized from memory when no longer needed
/// - Access is authenticated and authorized appropriately
///
/// # Example
///
/// ```rust,ignore
/// use mssql_auth::encryption::{KeyStoreProvider, EncryptionError};
///
/// struct AzureKeyVaultProvider {
///     vault_url: String,
///     credential: azure_identity::DefaultAzureCredential,
/// }
///
/// #[async_trait::async_trait]
/// impl KeyStoreProvider for AzureKeyVaultProvider {
///     fn provider_name(&self) -> &str {
///         "AZURE_KEY_VAULT"
///     }
///
///     async fn decrypt_cek(
///         &self,
///         cmk_path: &str,
///         algorithm: &str,
///         encrypted_cek: &[u8],
///     ) -> Result<Vec<u8>, EncryptionError> {
///         // Use Azure Key Vault to unwrap the CEK
///         // ...
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait KeyStoreProvider: Send + Sync {
    /// Returns the provider name as used in SQL Server metadata.
    ///
    /// Common values:
    /// - `"AZURE_KEY_VAULT"` - Azure Key Vault
    /// - `"MSSQL_CERTIFICATE_STORE"` - Windows Certificate Store
    /// - `"MSSQL_CNG_STORE"` - Windows CNG Store
    /// - `"MSSQL_CSP_PROVIDER"` - Windows CSP Provider
    fn provider_name(&self) -> &str;

    /// Decrypt a Column Encryption Key (CEK) using the Column Master Key (CMK).
    ///
    /// # Arguments
    ///
    /// * `cmk_path` - Path to the CMK in the key store
    /// * `algorithm` - The asymmetric algorithm (e.g., "RSA_OAEP")
    /// * `encrypted_cek` - The encrypted CEK bytes
    ///
    /// # Returns
    ///
    /// The decrypted CEK bytes, which can then be used for data encryption/decryption.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be found or decryption fails.
    async fn decrypt_cek(
        &self,
        cmk_path: &str,
        algorithm: &str,
        encrypted_cek: &[u8],
    ) -> Result<Vec<u8>, EncryptionError>;

    /// Sign data using the Column Master Key (optional).
    ///
    /// This is used for key attestation in Secure Enclaves.
    /// Default implementation returns an error indicating it's not supported.
    async fn sign_data(&self, _cmk_path: &str, _data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        Err(EncryptionError::UnsupportedOperation(
            "Signing not supported by this key store provider".into(),
        ))
    }

    /// Verify a signature (optional).
    ///
    /// This is used for key attestation in Secure Enclaves.
    /// Default implementation returns an error indicating it's not supported.
    async fn verify_signature(
        &self,
        _cmk_path: &str,
        _data: &[u8],
        _signature: &[u8],
    ) -> Result<bool, EncryptionError> {
        Err(EncryptionError::UnsupportedOperation(
            "Signature verification not supported by this key store provider".into(),
        ))
    }
}

/// Configuration for Always Encrypted.
#[derive(Default)]
pub struct ColumnEncryptionConfig {
    /// Whether column encryption is enabled.
    pub enabled: bool,
    /// Registered key store providers.
    providers: Vec<Box<dyn KeyStoreProvider>>,
    /// Cache decrypted CEKs (performance optimization).
    pub cache_ceks: bool,
    /// Allow unsafe operations (e.g., queries on encrypted columns without parameterization).
    pub allow_unsafe_operations: bool,
}

impl ColumnEncryptionConfig {
    /// Create a new configuration with encryption enabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: true,
            providers: Vec::new(),
            cache_ceks: true,
            allow_unsafe_operations: false,
        }
    }

    /// Register a key store provider.
    ///
    /// Multiple providers can be registered to support different key stores.
    pub fn register_provider(&mut self, provider: impl KeyStoreProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// Builder method to add a key store provider.
    #[must_use]
    pub fn with_provider(mut self, provider: impl KeyStoreProvider + 'static) -> Self {
        self.register_provider(provider);
        self
    }

    /// Builder method to control CEK caching.
    #[must_use]
    pub fn with_cek_caching(mut self, enabled: bool) -> Self {
        self.cache_ceks = enabled;
        self
    }

    /// Get a provider by name.
    pub fn get_provider(&self, name: &str) -> Option<&dyn KeyStoreProvider> {
        self.providers
            .iter()
            .find(|p| p.provider_name() == name)
            .map(|p| p.as_ref())
    }

    /// Check if encryption is enabled and providers are available.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.enabled && !self.providers.is_empty()
    }
}

impl fmt::Debug for ColumnEncryptionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColumnEncryptionConfig")
            .field("enabled", &self.enabled)
            .field(
                "providers",
                &self
                    .providers
                    .iter()
                    .map(|p| p.provider_name())
                    .collect::<Vec<_>>(),
            )
            .field("cache_ceks", &self.cache_ceks)
            .field("allow_unsafe_operations", &self.allow_unsafe_operations)
            .finish()
    }
}

/// Represents an encrypted value with its metadata.
///
/// This is used internally to track encrypted parameter values.
#[derive(Debug, Clone)]
pub struct EncryptedValue {
    /// The ciphertext bytes.
    pub ciphertext: Vec<u8>,
    /// The CEK ID used for encryption.
    pub cek_id: u32,
    /// The encryption type.
    pub encryption_type: EncryptionType,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_type_algorithm_names() {
        assert_eq!(
            EncryptionType::Deterministic.algorithm_name(),
            "AEAD_AES_256_CBC_HMAC_SHA_256_DETERMINISTIC"
        );
        assert_eq!(
            EncryptionType::Randomized.algorithm_name(),
            "AEAD_AES_256_CBC_HMAC_SHA_256_RANDOMIZED"
        );
    }

    #[test]
    fn test_encryption_type_from_sys_columns() {
        assert_eq!(
            EncryptionType::from_sys_columns_value(1),
            Some(EncryptionType::Deterministic)
        );
        assert_eq!(
            EncryptionType::from_sys_columns_value(2),
            Some(EncryptionType::Randomized)
        );
        assert_eq!(EncryptionType::from_sys_columns_value(0), None);
        assert_eq!(EncryptionType::from_sys_columns_value(99), None);
    }

    #[test]
    fn test_column_encryption_info_unencrypted() {
        let info = ColumnEncryptionInfo::unencrypted("name", 1);
        assert!(!info.is_encrypted);
        assert!(info.encryption_type.is_none());
        assert!(info.cek_metadata.is_none());
    }

    #[test]
    fn test_column_encryption_config_debug() {
        let config = ColumnEncryptionConfig::new();
        let debug = format!("{:?}", config);
        assert!(debug.contains("ColumnEncryptionConfig"));
        assert!(debug.contains("enabled: true"));
    }

    #[test]
    fn test_encryption_error_display() {
        let error = EncryptionError::KeyStoreNotFound("AZURE_KEY_VAULT".into());
        assert!(error.to_string().contains("AZURE_KEY_VAULT"));

        let error = EncryptionError::EncryptionFailed("test error".into());
        assert!(error.to_string().contains("test error"));
    }
}
