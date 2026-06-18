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

// Allow unwrap/expect ONLY for chrono construction from compile-time
// constants (epochs, midnight, UTC offset 0). Values that arrive from the
// wire must use checked construction and surface protocol errors — a
// malicious or buggy server must never be able to panic the client (see
// `smalldatetime_from_wire` / `datetime_from_wire`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::needless_range_loop)]

use std::sync::Arc;

use bytes::Buf;
use mssql_types::SqlValue;
// Scale primitives shared with the secondary decode stack (`mssql_types::decode`)
// so the scale→width mapping and the 100ns-interval conversion cannot drift
// between the two stacks (see #204). `time_bytes_for_scale` is pure scale math,
// used for frame-length validation even without `chrono`.
#[cfg(feature = "chrono")]
use mssql_types::__private::intervals_to_time;
use mssql_types::__private::time_bytes_for_scale;
use tds_protocol::token::{ColMetaData, Collation, ColumnData, NbcRow, RawRow};
use tds_protocol::types::TypeId;

use crate::error::{Error, Result};

/// Build a `NaiveDateTime` from SMALLDATETIME wire values (days since
/// 1900-01-01, minutes since midnight). Both come from the wire: out-of-range
/// values are protocol errors, never panics.
#[cfg(feature = "chrono")]
fn smalldatetime_from_wire(days: i64, minutes: u32) -> Result<chrono::NaiveDateTime> {
    let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("epoch 1900-01-01 is valid");
    let date = base
        .checked_add_signed(chrono::Duration::days(days))
        .ok_or_else(|| Error::Protocol(format!("SMALLDATETIME days out of range: {days}")))?;
    let secs = u64::from(minutes) * 60;
    let time = u32::try_from(secs)
        .ok()
        .and_then(|s| chrono::NaiveTime::from_num_seconds_from_midnight_opt(s, 0))
        .ok_or_else(|| Error::Protocol(format!("SMALLDATETIME minutes out of range: {minutes}")))?;
    Ok(date.and_time(time))
}

/// Build a `NaiveDateTime` from DATETIME wire values (days since 1900-01-01,
/// 1/300ths of a second since midnight). Both come from the wire:
/// out-of-range values are protocol errors, never panics.
#[cfg(feature = "chrono")]
fn datetime_from_wire(days: i64, time_300ths: u64) -> Result<chrono::NaiveDateTime> {
    let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("epoch 1900-01-01 is valid");
    let date = base
        .checked_add_signed(chrono::Duration::days(days))
        .ok_or_else(|| Error::Protocol(format!("DATETIME days out of range: {days}")))?;
    let total_ms = (time_300ths * 1000) / 300;
    let nanos = ((total_ms % 1000) * 1_000_000) as u32;
    let time = u32::try_from(total_ms / 1000)
        .ok()
        .and_then(|secs| chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos))
        .ok_or_else(|| {
            Error::Protocol(format!(
                "DATETIME time component out of range: {time_300ths}"
            ))
        })?;
    Ok(date.and_time(time))
}

/// Convert a RawRow to a client Row.
///
/// This parses the raw bytes back into SqlValue types based on column metadata.
pub(crate) fn convert_raw_row(
    raw: &RawRow,
    meta: &ColMetaData,
    row_meta: &Arc<crate::row::ColMetaData>,
) -> Result<crate::row::Row> {
    let mut values = Vec::with_capacity(meta.columns.len());
    let mut buf = raw.data.as_ref();

    for col in &meta.columns {
        let value = parse_column_value(&mut buf, col)?;
        values.push(value);
    }

    Ok(crate::row::Row::from_values_shared(
        Arc::clone(row_meta),
        values,
    ))
}

/// Convert an NbcRow to a client Row.
///
/// NbcRow has a null bitmap followed by only non-null values.
pub(crate) fn convert_nbc_row(
    nbc: &NbcRow,
    meta: &ColMetaData,
    row_meta: &Arc<crate::row::ColMetaData>,
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

    Ok(crate::row::Row::from_values_shared(
        Arc::clone(row_meta),
        values,
    ))
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
    row_meta: &Arc<crate::row::ColMetaData>,
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

    Ok(crate::row::Row::from_values_shared(
        Arc::clone(row_meta),
        values,
    ))
}

/// Convert an NbcRow to a client Row with Always Encrypted decryption.
///
/// Same as `convert_raw_row_decrypted` but handles the null bitmap.
#[cfg(feature = "always-encrypted")]
pub(crate) fn convert_nbc_row_decrypted(
    nbc: &NbcRow,
    meta: &ColMetaData,
    row_meta: &Arc<crate::row::ColMetaData>,
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

    Ok(crate::row::Row::from_values_shared(
        Arc::clone(row_meta),
        values,
    ))
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

    // The decrypted plaintext is Always Encrypted's *normalized* form (see
    // `encryption::normalize_for_encryption`), which is not the TDS wire form,
    // so it is denormalized directly rather than run through the wire parser.
    denormalize_decrypted(plaintext, base_col)
}

