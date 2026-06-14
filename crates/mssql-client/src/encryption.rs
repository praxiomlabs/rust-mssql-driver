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
//! builder, and multi-result queries. Parameter (write) encryption is wired
//! into parameterized `query`/`execute` for the common scalar types — `int`,
//! `tinyint`, `smallint`, `bigint`, `bit`, `real`, `float`, `nvarchar`,
//! `varbinary`, `uniqueidentifier`, `date`, `money`, `smallmoney`, `decimal`
//! (via `numeric(value, precision, scale)`), and typed `NULL` (via
//! `null::<T>()`): with `Column Encryption Setting=Enabled` the
//! driver describes the parameters (`sp_describe_parameter_encryption`),
//! encrypts those bound to encrypted columns client-side, and sends them as
//! encrypted RPC parameters (deterministic and randomized). The remaining
//! temporal types (`datetime`, `datetime2`, `time`, `datetimeoffset`) and the
//! fixed-width `char`/`nchar`/`binary` types are not yet supported and return an
//! error rather than sending plaintext.
//!
//! Bind a `decimal` parameter with `numeric(value, precision, scale)`, not a
//! plain `Decimal`: an encrypted `decimal` column requires the declared
//! precision and scale to match the column exactly, and a plain `Decimal` does
//! not carry a precision — so it is rejected by the server with `Operand type
//! clash` (Msg 206) at the describe step.
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

#[cfg(feature = "always-encrypted")]
use crate::{Error, row::Row, stream::ResultSet};
#[cfg(feature = "always-encrypted")]
use tds_protocol::crypto::CekValue;

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

/// Encryption directive for a single parameter, parsed from result set 2 of
/// `sp_describe_parameter_encryption`.
#[derive(Debug, Clone)]
pub struct ParameterCryptoInfo {
    /// 0-based index into [`ParameterEncryptionInfo::cek_table`].
    ///
    /// The server reports a (often 1-based) key ordinal; the parser translates
    /// it to this positional index so `cek_table.get(cek_ordinal)` resolves the
    /// entry directly.
    pub cek_ordinal: u16,
    /// Encryption type (deterministic or randomized).
    pub encryption_type: EncryptionTypeWire,
    /// Encryption algorithm ID (2 = AEAD_AES_256_CBC_HMAC_SHA256).
    pub algorithm_id: u8,
    /// Normalization rule version applied to the plaintext before encryption.
    pub normalization_rule_version: u8,
}

impl ParameterCryptoInfo {
    /// Create new parameter crypto info.
    pub fn new(
        cek_ordinal: u16,
        encryption_type: EncryptionTypeWire,
        algorithm_id: u8,
        normalization_rule_version: u8,
    ) -> Self {
        Self {
            cek_ordinal,
            encryption_type,
            algorithm_id,
            normalization_rule_version,
        }
    }
}

/// Parsing of the two result sets returned by `sp_describe_parameter_encryption`.
///
/// Result set 1 is the CEK table (one row per CMK-wrapping of each CEK); result
/// set 2 lists, per parameter, how the server expects it encrypted. The column
/// layout was captured against a live server (SQL Server 2016+): the first nine
/// RS1 columns are stable across versions; SQL Server 2019+ append two enclave
/// columns (`is_requested_by_enclave`, `column_master_key_signature`) which this
/// non-enclave path ignores. Columns are read positionally to match the
/// `Microsoft.Data.SqlClient` ordinals.
#[cfg(feature = "always-encrypted")]
impl ParameterEncryptionInfo {
    /// Minimum RS1 column count (SQL Server 2017 returns exactly this; 2019+
    /// return more, with the extra columns appended after these).
    const RS1_MIN_COLS: usize = 9;
    /// RS2 column count, stable across supported versions.
    const RS2_MIN_COLS: usize = 6;

