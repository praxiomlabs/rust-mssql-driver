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
}

impl<T: ToSql + ?Sized> ToSql for &T {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        (*self).to_sql()
    }

    fn sql_type(&self) -> &'static str {
        (*self).sql_type()
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
}
