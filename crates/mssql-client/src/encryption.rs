//! Always Encrypted client-side encryption and decryption.
//!
//! This module provides the infrastructure for SQL Server's Always Encrypted feature,
//! which enables client-side encryption of sensitive database columns.
//!
//! ## Architecture
//!
//! Always Encrypted uses a two-tier key hierarchy:
//!
//! ```text
//! Column Master Key (CMK) - External (KeyVault, CertStore, HSM)
//!         │
//!         ▼ RSA-OAEP unwrap
//! Column Encryption Key (CEK) - Stored encrypted in database
//!         │
//!         ▼ AEAD_AES_256_CBC_HMAC_SHA256
//! Encrypted Column Data
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_client::{Config, EncryptionConfig};
//! use mssql_auth::InMemoryKeyStore;
//!
//! // Create encryption configuration
//! let mut key_store = InMemoryKeyStore::new();
//! key_store.add_key("MyKey", &pem)?;
//!
//! let encryption_config = EncryptionConfig::new()
//!     .with_provider(key_store)
//!     .build();
//!
//! // Connect with encryption enabled
//! let config = Config::from_connection_string(conn_str)?
//!     .with_encryption(encryption_config);
//!
//! let client = Client::connect(config).await?;
//! ```
//!
//! ## Security Model
//!
//! - **Client-only decryption**: SQL Server never sees plaintext data
//! - **DBA protection**: Even database administrators cannot read encrypted data
//! - **Key separation**: CMK stays in secure key store, never transmitted

use std::collections::HashMap;

use mssql_auth::KeyStoreProvider;
use tds_protocol::crypto::{CekTable, CekTableEntry, CryptoMetadata, EncryptionTypeWire};

#[cfg(feature = "always-encrypted")]
use mssql_auth::{AeadEncryptor, CekCache, CekCacheKey, EncryptionError};
#[cfg(feature = "always-encrypted")]
use std::sync::Arc;

/// Configuration for Always Encrypted feature.
#[derive(Default)]
pub struct EncryptionConfig {
    /// Whether encryption is enabled.
    pub enabled: bool,
    /// Registered key store providers.
    providers: Vec<Box<dyn KeyStoreProvider>>,
    /// Whether to cache decrypted CEKs for performance.
    pub cache_ceks: bool,
}

impl EncryptionConfig {
    /// Create a new encryption configuration (disabled by default).
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: true,
            providers: Vec::new(),
            cache_ceks: true,
        }
    }

    /// Register a key store provider.
    pub fn register_provider(&mut self, provider: impl KeyStoreProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// Builder method to add a key store provider.
    #[must_use]
    pub fn with_provider(mut self, provider: impl KeyStoreProvider + 'static) -> Self {
        self.register_provider(provider);
        self
    }

    /// Enable or disable CEK caching.
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

    /// Check if encryption is ready (enabled and has providers).
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.enabled && !self.providers.is_empty()
    }
}

impl std::fmt::Debug for EncryptionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionConfig")
            .field("enabled", &self.enabled)
            .field("provider_count", &self.providers.len())
            .field("cache_ceks", &self.cache_ceks)
            .finish()
    }
}

/// Runtime context for encryption operations.
///
/// This is the active encryption state for a connected client,
/// including resolved CEKs and encryptors.
#[cfg(feature = "always-encrypted")]
pub struct EncryptionContext {
    /// Key store providers by name.
    providers: HashMap<String, Box<dyn KeyStoreProvider>>,
    /// Cache for decrypted CEKs.
    cek_cache: CekCache,
    /// Whether caching is enabled.
    cache_enabled: bool,
}

#[cfg(feature = "always-encrypted")]
impl EncryptionContext {
    /// Create a new encryption context from an Arc-wrapped configuration.
    ///
    /// This attempts to unwrap the Arc to get ownership of the config.
    /// If the Arc has been cloned (multiple references), it falls back
    /// to creating a context with no providers (connection string-only mode
    /// where providers must be registered separately).
    pub fn from_arc(config: std::sync::Arc<EncryptionConfig>) -> Self {
        match std::sync::Arc::try_unwrap(config) {
            Ok(owned) => Self::new(owned),
            Err(_arc) => {
                // Config was shared — create context without providers.
                // The caller should register providers separately.
                tracing::warn!(
                    "EncryptionConfig has multiple references; \
                     creating EncryptionContext without providers"
                );
                Self {
                    providers: std::collections::HashMap::new(),
                    cek_cache: CekCache::new(),
                    cache_enabled: true,
                }
            }
        }
    }

