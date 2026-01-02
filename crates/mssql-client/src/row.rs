//! Row representation for query results.
//!
//! This module implements the `Arc<Bytes>` pattern from ADR-004 for reduced-copy
//! row data access. The `Row` struct holds a shared reference to the raw packet
//! buffer, deferring allocation until explicitly requested.
//!
//! ## Access Patterns (per ADR-004)
//!
//! - `get_bytes()` - Returns borrowed slice into buffer (zero additional allocation)
//! - `get_str()` - Returns Cow - borrowed if valid UTF-8, owned if conversion needed
//! - `get_string()` - Allocates new String (explicit allocation)
//! - `get<T>()` - Type-converting accessor with allocation only if needed

use std::borrow::Cow;
use std::sync::Arc;

use bytes::Bytes;

use mssql_types::decode::{TypeInfo, decode_value};
use mssql_types::{FromSql, SqlValue, TypeError};

use crate::blob::BlobReader;

/// Column slice information pointing into the row buffer.
///
/// This is the internal representation that enables zero-copy access
/// to column data within the shared buffer.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSlice {
    /// Offset into the buffer where this column's data begins.
    pub offset: u32,
    /// Length of the column data in bytes.
    pub length: u32,
    /// Whether this column value is NULL.
    pub is_null: bool,
}

impl ColumnSlice {
    /// Create a new column slice.
    pub fn new(offset: u32, length: u32, is_null: bool) -> Self {
        Self {
            offset,
            length,
            is_null,
        }
    }

    /// Create a NULL column slice.
    pub fn null() -> Self {
        Self {
            offset: 0,
            length: 0,
            is_null: true,
        }
    }
}

/// Column metadata describing a result set column.
///
/// This struct is marked `#[non_exhaustive]` to allow adding new fields
/// in future versions without breaking semver compatibility. Use
/// [`Column::new()`] or builder methods to construct instances.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Column {
    /// Column name.
    pub name: String,
    /// Column index (0-based).
    pub index: usize,
    /// SQL type name (e.g., "INT", "NVARCHAR").
    pub type_name: String,
    /// Whether the column allows NULL values.
    pub nullable: bool,
    /// Maximum length for variable-length types.
    pub max_length: Option<u32>,
    /// Precision for numeric types.
    pub precision: Option<u8>,
    /// Scale for numeric types.
    pub scale: Option<u8>,
    /// Collation for string types (VARCHAR, CHAR, TEXT).
    ///
    /// Used for proper encoding/decoding of non-Unicode string data.
    /// When present, enables collation-aware decoding that correctly
    /// handles locale-specific ANSI encodings (e.g., Shift_JIS, GB18030).
    pub collation: Option<tds_protocol::Collation>,
}

