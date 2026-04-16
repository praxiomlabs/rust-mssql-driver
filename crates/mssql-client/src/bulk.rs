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
//! use mssql_client::{BulkInsertBuilder, BulkColumn, BulkOptions};
//!
//! let builder = BulkInsertBuilder::new("dbo.Users")
//!     .with_typed_columns(vec![
//!         BulkColumn::new("id", "INT", 0)?,
//!         BulkColumn::new("name", "NVARCHAR(100)", 1)?,
//!         BulkColumn::new("email", "NVARCHAR(200)", 2)?,
//!     ])
//!     .with_options(BulkOptions {
//!         batch_size: 1000,
//!         check_constraints: true,
//!         fire_triggers: false,
//!         keep_nulls: true,
//!         table_lock: true,
//!         order_hint: None,
//!     });
//!
//! let mut writer = client.bulk_insert(&builder).await?;
//!
//! // Send rows — buffered in memory, sent on finish()
//! for user in users {
//!     writer.send_row(&[&user.id, &user.name, &user.email])?;
//! }
//!
//! let result = writer.finish().await?;
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
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;

use mssql_types::{SqlValue, ToSql, TypeError};
use tds_protocol::packet::{PacketHeader, PacketStatus, PacketType};
use tds_protocol::token::{Collation, DoneStatus, TokenType};

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
    /// Collation for VARCHAR/CHAR columns.
    ///
    /// Populated automatically from the server's COLMETADATA when
    /// [`Client::bulk_insert`](crate::Client::bulk_insert) is used. Can be set
    /// manually via [`with_collation`](Self::with_collation) for the
    /// schema-discovery-free path. When `None`, VARCHAR values fall back to
    /// the default Latin1_General_CI_AS collation (Windows-1252).
    collation: Option<Collation>,
}

impl BulkColumn {
    /// Create a new bulk column definition.
    ///
    /// # Errors
    ///
    /// Returns [`TypeError::UnsupportedType`] when `sql_type` names a deprecated
    /// large object type (`TEXT`, `NTEXT`, `IMAGE`). Use `VARCHAR(MAX)` /
    /// `NVARCHAR(MAX)` / `VARBINARY(MAX)` instead — Microsoft deprecated
    /// `TEXT` / `NTEXT` / `IMAGE` in SQL Server 2005 and recommends the `MAX`
    /// types for all new development.
    pub fn new<S: Into<String>>(name: S, sql_type: S, ordinal: usize) -> Result<Self, TypeError> {
        let sql_type_str: String = sql_type.into();
        reject_unsupported_bulk_type(&sql_type_str)?;
        let (type_id, max_length, precision, scale) = parse_sql_type(&sql_type_str);

        Ok(Self {
            name: name.into(),
            sql_type: sql_type_str,
            nullable: true,
            ordinal,
            type_id,
            max_length,
            precision,
            scale,
            collation: None,
        })
    }

    /// Set whether this column allows NULL values.
    #[must_use]
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Set the collation used for VARCHAR/CHAR columns.
    ///
    /// Required when [`Client::bulk_insert_without_schema_discovery`](crate::Client::bulk_insert_without_schema_discovery)
    /// targets VARCHAR columns on a server whose default collation is not
    /// Latin1_General_CI_AS and the target column uses a different code page.
    /// Ignored for NVARCHAR/NCHAR columns (always UTF-16).
    #[must_use]
    pub fn with_collation(mut self, collation: Collation) -> Self {
        self.collation = Some(collation);
        self
    }
}

/// Parse SQL type string into TDS type information.
///
/// Type parameters (e.g., the "100" in `VARCHAR(100)`) are parsed with
/// `.parse().ok()` — if a parameter is malformed it falls through to the
/// type's SQL Server default length (e.g., 8000 for VARCHAR, 4000 for
/// NVARCHAR). This is intentional: bulk-insert column definitions come
/// from user code, and defaulting to max length is safer than rejecting
/// the operation when the base type is valid.
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

    // This returns the nullable type variant ID. `write_colmetadata` switches
    // to the fixed-width variant (e.g. 0x26 INTN → 0x38 Int4) when the target
    // column is NOT NULL, since SQL Server's BulkLoad rejects nullable type IDs
    // for NOT NULL columns with error 4816.
    match base {
        "BIT" => (0x68, Some(1), None, None),           // BITN
        "TINYINT" => (0x26, Some(1), None, None),       // INTN(1)
        "SMALLINT" => (0x26, Some(2), None, None),      // INTN(2)
        "INT" => (0x26, Some(4), None, None),            // INTN(4)
        "BIGINT" => (0x26, Some(8), None, None),         // INTN(8)
        "REAL" => (0x6D, Some(4), None, None),           // FLTN(4)
        "FLOAT" => (0x6D, Some(8), None, None),          // FLTN(8)
        "DATE" => (0x28, None, None, None),
        "TIME" => {
            let scale = params.and_then(|p| p.parse().ok()).unwrap_or(7);
            (0x29, None, None, Some(scale))
        }
        "DATETIME" => (0x6F, Some(8), None, None),      // DATETIMEN(8)
        "DATETIME2" => {
            let scale = params.and_then(|p| p.parse().ok()).unwrap_or(7);
            (0x2A, None, None, Some(scale))
        }
        "DATETIMEOFFSET" => {
            let scale = params.and_then(|p| p.parse().ok()).unwrap_or(7);
            (0x2B, None, None, Some(scale))
        }
        "SMALLDATETIME" => (0x6F, Some(4), None, None), // DATETIMEN(4)
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
        "MONEY" => (0x6E, Some(8), None, None),         // MONEYN(8)
        "SMALLMONEY" => (0x6E, Some(4), None, None),    // MONEYN(4)
        "XML" => (0xF1, Some(0xFFFF), None, None),
        _ => (0xE7, Some(8000), None, None), // Default to NVARCHAR(4000)
    }
}

