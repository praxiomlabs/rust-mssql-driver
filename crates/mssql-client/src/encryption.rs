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
//! ```rust,no_run
//! # async fn with_always_encrypted() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "always-encrypted")]
//! # {
//! # let conn_str = "Server=localhost;Database=db;Encrypt=strict;Column Encryption Setting=Enabled";
//! use mssql_client::{Client, Config, EncryptionConfig};
//! use mssql_auth::InMemoryKeyStore;
//!
//! // Register a key-store provider, then attach it to the connection config.
//! let key_store = InMemoryKeyStore::new();
//! let encryption_config = EncryptionConfig::new().with_provider(key_store);
//!
//! let config = Config::from_connection_string(conn_str)?
//!     .with_column_encryption(encryption_config);
//!
//! let _client = Client::connect(config).await?;
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! Equivalently, set `Column Encryption Setting=Enabled` in the connection
//! string. Production-ready providers ship in `mssql-auth`: `InMemoryKeyStore`
//! (dev/test), `AzureKeyVaultProvider` (`azure-identity` feature), and
//! `WindowsCertStoreProvider` (`sspi-auth`, Windows). Implement
//! [`mssql_auth::KeyStoreProvider`] for custom key storage. Do **not** substitute
//! T-SQL `ENCRYPTBYKEY` — the server can see that plaintext, defeating the point.
//!
//! ## How decryption works
//!
//! 1. Always Encrypted support is negotiated in LOGIN7 (`FEATURE_EXT`).
//! 2. `ColMetaData` carries [`CryptoMetadata`] and the [`CekTable`]; column
//!    encryption keys are resolved asynchronously up front (calling the key-store
//!    providers).
//! 3. Each encrypted cell is decrypted during row parsing via
//!    AEAD_AES_256_CBC_HMAC_SHA256, with the HMAC verified before decryption.
//!
//! Reads are transparent across `query`, `call_procedure`, the procedure
//! builder, and multi-result queries. **Limitation:** the parameter (write) path
//! only supports `NULL`; encrypting outbound parameter values is not yet
//! implemented.
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
use mssql_types::SqlValue;
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
///
/// The context holds an `Arc<EncryptionConfig>` so providers remain accessible
/// across connection retries/redirects where the `Config` (and its inner
/// encryption config Arc) gets cloned multiple times.
#[cfg(feature = "always-encrypted")]
pub struct EncryptionContext {
    /// Shared handle on the user-supplied configuration. Providers are looked
    /// up by name through this reference, so an arbitrary number of `Arc`
    /// clones do not lose access to them.
    config: std::sync::Arc<EncryptionConfig>,
    /// Cache for decrypted CEKs.
    cek_cache: CekCache,
    /// Whether caching is enabled.
    cache_enabled: bool,
}

#[cfg(feature = "always-encrypted")]
impl EncryptionContext {
    /// Create a new encryption context from an Arc-wrapped configuration.
    ///
    /// The Arc is retained by the context so provider lookups continue to
    /// work for the lifetime of the client — regardless of how many times
    /// the outer `Config` has been cloned for retry/redirect handling.
    pub fn from_arc(config: std::sync::Arc<EncryptionConfig>) -> Self {
        let cache_enabled = config.cache_ceks;
        Self {
            config,
            cek_cache: CekCache::new(),
            cache_enabled,
        }
    }

    /// Create a new encryption context from configuration.
    pub fn new(config: EncryptionConfig) -> Self {
        Self::from_arc(std::sync::Arc::new(config))
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

        // Find the appropriate key store provider via the shared config
        let provider = self
            .config
            .get_provider(&cek_value.key_store_provider_name)
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
        self.config.get_provider(name).is_some()
    }
}

#[cfg(feature = "always-encrypted")]
impl std::fmt::Debug for EncryptionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionContext")
            .field("provider_count", &self.config.providers.len())
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

/// Normalize a parameter value to the plaintext byte form Always Encrypted
/// encrypts — SQL Server's "normalized" form for the value's type. The result
/// is the plaintext input to [`EncryptionContext::encrypt_value`].
///
/// Normalization is type-specific and is **not** the regular TDS wire encoding:
/// e.g. INT normalizes to 8 little-endian bytes (not 4), and strings/binaries
/// carry no length prefix. These layouts are validated byte-for-byte against
/// Microsoft.Data.SqlClient (see the `ae_normalization` tests). Only the types
/// supported so far are handled; others return `UnsupportedOperation`.
#[cfg(feature = "always-encrypted")]
pub fn normalize_for_encryption(value: &SqlValue) -> Result<Vec<u8>, EncryptionError> {
    match value {
        // Signed integer types normalize to 8-byte little-endian (sign-extended).
        SqlValue::Int(v) => Ok(i64::from(*v).to_le_bytes().to_vec()),
        // NVARCHAR: UTF-16LE code units, no length prefix.
        SqlValue::String(s) => Ok(s.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        // VARBINARY: the raw bytes, no length prefix.
        SqlValue::Binary(b) => Ok(b.to_vec()),
        other => Err(EncryptionError::UnsupportedOperation(format!(
            "Always Encrypted parameter encryption is not yet implemented for {}",
            other.type_name()
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Reference ciphertexts captured from a live deterministic Always Encrypted
    /// INSERT via Microsoft.Data.SqlClient 5.2.2. Encrypting our normalization
    /// with the same CEK must reproduce them byte-for-byte — proving the
    /// normalized layout matches the real .NET client (notably INT -> 8 LE bytes,
    /// which is the layout a naive implementation would get wrong).
    #[cfg(feature = "always-encrypted")]
    #[test]
    fn ae_normalization_matches_dotnet() {
        use bytes::Bytes;

        fn unhex(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }

        let cek = unhex("B59D9F2C96784C232D53AB273D257DC79B7D2355BB82B1EC7054CE25E25F7B44");
        let enc = AeadEncryptor::new(&cek).unwrap();

        for (value, reference) in [
            (
                SqlValue::Int(42),
                "01102FC5DEC5D3E463A8F4BDF512AA74E6AB953BA9A2F3F9A98CD18446B007DE5A6E2A1D1EB775035EA189CA5160A935CE093CAA9BB7E9233BB333AADEE86FDE1D",
            ),
            (
                SqlValue::String("Ada".to_string()),
                "01BFAC40E6DA541ACEFAD8ECF5598DB77B0C5349CFACBC3C9221C01B6037E593B78E8F398F620F837BD6A4A2B644125C4188DF278B94479B2218466D91107FE417",
            ),
            (
                SqlValue::Binary(Bytes::from_static(&[0x01, 0x02, 0x03])),
                "01ADE71457495F00FC9A16456F1B1EECB901D88DE97887025C189B1C4432E02071AB7594C48518CA5621E90165FAE337475B4CF3A3D00EF2D862FB0473713DF1E1",
            ),
        ] {
            let norm = normalize_for_encryption(&value).unwrap();
            let cipher = enc
                .encrypt(&norm, mssql_auth::EncryptionType::Deterministic)
                .unwrap();
            assert_eq!(
                cipher,
                unhex(reference),
                "ciphertext for {} must match Microsoft.Data.SqlClient",
                value.type_name()
            );
        }
    }

    #[cfg(feature = "always-encrypted")]
    #[test]
    fn ae_normalization_rejects_unsupported_type() {
        assert!(normalize_for_encryption(&SqlValue::Bool(true)).is_err());
    }

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
