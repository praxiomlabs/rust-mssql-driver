//! Trait for converting from SQL values to Rust types.

use crate::error::TypeError;
use crate::value::SqlValue;

/// Trait for types that can be converted from SQL values.
///
/// This trait is implemented for common Rust types to enable
/// type-safe extraction of values from query results.
pub trait FromSql: Sized {
    /// Convert from a SQL value to this type.
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError>;

    /// Convert from an optional SQL value.
    ///
    /// Returns `None` if the value is NULL.
    fn from_sql_nullable(value: &SqlValue) -> Result<Option<Self>, TypeError> {
        if value.is_null() {
            Ok(None)
        } else {
            Self::from_sql(value).map(Some)
        }
    }
}

impl FromSql for bool {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Bool(v) => Ok(*v),
            SqlValue::TinyInt(v) => Ok(*v != 0),
            SqlValue::SmallInt(v) => Ok(*v != 0),
            SqlValue::Int(v) => Ok(*v != 0),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "bool",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for u8 {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::TinyInt(v) => Ok(*v),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "u8",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for i16 {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::SmallInt(v) => Ok(*v),
            SqlValue::TinyInt(v) => Ok(*v as i16),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "i16",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for i32 {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Int(v) => Ok(*v),
            SqlValue::SmallInt(v) => Ok(*v as i32),
            SqlValue::TinyInt(v) => Ok(*v as i32),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "i32",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for i64 {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::BigInt(v) => Ok(*v),
            SqlValue::Int(v) => Ok(*v as i64),
            SqlValue::SmallInt(v) => Ok(*v as i64),
            SqlValue::TinyInt(v) => Ok(*v as i64),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "i64",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for f32 {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Float(v) => Ok(*v),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "f32",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for f64 {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Double(v) => Ok(*v),
            SqlValue::Float(v) => Ok(*v as f64),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "f64",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for String {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::String(v) => Ok(v.clone()),
            SqlValue::Xml(v) => Ok(v.clone()),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "String",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl FromSql for Vec<u8> {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Binary(v) => Ok(v.to_vec()),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "Vec<u8>",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

impl<T: FromSql> FromSql for Option<T> {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        T::from_sql_nullable(value)
    }
}

#[cfg(feature = "uuid")]
impl FromSql for uuid::Uuid {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Uuid(v) => Ok(*v),
            SqlValue::Binary(b) if b.len() == 16 => {
                let bytes: [u8; 16] = b[..]
                    .try_into()
                    .map_err(|_| TypeError::InvalidUuid("invalid UUID length".to_string()))?;
                Ok(uuid::Uuid::from_bytes(bytes))
            }
            SqlValue::String(s) => s
                .parse()
                .map_err(|e| TypeError::InvalidUuid(format!("{e}"))),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "Uuid",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "decimal")]
impl FromSql for rust_decimal::Decimal {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Decimal(v) => Ok(*v),
            SqlValue::Int(v) => Ok(rust_decimal::Decimal::from(*v)),
            SqlValue::BigInt(v) => Ok(rust_decimal::Decimal::from(*v)),
            SqlValue::String(s) => s
                .parse()
                .map_err(|e| TypeError::InvalidDecimal(format!("{e}"))),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "Decimal",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "chrono")]
impl FromSql for chrono::NaiveDate {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Date(v) => Ok(*v),
            SqlValue::DateTime(v) => Ok(v.date()),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "NaiveDate",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "chrono")]
impl FromSql for chrono::NaiveTime {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Time(v) => Ok(*v),
            SqlValue::DateTime(v) => Ok(v.time()),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "NaiveTime",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "chrono")]
impl FromSql for chrono::NaiveDateTime {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::DateTime(v) => Ok(*v),
            SqlValue::DateTimeOffset(v) => Ok(v.naive_utc()),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "NaiveDateTime",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "chrono")]
impl FromSql for chrono::DateTime<chrono::FixedOffset> {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::DateTimeOffset(v) => Ok(*v),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "DateTime<FixedOffset>",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "chrono")]
impl FromSql for chrono::DateTime<chrono::Utc> {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::DateTimeOffset(v) => Ok(v.to_utc()),
            SqlValue::DateTime(v) => Ok(chrono::DateTime::from_naive_utc_and_offset(*v, chrono::Utc)),
            SqlValue::Null => Err(TypeError::UnexpectedNull),
            _ => Err(TypeError::TypeMismatch {
                expected: "DateTime<Utc>",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl FromSql for serde_json::Value {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        match value {
            SqlValue::Json(v) => Ok(v.clone()),
            SqlValue::String(s) => serde_json::from_str(s).map_err(|e| TypeError::TypeMismatch {
                expected: "JSON",
                actual: format!("invalid JSON: {e}"),
            }),
            SqlValue::Null => Ok(serde_json::Value::Null),
            _ => Err(TypeError::TypeMismatch {
                expected: "JSON",
                actual: value.type_name().to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_sql_i32() {
        let value = SqlValue::Int(42);
        assert_eq!(i32::from_sql(&value).unwrap(), 42);
    }

    #[test]
    fn test_from_sql_string() {
        let value = SqlValue::String("hello".to_string());
        assert_eq!(String::from_sql(&value).unwrap(), "hello");
    }

    #[test]
    fn test_from_sql_null() {
        let value = SqlValue::Null;
        assert!(i32::from_sql(&value).is_err());
    }

    #[test]
    fn test_from_sql_option() {
        let value = SqlValue::Int(42);
        assert_eq!(Option::<i32>::from_sql(&value).unwrap(), Some(42));

        let null = SqlValue::Null;
        assert_eq!(Option::<i32>::from_sql(&null).unwrap(), None);
    }
}
