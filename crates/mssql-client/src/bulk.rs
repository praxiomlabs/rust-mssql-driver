//! Bulk Copy Protocol (BCP) support.
//!
//! This module provides first-class support for bulk insert operations using
//! the TDS Bulk Load protocol (packet type 0x07). BCP is significantly more
//! efficient than individual INSERT statements for loading large amounts of data.
//!
//! ## Performance Benefits
//!
//! - Minimal logging (when using simple recovery model)
//! - Batch commits reduce lock contention
//! - Direct data streaming without SQL parsing overhead
//! - Optional table lock for maximum throughput
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_client::{Client, BulkInsert, BulkOptions};
//!
//! let mut bulk = client
//!     .bulk_insert("dbo.Users")
//!     .with_columns(&["id", "name", "email"])
//!     .with_options(BulkOptions {
//!         batch_size: 1000,
//!         check_constraints: true,
//!         fire_triggers: false,
//!         keep_nulls: true,
//!         table_lock: true,
//!     })
//!     .build()
//!     .await?;
//!
//! // Send rows
//! for user in users {
//!     bulk.send_row(&[&user.id, &user.name, &user.email]).await?;
//! }
//!
//! let result = bulk.finish().await?;
//! println!("Inserted {} rows", result.rows_affected);
//! ```
//!
//! ## Implementation Notes
//!
//! The bulk load protocol uses:
//! - Packet type 0x07 (BulkLoad)
//! - COLMETADATA token describing column structure
//! - ROW tokens containing actual data
//! - DONE token signaling completion
//!
//! Per MS-TDS specification, the row data format matches the server output format
//! (same as SELECT results) rather than storage format.

use bytes::{BufMut, BytesMut};
use std::sync::Arc;

use mssql_types::{SqlValue, ToSql, TypeError};
use tds_protocol::packet::{PacketHeader, PacketStatus, PacketType};
use tds_protocol::token::{DoneStatus, TokenType};

use crate::error::Error;

/// Options controlling bulk insert behavior.
///
/// These options map to SQL Server's BULK INSERT hints and
/// affect performance, logging, and constraint checking.
#[derive(Debug, Clone)]
pub struct BulkOptions {
    /// Number of rows per batch commit.
    ///
    /// Smaller batches use less memory but have more overhead.
    /// Larger batches are more efficient but hold locks longer.
    /// Default: 0 (single batch for entire operation).
    pub batch_size: usize,

    /// Check constraints during insert.
    ///
    /// Default: true
    pub check_constraints: bool,

    /// Fire INSERT triggers on the table.
    ///
    /// Default: false (better performance)
    pub fire_triggers: bool,

    /// Keep NULL values instead of using column defaults.
    ///
    /// Default: true
    pub keep_nulls: bool,

    /// Acquire a table-level lock for the duration of the bulk operation.
    ///
    /// This can significantly improve performance by reducing lock
    /// escalation overhead, but blocks all other access to the table.
    /// Default: false
    pub table_lock: bool,

    /// Order hint for the data being inserted.
    ///
    /// If data is pre-sorted by the clustered index, specify the columns
    /// here to avoid a sort operation on the server.
    /// Default: None
    pub order_hint: Option<Vec<String>>,

    /// Maximum errors allowed before aborting.
    ///
    /// Default: 0 (abort on first error)
    pub max_errors: u32,
}

impl Default for BulkOptions {
    fn default() -> Self {
        Self {
            batch_size: 0,
            check_constraints: true,
            fire_triggers: false,
            keep_nulls: true,
            table_lock: false,
            order_hint: None,
            max_errors: 0,
        }
    }
}

/// Column definition for bulk insert.
#[derive(Debug, Clone)]
pub struct BulkColumn {
    /// Column name.
    pub name: String,
    /// SQL Server type (e.g., "INT", "NVARCHAR(100)").
    pub sql_type: String,
    /// Whether the column allows NULL values.
    pub nullable: bool,
    /// Column ordinal (0-based).
    pub ordinal: usize,
    /// TDS type ID.
    type_id: u8,
    /// Maximum length for variable-length types.
    max_length: Option<u32>,
    /// Precision for decimal types.
    precision: Option<u8>,
    /// Scale for decimal types.
    scale: Option<u8>,
}

impl BulkColumn {
    /// Create a new bulk column definition.
    pub fn new<S: Into<String>>(name: S, sql_type: S, ordinal: usize) -> Self {
        let sql_type_str: String = sql_type.into();
        let (type_id, max_length, precision, scale) = parse_sql_type(&sql_type_str);

        Self {
            name: name.into(),
            sql_type: sql_type_str,
            nullable: true,
            ordinal,
            type_id,
            max_length,
            precision,
            scale,
        }
    }