    /// Parse `sp_describe_parameter_encryption` output into encryption metadata.
    ///
    /// `result_sets` must be the `ProcedureResult::result_sets` from that RPC.
    /// Plaintext parameters (encryption type 0) are omitted from the result.
    pub(crate) fn from_describe_result_sets(result_sets: &mut [ResultSet]) -> Result<Self, Error> {
        if result_sets.len() < 2 {
            return Err(Error::Protocol(format!(
                "sp_describe_parameter_encryption returned {} result set(s), expected 2",
                result_sets.len()
            )));
        }

        // --- Result set 1: CEK table ---
        let rs1_cols = result_sets[0].columns().len();
        if rs1_cols < Self::RS1_MIN_COLS {
            return Err(Error::Protocol(format!(
                "sp_describe_parameter_encryption result set 1 has {rs1_cols} columns, expected >= {}",
                Self::RS1_MIN_COLS
            )));
        }
        let rs1_rows = result_sets[0].collect_all()?;

        let mut entries: Vec<CekTableEntry> = Vec::new();
        // Server-assigned key ordinal -> positional index into `entries`.
        let mut ordinal_to_index: HashMap<i32, u16> = HashMap::new();

        for row in &rs1_rows {
            let key_ordinal = describe_int(row, 0, "column_encryption_key_ordinal")?;
            let value = CekValue {
                encrypted_value: describe_varbinary(
                    row,
                    5,
                    "column_encryption_key_encrypted_value",
                )?,
                key_store_provider_name: describe_nvarchar(
                    row,
                    6,
                    "column_master_key_store_provider_name",
                )?,
                cmk_path: describe_nvarchar(row, 7, "column_master_key_path")?,
                encryption_algorithm: describe_nvarchar(
                    row,
                    8,
                    "column_encryption_key_encryption_algorithm_name",
                )?,
            };

            if let Some(&idx) = ordinal_to_index.get(&key_ordinal) {
                // Another CMK-wrapping of an already-seen CEK (key rotation).
                entries[idx as usize].values.push(value);
            } else {
                let idx = u16::try_from(entries.len()).map_err(|_| {
                    Error::Protocol(
                        "sp_describe_parameter_encryption returned too many CEKs".into(),
                    )
                })?;
                ordinal_to_index.insert(key_ordinal, idx);
                entries.push(CekTableEntry {
                    database_id: describe_int(row, 1, "database_id")? as u32,
                    cek_id: describe_int(row, 2, "column_encryption_key_id")? as u32,
                    cek_version: describe_int(row, 3, "column_encryption_key_version")? as u32,
                    cek_md_version: describe_md_version(row, 4)?,
                    values: vec![value],
                });
            }
        }
        let cek_table = CekTable { entries };

        // --- Result set 2: per-parameter directives ---
        let rs2_cols = result_sets[1].columns().len();
        if rs2_cols < Self::RS2_MIN_COLS {
            return Err(Error::Protocol(format!(
                "sp_describe_parameter_encryption result set 2 has {rs2_cols} columns, expected >= {}",
                Self::RS2_MIN_COLS
            )));
        }
        let rs2_rows = result_sets[1].collect_all()?;

        let mut parameters = HashMap::new();
        for row in &rs2_rows {
            let name = describe_nvarchar(row, 1, "parameter_name")?;
            let encryption_type_byte = describe_tinyint(row, 3, "column_encryption_type")?;
            // 0 = the server determined this parameter needs no encryption.
            if encryption_type_byte == 0 {
                continue;
            }
            let encryption_type =
                EncryptionTypeWire::from_u8(encryption_type_byte).ok_or_else(|| {
                    Error::Protocol(format!(
                        "sp_describe_parameter_encryption: invalid column_encryption_type {encryption_type_byte} for {name}"
                    ))
                })?;
            let algorithm_id = describe_tinyint(row, 2, "column_encryption_algorithm")?;
            let server_ordinal = describe_int(row, 4, "column_encryption_key_ordinal")?;
            let normalization_rule_version =
                describe_tinyint(row, 5, "column_encryption_normalization_rule_version")?;

            let cek_ordinal = *ordinal_to_index.get(&server_ordinal).ok_or_else(|| {
                Error::Protocol(format!(
                    "sp_describe_parameter_encryption: parameter {name} references CEK ordinal {server_ordinal} absent from the CEK table"
                ))
            })?;

            parameters.insert(
                name,
                ParameterCryptoInfo {
                    cek_ordinal,
                    encryption_type,
                    algorithm_id,
                    normalization_rule_version,
                },
            );
        }

        Ok(Self {
            cek_table,
            parameters,
        })
    }
}

