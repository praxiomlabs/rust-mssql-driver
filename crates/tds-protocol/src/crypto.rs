//! Always Encrypted cryptography metadata for TDS protocol.
//!
//! This module defines the wire-level structures for SQL Server's Always Encrypted
//! feature. When a query returns encrypted columns, SQL Server sends additional
//! metadata describing how to decrypt the data.
//!
//! ## TDS Wire Format
//!
//! When Always Encrypted is enabled, the COLMETADATA token includes:
//!
//! 1. **CEK Table**: A table of Column Encryption Keys needed for the result set
//! 2. **CryptoMetadata**: Per-column encryption information
//!
//! ```text
//! COLMETADATA Token (with encryption):
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ Column Count (2 bytes)                                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ CEK Table (if encrypted columns present)                        │
//! │ ├── CEK Count (2 bytes)                                         │
//! │ ├── CEK Entry 1                                                 │
//! │ │   ├── Database ID (4 bytes)                                   │
//! │ │   ├── CEK ID (4 bytes)                                        │
//! │ │   ├── CEK Version (4 bytes)                                   │
//! │ │   ├── CEK MD Version (8 bytes)                                │
//! │ │   ├── CEK Value Count (1 byte)                                │
//! │ │   └── CEK Value(s)                                            │
//! │ │       ├── Encrypted Value Length (2 bytes)                    │
//! │ │       ├── Encrypted Value (variable)                          │
//! │ │       ├── Key Store Name (B_VARCHAR)                          │
//! │ │       ├── CMK Path (US_VARCHAR)                               │
//! │ │       └── Algorithm (B_VARCHAR)                               │
//! │ └── ...more CEK entries                                         │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ Column Definitions                                              │
//! │ ├── Column 1                                                    │
//! │ │   ├── User Type (4 bytes)                                     │
//! │ │   ├── Flags (2 bytes) - includes encryption flag              │
//! │ │   ├── Type ID (1 byte)                                        │
//! │ │   ├── Type Info (variable)                                    │
//! │ │   ├── CryptoMetadata (if encrypted)                           │
//! │ │   │   ├── CEK Table Ordinal (2 bytes)                         │
//! │ │   │   ├── Algorithm ID (1 byte)                               │
//! │ │   │   ├── Encryption Type (1 byte)                            │
//! │ │   │   └── Normalization Version (1 byte)                      │
//! │ │   └── Column Name (B_VARCHAR)                                 │
//! │ └── ...more columns                                             │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use bytes::{Buf, Bytes};

use crate::codec::{read_b_varchar, read_us_varchar};
use crate::error::ProtocolError;
use crate::prelude::*;

/// Column flags bit indicating the column is encrypted.
pub const COLUMN_FLAG_ENCRYPTED: u16 = 0x0800;

/// Algorithm ID for AEAD_AES_256_CBC_HMAC_SHA256.
pub const ALGORITHM_AEAD_AES_256_CBC_HMAC_SHA256: u8 = 2;

/// Encryption type: Deterministic.
pub const ENCRYPTION_TYPE_DETERMINISTIC: u8 = 1;

/// Encryption type: Randomized.
pub const ENCRYPTION_TYPE_RANDOMIZED: u8 = 2;

/// Current normalization rule version.
pub const NORMALIZATION_RULE_VERSION: u8 = 1;

/// Column Encryption Key table entry.
///
/// This represents a single CEK entry in the CEK table sent with COLMETADATA.
/// Multiple columns may share the same CEK.
#[derive(Debug, Clone)]
pub struct CekTableEntry {
    /// Database ID where the CEK is defined.
    pub database_id: u32,
    /// CEK ID within the database.
    pub cek_id: u32,
    /// CEK version (incremented on key rotation).
    pub cek_version: u32,
    /// Metadata version (changes with any metadata update).
    pub cek_md_version: u64,
    /// CEK value entries (usually one, but may have multiple for key rotation).
    pub values: Vec<CekValue>,
}

/// A single CEK value (encrypted by CMK).
///
/// A CEK may have multiple values when key rotation is in progress,
/// with different CMKs encrypting the same CEK.
#[derive(Debug, Clone)]
pub struct CekValue {
    /// The encrypted CEK bytes.
    pub encrypted_value: Bytes,
    /// Name of the key store provider (e.g., "AZURE_KEY_VAULT").
    pub key_store_provider_name: String,
    /// Path to the Column Master Key in the key store.
    pub cmk_path: String,
    /// Asymmetric algorithm used to encrypt the CEK (e.g., "RSA_OAEP").
    pub encryption_algorithm: String,
}