impl Column {
    /// Create a new column with basic metadata.
    pub fn new(name: impl Into<String>, index: usize, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            index,
            type_name: type_name.into(),
            nullable: true,
            max_length: None,
            precision: None,
            scale: None,
            collation: None,
        }
    }

    /// Set whether the column is nullable.
    #[must_use]
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Set the maximum length.
    #[must_use]
    pub fn with_max_length(mut self, max_length: u32) -> Self {
        self.max_length = Some(max_length);
        self
    }

    /// Set precision and scale for numeric types.
    #[must_use]
    pub fn with_precision_scale(mut self, precision: u8, scale: u8) -> Self {
        self.precision = Some(precision);
        self.scale = Some(scale);
        self
    }

    /// Set the collation for string types.
    ///
    /// Used for proper encoding/decoding of non-Unicode string data (VARCHAR, CHAR, TEXT).
    #[must_use]
    pub fn with_collation(mut self, collation: tds_protocol::Collation) -> Self {
        self.collation = Some(collation);
        self
    }

    /// Get the encoding name for this column's collation.
    ///
    /// Returns the name of the character encoding used for this column's data,
    /// or "unknown" if the collation is not set or the encoding feature is disabled.
    ///
    /// # Examples
    ///
    /// - `"Shift_JIS"` - Japanese encoding (LCID 0x0411)
    /// - `"GB18030"` - Simplified Chinese (LCID 0x0804)
    /// - `"UTF-8"` - SQL Server 2019+ UTF-8 collation
    /// - `"windows-1252"` - Latin/Western European (LCID 0x0409)
    /// - `"unknown"` - No collation or unsupported encoding
    #[must_use]
    pub fn encoding_name(&self) -> &'static str {
        #[cfg(feature = "encoding")]
        if let Some(ref collation) = self.collation {
            return collation.encoding_name();
        }
        "unknown"
    }

    /// Check if this column uses UTF-8 encoding.
    ///
    /// Returns `true` if the column has a SQL Server 2019+ UTF-8 collation,
    /// which is indicated by bit 27 (0x0800_0000) being set in the LCID.
    #[must_use]
    pub fn is_utf8_collation(&self) -> bool {
        #[cfg(feature = "encoding")]
        if let Some(ref collation) = self.collation {
            return collation.is_utf8();
        }
        false
    }

    /// Convert column metadata to TDS TypeInfo for decoding.
    ///
    /// Maps type names to TDS type IDs and constructs appropriate TypeInfo.
    pub fn to_type_info(&self) -> TypeInfo {
        let type_id = type_name_to_id(&self.type_name);
        TypeInfo {
            type_id,
            length: self.max_length,
            scale: self.scale,
            precision: self.precision,
            collation: self.collation.map(|c| mssql_types::decode::Collation {
                lcid: c.lcid,
                flags: c.sort_id,
            }),
        }
    }
}

/// Map SQL type name to TDS type ID.
fn type_name_to_id(name: &str) -> u8 {
    match name.to_uppercase().as_str() {
        // Integer types
        "INT" | "INTEGER" => 0x38,
        "BIGINT" => 0x7F,
        "SMALLINT" => 0x34,
        "TINYINT" => 0x30,
        "BIT" => 0x32,

        // Floating point
        "FLOAT" => 0x3E,
        "REAL" => 0x3B,

        // Decimal/Numeric
        "DECIMAL" | "NUMERIC" => 0x6C,
        "MONEY" | "SMALLMONEY" => 0x6E,

        // String types
        "NVARCHAR" | "NCHAR" | "NTEXT" => 0xE7,
        "VARCHAR" | "CHAR" | "TEXT" => 0xA7,

        // Binary types
        "VARBINARY" | "BINARY" | "IMAGE" => 0xA5,

        // Date/Time types
        "DATE" => 0x28,
        "TIME" => 0x29,
        "DATETIME2" => 0x2A,
        "DATETIMEOFFSET" => 0x2B,
        "DATETIME" => 0x3D,
        "SMALLDATETIME" => 0x3F,

        // GUID
        "UNIQUEIDENTIFIER" => 0x24,

        // XML
        "XML" => 0xF1,

        // Nullable variants (INTNTYPE, etc.)
        _ if name.ends_with("N") => 0x26,

        // Default to binary for unknown types
        _ => 0xA5,
    }
}

/// Shared column metadata for a result set.
///
/// This is shared across all rows in the result set to avoid
/// duplicating metadata per row.
#[derive(Debug, Clone)]
pub struct ColMetaData {
    /// Column definitions.
    pub columns: Arc<[Column]>,
}

impl ColMetaData {
    /// Create new column metadata from a list of columns.
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns: columns.into(),
        }
    }

    /// Get the number of columns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Check if there are no columns.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Get a column by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// Find a column index by name (case-insensitive).
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }
}

