//! Trait for converting Rust types to SQL values.

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
impl ToSql for chrono::DateTime<chrono::FixedOffset> {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        Ok(SqlValue::DateTimeOffset(*self))
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
mod tests {
    use super::*;

    #[test]
    fn test_to_sql_i32() {
        let value: i32 = 42;
        assert_eq!(value.to_sql().unwrap(), SqlValue::Int(42));
        assert_eq!(value.sql_type(), "INT");
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