/// Per-column encryption metadata.
///
/// This metadata is present for each encrypted column and describes
/// how to decrypt the column data.
#[derive(Debug, Clone)]
pub struct CryptoMetadata {
    /// Index into the CEK table (0-based).
    pub cek_table_ordinal: u16,
    /// Encryption algorithm ID.
    pub algorithm_id: u8,
    /// Encryption type (deterministic or randomized).
    pub encryption_type: EncryptionTypeWire,
    /// Normalization rule version.
    pub normalization_version: u8,
}

/// Wire-level encryption type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionTypeWire {
    /// Deterministic encryption (value 1).
    Deterministic,
    /// Randomized encryption (value 2).
    Randomized,
}

impl EncryptionTypeWire {
    /// Create from wire value.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            ENCRYPTION_TYPE_DETERMINISTIC => Some(Self::Deterministic),
            ENCRYPTION_TYPE_RANDOMIZED => Some(Self::Randomized),
            _ => None,
        }
    }

    /// Convert to wire value.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Deterministic => ENCRYPTION_TYPE_DETERMINISTIC,
            Self::Randomized => ENCRYPTION_TYPE_RANDOMIZED,
        }
    }
}

/// CEK table containing all Column Encryption Keys needed for a result set.
#[derive(Debug, Clone, Default)]
pub struct CekTable {
    /// CEK entries.
    pub entries: Vec<CekTableEntry>,
}

impl CekTable {
    /// Create an empty CEK table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a CEK entry by ordinal.
    #[must_use]
    pub fn get(&self, ordinal: u16) -> Option<&CekTableEntry> {
        self.entries.get(ordinal as usize)
    }

    /// Check if the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Decode a CEK table from the wire format.
    ///
    /// # Wire Format
    ///
    /// ```text
    /// CEK_TABLE:
    ///   cek_count: USHORT (2 bytes)
    ///   entries: CEK_ENTRY[cek_count]
    ///
    /// CEK_ENTRY:
    ///   database_id: DWORD (4 bytes)
    ///   cek_id: DWORD (4 bytes)
    ///   cek_version: DWORD (4 bytes)
    ///   cek_md_version: ULONGLONG (8 bytes)
    ///   value_count: BYTE (1 byte)
    ///   values: CEK_VALUE[value_count]
    ///
    /// CEK_VALUE:
    ///   encrypted_value_length: USHORT (2 bytes)
    ///   encrypted_value: BYTE[encrypted_value_length]
    ///   key_store_name: B_VARCHAR
    ///   cmk_path: US_VARCHAR
    ///   algorithm: B_VARCHAR
    /// ```
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let cek_count = src.get_u16_le() as usize;

        let mut entries = Vec::with_capacity(cek_count);

        for _ in 0..cek_count {
            let entry = CekTableEntry::decode(src)?;
            entries.push(entry);
        }

        Ok(Self { entries })
    }
}

impl CekTableEntry {
    /// Decode a CEK table entry from the wire format.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        // database_id (4) + cek_id (4) + cek_version (4) + cek_md_version (8) + value_count (1)
        if src.remaining() < 21 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let database_id = src.get_u32_le();
        let cek_id = src.get_u32_le();
        let cek_version = src.get_u32_le();
        let cek_md_version = src.get_u64_le();
        let value_count = src.get_u8() as usize;

        let mut values = Vec::with_capacity(value_count);

        for _ in 0..value_count {
            let value = CekValue::decode(src)?;
            values.push(value);
        }

        Ok(Self {
            database_id,
            cek_id,
            cek_version,
            cek_md_version,
            values,
        })
    }

    /// Get the first (primary) encrypted value.
    #[must_use]
    pub fn primary_value(&self) -> Option<&CekValue> {
        self.values.first()
    }
}

impl CekValue {
    /// Decode a CEK value from the wire format.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        // encrypted_value_length (2 bytes)
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let encrypted_value_length = src.get_u16_le() as usize;

        if src.remaining() < encrypted_value_length {
            return Err(ProtocolError::UnexpectedEof);
        }

        let encrypted_value = src.copy_to_bytes(encrypted_value_length);

        // key_store_name (B_VARCHAR)
        let key_store_provider_name = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        // cmk_path (US_VARCHAR)
        let cmk_path = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        // algorithm (B_VARCHAR)
        let encryption_algorithm = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        Ok(Self {
            encrypted_value,
            key_store_provider_name,
            cmk_path,
            encryption_algorithm,
        })
    }
}