    /// Create a new encryption context from configuration.
    pub fn new(config: EncryptionConfig) -> Self {
        let providers = config
            .providers
            .into_iter()
            .map(|p| (p.provider_name().to_string(), p))
            .collect();

        Self {
            providers,
            cek_cache: CekCache::new(),
            cache_enabled: config.cache_ceks,
        }
    }

    /// Get or decrypt a CEK for a column.
    ///
    /// This handles the CEK caching and decryption logic:
    /// 1. Check cache for existing encryptor
    /// 2. If not cached, decrypt CEK using the appropriate key store
    /// 3. Create and cache the encryptor
    pub async fn get_encryptor(
        &self,
        cek_entry: &CekTableEntry,
    ) -> Result<Arc<AeadEncryptor>, EncryptionError> {
        let cache_key = CekCacheKey::new(
            cek_entry.database_id,
            cek_entry.cek_id,
            cek_entry.cek_version,
        );

        // Check cache first
        if self.cache_enabled {
            if let Some(encryptor) = self.cek_cache.get(&cache_key) {
                return Ok(encryptor);
            }
        }

        // Get the primary CEK value
        let cek_value = cek_entry
            .primary_value()
            .ok_or_else(|| EncryptionError::CekDecryptionFailed("No CEK value available".into()))?;

        // Find the appropriate key store provider
        let provider = self
            .providers
            .get(&cek_value.key_store_provider_name)
            .ok_or_else(|| {
                EncryptionError::KeyStoreNotFound(cek_value.key_store_provider_name.clone())
            })?;

        // Decrypt the CEK
        let decrypted_cek = provider
            .decrypt_cek(
                &cek_value.cmk_path,
                &cek_value.encryption_algorithm,
                &cek_value.encrypted_value,
            )
            .await?;

        // Create encryptor and cache it
        if self.cache_enabled {
            self.cek_cache.insert(cache_key, decrypted_cek)
        } else {
            // Create encryptor without caching
            Ok(Arc::new(AeadEncryptor::new(&decrypted_cek)?))
        }
    }

    /// Encrypt a value for a column.
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The plaintext value to encrypt
    /// * `cek_entry` - The CEK table entry for this column
    /// * `encryption_type` - Deterministic or randomized encryption
    pub async fn encrypt_value(
        &self,
        plaintext: &[u8],
        cek_entry: &CekTableEntry,
        encryption_type: EncryptionTypeWire,
    ) -> Result<Vec<u8>, EncryptionError> {
        let encryptor = self.get_encryptor(cek_entry).await?;

        let enc_type = match encryption_type {
            EncryptionTypeWire::Deterministic => mssql_auth::EncryptionType::Deterministic,
            EncryptionTypeWire::Randomized => mssql_auth::EncryptionType::Randomized,
            _ => {
                return Err(EncryptionError::UnsupportedOperation(format!(
                    "unsupported encryption type: {encryption_type:?}"
                )));
            }
        };

        encryptor.encrypt(plaintext, enc_type)
    }

    /// Decrypt a value from an encrypted column.
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The encrypted value
    /// * `cek_entry` - The CEK table entry for this column
    pub async fn decrypt_value(
        &self,
        ciphertext: &[u8],
        cek_entry: &CekTableEntry,
    ) -> Result<Vec<u8>, EncryptionError> {
        let encryptor = self.get_encryptor(cek_entry).await?;
        encryptor.decrypt(ciphertext)
    }

    /// Clear the CEK cache.
    ///
    /// Call this when keys may have been rotated.
    pub fn clear_cache(&self) {
        self.cek_cache.clear();
    }

    /// Check if a provider is registered.
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }
}

#[cfg(feature = "always-encrypted")]
impl std::fmt::Debug for EncryptionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionContext")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .field("cache_entries", &self.cek_cache.len())
            .field("cache_enabled", &self.cache_enabled)
            .finish()
    }
}

/// Column encryption metadata for a result set.
///
/// This combines the CEK table with per-column crypto metadata,
/// providing all information needed to decrypt result columns.
#[derive(Debug, Clone)]
pub struct ResultSetEncryptionInfo {
    /// CEK table for this result set.
    pub cek_table: CekTable,
    /// Crypto metadata for each column (index matches column ordinal).
    pub column_crypto: Vec<Option<CryptoMetadata>>,
}

impl ResultSetEncryptionInfo {
    /// Create encryption info for a result set.
    pub fn new(cek_table: CekTable, column_count: usize) -> Self {
        Self {
            cek_table,
            column_crypto: vec![None; column_count],
        }
    }

    /// Set crypto metadata for a column.
    pub fn set_column_crypto(&mut self, ordinal: usize, metadata: CryptoMetadata) {
        if ordinal < self.column_crypto.len() {
            self.column_crypto[ordinal] = Some(metadata);
        }
    }