    /// Set whether this column allows NULL values.
    #[must_use]
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }
}

/// Parse SQL type string into TDS type information.
fn parse_sql_type(sql_type: &str) -> (u8, Option<u32>, Option<u8>, Option<u8>) {
    let upper = sql_type.to_uppercase();

    // Extract base type and parameters
    let (base, params) = if let Some(paren_pos) = upper.find('(') {
        let base = &upper[..paren_pos];
        let params_str = upper[paren_pos + 1..].trim_end_matches(')');
        (base, Some(params_str))
    } else {
        (upper.as_str(), None)
    };

    match base {
        "BIT" => (0x32, None, None, None),
        "TINYINT" => (0x30, None, None, None),
        "SMALLINT" => (0x34, None, None, None),
        "INT" => (0x38, None, None, None),
        "BIGINT" => (0x7F, None, None, None),
        "REAL" => (0x3B, None, None, None),
        "FLOAT" => (0x3E, None, None, None),
        "DATE" => (0x28, None, None, None),
        "TIME" => {
            let scale = params.and_then(|p| p.parse().ok()).unwrap_or(7);
            (0x29, None, None, Some(scale))
        }
        "DATETIME" => (0x3D, None, None, None),
        "DATETIME2" => {
            let scale = params.and_then(|p| p.parse().ok()).unwrap_or(7);
            (0x2A, None, None, Some(scale))
        }
        "DATETIMEOFFSET" => {
            let scale = params.and_then(|p| p.parse().ok()).unwrap_or(7);
            (0x2B, None, None, Some(scale))
        }
        "SMALLDATETIME" => (0x3F, None, None, None),
        "UNIQUEIDENTIFIER" => (0x24, Some(16), None, None),
        "VARCHAR" | "CHAR" => {
            let len = params
                .and_then(|p| {
                    if p == "MAX" {
                        Some(0xFFFF_u32)
                    } else {
                        p.parse().ok()
                    }
                })
                .unwrap_or(8000);
            (0xA7, Some(len), None, None)
        }
        "NVARCHAR" | "NCHAR" => {
            let is_max = params.map(|p| p == "MAX").unwrap_or(false);
            if is_max {
                // MAX types use 0xFFFF marker (not doubled)
                (0xE7, Some(0xFFFF), None, None)
            } else {
                // Normal lengths are in characters, double for UTF-16 byte length
                let len = params.and_then(|p| p.parse().ok()).unwrap_or(4000);
                (0xE7, Some(len * 2), None, None)
            }
        }
        "VARBINARY" | "BINARY" => {
            let len = params
                .and_then(|p| {
                    if p == "MAX" {
                        Some(0xFFFF_u32)
                    } else {
                        p.parse().ok()
                    }
                })
                .unwrap_or(8000);
            (0xA5, Some(len), None, None)
        }
        "DECIMAL" | "NUMERIC" => {
            let (precision, scale) = if let Some(p) = params {
                let parts: Vec<&str> = p.split(',').map(|s| s.trim()).collect();
                (
                    parts.first().and_then(|s| s.parse().ok()).unwrap_or(18),
                    parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
                )
            } else {
                (18, 0)
            };
            (0x6C, None, Some(precision), Some(scale))
        }
        "MONEY" => (0x3C, Some(8), None, None),
        "SMALLMONEY" => (0x7A, Some(4), None, None),
        "XML" => (0xF1, Some(0xFFFF), None, None),
        "TEXT" => (0x23, Some(0x7FFF_FFFF), None, None),
        "NTEXT" => (0x63, Some(0x7FFF_FFFF), None, None),
        "IMAGE" => (0x22, Some(0x7FFF_FFFF), None, None),
        _ => (0xE7, Some(8000), None, None), // Default to NVARCHAR(4000)
    }
}

/// Result of a bulk insert operation.
#[derive(Debug, Clone)]
pub struct BulkInsertResult {
    /// Total number of rows inserted.
    pub rows_affected: u64,
    /// Number of batches committed.
    pub batches_committed: u32,
    /// Whether any errors were encountered.
    pub has_errors: bool,
}

/// Builder for configuring a bulk insert operation.
#[derive(Debug)]
pub struct BulkInsertBuilder {
    table_name: String,
    columns: Vec<BulkColumn>,
    options: BulkOptions,
}

impl BulkInsertBuilder {
    /// Create a new bulk insert builder for the specified table.
    pub fn new<S: Into<String>>(table_name: S) -> Self {
        Self {
            table_name: table_name.into(),
            columns: Vec::new(),
            options: BulkOptions::default(),
        }
    }