/// A row from a query result.
///
/// Implements the `Arc<Bytes>` pattern from ADR-004 for reduced memory allocation.
/// The row holds a shared reference to the raw packet buffer and column slice
/// information, deferring parsing and allocation until values are accessed.
///
/// # Memory Model
///
/// ```text
/// Row {
///     buffer: Arc<Bytes> ──────────► [raw packet data...]
///     slices: Arc<[ColumnSlice]> ──► [{offset, length, is_null}, ...]
///     metadata: Arc<ColMetaData> ──► [Column definitions...]
/// }
/// ```
///
/// Multiple `Row` instances from the same result set share the `metadata`.
/// The `buffer` and `slices` are unique per row but use `Arc` for cheap cloning.
///
/// # Access Patterns
///
/// - **Zero-copy:** `get_bytes()`, `get_str()` (when UTF-8 valid)
/// - **Allocating:** `get_string()`, `get::<String>()`
/// - **Type-converting:** `get::<T>()` uses `FromSql` trait
#[derive(Clone)]
pub struct Row {
    /// Shared reference to raw packet body containing row data.
    buffer: Arc<Bytes>,
    /// Column offsets into buffer.
    slices: Arc<[ColumnSlice]>,
    /// Column metadata (shared across result set).
    metadata: Arc<ColMetaData>,
    /// Cached parsed values (lazily populated).
    /// This maintains backward compatibility with code expecting SqlValue access.
    values: Option<Arc<[SqlValue]>>,
}

impl Row {
    /// Create a new row with the `Arc<Bytes>` pattern.
    ///
    /// This is the primary constructor for the reduced-copy pattern.
    pub fn new(buffer: Arc<Bytes>, slices: Arc<[ColumnSlice]>, metadata: Arc<ColMetaData>) -> Self {
        Self {
            buffer,
            slices,
            metadata,
            values: None,
        }
    }

    /// Create a row from pre-parsed values (backward compatibility).
    ///
    /// This constructor supports existing code that works with `SqlValue` directly.
    /// It's less efficient than the buffer-based approach but maintains compatibility.
    #[allow(dead_code)]
    pub(crate) fn from_values(columns: Vec<Column>, values: Vec<SqlValue>) -> Self {
        let metadata = Arc::new(ColMetaData::new(columns));
        let slices: Arc<[ColumnSlice]> = values
            .iter()
            .enumerate()
            .map(|(i, v)| ColumnSlice::new(i as u32, 0, v.is_null()))
            .collect::<Vec<_>>()
            .into();

        Self {
            buffer: Arc::new(Bytes::new()),
            slices,
            metadata,
            values: Some(values.into()),
        }
    }

    // ========================================================================
    // Zero-Copy Access Methods (ADR-004)
    // ========================================================================

    /// Returns borrowed slice into buffer (zero additional allocation).
    ///
    /// This is the most efficient access method when you need raw bytes.
    #[must_use]
    pub fn get_bytes(&self, index: usize) -> Option<&[u8]> {
        let slice = self.slices.get(index)?;
        if slice.is_null {
            return None;
        }

        let start = slice.offset as usize;
        let end = start + slice.length as usize;

        if end <= self.buffer.len() {
            Some(&self.buffer[start..end])
        } else {
            None
        }
    }

    /// Returns Cow - borrowed if valid UTF-8, owned if conversion needed.
    ///
    /// For UTF-8 data, this returns a borrowed reference (zero allocation).
    /// For VARCHAR data with collation, uses collation-aware decoding.
    /// For UTF-16 data (NVARCHAR), decodes as UTF-16LE.
    ///
    /// # Collation-Aware Decoding
    ///
    /// When the `encoding` feature is enabled and the column has collation metadata,
    /// VARCHAR data is decoded using the appropriate character encoding based on the
    /// collation's LCID. This correctly handles:
    ///
    /// - Japanese (Shift_JIS/CP932)
    /// - Simplified Chinese (GB18030/CP936)
    /// - Traditional Chinese (Big5/CP950)
    /// - Korean (EUC-KR/CP949)
    /// - Windows code pages 874, 1250-1258
    /// - SQL Server 2019+ UTF-8 collations
    #[must_use]
    pub fn get_str(&self, index: usize) -> Option<Cow<'_, str>> {
        let bytes = self.get_bytes(index)?;

