//! SQL batch request encoding.
//!
//! This module provides encoding for SQL batch requests (packet type 0x01).
//! Per MS-TDS spec, a SQL batch contains:
//! - ALL_HEADERS section (required for TDS 7.2+)
//! - SQL text encoded as UTF-16LE

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::write_utf16_string;
use crate::prelude::*;

/// Encode a SQL batch request with auto-commit (no explicit transaction).
///
/// The SQL batch packet payload includes:
/// 1. ALL_HEADERS section (required for TDS 7.2+)
/// 2. SQL text encoded as UTF-16LE
///
/// This function returns the encoded payload (without the packet header).
/// For requests within an explicit transaction, use [`encode_sql_batch_with_transaction`].
///
/// # Example
///
/// ```
/// use tds_protocol::sql_batch::encode_sql_batch;
///
/// let sql = "SELECT * FROM users WHERE id = 1";
/// let payload = encode_sql_batch(sql);
///
/// // Payload includes ALL_HEADERS + UTF-16LE encoded SQL
/// assert!(!payload.is_empty());
/// ```
#[must_use]
pub fn encode_sql_batch(sql: &str) -> Bytes {
    encode_sql_batch_with_transaction(sql, 0)
}

/// Encode a SQL batch request with a transaction descriptor.
///
/// Per MS-TDS spec, when executing within an explicit transaction:
/// - The `transaction_descriptor` MUST be the value returned by the server
///   in the BeginTransaction EnvChange token.
/// - For auto-commit mode (no explicit transaction), use 0.
///
/// # Arguments
///
/// * `sql` - The SQL text to execute
/// * `transaction_descriptor` - The transaction descriptor from BeginTransaction EnvChange,
///   or 0 for auto-commit mode.
///
/// # Example
///
/// ```
/// use tds_protocol::sql_batch::encode_sql_batch_with_transaction;
///
/// // Within a transaction with descriptor 0x1234567890ABCDEF
/// let sql = "INSERT INTO users VALUES (1, 'Alice')";
/// let tx_descriptor = 0x1234567890ABCDEF_u64;
/// let payload = encode_sql_batch_with_transaction(sql, tx_descriptor);
/// ```
#[must_use]
pub fn encode_sql_batch_with_transaction(sql: &str, transaction_descriptor: u64) -> Bytes {
    // Capacity: ALL_HEADERS (22 bytes) + SQL UTF-16LE (sql.len() * 2)
    let mut buf = BytesMut::with_capacity(22 + sql.len() * 2);

    // ALL_HEADERS section (required for TDS 7.2+)
    // Per MS-TDS spec: ALL_HEADERS = TotalLength + Headers
    let all_headers_start = buf.len();
    buf.put_u32_le(0); // Total length placeholder

    // Transaction descriptor header (type 0x0002)
    // Per MS-TDS 2.2.5.3: HeaderLength (4) + HeaderType (2) + TransactionDescriptor (8) + OutstandingRequestCount (4)
    buf.put_u32_le(18); // Header length = 18 bytes
    buf.put_u16_le(0x0002); // Header type: transaction descriptor
    buf.put_u64_le(transaction_descriptor); // Transaction descriptor from BeginTransaction EnvChange
    buf.put_u32_le(1); // Outstanding request count (1 for non-MARS connections)

    // Fill in ALL_HEADERS total length
    let all_headers_len = buf.len() - all_headers_start;
    let len_bytes = (all_headers_len as u32).to_le_bytes();
    buf[all_headers_start..all_headers_start + 4].copy_from_slice(&len_bytes);

    // SQL text as UTF-16LE
    write_utf16_string(&mut buf, sql);

    buf.freeze()
}

/// SQL batch builder for more complex batches.
///
/// This can be used to build batches with multiple statements
/// or to add headers for specific features.
#[derive(Debug, Clone)]
pub struct SqlBatch {
    sql: String,
}

impl SqlBatch {
    /// Create a new SQL batch.
    #[must_use]
    pub fn new(sql: impl Into<String>) -> Self {
        Self { sql: sql.into() }
    }

    /// Get the SQL text.
    #[must_use]
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Encode the SQL batch to bytes.
    #[must_use]
    pub fn encode(&self) -> Bytes {
        encode_sql_batch(&self.sql)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_sql_batch() {
        let sql = "SELECT 1";
        let payload = encode_sql_batch(sql);

        // ALL_HEADERS (22 bytes) + UTF-16LE encoded (8 chars * 2 bytes = 16 bytes) = 38 bytes
        assert_eq!(payload.len(), 38);

        // Verify ALL_HEADERS section
        // Total length at bytes 0-3 (little-endian)
        assert_eq!(&payload[0..4], &[22, 0, 0, 0]); // TotalLength = 22

        // Header length at bytes 4-7
        assert_eq!(&payload[4..8], &[18, 0, 0, 0]); // HeaderLength = 18

        // Header type at bytes 8-9
        assert_eq!(&payload[8..10], &[0x02, 0x00]); // Transaction descriptor

        // Verify UTF-16LE SQL starts at byte 22
        // 'S' = 0x53, 'E' = 0x45, etc.
        assert_eq!(payload[22], b'S');
        assert_eq!(payload[23], 0);
        assert_eq!(payload[24], b'E');
        assert_eq!(payload[25], 0);
    }

    #[test]
    fn test_sql_batch_builder() {
        let batch = SqlBatch::new("SELECT @@VERSION");
        assert_eq!(batch.sql(), "SELECT @@VERSION");

        let payload = batch.encode();
        assert!(!payload.is_empty());
    }

    #[test]
    fn test_empty_batch() {
        let payload = encode_sql_batch("");
        // Even empty SQL has ALL_HEADERS (22 bytes)
        assert_eq!(payload.len(), 22);
    }
}
