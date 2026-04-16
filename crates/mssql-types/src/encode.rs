//! TDS binary encoding for SQL values.
//!
//! This module provides encoding of Rust values into TDS wire format
//! for transmission to SQL Server.

// Allow expect() for chrono date construction with known-valid constant dates
#![allow(clippy::expect_used)]

use bytes::{BufMut, BytesMut};

use crate::error::TypeError;
use crate::value::SqlValue;

/// Trait for encoding values to TDS binary format.
pub trait TdsEncode {
    /// Encode this value into the buffer in TDS format.
    fn encode(&self, buf: &mut BytesMut) -> Result<(), TypeError>;

    /// Get the TDS type ID for this value.
    fn type_id(&self) -> u8;
}

impl TdsEncode for SqlValue {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), TypeError> {
        match self {
            SqlValue::Null => {
                // NULL is represented by length indicator in most contexts
                // For INTNTYPE, length 0 means NULL
                Ok(())
            }
            SqlValue::Bool(v) => {
                buf.put_u8(if *v { 1 } else { 0 });
                Ok(())
            }
            SqlValue::TinyInt(v) => {
                buf.put_u8(*v);
                Ok(())
            }
            SqlValue::SmallInt(v) => {
                buf.put_i16_le(*v);
                Ok(())
            }
            SqlValue::Int(v) => {
                buf.put_i32_le(*v);
                Ok(())
            }
            SqlValue::BigInt(v) => {
                buf.put_i64_le(*v);
                Ok(())
            }
            SqlValue::Float(v) => {
                buf.put_f32_le(*v);
                Ok(())
            }
            SqlValue::Double(v) => {
                buf.put_f64_le(*v);
                Ok(())
            }
            SqlValue::String(s) => {
                // Encode as UTF-16LE for NVARCHAR
                encode_utf16_string(s, buf);
                Ok(())
            }
            SqlValue::Binary(b) => {
                // Length-prefixed binary data
                if b.len() > u16::MAX as usize {
                    return Err(TypeError::BufferTooSmall {
                        needed: b.len(),
                        available: u16::MAX as usize,
                    });
                }
                buf.put_u16_le(b.len() as u16);
                buf.put_slice(b);
                Ok(())
            }
            #[cfg(feature = "decimal")]
            SqlValue::Decimal(d) => {
                encode_decimal(*d, buf);
                Ok(())
            }
            #[cfg(feature = "decimal")]
            SqlValue::Money(d) => encode_money(*d, buf),
            #[cfg(feature = "decimal")]
            SqlValue::SmallMoney(d) => encode_smallmoney(*d, buf),
            #[cfg(feature = "uuid")]
            SqlValue::Uuid(u) => {
                encode_uuid(*u, buf);
                Ok(())
            }
            #[cfg(feature = "chrono")]
            SqlValue::Date(d) => {
                encode_date(*d, buf);
                Ok(())
            }
            #[cfg(feature = "chrono")]
            SqlValue::Time(t) => {
                encode_time(*t, buf);
                Ok(())
            }
            #[cfg(feature = "chrono")]
            SqlValue::DateTime(dt) => {
                encode_datetime2(*dt, buf);
                Ok(())
            }
            #[cfg(feature = "chrono")]
            SqlValue::SmallDateTime(dt) => encode_smalldatetime(*dt, buf),
            #[cfg(feature = "chrono")]
            SqlValue::DateTimeOffset(dto) => {
                encode_datetimeoffset(*dto, buf);
                Ok(())
            }
            #[cfg(feature = "json")]
            SqlValue::Json(j) => {
                // JSON is sent as NVARCHAR string
                let s = j.to_string();
                encode_utf16_string(&s, buf);
                Ok(())
            }
            SqlValue::Xml(x) => {
                // XML is sent as UTF-16LE string
                encode_utf16_string(x, buf);
                Ok(())
            }
            SqlValue::Tvp(_) => {
                // TVP encoding is handled at the RPC parameter level, not here.
                // This method is for encoding the value data portion; TVPs have
                // their own complex encoding structure that includes metadata.
                // See tds-protocol crate for full TVP encoding.
                Err(TypeError::UnsupportedConversion {
                    from: "TvpData".to_string(),
                    to: "raw bytes (use RPC parameter encoding)",
                })
            }
        }
    }

    fn type_id(&self) -> u8 {
        match self {
            SqlValue::Null => 0x1F,        // NULLTYPE
            SqlValue::Bool(_) => 0x32,     // BITTYPE
            SqlValue::TinyInt(_) => 0x30,  // INT1TYPE
            SqlValue::SmallInt(_) => 0x34, // INT2TYPE
            SqlValue::Int(_) => 0x38,      // INT4TYPE
            SqlValue::BigInt(_) => 0x7F,   // INT8TYPE
            SqlValue::Float(_) => 0x3B,    // FLT4TYPE
            SqlValue::Double(_) => 0x3E,   // FLT8TYPE
            SqlValue::String(_) => 0xE7,   // NVARCHARTYPE
            SqlValue::Binary(_) => 0xA5,   // BIGVARBINTYPE
            #[cfg(feature = "decimal")]
            SqlValue::Decimal(_) => 0x6C, // DECIMALTYPE
            #[cfg(feature = "decimal")]
            SqlValue::Money(_) => 0x6E, // MONEYNTYPE (8-byte payload)
            #[cfg(feature = "decimal")]
            SqlValue::SmallMoney(_) => 0x6E, // MONEYNTYPE (4-byte payload)
            #[cfg(feature = "uuid")]
            SqlValue::Uuid(_) => 0x24, // GUIDTYPE
            #[cfg(feature = "chrono")]
            SqlValue::Date(_) => 0x28, // DATETYPE
            #[cfg(feature = "chrono")]
            SqlValue::Time(_) => 0x29, // TIMETYPE
            #[cfg(feature = "chrono")]
            SqlValue::DateTime(_) => 0x2A, // DATETIME2TYPE
            #[cfg(feature = "chrono")]
            SqlValue::SmallDateTime(_) => 0x6F, // DATETIMENTYPE (4-byte payload)
            #[cfg(feature = "chrono")]
            SqlValue::DateTimeOffset(_) => 0x2B, // DATETIMEOFFSETTYPE
            #[cfg(feature = "json")]
            SqlValue::Json(_) => 0xE7, // NVARCHARTYPE (JSON as string)
            SqlValue::Xml(_) => 0xF1,      // XMLTYPE
            SqlValue::Tvp(_) => 0xF3,      // TVPTYPE
        }
    }
}

