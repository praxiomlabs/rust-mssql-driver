//! Table-Valued Parameter (TVP) data structures.
//!
//! This module provides the low-level data structures for TVP encoding.
//! These types are used by `SqlValue::Tvp` to carry TVP data through the
//! type system.
//!
//! ## Wire Format
//!
//! TVPs are encoded as type `0xF3` in the TDS protocol with this structure:
//!
//! ```text
//! TVP_TYPE_INFO = TVPTYPE TVP_TYPENAME TVP_COLMETADATA TVP_END_TOKEN *TVP_ROW TVP_END_TOKEN
//! ```
//!
//! See [MS-TDS 2.2.6.9] for the complete specification.
//!
//! [MS-TDS 2.2.6.9]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/c264db71-c1ec-4fe8-b5ef-19d54b1e6566

use crate::SqlValue;

/// Column type identifier for TVP columns.
///
/// This enum maps Rust/SQL types to their TDS type identifiers for encoding
/// within TVP column metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TvpColumnType {
    /// BIT type (boolean).
    Bit,
    /// TINYINT type (u8).
    TinyInt,
    /// SMALLINT type (i16).
    SmallInt,
    /// INT type (i32).
    Int,
    /// BIGINT type (i64).
    BigInt,
    /// REAL type (f32).
    Real,
    /// FLOAT type (f64).
    Float,
    /// DECIMAL/NUMERIC type with precision and scale.
    Decimal {
        /// Maximum number of digits.
        precision: u8,
        /// Number of digits after decimal point.
        scale: u8,
    },
    /// NVARCHAR type with max length in characters.
    NVarChar {
        /// Maximum length in characters. Use u16::MAX for MAX.
        max_length: u16,
    },
    /// VARCHAR type with max length in bytes.
    VarChar {
        /// Maximum length in bytes. Use u16::MAX for MAX.
        max_length: u16,
    },
    /// VARBINARY type with max length.
    VarBinary {
        /// Maximum length in bytes. Use u16::MAX for MAX.
        max_length: u16,
    },
    /// UNIQUEIDENTIFIER type (UUID).
    UniqueIdentifier,
    /// DATE type.
    Date,
    /// TIME type with scale.
    Time {
        /// Fractional seconds precision (0-7).
        scale: u8,
    },
    /// DATETIME2 type with scale.
    DateTime2 {
        /// Fractional seconds precision (0-7).
        scale: u8,
    },
    /// DATETIMEOFFSET type with scale.
    DateTimeOffset {
        /// Fractional seconds precision (0-7).
        scale: u8,
    },
    /// XML type.
    Xml,
}

impl TvpColumnType {
    /// Infer the TVP column type from an SQL type name string.
    ///
    /// This parses SQL type declarations like "INT", "NVARCHAR(100)", "DECIMAL(18,2)".
    #[must_use]
    pub fn from_sql_type(sql_type: &str) -> Option<Self> {
        let sql_type = sql_type.trim().to_uppercase();

        // Handle parameterized types
        if sql_type.starts_with("NVARCHAR") {
            let max_len = Self::parse_length(&sql_type).unwrap_or(4000);
            return Some(Self::NVarChar {
                max_length: max_len,
            });
        }
        if sql_type.starts_with("VARCHAR") {
            let max_len = Self::parse_length(&sql_type).unwrap_or(8000);
            return Some(Self::VarChar {
                max_length: max_len,
            });
        }
        if sql_type.starts_with("VARBINARY") {
            let max_len = Self::parse_length(&sql_type).unwrap_or(8000);
            return Some(Self::VarBinary {
                max_length: max_len,
            });
        }
        if sql_type.starts_with("DECIMAL") || sql_type.starts_with("NUMERIC") {
            let (precision, scale) = Self::parse_precision_scale(&sql_type).unwrap_or((18, 0));
            return Some(Self::Decimal { precision, scale });
        }
        if sql_type.starts_with("TIME") {
            let scale = Self::parse_scale(&sql_type).unwrap_or(7);
            return Some(Self::Time { scale });
        }
        if sql_type.starts_with("DATETIME2") {
            let scale = Self::parse_scale(&sql_type).unwrap_or(7);
            return Some(Self::DateTime2 { scale });
        }
        if sql_type.starts_with("DATETIMEOFFSET") {
            let scale = Self::parse_scale(&sql_type).unwrap_or(7);
            return Some(Self::DateTimeOffset { scale });
        }

        // Handle simple types
        match sql_type.as_str() {
            "BIT" => Some(Self::Bit),
            "TINYINT" => Some(Self::TinyInt),
            "SMALLINT" => Some(Self::SmallInt),
            "INT" | "INTEGER" => Some(Self::Int),
            "BIGINT" => Some(Self::BigInt),
            "REAL" => Some(Self::Real),
            "FLOAT" => Some(Self::Float),
            "UNIQUEIDENTIFIER" => Some(Self::UniqueIdentifier),
            "DATE" => Some(Self::Date),
            "XML" => Some(Self::Xml),
            _ => None,
        }
    }

