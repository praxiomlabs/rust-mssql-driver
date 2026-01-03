//! Table-Valued Parameter (TVP) wire format encoding.
//!
//! This module provides TDS protocol-level encoding for Table-Valued Parameters.
//! TVPs allow passing collections of structured data to SQL Server stored procedures.
//!
//! ## Wire Format
//!
//! TVPs are encoded as type `0xF3` with this structure:
//!
//! ```text
//! TVP_TYPE_INFO = TVPTYPE TVP_TYPENAME TVP_COLMETADATA TVP_END_TOKEN *TVP_ROW TVP_END_TOKEN
//!
//! TVPTYPE = %xF3
//! TVP_TYPENAME = DbName OwningSchema TypeName (all B_VARCHAR)
//! TVP_COLMETADATA = TVP_NULL_TOKEN / (Count TvpColumnMetaData*)
//! TVP_NULL_TOKEN = %xFFFF
//! TvpColumnMetaData = UserType Flags TYPE_INFO ColName
//! TVP_ROW = TVP_ROW_TOKEN AllColumnData
//! TVP_ROW_TOKEN = %x01
//! TVP_END_TOKEN = %x00
//! ```
//!
//! ## Important Constraints
//!
//! - `DbName` MUST be a zero-length string (empty)
//! - `ColName` MUST be a zero-length string in each column definition
//! - TVPs can only be used as input parameters (not output)
//! - Requires TDS 7.3 or later
//!
//! ## References
//!
//! - [MS-TDS 2.2.6.9](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/c264db71-c1ec-4fe8-b5ef-19d54b1e6566)

use bytes::{BufMut, BytesMut};

use crate::codec::write_utf16_string;
use crate::prelude::*;

/// TVP type identifier in TDS.
pub const TVP_TYPE_ID: u8 = 0xF3;

/// Token indicating end of TVP metadata or rows.
pub const TVP_END_TOKEN: u8 = 0x00;

/// Token indicating a TVP row follows.
pub const TVP_ROW_TOKEN: u8 = 0x01;

/// Token indicating no columns (NULL TVP metadata).
pub const TVP_NULL_TOKEN: u16 = 0xFFFF;

/// Default collation for string types in TVPs.
///
/// This is Latin1_General_CI_AS equivalent.
pub const DEFAULT_COLLATION: [u8; 5] = [0x09, 0x04, 0xD0, 0x00, 0x34];

/// TVP column type for wire encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvpWireType {
    /// BIT type.
    Bit,
    /// Integer type with size (1, 2, 4, or 8 bytes).
    Int {
        /// Size in bytes.
        size: u8,
    },
    /// Floating point type with size (4 or 8 bytes).
    Float {
        /// Size in bytes.
        size: u8,
    },
    /// Decimal/Numeric type.
    Decimal {
        /// Maximum number of digits.
        precision: u8,
        /// Number of digits after decimal point.
        scale: u8,
    },
    /// Unicode string (NVARCHAR).
    NVarChar {
        /// Maximum length in bytes. Use 0xFFFF for MAX.
        max_length: u16,
    },
    /// ASCII string (VARCHAR).
    VarChar {
        /// Maximum length in bytes. Use 0xFFFF for MAX.
        max_length: u16,
    },
    /// Binary data (VARBINARY).
    VarBinary {
        /// Maximum length in bytes. Use 0xFFFF for MAX.
        max_length: u16,
    },
    /// UNIQUEIDENTIFIER (UUID).
    Guid,
    /// DATE type.
    Date,
    /// TIME type with scale.
    Time {
        /// Fractional seconds precision (0-7).
        scale: u8,
    },
    /// DATETIME2 type with scale.
    DateTime2 {
        /// Fractional seconds precision (0-7).
        scale: u8,
    },
    /// DATETIMEOFFSET type with scale.
    DateTimeOffset {
        /// Fractional seconds precision (0-7).
        scale: u8,
    },
    /// XML type.
    Xml,
}