impl CryptoMetadata {
    /// Size of crypto metadata in bytes.
    pub const SIZE: usize = 5; // ordinal (2) + algorithm (1) + enc_type (1) + norm_version (1)

    /// Decode crypto metadata from the wire format.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < Self::SIZE {
            return Err(ProtocolError::UnexpectedEof);
        }

        let cek_table_ordinal = src.get_u16_le();
        let algorithm_id = src.get_u8();
        let encryption_type_byte = src.get_u8();
        let normalization_version = src.get_u8();

        let encryption_type = EncryptionTypeWire::from_u8(encryption_type_byte).ok_or(
            ProtocolError::InvalidField {
                field: "encryption_type",
                value: encryption_type_byte as u32,
            },
        )?;

        Ok(Self {
            cek_table_ordinal,
            algorithm_id,
            encryption_type,
            normalization_version,
        })
    }

    /// Check if this uses the standard AEAD algorithm.
    #[must_use]
    pub fn is_aead_aes_256(&self) -> bool {
        self.algorithm_id == ALGORITHM_AEAD_AES_256_CBC_HMAC_SHA256
    }

    /// Check if this uses deterministic encryption.
    #[must_use]
    pub fn is_deterministic(&self) -> bool {
        self.encryption_type == EncryptionTypeWire::Deterministic
    }

    /// Check if this uses randomized encryption.
    #[must_use]
    pub fn is_randomized(&self) -> bool {
        self.encryption_type == EncryptionTypeWire::Randomized
    }
}

/// Extended column metadata with encryption information.
///
/// This combines the base column metadata with optional crypto metadata
/// for Always Encrypted columns.
#[derive(Debug, Clone, Default)]
pub struct ColumnCryptoInfo {
    /// Crypto metadata (if column is encrypted).
    pub crypto_metadata: Option<CryptoMetadata>,
}

impl ColumnCryptoInfo {
    /// Create info for an unencrypted column.
    #[must_use]
    pub fn unencrypted() -> Self {
        Self {
            crypto_metadata: None,
        }
    }

    /// Create info for an encrypted column.
    #[must_use]
    pub fn encrypted(metadata: CryptoMetadata) -> Self {
        Self {
            crypto_metadata: Some(metadata),
        }
    }

    /// Check if this column is encrypted.
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.crypto_metadata.is_some()
    }
}

