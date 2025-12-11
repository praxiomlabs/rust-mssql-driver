//! SQL value representation.

use bytes::Bytes;

/// A SQL value that can represent any SQL Server data type.
///
/// This enum provides a type-safe way to handle SQL values that may be
/// of various types, including NULL.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    /// NULL value.
    Null,
    /// Boolean value (BIT).
    Bool(bool),
    /// 8-bit unsigned integer (TINYINT).
    TinyInt(u8),
    /// 16-bit signed integer (SMALLINT).
    SmallInt(i16),
    /// 32-bit signed integer (INT).
    Int(i32),
    /// 64-bit signed integer (BIGINT).
    BigInt(i64),
    /// 32-bit floating point (REAL).
    Float(f32),
    /// 64-bit floating point (FLOAT).
    Double(f64),
    /// String value (CHAR, VARCHAR, NCHAR, NVARCHAR, TEXT, NTEXT).
    String(String),
    /// Binary value (BINARY, VARBINARY, IMAGE).
    Binary(Bytes),
    /// Decimal value (DECIMAL, NUMERIC, MONEY, SMALLMONEY).
    #[cfg(feature = "decimal")]
    Decimal(rust_decimal::Decimal),
    /// UUID value (UNIQUEIDENTIFIER).
    #[cfg(feature = "uuid")]
    Uuid(uuid::Uuid),
    /// Date value (DATE).
    #[cfg(feature = "chrono")]
    Date(chrono::NaiveDate),
    /// Time value (TIME).
    #[cfg(feature = "chrono")]
    Time(chrono::NaiveTime),
    /// DateTime value (DATETIME, DATETIME2, SMALLDATETIME).
    #[cfg(feature = "chrono")]
    DateTime(chrono::NaiveDateTime),
    /// DateTimeOffset value (DATETIMEOFFSET).
    #[cfg(feature = "chrono")]
    DateTimeOffset(chrono::DateTime<chrono::FixedOffset>),
    /// JSON value (JSON type in SQL Server 2016+).
    #[cfg(feature = "json")]
    Json(serde_json::Value),
    /// XML value (XML type).
    Xml(String),
}

impl SqlValue {
    /// Check if the value is NULL.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Get the value as a bool, if it is one.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Get the value as an i32, if it is one.
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(*v),
            Self::SmallInt(v) => Some(*v as i32),
            Self::TinyInt(v) => Some(*v as i32),
            _ => None,
        }
    }

    /// Get the value as an i64, if it is one.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::BigInt(v) => Some(*v),
            Self::Int(v) => Some(*v as i64),
            Self::SmallInt(v) => Some(*v as i64),
            Self::TinyInt(v) => Some(*v as i64),
            _ => None,
        }
    }

    /// Get the value as an f64, if it is one.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Double(v) => Some(*v),
            Self::Float(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Get the value as a string slice, if it is one.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            Self::Xml(v) => Some(v),
            _ => None,
        }
    }

    /// Get the value as bytes, if it is binary.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Binary(v) => Some(v),
            _ => None,
        }
    }

    /// Get the type name as a string.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "NULL",
            Self::Bool(_) => "BIT",
            Self::TinyInt(_) => "TINYINT",
            Self::SmallInt(_) => "SMALLINT",
            Self::Int(_) => "INT",
            Self::BigInt(_) => "BIGINT",
            Self::Float(_) => "REAL",
            Self::Double(_) => "FLOAT",
            Self::String(_) => "NVARCHAR",
            Self::Binary(_) => "VARBINARY",
            #[cfg(feature = "decimal")]
            Self::Decimal(_) => "DECIMAL",
            #[cfg(feature = "uuid")]
            Self::Uuid(_) => "UNIQUEIDENTIFIER",
            #[cfg(feature = "chrono")]
            Self::Date(_) => "DATE",
            #[cfg(feature = "chrono")]
            Self::Time(_) => "TIME",
            #[cfg(feature = "chrono")]
            Self::DateTime(_) => "DATETIME2",
            #[cfg(feature = "chrono")]
            Self::DateTimeOffset(_) => "DATETIMEOFFSET",
            #[cfg(feature = "json")]
            Self::Json(_) => "JSON",
            Self::Xml(_) => "XML",
        }
    }
}

impl Default for SqlValue {
    fn default() -> Self {
        Self::Null
    }
}

impl From<bool> for SqlValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for SqlValue {
    fn from(v: i32) -> Self {
        Self::Int(v)
    }
}

impl From<i64> for SqlValue {
    fn from(v: i64) -> Self {
        Self::BigInt(v)
    }
}

impl From<f32> for SqlValue {
    fn from(v: f32) -> Self {
        Self::Float(v)
    }
}

impl From<f64> for SqlValue {
    fn from(v: f64) -> Self {
        Self::Double(v)
    }
}

impl From<String> for SqlValue {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for SqlValue {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

impl<T> From<Option<T>> for SqlValue
where
    T: Into<SqlValue>,
{
    fn from(v: Option<T>) -> Self {
        match v {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

#[cfg(feature = "uuid")]
impl From<uuid::Uuid> for SqlValue {
    fn from(v: uuid::Uuid) -> Self {
        Self::Uuid(v)
    }
}

#[cfg(feature = "decimal")]
impl From<rust_decimal::Decimal> for SqlValue {
    fn from(v: rust_decimal::Decimal) -> Self {
        Self::Decimal(v)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDate> for SqlValue {
    fn from(v: chrono::NaiveDate) -> Self {
        Self::Date(v)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDateTime> for SqlValue {
    fn from(v: chrono::NaiveDateTime) -> Self {
        Self::DateTime(v)
    }
}

#[cfg(feature = "json")]
impl From<serde_json::Value> for SqlValue {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}