impl TvpWireType {
    /// Get the TDS type ID.
    #[must_use]
    pub const fn type_id(&self) -> u8 {
        match self {
            Self::Bit => 0x68,                   // BITNTYPE
            Self::Int { .. } => 0x26,            // INTNTYPE
            Self::Float { .. } => 0x6D,          // FLTNTYPE
            Self::Decimal { .. } => 0x6C,        // DECIMALNTYPE
            Self::NVarChar { .. } => 0xE7,       // NVARCHARTYPE
            Self::VarChar { .. } => 0xA7,        // BIGVARCHARTYPE
            Self::VarBinary { .. } => 0xA5,      // BIGVARBINTYPE
            Self::Guid => 0x24,                  // GUIDTYPE
            Self::Date => 0x28,                  // DATETYPE
            Self::Time { .. } => 0x29,           // TIMETYPE
            Self::DateTime2 { .. } => 0x2A,      // DATETIME2TYPE
            Self::DateTimeOffset { .. } => 0x2B, // DATETIMEOFFSETTYPE
            Self::Xml => 0xF1,                   // XMLTYPE
        }
    }

    /// Encode the TYPE_INFO for this column type.
    pub fn encode_type_info(&self, buf: &mut BytesMut) {
        buf.put_u8(self.type_id());

        match self {
            Self::Bit => {
                buf.put_u8(1); // Max length
            }
            Self::Int { size } | Self::Float { size } => {
                buf.put_u8(*size);
            }
            Self::Decimal { precision, scale } => {
                buf.put_u8(17); // Max length for decimal
                buf.put_u8(*precision);
                buf.put_u8(*scale);
            }
            Self::NVarChar { max_length } => {
                buf.put_u16_le(*max_length);
                buf.put_slice(&DEFAULT_COLLATION);
            }
            Self::VarChar { max_length } => {
                buf.put_u16_le(*max_length);
                buf.put_slice(&DEFAULT_COLLATION);
            }
            Self::VarBinary { max_length } => {
                buf.put_u16_le(*max_length);
            }
            Self::Guid => {
                buf.put_u8(16); // Fixed 16 bytes
            }
            Self::Date => {
                // No additional info needed
            }
            Self::Time { scale } | Self::DateTime2 { scale } | Self::DateTimeOffset { scale } => {
                buf.put_u8(*scale);
            }
            Self::Xml => {
                // XML schema info - we use no schema
                buf.put_u8(0); // No schema collection
            }
        }
    }
}

/// Column flags for TVP columns.
#[derive(Debug, Clone, Copy, Default)]
pub struct TvpColumnFlags {
    /// Column is nullable.
    pub nullable: bool,
}

impl TvpColumnFlags {
    /// Encode flags to 2-byte value.
    #[must_use]
    pub const fn to_bits(&self) -> u16 {
        let mut flags = 0u16;
        if self.nullable {
            flags |= 0x0001;
        }
        flags
    }
}

/// TVP column definition for wire encoding.
#[derive(Debug, Clone)]
pub struct TvpColumnDef {
    /// Column type.
    pub wire_type: TvpWireType,
    /// Column flags.
    pub flags: TvpColumnFlags,
}

impl TvpColumnDef {
    /// Create a new TVP column definition.
    #[must_use]
    pub const fn new(wire_type: TvpWireType) -> Self {
        Self {
            wire_type,
            flags: TvpColumnFlags { nullable: false },
        }
    }

    /// Create a nullable TVP column definition.
    #[must_use]
    pub const fn nullable(wire_type: TvpWireType) -> Self {
        Self {
            wire_type,
            flags: TvpColumnFlags { nullable: true },
        }
    }

    /// Encode the column metadata.
    ///
    /// Format: UserType (4) + Flags (2) + TYPE_INFO + ColName (B_VARCHAR, must be empty)
    pub fn encode(&self, buf: &mut BytesMut) {
        // UserType (always 0 for TVP columns)
        buf.put_u32_le(0);

        // Flags
        buf.put_u16_le(self.flags.to_bits());

        // TYPE_INFO
        self.wire_type.encode_type_info(buf);

        // ColName - MUST be zero-length per MS-TDS spec
        buf.put_u8(0);
    }
}

/// TVP value encoder.
///
/// This provides the complete TVP encoding logic for RPC parameters.
#[derive(Debug)]
pub struct TvpEncoder<'a> {
    /// Database schema (e.g., "dbo"). Empty for default.
    pub schema: &'a str,
    /// Type name as defined in the database.
    pub type_name: &'a str,
    /// Column definitions.
    pub columns: &'a [TvpColumnDef],
}