    /// Specify the columns to insert.
    ///
    /// Columns will be queried from the server if not specified,
    /// but providing them explicitly is more efficient.
    #[must_use]
    pub fn with_columns(mut self, column_names: &[&str]) -> Self {
        self.columns = column_names
            .iter()
            .enumerate()
            .map(|(i, name)| BulkColumn::new(*name, "NVARCHAR(MAX)", i))
            .collect();
        self
    }

    /// Specify columns with full type information.
    #[must_use]
    pub fn with_typed_columns(mut self, columns: Vec<BulkColumn>) -> Self {
        self.columns = columns;
        self
    }

    /// Set bulk insert options.
    #[must_use]
    pub fn with_options(mut self, options: BulkOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the batch size.
    #[must_use]
    pub fn batch_size(mut self, size: usize) -> Self {
        self.options.batch_size = size;
        self
    }

    /// Enable or disable table lock.
    #[must_use]
    pub fn table_lock(mut self, enabled: bool) -> Self {
        self.options.table_lock = enabled;
        self
    }

    /// Enable or disable trigger firing.
    #[must_use]
    pub fn fire_triggers(mut self, enabled: bool) -> Self {
        self.options.fire_triggers = enabled;
        self
    }

    /// Get the table name.
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    /// Get the columns.
    pub fn columns(&self) -> &[BulkColumn] {
        &self.columns
    }

    /// Get the options.
    pub fn options(&self) -> &BulkOptions {
        &self.options
    }

    /// Build the INSERT BULK SQL statement.
    pub fn build_insert_bulk_statement(&self) -> String {
        let mut sql = format!("INSERT BULK {}", self.table_name);

        // Add column definitions
        if !self.columns.is_empty() {
            sql.push_str(" (");
            let cols: Vec<String> = self
                .columns
                .iter()
                .map(|c| format!("{} {}", c.name, c.sql_type))
                .collect();
            sql.push_str(&cols.join(", "));
            sql.push(')');
        }

        // Add WITH clause for options
        let mut hints: Vec<String> = Vec::new();

        if self.options.check_constraints {
            hints.push("CHECK_CONSTRAINTS".to_string());
        }
        if self.options.fire_triggers {
            hints.push("FIRE_TRIGGERS".to_string());
        }
        if self.options.keep_nulls {
            hints.push("KEEP_NULLS".to_string());
        }
        if self.options.table_lock {
            hints.push("TABLOCK".to_string());
        }
        if self.options.batch_size > 0 {
            hints.push(format!("ROWS_PER_BATCH = {}", self.options.batch_size));
        }

        if let Some(ref order) = self.options.order_hint {
            hints.push(format!("ORDER({})", order.join(", ")));
        }

        if !hints.is_empty() {
            sql.push_str(" WITH (");
            sql.push_str(&hints.join(", "));
            sql.push(')');
        }

        sql
    }
}

/// Active bulk insert operation.
///
/// This struct manages the streaming of row data to the server.
/// Call `send_row()` for each row, then `finish()` to complete.
pub struct BulkInsert {
    /// Column metadata.
    columns: Arc<[BulkColumn]>,
    /// Buffer for accumulating rows.
    buffer: BytesMut,
    /// Rows in current batch.
    rows_in_batch: usize,
    /// Total rows sent.
    total_rows: u64,
    /// Batch size (0 = single batch).
    batch_size: usize,
    /// Number of batches committed.
    batches_committed: u32,
    /// Packet ID counter.
    packet_id: u8,
}

impl BulkInsert {
    /// Create a new bulk insert operation.
    pub fn new(columns: Vec<BulkColumn>, batch_size: usize) -> Self {
        let mut bulk = Self {
            columns: columns.into(),
            buffer: BytesMut::with_capacity(64 * 1024), // 64KB initial buffer
            rows_in_batch: 0,
            total_rows: 0,
            batch_size,
            batches_committed: 0,
            packet_id: 1,
        };

        // Write COLMETADATA token
        bulk.write_colmetadata();

        bulk
    }

