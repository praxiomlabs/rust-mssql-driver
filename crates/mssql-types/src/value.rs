//! SQL value representation.

use bytes::Bytes;

use crate::tvp::TvpData;

/// A SQL value that can represent any SQL Server data type.
///
/// This enum provides a type-safe way to handle SQL values that may be
/// of various types, including NULL.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
    /// Decimal value (DECIMAL, NUMERIC).
    #[cfg(feature = "decimal")]
    Decimal(rust_decimal::Decimal),
    /// Money value (MONEY — fixed-point scaled by 10_000, signed 64-bit range).
    ///
    /// Distinct from [`Self::Decimal`] so that RPC parameter encoding can
    /// select the MONEY wire format (type 0x6E, 8-byte scaled integer) rather
    /// than the generic DECIMAL format. MONEY columns returned from queries
    /// decode back to [`Self::Decimal`] — the distinction is only meaningful
    /// on the send path.
    #[cfg(feature = "decimal")]
    Money(rust_decimal::Decimal),
    /// SmallMoney value (SMALLMONEY — fixed-point scaled by 10_000, signed 32-bit range).
    #[cfg(feature = "decimal")]
    SmallMoney(rust_decimal::Decimal),
    /// UUID value (UNIQUEIDENTIFIER).
    #[cfg(feature = "uuid")]
    Uuid(uuid::Uuid),
    /// Date value (DATE).
    #[cfg(feature = "chrono")]
    Date(chrono::NaiveDate),
    /// Time value (TIME).
    #[cfg(feature = "chrono")]
    Time(chrono::NaiveTime),
    /// DateTime value (DATETIME, DATETIME2).
    #[cfg(feature = "chrono")]
    DateTime(chrono::NaiveDateTime),
    /// SmallDateTime value (SMALLDATETIME — minute precision, 1900-01-01..2079-06-06).
    ///
    /// Distinct from [`Self::DateTime`] so that RPC parameter encoding can
    /// select the SMALLDATETIME wire format (type 0x6F, 4-byte days+minutes)
    /// rather than DATETIME2. SMALLDATETIME columns returned from queries
    /// decode back to [`Self::DateTime`].
    #[cfg(feature = "chrono")]
    SmallDateTime(chrono::NaiveDateTime),
    /// DateTimeOffset value (DATETIMEOFFSET).
    #[cfg(feature = "chrono")]
    DateTimeOffset(chrono::DateTime<chrono::FixedOffset>),
    /// JSON value (JSON type in SQL Server 2016+).
    #[cfg(feature = "json")]
    Json(serde_json::Value),
    /// XML value (XML type).
    Xml(String),
    /// Table-Valued Parameter (TVP).
    ///
    /// TVPs allow passing collections of structured data to SQL Server stored
    /// procedures. Boxed due to large size.
    Tvp(Box<TvpData>),
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
            #[cfg(feature = "decimal")]
            Self::Money(_) => "MONEY",
            #[cfg(feature = "decimal")]
            Self::SmallMoney(_) => "SMALLMONEY",
            #[cfg(feature = "uuid")]
            Self::Uuid(_) => "UNIQUEIDENTIFIER",
            #[cfg(feature = "chrono")]
            Self::Date(_) => "DATE",
            #[cfg(feature = "chrono")]
            Self::Time(_) => "TIME",
            #[cfg(feature = "chrono")]
            Self::DateTime(_) => "DATETIME2",
            #[cfg(feature = "chrono")]
            Self::SmallDateTime(_) => "SMALLDATETIME",
            #[cfg(feature = "chrono")]
            Self::DateTimeOffset(_) => "DATETIMEOFFSET",
            #[cfg(feature = "json")]
            Self::Json(_) => "JSON",
            Self::Xml(_) => "XML",
            Self::Tvp(_) => "TVP",
        }
    }

    /// Get the value as a TVP, if it is one.
    #[must_use]
    pub fn as_tvp(&self) -> Option<&TvpData> {
        match self {
            Self::Tvp(v) => Some(v),
            _ => None,
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

impl From<TvpData> for SqlValue {
    fn from(v: TvpData) -> Self {
        Self::Tvp(Box::new(v))
    }
}

/// Wrapper that sends its inner [`rust_decimal::Decimal`] as SQL Server MONEY
/// (signed 64-bit fixed-point scaled by 10_000) instead of DECIMAL.
///
/// Wrap a `Decimal` in `Money` when binding RPC parameters to force the MONEY
/// wire format (type 0x6E, 8 bytes) — a plain `Decimal` would bind as the
/// generic DECIMAL type (0x6C) and incur an implicit conversion on the server.
#[cfg(feature = "decimal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money(pub rust_decimal::Decimal);

/// Wrapper that sends its inner [`rust_decimal::Decimal`] as SQL Server
/// SMALLMONEY (signed 32-bit fixed-point scaled by 10_000).
#[cfg(feature = "decimal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmallMoney(pub rust_decimal::Decimal);

/// Wrapper that sends its inner [`chrono::NaiveDateTime`] as SQL Server
/// SMALLDATETIME (4-byte days-since-1900 + minutes-since-midnight) instead of
/// DATETIME2.
///
/// SMALLDATETIME has minute precision — seconds are rounded to the nearest
/// minute on the wire (30s rounds up per SQL Server semantics). The valid
/// range is 1900-01-01 through 2079-06-06.
#[cfg(feature = "chrono")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmallDateTime(pub chrono::NaiveDateTime);

#[cfg(feature = "decimal")]
impl From<Money> for SqlValue {
    fn from(v: Money) -> Self {
        Self::Money(v.0)
    }
}

#[cfg(feature = "decimal")]
impl From<SmallMoney> for SqlValue {
    fn from(v: SmallMoney) -> Self {
        Self::SmallMoney(v.0)
    }
}

#[cfg(feature = "chrono")]
impl From<SmallDateTime> for SqlValue {
    fn from(v: SmallDateTime) -> Self {
        Self::SmallDateTime(v.0)
    }
}