        // Try to interpret as UTF-8 first (zero allocation for ASCII/UTF-8 data)
        match std::str::from_utf8(bytes) {
            Ok(s) => Some(Cow::Borrowed(s)),
            Err(_) => {
                // Check if we have collation metadata for this column
                #[cfg(feature = "encoding")]
                if let Some(column) = self.metadata.get(index) {
                    if let Some(ref collation) = column.collation {
                        // Use collation-aware decoding for VARCHAR/CHAR types
                        if let Some(encoding) = collation.encoding() {
                            let (decoded, _, had_errors) = encoding.decode(bytes);
                            if had_errors {
                                tracing::warn!(
                                    column_name = %column.name,
                                    column_index = index,
                                    encoding = %encoding.name(),
                                    lcid = collation.lcid,
                                    byte_len = bytes.len(),
                                    "collation-aware decoding had errors, falling back to UTF-16LE"
                                );
                            } else {
                                return Some(Cow::Owned(decoded.into_owned()));
                            }
                        } else {
                            tracing::debug!(
                                column_name = %column.name,
                                column_index = index,
                                lcid = collation.lcid,
                                "no encoding found for LCID, falling back to UTF-16LE"
                            );
                        }
                    }
                }

                // Assume UTF-16LE (SQL Server NVARCHAR encoding)
                // This requires allocation for the conversion
                let utf16: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                String::from_utf16(&utf16).ok().map(Cow::Owned)
            }
        }
    }

    /// Allocates new String (explicit allocation).
    ///
    /// Use this when you need an owned String.
    #[must_use]
    pub fn get_string(&self, index: usize) -> Option<String> {
        self.get_str(index).map(|cow| cow.into_owned())
    }

    // ========================================================================
    // Streaming Access (LOB support)
    // ========================================================================

    /// Get a streaming reader for a binary/text column.
    ///
    /// Returns a [`BlobReader`] that implements [`tokio::io::AsyncRead`] for
    /// streaming access to large binary or text columns. This is useful for:
    ///
    /// - Streaming large data to files without fully loading into memory
    /// - Processing data in chunks with progress tracking
    /// - Copying data between I/O destinations efficiently
    ///
    /// # Supported Column Types
    ///
    /// - `VARBINARY`, `VARBINARY(MAX)`
    /// - `VARCHAR`, `VARCHAR(MAX)`
    /// - `NVARCHAR`, `NVARCHAR(MAX)`
    /// - `TEXT`, `NTEXT`, `IMAGE` (legacy types)
    /// - `XML`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use tokio::io::AsyncWriteExt;
    ///
    /// // Stream a large VARBINARY(MAX) column to a file
    /// let mut reader = row.get_stream(0)?;
    /// let mut file = tokio::fs::File::create("output.bin").await?;
    /// tokio::io::copy(&mut reader, &mut file).await?;
    /// ```
    ///
    /// # Returns
    ///
    /// - `Some(BlobReader)` if the column contains binary/text data
    /// - `None` if the column is NULL or the index is out of bounds
    #[must_use]
    pub fn get_stream(&self, index: usize) -> Option<BlobReader> {
        let slice = self.slices.get(index)?;
        if slice.is_null {
            return None;
        }

        let start = slice.offset as usize;
        let end = start + slice.length as usize;

        if end <= self.buffer.len() {
            // Use zero-copy slicing from Arc<Bytes>
            let data = self.buffer.slice(start..end);
            Some(BlobReader::from_bytes(data))
        } else {
            None
        }
    }

    /// Get a streaming reader for a binary/text column by name.
    ///
    /// See [`get_stream`](Self::get_stream) for details.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut reader = row.get_stream_by_name("document_content")?;
    /// // Process the blob stream...
    /// ```
    #[must_use]
    pub fn get_stream_by_name(&self, name: &str) -> Option<BlobReader> {
        let index = self.metadata.find_by_name(name)?;
        self.get_stream(index)
    }

    // ========================================================================
    // Type-Converting Access (FromSql trait)
    // ========================================================================

    /// Get a value by column index with type conversion.
    ///
    /// Uses the `FromSql` trait to convert the raw value to the requested type.
    pub fn get<T: FromSql>(&self, index: usize) -> Result<T, TypeError> {
        // If we have cached values, use them
        if let Some(ref values) = self.values {
            return values
                .get(index)
                .ok_or_else(|| TypeError::TypeMismatch {
                    expected: "valid column index",
                    actual: format!("index {index} out of bounds"),
                })
                .and_then(T::from_sql);
        }

        // Otherwise, parse on demand from the buffer
        let slice = self
            .slices
            .get(index)
            .ok_or_else(|| TypeError::TypeMismatch {
                expected: "valid column index",
                actual: format!("index {index} out of bounds"),
            })?;

        if slice.is_null {
            return Err(TypeError::UnexpectedNull);
        }

        // Parse via SqlValue then convert to target type
        // Note: parse_value uses zero-copy buffer slicing (Arc<Bytes>::slice)
        let value = self.parse_value(index, slice)?;
        T::from_sql(&value)
    }

    /// Get a value by column name with type conversion.
    pub fn get_by_name<T: FromSql>(&self, name: &str) -> Result<T, TypeError> {
        let index = self
            .metadata
            .find_by_name(name)
            .ok_or_else(|| TypeError::TypeMismatch {
                expected: "valid column name",
                actual: format!("column '{name}' not found"),
            })?;

        self.get(index)
    }

    /// Try to get a value by column index, returning None if NULL or not found.
    pub fn try_get<T: FromSql>(&self, index: usize) -> Option<T> {
        // If we have cached values, use them
        if let Some(ref values) = self.values {
            return values
                .get(index)
                .and_then(|v| T::from_sql_nullable(v).ok().flatten());
        }

        // Otherwise check the slice
        let slice = self.slices.get(index)?;
        if slice.is_null {
            return None;
        }

        self.get(index).ok()
    }

    /// Try to get a value by column name, returning None if NULL or not found.
    pub fn try_get_by_name<T: FromSql>(&self, name: &str) -> Option<T> {
        let index = self.metadata.find_by_name(name)?;
        self.try_get(index)
    }

    // ========================================================================
    // Raw Value Access (backward compatibility)
    // ========================================================================

    /// Get the raw SQL value by index.
    ///
    /// Note: This may allocate if values haven't been cached.
    #[must_use]
    pub fn get_raw(&self, index: usize) -> Option<SqlValue> {
        if let Some(ref values) = self.values {
            return values.get(index).cloned();
        }

        let slice = self.slices.get(index)?;
        self.parse_value(index, slice).ok()
    }

    /// Get the raw SQL value by column name.
    #[must_use]
    pub fn get_raw_by_name(&self, name: &str) -> Option<SqlValue> {
        let index = self.metadata.find_by_name(name)?;
        self.get_raw(index)
    }

    // ========================================================================
    // Metadata Access
    // ========================================================================

    /// Get the number of columns in the row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slices.len()
    }

    /// Check if the row is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slices.is_empty()
    }

    /// Get the column metadata.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.metadata.columns
    }

    /// Get the shared column metadata.
    #[must_use]
    pub fn metadata(&self) -> &Arc<ColMetaData> {
        &self.metadata
    }

    /// Check if a column value is NULL.
    #[must_use]
    pub fn is_null(&self, index: usize) -> bool {
        self.slices.get(index).map(|s| s.is_null).unwrap_or(true)
    }

    /// Check if a column value is NULL by name.
    #[must_use]
    pub fn is_null_by_name(&self, name: &str) -> bool {
        self.metadata
            .find_by_name(name)
            .map(|i| self.is_null(i))
            .unwrap_or(true)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Parse a value from the buffer at the given slice.
    ///
    /// Uses the mssql-types decode module for efficient binary parsing.
    /// Optimized to use zero-copy buffer slicing via Arc<Bytes>.
    fn parse_value(&self, index: usize, slice: &ColumnSlice) -> Result<SqlValue, TypeError> {
        if slice.is_null {
            return Ok(SqlValue::Null);
        }

        let column = self
            .metadata
            .get(index)
            .ok_or_else(|| TypeError::TypeMismatch {
                expected: "valid column metadata",
                actual: format!("no metadata for column {index}"),
            })?;

        // Calculate byte range for this column
        let start = slice.offset as usize;
        let end = start + slice.length as usize;

        // Validate range
        if end > self.buffer.len() {
            return Err(TypeError::TypeMismatch {
                expected: "valid byte range",
                actual: format!(
                    "range {}..{} exceeds buffer length {}",
                    start,
                    end,
                    self.buffer.len()
                ),
            });
        }

        // Convert column metadata to TypeInfo for the decode module
        let type_info = column.to_type_info();

        // Use zero-copy slice of the buffer instead of allocating
        // This avoids the overhead of Bytes::copy_from_slice
        let mut buf = self.buffer.slice(start..end);

        // Use the unified decode module for efficient parsing
        decode_value(&mut buf, &type_info)
    }
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Row")
            .field("columns", &self.metadata.columns.len())
            .field("buffer_size", &self.buffer.len())
            .field("has_cached_values", &self.values.is_some())
            .finish()
    }
}