/// Encode a string as UTF-16LE with length prefix.
pub fn encode_utf16_string(s: &str, buf: &mut BytesMut) {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let byte_len = utf16.len() * 2;

    // Write byte length (not char length)
    buf.put_u16_le(byte_len as u16);

    // Write UTF-16LE bytes
    for code_unit in utf16 {
        buf.put_u16_le(code_unit);
    }
}

/// Encode a string as UTF-16LE without length prefix (for fixed-length fields).
pub fn encode_utf16_string_no_len(s: &str, buf: &mut BytesMut) {
    for code_unit in s.encode_utf16() {
        buf.put_u16_le(code_unit);
    }
}

/// Encode a UUID in SQL Server's mixed-endian format.
///
/// SQL Server stores UUIDs in a unique byte order:
/// - First 4 bytes: little-endian
/// - Next 2 bytes: little-endian
/// - Next 2 bytes: little-endian
/// - Last 8 bytes: big-endian (as-is)
#[cfg(feature = "uuid")]
pub fn encode_uuid(uuid: uuid::Uuid, buf: &mut BytesMut) {
    let bytes = uuid.as_bytes();

    // First group (4 bytes) - reverse for little-endian
    buf.put_u8(bytes[3]);
    buf.put_u8(bytes[2]);
    buf.put_u8(bytes[1]);
    buf.put_u8(bytes[0]);

    // Second group (2 bytes) - reverse for little-endian
    buf.put_u8(bytes[5]);
    buf.put_u8(bytes[4]);

    // Third group (2 bytes) - reverse for little-endian
    buf.put_u8(bytes[7]);
    buf.put_u8(bytes[6]);

    // Last 8 bytes - big-endian (keep as-is)
    buf.put_slice(&bytes[8..16]);
}

