//! TDS binary decoding for SQL values.
//!
//! This module provides decoding of TDS wire format data into Rust values.

use bytes::{Buf, Bytes};

use crate::error::TypeError;
use crate::value::SqlValue;

/// Trait for decoding values from TDS binary format.
pub trait TdsDecode: Sized {
    /// Decode a value from the buffer.
    fn decode(buf: &mut Bytes, type_info: &TypeInfo) -> Result<Self, TypeError>;
}

/// TDS type information for decoding.
#[derive(Debug, Clone)]
pub struct TypeInfo {
    /// The TDS type ID.
    pub type_id: u8,
    /// Length/precision for variable-length types.
    pub length: Option<u32>,
    /// Scale for decimal/time types.
    pub scale: Option<u8>,
    /// Precision for decimal types.
    pub precision: Option<u8>,
    /// Collation for string types.
    pub collation: Option<Collation>,
}

/// SQL Server collation information.
#[derive(Debug, Clone, Copy)]
pub struct Collation {
    /// Locale ID.
    pub lcid: u32,
    /// Collation flags.
    pub flags: u8,
}

impl TypeInfo {
    /// Create type info for a fixed-length integer type.
    #[must_use]
    pub fn int(type_id: u8) -> Self {
        Self {
            type_id,
            length: None,
            scale: None,
            precision: None,
            collation: None,
        }
    }

    /// Create type info for a variable-length type.
    #[must_use]
    pub fn varchar(length: u32) -> Self {
        Self {
            type_id: 0xE7, // NVARCHARTYPE
            length: Some(length),
            scale: None,
            precision: None,
            collation: None,
        }
    }

    /// Create type info for a decimal type.
    #[must_use]
    pub fn decimal(precision: u8, scale: u8) -> Self {
        Self {
            type_id: 0x6C,
            length: None,
            scale: Some(scale),
            precision: Some(precision),
            collation: None,
        }
    }

    /// Create type info for a datetime type with scale.
    #[must_use]
    pub fn datetime_with_scale(type_id: u8, scale: u8) -> Self {
        Self {
            type_id,
            length: None,
            scale: Some(scale),
            precision: None,
            collation: None,
        }
    }
}

/// Decode a SQL value based on type information.
pub fn decode_value(buf: &mut Bytes, type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    match type_info.type_id {
        // Fixed-length types
        0x1F => Ok(SqlValue::Null), // NULLTYPE
        0x32 => decode_bit(buf),    // BITTYPE
        0x30 => decode_tinyint(buf), // INT1TYPE
        0x34 => decode_smallint(buf), // INT2TYPE
        0x38 => decode_int(buf),    // INT4TYPE
        0x7F => decode_bigint(buf), // INT8TYPE
        0x3B => decode_float(buf),  // FLT4TYPE
        0x3E => decode_double(buf), // FLT8TYPE

        // Nullable integer types (INTNTYPE)
        0x26 => decode_intn(buf, type_info),

        // Variable-length string types
        0xE7 => decode_nvarchar(buf, type_info), // NVARCHARTYPE
        0xAF => decode_varchar(buf, type_info),  // BIGCHARTYPE
        0xA7 => decode_varchar(buf, type_info),  // BIGVARCHARTYPE

        // Binary types
        0xA5 => decode_varbinary(buf, type_info), // BIGVARBINTYPE
        0xAD => decode_varbinary(buf, type_info), // BIGBINARYTYPE

        // GUID
        0x24 => decode_guid(buf),

        // Decimal/Numeric
        0x6C | 0x6A => decode_decimal(buf, type_info),

        // Date/Time types
        0x28 => decode_date(buf),                     // DATETYPE
        0x29 => decode_time(buf, type_info),          // TIMETYPE
        0x2A => decode_datetime2(buf, type_info),     // DATETIME2TYPE
        0x2B => decode_datetimeoffset(buf, type_info), // DATETIMEOFFSETTYPE
        0x3D => decode_datetime(buf),                 // DATETIMETYPE
        0x3F => decode_smalldatetime(buf),            // SMALLDATETIMETYPE

        // XML
        0xF1 => decode_xml(buf),

        _ => Err(TypeError::UnsupportedConversion {
            from: format!("TDS type 0x{:02X}", type_info.type_id),
            to: "SqlValue",
        }),
    }
}

