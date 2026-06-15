//! Trait for converting Rust types to SQL values.

// Allow expect() for chrono date construction with known-valid constant dates
#![allow(clippy::expect_used)]

use crate::error::TypeError;
use crate::value::SqlValue;

/// Trait for types that can be converted to SQL values.
///
/// This trait is implemented for common Rust types to enable
/// type-safe parameter binding in queries.
pub trait ToSql {
    /// Convert this value to a SQL value.
    fn to_sql(&self) -> Result<SqlValue, TypeError>;

    /// Get the SQL type name for this value.
    fn sql_type(&self) -> &'static str;

    /// The explicit SQL type a parameter must be declared and encrypted as,
    /// when the value alone cannot convey it.
    ///
    /// Returns `None` for every type except the typed-parameter wrappers
    /// (e.g. [`numeric`], [`datetime2`], [`time`]). An Always Encrypted column
    /// requires the declared type — including precision, scale, or length — to
    /// match the column exactly, which a bare value cannot always express (a
    /// `Decimal` carries no precision; a `NaiveDateTime` is ambiguous between
    /// `datetime` and `datetime2(n)`). The driver uses this to declare the
    /// parameter for `sp_describe_parameter_encryption` and to normalize the
    /// value before encryption.
    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        None
    }
}

/// The explicit SQL type for an Always Encrypted parameter whose value cannot
/// convey it (see [`numeric`], [`time`], [`datetime2`], [`datetimeoffset`],
/// [`datetime`]). Carries the precision/scale/length the encrypted column
/// requires the declared parameter type to match exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncryptedParamType {
    /// `decimal(precision, scale)`.
    Decimal {
        /// Total number of significant digits (1–38).
        precision: u8,
        /// Number of digits to the right of the decimal point.
        scale: u8,
    },
    /// `time(scale)`.
    Time {
        /// Fractional-second digits (0–7).
        scale: u8,
    },
    /// `datetime2(scale)`.
    DateTime2 {
        /// Fractional-second digits (0–7).
        scale: u8,
    },
    /// `datetimeoffset(scale)`.
    DateTimeOffset {
        /// Fractional-second digits (0–7).
        scale: u8,
    },
    /// Legacy `datetime` (8 bytes; ~3.33 ms resolution).
    DateTime,
}

impl ToSql for bool {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Bool(*self))
    }

    fn sql_type(&self) -> &'static str {
        "BIT"
    }
}

impl ToSql for u8 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::TinyInt(*self))
    }

    fn sql_type(&self) -> &'static str {
        "TINYINT"
    }
}

impl ToSql for i16 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::SmallInt(*self))
    }

    fn sql_type(&self) -> &'static str {
        "SMALLINT"
    }
}

impl ToSql for i32 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Int(*self))
    }

    fn sql_type(&self) -> &'static str {
        "INT"
    }
}

impl ToSql for i64 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::BigInt(*self))
    }

    fn sql_type(&self) -> &'static str {
        "BIGINT"
    }
}

impl ToSql for f32 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Float(*self))
    }

    fn sql_type(&self) -> &'static str {
        "REAL"
    }
}

impl ToSql for f64 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Double(*self))
    }

    fn sql_type(&self) -> &'static str {
        "FLOAT"
    }
}

impl ToSql for str {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::String(self.to_owned()))
    }

    fn sql_type(&self) -> &'static str {
        "NVARCHAR"
    }
}

impl ToSql for String {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::String(self.clone()))
    }

    fn sql_type(&self) -> &'static str {
        "NVARCHAR"
    }
}

impl ToSql for [u8] {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Binary(bytes::Bytes::copy_from_slice(self)))
    }

    fn sql_type(&self) -> &'static str {
        "VARBINARY"
    }
}

impl ToSql for Vec<u8> {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Binary(bytes::Bytes::copy_from_slice(self)))
    }

    fn sql_type(&self) -> &'static str {
        "VARBINARY"
    }
}

/// Associates a Rust type with its SQL type name so a typed NULL can be
/// declared without a value (see [`null`]).
///
/// `SQL_TYPE` must match what [`ToSql::sql_type`] returns for a value of the
/// same type.
pub trait SqlTyped {
    /// The SQL type name for this Rust type.
    const SQL_TYPE: &'static str;
}