/// Encode a decimal value.
///
/// TDS DECIMAL format:
/// - 1 byte: sign (0 = negative, 1 = positive)
/// - Remaining bytes: absolute value in little-endian
#[cfg(feature = "decimal")]
pub fn encode_decimal(decimal: rust_decimal::Decimal, buf: &mut BytesMut) {
    let sign = if decimal.is_sign_negative() { 0u8 } else { 1u8 };
    buf.put_u8(sign);

    // Get the mantissa and encode as 128-bit integer
    let mantissa = decimal.mantissa().unsigned_abs();
    buf.put_u128_le(mantissa);
}

/// Rescale a decimal to MONEY's 4-decimal fixed-point representation.
///
/// Returns the signed 128-bit integer representing the value multiplied by
/// 10_000. Excess precision past 4 decimal places is truncated toward zero.
#[cfg(feature = "decimal")]
fn decimal_to_money_cents(value: rust_decimal::Decimal) -> Result<i128, TypeError> {
    let mantissa: i128 = value.mantissa();
    let scale: u32 = value.scale();
    if scale <= 4 {
        let factor = 10_i128.pow(4 - scale);
        mantissa
            .checked_mul(factor)
            .ok_or(TypeError::OutOfRange { target_type: "MONEY" })
    } else {
        let factor = 10_i128.pow(scale - 4);
        Ok(mantissa / factor)
    }
}

/// Convert a decimal to the scaled i64 used on the MONEY wire.
///
/// This is the shared pre-encoding step for both RPC parameter encoding and
/// TVP column encoding — each path knows how to frame the payload, but they
/// agree on how to derive the scaled integer from the decimal.
#[cfg(feature = "decimal")]
pub fn decimal_to_money_cents_i64(value: rust_decimal::Decimal) -> Result<i64, TypeError> {
    let cents_i128 = decimal_to_money_cents(value)?;
    i64::try_from(cents_i128).map_err(|_| TypeError::OutOfRange { target_type: "MONEY" })
}

/// Convert a decimal to the scaled i32 used on the SMALLMONEY wire.
#[cfg(feature = "decimal")]
pub fn decimal_to_smallmoney_cents_i32(value: rust_decimal::Decimal) -> Result<i32, TypeError> {
    let cents_i128 = decimal_to_money_cents(value)?;
    i32::try_from(cents_i128).map_err(|_| TypeError::OutOfRange { target_type: "SMALLMONEY" })
}

/// Encode a decimal as MONEY (8 bytes): the signed 64-bit scaled integer is
/// written as the high 32 bits LE followed by the low 32 bits LE, per
/// MS-TDS §2.2.5.5.1.2.
#[cfg(feature = "decimal")]
pub fn encode_money(value: rust_decimal::Decimal, buf: &mut BytesMut) -> Result<(), TypeError> {
    let cents = decimal_to_money_cents_i64(value)?;
    let high = (cents >> 32) as i32;
    let low = (cents & 0xFFFF_FFFF) as u32;
    buf.put_i32_le(high);
    buf.put_u32_le(low);
    Ok(())
}

/// Encode a decimal as SMALLMONEY (4 bytes): the signed 32-bit scaled integer
/// is written little-endian.
#[cfg(feature = "decimal")]
pub fn encode_smallmoney(
    value: rust_decimal::Decimal,
    buf: &mut BytesMut,
) -> Result<(), TypeError> {
    let cents = decimal_to_smallmoney_cents_i32(value)?;
    buf.put_i32_le(cents);
    Ok(())
}