/// Denormalize Always Encrypted plaintext (the inverse of
/// `encryption::normalize_for_encryption`) into a [`SqlValue`].
///
/// Only the base types the encryption path supports are handled; anything else
/// is rejected rather than silently misparsed.
#[cfg(feature = "always-encrypted")]
fn denormalize_decrypted(plaintext: Vec<u8>, base_col: &ColumnData) -> Result<SqlValue> {
    match base_col.type_id {
        // Integer family and bit normalize to 8-byte little-endian (every width,
        // tinyint/smallint included). The base type's width picks the variant.
        TypeId::Bit
        | TypeId::BitN
        | TypeId::Int1
        | TypeId::Int2
        | TypeId::Int4
        | TypeId::Int8
        | TypeId::IntN => {
            let v = i64::from_le_bytes(decrypted_array::<8>(&plaintext, "integer")?);
            Ok(match base_col.type_id {
                TypeId::Bit | TypeId::BitN => SqlValue::Bool(v != 0),
                TypeId::Int1 => SqlValue::TinyInt(v as u8),
                TypeId::Int2 => SqlValue::SmallInt(v as i16),
                TypeId::Int8 => SqlValue::BigInt(v),
                // IntN carries the width in max_length; default to INT.
                TypeId::IntN => match base_col.type_info.max_length {
                    Some(1) => SqlValue::TinyInt(v as u8),
                    Some(2) => SqlValue::SmallInt(v as i16),
                    Some(8) => SqlValue::BigInt(v),
                    _ => SqlValue::Int(v as i32),
                },
                _ => SqlValue::Int(v as i32),
            })
        }
        // REAL/FLOAT normalize to their IEEE-754 bits, little-endian.
        TypeId::Float4 => Ok(SqlValue::Float(f32::from_le_bytes(decrypted_array::<4>(
            &plaintext, "REAL",
        )?))),
        TypeId::Float8 => Ok(SqlValue::Double(f64::from_le_bytes(decrypted_array::<8>(
            &plaintext, "FLOAT",
        )?))),
        TypeId::FloatN => match base_col.type_info.max_length {
            Some(4) => Ok(SqlValue::Float(f32::from_le_bytes(decrypted_array::<4>(
                &plaintext, "REAL",
            )?))),
            _ => Ok(SqlValue::Double(f64::from_le_bytes(decrypted_array::<8>(
                &plaintext, "FLOAT",
            )?))),
        },
        // NVARCHAR/NCHAR normalize to UTF-16LE code units, no length prefix.
        TypeId::NVarChar | TypeId::NChar => {
            if plaintext.len() % 2 != 0 {
                return Err(Error::Encryption(format!(
                    "decrypted NVARCHAR has an odd byte length ({})",
                    plaintext.len()
                )));
            }
            let units: Vec<u16> = plaintext
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let s = String::from_utf16(&units).map_err(|_| {
                Error::Encryption("decrypted NVARCHAR is not valid UTF-16".to_string())
            })?;
            // Fixed-width NCHAR is space-padded to its declared length (the AE
            // normalized form is unpadded); match SQL Server / .NET on read.
            let s = if base_col.type_id == TypeId::NChar {
                pad_fixed_char(s, base_col.type_info.max_length.unwrap_or(0) as usize / 2)
            } else {
                s
            };
            Ok(SqlValue::String(s))
        }
        // CHAR/VARCHAR normalize to the column code-page (Windows-1252) bytes.
        TypeId::BigChar | TypeId::Char | TypeId::BigVarChar | TypeId::VarChar => {
            let (s, _, had_errors) = encoding_rs::WINDOWS_1252.decode(&plaintext);
            if had_errors {
                return Err(Error::Encryption(
                    "decrypted CHAR is not valid Windows-1252".to_string(),
                ));
            }
            let s = s.into_owned();
            // Fixed-width CHAR is space-padded to its declared length.
            let s = if matches!(base_col.type_id, TypeId::Char | TypeId::BigChar) {
                pad_fixed_char(s, base_col.type_info.max_length.unwrap_or(0) as usize)
            } else {
                s
            };
            Ok(SqlValue::String(s))
        }
        // VARBINARY/BINARY normalize to the raw bytes.
        TypeId::BigVarBinary | TypeId::BigBinary | TypeId::VarBinary | TypeId::Binary => {
            Ok(SqlValue::Binary(bytes::Bytes::from(plaintext)))
        }
        // UNIQUEIDENTIFIER: 16 bytes in SQL Server's mixed-endian order; swap
        // the first three groups back to the RFC layout.
        #[cfg(feature = "uuid")]
        TypeId::Guid => {
            let b = decrypted_array::<16>(&plaintext, "uniqueidentifier")?;
            Ok(SqlValue::Uuid(uuid::Uuid::from_bytes([
                b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12],
                b[13], b[14], b[15],
            ])))
        }
        // DATE: 3-byte little-endian days since 0001-01-01 (CE day 1).
        #[cfg(feature = "chrono")]
        TypeId::Date => {
            let b = decrypted_array::<3>(&plaintext, "date")?;
            let days = u32::from(b[0]) | (u32::from(b[1]) << 8) | (u32::from(b[2]) << 16);
            chrono::NaiveDate::from_num_days_from_ce_opt(days as i32 + 1)
                .map(SqlValue::Date)
                .ok_or_else(|| {
                    Error::Encryption(format!("decrypted DATE day count {days} is out of range"))
                })
        }
        // DECIMAL/NUMERIC: 1 sign byte + 16-byte little-endian magnitude; the
        // base type's scale rescales the unscaled integer.
        #[cfg(feature = "decimal")]
        TypeId::Decimal | TypeId::Numeric | TypeId::DecimalN | TypeId::NumericN => {
            let b = decrypted_array::<17>(&plaintext, "decimal")?;
            let mut mag = [0u8; 16];
            mag.copy_from_slice(&b[1..17]);
            let magnitude = u128::from_le_bytes(mag) as i128;
            let signed = if b[0] == 0 { -magnitude } else { magnitude };
            let scale = u32::from(base_col.type_info.scale.unwrap_or(0));
            rust_decimal::Decimal::try_from_i128_with_scale(signed, scale)
                .map(SqlValue::Decimal)
                .map_err(|e| Error::Encryption(format!("decrypted DECIMAL out of range: {e}")))
        }
        // MONEY/SMALLMONEY both normalize to the 8-byte MONEY form.
        #[cfg(feature = "decimal")]
        TypeId::Money | TypeId::Money4 | TypeId::MoneyN => {
            if plaintext.len() != 8 {
                return Err(Error::Encryption(format!(
                    "decrypted MONEY has {} bytes, expected 8",
                    plaintext.len()
                )));
            }
            parse_money_value(&mut plaintext.as_slice(), 8)
        }
        // TIME: a fixed 5-byte scale-7 tick count → NaiveTime (the value was
        // already quantized to the column scale on the write side).
        #[cfg(feature = "chrono")]
        TypeId::Time => ae_time_from_bytes(&plaintext).map(SqlValue::Time),
        // DATETIME2: a fixed 5-byte time followed by a 3-byte date (8 bytes).
        #[cfg(feature = "chrono")]
        TypeId::DateTime2 => {
            if plaintext.len() != 8 {
                return Err(Error::Encryption(format!(
                    "decrypted DATETIME2 has {} bytes, expected 8",
                    plaintext.len()
                )));
            }
            let time = ae_time_from_bytes(&plaintext[..5])?;
            let date = ae_date_from_bytes(&plaintext[5..8])?;
            Ok(SqlValue::DateTime(date.and_time(time)))
        }
        // DATETIMEOFFSET: a fixed 5-byte UTC time + 3-byte UTC date + 2-byte
        // signed offset minutes (10 bytes).
        #[cfg(feature = "chrono")]
        TypeId::DateTimeOffset => {
            use chrono::TimeZone;
            if plaintext.len() != 10 {
                return Err(Error::Encryption(format!(
                    "decrypted DATETIMEOFFSET has {} bytes, expected 10",
                    plaintext.len()
                )));
            }
            let time = ae_time_from_bytes(&plaintext[..5])?;
            let date = ae_date_from_bytes(&plaintext[5..8])?;
            let offset_min = i16::from_le_bytes([plaintext[8], plaintext[9]]);
            let offset =
                chrono::FixedOffset::east_opt(i32::from(offset_min) * 60).ok_or_else(|| {
                    Error::Encryption(format!(
                        "decrypted DATETIMEOFFSET offset {offset_min} invalid"
                    ))
                })?;
            Ok(SqlValue::DateTimeOffset(
                offset.from_utc_datetime(&date.and_time(time)),
            ))
        }
        // Legacy DATETIME (8 bytes) / SMALLDATETIME (4 bytes); the AE normalized
        // form equals the wire form, so reuse the wire decoders.
        #[cfg(feature = "chrono")]
        TypeId::DateTime | TypeId::DateTime4 | TypeId::DateTimeN => match plaintext.len() {
            8 => {
                let b = decrypted_array::<8>(&plaintext, "datetime")?;
                let days = i64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                let ticks = u64::from(u32::from_le_bytes([b[4], b[5], b[6], b[7]]));
                datetime_from_wire(days, ticks).map(SqlValue::DateTime)
            }
            4 => {
                let b = decrypted_array::<4>(&plaintext, "smalldatetime")?;
                let days = i64::from(u16::from_le_bytes([b[0], b[1]]));
                let minutes = u32::from(u16::from_le_bytes([b[2], b[3]]));
                smalldatetime_from_wire(days, minutes).map(SqlValue::DateTime)
            }
            n => Err(Error::Encryption(format!(
                "decrypted DATETIME has {n} bytes, expected 4 or 8"
            ))),
        },
        other => Err(Error::Encryption(format!(
            "Always Encrypted read is not yet implemented for base type {other:?}"
        ))),
    }
}