impl SqlTyped for bool {
    const SQL_TYPE: &'static str = "BIT";
}
impl SqlTyped for u8 {
    const SQL_TYPE: &'static str = "TINYINT";
}
impl SqlTyped for i16 {
    const SQL_TYPE: &'static str = "SMALLINT";
}
impl SqlTyped for i32 {
    const SQL_TYPE: &'static str = "INT";
}
impl SqlTyped for i64 {
    const SQL_TYPE: &'static str = "BIGINT";
}
impl SqlTyped for f32 {
    const SQL_TYPE: &'static str = "REAL";
}
impl SqlTyped for f64 {
    const SQL_TYPE: &'static str = "FLOAT";
}
impl SqlTyped for String {
    const SQL_TYPE: &'static str = "NVARCHAR";
}
impl SqlTyped for Vec<u8> {
    const SQL_TYPE: &'static str = "VARBINARY";
}
#[cfg(feature = "uuid")]
impl SqlTyped for uuid::Uuid {
    const SQL_TYPE: &'static str = "UNIQUEIDENTIFIER";
}
#[cfg(feature = "chrono")]
impl SqlTyped for chrono::NaiveDate {
    const SQL_TYPE: &'static str = "DATE";
}

/// A typed NULL parameter, created with [`null`].
///
/// Unlike `Option::<T>::None`, which produces an untyped NULL declared as
/// `nvarchar(1)`, this carries its SQL type. That matters for Always Encrypted
/// columns, whose strict typing rejects an untyped NULL bound to, for example,
/// an `int` or `varbinary` column.
#[derive(Debug, Clone, Copy)]
pub struct TypedNull {
    sql_type: &'static str,
}

impl ToSql for TypedNull {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Null)
    }

    fn sql_type(&self) -> &'static str {
        self.sql_type
    }
}

/// Create a typed NULL parameter for SQL type `T`, e.g. `null::<i32>()`.
///
/// Use this in place of `Option::<T>::None` when binding NULL to a strongly
/// typed column — required for an Always Encrypted column of a non-string type.
#[must_use]
pub fn null<T: SqlTyped>() -> TypedNull {
    TypedNull {
        sql_type: T::SQL_TYPE,
    }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        match self {
            Some(v) => v.to_sql(),
            None => Ok(SqlValue::Null),
        }
    }

    fn sql_type(&self) -> &'static str {
        match self {
            Some(v) => v.sql_type(),
            None => "NULL",
        }
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        self.as_ref().and_then(ToSql::encrypted_param_type)
    }
}

impl<T: ToSql + ?Sized> ToSql for &T {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        (*self).to_sql()
    }

    fn sql_type(&self) -> &'static str {
        (*self).sql_type()
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        (*self).encrypted_param_type()
    }
}

#[cfg(feature = "uuid")]
impl ToSql for uuid::Uuid {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Uuid(*self))
    }

    fn sql_type(&self) -> &'static str {
        "UNIQUEIDENTIFIER"
    }
}

#[cfg(feature = "decimal")]
impl ToSql for rust_decimal::Decimal {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Decimal(*self))
    }

    fn sql_type(&self) -> &'static str {
        "DECIMAL"
    }
}

/// A `decimal`/`numeric` parameter with explicit precision and scale.
///
/// A plain [`rust_decimal::Decimal`] carries scale but not precision, so it
/// cannot be matched against an Always Encrypted `decimal` column, whose
/// declared `decimal(precision, scale)` must match the column exactly.
/// Construct one with [`numeric`].
#[cfg(feature = "decimal")]
#[derive(Debug, Clone, Copy)]
pub struct Numeric {
    value: rust_decimal::Decimal,
    precision: u8,
    scale: u8,
}

/// Create a `decimal`/`numeric` parameter with explicit precision and scale.
///
/// Required when binding to an Always Encrypted `decimal` column, whose declared
/// `decimal(precision, scale)` must match the column exactly.
///
/// The value is rescaled to `scale`, which **rounds** when the value has more
/// fractional digits than `scale` (e.g. `numeric(dec!(12.999), 18, 2)` stores
/// `13.00`). If the rescaled value has more significant digits than `precision`,
/// [`ToSql::to_sql`] returns an error rather than silently storing a value
/// outside the column's domain — the server cannot range-check an encrypted
/// value, so the client enforces it.
#[cfg(feature = "decimal")]
#[must_use]
pub fn numeric(value: rust_decimal::Decimal, precision: u8, scale: u8) -> Numeric {
    Numeric {
        value,
        precision,
        scale,
    }
}

#[cfg(feature = "decimal")]
impl ToSql for Numeric {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        let mut value = self.value;
        value.rescale(u32::from(self.scale));
        // The server cannot range-check an encrypted value, so a value that
        // exceeds the declared precision must be rejected client-side rather
        // than silently stored out of the column's domain (matches the
        // Always Encrypted behaviour of Microsoft.Data.SqlClient). After
        // rescaling, the magnitude bound `|mantissa| < 10^precision` is exactly
        // the `decimal(precision, scale)` domain.
        let mantissa = value.mantissa().unsigned_abs();
        let digits = if mantissa == 0 {
            0
        } else {
            mantissa.ilog10() + 1
        };
        if digits > u32::from(self.precision) {
            return Err(TypeError::InvalidDecimal(format!(
                "value has {digits} significant digits, which exceeds the declared precision {}",
                self.precision
            )));
        }
        Ok(SqlValue::Decimal(value))
    }

    fn sql_type(&self) -> &'static str {
        "DECIMAL"
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        Some(EncryptedParamType::Decimal {
            precision: self.precision,
            scale: self.scale,
        })
    }
}

