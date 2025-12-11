//! # mssql-types
//!
//! SQL Server to Rust type mappings and conversions.
//!
//! This crate provides bidirectional mapping between SQL Server data types
//! and Rust types, handling the encoding and decoding of values in TDS format.
//!
//! ## Features
//!
//! - `chrono` (default): Enable date/time type support via chrono
//! - `uuid` (default): Enable UUID type support
//! - `decimal` (default): Enable decimal type support via rust_decimal
//! - `json`: Enable JSON type support via serde_json
//!
//! ## Type Mappings
//!
//! | SQL Server Type | Rust Type |
//! |-----------------|-----------|
//! | `BIT` | `bool` |
//! | `TINYINT` | `u8` |
//! | `SMALLINT` | `i16` |
//! | `INT` | `i32` |
//! | `BIGINT` | `i64` |
//! | `REAL` | `f32` |
//! | `FLOAT` | `f64` |
//! | `DECIMAL`/`NUMERIC` | `rust_decimal::Decimal` |
//! | `CHAR`/`VARCHAR` | `String` |
//! | `NCHAR`/`NVARCHAR` | `String` |
//! | `DATE` | `chrono::NaiveDate` |
//! | `TIME` | `chrono::NaiveTime` |
//! | `DATETIME2` | `chrono::NaiveDateTime` |
//! | `UNIQUEIDENTIFIER` | `uuid::Uuid` |

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod decode;
pub mod encode;
pub mod error;
pub mod from_sql;
pub mod to_sql;
pub mod value;

pub use decode::{decode_utf16_string, decode_value, Collation, TdsDecode, TypeInfo};
pub use encode::{encode_utf16_string, TdsEncode};
pub use error::TypeError;
pub use from_sql::FromSql;
pub use to_sql::ToSql;
pub use value::SqlValue;