/// Read an `int` describe column, erroring if it is absent or a different type.
#[cfg(feature = "always-encrypted")]
fn describe_int(row: &Row, idx: usize, col: &str) -> Result<i32, Error> {
    match row.get_raw(idx) {
        Some(SqlValue::Int(v)) => Ok(v),
        other => Err(describe_type_error(col, idx, "int", other.as_ref())),
    }
}

/// Read a `tinyint` describe column.
#[cfg(feature = "always-encrypted")]
fn describe_tinyint(row: &Row, idx: usize, col: &str) -> Result<u8, Error> {
    match row.get_raw(idx) {
        Some(SqlValue::TinyInt(v)) => Ok(v),
        other => Err(describe_type_error(col, idx, "tinyint", other.as_ref())),
    }
}

/// Read an `nvarchar` describe column.
#[cfg(feature = "always-encrypted")]
fn describe_nvarchar(row: &Row, idx: usize, col: &str) -> Result<String, Error> {
    match row.get_raw(idx) {
        Some(SqlValue::String(v)) => Ok(v),
        other => Err(describe_type_error(col, idx, "nvarchar", other.as_ref())),
    }
}

/// Read a `varbinary` describe column.
#[cfg(feature = "always-encrypted")]
fn describe_varbinary(row: &Row, idx: usize, col: &str) -> Result<bytes::Bytes, Error> {
    match row.get_raw(idx) {
        Some(SqlValue::Binary(v)) => Ok(v),
        other => Err(describe_type_error(col, idx, "varbinary", other.as_ref())),
    }
}

/// Read the `binary(8)` metadata-version column as a little-endian `u64`.
#[cfg(feature = "always-encrypted")]
fn describe_md_version(row: &Row, idx: usize) -> Result<u64, Error> {
    match row.get_raw(idx) {
        Some(SqlValue::Binary(b)) if b.len() == 8 => {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&b[..8]);
            Ok(u64::from_le_bytes(bytes))
        }
        other => Err(describe_type_error(
            "column_encryption_key_metadata_version",
            idx,
            "binary(8)",
            other.as_ref(),
        )),
    }
}

