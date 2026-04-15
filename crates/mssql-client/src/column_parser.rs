//! Column value parsing for TDS row data.
//!
//! This module contains all logic for parsing SQL Server column values from
//! raw TDS wire bytes into `SqlValue` types. It handles:
//!
//! - Fixed-length types (INT, BIGINT, FLOAT, BIT, etc.)
//! - Variable-length nullable types (IntN, FloatN, BitN, etc.)
//! - Date/time types (DATE, TIME, DATETIME, DATETIME2, DATETIMEOFFSET)
//! - String types (VARCHAR, NVARCHAR, CHAR, NCHAR, TEXT, NTEXT)
//! - Binary types (VARBINARY, BINARY, IMAGE)
//! - Decimal/Numeric types
//! - Money types (MONEY, SMALLMONEY)
//! - PLP (Partially Length-Prefixed) encoding for MAX types
//! - SQL_VARIANT with embedded type information
//! - XML, GUID, UDT
//!
//! All functions are pure (no `self` parameter) and operate on byte buffers
//! with TDS column metadata.

// Allow unwrap/expect for chrono date construction with known-valid constant dates
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::needless_range_loop)]

use bytes::Buf;
use mssql_types::SqlValue;
use tds_protocol::token::{ColMetaData, Collation, ColumnData, NbcRow, RawRow};
use tds_protocol::types::TypeId;

use crate::error::{Error, Result};

/// Convert a RawRow to a client Row.
///
/// This parses the raw bytes back into SqlValue types based on column metadata.
pub(crate) fn convert_raw_row(
    raw: &RawRow,
    meta: &ColMetaData,
    columns: &[crate::row::Column],
) -> Result<crate::row::Row> {
    let mut values = Vec::with_capacity(meta.columns.len());
    let mut buf = raw.data.as_ref();

    for col in &meta.columns {
        let value = parse_column_value(&mut buf, col)?;
        values.push(value);
    }

    Ok(crate::row::Row::from_values(columns.to_vec(), values))
}

/// Convert an NbcRow to a client Row.
///
/// NbcRow has a null bitmap followed by only non-null values.
pub(crate) fn convert_nbc_row(
    nbc: &NbcRow,
    meta: &ColMetaData,
    columns: &[crate::row::Column],
) -> Result<crate::row::Row> {
    let mut values = Vec::with_capacity(meta.columns.len());
    let mut buf = nbc.data.as_ref();

    for (i, col) in meta.columns.iter().enumerate() {
        if nbc.is_null(i) {
            values.push(mssql_types::SqlValue::Null);
        } else {
            let value = parse_column_value(&mut buf, col)?;
            values.push(value);
        }
    }

    Ok(crate::row::Row::from_values(columns.to_vec(), values))
}

/// Convert a RawRow to a client Row with Always Encrypted decryption.
///
/// For encrypted columns (identified by the decryptor), this:
/// 1. Parses the column value as raw bytes (BigVarBinary on the wire)
/// 2. Decrypts the ciphertext using the pre-resolved encryptor
/// 3. Re-parses the decrypted plaintext using the base column type
///
/// For unencrypted columns, this delegates to `parse_column_value` as normal.
#[cfg(feature = "always-encrypted")]
pub(crate) fn convert_raw_row_decrypted(
    raw: &RawRow,
    meta: &ColMetaData,
    columns: &[crate::row::Column],
    decryptor: &crate::column_decryptor::ColumnDecryptor,
) -> Result<crate::row::Row> {
    let mut values = Vec::with_capacity(meta.columns.len());
    let mut buf = raw.data.as_ref();

    for (i, col) in meta.columns.iter().enumerate() {
        let value = if decryptor.is_encrypted(i) {
            decrypt_column(&mut buf, col, decryptor, i)?
        } else {
            parse_column_value(&mut buf, col)?
        };
        values.push(value);
    }

    Ok(crate::row::Row::from_values(columns.to_vec(), values))
}

/// Convert an NbcRow to a client Row with Always Encrypted decryption.
///
/// Same as `convert_raw_row_decrypted` but handles the null bitmap.
#[cfg(feature = "always-encrypted")]
pub(crate) fn convert_nbc_row_decrypted(
    nbc: &NbcRow,
    meta: &ColMetaData,
    columns: &[crate::row::Column],
    decryptor: &crate::column_decryptor::ColumnDecryptor,
) -> Result<crate::row::Row> {
    let mut values = Vec::with_capacity(meta.columns.len());
    let mut buf = nbc.data.as_ref();

    for (i, col) in meta.columns.iter().enumerate() {
        if nbc.is_null(i) {
            values.push(SqlValue::Null);
        } else {
            let value = if decryptor.is_encrypted(i) {
                decrypt_column(&mut buf, col, decryptor, i)?
            } else {
                parse_column_value(&mut buf, col)?
            };
            values.push(value);
        }
    }

    Ok(crate::row::Row::from_values(columns.to_vec(), values))
}

/// Decrypt an encrypted column value and re-parse it as the plaintext type.
///
/// Encrypted columns are transmitted as BigVarBinary (2-byte length prefix).
/// This function reads the ciphertext, decrypts it, then re-parses the
/// plaintext bytes using the base column type from CryptoMetadata.
#[cfg(feature = "always-encrypted")]
fn decrypt_column(
    buf: &mut &[u8],
    _col: &ColumnData,
    decryptor: &crate::column_decryptor::ColumnDecryptor,
    ordinal: usize,
) -> Result<SqlValue> {
    // Encrypted column wire type is BigVarBinary: 2-byte length prefix.
    // 0xFFFF = NULL, otherwise length followed by ciphertext bytes.
    if buf.remaining() < 2 {
        return Err(Error::Protocol(
            "unexpected EOF reading encrypted column length".to_string(),
        ));
    }

    let length = buf.get_u16_le();

    if length == 0xFFFF {
        // NULL encrypted value
        return Ok(SqlValue::Null);
    }

    let length = length as usize;
    if buf.remaining() < length {
        return Err(Error::Protocol(format!(
            "unexpected EOF reading encrypted column data: need {length} bytes, have {}",
            buf.remaining()
        )));
    }

    // Extract ciphertext bytes
    let ciphertext = &buf[..length];
    buf.advance(length);

    // Decrypt and get the base column metadata
    let (plaintext, base_col) = decryptor.decrypt_column_value(ordinal, ciphertext)?;

    // Re-parse the decrypted plaintext using the base column's type info
    let mut pt_buf: &[u8] = &plaintext;
    parse_column_value(&mut pt_buf, base_col)
}

/// Parse money value from buffer and convert to appropriate type.
///
/// Money is stored as fixed-point with 4 decimal places.
/// - 4 bytes: SMALLMONEY
/// - 8 bytes: MONEY
fn parse_money_value(buf: &mut &[u8], bytes: usize) -> Result<SqlValue> {
    if bytes == 0 {
        return Ok(SqlValue::Null);
    }

    let cents = match bytes {
        4 => buf.get_i32_le() as i64,
        8 => {
            let high = buf.get_i32_le();
            let low = buf.get_u32_le();
            ((high as i64) << 32) | (low as i64)
        }
        _ => return Err(Error::Protocol(format!("invalid money length: {bytes}"))),
    };

    #[cfg(feature = "decimal")]
    {
        use rust_decimal::Decimal;
        Ok(SqlValue::Decimal(Decimal::from_i128_with_scale(
            cents as i128,
            4,
        )))
    }

    #[cfg(not(feature = "decimal"))]
    {
        Ok(SqlValue::Double((cents as f64) / 10000.0))
    }
}