    /// Write the COLMETADATA token to the buffer.
    fn write_colmetadata(&mut self) {
        let buf = &mut self.buffer;

        // Token type
        buf.put_u8(TokenType::ColMetaData as u8);

        // Column count
        buf.put_u16_le(self.columns.len() as u16);

        for col in self.columns.iter() {
            // User type (always 0 for basic types)
            buf.put_u32_le(0);

            // Flags: Nullable (bit 0) | CaseSen (bit 1) | Updateable (bits 2-3) | etc.
            let flags: u16 = if col.nullable { 0x0001 } else { 0x0000 };
            buf.put_u16_le(flags);

            // Type info
            buf.put_u8(col.type_id);

            // Type-specific length/precision/scale
            match col.type_id {
                // Fixed-length types - no additional info needed
                0x32 | 0x30 | 0x34 | 0x38 | 0x7F | 0x3B | 0x3E | 0x3D | 0x3F | 0x28 => {}

                // Variable-length string/binary types
                0xE7 | 0xA7 | 0xA5 | 0xAD => {
                    // Max length (2 bytes for normal, 4 bytes for MAX)
                    let max_len = col.max_length.unwrap_or(8000);
                    if max_len == 0xFFFF {
                        buf.put_u16_le(0xFFFF);
                    } else {
                        buf.put_u16_le(max_len as u16);
                    }

                    // Collation for string types (5 bytes)
                    if col.type_id == 0xE7 || col.type_id == 0xA7 {
                        // Default collation (Latin1_General_CI_AS)
                        buf.put_u32_le(0x0409_0904); // LCID + flags
                        buf.put_u8(52); // Sort ID
                    }
                }

                // Decimal/Numeric
                0x6C | 0x6A => {
                    // Length (calculated from precision)
                    let precision = col.precision.unwrap_or(18);
                    let len = decimal_byte_length(precision);
                    buf.put_u8(len);
                    buf.put_u8(precision);
                    buf.put_u8(col.scale.unwrap_or(0));
                }

                // Time-based with scale
                0x29..=0x2B => {
                    buf.put_u8(col.scale.unwrap_or(7));
                }

                // GUID
                0x24 => {
                    buf.put_u8(16);
                }

                // Other types - write max length if present
                _ => {
                    if let Some(len) = col.max_length {
                        if len <= 0xFFFF {
                            buf.put_u16_le(len as u16);
                        }
                    }
                }
            }

            // Column name (B_VARCHAR format: 1-byte length prefix)
            let name_utf16: Vec<u16> = col.name.encode_utf16().collect();
            buf.put_u8(name_utf16.len() as u8);
            for code_unit in name_utf16 {
                buf.put_u16_le(code_unit);
            }
        }
    }

    /// Send a row of data.
    ///
    /// The values must match the column order and types specified
    /// when creating the bulk insert.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Wrong number of values provided
    /// - A value cannot be converted to the expected type
    pub fn send_row<T: ToSql>(&mut self, values: &[T]) -> Result<(), Error> {
        if values.len() != self.columns.len() {
            return Err(Error::Config(format!(
                "expected {} values, got {}",
                self.columns.len(),
                values.len()
            )));
        }

        // Convert all values to SqlValue
        let sql_values: Result<Vec<SqlValue>, TypeError> =
            values.iter().map(|v| v.to_sql()).collect();
        let sql_values = sql_values.map_err(Error::from)?;

        self.write_row(&sql_values)?;

        self.rows_in_batch += 1;
        self.total_rows += 1;

        Ok(())
    }

    /// Send a row of pre-converted SQL values.
    pub fn send_row_values(&mut self, values: &[SqlValue]) -> Result<(), Error> {
        if values.len() != self.columns.len() {
            return Err(Error::Config(format!(
                "expected {} values, got {}",
                self.columns.len(),
                values.len()
            )));
        }

        self.write_row(values)?;

        self.rows_in_batch += 1;
        self.total_rows += 1;

        Ok(())
    }

    /// Write a ROW token to the buffer.
    fn write_row(&mut self, values: &[SqlValue]) -> Result<(), Error> {
        // ROW token type
        self.buffer.put_u8(TokenType::Row as u8);

        // Collect column info needed for encoding to avoid borrow conflict
        let columns: Vec<_> = self.columns.iter().cloned().collect();

        // Write each column value
        for (i, (col, value)) in columns.iter().zip(values.iter()).enumerate() {
            self.encode_column_value(col, value)
                .map_err(|e| Error::Config(format!("failed to encode column {}: {}", i, e)))?;
        }

        Ok(())
    }

