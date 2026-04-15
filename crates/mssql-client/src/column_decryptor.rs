//! Always Encrypted column decryption bridge.
//!
//! This module provides the [`ColumnDecryptor`] type which bridges the async
//! key resolution step (at ColMetaData time) with the synchronous per-row
//! decryption step (during row parsing).
//!
//! ## Design
//!
//! `EncryptionContext::get_encryptor()` is async because key store providers
//! (e.g., Azure Key Vault) may perform network I/O. However, column value
//! parsing is synchronous. The solution is to pre-resolve all encryptors when
//! the ColMetaData token arrives (in an async context), then use them
//! synchronously when decrypting individual row values.

use std::sync::Arc;

use mssql_auth::AeadEncryptor;
use tds_protocol::crypto::CryptoMetadata;
use tds_protocol::token::{ColMetaData, ColumnData};

use crate::encryption::EncryptionContext;
use crate::error::{Error, Result};

/// Pre-resolved encryption state for a result set.
///
/// Created asynchronously when a ColMetaData token with encryption metadata
/// is received. Used synchronously when parsing row values.
pub(crate) struct ColumnDecryptor {
    /// Per-column decryption info. `None` for unencrypted columns.
    columns: Vec<Option<ColumnDecryptionInfo>>,
}

/// Decryption info for a single encrypted column.
struct ColumnDecryptionInfo {
    /// The pre-resolved AEAD encryptor (contains the decrypted CEK).
    encryptor: Arc<AeadEncryptor>,
    /// The plaintext column metadata (from CryptoMetadata base type info).
    /// Used to re-parse decrypted bytes into the correct SqlValue type.
    base_column: ColumnData,
}

impl ColumnDecryptor {
    /// Pre-resolve all encryptors for the columns in this result set.
    ///
    /// This is the async entry point — called when a ColMetaData token arrives.
    /// It resolves CEKs via the key store providers (which may be async),
    /// then returns a ColumnDecryptor that can be used synchronously for rows.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A key store provider is not registered for a CEK's provider name
    /// - CEK decryption fails (wrong key, corrupted CEK)
    /// - The CekTable doesn't contain an entry for a column's ordinal
    pub(crate) async fn from_metadata(meta: &ColMetaData, ctx: &EncryptionContext) -> Result<Self> {
        let cek_table = meta.cek_table.as_ref().ok_or_else(|| {
            Error::Encryption("encrypted result set has no CEK table".to_string())
        })?;

        let mut columns = Vec::with_capacity(meta.columns.len());

        for col in &meta.columns {
            if let Some(ref crypto) = col.crypto_metadata {
                // Look up the CEK entry
                let cek_entry = cek_table.get(crypto.cek_table_ordinal).ok_or_else(|| {
                    Error::Encryption(format!(
                        "CEK table ordinal {} out of range (table has {} entries)",
                        crypto.cek_table_ordinal,
                        cek_table.len()
                    ))
                })?;

                // Resolve the encryptor (async — may call Azure Key Vault etc.)
                let encryptor = ctx.get_encryptor(cek_entry).await?;

                // Build the base ColumnData for the plaintext type.
                // This is used when re-parsing decrypted bytes.
                let base_column = build_base_column(col, crypto);

                columns.push(Some(ColumnDecryptionInfo {
                    encryptor,
                    base_column,
                }));
            } else {
                columns.push(None);
            }
        }

        Ok(Self { columns })
    }

    /// Check if a column at the given ordinal is encrypted.
    #[inline]
    pub(crate) fn is_encrypted(&self, ordinal: usize) -> bool {
        self.columns.get(ordinal).is_some_and(|c| c.is_some())
    }

    /// Decrypt a column value and return the plaintext bytes with the base column metadata.
    ///
    /// The caller should re-parse the returned bytes using the base `ColumnData`
    /// to get the correct `SqlValue` for the plaintext type.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The column is not encrypted (caller should check `is_encrypted` first)
    /// - HMAC verification fails (corrupted or tampered ciphertext)
    /// - AES decryption fails
    pub(crate) fn decrypt_column_value(
        &self,
        ordinal: usize,
        ciphertext: &[u8],
    ) -> Result<(Vec<u8>, &ColumnData)> {
        let info = self
            .columns
            .get(ordinal)
            .and_then(|c| c.as_ref())
            .ok_or_else(|| {
                Error::Encryption(format!("column {ordinal} is not encrypted or out of range"))
            })?;

        // SECURITY: This is the actual decryption.
        // AeadEncryptor::decrypt verifies the HMAC (constant-time) before
        // decrypting. If verification fails, an error is returned — never
        // garbled data.
        let plaintext = info.encryptor.decrypt(ciphertext).map_err(|e| {
            // Do NOT log ciphertext or plaintext in error messages
            Error::Encryption(format!("column {ordinal} decryption failed: {e}"))
        })?;

        Ok((plaintext, &info.base_column))
    }
}

/// Build a synthetic `ColumnData` representing the plaintext type.
///
/// When a column is encrypted, its wire type is `BigVarBinary` (the ciphertext
/// container). The real type lives in `CryptoMetadata.base_type_info`. This
/// function creates a `ColumnData` with the base type info so the column parser
/// can correctly parse the decrypted plaintext bytes.
fn build_base_column(col: &ColumnData, crypto: &CryptoMetadata) -> ColumnData {
    let base_type_id = crypto.base_type_id();
    ColumnData {
        name: col.name.clone(),
        type_id: base_type_id,
        col_type: crypto.base_col_type,
        flags: col.flags,
        user_type: crypto.base_user_type,
        type_info: crypto.base_type_info.clone(),
        crypto_metadata: None, // The base column is not itself encrypted
    }
}

impl std::fmt::Debug for ColumnDecryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColumnDecryptor")
            .field("column_count", &self.columns.len())
            .field(
                "encrypted_count",
                &self.columns.iter().filter(|c| c.is_some()).count(),
            )
            .finish()
    }
}