/// Convert a NaiveDateTime to the DATETIME wire representation.
///
/// Returns `(days_since_1900_i32, ticks_u32)` where each tick is 1/300 second.
/// This is the shared pre-encoding step for both RPC parameter encoding and
/// TVP column encoding.
#[cfg(feature = "chrono")]
pub fn datetime_to_legacy_days_ticks(dt: chrono::NaiveDateTime) -> (i32, u32) {
    use chrono::Timelike;
    let epoch = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("epoch 1900-01-01 is valid");
    let days = (dt.date() - epoch).num_days() as i32;

    let since_midnight = dt.time().num_seconds_from_midnight() as u64 * 1000
        + u64::from(dt.time().nanosecond()) / 1_000_000;
    // Convert ms → 1/300s ticks: ticks = round(ms * 300 / 1000) = round(ms * 3 / 10)
    let ticks = ((since_midnight * 3 + 5) / 10) as u32;
    (days, ticks)
}

/// Encode a DATETIME value (8 bytes): days since 1900 (`i32` LE) + time units
/// since midnight (`u32` LE) where each unit is 1/300 of a second.
#[cfg(feature = "chrono")]
pub fn encode_datetime_legacy(dt: chrono::NaiveDateTime, buf: &mut BytesMut) {
    let (days, ticks) = datetime_to_legacy_days_ticks(dt);
    buf.put_i32_le(days);
    buf.put_u32_le(ticks);
}

/// Convert a NaiveDateTime to the SMALLDATETIME wire representation.
///
/// Returns `(days_since_1900_u16, minutes_since_midnight_u16)`. Seconds are
/// rounded to the nearest minute (30s rounds up per SQL Server semantics); when
/// that rounding lands on or past 24:00 the carry propagates into the next day
/// — e.g. 23:59:45 → next day 00:00 — so the result stays within SQL Server's
/// valid minute range of 0..1439. Returns `Err` if the resulting date is
/// outside the SMALLDATETIME range (1900-01-01 through 2079-06-06).
#[cfg(feature = "chrono")]
pub fn datetime_to_smalldatetime_days_minutes(
    dt: chrono::NaiveDateTime,
) -> Result<(u16, u16), TypeError> {
    use chrono::Timelike;
    let epoch = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("epoch 1900-01-01 is valid");

    let total_seconds = u32::from(dt.time().hour()) * 3600
        + u32::from(dt.time().minute()) * 60
        + u32::from(dt.time().second());
    let minutes_raw = (total_seconds + 30) / 60;
    // Carry over into the next day when seconds round up past 24:00 so that
    // the returned minute count stays within SQL Server's valid 0..1439 range.
    // SQL Server itself does the same thing when casting 23:59:45 to
    // SMALLDATETIME — sending minutes=1440 directly on the wire is rejected
    // as "invalid instance of data type smalldatetime".
    let (day_carry, minutes) = if minutes_raw >= 1440 {
        (1i64, 0u16)
    } else {
        (0i64, minutes_raw as u16)
    };

    let days_i64 = (dt.date() - epoch).num_days() + day_carry;
    let days: u16 = u16::try_from(days_i64).map_err(|_| {
        TypeError::InvalidDateTime(format!(
            "SMALLDATETIME year must be 1900-2079, got date with {days_i64} days since 1900-01-01"
        ))
    })?;

    Ok((days, minutes))
}

/// Encode a SMALLDATETIME value (4 bytes): days since 1900 (`u16` LE) +
/// minutes since midnight (`u16` LE). Seconds are rounded to the nearest
/// minute (30s rounds up per SQL Server semantics).
///
/// Returns an error if the date is outside the SMALLDATETIME range
/// (1900-01-01 through 2079-06-06).
#[cfg(feature = "chrono")]
pub fn encode_smalldatetime(
    dt: chrono::NaiveDateTime,
    buf: &mut BytesMut,
) -> Result<(), TypeError> {
    let (days, minutes) = datetime_to_smalldatetime_days_minutes(dt)?;
    buf.put_u16_le(days);
    buf.put_u16_le(minutes);
    Ok(())
}