/// Decode an AE-normalized time value back to a `NaiveTime`. Always Encrypted
/// stores it as a fixed 5-byte little-endian scale-7 (100ns) tick count,
/// regardless of the column scale (the value was quantized to the column scale
/// on the write side).
#[cfg(all(feature = "always-encrypted", feature = "chrono"))]
fn ae_time_from_bytes(b: &[u8]) -> Result<chrono::NaiveTime> {
    if b.len() != 5 {
        return Err(Error::Encryption(format!(
            "decrypted TIME has {} bytes, expected 5",
            b.len()
        )));
    }
    let mut buf = [0u8; 8];
    buf[..5].copy_from_slice(b);
    let ticks7 = u64::from_le_bytes(buf);
    let nanos = ticks7
        .checked_mul(100)
        .ok_or_else(|| Error::Encryption("decrypted TIME out of range".to_string()))?;
    let secs = u32::try_from(nanos / 1_000_000_000)
        .map_err(|_| Error::Encryption("decrypted TIME out of range".to_string()))?;
    let nsub = (nanos % 1_000_000_000) as u32;
    chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nsub)
        .ok_or_else(|| Error::Encryption(format!("decrypted TIME {secs}s out of range")))
}

/// Right-pad a decrypted fixed-width `char`/`nchar` value with spaces to its
/// declared character length (the AE normalized form is stored unpadded, but
/// fixed-width columns read back space-padded, matching SQL Server / .NET).
#[cfg(feature = "always-encrypted")]
fn pad_fixed_char(mut s: String, target_chars: usize) -> String {
    let cur = s.chars().count();
    if cur < target_chars {
        s.extend(std::iter::repeat_n(' ', target_chars - cur));
    }
    s
}

/// Decode a 3-byte little-endian days-since-0001 count back to a `NaiveDate`.
#[cfg(all(feature = "always-encrypted", feature = "chrono"))]
fn ae_date_from_bytes(b: &[u8]) -> Result<chrono::NaiveDate> {
    if b.len() != 3 {
        return Err(Error::Encryption(format!(
            "decrypted date has {} bytes, expected 3",
            b.len()
        )));
    }
    let days = u32::from(b[0]) | (u32::from(b[1]) << 8) | (u32::from(b[2]) << 16);
    chrono::NaiveDate::from_num_days_from_ce_opt(days as i32 + 1).ok_or_else(|| {
        Error::Encryption(format!("decrypted date day count {days} is out of range"))
    })
}

/// Convert decrypted plaintext into a fixed-size array, erroring on a length
/// mismatch rather than panicking.
#[cfg(feature = "always-encrypted")]
fn decrypted_array<const N: usize>(plaintext: &[u8], what: &str) -> Result<[u8; N]> {
    plaintext.try_into().map_err(|_| {
        Error::Encryption(format!(
            "decrypted {what} has {} bytes, expected {N}",
            plaintext.len()
        ))
    })
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
        // `cents` is an i64 (fixed 4- or 8-byte MONEY) at scale 4, always
        // inside rust_decimal's 96-bit range — but use the checked
        // constructor anyway so no panicking `from_i128_with_scale` call
        // remains in the decoder (invariant: grep returns nothing).
        match Decimal::try_from_i128_with_scale(cents as i128, 4) {
            Ok(decimal) => Ok(SqlValue::Decimal(decimal)),
            Err(_) => Ok(SqlValue::Double((cents as f64) / 10000.0)),
        }
    }

    #[cfg(not(feature = "decimal"))]
    {
        Ok(SqlValue::Double((cents as f64) / 10000.0))
    }
}

/// Parse a single column value from a buffer based on column metadata.
// `pub` for the `__fuzzing` re-export; the module itself is `pub(crate)`,
// so this stays crate-private unless the `fuzzing` feature is enabled.
pub fn parse_column_value(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
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
        TypeId::Money | TypeId::Money4 | TypeId::MoneyN => parse_money(buf, col.type_id)?,

        // Variable-length nullable types (IntN, FloatN, etc.)
        TypeId::IntN => parse_intn(buf)?,
        TypeId::FloatN => parse_floatn(buf)?,
        TypeId::BitN => parse_bitn(buf)?,

        TypeId::Decimal | TypeId::Numeric | TypeId::DecimalN | TypeId::NumericN => {
            parse_decimal(buf, col)?
        }

        // DATETIME/SMALLDATETIME nullable (1-byte length prefix)
        TypeId::DateTimeN => parse_datetimen(buf)?,
        // Fixed DATETIME (8 bytes)
        TypeId::DateTime => parse_legacy_datetime(buf)?,
        // Fixed SMALLDATETIME (4 bytes)
        TypeId::DateTime4 => parse_legacy_smalldatetime(buf)?,
        // DATE (3 bytes, nullable with 1-byte length prefix)
        TypeId::Date => parse_date(buf)?,
        // TIME (variable length with scale, 1-byte length prefix)
        TypeId::Time => parse_time(buf, col)?,
        // DATETIME2 (variable length: TIME bytes + 3 bytes date, 1-byte length prefix)
        TypeId::DateTime2 => parse_datetime2(buf, col)?,
        // DATETIMEOFFSET (variable length: TIME bytes + 3 bytes date + 2 bytes offset)
        TypeId::DateTimeOffset => parse_datetimeoffset(buf, col)?,

        // TEXT type - always uses PLP encoding (deprecated LOB type)
        TypeId::Text => parse_plp_varchar(buf, col.type_info.collation.as_ref())?,

        // Legacy byte-length string types (Char, VarChar) - 1-byte length prefix
        TypeId::Char | TypeId::VarChar => parse_legacy_varchar(buf, col)?,
        // Variable-length string types (BigVarChar, BigChar)
        TypeId::BigVarChar | TypeId::BigChar => parse_bigvarchar(buf, col)?,
        // NTEXT type - always uses PLP encoding (deprecated LOB type)
        TypeId::NText => parse_plp_nvarchar(buf)?,
        // Variable-length Unicode string types (NVarChar, NChar)
        TypeId::NVarChar | TypeId::NChar => parse_nvarchar(buf, col)?,

        // IMAGE type - always uses PLP encoding (deprecated LOB type)
        TypeId::Image => parse_plp_varbinary(buf)?,
        // Legacy byte-length binary types (Binary, VarBinary) - 1-byte length prefix
        TypeId::Binary | TypeId::VarBinary => parse_legacy_varbinary(buf)?,
        // Variable-length binary types (BigVarBinary, BigBinary)
        TypeId::BigVarBinary | TypeId::BigBinary => parse_bigvarbinary(buf, col)?,

        // XML type - always uses PLP encoding
        TypeId::Xml => parse_xml(buf)?,
        // GUID/UniqueIdentifier
        TypeId::Guid => parse_guid(buf)?,
        // SQL_VARIANT - contains embedded type info
        TypeId::Variant => parse_sql_variant(buf)?,
        // UDT (User-Defined Type) - uses PLP encoding, return as binary
        TypeId::Udt => parse_plp_varbinary(buf)?,

        // Default: treat as binary with 2-byte length prefix
        _ => parse_default_binary(buf, col)?,
    };

    Ok(value)
}