/// Reject deprecated large object types that this driver does not support in
/// bulk insert. `TEXT` / `NTEXT` / `IMAGE` have been deprecated since SQL
/// Server 2005 and use a legacy TEXTPTR wire format. Users should use
/// `VARCHAR(MAX)` / `NVARCHAR(MAX)` / `VARBINARY(MAX)` which the driver
/// supports end-to-end.
fn reject_unsupported_bulk_type(sql_type: &str) -> Result<(), TypeError> {
    let base = sql_type
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_uppercase();
    match base.as_str() {
        "TEXT" | "NTEXT" => Err(TypeError::UnsupportedType {
            sql_type: base,
            reason: "TEXT/NTEXT are not supported. Use VARCHAR(MAX) / \
                     NVARCHAR(MAX) instead (Microsoft deprecated TEXT/NTEXT in \
                     SQL Server 2005)."
                .to_string(),
        }),
        "IMAGE" => Err(TypeError::UnsupportedType {
            sql_type: base,
            reason: "IMAGE is not supported. Use VARBINARY(MAX) instead \
                     (Microsoft deprecated IMAGE in SQL Server 2005)."
                .to_string(),
        }),
        _ => Ok(()),
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
            .map(|(i, name)| {
                BulkColumn::new(*name, "NVARCHAR(MAX)", i)
                    .expect("NVARCHAR(MAX) is always a supported type")
            })
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
    ///
    /// # Errors
    ///
    /// Returns an error if the table name or any column name fails identifier
    /// validation, preventing SQL injection.
    pub fn build_insert_bulk_statement(&self) -> Result<String, Error> {
        // Validate table name (may be schema-qualified: dbo.Users, catalog.schema.table)
        crate::validation::validate_qualified_identifier(&self.table_name)?;

        // Validate column names
        for col in &self.columns {
            crate::validation::validate_identifier(&col.name)?;
        }

        let mut sql = format!("INSERT BULK {}", self.table_name);

        // Add column definitions
        if !self.columns.is_empty() {
            sql.push_str(" (");
            let cols: Vec<String> = self
                .columns
                .iter()
                .map(|c| {
                    // Validate sql_type to prevent SQL injection: only allow
                    // alphanumerics, parentheses (for length/precision), commas,
                    // spaces, and the MAX keyword — which covers all valid T-SQL
                    // type specifiers like "NVARCHAR(100)", "DECIMAL(18, 2)",
                    // "VARBINARY(MAX)", etc.
                    validate_sql_type(&c.sql_type)?;
                    Ok(format!("{} {}", c.name, c.sql_type))
                })
                .collect::<Result<Vec<_>, Error>>()?;
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
            // Validate order hint column names
            for col_name in order {
                crate::validation::validate_identifier(col_name)?;
            }
            hints.push(format!("ORDER({})", order.join(", ")));
        }

        if !hints.is_empty() {
            sql.push_str(" WITH (");
            sql.push_str(&hints.join(", "));
            sql.push(')');
        }

        Ok(sql)
    }
}

/// Validate a SQL type specifier to prevent SQL injection.
///
/// Allows only characters that can appear in valid T-SQL type declarations:
/// letters, digits, parentheses, commas, spaces, and periods.
/// Examples: "INT", "NVARCHAR(100)", "DECIMAL(18, 2)", "VARBINARY(MAX)".
fn validate_sql_type(type_str: &str) -> Result<(), Error> {
    #[allow(clippy::expect_used)] // Static regex compilation with known-valid pattern
    static SQL_TYPE_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9_ ()\.,]{0,127}$").expect("valid regex"));

    if type_str.is_empty() {
        return Err(Error::Config("SQL type cannot be empty".into()));
    }

    if !SQL_TYPE_RE.is_match(type_str) {
        return Err(Error::Config(format!(
            "invalid SQL type '{type_str}': contains disallowed characters"
        )));
    }

    Ok(())
}