impl<'a> TvpEncoder<'a> {
    /// Create a new TVP encoder.
    #[must_use]
    pub const fn new(schema: &'a str, type_name: &'a str, columns: &'a [TvpColumnDef]) -> Self {
        Self {
            schema,
            type_name,
            columns,
        }
    }

    /// Encode the complete TVP type info and metadata.
    ///
    /// This encodes:
    /// - TVP type ID (0xF3)
    /// - TVP_TYPENAME (DbName, OwningSchema, TypeName)
    /// - TVP_COLMETADATA
    /// - TVP_END_TOKEN (marks end of column metadata)
    ///
    /// After calling this, use [`Self::encode_row`] for each row, then
    /// [`Self::encode_end`] to finish.
    pub fn encode_metadata(&self, buf: &mut BytesMut) {
        // TVP type ID
        buf.put_u8(TVP_TYPE_ID);

        // TVP_TYPENAME
        // DbName - MUST be empty per MS-TDS spec
        buf.put_u8(0);

        // OwningSchema (B_VARCHAR)
        let schema_len = self.schema.encode_utf16().count() as u8;
        buf.put_u8(schema_len);
        if schema_len > 0 {
            write_utf16_string(buf, self.schema);
        }

        // TypeName (B_VARCHAR)
        let type_len = self.type_name.encode_utf16().count() as u8;
        buf.put_u8(type_len);
        if type_len > 0 {
            write_utf16_string(buf, self.type_name);
        }

        // TVP_COLMETADATA
        if self.columns.is_empty() {
            // No columns - use null token
            buf.put_u16_le(TVP_NULL_TOKEN);
        } else {
            // Column count (2 bytes)
            buf.put_u16_le(self.columns.len() as u16);

            // Encode each column
            for col in self.columns {
                col.encode(buf);
            }
        }

        // Optional: TVP_ORDER_UNIQUE and TVP_COLUMN_ORDERING could go here
        // We don't use them for now

        // TVP_END_TOKEN marks end of metadata
        buf.put_u8(TVP_END_TOKEN);
    }

    /// Encode a TVP row.
    ///
    /// # Arguments
    ///
    /// * `encode_values` - A closure that encodes the column values into the buffer.
    ///   Each value should be encoded according to its type (similar to RPC param encoding).
    pub fn encode_row<F>(&self, buf: &mut BytesMut, encode_values: F)
    where
        F: FnOnce(&mut BytesMut),
    {
        // TVP_ROW_TOKEN
        buf.put_u8(TVP_ROW_TOKEN);

        // AllColumnData - caller provides the value encoding
        encode_values(buf);
    }

    /// Encode the TVP end marker.
    ///
    /// This must be called after all rows have been encoded.
    pub fn encode_end(&self, buf: &mut BytesMut) {
        buf.put_u8(TVP_END_TOKEN);
    }
}

/// Encode a NULL value for a TVP column.
///
/// Different types use different NULL indicators.
pub fn encode_tvp_null(wire_type: &TvpWireType, buf: &mut BytesMut) {
    match wire_type {
        TvpWireType::NVarChar { max_length } | TvpWireType::VarChar { max_length } => {
            if *max_length == 0xFFFF {
                // MAX type uses PLP NULL
                buf.put_u64_le(0xFFFFFFFFFFFFFFFF);
            } else {
                // Regular type uses 0xFFFF
                buf.put_u16_le(0xFFFF);
            }
        }
        TvpWireType::VarBinary { max_length } => {
            if *max_length == 0xFFFF {
                buf.put_u64_le(0xFFFFFFFFFFFFFFFF);
            } else {
                buf.put_u16_le(0xFFFF);
            }
        }
        TvpWireType::Xml => {
            // XML uses PLP NULL
            buf.put_u64_le(0xFFFFFFFFFFFFFFFF);
        }
        _ => {
            // Most types use 0 length
            buf.put_u8(0);
        }
    }
}

/// Encode a BIT value for TVP.
pub fn encode_tvp_bit(value: bool, buf: &mut BytesMut) {
    buf.put_u8(1); // Length
    buf.put_u8(if value { 1 } else { 0 });
}