// =============================================================================
// Per-type value parsers (the arms of `parse_column_value`, extracted for
// readability and unit-testability — see #309). Each consumes exactly the bytes
// of one column value from `buf` and returns its `SqlValue`. Behavior is
// identical to the inlined arms.
// =============================================================================

/// MONEY / SMALLMONEY / MONEYN — fixed-point with 4 decimal places.
fn parse_money(buf: &mut &[u8], type_id: TypeId) -> Result<SqlValue> {
    let bytes = match type_id {
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
        // Caller dispatches only Money | Money4 | MoneyN to this helper.
        _ => unreachable!("parse_money is only called for Money|Money4|MoneyN"),
    };

    if buf.remaining() < bytes {
        return Err(Error::Protocol(format!(
            "unexpected EOF reading money data ({bytes} bytes)"
        )));
    }

    parse_money_value(buf, bytes)
}

/// INTN — nullable integer with a 1-byte length prefix (0/1/2/4/8).
fn parse_intn(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol("unexpected EOF reading IntN length".into()));
    }
    let len = buf.get_u8();
    if buf.remaining() < len as usize {
        return Err(Error::Protocol("unexpected EOF reading IntN data".into()));
    }
    Ok(match len {
        0 => SqlValue::Null,
        1 => SqlValue::TinyInt(buf.get_u8()),
        2 => SqlValue::SmallInt(buf.get_i16_le()),
        4 => SqlValue::Int(buf.get_i32_le()),
        8 => SqlValue::BigInt(buf.get_i64_le()),
        _ => {
            return Err(Error::Protocol(format!("invalid IntN length: {len}")));
        }
    })
}

/// FLOATN — nullable float with a 1-byte length prefix (0/4/8).
fn parse_floatn(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol(
            "unexpected EOF reading FloatN length".into(),
        ));
    }
    let len = buf.get_u8();
    if buf.remaining() < len as usize {
        return Err(Error::Protocol("unexpected EOF reading FloatN data".into()));
    }
    Ok(match len {
        0 => SqlValue::Null,
        4 => SqlValue::Float(buf.get_f32_le()),
        8 => SqlValue::Double(buf.get_f64_le()),
        _ => {
            return Err(Error::Protocol(format!("invalid FloatN length: {len}")));
        }
    })
}

/// BITN — nullable bit with a 1-byte length prefix (0/1).
fn parse_bitn(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol("unexpected EOF reading BitN length".into()));
    }
    let len = buf.get_u8();
    if buf.remaining() < len as usize {
        return Err(Error::Protocol("unexpected EOF reading BitN data".into()));
    }
    Ok(match len {
        0 => SqlValue::Null,
        1 => SqlValue::Bool(buf.get_u8() != 0),
        _ => {
            return Err(Error::Protocol(format!("invalid BitN length: {len}")));
        }
    })
}

/// DECIMAL / NUMERIC (and the N variants). The mantissa decode and the
/// 96-bit/scale-28 overflow policy are shared with the secondary decode stack
/// (`mssql_types::decode`) so the two cannot drift — that drift was issue #188.
/// Only `scale` is consulted by the shared decoder.
fn parse_decimal(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    let type_info = mssql_types::TypeInfo::decimal(
        col.type_info.precision.unwrap_or(18),
        col.type_info.scale.unwrap_or(0),
    );
    Ok(mssql_types::__private::decode_decimal(buf, &type_info)?)
}

/// DATETIMEN — nullable legacy datetime with a 1-byte length prefix (4 or 8).
fn parse_datetimen(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol(
            "unexpected EOF reading DateTimeN length".into(),
        ));
    }
    let len = buf.get_u8() as usize;
    Ok(if len == 0 {
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
                    SqlValue::DateTime(smalldatetime_from_wire(days, minutes)?)
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
                    SqlValue::DateTime(datetime_from_wire(days, time_300ths)?)
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
    })
}

/// DATETIME — fixed 8-byte legacy datetime.
fn parse_legacy_datetime(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 8 {
        return Err(Error::Protocol("unexpected EOF reading DATETIME".into()));
    }
    let days = buf.get_i32_le() as i64;
    let time_300ths = buf.get_u32_le() as u64;
    #[cfg(feature = "chrono")]
    {
        Ok(SqlValue::DateTime(datetime_from_wire(days, time_300ths)?))
    }
    #[cfg(not(feature = "chrono"))]
    {
        Ok(SqlValue::String(format!("DATETIME({days},{time_300ths})")))
    }
}

/// SMALLDATETIME — fixed 4-byte legacy datetime.
fn parse_legacy_smalldatetime(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 4 {
        return Err(Error::Protocol(
            "unexpected EOF reading SMALLDATETIME".into(),
        ));
    }
    let days = buf.get_u16_le() as i64;
    let minutes = buf.get_u16_le() as u32;
    #[cfg(feature = "chrono")]
    {
        Ok(SqlValue::DateTime(smalldatetime_from_wire(days, minutes)?))
    }
    #[cfg(not(feature = "chrono"))]
    {
        Ok(SqlValue::String(format!("SMALLDATETIME({days},{minutes})")))
    }
}

/// DATE — 3 bytes (days since 0001-01-01), nullable with a 1-byte length prefix.
fn parse_date(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol("unexpected EOF reading DATE length".into()));
    }
    let len = buf.get_u8() as usize;
    Ok(if len == 0 {
        SqlValue::Null
    } else if len != 3 {
        return Err(Error::Protocol(format!("invalid DATE length: {len}")));
    } else if buf.remaining() < 3 {
        return Err(Error::Protocol("unexpected EOF reading DATE".into()));
    } else {
        // 3 bytes little-endian days since 0001-01-01
        let days =
            buf.get_u8() as u32 | ((buf.get_u8() as u32) << 8) | ((buf.get_u8() as u32) << 16);
        #[cfg(feature = "chrono")]
        {
            let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
            let date = base
                .checked_add_signed(chrono::Duration::days(days as i64))
                .ok_or_else(|| Error::Protocol(format!("date field days out of range: {days}")))?;
            SqlValue::Date(date)
        }
        #[cfg(not(feature = "chrono"))]
        {
            SqlValue::String(format!("DATE({days})"))
        }
    })
}