/// Fractional-second scale for `time`/`datetime2`/`datetimeoffset` is 0–7.
#[cfg(feature = "chrono")]
fn validate_temporal_scale(scale: u8) -> Result<(), TypeError> {
    if scale > 7 {
        return Err(TypeError::InvalidDateTime(format!(
            "fractional-second scale {scale} is out of range (0–7)"
        )));
    }
    Ok(())
}

/// A `time(scale)` parameter for an Always Encrypted column (see [`time`]).
#[cfg(feature = "chrono")]
#[derive(Debug, Clone, Copy)]
pub struct Time {
    value: chrono::NaiveTime,
    scale: u8,
}

/// Create a `time(scale)` parameter for an Always Encrypted `time` column.
///
/// AE requires the declared `time(scale)` to match the column exactly, and the
/// scale also determines the encrypted byte length, so the value alone is
/// insufficient. `scale` is the fractional-second digits (0–7).
#[cfg(feature = "chrono")]
#[must_use]
pub fn time(value: chrono::NaiveTime, scale: u8) -> Time {
    Time { value, scale }
}

#[cfg(feature = "chrono")]
impl ToSql for Time {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        validate_temporal_scale(self.scale)?;
        Ok(SqlValue::Time(self.value))
    }

    fn sql_type(&self) -> &'static str {
        "TIME"
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        Some(EncryptedParamType::Time { scale: self.scale })
    }
}

/// A `datetime2(scale)` parameter for an Always Encrypted column (see [`datetime2`]).
#[cfg(feature = "chrono")]
#[derive(Debug, Clone, Copy)]
pub struct DateTime2 {
    value: chrono::NaiveDateTime,
    scale: u8,
}

/// Create a `datetime2(scale)` parameter for an Always Encrypted `datetime2`
/// column. A plain `NaiveDateTime` defaults to `datetime2(7)`, so an explicit
/// scale is required to match a column with a different scale (and to encrypt
/// at the right byte length). `scale` is the fractional-second digits (0–7).
#[cfg(feature = "chrono")]
#[must_use]
pub fn datetime2(value: chrono::NaiveDateTime, scale: u8) -> DateTime2 {
    DateTime2 { value, scale }
}

#[cfg(feature = "chrono")]
impl ToSql for DateTime2 {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        validate_temporal_scale(self.scale)?;
        Ok(SqlValue::DateTime(self.value))
    }

    fn sql_type(&self) -> &'static str {
        "DATETIME2"
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        Some(EncryptedParamType::DateTime2 { scale: self.scale })
    }
}

/// A `datetimeoffset(scale)` parameter for an Always Encrypted column (see
/// [`datetimeoffset`]).
#[cfg(feature = "chrono")]
#[derive(Debug, Clone, Copy)]
pub struct DateTimeOffset {
    value: chrono::DateTime<chrono::FixedOffset>,
    scale: u8,
}

/// Create a `datetimeoffset(scale)` parameter for an Always Encrypted
/// `datetimeoffset` column. `scale` is the fractional-second digits (0–7).
#[cfg(feature = "chrono")]
#[must_use]
pub fn datetimeoffset(value: chrono::DateTime<chrono::FixedOffset>, scale: u8) -> DateTimeOffset {
    DateTimeOffset { value, scale }
}

#[cfg(feature = "chrono")]
impl ToSql for DateTimeOffset {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        validate_temporal_scale(self.scale)?;
        Ok(SqlValue::DateTimeOffset(self.value))
    }

    fn sql_type(&self) -> &'static str {
        "DATETIMEOFFSET"
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        Some(EncryptedParamType::DateTimeOffset { scale: self.scale })
    }
}

/// A legacy `datetime` parameter for an Always Encrypted column (see [`datetime`]).
#[cfg(feature = "chrono")]
#[derive(Debug, Clone, Copy)]
pub struct DateTimeLegacy {
    value: chrono::NaiveDateTime,
}

/// Create a legacy `datetime` parameter for an Always Encrypted `datetime`
/// column. A plain `NaiveDateTime` defaults to `datetime2`, which an encrypted
/// legacy `datetime` column rejects; this declares `datetime` explicitly.
#[cfg(feature = "chrono")]
#[must_use]
pub fn datetime(value: chrono::NaiveDateTime) -> DateTimeLegacy {
    DateTimeLegacy { value }
}