    /// Encode a column value according to its type.
    fn encode_column_value(&mut self, col: &BulkColumn, value: &SqlValue) -> Result<(), TypeError> {
        let buf = &mut self.buffer;

        // Check if this column uses PLP (Partially Length-Prefixed) encoding
        // MAX types (max_length == 0xFFFF) use PLP format
        let is_plp_type =
            col.max_length == Some(0xFFFF) && matches!(col.type_id, 0xE7 | 0xA7 | 0xA5 | 0xAD);

        match value {
            SqlValue::Null => {
                // NULL encoding depends on type
                match col.type_id {
                    // Variable-length types
                    0xE7 | 0xA7 | 0xA5 | 0xAD => {
                        if is_plp_type {
                            // PLP NULL: 0xFFFFFFFFFFFFFFFF
                            buf.put_u64_le(0xFFFF_FFFF_FFFF_FFFF);
                        } else {
                            // Standard NULL: 0xFFFF length marker
                            buf.put_u16_le(0xFFFF);
                        }
                    }
                    // Nullable fixed types use 0 length
                    0x26 | 0x6C | 0x6A | 0x24 | 0x29 | 0x2A | 0x2B => {
                        buf.put_u8(0);
                    }
                    // Fixed types without nullable variant
                    _ => {
                        if col.nullable {
                            buf.put_u8(0);
                        } else {
                            return Err(TypeError::UnexpectedNull);
                        }
                    }
                }
            }

            SqlValue::Bool(v) => {
                buf.put_u8(1); // Length
                buf.put_u8(if *v { 1 } else { 0 });
            }

            SqlValue::TinyInt(v) => {
                buf.put_u8(1); // Length
                buf.put_u8(*v);
            }

            SqlValue::SmallInt(v) => {
                buf.put_u8(2); // Length
                buf.put_i16_le(*v);
            }

            SqlValue::Int(v) => {
                buf.put_u8(4); // Length
                buf.put_i32_le(*v);
            }

            SqlValue::BigInt(v) => {
                buf.put_u8(8); // Length
                buf.put_i64_le(*v);
            }

            SqlValue::Float(v) => {
                buf.put_u8(4); // Length
                buf.put_f32_le(*v);
            }

            SqlValue::Double(v) => {
                buf.put_u8(8); // Length
                buf.put_f64_le(*v);
            }

            SqlValue::String(s) => {
                // UTF-16LE encoding for NVARCHAR
                let utf16: Vec<u16> = s.encode_utf16().collect();
                let byte_len = utf16.len() * 2;

                if is_plp_type {
                    // PLP format for MAX types - supports unlimited size
                    // Send as a single chunk for simplicity
                    encode_plp_string(&utf16, buf);
                } else if byte_len > 0xFFFF {
                    // Non-MAX column can't hold this much data
                    return Err(TypeError::BufferTooSmall {
                        needed: byte_len,
                        available: 0xFFFF,
                    });
                } else {
                    // Standard encoding with 2-byte length prefix
                    buf.put_u16_le(byte_len as u16);
                    for code_unit in utf16 {
                        buf.put_u16_le(code_unit);
                    }
                }
            }

            SqlValue::Binary(b) => {
                if is_plp_type {
                    // PLP format for MAX types - supports unlimited size
                    encode_plp_binary(b, buf);
                } else if b.len() > 0xFFFF {
                    // Non-MAX column can't hold this much data
                    return Err(TypeError::BufferTooSmall {
                        needed: b.len(),
                        available: 0xFFFF,
                    });
                } else {
                    // Standard encoding with 2-byte length prefix
                    buf.put_u16_le(b.len() as u16);
                    buf.put_slice(b);
                }
            }

            // Feature-gated types - use mssql_types::encode module
            #[cfg(feature = "decimal")]
            SqlValue::Decimal(d) => {
                let precision = col.precision.unwrap_or(18);
                let len = decimal_byte_length(precision);
                buf.put_u8(len);

                // Sign: 0 = negative, 1 = positive
                buf.put_u8(if d.is_sign_negative() { 0 } else { 1 });

                // Mantissa as unsigned 128-bit integer
                let mantissa = d.mantissa().unsigned_abs();
                let mantissa_bytes = mantissa.to_le_bytes();
                buf.put_slice(&mantissa_bytes[..((len - 1) as usize)]);
            }

            #[cfg(feature = "uuid")]
            SqlValue::Uuid(u) => {
                buf.put_u8(16); // Length
                // Use mssql_types encode function
                mssql_types::encode::encode_uuid(*u, buf);
            }

            #[cfg(feature = "chrono")]
            SqlValue::Date(d) => {
                buf.put_u8(3); // Length
                mssql_types::encode::encode_date(*d, buf);
            }

            #[cfg(feature = "chrono")]
            SqlValue::Time(t) => {
                let scale = col.scale.unwrap_or(7);
                let len = time_byte_length(scale);
                buf.put_u8(len);
                // Encode time with proper scale handling
                encode_time_with_scale(*t, scale, buf);
            }

            #[cfg(feature = "chrono")]
            SqlValue::DateTime(dt) => {
                let scale = col.scale.unwrap_or(7);
                let time_len = time_byte_length(scale);
                let total_len = time_len + 3;
                buf.put_u8(total_len);
                // Encode time then date
                encode_time_with_scale(dt.time(), scale, buf);
                mssql_types::encode::encode_date(dt.date(), buf);
            }

            #[cfg(feature = "chrono")]
            SqlValue::DateTimeOffset(dto) => {
                let scale = col.scale.unwrap_or(7);
                let time_len = time_byte_length(scale);
                let total_len = time_len + 3 + 2;
                buf.put_u8(total_len);
                // Use mssql_types encode
                encode_time_with_scale(dto.time(), scale, buf);
                mssql_types::encode::encode_date(dto.date_naive(), buf);
                // Timezone offset in minutes
                use chrono::Offset;
                let offset_minutes = (dto.offset().fix().local_minus_utc() / 60) as i16;
                buf.put_i16_le(offset_minutes);
            }

            #[cfg(feature = "json")]
            SqlValue::Json(j) => {
                let s = j.to_string();
                encode_nvarchar_value(&s, buf)?;
            }

            SqlValue::Xml(x) => {
                encode_nvarchar_value(x, buf)?;
            }

            SqlValue::Tvp(_) => {
                // TVPs are not valid in bulk copy operations - they're for RPC parameters only
                return Err(TypeError::UnsupportedConversion {
                    from: "TVP".to_string(),
                    to: "bulk copy value",
                });
            }
            // Handle future SqlValue variants
            _ => {
                return Err(TypeError::UnsupportedConversion {
                    from: value.type_name().to_string(),
                    to: "bulk copy value",
                });
            }
        }

        Ok(())
    }
}

/// Encode a string as NVARCHAR with length prefix.
fn encode_nvarchar_value(s: &str, buf: &mut BytesMut) -> Result<(), TypeError> {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let byte_len = utf16.len() * 2;

    if byte_len > 0xFFFF {
        return Err(TypeError::BufferTooSmall {
            needed: byte_len,
            available: 0xFFFF,
        });
    }

    buf.put_u16_le(byte_len as u16);
    for code_unit in utf16 {
        buf.put_u16_le(code_unit);
    }
    Ok(())
}

/// Encode a UTF-16 string using PLP (Partially Length-Prefixed) format.
///
/// PLP format (per MS-TDS specification):
/// - 8 bytes: total length in bytes (little-endian)
/// - Chunks: 4-byte chunk length + data, repeated
/// - Terminator: 4 bytes of zero
///
/// For simplicity, we send the entire value as a single chunk.
/// This is efficient for bulk operations where we already have the complete data.
fn encode_plp_string(utf16: &[u16], buf: &mut BytesMut) {
    let byte_len = utf16.len() * 2;

    // Total length (8 bytes)
    buf.put_u64_le(byte_len as u64);

    if byte_len > 0 {
        // Single chunk: length (4 bytes) + data
        buf.put_u32_le(byte_len as u32);
        for code_unit in utf16 {
            buf.put_u16_le(*code_unit);
        }
    }

    // Terminator chunk (length = 0)
    buf.put_u32_le(0);
}

/// Encode binary data using PLP (Partially Length-Prefixed) format.
///
/// PLP format (per MS-TDS specification):
/// - 8 bytes: total length in bytes (little-endian)
/// - Chunks: 4-byte chunk length + data, repeated
/// - Terminator: 4 bytes of zero
///
/// For simplicity, we send the entire value as a single chunk.
fn encode_plp_binary(data: &[u8], buf: &mut BytesMut) {
    // Total length (8 bytes)
    buf.put_u64_le(data.len() as u64);

    if !data.is_empty() {
        // Single chunk: length (4 bytes) + data
        buf.put_u32_le(data.len() as u32);
        buf.put_slice(data);
    }

    // Terminator chunk (length = 0)
    buf.put_u32_le(0);
}

/// Encode time with specific scale (for bulk copy).
#[cfg(feature = "chrono")]
fn encode_time_with_scale(time: chrono::NaiveTime, scale: u8, buf: &mut BytesMut) {
    use chrono::Timelike;

    let nanos = time.num_seconds_from_midnight() as u64 * 1_000_000_000 + time.nanosecond() as u64;
    let intervals = nanos / time_scale_divisor(scale);
    let len = time_byte_length(scale);

    for i in 0..len {
        buf.put_u8(((intervals >> (i * 8)) & 0xFF) as u8);
    }
}

impl BulkInsert {
    /// Write the DONE token signaling completion.
    fn write_done(&mut self) {
        let buf = &mut self.buffer;

        buf.put_u8(TokenType::Done as u8);

        // Status: FINAL (0x00) | COUNT (0x10)
        let status = DoneStatus {
            more: false,
            error: false,
            in_xact: false,
            count: true,
            attn: false,
            srverror: false,
        };
        buf.put_u16_le(status.to_bits());

        // Current command (0 for bulk load)
        buf.put_u16_le(0);

        // Row count
        buf.put_u64_le(self.total_rows);
    }