/// TIME — variable length driven by the column's scale, 1-byte length prefix.
fn parse_time(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol("unexpected EOF reading TIME length".into()));
    }
    let len = buf.get_u8() as usize;
    Ok(if len == 0 {
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
            let _ = col;
            SqlValue::String(format!("TIME({intervals})"))
        }
    })
}

/// DATETIME2 — TIME bytes (scale-driven) + 3 date bytes, 1-byte length prefix.
fn parse_datetime2(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol(
            "unexpected EOF reading DATETIME2 length".into(),
        ));
    }
    let len = buf.get_u8() as usize;
    Ok(if len == 0 {
        SqlValue::Null
    } else if buf.remaining() < len {
        return Err(Error::Protocol("unexpected EOF reading DATETIME2".into()));
    } else {
        let scale = col.type_info.scale.unwrap_or(7);
        let time_len = time_bytes_for_scale(scale);
        // Reads below are driven by scale metadata, not by `len`:
        // a short declared length must be an error, not a panic.
        if len < time_len + 3 {
            return Err(Error::Protocol(format!(
                "DATETIME2 length {len} too short for scale {scale}"
            )));
        }

        // Read time
        let mut time_bytes = [0u8; 8];
        for byte in time_bytes.iter_mut().take(time_len) {
            *byte = buf.get_u8();
        }
        let intervals = u64::from_le_bytes(time_bytes);

        // Read date (3 bytes)
        let days =
            buf.get_u8() as u32 | ((buf.get_u8() as u32) << 8) | ((buf.get_u8() as u32) << 16);

        #[cfg(feature = "chrono")]
        {
            let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
            let date = base
                .checked_add_signed(chrono::Duration::days(days as i64))
                .ok_or_else(|| Error::Protocol(format!("date field days out of range: {days}")))?;
            let time = intervals_to_time(intervals, scale);
            SqlValue::DateTime(date.and_time(time))
        }
        #[cfg(not(feature = "chrono"))]
        {
            SqlValue::String(format!("DATETIME2({days},{intervals})"))
        }
    })
}

/// DATETIMEOFFSET — TIME bytes (scale-driven) + 3 date bytes + 2 offset bytes.
fn parse_datetimeoffset(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol(
            "unexpected EOF reading DATETIMEOFFSET length".into(),
        ));
    }
    let len = buf.get_u8() as usize;
    Ok(if len == 0 {
        SqlValue::Null
    } else if buf.remaining() < len {
        return Err(Error::Protocol(
            "unexpected EOF reading DATETIMEOFFSET".into(),
        ));
    } else {
        let scale = col.type_info.scale.unwrap_or(7);
        let time_len = time_bytes_for_scale(scale);
        // Reads below are driven by scale metadata, not by `len`:
        // a short declared length must be an error, not a panic.
        if len < time_len + 5 {
            return Err(Error::Protocol(format!(
                "DATETIMEOFFSET length {len} too short for scale {scale}"
            )));
        }

        // Read time
        let mut time_bytes = [0u8; 8];
        for byte in time_bytes.iter_mut().take(time_len) {
            *byte = buf.get_u8();
        }
        let intervals = u64::from_le_bytes(time_bytes);

        // Read date (3 bytes)
        let days =
            buf.get_u8() as u32 | ((buf.get_u8() as u32) << 8) | ((buf.get_u8() as u32) << 16);

        // Read offset in minutes (2 bytes, signed)
        let offset_minutes = buf.get_i16_le();

        #[cfg(feature = "chrono")]
        {
            use chrono::TimeZone;
            let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
            let date = base
                .checked_add_signed(chrono::Duration::days(days as i64))
                .ok_or_else(|| Error::Protocol(format!("date field days out of range: {days}")))?;
            let time = intervals_to_time(intervals, scale);
            let offset = chrono::FixedOffset::east_opt((offset_minutes as i32) * 60)
                .unwrap_or_else(|| {
                    chrono::FixedOffset::east_opt(0).expect("UTC offset 0 is valid")
                });
            // The wire date/time portion is UTC per MS-TDS §2.2.5.5.1.9;
            // attach the offset without shifting the instant.
            let datetime = offset.from_utc_datetime(&date.and_time(time));
            SqlValue::DateTimeOffset(datetime)
        }
        #[cfg(not(feature = "chrono"))]
        {
            SqlValue::String(format!(
                "DATETIMEOFFSET({days},{intervals},{offset_minutes})"
            ))
        }
    })
}

/// CHAR / VARCHAR — legacy byte-length strings with a 1-byte length prefix.
fn parse_legacy_varchar(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol(
            "unexpected EOF reading legacy varchar length".into(),
        ));
    }
    let len = buf.get_u8();
    Ok(if len == 0xFF {
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
    })
}

/// BIGVARCHAR / BIGCHAR — 2-byte length prefix, or PLP for the MAX variant.
fn parse_bigvarchar(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    // Check if this is a MAX type (uses PLP encoding)
    if col.type_info.max_length == Some(0xFFFF) {
        // PLP format: 8-byte total length, then chunks
        return parse_plp_varchar(buf, col.type_info.collation.as_ref());
    }
    // 2-byte length prefix for non-MAX types
    if buf.remaining() < 2 {
        return Err(Error::Protocol(
            "unexpected EOF reading varchar length".into(),
        ));
    }
    let len = buf.get_u16_le();
    Ok(if len == 0xFFFF {
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
    })
}

/// NVARCHAR / NCHAR — 2-byte length prefix (bytes), or PLP for the MAX variant.
fn parse_nvarchar(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    // Check if this is a MAX type (uses PLP encoding)
    if col.type_info.max_length == Some(0xFFFF) {
        // PLP format: 8-byte total length, then chunks
        return parse_plp_nvarchar(buf);
    }
    // 2-byte length prefix (in bytes, not chars) for non-MAX types
    if buf.remaining() < 2 {
        return Err(Error::Protocol(
            "unexpected EOF reading nvarchar length".into(),
        ));
    }
    let len = buf.get_u16_le();
    Ok(if len == 0xFFFF {
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
    })
}

/// BINARY / VARBINARY — legacy byte-length binary with a 1-byte length prefix.
fn parse_legacy_varbinary(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol(
            "unexpected EOF reading legacy varbinary length".into(),
        ));
    }
    let len = buf.get_u8();
    Ok(if len == 0xFF {
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
    })
}