#[cfg(feature = "chrono")]
impl ToSql for DateTimeLegacy {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::DateTime(self.value))
    }

    fn sql_type(&self) -> &'static str {
        "DATETIME"
    }

    fn encrypted_param_type(&self) -> Option<EncryptedParamType> {
        Some(EncryptedParamType::DateTime)
    }
}

#[cfg(feature = "decimal")]
impl ToSql for crate::value::Money {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Money(self.0))
    }

    fn sql_type(&self) -> &'static str {
        "MONEY"
    }
}

#[cfg(feature = "decimal")]
impl ToSql for crate::value::SmallMoney {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::SmallMoney(self.0))
    }

    fn sql_type(&self) -> &'static str {
        "SMALLMONEY"
    }
}

#[cfg(feature = "chrono")]
impl ToSql for chrono::NaiveDate {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Date(*self))
    }

    fn sql_type(&self) -> &'static str {
        "DATE"
    }
}

#[cfg(feature = "chrono")]
impl ToSql for chrono::NaiveTime {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Time(*self))
    }

    fn sql_type(&self) -> &'static str {
        "TIME"
    }
}

#[cfg(feature = "chrono")]
impl ToSql for chrono::NaiveDateTime {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::DateTime(*self))
    }

    fn sql_type(&self) -> &'static str {
        "DATETIME2"
    }
}

#[cfg(feature = "chrono")]
impl ToSql for crate::value::SmallDateTime {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::SmallDateTime(self.0))
    }

    fn sql_type(&self) -> &'static str {
        "SMALLDATETIME"
    }
}

#[cfg(feature = "chrono")]
impl ToSql for chrono::DateTime<chrono::FixedOffset> {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::DateTimeOffset(*self))
    }

    fn sql_type(&self) -> &'static str {
        "DATETIMEOFFSET"
    }
}

#[cfg(feature = "chrono")]
impl ToSql for chrono::DateTime<chrono::Utc> {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        // Convert UTC to FixedOffset with +00:00 offset
        let fixed = self.with_timezone(&chrono::FixedOffset::east_opt(0).expect("valid offset"));
        Ok(SqlValue::DateTimeOffset(fixed))
    }

    fn sql_type(&self) -> &'static str {
        "DATETIMEOFFSET"
    }
}

#[cfg(feature = "json")]
impl ToSql for serde_json::Value {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::Json(self.clone()))
    }

    fn sql_type(&self) -> &'static str {
        "NVARCHAR(MAX)"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_to_sql_i32() {
        let value: i32 = 42;
        assert_eq!(value.to_sql().unwrap(), SqlValue::Int(42));
        assert_eq!(value.sql_type(), "INT");
    }

    #[test]
    fn test_typed_null_carries_type() {
        // A typed NULL is a NULL value that still reports its SQL type, and that
        // type matches what a value of the same Rust type reports.
        assert_eq!(null::<i32>().to_sql().unwrap(), SqlValue::Null);
        assert_eq!(null::<i32>().sql_type(), 42i32.sql_type());
        assert_eq!(null::<i64>().sql_type(), "BIGINT");
        assert_eq!(null::<Vec<u8>>().sql_type(), "VARBINARY");
        assert_eq!(null::<String>().sql_type(), "NVARCHAR");
    }

    #[test]
    fn test_to_sql_string() {
        let value = "hello".to_string();
        assert_eq!(
            value.to_sql().unwrap(),
            SqlValue::String("hello".to_string())
        );
        assert_eq!(value.sql_type(), "NVARCHAR");
    }

    #[test]
    fn test_to_sql_option() {
        let some: Option<i32> = Some(42);
        assert_eq!(some.to_sql().unwrap(), SqlValue::Int(42));

        let none: Option<i32> = None;
        assert_eq!(none.to_sql().unwrap(), SqlValue::Null);
    }

    #[cfg(feature = "decimal")]
    #[test]
    fn test_numeric_precision_validation() {
        use rust_decimal::Decimal;

        // Fits: a scale-2 value declared decimal(18,4) rescales without loss.
        assert!(numeric(Decimal::new(1_234_567, 2), 18, 4).to_sql().is_ok());

        // Exceeds declared precision: 6 significant digits into decimal(4,0).
        assert!(
            numeric(Decimal::new(123_456, 0), 4, 0).to_sql().is_err(),
            "value exceeding the declared precision must error"
        );

        // Rounds (does not error) when the value scale exceeds the declared scale.
        let rounded = numeric(Decimal::new(12_999, 3), 18, 2).to_sql().unwrap();
        assert_eq!(rounded, SqlValue::Decimal(Decimal::new(1_300, 2)));

        // Zero fits any precision.
        assert!(numeric(Decimal::ZERO, 1, 0).to_sql().is_ok());
    }
}