    /// Get the buffered data as packets ready to send.
    ///
    /// Returns a vector of complete TDS packets with BulkLoad packet type (0x07).
    pub fn take_packets(&mut self) -> Vec<BytesMut> {
        const MAX_PACKET_SIZE: usize = 4096;
        const HEADER_SIZE: usize = 8;
        const MAX_PAYLOAD: usize = MAX_PACKET_SIZE - HEADER_SIZE;

        let data = self.buffer.split();
        let mut packets = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            let remaining = data.len() - offset;
            let payload_size = remaining.min(MAX_PAYLOAD);
            let is_last = offset + payload_size >= data.len();

            let mut packet = BytesMut::with_capacity(MAX_PACKET_SIZE);

            // Write packet header
            let header = PacketHeader {
                packet_type: PacketType::BulkLoad,
                status: if is_last {
                    PacketStatus::END_OF_MESSAGE
                } else {
                    PacketStatus::NORMAL
                },
                length: (HEADER_SIZE + payload_size) as u16,
                spid: 0,
                packet_id: self.packet_id,
                window: 0,
            };

            header.encode(&mut packet);

            // Write payload
            packet.put_slice(&data[offset..offset + payload_size]);

            packets.push(packet);
            offset += payload_size;
            self.packet_id = self.packet_id.wrapping_add(1);
        }

