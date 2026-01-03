#![no_main]

use arbitrary::Arbitrary;
use bytes::{BufMut, Bytes, BytesMut};
use libfuzzer_sys::fuzz_target;
use mssql_types::{SqlValue, ToSql};

/// Arbitrary SQL values for round-trip fuzzing.
#[derive(Debug, Arbitrary)]
enum FuzzSqlValue {
    Null,
    Bool(bool),
    TinyInt(u8),
    SmallInt(i16),
    Int(i32),
    BigInt(i64),
    Float(f32),
    Double(f64),
    String(String),
    Binary(Vec<u8>),
}

fuzz_target!(|input: FuzzSqlValue| {
    // Convert arbitrary input to SqlValue
    let value: SqlValue = match input {
        FuzzSqlValue::Null => SqlValue::Null,
        FuzzSqlValue::Bool(v) => SqlValue::Bool(v),
        FuzzSqlValue::TinyInt(v) => SqlValue::TinyInt(v),
        FuzzSqlValue::SmallInt(v) => SqlValue::SmallInt(v),
        FuzzSqlValue::Int(v) => SqlValue::Int(v),
        FuzzSqlValue::BigInt(v) => SqlValue::BigInt(v),
        FuzzSqlValue::Float(v) => SqlValue::Float(v),
        FuzzSqlValue::Double(v) => SqlValue::Double(v),
        FuzzSqlValue::String(v) => SqlValue::String(v),
        FuzzSqlValue::Binary(v) => SqlValue::Binary(Bytes::from(v)),
    };

    // Test is_null
    let _is_null = value.is_null();

    // Test debug formatting (shouldn't panic)
    let _debug = format!("{:?}", value);
});