/// Build a uniform error for an unexpected describe-column type.
#[cfg(feature = "always-encrypted")]
fn describe_type_error(col: &str, idx: usize, expected: &str, got: Option<&SqlValue>) -> Error {
    let got = got.map_or("missing", SqlValue::type_name);
    Error::Protocol(format!(
        "sp_describe_parameter_encryption column {col} (#{idx}): expected {expected}, got {got}"
    ))
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
        // All integer types AND bit normalize to 8-byte little-endian (the value
        // widened to i64). Validated against .NET: tinyint/smallint are 8 bytes,
        // not their native 1/2 — a spec-reading would get this wrong.
        SqlValue::Bool(v) => Ok(i64::from(*v).to_le_bytes().to_vec()),
        SqlValue::TinyInt(v) => Ok(i64::from(*v).to_le_bytes().to_vec()),
        SqlValue::SmallInt(v) => Ok(i64::from(*v).to_le_bytes().to_vec()),
        SqlValue::Int(v) => Ok(i64::from(*v).to_le_bytes().to_vec()),
        SqlValue::BigInt(v) => Ok(v.to_le_bytes().to_vec()),
        // REAL/FLOAT: the IEEE-754 bits, little-endian (4 and 8 bytes).
        SqlValue::Float(v) => Ok(v.to_le_bytes().to_vec()),
        SqlValue::Double(v) => Ok(v.to_le_bytes().to_vec()),
        // NVARCHAR: UTF-16LE code units, no length prefix.
        SqlValue::String(s) => Ok(s.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        // VARBINARY: the raw bytes, no length prefix.
        SqlValue::Binary(b) => Ok(b.to_vec()),
        // UNIQUEIDENTIFIER: SQL Server's 16-byte mixed-endian GUID order (first
        // three groups byte-reversed from the RFC layout, last 8 as-is).
        #[cfg(feature = "uuid")]
        SqlValue::Uuid(u) => {
            let b = u.as_bytes();
            Ok(vec![
                b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12],
                b[13], b[14], b[15],
            ])
        }
        // DATE: 3-byte little-endian count of days since 0001-01-01.
        // `num_days_from_ce` counts from day 1, so subtract 1.
        #[cfg(feature = "chrono")]
        SqlValue::Date(d) => {
            use chrono::Datelike;
            let days = (d.num_days_from_ce() - 1) as u32;
            Ok(days.to_le_bytes()[..3].to_vec())
        }
        // DECIMAL/NUMERIC: 1 sign byte (0 negative, 1 positive) + 16-byte
        // little-endian unscaled magnitude. Uses the value's own scale.
        #[cfg(feature = "decimal")]
        SqlValue::Decimal(d) => {
            let mut out = Vec::with_capacity(17);
            out.push(u8::from(!d.is_sign_negative()));
            out.extend_from_slice(&d.mantissa().unsigned_abs().to_le_bytes());
            Ok(out)
        }
        // MONEY and SMALLMONEY both normalize to the 8-byte MONEY form: the
        // value scaled by 10_000 as an i64, high 32 bits then low 32 bits.
        #[cfg(feature = "decimal")]
        SqlValue::Money(d) | SqlValue::SmallMoney(d) => {
            let cents = money_cents(d)?;
            let mut out = ((cents >> 32) as i32).to_le_bytes().to_vec();
            out.extend_from_slice(&(cents as u32).to_le_bytes());
            Ok(out)
        }
        other => Err(EncryptionError::UnsupportedOperation(format!(
            "Always Encrypted parameter encryption is not yet implemented for {}",
            other.type_name()
        ))),
    }
}

