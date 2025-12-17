# mssql-types

SQL Server to Rust type mappings and conversions for the rust-mssql-driver project.

## Overview

This crate provides bidirectional mapping between SQL Server data types and Rust types, handling encoding and decoding of values in TDS wire format.

## Type Mappings

| SQL Server Type | Rust Type | Feature |
|-----------------|-----------|---------|
| `BIT` | `bool` | default |
| `TINYINT` | `u8` | default |
| `SMALLINT` | `i16` | default |
| `INT` | `i32` | default |
| `BIGINT` | `i64` | default |
| `REAL` | `f32` | default |
| `FLOAT` | `f64` | default |
| `DECIMAL`/`NUMERIC` | `rust_decimal::Decimal` | `decimal` |
| `CHAR`/`VARCHAR` | `String` | default |
| `NCHAR`/`NVARCHAR` | `String` | default |
| `BINARY`/`VARBINARY` | `Bytes` | default |
| `DATE` | `chrono::NaiveDate` | `chrono` |
| `TIME` | `chrono::NaiveTime` | `chrono` |
| `DATETIME2` | `chrono::NaiveDateTime` | `chrono` |
| `DATETIMEOFFSET` | `chrono::DateTime<FixedOffset>` | `chrono` |
| `UNIQUEIDENTIFIER` | `uuid::Uuid` | `uuid` |
| `JSON` | `serde_json::Value` | `json` |

## Usage

```rust
use mssql_types::{SqlValue, FromSql, ToSql};

// Convert Rust value to SQL value
let rust_val: i32 = 42;
let sql_val = rust_val.to_sql()?;

// Convert SQL value to Rust type
let sql_val = SqlValue::Int(42);
let rust_val: i32 = i32::from_sql(&sql_val)?;

// NULL handling with Option
let nullable: Option<i32> = None;
let sql_val = nullable.to_sql()?;  // SqlValue::Null
```

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `chrono` | Yes | Date/time type support |
| `uuid` | Yes | UUID type support |
| `decimal` | Yes | Decimal type support via rust_decimal |
| `json` | No | JSON type support via serde_json |

## Traits

### `ToSql`

Convert Rust types to SQL values:

```rust
pub trait ToSql {
    fn to_sql(&self) -> Result<SqlValue, TypeError>;
}
```

### `FromSql`

Convert SQL values to Rust types:

```rust
pub trait FromSql: Sized {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError>;
}
```

## License

MIT OR Apache-2.0