/// Active bulk insert operation.
///
/// This struct manages the streaming of row data to the server.
/// Call `send_row()` for each row, then `finish()` to complete.
pub struct BulkInsert {
    /// Column metadata.
    columns: Arc<[BulkColumn]>,
    /// Whether each column uses a fixed-length type on the wire.
    /// When true, row values for that column are written without a length prefix.
    fixed_len: Arc<[bool]>,
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
        Self::new_with_server_metadata(columns, batch_size, None, None)
    }

    /// Create a new bulk insert operation using server metadata.
    ///
    /// When `raw_colmetadata` is provided, it is written directly into the
    /// BulkLoad buffer, ensuring the COLMETADATA matches the server's exact
    /// encoding. `server_columns` provides per-column type info so row values
    /// are encoded correctly (fixed-length types have no length prefix).
    ///
    /// This follows the pattern used by Tiberius: the server's own metadata
    /// from `SELECT TOP 0` is echoed back rather than constructing it from
    /// user-specified types.
    pub fn new_with_server_metadata(
        mut columns: Vec<BulkColumn>,
        batch_size: usize,
        raw_colmetadata: Option<bytes::Bytes>,
        server_columns: Option<&[tds_protocol::token::ColumnData]>,
    ) -> Self {
        // Determine which columns use fixed-length types on the wire.
        // Fixed-length types omit the per-row length prefix.
        let fixed_len: Vec<bool> = if let Some(srv_cols) = server_columns {
            // Propagate collation from server metadata for VARCHAR/CHAR columns.
            // The user's BulkColumn is constructed from type strings alone and
            // has no collation until we see the server's COLMETADATA — falling
            // back to the default Latin1 on NON-Latin servers would silently
            // corrupt extended characters.
            for (col, srv) in columns.iter_mut().zip(srv_cols.iter()) {
                if col.collation.is_none() {
                    col.collation = srv.type_info.collation;
                }
            }
            srv_cols.iter().map(|c| c.type_id.is_fixed_length()).collect()
        } else {
            // Without server metadata, NOT NULL columns of fixed-width types
            // must use the fixed type ID variant (e.g. INT NOT NULL uses 0x38
            // Int4, not 0x26 INTN). SQL Server rejects nullable type IDs for
            // NOT NULL target columns with error 4816.
            columns
                .iter()
                .map(|c| !c.nullable && nullable_to_fixed_type(c.type_id, c.max_length).is_some())
                .collect()
        };

        let mut bulk = Self {
            columns: columns.into(),
            fixed_len: fixed_len.into(),
            buffer: BytesMut::with_capacity(64 * 1024),
            rows_in_batch: 0,
            total_rows: 0,
            batch_size,
            batches_committed: 0,
            packet_id: 1,
        };

        if let Some(raw) = raw_colmetadata {
            bulk.buffer.extend_from_slice(&raw);
        } else {
            bulk.write_colmetadata();
        }

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

            // For NOT NULL columns with a fixed-width type, use the fixed type ID
            // variant (e.g. INT NOT NULL → 0x38 Int4 instead of 0x26 INTN).
            // SQL Server's BCP rejects nullable type IDs for NOT NULL columns.
            let effective_type_id = if !col.nullable {
                nullable_to_fixed_type(col.type_id, col.max_length).unwrap_or(col.type_id)
            } else {
                col.type_id
            };
            let is_fixed_variant = effective_type_id != col.type_id;

            // Flags: Nullable (bit 0) | Updateable (bit 3)
            // BulkLoad columns must have Updateable set to indicate they accept writes.
            let mut flags: u16 = 0x0008; // Updateable
            if col.nullable {
                flags |= 0x0001; // Nullable
            }
            buf.put_u16_le(flags);

            // Type info
            buf.put_u8(effective_type_id);

            // Fixed-width types have no additional TYPE_INFO bytes — skip straight
            // to the column name.
            if is_fixed_variant {
                let name_utf16: Vec<u16> = col.name.encode_utf16().collect();
                buf.put_u8(name_utf16.len() as u8);
                for code_unit in name_utf16 {
                    buf.put_u16_le(code_unit);
                }
                continue;
            }

            // Type-specific length/precision/scale
            match col.type_id {
                // Nullable fixed-length types — 1-byte max-length follows type ID
                // INTN(0x26), BITN(0x68), FLTN(0x6D), MONEYN(0x6E), DATETIMEN(0x6F)
                0x26 | 0x68 | 0x6D | 0x6E | 0x6F => {
                    buf.put_u8(col.max_length.unwrap_or(4) as u8);
                }

                // DATE has no length byte (fixed 3-byte value)
                0x28 => {}

                // Variable-length string/binary types
                0xE7 | 0xA7 | 0xA5 | 0xAD => {
                    // Max length (2 bytes for normal, 4 bytes for MAX)
                    let max_len = col.max_length.unwrap_or(8000);
                    if max_len == 0xFFFF {
                        buf.put_u16_le(0xFFFF);
                    } else {
                        buf.put_u16_le(max_len as u16);
                    }

                    // Collation for string types (5 bytes). Use the caller-
                    // supplied collation when present (via `with_collation()`),
                    // otherwise fall back to Latin1_General_CI_AS.
                    if col.type_id == 0xE7 || col.type_id == 0xA7 {
                        if let Some(coll) = col.collation.as_ref() {
                            buf.put_slice(&coll.to_bytes());
                        } else {
                            // Default collation: Latin1_General_CI_AS
                            // Bytes: LCID(0x0409) + flags(0xD000) + SortId(0x34)
                            buf.put_slice(&[0x09, 0x04, 0xD0, 0x00, 0x34]);
                        }
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
        let fixed_len = self.fixed_len.clone();

        // Write each column value
        for (i, (col, value)) in columns.iter().zip(values.iter()).enumerate() {
            let is_fixed = *fixed_len.get(i).unwrap_or(&false);
            self.encode_column_value(col, value, is_fixed)
                .map_err(|e| Error::Config(format!("failed to encode column {i}: {e}")))?;
        }

        Ok(())
    }

    /// Encode a column value according to its type.
    ///
    /// When `is_fixed` is true, the column uses a fixed-length type on the wire
    /// and values are written without a length prefix. When false, values include
    /// a length prefix (1 byte for numeric nullable types, 2 bytes for strings).
    fn encode_column_value(
        &mut self,
        col: &BulkColumn,
        value: &SqlValue,
        is_fixed: bool,
    ) -> Result<(), TypeError> {
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
                    // INTN, BITN, FLTN, MONEYN, DATETIMEN, Decimal, GUID, temporal
                    0x26 | 0x68 | 0x6D | 0x6E | 0x6F | 0x6C | 0x6A | 0x24 | 0x28
                    | 0x29 | 0x2A | 0x2B => {
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
                if !is_fixed { buf.put_u8(1); }
                buf.put_u8(if *v { 1 } else { 0 });
            }

            SqlValue::TinyInt(v) => {
                if !is_fixed { buf.put_u8(1); }
                buf.put_u8(*v);
            }

            SqlValue::SmallInt(v) => {
                if !is_fixed { buf.put_u8(2); }
                buf.put_i16_le(*v);
            }

            SqlValue::Int(v) => {
                if !is_fixed { buf.put_u8(4); }
                buf.put_i32_le(*v);
            }

            SqlValue::BigInt(v) => {
                if !is_fixed { buf.put_u8(8); }
                buf.put_i64_le(*v);
            }

            SqlValue::Float(v) => {
                if !is_fixed { buf.put_u8(4); }
                buf.put_f32_le(*v);
            }

            SqlValue::Double(v) => {
                if !is_fixed { buf.put_u8(8); }
                buf.put_f64_le(*v);
            }

            SqlValue::String(s) => {
                // NVARCHAR/NCHAR columns (0xE7/0xEF) use UTF-16LE on the wire.
                // VARCHAR/CHAR/BIGCHAR columns (0xA7/0x2F/0xAF) use the
                // collation's code page for single-byte encoding — writing UTF-16
                // into a VARCHAR column lands each surrogate half in its own
                // single-byte slot and silently corrupts the data.
                let is_varchar = matches!(col.type_id, 0xA7 | 0x2F | 0xAF);

                if is_varchar {
                    let encoded = encode_varchar_for_collation(s, col.collation.as_ref());
                    let byte_len = encoded.len();

                    if is_plp_type {
                        encode_plp_binary(&encoded, buf);
                    } else if byte_len > 0xFFFF {
                        return Err(TypeError::BufferTooSmall {
                            needed: byte_len,
                            available: 0xFFFF,
                        });
                    } else {
                        buf.put_u16_le(byte_len as u16);
                        buf.put_slice(&encoded);
                    }
                } else {
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
                if col.type_id == 0x6E {
                    // MONEY / SMALLMONEY — fixed-point scaled by 10_000, not DECIMAL format.
                    encode_money_value(*d, col, buf, is_fixed)?;
                } else {
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
                // Type 0x6F is DATETIMEN — legacy DATETIME (8 bytes) or
                // SMALLDATETIME (4 bytes) format selected by max_length. The
                // wire format differs from DATETIME2 (type 0x2A), which uses a
                // scale-aware time-then-date layout.
                if col.type_id == 0x6F {
                    let total_len = col.max_length.unwrap_or(8) as u8;
                    if !is_fixed {
                        buf.put_u8(total_len);
                    }
                    match total_len {
                        8 => mssql_types::encode::encode_datetime_legacy(*dt, buf),
                        4 => mssql_types::encode::encode_smalldatetime(*dt, buf)?,
                        _ => {
                            return Err(TypeError::InvalidDateTime(format!(
                                "DATETIMEN max_length must be 4 or 8, got {total_len}"
                            )));
                        }
                    }
                } else {
                    let scale = col.scale.unwrap_or(7);
                    let time_len = time_byte_length(scale);
                    let total_len = time_len + 3;
                    buf.put_u8(total_len);
                    // Encode time then date
                    encode_time_with_scale(dt.time(), scale, buf);
                    mssql_types::encode::encode_date(dt.date(), buf);
                }
            }
            #[cfg(feature = "chrono")]
            SqlValue::SmallDateTime(dt) => {
                // Explicit SMALLDATETIME variant — always 4-byte days+minutes,
                // regardless of column metadata.
                if !is_fixed {
                    buf.put_u8(4);
                }
                mssql_types::encode::encode_smalldatetime(*dt, buf)?;
            }
            #[cfg(feature = "decimal")]
            SqlValue::Money(d) => {
                // Force 8-byte MONEY encoding regardless of column metadata.
                if !is_fixed {
                    buf.put_u8(8);
                }
                mssql_types::encode::encode_money(*d, buf)?;
            }
            #[cfg(feature = "decimal")]
            SqlValue::SmallMoney(d) => {
                if !is_fixed {
                    buf.put_u8(4);
                }
                mssql_types::encode::encode_smallmoney(*d, buf)?;
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

/// Encode a MONEY or SMALLMONEY column value with the appropriate length prefix.
///
/// When `is_fixed` is true (fixed type ID 0x3C or 0x7A), no length byte
/// precedes the payload. Otherwise a 1-byte length prefix is written
/// (matching the MONEYN nullable variant).
#[cfg(feature = "decimal")]
fn encode_money_value(
    value: rust_decimal::Decimal,
    col: &BulkColumn,
    buf: &mut BytesMut,
    is_fixed: bool,
) -> Result<(), TypeError> {
    let money_bytes: u8 = col.max_length.unwrap_or(8) as u8;
    if !is_fixed {
        buf.put_u8(money_bytes);
    }
    match money_bytes {
        4 => mssql_types::encode::encode_smallmoney(value, buf),
        8 => mssql_types::encode::encode_money(value, buf),
        _ => Err(TypeError::InvalidDecimal(format!(
            "MONEY column has invalid max_length: {money_bytes}"
        ))),
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

/// PLP marker for an unknown total length (MS-TDS 2.2.5.2.3).
/// When the client doesn't know or doesn't wish to compute the total in advance,
/// the 8-byte ULONGLONGLEN is set to this value and the server relies on chunk
/// framing + the 4-byte terminator to detect the end.
const PLP_UNKNOWN_LEN: u64 = 0xFFFFFFFFFFFFFFFE;

/// Encode a UTF-16 string using PLP (Partially Length-Prefixed) format.
///
/// PLP format (per MS-TDS 2.2.5.2.3):
/// - 8 bytes: ULONGLONGLEN — PLP_UNKNOWN_LEN or actual total byte count
/// - One or more chunks: 4-byte chunk length + chunk bytes
/// - Terminator: 4-byte zero
///
/// We emit `PLP_UNKNOWN_LEN` for compatibility with SQL Server's BulkLoad
/// parser. Empirically, some server versions reject a concrete total length
/// in the BulkLoad (0x07) path even though the token-stream spec allows it
/// ("premature end-of-message" errors for NVARCHAR(MAX) bulk inserts).
/// Tiberius uses the same approach.
fn encode_plp_string(utf16: &[u16], buf: &mut BytesMut) {
    let byte_len = utf16.len() * 2;

    buf.put_u64_le(PLP_UNKNOWN_LEN);

    if byte_len > 0 {
        buf.put_u32_le(byte_len as u32);
        for code_unit in utf16 {
            buf.put_u16_le(*code_unit);
        }
    }

    buf.put_u32_le(0);
}

/// Encode binary data using PLP (Partially Length-Prefixed) format.
/// See [`encode_plp_string`] for the format specification.
fn encode_plp_binary(data: &[u8], buf: &mut BytesMut) {
    buf.put_u64_le(PLP_UNKNOWN_LEN);

    if !data.is_empty() {
        buf.put_u32_le(data.len() as u32);
        buf.put_slice(data);
    }

    buf.put_u32_le(0);
}

/// Encode a Rust string into single-byte VARCHAR bytes using the column's collation.
///
/// Delegates to [`tds_protocol::collation::encode_str_for_collation`] so the
/// RPC parameter path and the bulk insert path share one implementation.
fn encode_varchar_for_collation(value: &str, collation: Option<&Collation>) -> Vec<u8> {
    tds_protocol::collation::encode_str_for_collation(value, collation)
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
        let status = DoneStatus::from_bits(0x0010); // DONE_COUNT
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

/// Active streaming writer for bulk insert operations.
///
/// Created via [`crate::client::Client::bulk_insert()`]. Rows are buffered in
/// memory as they are added with [`send_row()`](BulkWriter::send_row), then
/// transmitted to the server when [`finish()`](BulkWriter::finish) is called.
///
/// The writer holds a mutable reference to the [`Client`], preventing other
/// operations on the connection while the bulk insert is in progress.
///
/// # Example
///
/// ```rust,ignore
/// let builder = BulkInsertBuilder::new("dbo.Users")
///     .with_typed_columns(vec![
///         BulkColumn::new("id", "INT", 0)?,
///         BulkColumn::new("name", "NVARCHAR(100)", 1)?,
///     ]);
///
/// let mut writer = client.bulk_insert(&builder).await?;
/// writer.send_row(&[&1i32, &"Alice"])?;
/// writer.send_row(&[&2i32, &"Bob"])?;
/// let result = writer.finish().await?;
/// ```
pub struct BulkWriter<'a, S: crate::state::ConnectionState> {
    client: &'a mut crate::client::Client<S>,
    bulk: BulkInsert,
}

impl<'a, S: crate::state::ConnectionState> BulkWriter<'a, S> {
    /// Create a new bulk writer.
    pub(crate) fn new(client: &'a mut crate::client::Client<S>, bulk: BulkInsert) -> Self {
        Self { client, bulk }
    }

    /// Add a row to the bulk insert buffer.
    ///
    /// Values are encoded immediately but not sent to the server until
    /// [`finish()`](BulkWriter::finish) is called. The number of values must
    /// match the number of columns defined for this bulk insert.
    pub fn send_row<T: ToSql>(&mut self, values: &[T]) -> Result<(), Error> {
        self.bulk.send_row(values)
    }

    /// Add a row of pre-converted SQL values to the buffer.
    pub fn send_row_values(&mut self, values: &[SqlValue]) -> Result<(), Error> {
        self.bulk.send_row_values(values)
    }

    /// Get the number of rows buffered so far.
    pub fn total_rows(&self) -> u64 {
        self.bulk.total_rows()
    }

    /// Finish the bulk insert operation and send all buffered data to the server.
    ///
    /// Writes the DONE token, sends the accumulated row data as a BulkLoad
    /// (0x07) message, and reads the server's response.
    pub async fn finish(mut self) -> Result<BulkInsertResult, Error> {
        let total_rows = self.bulk.total_rows();
        tracing::debug!(total_rows = total_rows, "finishing bulk insert");

        // Write DONE token and freeze the payload
        self.bulk.write_done();
        let payload = self.bulk.buffer.split().freeze();

        // Send BulkLoad data and read server response
        let rows_affected = self.client.send_and_read_bulk_load(payload).await?;

        Ok(BulkInsertResult {
            rows_affected,
            batches_committed: 1,
            has_errors: false,
        })
    }
}

/// Map a nullable type ID to its fixed-width counterpart.
///
/// SQL Server's BulkLoad protocol rejects nullable type IDs (INTN, BITN, etc.)
/// when the target column is NOT NULL. For those columns, the fixed type ID
/// variant must be sent instead — with no max_length and no per-row length
/// prefix.
///
/// Returns `None` for types that have no fixed-width variant (e.g. NVARCHAR,
/// VARBINARY, DECIMAL, temporal types other than DATETIME/SMALLDATETIME).
fn nullable_to_fixed_type(type_id: u8, max_length: Option<u32>) -> Option<u8> {
    match (type_id, max_length) {
        (0x68, _) => Some(0x32),           // BITN → Bit
        (0x26, Some(1)) => Some(0x30),      // INTN(1) → Int1 (TINYINT)
        (0x26, Some(2)) => Some(0x34),      // INTN(2) → Int2 (SMALLINT)
        (0x26, Some(4)) => Some(0x38),      // INTN(4) → Int4 (INT)
        (0x26, Some(8)) => Some(0x7F),      // INTN(8) → Int8 (BIGINT)
        (0x6D, Some(4)) => Some(0x3B),      // FLTN(4) → Float4 (REAL)
        (0x6D, Some(8)) => Some(0x3E),      // FLTN(8) → Float8 (FLOAT)
        (0x6E, Some(4)) => Some(0x7A),      // MONEYN(4) → Money4 (SMALLMONEY)
        (0x6E, Some(8)) => Some(0x3C),      // MONEYN(8) → Money (MONEY)
        (0x6F, Some(4)) => Some(0x3A),      // DATETIMEN(4) → DateTime4 (SMALLDATETIME)
        (0x6F, Some(8)) => Some(0x3D),      // DATETIMEN(8) → DateTime (DATETIME)
        _ => None,
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
        let col = BulkColumn::new("id", "INT", 0).unwrap();
        assert_eq!(col.name, "id");
        assert_eq!(col.type_id, 0x26); // INTN
        assert_eq!(col.max_length, Some(4));
        assert!(col.nullable);
    }

    #[test]
    fn test_bulk_column_rejects_text() {
        let err = BulkColumn::new("body", "TEXT", 0).unwrap_err();
        match err {
            TypeError::UnsupportedType { sql_type, reason } => {
                assert_eq!(sql_type, "TEXT");
                assert!(
                    reason.contains("VARCHAR(MAX)"),
                    "error should redirect to VARCHAR(MAX), got: {reason}"
                );
                assert!(
                    reason.contains("deprecated"),
                    "error should mention deprecation, got: {reason}"
                );
            }
            other => panic!("expected UnsupportedType, got {other:?}"),
        }
    }

    #[test]
    fn test_bulk_column_rejects_ntext() {
        let err = BulkColumn::new("body", "NTEXT", 0).unwrap_err();
        match err {
            TypeError::UnsupportedType { sql_type, reason } => {
                assert_eq!(sql_type, "NTEXT");
                assert!(
                    reason.contains("NVARCHAR(MAX)"),
                    "error should redirect to NVARCHAR(MAX), got: {reason}"
                );
                assert!(
                    reason.contains("deprecated"),
                    "error should mention deprecation, got: {reason}"
                );
            }
            other => panic!("expected UnsupportedType, got {other:?}"),
        }
    }

    #[test]
    fn test_bulk_column_rejects_text_case_insensitive() {
        assert!(matches!(
            BulkColumn::new("body", "text", 0),
            Err(TypeError::UnsupportedType { .. })
        ));
        assert!(matches!(
            BulkColumn::new("body", "Ntext", 0),
            Err(TypeError::UnsupportedType { .. })
        ));
    }

    #[test]
    fn test_bulk_column_rejects_image() {
        let err = BulkColumn::new("blob", "IMAGE", 0).unwrap_err();
        match err {
            TypeError::UnsupportedType { sql_type, reason } => {
                assert_eq!(sql_type, "IMAGE");
                assert!(
                    reason.contains("VARBINARY(MAX)"),
                    "error should redirect to VARBINARY(MAX), got: {reason}"
                );
                assert!(
                    reason.contains("deprecated"),
                    "error should mention deprecation, got: {reason}"
                );
            }
            other => panic!("expected UnsupportedType, got {other:?}"),
        }
    }

    #[test]
    fn test_bulk_column_rejects_image_case_insensitive() {
        assert!(matches!(
            BulkColumn::new("blob", "image", 0),
            Err(TypeError::UnsupportedType { .. })
        ));
        assert!(matches!(
            BulkColumn::new("blob", "Image", 0),
            Err(TypeError::UnsupportedType { .. })
        ));
    }

    #[test]
    fn test_parse_sql_type() {
        // Integer types → INTN (0x26) with appropriate length
        let (type_id, len, _prec, _scale) = parse_sql_type("INT");
        assert_eq!(type_id, 0x26);
        assert_eq!(len, Some(4));

        let (type_id, len, _, _) = parse_sql_type("NVARCHAR(100)");
        assert_eq!(type_id, 0xE7);
        assert_eq!(len, Some(200)); // UTF-16 doubles

        let (type_id, _, prec, scale) = parse_sql_type("DECIMAL(10,2)");
        assert_eq!(type_id, 0x6C);
        assert_eq!(prec, Some(10));
        assert_eq!(scale, Some(2));

        // SMALLDATETIME/DATETIME → DATETIMEN (0x6F)
        let (type_id, len, _, _) = parse_sql_type("SMALLDATETIME");
        assert_eq!(type_id, 0x6F);
        assert_eq!(len, Some(4));

        let (type_id, len, _, _) = parse_sql_type("DATETIME");
        assert_eq!(type_id, 0x6F);
        assert_eq!(len, Some(8));
    }

    #[test]
    fn test_insert_bulk_statement() {
        let builder = BulkInsertBuilder::new("dbo.Users")
            .with_typed_columns(vec![
                BulkColumn::new("id", "INT", 0).unwrap(),
                BulkColumn::new("name", "NVARCHAR(100)", 1).unwrap(),
            ])
            .table_lock(true);

        let sql = builder.build_insert_bulk_statement().unwrap();
        assert!(sql.contains("INSERT BULK dbo.Users"));
        assert!(sql.contains("TABLOCK"));
    }

    #[test]
    fn test_bulk_insert_rejects_injection() {
        let builder = BulkInsertBuilder::new("table;DROP TABLE users")
            .with_typed_columns(vec![BulkColumn::new("id", "INT", 0).unwrap()]);

        assert!(builder.build_insert_bulk_statement().is_err());
    }

    #[test]
    fn test_bulk_insert_validates_column_names() {
        let builder = BulkInsertBuilder::new("Users").with_typed_columns(vec![BulkColumn::new(
            "col;DROP TABLE x",
            "INT",
            0,
        )
        .unwrap()]);

        assert!(builder.build_insert_bulk_statement().is_err());
    }

    #[test]
    fn test_bulk_insert_accepts_qualified_names() {
        let builder = BulkInsertBuilder::new("catalog.dbo.Users")
            .with_typed_columns(vec![BulkColumn::new("id", "INT", 0).unwrap()]);

        assert!(builder.build_insert_bulk_statement().is_ok());
    }

    #[test]
    fn test_bulk_insert_creation() {
        let columns = vec![
            BulkColumn::new("id", "INT", 0).unwrap(),
            BulkColumn::new("name", "NVARCHAR(100)", 1).unwrap(),
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
        // - 8 bytes PLP_UNKNOWN_LEN marker
        // - 4 bytes chunk length
        // - data (5 chars * 2 bytes = 10 bytes)
        // - 4 bytes terminator (0)
        assert_eq!(buf.len(), 8 + 4 + 10 + 4);

        // Check total length marker (PLP_UNKNOWN_LEN)
        assert_eq!(&buf[0..8], &PLP_UNKNOWN_LEN.to_le_bytes());

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
        // - 8 bytes PLP_UNKNOWN_LEN marker
        // - 4 bytes chunk length
        // - data (16 bytes)
        // - 4 bytes terminator (0)
        assert_eq!(buf.len(), 8 + 4 + 16 + 4);

        // Check total length marker
        assert_eq!(&buf[0..8], &PLP_UNKNOWN_LEN.to_le_bytes());

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

        // Empty string: PLP_UNKNOWN_LEN (8) + terminator (4)
        assert_eq!(buf.len(), 8 + 4);

        // Check total length marker
        assert_eq!(&buf[0..8], &PLP_UNKNOWN_LEN.to_le_bytes());

        // Check terminator
        assert_eq!(&buf[8..12], &0u32.to_le_bytes());
    }

    #[test]
    fn test_plp_empty_binary() {
        let mut buf = BytesMut::new();

        encode_plp_binary(&[], &mut buf);

        // Empty binary: PLP_UNKNOWN_LEN (8) + terminator (4)
        assert_eq!(buf.len(), 8 + 4);

        // Check total length marker
        assert_eq!(&buf[0..8], &PLP_UNKNOWN_LEN.to_le_bytes());

        // Check terminator
        assert_eq!(&buf[8..12], &0u32.to_le_bytes());
    }

    /// Verify that write_colmetadata() produces bytes that the TDS parser can
    /// decode correctly for all supported column types (nullable variants).
    #[test]
    fn test_write_colmetadata_roundtrip() {
        use tds_protocol::token::ColMetaData;

        let columns = vec![
            BulkColumn::new("id", "INT", 0).unwrap(),
            BulkColumn::new("tiny", "TINYINT", 1).unwrap(),
            BulkColumn::new("small", "SMALLINT", 2).unwrap(),
            BulkColumn::new("big", "BIGINT", 3).unwrap(),
            BulkColumn::new("flag", "BIT", 4).unwrap(),
            BulkColumn::new("r", "REAL", 5).unwrap(),
            BulkColumn::new("f", "FLOAT", 6).unwrap(),
            BulkColumn::new("name", "NVARCHAR(100)", 7).unwrap(),
            BulkColumn::new("code", "VARCHAR(50)", 8).unwrap(),
            BulkColumn::new("data", "VARBINARY(200)", 9).unwrap(),
            BulkColumn::new("d", "DATE", 10).unwrap(),
            BulkColumn::new("t", "TIME(3)", 11).unwrap(),
            BulkColumn::new("dt", "DATETIME", 12).unwrap(),
            BulkColumn::new("dt2", "DATETIME2(7)", 13).unwrap(),
            BulkColumn::new("dto", "DATETIMEOFFSET(7)", 14).unwrap(),
            BulkColumn::new("sdt", "SMALLDATETIME", 15).unwrap(),
            BulkColumn::new("uid", "UNIQUEIDENTIFIER", 16).unwrap(),
            BulkColumn::new("amt", "DECIMAL(18,2)", 17).unwrap(),
            BulkColumn::new("price", "MONEY", 18).unwrap(),
            BulkColumn::new("smoney", "SMALLMONEY", 19).unwrap(),
            BulkColumn::new("nmax", "NVARCHAR(MAX)", 20).unwrap(),
            BulkColumn::new("vmax", "VARCHAR(MAX)", 21).unwrap(),
            BulkColumn::new("bmax", "VARBINARY(MAX)", 22).unwrap(),
        ];

        let bulk = BulkInsert::new(columns.clone(), 0);

        // Extract COLMETADATA bytes (skip the 0x81 token type byte)
        let buf = &bulk.buffer[1..];
        let mut cursor = bytes::Bytes::copy_from_slice(buf);
        let meta = ColMetaData::decode(&mut cursor)
            .expect("write_colmetadata output should be parseable by TDS decoder");

        assert_eq!(meta.columns.len(), columns.len());

        // Verify each column parsed correctly
        for (i, (parsed, original)) in meta.columns.iter().zip(columns.iter()).enumerate() {
            assert_eq!(
                parsed.name, original.name,
                "column {i} name mismatch"
            );
            assert_eq!(
                parsed.col_type, original.type_id,
                "column {i} ({}) type mismatch",
                original.name
            );

            // Verify type-specific metadata
            match original.type_id {
                // INTN — max_length should match
                0x26 => {
                    assert_eq!(
                        parsed.type_info.max_length,
                        original.max_length,
                        "column {i} ({}) INTN max_length",
                        original.name
                    );
                }
                // BITN
                0x68 => {
                    assert_eq!(parsed.type_info.max_length, Some(1));
                }
                // FLTN
                0x6D => {
                    assert_eq!(
                        parsed.type_info.max_length,
                        original.max_length,
                        "column {i} ({}) FLTN max_length",
                        original.name
                    );
                }
                // MONEYN
                0x6E => {
                    assert_eq!(
                        parsed.type_info.max_length,
                        original.max_length,
                        "column {i} ({}) MONEYN max_length",
                        original.name
                    );
                }
                // DATETIMEN
                0x6F => {
                    assert_eq!(
                        parsed.type_info.max_length,
                        original.max_length,
                        "column {i} ({}) DATETIMEN max_length",
                        original.name
                    );
                }
                // GUID
                0x24 => {
                    assert_eq!(parsed.type_info.max_length, Some(16));
                }
                // DATE — no extra metadata
                0x28 => {}
                // TIME/DATETIME2/DATETIMEOFFSET — scale
                0x29..=0x2B => {
                    assert_eq!(
                        parsed.type_info.scale,
                        original.scale,
                        "column {i} ({}) scale",
                        original.name
                    );
                }
                // NVARCHAR/VARCHAR — max_length + collation
                0xE7 | 0xA7 => {
                    assert_eq!(
                        parsed.type_info.max_length,
                        original.max_length,
                        "column {i} ({}) string max_length",
                        original.name
                    );
                    assert!(
                        parsed.type_info.collation.is_some(),
                        "column {i} ({}) should have collation",
                        original.name
                    );
                }
                // VARBINARY — max_length, no collation
                0xA5 => {
                    assert_eq!(
                        parsed.type_info.max_length,
                        original.max_length,
                        "column {i} ({}) binary max_length",
                        original.name
                    );
                    assert!(
                        parsed.type_info.collation.is_none(),
                        "column {i} ({}) should not have collation",
                        original.name
                    );
                }
                // DECIMAL
                0x6C => {
                    assert_eq!(
                        parsed.type_info.precision,
                        original.precision,
                        "column {i} ({}) precision",
                        original.name
                    );
                    assert_eq!(
                        parsed.type_info.scale,
                        original.scale,
                        "column {i} ({}) scale",
                        original.name
                    );
                }
                _ => {}
            }
        }
    }

    /// Verify that NOT NULL columns use fixed-width type IDs (0x38 Int4,
    /// 0x32 Bit, etc.) rather than nullable type IDs (0x26 INTN, 0x68 BITN).
    /// SQL Server's BulkLoad rejects nullable IDs for NOT NULL columns.
    #[test]
    fn test_write_colmetadata_not_null_uses_fixed_types() {
        use tds_protocol::token::ColMetaData;
        use tds_protocol::types::TypeId;

        let columns = vec![
            BulkColumn::new("id", "INT", 0).unwrap().with_nullable(false),
            BulkColumn::new("tiny", "TINYINT", 1).unwrap().with_nullable(false),
            BulkColumn::new("small", "SMALLINT", 2).unwrap().with_nullable(false),
            BulkColumn::new("big", "BIGINT", 3).unwrap().with_nullable(false),
            BulkColumn::new("flag", "BIT", 4).unwrap().with_nullable(false),
            BulkColumn::new("r", "REAL", 5).unwrap().with_nullable(false),
            BulkColumn::new("f", "FLOAT", 6).unwrap().with_nullable(false),
            BulkColumn::new("dt", "DATETIME", 7).unwrap().with_nullable(false),
            BulkColumn::new("sdt", "SMALLDATETIME", 8).unwrap().with_nullable(false),
            BulkColumn::new("mny", "MONEY", 9).unwrap().with_nullable(false),
            BulkColumn::new("smny", "SMALLMONEY", 10).unwrap().with_nullable(false),
        ];

        let bulk = BulkInsert::new(columns.clone(), 0);

        // Every NOT NULL fixed-width column should have fixed_len=true
        for (i, fixed) in bulk.fixed_len.iter().enumerate() {
            assert!(*fixed, "column {i} ({}) should be fixed_len", columns[i].name);
        }

        // Parse the generated COLMETADATA
        let buf = &bulk.buffer[1..]; // skip token type byte
        let mut cursor = bytes::Bytes::copy_from_slice(buf);
        let meta = ColMetaData::decode(&mut cursor).expect("parseable");

        // Verify each column has the expected fixed type ID and no Nullable flag
        let expected: &[(&str, TypeId)] = &[
            ("id", TypeId::Int4),
            ("tiny", TypeId::Int1),
            ("small", TypeId::Int2),
            ("big", TypeId::Int8),
            ("flag", TypeId::Bit),
            ("r", TypeId::Float4),
            ("f", TypeId::Float8),
            ("dt", TypeId::DateTime),
            ("sdt", TypeId::DateTime4),
            ("mny", TypeId::Money),
            ("smny", TypeId::Money4),
        ];

        for (i, (name, ty)) in expected.iter().enumerate() {
            assert_eq!(meta.columns[i].name, *name, "column {i} name");
            assert_eq!(meta.columns[i].type_id, *ty, "column {i} ({name}) type");
            assert_eq!(
                meta.columns[i].flags & 0x0001,
                0,
                "column {i} ({name}) should not have Nullable flag set"
            );
        }
    }

    /// Verify that `with_collation()` on a VARCHAR column propagates into
    /// the COLMETADATA token — the hand-crafted path previously hardcoded
    /// Latin1_General_CI_AS regardless of the caller-supplied collation.
    #[test]
    fn test_write_colmetadata_uses_caller_collation() {
        use tds_protocol::token::{ColMetaData, Collation};

        // Chinese_PRC_CI_AS: LCID 0x0804, sort_id 0x52 (just a non-default pair)
        let chinese = Collation {
            lcid: 0x0804,
            sort_id: 0x52,
        };

        let columns = vec![
            BulkColumn::new("s", "VARCHAR(50)", 0).unwrap().with_collation(chinese.clone()),
            // NVARCHAR also writes 5 collation bytes — should honor caller too
            BulkColumn::new("n", "NVARCHAR(50)", 1).unwrap().with_collation(chinese.clone()),
            // VARCHAR without with_collation should keep the Latin1 default
            BulkColumn::new("d", "VARCHAR(10)", 2).unwrap(),
        ];
        let bulk = BulkInsert::new(columns, 0);

        let buf = &bulk.buffer[1..];
        let mut cursor = bytes::Bytes::copy_from_slice(buf);
        let meta = ColMetaData::decode(&mut cursor).expect("parseable");

        let c0 = meta.columns[0]
            .type_info
            .collation
            .as_ref()
            .expect("VARCHAR has collation");
        assert_eq!(c0.lcid, chinese.lcid, "VARCHAR caller LCID");
        assert_eq!(c0.sort_id, chinese.sort_id, "VARCHAR caller sort_id");

        let c1 = meta.columns[1]
            .type_info
            .collation
            .as_ref()
            .expect("NVARCHAR has collation");
        assert_eq!(c1.lcid, chinese.lcid, "NVARCHAR caller LCID");
        assert_eq!(c1.sort_id, chinese.sort_id, "NVARCHAR caller sort_id");

        // Default collation: Latin1_General_CI_AS wire bytes
        // [0x09, 0x04, 0xD0, 0x00, 0x34] → lcid u32 LE = 0x00D0_0409, sort_id = 0x34
        let default = meta.columns[2]
            .type_info
            .collation
            .as_ref()
            .expect("VARCHAR has default collation");
        assert_eq!(default.to_bytes(), [0x09, 0x04, 0xD0, 0x00, 0x34]);
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