/// Check if a column flags value indicates encryption.
#[must_use]
pub fn is_column_encrypted(flags: u16) -> bool {
    (flags & COLUMN_FLAG_ENCRYPTED) != 0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_encryption_type_wire_roundtrip() {
        assert_eq!(
            EncryptionTypeWire::from_u8(1),
            Some(EncryptionTypeWire::Deterministic)
        );
        assert_eq!(
            EncryptionTypeWire::from_u8(2),
            Some(EncryptionTypeWire::Randomized)
        );
        assert_eq!(EncryptionTypeWire::from_u8(0), None);
        assert_eq!(EncryptionTypeWire::from_u8(99), None);

        assert_eq!(EncryptionTypeWire::Deterministic.to_u8(), 1);
        assert_eq!(EncryptionTypeWire::Randomized.to_u8(), 2);
    }

    #[test]
    fn test_crypto_metadata_decode() {
        let data = [
            0x00, 0x00, // cek_table_ordinal = 0
            0x02, // algorithm_id = AEAD_AES_256_CBC_HMAC_SHA256
            0x01, // encryption_type = Deterministic
            0x01, // normalization_version = 1
        ];

        let mut cursor: &[u8] = &data;
        let metadata = CryptoMetadata::decode(&mut cursor).unwrap();

        assert_eq!(metadata.cek_table_ordinal, 0);
        assert_eq!(
            metadata.algorithm_id,
            ALGORITHM_AEAD_AES_256_CBC_HMAC_SHA256
        );
        assert_eq!(metadata.encryption_type, EncryptionTypeWire::Deterministic);
        assert_eq!(metadata.normalization_version, 1);
        assert!(metadata.is_aead_aes_256());
        assert!(metadata.is_deterministic());
        assert!(!metadata.is_randomized());
    }

    #[test]
    fn test_cek_value_decode() {
        let mut data = BytesMut::new();

        // encrypted_value_length = 4
        data.extend_from_slice(&[0x04, 0x00]);
        // encrypted_value = [0xDE, 0xAD, 0xBE, 0xEF]
        data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        // key_store_name = "TEST" (B_VARCHAR: 1 byte len + utf16le)
        data.extend_from_slice(&[0x04]); // 4 chars
        data.extend_from_slice(&[b'T', 0x00, b'E', 0x00, b'S', 0x00, b'T', 0x00]);
        // cmk_path = "key1" (US_VARCHAR: 2 byte len + utf16le)
        data.extend_from_slice(&[0x04, 0x00]); // 4 chars
        data.extend_from_slice(&[b'k', 0x00, b'e', 0x00, b'y', 0x00, b'1', 0x00]);
        // algorithm = "RSA" (B_VARCHAR)
        data.extend_from_slice(&[0x03]); // 3 chars
        data.extend_from_slice(&[b'R', 0x00, b'S', 0x00, b'A', 0x00]);

        let mut cursor: &[u8] = &data;
        let value = CekValue::decode(&mut cursor).unwrap();

        assert_eq!(value.encrypted_value.as_ref(), &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(value.key_store_provider_name, "TEST");
        assert_eq!(value.cmk_path, "key1");
        assert_eq!(value.encryption_algorithm, "RSA");
    }

    #[test]
    fn test_cek_table_entry_decode() {
        let mut data = BytesMut::new();

        // database_id = 1
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        // cek_id = 2
        data.extend_from_slice(&[0x02, 0x00, 0x00, 0x00]);
        // cek_version = 1
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        // cek_md_version = 100
        data.extend_from_slice(&[0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        // value_count = 1
        data.extend_from_slice(&[0x01]);

        // CEK value
        data.extend_from_slice(&[0x04, 0x00]); // encrypted_value_length = 4
        data.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]); // encrypted_value
        data.extend_from_slice(&[0x02]); // key_store_name length = 2
        data.extend_from_slice(&[b'K', 0x00, b'S', 0x00]); // "KS"
        data.extend_from_slice(&[0x01, 0x00]); // cmk_path length = 1
        data.extend_from_slice(&[b'P', 0x00]); // "P"
        data.extend_from_slice(&[0x01]); // algorithm length = 1
        data.extend_from_slice(&[b'A', 0x00]); // "A"

        let mut cursor: &[u8] = &data;
        let entry = CekTableEntry::decode(&mut cursor).expect("should decode entry");

        assert_eq!(entry.database_id, 1);
        assert_eq!(entry.cek_id, 2);
        assert_eq!(entry.cek_version, 1);
        assert_eq!(entry.cek_md_version, 100);
        assert_eq!(entry.values.len(), 1);

        let value = entry.primary_value().expect("should have primary value");
        assert_eq!(value.encrypted_value.as_ref(), &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn test_cek_table_decode() {
        let mut data = BytesMut::new();

        // cek_count = 1
        data.extend_from_slice(&[0x01, 0x00]);

        // CEK entry
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // database_id
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // cek_id
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // cek_version
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // cek_md_version
        data.extend_from_slice(&[0x01]); // value_count

        // CEK value
        data.extend_from_slice(&[0x02, 0x00]); // encrypted_value_length = 2
        data.extend_from_slice(&[0xAB, 0xCD]); // encrypted_value
        data.extend_from_slice(&[0x01]); // key_store_name = "K"
        data.extend_from_slice(&[b'K', 0x00]);
        data.extend_from_slice(&[0x01, 0x00]); // cmk_path = "P"
        data.extend_from_slice(&[b'P', 0x00]);
        data.extend_from_slice(&[0x01]); // algorithm = "A"
        data.extend_from_slice(&[b'A', 0x00]);

        let mut cursor: &[u8] = &data;
        let table = CekTable::decode(&mut cursor).expect("should decode table");

        assert_eq!(table.len(), 1);
        assert!(!table.is_empty());

        let entry = table.get(0).expect("should have first entry");
        assert_eq!(entry.database_id, 1);
    }

    #[test]
    fn test_is_column_encrypted() {
        assert!(!is_column_encrypted(0x0000));
        assert!(!is_column_encrypted(0x0001)); // nullable
        assert!(is_column_encrypted(0x0800)); // encrypted flag
        assert!(is_column_encrypted(0x0801)); // encrypted + nullable
    }

    #[test]
    fn test_column_crypto_info() {
        let unencrypted = ColumnCryptoInfo::unencrypted();
        assert!(!unencrypted.is_encrypted());

        let metadata = CryptoMetadata {
            cek_table_ordinal: 0,
            algorithm_id: 2,
            encryption_type: EncryptionTypeWire::Randomized,
            normalization_version: 1,
        };
        let encrypted = ColumnCryptoInfo::encrypted(metadata);
        assert!(encrypted.is_encrypted());
    }
}