    /// Parse length from types like "NVARCHAR(100)" or "NVARCHAR(MAX)".
    fn parse_length(sql_type: &str) -> Option<u16> {
        let start = sql_type.find('(')?;
        let end = sql_type.find(')')?;
        let inner = sql_type[start + 1..end].trim();

        if inner.eq_ignore_ascii_case("MAX") {
            Some(u16::MAX)
        } else {
            inner.parse().ok()
        }
    }

    /// Parse precision and scale from types like "DECIMAL(18,2)".
    fn parse_precision_scale(sql_type: &str) -> Option<(u8, u8)> {
        let start = sql_type.find('(')?;
        let end = sql_type.find(')')?;
        let inner = sql_type[start + 1..end].trim();

        if let Some(comma) = inner.find(',') {
            let precision = inner[..comma].trim().parse().ok()?;
            let scale = inner[comma + 1..].trim().parse().ok()?;
            Some((precision, scale))
        } else {
            let precision = inner.parse().ok()?;
            Some((precision, 0))
        }
    }

    /// Parse scale from types like "TIME(3)" or "DATETIME2(7)".
    fn parse_scale(sql_type: &str) -> Option<u8> {
        let start = sql_type.find('(')?;
        let end = sql_type.find(')')?;
        let inner = sql_type[start + 1..end].trim();
        inner.parse().ok()
    }

    /// Get the TDS type ID for this column type.
    #[must_use]
    pub const fn type_id(&self) -> u8 {
        match self {
            Self::Bit => 0x68,                   // BITNTYPE
            Self::TinyInt => 0x26,               // INTNTYPE (len 1)
            Self::SmallInt => 0x26,              // INTNTYPE (len 2)
            Self::Int => 0x26,                   // INTNTYPE (len 4)
            Self::BigInt => 0x26,                // INTNTYPE (len 8)
            Self::Real => 0x6D,                  // FLTNTYPE (len 4)
            Self::Float => 0x6D,                 // FLTNTYPE (len 8)
            Self::Decimal { .. } => 0x6C,        // DECIMALNTYPE
            Self::NVarChar { .. } => 0xE7,       // NVARCHARTYPE
            Self::VarChar { .. } => 0xA7,        // BIGVARCHARTYPE
            Self::VarBinary { .. } => 0xA5,      // BIGVARBINTYPE
            Self::UniqueIdentifier => 0x24,      // GUIDTYPE
            Self::Date => 0x28,                  // DATETYPE
            Self::Time { .. } => 0x29,           // TIMETYPE
            Self::DateTime2 { .. } => 0x2A,      // DATETIME2TYPE
            Self::DateTimeOffset { .. } => 0x2B, // DATETIMEOFFSETTYPE
            Self::Xml => 0xF1,                   // XMLTYPE
        }
    }

    /// Get the max length field for this column type.
    #[must_use]
    pub const fn max_length(&self) -> Option<u16> {
        match self {
            Self::Bit => Some(1),
            Self::TinyInt => Some(1),
            Self::SmallInt => Some(2),
            Self::Int => Some(4),
            Self::BigInt => Some(8),
            Self::Real => Some(4),
            Self::Float => Some(8),
            Self::Decimal { .. } => Some(17), // Max decimal size
            Self::NVarChar { max_length } => Some(if *max_length == u16::MAX {
                0xFFFF
            } else {
                *max_length * 2
            }),
            Self::VarChar { max_length } => Some(*max_length),
            Self::VarBinary { max_length } => Some(*max_length),
            Self::UniqueIdentifier => Some(16),
            Self::Date => None,
            Self::Time { .. } => None,
            Self::DateTime2 { .. } => None,
            Self::DateTimeOffset { .. } => None,
            Self::Xml => Some(0xFFFF), // MAX
        }
    }
}

/// Column definition for a table-valued parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct TvpColumnDef {
    /// The column type.
    pub column_type: TvpColumnType,
    /// Whether the column is nullable.
    pub nullable: bool,
}

impl TvpColumnDef {
    /// Create a new non-nullable column definition.
    #[must_use]
    pub const fn new(column_type: TvpColumnType) -> Self {
        Self {
            column_type,
            nullable: false,
        }
    }

    /// Create a new nullable column definition.
    #[must_use]
    pub const fn nullable(column_type: TvpColumnType) -> Self {
        Self {
            column_type,
            nullable: true,
        }
    }

    /// Create from an SQL type string (e.g., "INT", "NVARCHAR(100)").
    ///
    /// Returns `None` if the SQL type is not recognized.
    #[must_use]
    pub fn from_sql_type(sql_type: &str) -> Option<Self> {
        TvpColumnType::from_sql_type(sql_type).map(Self::new)
    }
}