        packets
    }

    /// Get total rows sent so far.
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Get rows in current batch.
    pub fn rows_in_batch(&self) -> usize {
        self.rows_in_batch
    }

    /// Check if a batch flush is needed.
    pub fn should_flush(&self) -> bool {
        self.batch_size > 0 && self.rows_in_batch >= self.batch_size
    }

    /// Prepare for finishing the bulk operation.
    /// Writes the DONE token and returns final packets.
    pub fn finish_packets(&mut self) -> Vec<BytesMut> {
        self.write_done();
        self.take_packets()
    }

    /// Create a result from the current state.
    pub fn result(&self) -> BulkInsertResult {
        BulkInsertResult {
            rows_affected: self.total_rows,
            batches_committed: self.batches_committed,
            has_errors: false,
        }
    }
}

/// Calculate byte length for decimal based on precision.
fn decimal_byte_length(precision: u8) -> u8 {
    match precision {
        1..=9 => 5,
        10..=19 => 9,
        20..=28 => 13,
        29..=38 => 17,
        _ => 17, // Max precision
    }
}

/// Calculate byte length for time based on scale.
#[cfg(feature = "chrono")]
fn time_byte_length(scale: u8) -> u8 {
    match scale {
        0..=2 => 3,
        3..=4 => 4,
        5..=7 => 5,
        _ => 5,
    }
}