/// Parse a single column value from a buffer based on column metadata.
pub(crate) fn parse_column_value(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    let value = match col.type_id {
        // Fixed-length null type
        TypeId::Null => SqlValue::Null,

        // 1-byte types
        TypeId::Int1 => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading TINYINT".into()));
            }
            SqlValue::TinyInt(buf.get_u8())
        }
        TypeId::Bit => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading BIT".into()));
            }
            SqlValue::Bool(buf.get_u8() != 0)
        }

        // 2-byte types
        TypeId::Int2 => {
            if buf.remaining() < 2 {
                return Err(Error::Protocol("unexpected EOF reading SMALLINT".into()));
            }
            SqlValue::SmallInt(buf.get_i16_le())
        }

        // 4-byte types
        TypeId::Int4 => {
            if buf.remaining() < 4 {
                return Err(Error::Protocol("unexpected EOF reading INT".into()));
            }
            SqlValue::Int(buf.get_i32_le())
        }
        TypeId::Float4 => {
            if buf.remaining() < 4 {
                return Err(Error::Protocol("unexpected EOF reading REAL".into()));
            }
            SqlValue::Float(buf.get_f32_le())
        }

        // 8-byte types
        TypeId::Int8 => {
            if buf.remaining() < 8 {
                return Err(Error::Protocol("unexpected EOF reading BIGINT".into()));
            }
            SqlValue::BigInt(buf.get_i64_le())
        }
        TypeId::Float8 => {
            if buf.remaining() < 8 {
                return Err(Error::Protocol("unexpected EOF reading FLOAT".into()));
            }
            SqlValue::Double(buf.get_f64_le())
        }

        // Money types (fixed-point with 4 decimal places)
        TypeId::Money | TypeId::Money4 | TypeId::MoneyN => {
            let bytes = match col.type_id {
                TypeId::Money => 8,
                TypeId::Money4 => 4,
                TypeId::MoneyN => {
                    if buf.remaining() < 1 {
                        return Err(Error::Protocol(
                            "unexpected EOF reading MoneyN length".into(),
                        ));
                    }
                    buf.get_u8() as usize
                }
                // The outer match arm restricts col.type_id to Money | Money4 | MoneyN,
                // so this branch is unreachable.
                _ => unreachable!("inner match is bounded by outer Money|Money4|MoneyN arm"),
            };

            if buf.remaining() < bytes {
                return Err(Error::Protocol(format!(
                    "unexpected EOF reading money data ({bytes} bytes)"
                )));
            }

            parse_money_value(buf, bytes)?
        }

        // Variable-length nullable types (IntN, FloatN, etc.)
        TypeId::IntN => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading IntN length".into()));
            }
            let len = buf.get_u8();
            match len {
                0 => SqlValue::Null,
                1 => SqlValue::TinyInt(buf.get_u8()),
                2 => SqlValue::SmallInt(buf.get_i16_le()),
                4 => SqlValue::Int(buf.get_i32_le()),
                8 => SqlValue::BigInt(buf.get_i64_le()),
                _ => {
                    return Err(Error::Protocol(format!("invalid IntN length: {len}")));
                }
            }
        }
        TypeId::FloatN => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading FloatN length".into(),
                ));
            }
            let len = buf.get_u8();
            match len {
                0 => SqlValue::Null,
                4 => SqlValue::Float(buf.get_f32_le()),
                8 => SqlValue::Double(buf.get_f64_le()),
                _ => {
                    return Err(Error::Protocol(format!("invalid FloatN length: {len}")));
                }
            }
        }
        TypeId::BitN => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading BitN length".into()));
            }
            let len = buf.get_u8();
            match len {
                0 => SqlValue::Null,
                1 => SqlValue::Bool(buf.get_u8() != 0),
                _ => {
                    return Err(Error::Protocol(format!("invalid BitN length: {len}")));
                }
            }
        }

        // DECIMAL/NUMERIC types (1-byte length prefix)
        TypeId::Decimal | TypeId::Numeric | TypeId::DecimalN | TypeId::NumericN => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading DECIMAL/NUMERIC length".into(),
                ));
            }
            let len = buf.get_u8() as usize;
            if len == 0 {
                SqlValue::Null
            } else {
                if buf.remaining() < len {
                    return Err(Error::Protocol(
                        "unexpected EOF reading DECIMAL/NUMERIC data".into(),
                    ));
                }

                // First byte is sign: 0 = negative, 1 = positive
                let sign = buf.get_u8();
                let mantissa_len = len - 1;

                // Read mantissa as little-endian integer (up to 16 bytes for max precision 38)
                let mut mantissa_bytes = [0u8; 16];
                for i in 0..mantissa_len.min(16) {
                    mantissa_bytes[i] = buf.get_u8();
                }
                // Skip any excess bytes (shouldn't happen with valid data)
                for _ in 16..mantissa_len {
                    buf.get_u8();
                }

                let mantissa = u128::from_le_bytes(mantissa_bytes);
                let scale = col.type_info.scale.unwrap_or(0) as u32;

                #[cfg(feature = "decimal")]
                {
                    use rust_decimal::Decimal;
                    // rust_decimal supports max scale of 28
                    // For scales > 28, fall back to f64 to avoid overflow/hang
                    if scale > 28 {
                        // Fall back to f64 for high-scale decimals
                        let divisor = 10f64.powi(scale as i32);
                        let value = (mantissa as f64) / divisor;
                        let value = if sign == 0 { -value } else { value };
                        SqlValue::Double(value)
                    } else {
                        let mut decimal = Decimal::from_i128_with_scale(mantissa as i128, scale);
                        if sign == 0 {
                            decimal.set_sign_negative(true);
                        }
                        SqlValue::Decimal(decimal)
                    }
                }

                #[cfg(not(feature = "decimal"))]
                {
                    // Without the decimal feature, convert to f64
                    let divisor = 10f64.powi(scale as i32);
                    let value = (mantissa as f64) / divisor;
                    let value = if sign == 0 { -value } else { value };
                    SqlValue::Double(value)
                }
            }
        }

        // DATETIME/SMALLDATETIME nullable (1-byte length prefix)
        TypeId::DateTimeN => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading DateTimeN length".into(),
                ));
            }
            let len = buf.get_u8() as usize;
            if len == 0 {
                SqlValue::Null
            } else if buf.remaining() < len {
                return Err(Error::Protocol("unexpected EOF reading DateTimeN".into()));
            } else {
                match len {
                    4 => {
                        // SMALLDATETIME: 2 bytes days + 2 bytes minutes
                        let days = buf.get_u16_le() as i64;
                        let minutes = buf.get_u16_le() as u32;
                        #[cfg(feature = "chrono")]
                        {
                            let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1)
                                .expect("epoch 1900-01-01 is valid");
                            let date = base + chrono::Duration::days(days);
                            let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
                                minutes * 60,
                                0,
                            )
                            .expect("SMALLDATETIME minutes should be 0-1439");
                            SqlValue::DateTime(date.and_time(time))
                        }
                        #[cfg(not(feature = "chrono"))]
                        {
                            SqlValue::String(format!("SMALLDATETIME({days},{minutes})"))
                        }
                    }
                    8 => {
                        // DATETIME: 4 bytes days + 4 bytes 1/300ths of second
                        let days = buf.get_i32_le() as i64;
                        let time_300ths = buf.get_u32_le() as u64;
                        #[cfg(feature = "chrono")]
                        {
                            let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1)
                                .expect("epoch 1900-01-01 is valid");
                            let date = base + chrono::Duration::days(days);
                            // Convert 300ths of second to nanoseconds
                            let total_ms = (time_300ths * 1000) / 300;
                            let secs = (total_ms / 1000) as u32;
                            let nanos = ((total_ms % 1000) * 1_000_000) as u32;
                            let time =
                                chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos)
                                    .expect("DATETIME time component should be valid");
                            SqlValue::DateTime(date.and_time(time))
                        }
                        #[cfg(not(feature = "chrono"))]
                        {
                            SqlValue::String(format!("DATETIME({days},{time_300ths})"))
                        }
                    }
                    _ => {
                        return Err(Error::Protocol(format!("invalid DateTimeN length: {len}")));
                    }
                }
            }
        }

        // Fixed DATETIME (8 bytes)
        TypeId::DateTime => {
            if buf.remaining() < 8 {
                return Err(Error::Protocol("unexpected EOF reading DATETIME".into()));
            }
            let days = buf.get_i32_le() as i64;
            let time_300ths = buf.get_u32_le() as u64;
            #[cfg(feature = "chrono")]
            {
                let base =
                    chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("epoch 1900-01-01 is valid");
                let date = base + chrono::Duration::days(days);
                let total_ms = (time_300ths * 1000) / 300;
                let secs = (total_ms / 1000) as u32;
                let nanos = ((total_ms % 1000) * 1_000_000) as u32;
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos)
                    .expect("DATETIME time component should be valid");
                SqlValue::DateTime(date.and_time(time))
            }
            #[cfg(not(feature = "chrono"))]
            {
                SqlValue::String(format!("DATETIME({days},{time_300ths})"))
            }
        }

        // Fixed SMALLDATETIME (4 bytes)
        TypeId::DateTime4 => {
            if buf.remaining() < 4 {
                return Err(Error::Protocol(
                    "unexpected EOF reading SMALLDATETIME".into(),
                ));
            }
            let days = buf.get_u16_le() as i64;
            let minutes = buf.get_u16_le() as u32;
            #[cfg(feature = "chrono")]
            {
                let base =
                    chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("epoch 1900-01-01 is valid");
                let date = base + chrono::Duration::days(days);
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(minutes * 60, 0)
                    .expect("SMALLDATETIME minutes should be 0-1439");
                SqlValue::DateTime(date.and_time(time))
            }
            #[cfg(not(feature = "chrono"))]
            {
                SqlValue::String(format!("SMALLDATETIME({days},{minutes})"))
            }
        }

        // DATE (3 bytes, nullable with 1-byte length prefix)
        TypeId::Date => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading DATE length".into()));
            }
            let len = buf.get_u8() as usize;
            if len == 0 {
                SqlValue::Null
            } else if len != 3 {
                return Err(Error::Protocol(format!("invalid DATE length: {len}")));
            } else if buf.remaining() < 3 {
                return Err(Error::Protocol("unexpected EOF reading DATE".into()));
            } else {
                // 3 bytes little-endian days since 0001-01-01
                let days = buf.get_u8() as u32
                    | ((buf.get_u8() as u32) << 8)
                    | ((buf.get_u8() as u32) << 16);
                #[cfg(feature = "chrono")]
                {
                    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1)
                        .expect("epoch 0001-01-01 is valid");
                    let date = base + chrono::Duration::days(days as i64);
                    SqlValue::Date(date)
                }
                #[cfg(not(feature = "chrono"))]
                {
                    SqlValue::String(format!("DATE({days})"))
                }
            }
        }

        // TIME (variable length with scale, 1-byte length prefix)
        TypeId::Time => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading TIME length".into()));
            }
            let len = buf.get_u8() as usize;
            if len == 0 {
                SqlValue::Null
            } else if buf.remaining() < len {
                return Err(Error::Protocol("unexpected EOF reading TIME".into()));
            } else {
                let mut time_bytes = [0u8; 8];
                for byte in time_bytes.iter_mut().take(len) {
                    *byte = buf.get_u8();
                }
                let intervals = u64::from_le_bytes(time_bytes);
                #[cfg(feature = "chrono")]
                {
                    let scale = col.type_info.scale.unwrap_or(7);
                    let time = intervals_to_time(intervals, scale);
                    SqlValue::Time(time)
                }
                #[cfg(not(feature = "chrono"))]
                {
                    SqlValue::String(format!("TIME({intervals})"))
                }
            }
        }

        // DATETIME2 (variable length: TIME bytes + 3 bytes date, 1-byte length prefix)
        TypeId::DateTime2 => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading DATETIME2 length".into(),
                ));
            }
            let len = buf.get_u8() as usize;
            if len == 0 {
                SqlValue::Null
            } else if buf.remaining() < len {
                return Err(Error::Protocol("unexpected EOF reading DATETIME2".into()));
            } else {
                let scale = col.type_info.scale.unwrap_or(7);
                let time_len = time_bytes_for_scale(scale);

                // Read time
                let mut time_bytes = [0u8; 8];
                for byte in time_bytes.iter_mut().take(time_len) {
                    *byte = buf.get_u8();
                }
                let intervals = u64::from_le_bytes(time_bytes);

                // Read date (3 bytes)
                let days = buf.get_u8() as u32
                    | ((buf.get_u8() as u32) << 8)
                    | ((buf.get_u8() as u32) << 16);

                #[cfg(feature = "chrono")]
                {
                    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1)
                        .expect("epoch 0001-01-01 is valid");
                    let date = base + chrono::Duration::days(days as i64);
                    let time = intervals_to_time(intervals, scale);
                    SqlValue::DateTime(date.and_time(time))
                }
                #[cfg(not(feature = "chrono"))]
                {
                    SqlValue::String(format!("DATETIME2({days},{intervals})"))
                }
            }
        }

        // DATETIMEOFFSET (variable length: TIME bytes + 3 bytes date + 2 bytes offset)
        TypeId::DateTimeOffset => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading DATETIMEOFFSET length".into(),
                ));
            }
            let len = buf.get_u8() as usize;
            if len == 0 {
                SqlValue::Null
            } else if buf.remaining() < len {
                return Err(Error::Protocol(
                    "unexpected EOF reading DATETIMEOFFSET".into(),
                ));
            } else {
                let scale = col.type_info.scale.unwrap_or(7);
                let time_len = time_bytes_for_scale(scale);

                // Read time
                let mut time_bytes = [0u8; 8];
                for byte in time_bytes.iter_mut().take(time_len) {
                    *byte = buf.get_u8();
                }
                let intervals = u64::from_le_bytes(time_bytes);

                // Read date (3 bytes)
                let days = buf.get_u8() as u32
                    | ((buf.get_u8() as u32) << 8)
                    | ((buf.get_u8() as u32) << 16);

                // Read offset in minutes (2 bytes, signed)
                let offset_minutes = buf.get_i16_le();

                #[cfg(feature = "chrono")]
                {
                    use chrono::TimeZone;
                    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1)
                        .expect("epoch 0001-01-01 is valid");
                    let date = base + chrono::Duration::days(days as i64);
                    let time = intervals_to_time(intervals, scale);
                    let offset = chrono::FixedOffset::east_opt((offset_minutes as i32) * 60)
                        .unwrap_or_else(|| {
                            chrono::FixedOffset::east_opt(0).expect("UTC offset 0 is valid")
                        });
                    let datetime = offset
                        .from_local_datetime(&date.and_time(time))
                        .single()
                        .unwrap_or_else(|| offset.from_utc_datetime(&date.and_time(time)));
                    SqlValue::DateTimeOffset(datetime)
                }
                #[cfg(not(feature = "chrono"))]
                {
                    SqlValue::String(format!(
                        "DATETIMEOFFSET({days},{intervals},{offset_minutes})"
                    ))
                }
            }
        }

        // TEXT type - always uses PLP encoding (deprecated LOB type)
        TypeId::Text => parse_plp_varchar(buf, col.type_info.collation.as_ref())?,

        // Legacy byte-length string types (Char, VarChar) - 1-byte length prefix
        TypeId::Char | TypeId::VarChar => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading legacy varchar length".into(),
                ));
            }
            let len = buf.get_u8();
            if len == 0xFF {
                SqlValue::Null
            } else if len == 0 {
                SqlValue::String(String::new())
            } else if buf.remaining() < len as usize {
                return Err(Error::Protocol(
                    "unexpected EOF reading legacy varchar data".into(),
                ));
            } else {
                let data = &buf[..len as usize];
                // Use collation-aware decoding for non-ASCII text
                let s = decode_varchar_string(data, col.type_info.collation.as_ref());
                buf.advance(len as usize);
                SqlValue::String(s)
            }
        }

        // Variable-length string types (BigVarChar, BigChar)
        TypeId::BigVarChar | TypeId::BigChar => {
            // Check if this is a MAX type (uses PLP encoding)
            if col.type_info.max_length == Some(0xFFFF) {
                // PLP format: 8-byte total length, then chunks
                parse_plp_varchar(buf, col.type_info.collation.as_ref())?
            } else {
                // 2-byte length prefix for non-MAX types
                if buf.remaining() < 2 {
                    return Err(Error::Protocol(
                        "unexpected EOF reading varchar length".into(),
                    ));
                }
                let len = buf.get_u16_le();
                if len == 0xFFFF {
                    SqlValue::Null
                } else if buf.remaining() < len as usize {
                    return Err(Error::Protocol(
                        "unexpected EOF reading varchar data".into(),
                    ));
                } else {
                    let data = &buf[..len as usize];
                    // Use collation-aware decoding for non-ASCII text
                    let s = decode_varchar_string(data, col.type_info.collation.as_ref());
                    buf.advance(len as usize);
                    SqlValue::String(s)
                }
            }
        }

        // NTEXT type - always uses PLP encoding (deprecated LOB type)
        TypeId::NText => parse_plp_nvarchar(buf)?,

        // Variable-length Unicode string types (NVarChar, NChar)
        TypeId::NVarChar | TypeId::NChar => {
            // Check if this is a MAX type (uses PLP encoding)
            if col.type_info.max_length == Some(0xFFFF) {
                // PLP format: 8-byte total length, then chunks
                parse_plp_nvarchar(buf)?
            } else {
                // 2-byte length prefix (in bytes, not chars) for non-MAX types
                if buf.remaining() < 2 {
                    return Err(Error::Protocol(
                        "unexpected EOF reading nvarchar length".into(),
                    ));
                }
                let len = buf.get_u16_le();
                if len == 0xFFFF {
                    SqlValue::Null
                } else if buf.remaining() < len as usize {
                    return Err(Error::Protocol(
                        "unexpected EOF reading nvarchar data".into(),
                    ));
                } else {
                    let data = &buf[..len as usize];
                    // UTF-16LE to String
                    let utf16: Vec<u16> = data
                        .chunks_exact(2)
                        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                        .collect();
                    let s = String::from_utf16(&utf16)
                        .map_err(|_| Error::Protocol("invalid UTF-16 in nvarchar".into()))?;
                    buf.advance(len as usize);
                    SqlValue::String(s)
                }
            }
        }

        // IMAGE type - always uses PLP encoding (deprecated LOB type)
        TypeId::Image => parse_plp_varbinary(buf)?,

        // Legacy byte-length binary types (Binary, VarBinary) - 1-byte length prefix
        TypeId::Binary | TypeId::VarBinary => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol(
                    "unexpected EOF reading legacy varbinary length".into(),
                ));
            }
            let len = buf.get_u8();
            if len == 0xFF {
                SqlValue::Null
            } else if len == 0 {
                SqlValue::Binary(bytes::Bytes::new())
            } else if buf.remaining() < len as usize {
                return Err(Error::Protocol(
                    "unexpected EOF reading legacy varbinary data".into(),
                ));
            } else {
                let data = bytes::Bytes::copy_from_slice(&buf[..len as usize]);
                buf.advance(len as usize);
                SqlValue::Binary(data)
            }
        }

        // Variable-length binary types (BigVarBinary, BigBinary)
        TypeId::BigVarBinary | TypeId::BigBinary => {
            // Check if this is a MAX type (uses PLP encoding)
            if col.type_info.max_length == Some(0xFFFF) {
                // PLP format: 8-byte total length, then chunks
                parse_plp_varbinary(buf)?
            } else {
                if buf.remaining() < 2 {
                    return Err(Error::Protocol(
                        "unexpected EOF reading varbinary length".into(),
                    ));
                }
                let len = buf.get_u16_le();
                if len == 0xFFFF {
                    SqlValue::Null
                } else if buf.remaining() < len as usize {
                    return Err(Error::Protocol(
                        "unexpected EOF reading varbinary data".into(),
                    ));
                } else {
                    let data = bytes::Bytes::copy_from_slice(&buf[..len as usize]);
                    buf.advance(len as usize);
                    SqlValue::Binary(data)
                }
            }
        }

        // XML type - always uses PLP encoding
        TypeId::Xml => {
            // Parse as PLP NVARCHAR (XML is UTF-16 encoded in TDS)
            match parse_plp_nvarchar(buf)? {
                SqlValue::Null => SqlValue::Null,
                SqlValue::String(s) => SqlValue::Xml(s),
                _ => {
                    return Err(Error::Protocol(
                        "unexpected value type when parsing XML".into(),
                    ));
                }
            }
        }

        // GUID/UniqueIdentifier
        TypeId::Guid => {
            if buf.remaining() < 1 {
                return Err(Error::Protocol("unexpected EOF reading GUID length".into()));
            }
            let len = buf.get_u8();
            if len == 0 {
                SqlValue::Null
            } else if len != 16 {
                return Err(Error::Protocol(format!("invalid GUID length: {len}")));
            } else if buf.remaining() < 16 {
                return Err(Error::Protocol("unexpected EOF reading GUID".into()));
            } else {
                // SQL Server stores GUIDs in mixed-endian format
                let data = bytes::Bytes::copy_from_slice(&buf[..16]);
                buf.advance(16);
                SqlValue::Binary(data)
            }
        }

        // SQL_VARIANT - contains embedded type info
        TypeId::Variant => parse_sql_variant(buf)?,

        // UDT (User-Defined Type) - uses PLP encoding, return as binary
        TypeId::Udt => parse_plp_varbinary(buf)?,

        // Default: treat as binary with 2-byte length prefix
        _ => {
            // Try to read as variable-length with 2-byte length
            if buf.remaining() < 2 {
                return Err(Error::Protocol(format!(
                    "unexpected EOF reading {:?}",
                    col.type_id
                )));
            }
            let len = buf.get_u16_le();
            if len == 0xFFFF {
                SqlValue::Null
            } else if buf.remaining() < len as usize {
                return Err(Error::Protocol(format!(
                    "unexpected EOF reading {:?} data",
                    col.type_id
                )));
            } else {
                let data = bytes::Bytes::copy_from_slice(&buf[..len as usize]);
                buf.advance(len as usize);
                SqlValue::Binary(data)
            }
        }
    };

    Ok(value)
}