fn decode_bit(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::Bool(buf.get_u8() != 0))
}

fn decode_tinyint(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::TinyInt(buf.get_u8()))
}

fn decode_smallint(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 2 {
        return Err(TypeError::BufferTooSmall {
            needed: 2,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::SmallInt(buf.get_i16_le()))
}

fn decode_int(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 4 {
        return Err(TypeError::BufferTooSmall {
            needed: 4,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::Int(buf.get_i32_le()))
}

fn decode_bigint(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 8 {
        return Err(TypeError::BufferTooSmall {
            needed: 8,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::BigInt(buf.get_i64_le()))
}

fn decode_float(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 4 {
        return Err(TypeError::BufferTooSmall {
            needed: 4,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::Float(buf.get_f32_le()))
}

fn decode_double(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 8 {
        return Err(TypeError::BufferTooSmall {
            needed: 8,
            available: buf.remaining(),
        });
    }
    Ok(SqlValue::Double(buf.get_f64_le()))
}

fn decode_intn(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let actual_len = buf.get_u8() as usize;
    if actual_len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < actual_len {
        return Err(TypeError::BufferTooSmall {
            needed: actual_len,
            available: buf.remaining(),
        });
    }

    match actual_len {
        1 => Ok(SqlValue::TinyInt(buf.get_u8())),
        2 => Ok(SqlValue::SmallInt(buf.get_i16_le())),
        4 => Ok(SqlValue::Int(buf.get_i32_le())),
        8 => Ok(SqlValue::BigInt(buf.get_i64_le())),
        _ => Err(TypeError::InvalidBinary(format!(
            "invalid INTN length: {actual_len}"
        ))),
    }
}

fn decode_nvarchar(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 2 {
        return Err(TypeError::BufferTooSmall {
            needed: 2,
            available: buf.remaining(),
        });
    }

    let byte_len = buf.get_u16_le() as usize;

    // 0xFFFF indicates NULL
    if byte_len == 0xFFFF {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < byte_len {
        return Err(TypeError::BufferTooSmall {
            needed: byte_len,
            available: buf.remaining(),
        });
    }

    let utf16_data = buf.copy_to_bytes(byte_len);
    let s = decode_utf16_string(&utf16_data)?;
    Ok(SqlValue::String(s))
}

fn decode_varchar(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 2 {
        return Err(TypeError::BufferTooSmall {
            needed: 2,
            available: buf.remaining(),
        });
    }

    let byte_len = buf.get_u16_le() as usize;

    // 0xFFFF indicates NULL
    if byte_len == 0xFFFF {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < byte_len {
        return Err(TypeError::BufferTooSmall {
            needed: byte_len,
            available: buf.remaining(),
        });
    }

    let data = buf.copy_to_bytes(byte_len);
    let s = String::from_utf8(data.to_vec())
        .map_err(|e| TypeError::InvalidEncoding(e.to_string()))?;
    Ok(SqlValue::String(s))
}

fn decode_varbinary(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 2 {
        return Err(TypeError::BufferTooSmall {
            needed: 2,
            available: buf.remaining(),
        });
    }

    let byte_len = buf.get_u16_le() as usize;

    // 0xFFFF indicates NULL
    if byte_len == 0xFFFF {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < byte_len {
        return Err(TypeError::BufferTooSmall {
            needed: byte_len,
            available: buf.remaining(),
        });
    }

    let data = buf.copy_to_bytes(byte_len);
    Ok(SqlValue::Binary(data))
}

#[cfg(feature = "uuid")]
fn decode_guid(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if len != 16 {
        return Err(TypeError::InvalidBinary(format!(
            "invalid GUID length: {len}"
        )));
    }

    if buf.remaining() < 16 {
        return Err(TypeError::BufferTooSmall {
            needed: 16,
            available: buf.remaining(),
        });
    }

    // SQL Server stores UUIDs in mixed-endian format
    let mut bytes = [0u8; 16];

    // First 4 bytes - little-endian (reverse)
    bytes[3] = buf.get_u8();
    bytes[2] = buf.get_u8();
    bytes[1] = buf.get_u8();
    bytes[0] = buf.get_u8();

    // Next 2 bytes - little-endian (reverse)
    bytes[5] = buf.get_u8();
    bytes[4] = buf.get_u8();

    // Next 2 bytes - little-endian (reverse)
    bytes[7] = buf.get_u8();
    bytes[6] = buf.get_u8();

    // Last 8 bytes - big-endian (keep as-is)
    for byte in &mut bytes[8..16] {
        *byte = buf.get_u8();
    }

    Ok(SqlValue::Uuid(uuid::Uuid::from_bytes(bytes)))
}

#[cfg(not(feature = "uuid"))]
fn decode_guid(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    // Skip the GUID data
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    let data = buf.copy_to_bytes(len);
    Ok(SqlValue::Binary(data))
}

#[cfg(feature = "decimal")]
fn decode_decimal(buf: &mut Bytes, type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    use rust_decimal::Decimal;

    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    // First byte is sign (0 = negative, 1 = positive)
    let sign = buf.get_u8();
    let remaining = len - 1;

    // Read mantissa (little-endian)
    let mut mantissa_bytes = [0u8; 16];
    for i in 0..remaining.min(16) {
        mantissa_bytes[i] = buf.get_u8();
    }

    let mantissa = u128::from_le_bytes(mantissa_bytes);
    let scale = type_info.scale.unwrap_or(0) as u32;

    let mut decimal = Decimal::from_i128_with_scale(mantissa as i128, scale);
    if sign == 0 {
        decimal.set_sign_negative(true);
    }

    Ok(SqlValue::Decimal(decimal))
}

#[cfg(not(feature = "decimal"))]
fn decode_decimal(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    // Skip decimal data and return as string
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    buf.advance(len);
    Ok(SqlValue::String("DECIMAL (feature disabled)".to_string()))
}

#[cfg(feature = "chrono")]
fn decode_date(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if len != 3 {
        return Err(TypeError::InvalidDateTime(format!(
            "invalid DATE length: {len}"
        )));
    }

    if buf.remaining() < 3 {
        return Err(TypeError::BufferTooSmall {
            needed: 3,
            available: buf.remaining(),
        });
    }

    // 3 bytes little-endian representing days since 0001-01-01
    let days = buf.get_u8() as u32
        | ((buf.get_u8() as u32) << 8)
        | ((buf.get_u8() as u32) << 16);

    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("valid date");
    let date = base + chrono::Duration::days(days as i64);

    Ok(SqlValue::Date(date))
}

#[cfg(not(feature = "chrono"))]
fn decode_date(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    buf.advance(len);
    Ok(SqlValue::String("DATE (feature disabled)".to_string()))
}

#[cfg(feature = "chrono")]
fn decode_time(buf: &mut Bytes, type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    let scale = type_info.scale.unwrap_or(7);
    let time_len = time_bytes_for_scale(scale);

    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    // Read time bytes (variable length based on scale)
    let mut time_bytes = [0u8; 8];
    for byte in time_bytes.iter_mut().take(time_len) {
        *byte = buf.get_u8();
    }

    let intervals = u64::from_le_bytes(time_bytes);
    let time = intervals_to_time(intervals, scale);

    Ok(SqlValue::Time(time))
}

#[cfg(not(feature = "chrono"))]
fn decode_time(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    buf.advance(len);
    Ok(SqlValue::String("TIME (feature disabled)".to_string()))
}

#[cfg(feature = "chrono")]
fn decode_datetime2(buf: &mut Bytes, type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    let scale = type_info.scale.unwrap_or(7);
    let time_len = time_bytes_for_scale(scale);

    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    // Decode time
    let mut time_bytes = [0u8; 8];
    for byte in time_bytes.iter_mut().take(time_len) {
        *byte = buf.get_u8();
    }
    let intervals = u64::from_le_bytes(time_bytes);
    let time = intervals_to_time(intervals, scale);

    // Decode date
    let days = buf.get_u8() as u32
        | ((buf.get_u8() as u32) << 8)
        | ((buf.get_u8() as u32) << 16);
    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("valid date");
    let date = base + chrono::Duration::days(days as i64);

    Ok(SqlValue::DateTime(date.and_time(time)))
}

#[cfg(not(feature = "chrono"))]
fn decode_datetime2(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    buf.advance(len);
    Ok(SqlValue::String("DATETIME2 (feature disabled)".to_string()))
}

#[cfg(feature = "chrono")]
fn decode_datetimeoffset(buf: &mut Bytes, type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    use chrono::TimeZone;

    let scale = type_info.scale.unwrap_or(7);
    let time_len = time_bytes_for_scale(scale);

    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    // Decode time
    let mut time_bytes = [0u8; 8];
    for byte in time_bytes.iter_mut().take(time_len) {
        *byte = buf.get_u8();
    }
    let intervals = u64::from_le_bytes(time_bytes);
    let time = intervals_to_time(intervals, scale);

    // Decode date
    let days = buf.get_u8() as u32
        | ((buf.get_u8() as u32) << 8)
        | ((buf.get_u8() as u32) << 16);
    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("valid date");
    let date = base + chrono::Duration::days(days as i64);

    // Decode timezone offset in minutes
    let offset_minutes = buf.get_i16_le();
    let offset = chrono::FixedOffset::east_opt((offset_minutes as i32) * 60)
        .ok_or_else(|| TypeError::InvalidDateTime(format!("invalid offset: {offset_minutes}")))?;

    let datetime = offset
        .from_local_datetime(&date.and_time(time))
        .single()
        .ok_or_else(|| TypeError::InvalidDateTime("ambiguous datetime".to_string()))?;

    Ok(SqlValue::DateTimeOffset(datetime))
}

#[cfg(not(feature = "chrono"))]
fn decode_datetimeoffset(buf: &mut Bytes, _type_info: &TypeInfo) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::BufferTooSmall {
            needed: 1,
            available: buf.remaining(),
        });
    }

    let len = buf.get_u8() as usize;
    if len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < len {
        return Err(TypeError::BufferTooSmall {
            needed: len,
            available: buf.remaining(),
        });
    }

    buf.advance(len);
    Ok(SqlValue::String("DATETIMEOFFSET (feature disabled)".to_string()))
}

#[cfg(feature = "chrono")]
fn decode_datetime(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    // DATETIME is 8 bytes: 4 bytes days since 1900-01-01 + 4 bytes 300ths of second
    if buf.remaining() < 8 {
        return Err(TypeError::BufferTooSmall {
            needed: 8,
            available: buf.remaining(),
        });
    }

    let days = buf.get_i32_le();
    let time_300ths = buf.get_u32_le();

    let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("valid date");
    let date = base + chrono::Duration::days(days as i64);

    // Convert 300ths of second to time
    let total_ms = (time_300ths as u64 * 1000) / 300;
    let secs = (total_ms / 1000) as u32;
    let nanos = ((total_ms % 1000) * 1_000_000) as u32;

    let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos)
        .ok_or_else(|| TypeError::InvalidDateTime("invalid DATETIME time".to_string()))?;

    Ok(SqlValue::DateTime(date.and_time(time)))
}

#[cfg(not(feature = "chrono"))]
fn decode_datetime(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 8 {
        return Err(TypeError::BufferTooSmall {
            needed: 8,
            available: buf.remaining(),
        });
    }

    buf.advance(8);
    Ok(SqlValue::String("DATETIME (feature disabled)".to_string()))
}

#[cfg(feature = "chrono")]
fn decode_smalldatetime(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    // SMALLDATETIME is 4 bytes: 2 bytes days since 1900-01-01 + 2 bytes minutes
    if buf.remaining() < 4 {
        return Err(TypeError::BufferTooSmall {
            needed: 4,
            available: buf.remaining(),
        });
    }

    let days = buf.get_u16_le();
    let minutes = buf.get_u16_le();

    let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("valid date");
    let date = base + chrono::Duration::days(days as i64);

    let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt((minutes as u32) * 60, 0)
        .ok_or_else(|| TypeError::InvalidDateTime("invalid SMALLDATETIME time".to_string()))?;

    Ok(SqlValue::DateTime(date.and_time(time)))
}

#[cfg(not(feature = "chrono"))]
fn decode_smalldatetime(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    if buf.remaining() < 4 {
        return Err(TypeError::BufferTooSmall {
            needed: 4,
            available: buf.remaining(),
        });
    }

    buf.advance(4);
    Ok(SqlValue::String("SMALLDATETIME (feature disabled)".to_string()))
}

fn decode_xml(buf: &mut Bytes) -> Result<SqlValue, TypeError> {
    // XML is sent as UTF-16LE string with length prefix
    if buf.remaining() < 2 {
        return Err(TypeError::BufferTooSmall {
            needed: 2,
            available: buf.remaining(),
        });
    }

    let byte_len = buf.get_u16_le() as usize;

    if byte_len == 0xFFFF {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < byte_len {
        return Err(TypeError::BufferTooSmall {
            needed: byte_len,
            available: buf.remaining(),
        });
    }

    let utf16_data = buf.copy_to_bytes(byte_len);
    let s = decode_utf16_string(&utf16_data)?;
    Ok(SqlValue::Xml(s))
}

/// Decode a UTF-16LE string from bytes.
pub fn decode_utf16_string(data: &[u8]) -> Result<String, TypeError> {
    if data.len() % 2 != 0 {
        return Err(TypeError::InvalidEncoding(
            "UTF-16 data must have even length".to_string(),
        ));
    }

    let utf16: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    String::from_utf16(&utf16).map_err(|e| TypeError::InvalidEncoding(e.to_string()))
}

/// Calculate number of bytes needed for TIME based on scale.
#[cfg(feature = "chrono")]
fn time_bytes_for_scale(scale: u8) -> usize {
    match scale {
        0..=2 => 3,
        3..=4 => 4,
        5..=7 => 5,
        _ => 5, // Default to max precision
    }
}

/// Convert 100-nanosecond intervals to NaiveTime.
#[cfg(feature = "chrono")]
fn intervals_to_time(intervals: u64, scale: u8) -> chrono::NaiveTime {
    // Scale determines the unit:
    // scale 0: seconds
    // scale 1: 100ms
    // scale 2: 10ms
    // scale 3: 1ms
    // scale 4: 100us
    // scale 5: 10us
    // scale 6: 1us
    // scale 7: 100ns

    let nanos = match scale {
        0 => intervals * 1_000_000_000,
        1 => intervals * 100_000_000,
        2 => intervals * 10_000_000,
        3 => intervals * 1_000_000,
        4 => intervals * 100_000,
        5 => intervals * 10_000,
        6 => intervals * 1_000,
        7 => intervals * 100,
        _ => intervals * 100,
    };

    let secs = (nanos / 1_000_000_000) as u32;
    let nano_part = (nanos % 1_000_000_000) as u32;

    chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nano_part)
        .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).expect("valid time"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_int() {
        let mut buf = Bytes::from_static(&[42, 0, 0, 0]);
        let type_info = TypeInfo::int(0x38);
        let result = decode_value(&mut buf, &type_info).unwrap();
        assert_eq!(result, SqlValue::Int(42));
    }

    #[test]
    fn test_decode_utf16_string() {
        // "AB" in UTF-16LE
        let data = [0x41, 0x00, 0x42, 0x00];
        let result = decode_utf16_string(&data).unwrap();
        assert_eq!(result, "AB");
    }

    #[test]
    fn test_decode_nvarchar() {
        // Length (4 bytes for "AB") + "AB" in UTF-16LE
        let mut buf = Bytes::from_static(&[4, 0, 0x41, 0x00, 0x42, 0x00]);
        let type_info = TypeInfo::varchar(100);
        let type_info = TypeInfo {
            type_id: 0xE7,
            ..type_info
        };
        let result = decode_value(&mut buf, &type_info).unwrap();
        assert_eq!(result, SqlValue::String("AB".to_string()));
    }

    #[test]
    fn test_decode_null_nvarchar() {
        // 0xFFFF indicates NULL
        let mut buf = Bytes::from_static(&[0xFF, 0xFF]);
        let type_info = TypeInfo {
            type_id: 0xE7,
            length: Some(100),
            scale: None,
            precision: None,
            collation: None,
        };
        let result = decode_value(&mut buf, &type_info).unwrap();
        assert_eq!(result, SqlValue::Null);
    }
}