/// Get the divisor for time scale.
#[cfg(feature = "chrono")]
fn time_scale_divisor(scale: u8) -> u64 {
    match scale {
        0 => 1_000_000_000,
        1 => 100_000_000,
        2 => 10_000_000,
        3 => 1_000_000,
        4 => 100_000,
        5 => 10_000,
        6 => 1_000,
        7 => 100,
        _ => 100,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_bulk_options_default() {
        let opts = BulkOptions::default();
        assert_eq!(opts.batch_size, 0);
        assert!(opts.check_constraints);
        assert!(!opts.fire_triggers);
        assert!(opts.keep_nulls);
        assert!(!opts.table_lock);
    }

    #[test]
    fn test_bulk_column_creation() {
        let col = BulkColumn::new("id", "INT", 0);
        assert_eq!(col.name, "id");
        assert_eq!(col.type_id, 0x38);
        assert!(col.nullable);
    }

    #[test]
    fn test_parse_sql_type() {
        let (type_id, len, _prec, _scale) = parse_sql_type("INT");
        assert_eq!(type_id, 0x38);
        assert!(len.is_none());

        let (type_id, len, _, _) = parse_sql_type("NVARCHAR(100)");
        assert_eq!(type_id, 0xE7);
        assert_eq!(len, Some(200)); // UTF-16 doubles

        let (type_id, _, prec, scale) = parse_sql_type("DECIMAL(10,2)");
        assert_eq!(type_id, 0x6C);
        assert_eq!(prec, Some(10));
        assert_eq!(scale, Some(2));
    }

    #[test]
    fn test_insert_bulk_statement() {
        let builder = BulkInsertBuilder::new("dbo.Users")
            .with_typed_columns(vec![
                BulkColumn::new("id", "INT", 0),
                BulkColumn::new("name", "NVARCHAR(100)", 1),
            ])
            .table_lock(true);

        let sql = builder.build_insert_bulk_statement();
        assert!(sql.contains("INSERT BULK dbo.Users"));
        assert!(sql.contains("TABLOCK"));
    }

    #[test]
    fn test_bulk_insert_creation() {
        let columns = vec![
            BulkColumn::new("id", "INT", 0),
            BulkColumn::new("name", "NVARCHAR(100)", 1),
        ];

        let bulk = BulkInsert::new(columns, 1000);
        assert_eq!(bulk.total_rows(), 0);
        assert_eq!(bulk.rows_in_batch(), 0);
        assert!(!bulk.should_flush());
    }

    #[test]
    fn test_decimal_byte_length() {
        assert_eq!(decimal_byte_length(5), 5);
        assert_eq!(decimal_byte_length(15), 9);
        assert_eq!(decimal_byte_length(25), 13);
        assert_eq!(decimal_byte_length(35), 17);
    }

    #[test]
    #[cfg(feature = "chrono")]
    fn test_time_byte_length() {
        assert_eq!(time_byte_length(0), 3);
        assert_eq!(time_byte_length(3), 4);
        assert_eq!(time_byte_length(7), 5);
    }

    #[test]
    fn test_plp_string_encoding() {
        let mut buf = BytesMut::new();
        let text = "Hello";
        let utf16: Vec<u16> = text.encode_utf16().collect();

        encode_plp_string(&utf16, &mut buf);

        // Verify structure:
        // - 8 bytes total length
        // - 4 bytes chunk length
        // - data (5 chars * 2 bytes = 10 bytes)
        // - 4 bytes terminator (0)
        assert_eq!(buf.len(), 8 + 4 + 10 + 4);

        // Check total length
        assert_eq!(&buf[0..8], &10u64.to_le_bytes());

        // Check chunk length
        assert_eq!(&buf[8..12], &10u32.to_le_bytes());

        // Check terminator
        assert_eq!(&buf[22..26], &0u32.to_le_bytes());
    }

    #[test]
    fn test_plp_binary_encoding() {
        let mut buf = BytesMut::new();
        let data = b"test binary data";

        encode_plp_binary(data, &mut buf);

        // Verify structure:
        // - 8 bytes total length
        // - 4 bytes chunk length
        // - data (16 bytes)
        // - 4 bytes terminator (0)
        assert_eq!(buf.len(), 8 + 4 + 16 + 4);

        // Check total length
        assert_eq!(&buf[0..8], &16u64.to_le_bytes());

        // Check chunk length
        assert_eq!(&buf[8..12], &16u32.to_le_bytes());

        // Check data
        assert_eq!(&buf[12..28], data);

        // Check terminator
        assert_eq!(&buf[28..32], &0u32.to_le_bytes());
    }

    #[test]
    fn test_plp_empty_string() {
        let mut buf = BytesMut::new();
        let utf16: Vec<u16> = "".encode_utf16().collect();

        encode_plp_string(&utf16, &mut buf);

        // Empty string: total length (8) + terminator (4)
        assert_eq!(buf.len(), 8 + 4);

        // Check total length is 0
        assert_eq!(&buf[0..8], &0u64.to_le_bytes());

        // Check terminator
        assert_eq!(&buf[8..12], &0u32.to_le_bytes());
    }

    #[test]
    fn test_plp_empty_binary() {
        let mut buf = BytesMut::new();

        encode_plp_binary(&[], &mut buf);

        // Empty binary: total length (8) + terminator (4)
        assert_eq!(buf.len(), 8 + 4);

        // Check total length is 0
        assert_eq!(&buf[0..8], &0u64.to_le_bytes());

        // Check terminator
        assert_eq!(&buf[8..12], &0u32.to_le_bytes());
    }

    #[test]
    fn test_parse_sql_type_max() {
        // Test NVARCHAR(MAX) parsing - uses 0xFFFF marker (not doubled for MAX)
        let (type_id, len, _, _) = parse_sql_type("NVARCHAR(MAX)");
        assert_eq!(type_id, 0xE7);
        assert_eq!(len, Some(0xFFFF)); // MAX marker is 0xFFFF

        // Test VARBINARY(MAX) parsing
        let (type_id, len, _, _) = parse_sql_type("VARBINARY(MAX)");
        assert_eq!(type_id, 0xA5);
        assert_eq!(len, Some(0xFFFF));

        // Test VARCHAR(MAX) parsing
        let (type_id, len, _, _) = parse_sql_type("VARCHAR(MAX)");
        assert_eq!(type_id, 0xA7);
        assert_eq!(len, Some(0xFFFF));

        // Verify normal NVARCHAR does double the length
        let (type_id, len, _, _) = parse_sql_type("NVARCHAR(100)");
        assert_eq!(type_id, 0xE7);
        assert_eq!(len, Some(200)); // 100 * 2 for UTF-16
    }
}