/// Parse PLP-encoded NVARCHAR(MAX) data.
///
/// PLP format stored by decode_plp_type:
/// - 8-byte total length (0xFFFFFFFFFFFFFFFF = NULL)
/// - Chunks: 4-byte chunk length + chunk data, terminated by 0 length
pub(crate) fn parse_plp_nvarchar(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 8 {
        return Err(Error::Protocol(
            "unexpected EOF reading PLP total length".into(),
        ));
    }

    let total_len = buf.get_u64_le();
    if total_len == 0xFFFFFFFFFFFFFFFF {
        return Ok(SqlValue::Null);
    }

    // Read all chunks and concatenate the data
    let mut all_data = Vec::new();
    loop {
        if buf.remaining() < 4 {
            return Err(Error::Protocol(
                "unexpected EOF reading PLP chunk length".into(),
            ));
        }
        let chunk_len = buf.get_u32_le() as usize;
        if chunk_len == 0 {
            break; // End of PLP data
        }
        if buf.remaining() < chunk_len {
            return Err(Error::Protocol(
                "unexpected EOF reading PLP chunk data".into(),
            ));
        }
        all_data.extend_from_slice(&buf[..chunk_len]);
        buf.advance(chunk_len);
    }

    // Convert UTF-16LE to String
    let utf16: Vec<u16> = all_data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let s = String::from_utf16(&utf16)
        .map_err(|_| Error::Protocol("invalid UTF-16 in PLP nvarchar".into()))?;
    Ok(SqlValue::String(s))
}

