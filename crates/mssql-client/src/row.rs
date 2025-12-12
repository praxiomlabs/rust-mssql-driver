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

use mssql_types::{FromSql, SqlValue, TypeError};

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
#[derive(Debug, Clone)]
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
    /// Create a new row with the Arc<Bytes> pattern.
    ///
    /// This is the primary constructor for the reduced-copy pattern.
    pub fn new(
        buffer: Arc<Bytes>,
        slices: Arc<[ColumnSlice]>,
        metadata: Arc<ColMetaData>,
    ) -> Self {
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
    /// For UTF-16 data (NVARCHAR), this allocates a new String.
    #[must_use]
    pub fn get_str(&self, index: usize) -> Option<Cow<'_, str>> {
        let bytes = self.get_bytes(index)?;

        // Try to interpret as UTF-8 first (zero allocation for ASCII/UTF-8 data)
        match std::str::from_utf8(bytes) {
            Ok(s) => Some(Cow::Borrowed(s)),
            Err(_) => {
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
        let slice = self.slices.get(index).ok_or_else(|| TypeError::TypeMismatch {
            expected: "valid column index",
            actual: format!("index {index} out of bounds"),
        })?;

        if slice.is_null {
            return Err(TypeError::UnexpectedNull);
        }

        // For now, convert to SqlValue then use FromSql
        // TODO: Direct parsing from bytes for better performance
        let value = self.parse_value(index, slice)?;
        T::from_sql(&value)
    }

    /// Get a value by column name with type conversion.
    pub fn get_by_name<T: FromSql>(&self, name: &str) -> Result<T, TypeError> {
        let index = self.metadata.find_by_name(name).ok_or_else(|| {
            TypeError::TypeMismatch {
                expected: "valid column name",
                actual: format!("column '{name}' not found"),
            }
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
        self.slices
            .get(index)
            .map(|s| s.is_null)
            .unwrap_or(true)
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
    fn parse_value(&self, index: usize, slice: &ColumnSlice) -> Result<SqlValue, TypeError> {
        if slice.is_null {
            return Ok(SqlValue::Null);
        }

        let column = self.metadata.get(index).ok_or_else(|| TypeError::TypeMismatch {
            expected: "valid column metadata",
            actual: format!("no metadata for column {index}"),
        })?;

        let bytes = self.get_bytes(index).ok_or_else(|| TypeError::TypeMismatch {
            expected: "valid byte slice",
            actual: "buffer access failed".to_string(),
        })?;

        // Parse based on type
        // This is a simplified implementation - full implementation would
        // use mssql_types::decode module
        match column.type_name.to_uppercase().as_str() {
            "INT" if bytes.len() >= 4 => {
                let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                Ok(SqlValue::Int(value))
            }
            "BIGINT" if bytes.len() >= 8 => {
                let value = i64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                Ok(SqlValue::BigInt(value))
            }
            "SMALLINT" if bytes.len() >= 2 => {
                let value = i16::from_le_bytes([bytes[0], bytes[1]]);
                Ok(SqlValue::SmallInt(value))
            }
            "TINYINT" if !bytes.is_empty() => Ok(SqlValue::TinyInt(bytes[0])),
            "BIT" if !bytes.is_empty() => Ok(SqlValue::Bool(bytes[0] != 0)),
            "NVARCHAR" | "NCHAR" | "NTEXT" => {
                // UTF-16LE to String
                let utf16: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                let s = String::from_utf16(&utf16).map_err(|_| TypeError::TypeMismatch {
                    expected: "valid UTF-16",
                    actual: "invalid UTF-16 data".to_string(),
                })?;
                Ok(SqlValue::String(s))
            }
            "VARCHAR" | "CHAR" | "TEXT" => {
                let s = String::from_utf8_lossy(bytes).into_owned();
                Ok(SqlValue::String(s))
            }
            _ => {
                // Default: return as binary
                Ok(SqlValue::Binary(Bytes::copy_from_slice(bytes)))
            }
        }
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

impl<'a> Iterator for RowIter<'a> {
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
            ColumnSlice::new(0, 5, false),  // "Hello"
            ColumnSlice::new(6, 5, false),  // "World"
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
}