/// Encode a DATE value.
///
/// TDS DATE is the number of days since 0001-01-01.
#[cfg(feature = "chrono")]
pub fn encode_date(date: chrono::NaiveDate, buf: &mut BytesMut) {
    // Calculate days since 0001-01-01
    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("valid date");
    let days = date.signed_duration_since(base).num_days() as u32;

    // DATE is encoded as 3 bytes (little-endian)
    buf.put_u8((days & 0xFF) as u8);
    buf.put_u8(((days >> 8) & 0xFF) as u8);
    buf.put_u8(((days >> 16) & 0xFF) as u8);
}

/// Encode a TIME value.
///
/// TDS TIME is encoded as 100-nanosecond intervals since midnight.
#[cfg(feature = "chrono")]
pub fn encode_time(time: chrono::NaiveTime, buf: &mut BytesMut) {
    use chrono::Timelike;

    // Calculate 100-ns intervals since midnight
    // Scale = 7 (100-nanosecond precision)
    let nanos = time.num_seconds_from_midnight() as u64 * 1_000_000_000 + time.nanosecond() as u64;
    let intervals = nanos / 100;

    // TIME with scale 7 uses 5 bytes
    buf.put_u8((intervals & 0xFF) as u8);
    buf.put_u8(((intervals >> 8) & 0xFF) as u8);
    buf.put_u8(((intervals >> 16) & 0xFF) as u8);
    buf.put_u8(((intervals >> 24) & 0xFF) as u8);
    buf.put_u8(((intervals >> 32) & 0xFF) as u8);
}

/// Encode a DATETIME2 value.
///
/// DATETIME2 is encoded as TIME followed by DATE.
#[cfg(feature = "chrono")]
pub fn encode_datetime2(datetime: chrono::NaiveDateTime, buf: &mut BytesMut) {
    encode_time(datetime.time(), buf);
    encode_date(datetime.date(), buf);
}

/// Encode a DATETIMEOFFSET value.
///
/// DATETIMEOFFSET is encoded as TIME + DATE + offset (in minutes).
#[cfg(feature = "chrono")]
pub fn encode_datetimeoffset(datetime: chrono::DateTime<chrono::FixedOffset>, buf: &mut BytesMut) {
    use chrono::Offset;

    // Encode time and date components
    encode_time(datetime.time(), buf);
    encode_date(datetime.date_naive(), buf);

    // Encode timezone offset in minutes (signed 16-bit)
    let offset_seconds = datetime.offset().fix().local_minus_utc();
    let offset_minutes = (offset_seconds / 60) as i16;
    buf.put_i16_le(offset_minutes);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_int() {
        let mut buf = BytesMut::new();
        SqlValue::Int(42).encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[42, 0, 0, 0]);
    }

    #[test]
    fn test_encode_bigint() {
        let mut buf = BytesMut::new();
        SqlValue::BigInt(0x0102030405060708)
            .encode(&mut buf)
            .unwrap();
        assert_eq!(&buf[..], &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_encode_utf16_string() {
        let mut buf = BytesMut::new();
        encode_utf16_string("AB", &mut buf);
        // Length (4 bytes for 2 UTF-16 code units) + "AB" in UTF-16LE
        assert_eq!(&buf[..], &[4, 0, 0x41, 0, 0x42, 0]);
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn test_encode_uuid() {
        let mut buf = BytesMut::new();
        let uuid = uuid::Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        encode_uuid(uuid, &mut buf);
        // SQL Server mixed-endian format
        assert_eq!(
            &buf[..],
            &[
                0x78, 0x56, 0x34, 0x12, // First group reversed
                0x34, 0x12, // Second group reversed
                0x78, 0x56, // Third group reversed
                0x12, 0x34, 0x56, 0x78, 0x12, 0x34, 0x56, 0x78 // Last 8 bytes as-is
            ]
        );
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn test_encode_date() {
        let mut buf = BytesMut::new();
        let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        encode_date(date, &mut buf);
        // Should be 3 bytes representing days since 0001-01-01
        assert_eq!(buf.len(), 3);
    }
}