    /// Get the CEK entry for a column.
    pub fn get_cek_for_column(&self, ordinal: usize) -> Option<&CekTableEntry> {
        let crypto = self.column_crypto.get(ordinal)?.as_ref()?;
        self.cek_table.get(crypto.cek_table_ordinal)
    }

    /// Check if a column is encrypted.
    pub fn is_column_encrypted(&self, ordinal: usize) -> bool {
        self.column_crypto
            .get(ordinal)
            .map(|c| c.is_some())
            .unwrap_or(false)
    }

    /// Get the encryption type for a column.
    pub fn get_encryption_type(&self, ordinal: usize) -> Option<EncryptionTypeWire> {
        self.column_crypto
            .get(ordinal)?
            .as_ref()
            .map(|c| c.encryption_type)
    }
}

/// Parameter encryption metadata for a query.
///
/// This is returned by `sp_describe_parameter_encryption` and describes
/// how each parameter should be encrypted.
#[derive(Debug, Clone)]
pub struct ParameterEncryptionInfo {
    /// CEK table for parameters.
    pub cek_table: CekTable,
    /// Mapping from parameter name to crypto metadata.
    pub parameters: HashMap<String, ParameterCryptoInfo>,
}

impl ParameterEncryptionInfo {
    /// Create empty parameter encryption info.
    pub fn new() -> Self {
        Self {
            cek_table: CekTable::new(),
            parameters: HashMap::new(),
        }
    }

    /// Add encryption info for a parameter.
    pub fn add_parameter(&mut self, name: String, info: ParameterCryptoInfo) {
        self.parameters.insert(name, info);
    }

    /// Get encryption info for a parameter.
    pub fn get_parameter(&self, name: &str) -> Option<&ParameterCryptoInfo> {
        self.parameters.get(name)
    }

    /// Check if a parameter needs encryption.
    pub fn needs_encryption(&self, name: &str) -> bool {
        self.parameters.contains_key(name)
    }
}

impl Default for ParameterEncryptionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Encryption metadata for a single parameter.
#[derive(Debug, Clone)]
pub struct ParameterCryptoInfo {
    /// Index into the CEK table.
    pub cek_ordinal: u16,
    /// Encryption type (deterministic or randomized).
    pub encryption_type: EncryptionTypeWire,
    /// Algorithm ID.
    pub algorithm_id: u8,
    /// Target column ordinal in the table (for type information).
    pub column_ordinal: u16,
    /// Target column database ID.
    pub database_id: u32,
}

impl ParameterCryptoInfo {
    /// Create new parameter crypto info.
    pub fn new(
        cek_ordinal: u16,
        encryption_type: EncryptionTypeWire,
        algorithm_id: u8,
        column_ordinal: u16,
        database_id: u32,
    ) -> Self {
        Self {
            cek_ordinal,
            encryption_type,
            algorithm_id,
            column_ordinal,
            database_id,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_config_defaults() {
        let config = EncryptionConfig::new();
        assert!(config.enabled);
        assert!(config.cache_ceks);
        assert!(!config.is_ready()); // No providers
    }

    #[test]
    fn test_result_set_encryption_info() {
        let cek_table = CekTable::new();
        let mut info = ResultSetEncryptionInfo::new(cek_table, 3);

        assert!(!info.is_column_encrypted(0));
        assert!(!info.is_column_encrypted(1));
        assert!(!info.is_column_encrypted(2));

        let metadata = CryptoMetadata {
            cek_table_ordinal: 0,
            base_user_type: 0,
            base_col_type: 0x26,
            base_type_info: tds_protocol::token::TypeInfo::default(),
            algorithm_id: 2,
            encryption_type: EncryptionTypeWire::Deterministic,
            normalization_version: 1,
        };

        info.set_column_crypto(1, metadata);
        assert!(!info.is_column_encrypted(0));
        assert!(info.is_column_encrypted(1));
        assert!(!info.is_column_encrypted(2));

        assert_eq!(
            info.get_encryption_type(1),
            Some(EncryptionTypeWire::Deterministic)
        );
    }

    #[test]
    fn test_parameter_encryption_info() {
        let mut info = ParameterEncryptionInfo::new();

        assert!(!info.needs_encryption("@p1"));

        let crypto = ParameterCryptoInfo::new(0, EncryptionTypeWire::Randomized, 2, 1, 1);
        info.add_parameter("@p1".to_string(), crypto);

        assert!(info.needs_encryption("@p1"));
        assert!(!info.needs_encryption("@p2"));

        let param = info.get_parameter("@p1").unwrap();
        assert_eq!(param.encryption_type, EncryptionTypeWire::Randomized);
    }
}