/// Encode an integer value for TVP.
pub fn encode_tvp_int(value: i64, size: u8, buf: &mut BytesMut) {
    buf.put_u8(size); // Length
    match size {
        1 => buf.put_i8(value as i8),
        2 => buf.put_i16_le(value as i16),
        4 => buf.put_i32_le(value as i32),
        8 => buf.put_i64_le(value),
        _ => unreachable!("invalid int size"),
    }
}

/// Encode a float value for TVP.
pub fn encode_tvp_float(value: f64, size: u8, buf: &mut BytesMut) {
    buf.put_u8(size); // Length
    match size {
        4 => buf.put_f32_le(value as f32),
        8 => buf.put_f64_le(value),
        _ => unreachable!("invalid float size"),
    }
}

/// Encode a NVARCHAR value for TVP.
pub fn encode_tvp_nvarchar(value: &str, max_length: u16, buf: &mut BytesMut) {
    let utf16: Vec<u16> = value.encode_utf16().collect();
    let byte_len = utf16.len() * 2;

    if max_length == 0xFFFF {
        // MAX type - use PLP format
        buf.put_u64_le(byte_len as u64); // Total length
        buf.put_u32_le(byte_len as u32); // Chunk length
        for code_unit in utf16 {
            buf.put_u16_le(code_unit);
        }
        buf.put_u32_le(0); // Terminator
    } else {
        // Regular type
        buf.put_u16_le(byte_len as u16);
        for code_unit in utf16 {
            buf.put_u16_le(code_unit);
        }
    }
}

/// Encode a VARBINARY value for TVP.
pub fn encode_tvp_varbinary(value: &[u8], max_length: u16, buf: &mut BytesMut) {
    if max_length == 0xFFFF {
        // MAX type - use PLP format
        buf.put_u64_le(value.len() as u64);
        buf.put_u32_le(value.len() as u32);
        buf.put_slice(value);
        buf.put_u32_le(0); // Terminator
    } else {
        buf.put_u16_le(value.len() as u16);
        buf.put_slice(value);
    }
}

/// Encode a UNIQUEIDENTIFIER value for TVP.
///
/// SQL Server uses mixed-endian format for UUIDs.
pub fn encode_tvp_guid(uuid_bytes: &[u8; 16], buf: &mut BytesMut) {
    buf.put_u8(16); // Length

    // Mixed-endian: first 3 groups little-endian, last 2 groups big-endian
    buf.put_u8(uuid_bytes[3]);
    buf.put_u8(uuid_bytes[2]);
    buf.put_u8(uuid_bytes[1]);
    buf.put_u8(uuid_bytes[0]);

    buf.put_u8(uuid_bytes[5]);
    buf.put_u8(uuid_bytes[4]);

    buf.put_u8(uuid_bytes[7]);
    buf.put_u8(uuid_bytes[6]);

    buf.put_slice(&uuid_bytes[8..16]);
}

/// Encode a DATE value for TVP (days since 0001-01-01).
pub fn encode_tvp_date(days: u32, buf: &mut BytesMut) {
    // DATE is 3 bytes
    buf.put_u8((days & 0xFF) as u8);
    buf.put_u8(((days >> 8) & 0xFF) as u8);
    buf.put_u8(((days >> 16) & 0xFF) as u8);
}

/// Encode a TIME value for TVP.
///
/// Time is encoded as 100-nanosecond intervals since midnight.
pub fn encode_tvp_time(intervals: u64, scale: u8, buf: &mut BytesMut) {
    // Length depends on scale
    let len = match scale {
        0..=2 => 3,
        3..=4 => 4,
        5..=7 => 5,
        _ => 5,
    };
    buf.put_u8(len);

    for i in 0..len {
        buf.put_u8((intervals >> (8 * i)) as u8);
    }
}