/// Iterator over row values as SqlValue.
pub struct RowIter<'a> {
    row: &'a Row,
    index: usize,
}

impl Iterator for RowIter<'_> {
    type Item = SqlValue;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.row.len() {
            return None;
        }
        let value = self.row.get_raw(self.index);
        self.index += 1;
        value
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.row.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> IntoIterator for &'a Row {
    type Item = SqlValue;
    type IntoIter = RowIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        RowIter {
            row: self,
            index: 0,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_column_slice_null() {
        let slice = ColumnSlice::null();
        assert!(slice.is_null);
        assert_eq!(slice.offset, 0);
        assert_eq!(slice.length, 0);
    }

    #[test]
    fn test_column_metadata() {
        let col = Column::new("id", 0, "INT")
            .with_nullable(false)
            .with_precision_scale(10, 0);

        assert_eq!(col.name, "id");
        assert_eq!(col.index, 0);
        assert!(!col.nullable);
        assert_eq!(col.precision, Some(10));
    }

    #[test]
    fn test_col_metadata_find_by_name() {
        let meta = ColMetaData::new(vec![
            Column::new("id", 0, "INT"),
            Column::new("Name", 1, "NVARCHAR"),
        ]);

        assert_eq!(meta.find_by_name("id"), Some(0));
        assert_eq!(meta.find_by_name("ID"), Some(0)); // case-insensitive
        assert_eq!(meta.find_by_name("name"), Some(1));
        assert_eq!(meta.find_by_name("unknown"), None);
    }

    #[test]
    fn test_row_from_values_backward_compat() {
        let columns = vec![
            Column::new("id", 0, "INT"),
            Column::new("name", 1, "NVARCHAR"),
        ];
        let values = vec![SqlValue::Int(42), SqlValue::String("Alice".to_string())];

        let row = Row::from_values(columns, values);

        assert_eq!(row.len(), 2);
        assert_eq!(row.get::<i32>(0).unwrap(), 42);
        assert_eq!(row.get_by_name::<String>("name").unwrap(), "Alice");
    }

    #[test]
    fn test_row_is_null() {
        let columns = vec![
            Column::new("id", 0, "INT"),
            Column::new("nullable_col", 1, "NVARCHAR"),
        ];
        let values = vec![SqlValue::Int(1), SqlValue::Null];

        let row = Row::from_values(columns, values);

        assert!(!row.is_null(0));
        assert!(row.is_null(1));
        assert!(row.is_null(99)); // Out of bounds returns true
    }

    #[test]
    fn test_row_get_bytes_with_buffer() {
        let buffer = Arc::new(Bytes::from_static(b"Hello World"));
        let slices: Arc<[ColumnSlice]> = vec![
            ColumnSlice::new(0, 5, false), // "Hello"
            ColumnSlice::new(6, 5, false), // "World"
        ]
        .into();
        let meta = Arc::new(ColMetaData::new(vec![
            Column::new("greeting", 0, "VARCHAR"),
            Column::new("subject", 1, "VARCHAR"),
        ]));

        let row = Row::new(buffer, slices, meta);

        assert_eq!(row.get_bytes(0), Some(b"Hello".as_slice()));
        assert_eq!(row.get_bytes(1), Some(b"World".as_slice()));
    }

    #[test]
    fn test_row_get_str() {
        let buffer = Arc::new(Bytes::from_static(b"Test"));
        let slices: Arc<[ColumnSlice]> = vec![ColumnSlice::new(0, 4, false)].into();
        let meta = Arc::new(ColMetaData::new(vec![Column::new("val", 0, "VARCHAR")]));

        let row = Row::new(buffer, slices, meta);

        let s = row.get_str(0).unwrap();
        assert_eq!(s, "Test");
        // Should be borrowed for valid UTF-8
        assert!(matches!(s, Cow::Borrowed(_)));
    }

    #[test]
    fn test_row_metadata_access() {
        let columns = vec![Column::new("col1", 0, "INT")];
        let row = Row::from_values(columns, vec![SqlValue::Int(1)]);

        assert_eq!(row.columns().len(), 1);
        assert_eq!(row.columns()[0].name, "col1");
        assert_eq!(row.metadata().len(), 1);
    }

    #[test]
    fn test_row_get_stream() {
        let buffer = Arc::new(Bytes::from_static(b"Hello, World!"));
        let slices: Arc<[ColumnSlice]> = vec![
            ColumnSlice::new(0, 5, false), // "Hello"
            ColumnSlice::new(7, 5, false), // "World"
            ColumnSlice::null(),           // NULL column
        ]
        .into();
        let meta = Arc::new(ColMetaData::new(vec![
            Column::new("greeting", 0, "VARBINARY"),
            Column::new("subject", 1, "VARBINARY"),
            Column::new("nullable", 2, "VARBINARY"),
        ]));

        let row = Row::new(buffer, slices, meta);

        // Get stream for first column
        let reader = row.get_stream(0).unwrap();
        assert_eq!(reader.len(), Some(5));
        assert_eq!(reader.as_bytes().as_ref(), b"Hello");

        // Get stream for second column
        let reader = row.get_stream(1).unwrap();
        assert_eq!(reader.len(), Some(5));
        assert_eq!(reader.as_bytes().as_ref(), b"World");

        // NULL column returns None
        assert!(row.get_stream(2).is_none());

        // Out of bounds returns None
        assert!(row.get_stream(99).is_none());
    }

    #[test]
    fn test_row_get_stream_by_name() {
        let buffer = Arc::new(Bytes::from_static(b"Binary data here"));
        let slices: Arc<[ColumnSlice]> = vec![ColumnSlice::new(0, 11, false)].into();
        let meta = Arc::new(ColMetaData::new(vec![Column::new(
            "document",
            0,
            "VARBINARY",
        )]));

        let row = Row::new(buffer, slices, meta);

        // Get by name (case-insensitive)
        let reader = row.get_stream_by_name("document").unwrap();
        assert_eq!(reader.len(), Some(11));

        let reader = row.get_stream_by_name("DOCUMENT").unwrap();
        assert_eq!(reader.len(), Some(11));

        // Unknown column returns None
        assert!(row.get_stream_by_name("unknown").is_none());
    }
}