/// Raw table-valued parameter data for encoding.
///
/// This structure holds all the information needed to encode a TVP
/// in the TDS wire format.
#[derive(Debug, Clone, PartialEq)]
pub struct TvpData {
    /// The database schema (e.g., "dbo"). Empty for default schema.
    pub schema: String,
    /// The TVP type name as defined in the database.
    pub type_name: String,
    /// Column definitions.
    pub columns: Vec<TvpColumnDef>,
    /// Row data - each row is a Vec of SqlValues matching the columns.
    pub rows: Vec<Vec<SqlValue>>,
}

impl TvpData {
    /// Create a new empty TVP with the given schema and type name.
    #[must_use]
    pub fn new(schema: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
            type_name: type_name.into(),
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Add a column definition.
    #[must_use]
    pub fn with_column(mut self, column: TvpColumnDef) -> Self {
        self.columns.push(column);
        self
    }

    /// Add a row of values.
    ///
    /// # Panics
    ///
    /// Panics if the number of values doesn't match the number of columns.
    #[must_use]
    pub fn with_row(mut self, values: Vec<SqlValue>) -> Self {
        assert_eq!(
            values.len(),
            self.columns.len(),
            "Row value count ({}) must match column count ({})",
            values.len(),
            self.columns.len()
        );
        self.rows.push(values);
        self
    }

    /// Add a row of values without panicking.
    ///
    /// Returns `Err` if the number of values doesn't match the number of columns.
    pub fn try_add_row(&mut self, values: Vec<SqlValue>) -> Result<(), TvpError> {
        if values.len() != self.columns.len() {
            return Err(TvpError::ColumnCountMismatch {
                expected: self.columns.len(),
                actual: values.len(),
            });
        }
        self.rows.push(values);
        Ok(())
    }

    /// Get the number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if the TVP has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get the number of columns.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

/// Errors that can occur when working with TVPs.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum TvpError {
    /// Column count mismatch between definition and row data.
    #[error("column count mismatch: expected {expected}, got {actual}")]
    ColumnCountMismatch {
        /// Expected number of columns.
        expected: usize,
        /// Actual number of values in the row.
        actual: usize,
    },
    /// Unknown SQL type.
    #[error("unknown SQL type: {0}")]
    UnknownSqlType(String),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_column_type_from_sql_type() {
        assert!(matches!(
            TvpColumnType::from_sql_type("INT"),
            Some(TvpColumnType::Int)
        ));
        assert!(matches!(
            TvpColumnType::from_sql_type("BIGINT"),
            Some(TvpColumnType::BigInt)
        ));
        assert!(matches!(
            TvpColumnType::from_sql_type("nvarchar(100)"),
            Some(TvpColumnType::NVarChar { max_length: 100 })
        ));
        assert!(matches!(
            TvpColumnType::from_sql_type("NVARCHAR(MAX)"),
            Some(TvpColumnType::NVarChar { max_length: 65535 })
        ));
        assert!(matches!(
            TvpColumnType::from_sql_type("DECIMAL(18, 2)"),
            Some(TvpColumnType::Decimal {
                precision: 18,
                scale: 2
            })
        ));
        assert!(matches!(
            TvpColumnType::from_sql_type("datetime2(3)"),
            Some(TvpColumnType::DateTime2 { scale: 3 })
        ));
    }

    #[test]
    fn test_tvp_data_builder() {
        let tvp = TvpData::new("dbo", "UserIdList")
            .with_column(TvpColumnDef::new(TvpColumnType::Int))
            .with_row(vec![SqlValue::Int(1)])
            .with_row(vec![SqlValue::Int(2)])
            .with_row(vec![SqlValue::Int(3)]);

        assert_eq!(tvp.schema, "dbo");
        assert_eq!(tvp.type_name, "UserIdList");
        assert_eq!(tvp.column_count(), 1);
        assert_eq!(tvp.len(), 3);
    }

    #[test]
    #[should_panic(expected = "Row value count (2) must match column count (1)")]
    fn test_tvp_data_row_mismatch_panics() {
        let _ = TvpData::new("dbo", "Test")
            .with_column(TvpColumnDef::new(TvpColumnType::Int))
            .with_row(vec![SqlValue::Int(1), SqlValue::Int(2)]);
    }

    #[test]
    fn test_tvp_data_try_add_row_error() {
        let mut tvp =
            TvpData::new("dbo", "Test").with_column(TvpColumnDef::new(TvpColumnType::Int));

        let result = tvp.try_add_row(vec![SqlValue::Int(1), SqlValue::Int(2)]);
        assert!(matches!(result, Err(TvpError::ColumnCountMismatch { .. })));
    }
}