/// BIGVARBINARY / BIGBINARY — 2-byte length prefix, or PLP for the MAX variant.
fn parse_bigvarbinary(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    // Check if this is a MAX type (uses PLP encoding)
    if col.type_info.max_length == Some(0xFFFF) {
        // PLP format: 8-byte total length, then chunks
        return parse_plp_varbinary(buf);
    }
    if buf.remaining() < 2 {
        return Err(Error::Protocol(
            "unexpected EOF reading varbinary length".into(),
        ));
    }
    let len = buf.get_u16_le();
    Ok(if len == 0xFFFF {
        SqlValue::Null
    } else if buf.remaining() < len as usize {
        return Err(Error::Protocol(
            "unexpected EOF reading varbinary data".into(),
        ));
    } else {
        let data = bytes::Bytes::copy_from_slice(&buf[..len as usize]);
        buf.advance(len as usize);
        SqlValue::Binary(data)
    })
}

/// XML — PLP-encoded UTF-16, surfaced as [`SqlValue::Xml`].
fn parse_xml(buf: &mut &[u8]) -> Result<SqlValue> {
    // Parse as PLP NVARCHAR (XML is UTF-16 encoded in TDS)
    match parse_plp_nvarchar(buf)? {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::String(s) => Ok(SqlValue::Xml(s)),
        _ => Err(Error::Protocol(
            "unexpected value type when parsing XML".into(),
        )),
    }
}

/// UNIQUEIDENTIFIER — 16 bytes in SQL Server mixed-endian, 1-byte length prefix.
fn parse_guid(buf: &mut &[u8]) -> Result<SqlValue> {
    if buf.remaining() < 1 {
        return Err(Error::Protocol("unexpected EOF reading GUID length".into()));
    }
    let len = buf.get_u8();
    Ok(if len == 0 {
        SqlValue::Null
    } else if len != 16 {
        return Err(Error::Protocol(format!("invalid GUID length: {len}")));
    } else if buf.remaining() < 16 {
        return Err(Error::Protocol("unexpected EOF reading GUID".into()));
    } else {
        // SQL Server stores GUIDs in mixed-endian format:
        // first 3 groups byte-swapped, last 2 groups big-endian.
        // Swap back to RFC 4122 big-endian format.
        decode_guid_bytes(buf)
    })
}