/// Decode VARCHAR bytes to a String using collation-aware encoding.
///
/// When the `encoding` feature is enabled and a collation is provided,
/// this decodes the bytes using the appropriate character encoding based
/// on the collation's LCID. Otherwise falls back to UTF-8 lossy conversion.
#[allow(unused_variables)]
fn decode_varchar_string(data: &[u8], collation: Option<&Collation>) -> String {
    // Try collation-aware decoding first (handles GBK, Shift-JIS, etc.)
    #[cfg(feature = "encoding")]
    if let Some(coll) = collation {
        if let Some(encoding) = coll.encoding() {
            let (decoded, _, had_errors) = encoding.decode(data);
            if !had_errors {
                return decoded.into_owned();
            }
        }
    }

    // Fallback: lossy UTF-8 conversion
    String::from_utf8_lossy(data).into_owned()
}

/// Parse PLP-encoded VARCHAR(MAX) data.
fn parse_plp_varchar(buf: &mut &[u8], collation: Option<&Collation>) -> Result<SqlValue> {
    if buf.remaining() < 8 {
        return Err(Error::Protocol(
            "unexpected EOF reading PLP total length".into(),
        ));
    }

    let total_len = buf.get_u64_le();
    if total_len == 0xFFFFFFFFFFFFFFFF {
        return Ok(SqlValue::Null);
    }

    // Read all chunks and concatenate the data
    let mut all_data = Vec::new();
    loop {
        if buf.remaining() < 4 {
            return Err(Error::Protocol(
                "unexpected EOF reading PLP chunk length".into(),
            ));
        }
        let chunk_len = buf.get_u32_le() as usize;
        if chunk_len == 0 {
            break; // End of PLP data
        }
        if buf.remaining() < chunk_len {
            return Err(Error::Protocol(
                "unexpected EOF reading PLP chunk data".into(),
            ));
        }
        all_data.extend_from_slice(&buf[..chunk_len]);
        buf.advance(chunk_len);
    }

    // Decode using collation-aware encoding
    let s = decode_varchar_string(&all_data, collation);
    Ok(SqlValue::String(s))
}