/// Encode a DATETIME2 value for TVP.
///
/// DATETIME2 is TIME followed by DATE.
pub fn encode_tvp_datetime2(time_intervals: u64, days: u32, scale: u8, buf: &mut BytesMut) {
    // Length depends on scale (time bytes + 3 date bytes)
    let time_len = match scale {
        0..=2 => 3,
        3..=4 => 4,
        5..=7 => 5,
        _ => 5,
    };
    buf.put_u8(time_len + 3);

    // Time component
    for i in 0..time_len {
        buf.put_u8((time_intervals >> (8 * i)) as u8);
    }

    // Date component
    buf.put_u8((days & 0xFF) as u8);
    buf.put_u8(((days >> 8) & 0xFF) as u8);
    buf.put_u8(((days >> 16) & 0xFF) as u8);
}

/// Encode a DATETIMEOFFSET value for TVP.
///
/// DATETIMEOFFSET is TIME followed by DATE followed by timezone offset.
///
/// # Arguments
///
/// * `time_intervals` - Time in 100-nanosecond intervals since midnight
/// * `days` - Days since year 1 (0001-01-01)
/// * `offset_minutes` - Timezone offset in minutes (e.g., -480 for UTC-8, 330 for UTC+5:30)
/// * `scale` - Fractional seconds precision (0-7)
pub fn encode_tvp_datetimeoffset(
    time_intervals: u64,
    days: u32,
    offset_minutes: i16,
    scale: u8,
    buf: &mut BytesMut,
) {
    // Length depends on scale (time bytes + 3 date bytes + 2 offset bytes)
    let time_len = match scale {
        0..=2 => 3,
        3..=4 => 4,
        5..=7 => 5,
        _ => 5,
    };
    buf.put_u8(time_len + 3 + 2); // time + date + offset

    // Time component
    for i in 0..time_len {
        buf.put_u8((time_intervals >> (8 * i)) as u8);
    }

    // Date component
    buf.put_u8((days & 0xFF) as u8);
    buf.put_u8(((days >> 8) & 0xFF) as u8);
    buf.put_u8(((days >> 16) & 0xFF) as u8);

    // Timezone offset in minutes (signed 16-bit little-endian)
    buf.put_i16_le(offset_minutes);
}

/// Encode a DECIMAL value for TVP.
///
/// # Arguments
///
/// * `sign` - 0 for negative, 1 for positive
/// * `mantissa` - The absolute value as a 128-bit integer
pub fn encode_tvp_decimal(sign: u8, mantissa: u128, buf: &mut BytesMut) {
    buf.put_u8(17); // Length: 1 byte sign + 16 bytes mantissa
    buf.put_u8(sign);
    buf.put_u128_le(mantissa);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_tvp_metadata_encoding() {
        let columns = vec![TvpColumnDef::new(TvpWireType::Int { size: 4 })];

        let encoder = TvpEncoder::new("dbo", "UserIdList", &columns);
        let mut buf = BytesMut::new();

        encoder.encode_metadata(&mut buf);

        // Should start with TVP type ID
        assert_eq!(buf[0], TVP_TYPE_ID);

        // DbName should be empty (length 0)
        assert_eq!(buf[1], 0);
    }

    #[test]
    fn test_tvp_column_def_encoding() {
        let col = TvpColumnDef::nullable(TvpWireType::Int { size: 4 });
        let mut buf = BytesMut::new();

        col.encode(&mut buf);

        // UserType (4) + Flags (2) + TypeId (1) + MaxLen (1) + ColName (1)
        assert!(buf.len() >= 9);

        // UserType should be 0
        assert_eq!(&buf[0..4], &[0, 0, 0, 0]);

        // Flags should have nullable bit set
        assert_eq!(buf[4], 0x01);
        assert_eq!(buf[5], 0x00);
    }

    #[test]
    fn test_tvp_nvarchar_encoding() {
        let mut buf = BytesMut::new();
        encode_tvp_nvarchar("test", 100, &mut buf);

        // Length prefix (2) + UTF-16 data (4 chars * 2 bytes)
        assert_eq!(buf.len(), 2 + 8);
        assert_eq!(buf[0], 8); // Byte length
        assert_eq!(buf[1], 0);
    }

    #[test]
    fn test_tvp_int_encoding() {
        let mut buf = BytesMut::new();
        encode_tvp_int(42, 4, &mut buf);

        // Length (1) + value (4)
        assert_eq!(buf.len(), 5);
        assert_eq!(buf[0], 4);
        assert_eq!(buf[1], 42);
    }
}