/// Fallback: read an unrecognized type as binary with a 2-byte length prefix.
fn parse_default_binary(buf: &mut &[u8], col: &ColumnData) -> Result<SqlValue> {
    // Try to read as variable-length with 2-byte length
    if buf.remaining() < 2 {
        return Err(Error::Protocol(format!(
            "unexpected EOF reading {:?}",
            col.type_id
        )));
    }
    let len = buf.get_u16_le();
    Ok(if len == 0xFFFF {
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
    })
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
        0x6D => variant_floatn(buf, prop_count, data_len),
        0x6E => variant_moneyn(buf, prop_count, data_len),
        0x6F => variant_datetimen(buf, prop_count, data_len),
        0x6A | 0x6C => variant_decimal(buf, prop_count, data_len),
        0x24 => {
            // UNIQUEIDENTIFIER (no properties)
            buf.advance(prop_count);
            if data_len < 16 {
                return Ok(SqlValue::Null);
            }
            // SQL Server stores GUIDs in mixed-endian format — swap back to RFC 4122
            Ok(decode_guid_bytes(buf))
        }
        0x28 => variant_date(buf, prop_count, data_len),
        0x29 => variant_time(buf, prop_count, data_len),
        0x2A => variant_datetime2(buf, prop_count, data_len),
        0x2B => variant_datetimeoffset(buf, prop_count, data_len),
        0xA7 | 0x2F | 0x27 => variant_varchar(buf, prop_count, data_len),
        0xE7 | 0xEF => variant_nvarchar(buf, prop_count, data_len),
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

// =============================================================================
// Per-base-type SQL_VARIANT value parsers (the arms of `parse_sql_variant`,
// extracted for readability — see #309). Each receives the variant's property
// byte count and computed data length, consumes the property bytes and the
// value, and returns its `SqlValue`. Behavior is identical to the inlined arms.
// =============================================================================

/// SQL_VARIANT FLOATN — 1 property byte (length).
fn variant_floatn(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
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

/// SQL_VARIANT MONEYN — 1 property byte (length).
fn variant_moneyn(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
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

/// SQL_VARIANT DATETIMEN — 1 property byte (length).
fn variant_datetimen(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
    #[cfg(feature = "chrono")]
    let dt_len = if prop_count >= 1 { buf.get_u8() } else { 8 };
    #[cfg(not(feature = "chrono"))]
    if prop_count >= 1 {
        buf.get_u8();
    }
    buf.advance(prop_count.saturating_sub(1));

    #[cfg(feature = "chrono")]
    {
        if dt_len == 4 && data_len >= 4 {
            // SMALLDATETIME
            let days = buf.get_u16_le() as i64;
            let mins = buf.get_u16_le() as u32;
            Ok(SqlValue::DateTime(smalldatetime_from_wire(days, mins)?))
        } else if data_len >= 8 {
            // DATETIME
            let days = buf.get_i32_le() as i64;
            let ticks = buf.get_u32_le() as u64;
            Ok(SqlValue::DateTime(datetime_from_wire(days, ticks)?))
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

/// SQL_VARIANT DECIMALN/NUMERICN — 2 property bytes (precision, scale). Shares
/// the mantissa decode and the 96-bit/scale-28 overflow policy with the
/// top-level path and the secondary stack (#204): reframe the variant payload
/// (`[sign][mantissa]`, `data_len` bytes) as the `[len][sign][mantissa]` form
/// `decode_decimal` reads, with `data_len` standing in for the length byte.
fn variant_decimal(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
    let precision = if prop_count >= 1 { buf.get_u8() } else { 18 };
    let scale = if prop_count >= 2 { buf.get_u8() } else { 0 };
    buf.advance(prop_count.saturating_sub(2));

    // A valid NUMERIC/DECIMAL payload is at most 17 bytes (sign + 16 mantissa).
    // Anything larger is malformed; skip it and return Null, preserving the
    // pre-#204 behavior (the old `mantissa_len > 16` guard) rather than feeding
    // the shared decoder a payload it would partially decode. This also keeps
    // `data_len` within `u8`, so the length-prefix reframing below cannot
    // truncate or panic.
    if data_len > 17 {
        buf.advance(data_len);
        return Ok(SqlValue::Null);
    }

    let type_info = mssql_types::TypeInfo::decimal(precision, scale);
    let result = {
        let len_prefix = [data_len as u8];
        let mut framed = (&len_prefix[..]).chain(&buf[..data_len]);
        mssql_types::__private::decode_decimal(&mut framed, &type_info)
    };
    buf.advance(data_len);
    result.map_err(Into::into)
}

/// SQL_VARIANT DATE — no properties.
fn variant_date(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
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
        let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
        let date = base
            .checked_add_signed(chrono::Duration::days(days as i64))
            .ok_or_else(|| Error::Protocol(format!("date field days out of range: {days}")))?;
        Ok(SqlValue::Date(date))
    }
    #[cfg(not(feature = "chrono"))]
    {
        buf.advance(data_len);
        Ok(SqlValue::Null)
    }
}

/// SQL_VARIANT TIME — 1 property byte (scale).
fn variant_time(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
    #[cfg_attr(not(feature = "chrono"), allow(unused_variables))]
    let scale = if prop_count >= 1 { buf.get_u8() } else { 7 };
    buf.advance(prop_count.saturating_sub(1));

    #[cfg(feature = "chrono")]
    {
        if data_len == 0 {
            return Ok(SqlValue::Null);
        }
        let time_len = time_bytes_for_scale(scale);
        if data_len < time_len {
            return Ok(SqlValue::Null);
        }
        let mut time_bytes = [0u8; 8];
        for byte in time_bytes.iter_mut().take(time_len) {
            *byte = buf.get_u8();
        }
        // Consume any remaining data bytes beyond the time portion
        if data_len > time_len {
            buf.advance(data_len - time_len);
        }
        let intervals = u64::from_le_bytes(time_bytes);
        Ok(SqlValue::Time(intervals_to_time(intervals, scale)))
    }
    #[cfg(not(feature = "chrono"))]
    {
        buf.advance(data_len);
        Ok(SqlValue::Null)
    }
}

/// SQL_VARIANT DATETIME2 — 1 property byte (scale).
fn variant_datetime2(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
    #[cfg_attr(not(feature = "chrono"), allow(unused_variables))]
    let scale = if prop_count >= 1 { buf.get_u8() } else { 7 };
    buf.advance(prop_count.saturating_sub(1));

    #[cfg(feature = "chrono")]
    {
        let time_len = time_bytes_for_scale(scale);
        if data_len < time_len + 3 {
            return Ok(SqlValue::Null);
        }

        let mut time_bytes = [0u8; 8];
        for byte in time_bytes.iter_mut().take(time_len) {
            *byte = buf.get_u8();
        }
        let intervals = u64::from_le_bytes(time_bytes);

        let days =
            buf.get_u8() as u32 | ((buf.get_u8() as u32) << 8) | ((buf.get_u8() as u32) << 16);

        // Consume any remaining data bytes
        let consumed = time_len + 3;
        if data_len > consumed {
            buf.advance(data_len - consumed);
        }

        let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
        let date = base
            .checked_add_signed(chrono::Duration::days(days as i64))
            .ok_or_else(|| Error::Protocol(format!("date field days out of range: {days}")))?;
        let time = intervals_to_time(intervals, scale);
        Ok(SqlValue::DateTime(date.and_time(time)))
    }
    #[cfg(not(feature = "chrono"))]
    {
        buf.advance(data_len);
        Ok(SqlValue::Null)
    }
}

/// SQL_VARIANT DATETIMEOFFSET — 1 property byte (scale).
fn variant_datetimeoffset(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
    #[cfg_attr(not(feature = "chrono"), allow(unused_variables))]
    let scale = if prop_count >= 1 { buf.get_u8() } else { 7 };
    buf.advance(prop_count.saturating_sub(1));

    #[cfg(feature = "chrono")]
    {
        let time_len = time_bytes_for_scale(scale);
        if data_len < time_len + 3 + 2 {
            return Ok(SqlValue::Null);
        }

        let mut time_bytes = [0u8; 8];
        for byte in time_bytes.iter_mut().take(time_len) {
            *byte = buf.get_u8();
        }
        let intervals = u64::from_le_bytes(time_bytes);

        let days =
            buf.get_u8() as u32 | ((buf.get_u8() as u32) << 8) | ((buf.get_u8() as u32) << 16);

        let offset_minutes = buf.get_i16_le();

        // Consume any remaining data bytes
        let consumed = time_len + 3 + 2;
        if data_len > consumed {
            buf.advance(data_len - consumed);
        }

        use chrono::TimeZone;
        let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
        let date = base
            .checked_add_signed(chrono::Duration::days(days as i64))
            .ok_or_else(|| Error::Protocol(format!("date field days out of range: {days}")))?;
        let time = intervals_to_time(intervals, scale);
        let offset = chrono::FixedOffset::east_opt((offset_minutes as i32) * 60)
            .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("UTC offset 0 is valid"));
        // The wire date/time portion is UTC per MS-TDS §2.2.5.5.1.9;
        // attach the offset without shifting the instant.
        let datetime = offset.from_utc_datetime(&date.and_time(time));
        Ok(SqlValue::DateTimeOffset(datetime))
    }
    #[cfg(not(feature = "chrono"))]
    {
        buf.advance(data_len);
        Ok(SqlValue::Null)
    }
}

/// SQL_VARIANT BigVarChar/BigChar/VarChar/Char — 7 property bytes
/// (collation 5 + max length 2).
fn variant_varchar(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
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

/// SQL_VARIANT NVarChar/NChar — 7 property bytes (collation 5 + max length 2).
fn variant_nvarchar(buf: &mut &[u8], prop_count: usize, data_len: usize) -> Result<SqlValue> {
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

/// Decode 16 GUID bytes from SQL Server mixed-endian wire format to RFC 4122 format.
///
/// SQL Server stores UUIDs with the first 3 groups byte-swapped (little-endian)
/// and the last 2 groups in big-endian order. This function reads 16 bytes,
/// swaps the first 3 groups back, and returns the appropriate SqlValue.
fn decode_guid_bytes(buf: &mut &[u8]) -> SqlValue {
    let mut bytes = [0u8; 16];

    // First 4 bytes — little-endian on wire, swap to big-endian
    bytes[3] = buf.get_u8();
    bytes[2] = buf.get_u8();
    bytes[1] = buf.get_u8();
    bytes[0] = buf.get_u8();

    // Next 2 bytes — little-endian on wire, swap to big-endian
    bytes[5] = buf.get_u8();
    bytes[4] = buf.get_u8();

    // Next 2 bytes — little-endian on wire, swap to big-endian
    bytes[7] = buf.get_u8();
    bytes[6] = buf.get_u8();

    // Last 8 bytes — big-endian, keep as-is
    for byte in &mut bytes[8..16] {
        *byte = buf.get_u8();
    }

    #[cfg(feature = "uuid")]
    {
        SqlValue::Uuid(uuid::Uuid::from_bytes(bytes))
    }
    #[cfg(not(feature = "uuid"))]
    {
        SqlValue::Binary(bytes::Bytes::copy_from_slice(&bytes))
    }
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

    // ========================================================================
    // Hostile wire data: out-of-range date/time values from a malicious or
    // buggy server must produce protocol errors, never panics.
    // ========================================================================

    #[cfg(feature = "chrono")]
    fn datetime_col(type_id: TypeId, col_type: u8, max_length: Option<u32>) -> ColumnData {
        ColumnData {
            name: "c".to_string(),
            type_id,
            col_type,
            flags: 0x01,
            user_type: 0,
            type_info: TypeInfo {
                max_length,
                precision: None,
                scale: None,
                collation: None,
            },
            crypto_metadata: None,
        }
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_smalldatetime_minutes_is_error_not_panic() {
        // DateTimeN, len=4: days=0, minutes=0xFFFF (>= 1440 is invalid)
        let data = [4u8, 0x00, 0x00, 0xFF, 0xFF];
        let col = datetime_col(TypeId::DateTimeN, 0x6F, Some(4));
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());

        // Fixed SMALLDATETIME (DateTime4): same payload, no length prefix
        let data = [0x00, 0x00, 0xFF, 0xFF];
        let col = datetime_col(TypeId::DateTime4, 0x3A, None);
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_datetime_days_overflow_is_error_not_panic() {
        // DateTimeN, len=8: days=i32::MAX overflows chrono's date range
        let mut data = vec![8u8];
        data.extend_from_slice(&i32::MAX.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let col = datetime_col(TypeId::DateTimeN, 0x6F, Some(8));
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());

        // Fixed DATETIME: days=i32::MIN
        let mut data = Vec::new();
        data.extend_from_slice(&i32::MIN.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let col = datetime_col(TypeId::DateTime, 0x3D, None);
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_datetime_time_300ths_is_error_not_panic() {
        // DateTimeN, len=8: valid days, time_300ths=u32::MAX (> 24h of 300ths)
        let mut data = vec![8u8];
        data.extend_from_slice(&0i32.to_le_bytes());
        data.extend_from_slice(&u32::MAX.to_le_bytes());
        let col = datetime_col(TypeId::DateTimeN, 0x6F, Some(8));
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_truncated_n_types_are_error_not_panic() {
        // Declared length with no payload bytes behind it (found by the
        // parse_column_value fuzz target on FloatN).
        for (type_id, col_type, len) in [
            (TypeId::IntN, 0x26u8, 8u8),
            (TypeId::FloatN, 0x6D, 4),
            (TypeId::BitN, 0x68, 1),
        ] {
            let data = [len];
            let col = datetime_col(type_id, col_type, Some(len as u32));
            let mut buf: &[u8] = &data;
            assert!(
                parse_column_value(&mut buf, &col).is_err(),
                "{type_id:?} must error on truncated payload"
            );
        }
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_short_datetime2_len_is_error_not_panic() {
        // DATETIME2 declared len=1 (1 byte present) but scale 7 implies
        // time_len=5 + 3 date bytes; reads are scale-driven, so a short
        // declared length must error.
        let data = [1u8, 0xAA];
        let mut col = datetime_col(TypeId::DateTime2, 0x2A, None);
        col.type_info.scale = Some(7);
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());

        // Same for DATETIMEOFFSET (time_len + 5)
        let data = [1u8, 0xAA];
        let mut col = datetime_col(TypeId::DateTimeOffset, 0x2B, None);
        col.type_info.scale = Some(7);
        let mut buf: &[u8] = &data;
        assert!(parse_column_value(&mut buf, &col).is_err());
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_variant_datetime_days_overflow_is_error_not_panic() {
        // SQL_VARIANT: total_len=11, base_type=0x6F (DATETIMEN), prop_count=1,
        // dt_len=8, then days=i32::MAX + ticks=0
        let mut data = Vec::new();
        data.extend_from_slice(&11u32.to_le_bytes());
        data.push(0x6F);
        data.push(0x01);
        data.push(0x08);
        data.extend_from_slice(&i32::MAX.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let mut buf: &[u8] = &data;
        assert!(parse_sql_variant(&mut buf).is_err());
    }

    #[cfg(feature = "decimal")]
    #[test]
    fn hostile_variant_decimal_over_96bit_is_error_not_panic() {
        // SQL_VARIANT DECIMALN (0x6A) with a 16-byte mantissa above 2^96.
        // rust_decimal cannot hold it; this must be a descriptive error,
        // never a panic — and never a silent f64 fallback, which corrupted
        // legitimate 38-digit NUMERIC values down to ~15-16 significant
        // digits (issue #157).
        // total_len=21: base_type + prop_count + precision + scale + sign(1)
        // + mantissa(16) = 2 + 2 + 17.
        let mantissa = 1u128 << 100;
        let mut data = Vec::new();
        data.extend_from_slice(&21u32.to_le_bytes());
        data.push(0x6A); // DECIMALN
        data.push(0x02); // prop_count: precision, scale
        data.push(38); // precision
        data.push(10); // scale
        data.push(0x01); // sign (positive)
        data.extend_from_slice(&mantissa.to_le_bytes());
        let mut buf: &[u8] = &data;
        let err = parse_sql_variant(&mut buf).expect_err("oversized NUMERIC must error");
        assert!(
            err.to_string().contains("rust_decimal"),
            "error should explain the range limitation: {err}"
        );
    }

    /// A valid SQL_VARIANT DECIMALN decodes through the shared decoder (#204):
    /// 123.45 as NUMERIC(5,2). total_len=7: base_type + prop_count + precision
    /// + scale + sign(1) + mantissa(2).
    #[cfg(feature = "decimal")]
    #[test]
    fn variant_decimal_decodes_via_shared_decoder() {
        let mut data = Vec::new();
        data.extend_from_slice(&7u32.to_le_bytes());
        data.push(0x6A); // DECIMALN
        data.push(0x02); // prop_count: precision, scale
        data.push(5); // precision
        data.push(2); // scale
        data.push(0x01); // sign (positive)
        data.extend_from_slice(&12345u16.to_le_bytes()); // mantissa LE
        let mut buf: &[u8] = &data;
        let value = parse_sql_variant(&mut buf).expect("valid NUMERIC must decode");
        assert_eq!(value, SqlValue::Decimal("123.45".parse().unwrap()));
    }

    /// A SQL_VARIANT DECIMALN whose payload exceeds the 17-byte NUMERIC maximum
    /// (sign + 16 mantissa) is malformed: it decodes to Null, not through the
    /// shared decoder. data_len = 18 (sign + 17 mantissa); total_len = 22.
    #[test]
    fn variant_decimal_oversized_payload_is_null() {
        let mut data = Vec::new();
        data.extend_from_slice(&22u32.to_le_bytes());
        data.push(0x6A); // DECIMALN
        data.push(0x02); // prop_count: precision, scale
        data.push(38); // precision
        data.push(0); // scale
        data.push(0x01); // sign
        data.extend_from_slice(&[0u8; 17]); // 17 mantissa bytes => data_len 18
        let mut buf: &[u8] = &data;
        let value = parse_sql_variant(&mut buf).expect("oversized payload must not error");
        assert_eq!(value, SqlValue::Null);
        assert!(buf.is_empty(), "the whole payload must be consumed");
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn hostile_time_intervals_do_not_panic() {
        // intervals_to_time multiplies wire-supplied u64 by up to 1e9; must
        // not overflow-panic in debug builds. Falls back to midnight today.
        let t = intervals_to_time(u64::MAX, 0);
        let _ = t; // any non-panicking result is acceptable
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
