#![no_main]

use arbitrary::Arbitrary;
use bytes::{BufMut, Bytes, BytesMut};
use libfuzzer_sys::fuzz_target;
use mssql_types::encode::TdsEncode;
use mssql_types::SqlValue;

/// Arbitrary SQL values for round-trip fuzzing.
///
/// Covers all value-carrying SqlValue variants including Decimal, UUID,
/// Date, Time, DateTime, DateTimeOffset, and Xml.
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
    // Decimal: use i64 mantissa + scale to construct valid Decimal values
    Decimal { mantissa: i64, scale: u8 },
    // UUID: construct from raw bytes
    Uuid([u8; 16]),
    // Date: days since epoch (constrained to valid range)
    Date(u32),
    // Time: nanoseconds since midnight (constrained)
    Time(u64),
    // DateTime: date days + time nanoseconds
    DateTime { days: u32, nanos: u64 },
    // DateTimeOffset: datetime + offset minutes
    DateTimeOffset { days: u32, nanos: u64, offset_minutes: i16 },
    // Xml: string content
    Xml(String),
}

impl FuzzSqlValue {
    fn to_sql_value(self) -> SqlValue {
        match self {
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
            FuzzSqlValue::Decimal { mantissa, scale } => {
                let mut d = rust_decimal::Decimal::from(mantissa);
                // scale must be 0..=28 for rust_decimal
                let _ = d.set_scale(scale.min(28) as u32);
                SqlValue::Decimal(d)
            }
            FuzzSqlValue::Uuid(bytes) => SqlValue::Uuid(uuid::Uuid::from_bytes(bytes)),
            FuzzSqlValue::Date(days) => {
                // Constrain to valid TDS date range: 0001-01-01 to 9999-12-31
                // Max days = 3652058
                let days = days % 3_652_059;
                let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                let date = base + chrono::Duration::days(days as i64);
                SqlValue::Date(date)
            }
            FuzzSqlValue::Time(nanos) => {
                // Constrain to valid range: 0..86_400_000_000_000 nanoseconds
                let nanos = nanos % 86_400_000_000_000;
                let secs = (nanos / 1_000_000_000) as u32;
                let nsec = (nanos % 1_000_000_000) as u32;
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nsec)
                    .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                SqlValue::Time(time)
            }
            FuzzSqlValue::DateTime { days, nanos } => {
                let days = days % 3_652_059;
                let nanos = nanos % 86_400_000_000_000;
                let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                let date = base + chrono::Duration::days(days as i64);
                let secs = (nanos / 1_000_000_000) as u32;
                let nsec = (nanos % 1_000_000_000) as u32;
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nsec)
                    .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                SqlValue::DateTime(chrono::NaiveDateTime::new(date, time))
            }
            FuzzSqlValue::DateTimeOffset { days, nanos, offset_minutes } => {
                let days = days % 3_652_059;
                let nanos = nanos % 86_400_000_000_000;
                // Clamp offset to ±14:00 (±840 minutes)
                let offset_minutes = offset_minutes.clamp(-840, 840);
                let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                let date = base + chrono::Duration::days(days as i64);
                let secs = (nanos / 1_000_000_000) as u32;
                let nsec = (nanos % 1_000_000_000) as u32;
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nsec)
                    .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                let naive = chrono::NaiveDateTime::new(date, time);
                let offset = chrono::FixedOffset::east_opt(offset_minutes as i32 * 60)
                    .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
                SqlValue::DateTimeOffset(chrono::DateTime::from_naive_utc_and_offset(naive, offset))
            }
            FuzzSqlValue::Xml(v) => SqlValue::Xml(v),
        }
    }
}

/// Build a decode buffer by prepending a length byte to encoded data.
///
/// TDS variable-length decode functions expect: [len_u8][data...].
/// The encode functions only write the data. This bridges the gap.
fn frame_for_decode(encoded: &[u8]) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + encoded.len());
    buf.put_u8(encoded.len() as u8);
    buf.put_slice(encoded);
    buf.freeze()
}

fuzz_target!(|input: FuzzSqlValue| {
    let value = input.to_sql_value();

    // Test is_null
    let _is_null = value.is_null();

    // Test debug formatting (shouldn't panic)
    let _debug = format!("{:?}", value);

    // Test type_name (shouldn't panic)
    let _name = value.type_name();

    // Test encode (shouldn't panic on valid values)
    let mut encode_buf = BytesMut::new();
    let type_id = value.type_id();
    let encode_result = value.encode(&mut encode_buf);

    // If encoding succeeded, try decode roundtrip for types with
    // symmetric encode/decode format
    if let Ok(()) = encode_result {
        let encoded = encode_buf.freeze();

        // Build TypeInfo for the decode path
        let type_info = match type_id {
            0x6C => {
                // DECIMALTYPE: decode expects len prefix + sign + mantissa
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: Some(17),
                    scale: Some(10),
                    precision: Some(28),
                    collation: None,
                }
            }
            0x24 => {
                // GUIDTYPE: decode expects len prefix + 16 bytes
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: Some(16),
                    scale: None,
                    precision: None,
                    collation: None,
                }
            }
            0x28 => {
                // DATETYPE: decode expects len prefix + 3 bytes
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: Some(3),
                    scale: None,
                    precision: None,
                    collation: None,
                }
            }
            0x29 => {
                // TIMETYPE: decode expects len prefix + 5 bytes (scale 7)
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: Some(5),
                    scale: Some(7),
                    precision: None,
                    collation: None,
                }
            }
            0x2A => {
                // DATETIME2TYPE: decode expects len prefix + 8 bytes (5 time + 3 date, scale 7)
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: Some(8),
                    scale: Some(7),
                    precision: None,
                    collation: None,
                }
            }
            0x2B => {
                // DATETIMEOFFSETTYPE: decode expects len prefix + 10 bytes (5 time + 3 date + 2 offset)
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: Some(10),
                    scale: Some(7),
                    precision: None,
                    collation: None,
                }
            }
            // Fixed-size types don't need length prefix — decode reads exactly N bytes
            0x30 | 0x32 | 0x34 | 0x38 | 0x7F | 0x3B | 0x3E => {
                mssql_types::decode::TypeInfo {
                    type_id,
                    length: None,
                    scale: None,
                    precision: None,
                    collation: None,
                }
            }
            _ => {
                // String, Binary, Xml, Null — skip decode roundtrip
                // (different framing between encode and TDS wire format)
                return;
            }
        };

        // For variable-length types, prepend length byte
        let mut decode_bytes = match type_id {
            0x6C | 0x24 | 0x28 | 0x29 | 0x2A | 0x2B => frame_for_decode(&encoded),
            _ => Bytes::from(encoded.to_vec()),
        };

        // Attempt decode — should not panic
        let decode_result = mssql_types::decode::decode_value(&mut decode_bytes, &type_info);

        // If decode succeeded, verify basic invariants
        if let Ok(decoded) = decode_result {
            // Roundtrip should preserve the type (same variant)
            assert_eq!(
                std::mem::discriminant(&decoded),
                std::mem::discriminant(&value.clone()),
                "roundtrip changed type: encoded {:?}, decoded {:?}",
                value.type_name(),
                decoded.type_name(),
            );

            // Non-null values should decode as non-null
            if !value.is_null() {
                assert!(!decoded.is_null(), "non-null value decoded as null");
            }
        }
    }
});