/// Parse PLP-encoded VARBINARY(MAX) data.
pub(crate) fn parse_plp_varbinary(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 8 {
        return Err(Error::Protocol(
            "unexpected EOF reading PLP total length".into(),
        ));
    }

    let total_len = buf.get_u64_le();
    if total_len == 0xFFFFFFFFFFFFFFFF {
        return Ok(SqlValue::Null);
    }

    // Read all chunks and concatenate the data
    let mut all_data = Vec::new();
    loop {
        if buf.remaining() < 4 {
            return Err(Error::Protocol(
                "unexpected EOF reading PLP chunk length".into(),
            ));
        }
        let chunk_len = buf.get_u32_le() as usize;
        if chunk_len == 0 {
            break; // End of PLP data
        }
        if buf.remaining() < chunk_len {
            return Err(Error::Protocol(
                "unexpected EOF reading PLP chunk data".into(),
            ));
        }
        all_data.extend_from_slice(&buf[..chunk_len]);
        buf.advance(chunk_len);
    }

    Ok(SqlValue::Binary(bytes::Bytes::from(all_data)))
}

/// Parse SQL_VARIANT data which contains embedded type information.
///
/// SQL_VARIANT format:
/// - 4 bytes: total length (0 = NULL)
/// - 1 byte: base type ID
/// - 1 byte: property byte count
/// - N bytes: type-specific properties
/// - Remaining bytes: actual data
fn parse_sql_variant(buf: &mut &[u8]) -> Result<SqlValue> {
    // Read 4-byte length
    if buf.remaining() < 4 {
        return Err(Error::Protocol(
            "unexpected EOF reading SQL_VARIANT length".into(),
        ));
    }
    let total_len = buf.get_u32_le() as usize;

    if total_len == 0 {
        return Ok(SqlValue::Null);
    }

    if buf.remaining() < total_len {
        return Err(Error::Protocol(
            "unexpected EOF reading SQL_VARIANT data".into(),
        ));
    }

    // Read type info
    if total_len < 2 {
        return Err(Error::Protocol(
            "SQL_VARIANT too short for type info".into(),
        ));
    }

    let base_type = buf.get_u8();
    let prop_count = buf.get_u8() as usize;

    if buf.remaining() < prop_count {
        return Err(Error::Protocol(
            "unexpected EOF reading SQL_VARIANT properties".into(),
        ));
    }

    // Data length is total_len - 2 (type, prop_count) - prop_count
    let data_len = total_len.saturating_sub(2).saturating_sub(prop_count);

    // Parse based on base type
    // See MS-TDS SQL_VARIANT specification for type mappings
    match base_type {
        // Integer types (no properties)
        0x30 => {
            // TINYINT
            buf.advance(prop_count);
            if data_len < 1 {
                return Ok(SqlValue::Null);
            }
            let v = buf.get_u8();
            Ok(SqlValue::TinyInt(v))
        }
        0x32 => {
            // BIT
            buf.advance(prop_count);
            if data_len < 1 {
                return Ok(SqlValue::Null);
            }
            let v = buf.get_u8();
            Ok(SqlValue::Bool(v != 0))
        }
        0x34 => {
            // SMALLINT
            buf.advance(prop_count);
            if data_len < 2 {
                return Ok(SqlValue::Null);
            }
            let v = buf.get_i16_le();
            Ok(SqlValue::SmallInt(v))
        }
        0x38 => {
            // INT
            buf.advance(prop_count);
            if data_len < 4 {
                return Ok(SqlValue::Null);
            }
            let v = buf.get_i32_le();
            Ok(SqlValue::Int(v))
        }
        0x7F => {
            // BIGINT
            buf.advance(prop_count);
            if data_len < 8 {
                return Ok(SqlValue::Null);
            }
            let v = buf.get_i64_le();
            Ok(SqlValue::BigInt(v))
        }
        0x6D => {
            // FLOATN - 1 prop byte (length)
            let float_len = if prop_count >= 1 { buf.get_u8() } else { 8 };
            buf.advance(prop_count.saturating_sub(1));

            if float_len == 4 && data_len >= 4 {
                let v = buf.get_f32_le();
                Ok(SqlValue::Float(v))
            } else if data_len >= 8 {
                let v = buf.get_f64_le();
                Ok(SqlValue::Double(v))
            } else {
                Ok(SqlValue::Null)
            }
        }
        0x6E => {
            // MONEYN - 1 prop byte (length)
            let money_len = if prop_count >= 1 { buf.get_u8() } else { 8 };
            buf.advance(prop_count.saturating_sub(1));

            if money_len == 0 || data_len == 0 {
                Ok(SqlValue::Null)
            } else if (money_len == 4 && data_len >= 4) || (money_len == 8 && data_len >= 8) {
                parse_money_value(buf, money_len as usize)
            } else {
                buf.advance(data_len);
                Ok(SqlValue::Null)
            }
        }
        0x6F => {
            // DATETIMEN - 1 prop byte (length)
            #[cfg(feature = "chrono")]
            let dt_len = if prop_count >= 1 { buf.get_u8() } else { 8 };
            #[cfg(not(feature = "chrono"))]
            if prop_count >= 1 {
                buf.get_u8();
            }
            buf.advance(prop_count.saturating_sub(1));

            #[cfg(feature = "chrono")]
            {
                use chrono::NaiveDate;
                if dt_len == 4 && data_len >= 4 {
                    // SMALLDATETIME
                    let days = buf.get_u16_le() as i64;
                    let mins = buf.get_u16_le() as u32;
                    let base = NaiveDate::from_ymd_opt(1900, 1, 1)
                        .expect("epoch 1900-01-01 is valid")
                        .and_hms_opt(0, 0, 0)
                        .expect("midnight is valid");
                    let dt = base
                        + chrono::Duration::days(days)
                        + chrono::Duration::minutes(mins as i64);
                    Ok(SqlValue::DateTime(dt))
                } else if data_len >= 8 {
                    // DATETIME
                    let days = buf.get_i32_le() as i64;
                    let ticks = buf.get_u32_le() as i64;
                    let base = NaiveDate::from_ymd_opt(1900, 1, 1)
                        .expect("epoch 1900-01-01 is valid")
                        .and_hms_opt(0, 0, 0)
                        .expect("midnight is valid");
                    let millis = (ticks * 10) / 3;
                    let dt = base
                        + chrono::Duration::days(days)
                        + chrono::Duration::milliseconds(millis);
                    Ok(SqlValue::DateTime(dt))
                } else {
                    Ok(SqlValue::Null)
                }
            }
            #[cfg(not(feature = "chrono"))]
            {
                buf.advance(data_len);
                Ok(SqlValue::Null)
            }
        }
        0x6A | 0x6C => {
            // DECIMALN/NUMERICN - 2 prop bytes (precision, scale)
            let _precision = if prop_count >= 1 { buf.get_u8() } else { 18 };
            let scale = if prop_count >= 2 { buf.get_u8() } else { 0 };
            buf.advance(prop_count.saturating_sub(2));

            if data_len < 1 {
                return Ok(SqlValue::Null);
            }

            let sign = buf.get_u8();
            let mantissa_len = data_len - 1;

            if mantissa_len > 16 {
                // Too large, skip and return null
                buf.advance(mantissa_len);
                return Ok(SqlValue::Null);
            }

            let mut mantissa_bytes = [0u8; 16];
            for i in 0..mantissa_len.min(16) {
                mantissa_bytes[i] = buf.get_u8();
            }
            let mantissa = u128::from_le_bytes(mantissa_bytes);

            #[cfg(feature = "decimal")]
            {
                use rust_decimal::Decimal;
                if scale > 28 {
                    // Fall back to f64
                    let divisor = 10f64.powi(scale as i32);
                    let value = (mantissa as f64) / divisor;
                    let value = if sign == 0 { -value } else { value };
                    Ok(SqlValue::Double(value))
                } else {
                    let mut decimal = Decimal::from_i128_with_scale(mantissa as i128, scale as u32);
                    if sign == 0 {
                        decimal.set_sign_negative(true);
                    }
                    Ok(SqlValue::Decimal(decimal))
                }
            }
            #[cfg(not(feature = "decimal"))]
            {
                let divisor = 10f64.powi(scale as i32);
                let value = (mantissa as f64) / divisor;
                let value = if sign == 0 { -value } else { value };
                Ok(SqlValue::Double(value))
            }
        }
        0x24 => {
            // UNIQUEIDENTIFIER (no properties)
            buf.advance(prop_count);
            if data_len < 16 {
                return Ok(SqlValue::Null);
            }
            let mut guid_bytes = [0u8; 16];
            for byte in &mut guid_bytes {
                *byte = buf.get_u8();
            }
            Ok(SqlValue::Binary(bytes::Bytes::copy_from_slice(&guid_bytes)))
        }
        0x28 => {
            // DATE (no properties)
            buf.advance(prop_count);
            #[cfg(feature = "chrono")]
            {
                if data_len < 3 {
                    return Ok(SqlValue::Null);
                }
                let mut date_bytes = [0u8; 4];
                date_bytes[0] = buf.get_u8();
                date_bytes[1] = buf.get_u8();
                date_bytes[2] = buf.get_u8();
                let days = u32::from_le_bytes(date_bytes);
                let base =
                    chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
                let date = base + chrono::Duration::days(days as i64);
                Ok(SqlValue::Date(date))
            }
            #[cfg(not(feature = "chrono"))]
            {
                buf.advance(data_len);
                Ok(SqlValue::Null)
            }
        }
        0xA7 | 0x2F | 0x27 => {
            // BigVarChar/BigChar/VarChar/Char - 7 prop bytes (collation 5 + maxlen 2)
            // Parse collation from property bytes (5 bytes: 4 LCID + 1 sort_id)
            let collation = if prop_count >= 5 && buf.remaining() >= 5 {
                let lcid = buf.get_u32_le();
                let sort_id = buf.get_u8();
                buf.advance(prop_count.saturating_sub(5)); // Skip remaining props (max_length)
                Some(Collation { lcid, sort_id })
            } else {
                buf.advance(prop_count);
                None
            };
            if data_len == 0 {
                return Ok(SqlValue::String(String::new()));
            }
            let data = &buf[..data_len];
            // Use collation-aware decoding for non-ASCII text
            let s = decode_varchar_string(data, collation.as_ref());
            buf.advance(data_len);
            Ok(SqlValue::String(s))
        }
        0xE7 | 0xEF => {
            // NVarChar/NChar - 7 prop bytes (collation 5 + maxlen 2)
            buf.advance(prop_count);
            if data_len == 0 {
                return Ok(SqlValue::String(String::new()));
            }
            // UTF-16LE encoded
            let utf16: Vec<u16> = buf[..data_len]
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            buf.advance(data_len);
            let s = String::from_utf16(&utf16)
                .map_err(|_| Error::Protocol("invalid UTF-16 in SQL_VARIANT nvarchar".into()))?;
            Ok(SqlValue::String(s))
        }
        0xA5 | 0x2D | 0x25 => {
            // BigVarBinary/BigBinary/Binary/VarBinary - 2 prop bytes (maxlen)
            buf.advance(prop_count);
            let data = bytes::Bytes::copy_from_slice(&buf[..data_len]);
            buf.advance(data_len);
            Ok(SqlValue::Binary(data))
        }
        _ => {
            // Unknown type - return as binary
            buf.advance(prop_count);
            let data = bytes::Bytes::copy_from_slice(&buf[..data_len]);
            buf.advance(data_len);
            Ok(SqlValue::Binary(data))
        }
    }
}