/// The MONEY fixed-point value (`value * 10_000`) as an `i64`, rounding excess
/// precision toward zero. Used by both MONEY and SMALLMONEY normalization.
#[cfg(all(feature = "always-encrypted", feature = "decimal"))]
fn money_cents(value: &rust_decimal::Decimal) -> Result<i64, EncryptionError> {
    let mantissa = value.mantissa();
    let scale = value.scale();
    let cents: i128 = if scale <= 4 {
        mantissa
            .checked_mul(10_i128.pow(4 - scale))
            .ok_or_else(|| {
                EncryptionError::UnsupportedOperation("MONEY value out of range".into())
            })?
    } else {
        mantissa / 10_i128.pow(scale - 4)
    };
    i64::try_from(cents)
        .map_err(|_| EncryptionError::UnsupportedOperation("MONEY value out of range".into()))
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

    /// `normalize_for_encryption` rejects values it has no normalization for
    /// rather than silently producing wrong bytes. NULL is never normalized
    /// (it is handled as a NULL parameter upstream), so it exercises the
    /// catch-all rejection arm and stays unsupported as more types are added.
    #[cfg(feature = "always-encrypted")]
    #[test]
    fn ae_normalization_rejects_unnormalizable_value() {
        assert!(normalize_for_encryption(&SqlValue::Null).is_err());
    }

    /// Numeric-scalar normalization, validated byte-for-byte against
    /// Microsoft.Data.SqlClient (same method as [`ae_normalization_matches_dotnet`],
    /// captured with a fresh CEK). This is the interop guarantee: a value the
    /// driver encrypts is the value .NET would encrypt. Notable: every integer
    /// width and bit normalize to 8 bytes, real to 4, float to 8.
    #[cfg(feature = "always-encrypted")]
    #[test]
    fn ae_normalization_matches_dotnet_numeric() {
        fn unhex(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }

        let cek = unhex("9590E42A8A6C8F13B5D09B8D5A128EF8B3A4A10301C7AF24AFC62ED0E02342F7");
        let enc = AeadEncryptor::new(&cek).unwrap();

        for (value, reference) in [
            (
                SqlValue::BigInt(0x0102030405060708),
                "01E765FC4696660028BFD48FCAEAED81E0EB423CFF433CA97F1B2FF02F70744E7265C2AE73CAA562FFA98AF98CB1D3EF6A4649B3640359E1DB7D170C80E639DA68",
            ),
            (
                SqlValue::SmallInt(258),
                "012545AB817E1AEBDCEE1C00AEBFF3A013CAD20E0377BEFDD9186C263F8D1A909C313A753996F1B5E4A4AE17E901F6F781DCA707544766995D339601CA414063A0",
            ),
            (
                SqlValue::TinyInt(200),
                "01A97C33480277D16FFAEDA9068173D4173378542F2887EBCD31CDEEEB116BD59D48F9D459BDDCABAE469E891B4F82AA3D283440CA1B5E9FFC150F9D0AE54EC21E",
            ),
            (
                SqlValue::Bool(true),
                "01DDE18564051D630EE026331BCCAFC8F4122CC3919F81459F37D9C0E0C64A5317FCA08660FE5FC855917B97B72013F25B85ADD14ADDD7D5ED022EB1297FF29A7E",
            ),
            (
                SqlValue::Float(3.5),
                "017A452760E7BA7AA6A716F6707F55D9C3A81683C04A6B561B13AC1D8A848E93E239BB922EE3EE628B6D0081A590BB11747CC25D216240FB10171A0FA3B99A2DB3",
            ),
            (
                SqlValue::Double(3.5),
                "0171611557351FBC4561EBF0B9C98E0DC38AD2BD3E2C1D1E82F185D7E67D0425E506D11DD67BA3EB38F34FB01A8FCEF7E4B9A7256944334A521526613CFF6C8C5F",
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

    /// UUID and DATE normalization, validated byte-for-byte against
    /// Microsoft.Data.SqlClient: uuid uses SQL Server's mixed-endian GUID byte
    /// order, date is a 3-byte little-endian day count since 0001-01-01.
    #[cfg(all(feature = "always-encrypted", feature = "uuid", feature = "chrono"))]
    #[test]
    fn ae_normalization_matches_dotnet_uuid_date() {
        fn unhex(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }

        let cek = unhex("9590E42A8A6C8F13B5D09B8D5A128EF8B3A4A10301C7AF24AFC62ED0E02342F7");
        let enc = AeadEncryptor::new(&cek).unwrap();

        for (value, reference) in [
            (
                SqlValue::Uuid(
                    uuid::Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap(),
                ),
                "01F58635AA18692D68BDF551ECDD7AC3A56682D3F91F111F8D8F36D5425C405A8F6AB3ED3C3666444478476BD65FF40DC83F6831F502826AFEEC3116F71A7A2020CCD254F4BA28FCDC0F96BA2E5264AE9E",
            ),
            (
                SqlValue::Date(chrono::NaiveDate::from_ymd_opt(2024, 3, 15).unwrap()),
                "0188B4F75A1F4BDA53C9CDDC1918C09CB57F68E13F5560F1F1D7168FE70707337B1156A97915B244F3C03D3E7352882A599511BD243471FD03683F371CF44E4B76",
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

    /// DECIMAL and MONEY/SMALLMONEY normalization, validated byte-for-byte
    /// against Microsoft.Data.SqlClient: decimal is a sign byte plus a 16-byte
    /// little-endian unscaled magnitude; money and smallmoney both use the
    /// 8-byte MONEY form (value × 10_000, high then low 32 bits).
    #[cfg(all(feature = "always-encrypted", feature = "decimal"))]
    #[test]
    fn ae_normalization_matches_dotnet_decimal_money() {
        fn unhex(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }

        let cek = unhex("CBFB5AE21FB517C65DA0C6E8E11969C630798E473EF5827A70398012DF1D4B9E");
        let enc = AeadEncryptor::new(&cek).unwrap();
        let dec = rust_decimal::Decimal::new(123_456_789, 4); // 12345.6789
        let money = rust_decimal::Decimal::new(123_400, 4); // 12.3400

        for (value, reference) in [
            (
                SqlValue::Decimal(dec),
                "018FAE46024B9B406C23600E6A9C694F9A9B39B785A995689EBE19437BA7E75768011A035A5B54B5E495512EBB46AE1146130940A0D0D834D61AA89B5AD9F71FFAF6EEEAE77E4856BA2AA5E016E2950A8D",
            ),
            (
                SqlValue::Money(money),
                "01B4CE4CAD8D6B241A1555C377A0ADD4C79424DD5162F710D116594F725C1BAB015169A0C7716076EEC90E013519B961DEF427BFC32462D9E45D166C791B73F793",
            ),
            (
                SqlValue::SmallMoney(money),
                "01B4CE4CAD8D6B241A1555C377A0ADD4C79424DD5162F710D116594F725C1BAB015169A0C7716076EEC90E013519B961DEF427BFC32462D9E45D166C791B73F793",
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

        let crypto = ParameterCryptoInfo::new(0, EncryptionTypeWire::Randomized, 2, 1);
        info.add_parameter("@p1".to_string(), crypto);

        assert!(info.needs_encryption("@p1"));
        assert!(!info.needs_encryption("@p2"));

        let param = info.get_parameter("@p1").unwrap();
        assert_eq!(param.encryption_type, EncryptionTypeWire::Randomized);
    }

    /// Parse synthetic `sp_describe_parameter_encryption` result sets that mirror
    /// the live wire shape (captured in `.tmp/ae-3a2-describe-schema.md`). The
    /// column *order* is validated separately by the live test; this exercises
    /// the logic the live single-CEK/single-CMK case cannot: grouping multiple
    /// CMK-wrappings under one CEK, translating the server's (1-based) key
    /// ordinal to a positional index, little-endian `binary(8)` md-version
    /// decode, and skipping plaintext parameters.
    #[cfg(feature = "always-encrypted")]
    #[test]
    fn parse_describe_result_sets_groups_ceks_and_skips_plaintext() {
        use crate::row::{Column, Row};
        use crate::stream::ResultSet;
        use bytes::Bytes;

        fn rs(n_cols: usize, rows: Vec<Vec<SqlValue>>) -> ResultSet {
            let cols: Vec<Column> = (0..n_cols)
                .map(|i| Column::new(format!("c{i}"), i, "x"))
                .collect();
            let rows = rows
                .into_iter()
                .map(|vals| Row::from_values(cols.clone(), vals))
                .collect();
            ResultSet::new(cols, rows)
        }

        let mdv1 = Bytes::from_static(&[1, 0, 0, 0, 0, 0, 0, 0]); // -> 1
        let mdv2 = Bytes::from_static(&[255, 0, 0, 0, 0, 0, 0, 0]); // -> 255

        // RS1: CEK ordinal 1 wrapped by two CMKs (rotation), plus CEK ordinal 2.
        let rs1 = rs(
            9,
            vec![
                vec![
                    SqlValue::Int(1),
                    SqlValue::Int(7),
                    SqlValue::Int(56),
                    SqlValue::Int(1),
                    SqlValue::Binary(mdv1.clone()),
                    SqlValue::Binary(Bytes::from_static(b"env-a")),
                    SqlValue::String("IN_MEMORY_KEY_STORE".into()),
                    SqlValue::String("path-a".into()),
                    SqlValue::String("RSA_OAEP".into()),
                ],
                vec![
                    SqlValue::Int(1),
                    SqlValue::Int(7),
                    SqlValue::Int(56),
                    SqlValue::Int(1),
                    SqlValue::Binary(mdv1),
                    SqlValue::Binary(Bytes::from_static(b"env-a2")),
                    SqlValue::String("PROV_2".into()),
                    SqlValue::String("path-a2".into()),
                    SqlValue::String("RSA_OAEP".into()),
                ],
                vec![
                    SqlValue::Int(2),
                    SqlValue::Int(7),
                    SqlValue::Int(57),
                    SqlValue::Int(1),
                    SqlValue::Binary(mdv2),
                    SqlValue::Binary(Bytes::from_static(b"env-b")),
                    SqlValue::String("IN_MEMORY_KEY_STORE".into()),
                    SqlValue::String("path-b".into()),
                    SqlValue::String("RSA_OAEP".into()),
                ],
            ],
        );

        // RS2: @det on CEK ordinal 1, @rand on CEK ordinal 2, @plain plaintext.
        let rs2 = rs(
            6,
            vec![
                vec![
                    SqlValue::Int(1),
                    SqlValue::String("@det".into()),
                    SqlValue::TinyInt(2),
                    SqlValue::TinyInt(1),
                    SqlValue::Int(1),
                    SqlValue::TinyInt(1),
                ],
                vec![
                    SqlValue::Int(2),
                    SqlValue::String("@rand".into()),
                    SqlValue::TinyInt(2),
                    SqlValue::TinyInt(2),
                    SqlValue::Int(2),
                    SqlValue::TinyInt(1),
                ],
                vec![
                    SqlValue::Int(3),
                    SqlValue::String("@plain".into()),
                    SqlValue::TinyInt(0),
                    SqlValue::TinyInt(0),
                    SqlValue::Int(0),
                    SqlValue::TinyInt(0),
                ],
            ],
        );

        let mut sets = vec![rs1, rs2];
        let info = ParameterEncryptionInfo::from_describe_result_sets(&mut sets).unwrap();

        assert_eq!(info.cek_table.len(), 2);
        let e0 = info.cek_table.get(0).unwrap();
        assert_eq!(e0.cek_id, 56);
        assert_eq!(e0.cek_md_version, 1);
        assert_eq!(e0.values.len(), 2, "two CMK-wrappings group under one CEK");
        assert_eq!(e0.values[0].key_store_provider_name, "IN_MEMORY_KEY_STORE");
        assert_eq!(e0.values[1].key_store_provider_name, "PROV_2");
        let e1 = info.cek_table.get(1).unwrap();
        assert_eq!(e1.cek_id, 57);
        assert_eq!(e1.cek_md_version, 255);

        let det = info.get_parameter("@det").unwrap();
        assert_eq!(det.encryption_type, EncryptionTypeWire::Deterministic);
        assert_eq!(det.algorithm_id, 2);
        assert_eq!(det.normalization_rule_version, 1);
        assert_eq!(det.cek_ordinal, 0, "server ordinal 1 -> positional index 0");

        let rand = info.get_parameter("@rand").unwrap();
        assert_eq!(rand.encryption_type, EncryptionTypeWire::Randomized);
        assert_eq!(
            rand.cek_ordinal, 1,
            "server ordinal 2 -> positional index 1"
        );

        assert!(!info.needs_encryption("@plain"));
        assert_eq!(info.parameters.len(), 2);
    }

    /// A truncated response (fewer than two result sets) must be rejected, not
    /// silently treated as "no parameters need encryption".
    #[cfg(feature = "always-encrypted")]
    #[test]
    fn parse_describe_result_sets_rejects_missing_result_set() {
        use crate::row::{Column, Row};
        use crate::stream::ResultSet;

        let cols: Vec<Column> = (0..9)
            .map(|i| Column::new(format!("c{i}"), i, "x"))
            .collect();
        let mut sets = vec![ResultSet::new(cols, Vec::<Row>::new())];
        assert!(ParameterEncryptionInfo::from_describe_result_sets(&mut sets).is_err());
    }
}