/// Calculate number of bytes needed for TIME based on scale.
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
        .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).expect("midnight is valid"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use tds_protocol::token::TypeInfo;

    // ========================================================================
    // PLP (Partially Length-Prefixed) Parsing Tests
    // ========================================================================
    //
    // These tests verify that MAX type (NVARCHAR(MAX), VARCHAR(MAX), VARBINARY(MAX))
    // data is correctly parsed from the PLP wire format.

    /// Helper to create PLP data with a single chunk.
    fn make_plp_data(total_len: u64, chunks: &[&[u8]]) -> Vec<u8> {
        let mut data = Vec::new();
        // 8-byte total length
        data.extend_from_slice(&total_len.to_le_bytes());
        // Chunks
        for chunk in chunks {
            let len = chunk.len() as u32;
            data.extend_from_slice(&len.to_le_bytes());
            data.extend_from_slice(chunk);
        }
        // Terminating zero-length chunk
        data.extend_from_slice(&0u32.to_le_bytes());
        data
    }

    #[test]
    fn test_parse_plp_nvarchar_simple() {
        // "Hello" in UTF-16LE: H=0x0048, e=0x0065, l=0x006C, l=0x006C, o=0x006F
        let utf16_data = [0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00];
        let plp = make_plp_data(10, &[&utf16_data]);
        let mut buf: &[u8] = &plp;

        let result = parse_plp_nvarchar(&mut buf).unwrap();
        match result {
            SqlValue::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("expected String, got {result:?}"),
        }
    }

    #[test]
    fn test_parse_plp_nvarchar_null() {
        // NULL is indicated by total_len = 0xFFFFFFFFFFFFFFFF
        let plp = 0xFFFFFFFFFFFFFFFFu64.to_le_bytes();
        let mut buf: &[u8] = &plp;

        let result = parse_plp_nvarchar(&mut buf).unwrap();
        assert!(matches!(result, SqlValue::Null));
    }

    #[test]
    fn test_parse_plp_nvarchar_empty() {
        // Empty string: total_len=0, single zero-length chunk
        let plp = make_plp_data(0, &[]);
        let mut buf: &[u8] = &plp;

        let result = parse_plp_nvarchar(&mut buf).unwrap();
        match result {
            SqlValue::String(s) => assert_eq!(s, ""),
            _ => panic!("expected empty String"),
        }
    }

    #[test]
    fn test_parse_plp_nvarchar_multi_chunk() {
        // "Hello" split across two chunks: "Hel" + "lo"
        let chunk1 = [0x48, 0x00, 0x65, 0x00, 0x6C, 0x00]; // "Hel"
        let chunk2 = [0x6C, 0x00, 0x6F, 0x00]; // "lo"
        let plp = make_plp_data(10, &[&chunk1, &chunk2]);
        let mut buf: &[u8] = &plp;

        let result = parse_plp_nvarchar(&mut buf).unwrap();
        match result {
            SqlValue::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn test_parse_plp_varchar_simple() {
        let data = b"Hello World";
        let plp = make_plp_data(11, &[data]);
        let mut buf: &[u8] = &plp;

        let result = parse_plp_varchar(&mut buf, None).unwrap();
        match result {
            SqlValue::String(s) => assert_eq!(s, "Hello World"),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn test_parse_plp_varchar_null() {
        let plp = 0xFFFFFFFFFFFFFFFFu64.to_le_bytes();
        let mut buf: &[u8] = &plp;

        let result = parse_plp_varchar(&mut buf, None).unwrap();
        assert!(matches!(result, SqlValue::Null));
    }

    #[test]
    fn test_parse_plp_varbinary_simple() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        let plp = make_plp_data(5, &[&data]);
        let mut buf: &[u8] = &plp;

        let result = parse_plp_varbinary(&mut buf).unwrap();
        match result {
            SqlValue::Binary(b) => assert_eq!(&b[..], &[0x01, 0x02, 0x03, 0x04, 0x05]),
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn test_parse_plp_varbinary_null() {
        let plp = 0xFFFFFFFFFFFFFFFFu64.to_le_bytes();
        let mut buf: &[u8] = &plp;

        let result = parse_plp_varbinary(&mut buf).unwrap();
        assert!(matches!(result, SqlValue::Null));
    }

    #[test]
    fn test_parse_plp_varbinary_large() {
        // Test with larger data split across multiple chunks
        let chunk1: Vec<u8> = (0..100u8).collect();
        let chunk2: Vec<u8> = (100..200u8).collect();
        let chunk3: Vec<u8> = (200..255u8).collect();
        let total_len = chunk1.len() + chunk2.len() + chunk3.len();
        let plp = make_plp_data(total_len as u64, &[&chunk1, &chunk2, &chunk3]);
        let mut buf: &[u8] = &plp;

        let result = parse_plp_varbinary(&mut buf).unwrap();
        match result {
            SqlValue::Binary(b) => {
                assert_eq!(b.len(), 255);
                // Verify data integrity
                for (i, &byte) in b.iter().enumerate() {
                    assert_eq!(byte, i as u8);
                }
            }
            _ => panic!("expected Binary"),
        }
    }

    // ========================================================================
    // Multi-Column Row Parsing Tests
    // ========================================================================
    //
    // These tests verify that parsing multiple columns in a row works correctly,
    // especially for scenarios where string columns are followed by integer columns.

    /// Build raw row data for a non-MAX NVarChar followed by an IntN.
    /// This mimics the scenario: SELECT @name AS greeting, @value AS number
    fn make_nvarchar_int_row(nvarchar_value: &str, int_value: i32) -> Vec<u8> {
        let mut data = Vec::new();

        // Column 0: NVarChar (non-MAX) - 2-byte length prefix (in bytes)
        let utf16: Vec<u16> = nvarchar_value.encode_utf16().collect();
        let byte_len = (utf16.len() * 2) as u16;
        data.extend_from_slice(&byte_len.to_le_bytes());
        for code_unit in utf16 {
            data.extend_from_slice(&code_unit.to_le_bytes());
        }

        // Column 1: IntN - 1-byte length prefix
        data.push(4); // 4 bytes for INT
        data.extend_from_slice(&int_value.to_le_bytes());

        data
    }

    #[test]
    fn test_parse_row_nvarchar_then_int() {
        // Build raw row data for: "World", 42
        let raw_data = make_nvarchar_int_row("World", 42);

        // Create column metadata
        let col0 = ColumnData {
            name: "greeting".to_string(),
            type_id: TypeId::NVarChar,
            col_type: 0xE7,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length: Some(10), // 5 chars * 2 bytes = 10
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        };

        let col1 = ColumnData {
            name: "number".to_string(),
            type_id: TypeId::IntN,
            col_type: 0x26,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length: Some(4),
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        };

        let mut buf: &[u8] = &raw_data;

        // Parse column 0 (NVarChar)
        let value0 = parse_column_value(&mut buf, &col0).unwrap();
        match value0 {
            SqlValue::String(s) => assert_eq!(s, "World"),
            _ => panic!("expected String, got {value0:?}"),
        }

        // Parse column 1 (IntN)
        let value1 = parse_column_value(&mut buf, &col1).unwrap();
        match value1 {
            SqlValue::Int(i) => assert_eq!(i, 42),
            _ => panic!("expected Int, got {value1:?}"),
        }

        // Buffer should be fully consumed
        assert_eq!(buf.len(), 0, "buffer should be fully consumed");
    }

    #[test]
    fn test_parse_row_multiple_types() {
        // Build raw data for: NULL (NVarChar), 123 (IntN), "Test" (NVarChar), NULL (IntN)
        let mut data = Vec::new();

        // Column 0: NVarChar NULL (0xFFFF)
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());

        // Column 1: IntN with value 123
        data.push(4); // 4 bytes
        data.extend_from_slice(&123i32.to_le_bytes());

        // Column 2: NVarChar "Test"
        let utf16: Vec<u16> = "Test".encode_utf16().collect();
        data.extend_from_slice(&((utf16.len() * 2) as u16).to_le_bytes());
        for code_unit in utf16 {
            data.extend_from_slice(&code_unit.to_le_bytes());
        }

        // Column 3: IntN NULL (0 length)
        data.push(0);

        // Metadata for 4 columns
        let col0 = ColumnData {
            name: "col0".to_string(),
            type_id: TypeId::NVarChar,
            col_type: 0xE7,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length: Some(100),
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        };
        let col1 = ColumnData {
            name: "col1".to_string(),
            type_id: TypeId::IntN,
            col_type: 0x26,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length: Some(4),
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        };
        let col2 = col0.clone();
        let col3 = col1.clone();

        let mut buf: &[u8] = &data;

        // Parse all 4 columns
        let v0 = parse_column_value(&mut buf, &col0).unwrap();
        assert!(matches!(v0, SqlValue::Null), "col0 should be Null");

        let v1 = parse_column_value(&mut buf, &col1).unwrap();
        assert!(matches!(v1, SqlValue::Int(123)), "col1 should be 123");

        let v2 = parse_column_value(&mut buf, &col2).unwrap();
        match v2 {
            SqlValue::String(s) => assert_eq!(s, "Test"),
            _ => panic!("col2 should be 'Test'"),
        }

        let v3 = parse_column_value(&mut buf, &col3).unwrap();
        assert!(matches!(v3, SqlValue::Null), "col3 should be Null");

        // Buffer should be fully consumed
        assert_eq!(buf.len(), 0, "buffer should be fully consumed");
    }

    #[test]
    fn test_parse_row_with_unicode() {
        // Test with Unicode characters that need proper UTF-16 encoding
        let test_str = "Héllo Wörld 日本語";
        let mut data = Vec::new();

        // NVarChar with Unicode
        let utf16: Vec<u16> = test_str.encode_utf16().collect();
        data.extend_from_slice(&((utf16.len() * 2) as u16).to_le_bytes());
        for code_unit in utf16 {
            data.extend_from_slice(&code_unit.to_le_bytes());
        }

        // IntN value
        data.push(8); // BIGINT
        data.extend_from_slice(&9999999999i64.to_le_bytes());

        let col0 = ColumnData {
            name: "text".to_string(),
            type_id: TypeId::NVarChar,
            col_type: 0xE7,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length: Some(100),
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        };
        let col1 = ColumnData {
            name: "num".to_string(),
            type_id: TypeId::IntN,
            col_type: 0x26,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length: Some(8),
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        };

        let mut buf: &[u8] = &data;

        let v0 = parse_column_value(&mut buf, &col0).unwrap();
        match v0 {
            SqlValue::String(s) => assert_eq!(s, test_str),
            _ => panic!("expected String"),
        }

        let v1 = parse_column_value(&mut buf, &col1).unwrap();
        match v1 {
            SqlValue::BigInt(i) => assert_eq!(i, 9999999999),
            _ => panic!("expected BigInt"),
        }

        assert_eq!(buf.len(), 0, "buffer should be fully consumed");
    }
}
