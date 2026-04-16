//! Parameter conversion for SQL Server RPC calls.
//!
//! This module converts Rust types (via `ToSql`) into TDS wire-format
//! RPC parameters, including Table-Valued Parameter (TVP) encoding.

use bytes::BytesMut;
use tds_protocol::rpc::{RpcParam, TypeInfo as RpcTypeInfo};
#[cfg(feature = "decimal")]
use tds_protocol::tvp::encode_tvp_decimal;
use tds_protocol::tvp::{
    TvpColumnDef as TvpWireColumnDef, TvpEncoder, TvpWireType, encode_tvp_bit, encode_tvp_float,
    encode_tvp_int, encode_tvp_null, encode_tvp_nvarchar, encode_tvp_varbinary,
};

use crate::error::{Error, Result};
use crate::state::ConnectionState;

use super::Client;

impl<S: ConnectionState> Client<S> {
    /// Convert a `SqlValue` into an `RpcParam` with the given name.
    ///
    /// This is the core conversion logic shared by positional parameters
    /// (`convert_params`), named procedure parameters (`ProcedureBuilder::input`),
    /// and named query parameters (`convert_named_params`).
    ///
    /// When `send_unicode` is `false`, `SqlValue::String` values are encoded
    /// as VARCHAR (single-byte) instead of NVARCHAR (UTF-16), which allows
    /// SQL Server to use index seeks on VARCHAR columns.
    pub(crate) fn sql_value_to_rpc_param(
        name: &str,
        sql_value: &mssql_types::SqlValue,
        send_unicode: bool,
    ) -> Result<RpcParam> {
        use bytes::{BufMut, BytesMut};
        use mssql_types::SqlValue;

        Ok(match sql_value {
            SqlValue::Null => RpcParam::null(name, RpcTypeInfo::nvarchar(1)),
            SqlValue::Bool(v) => {
                let mut buf = BytesMut::with_capacity(1);
                buf.put_u8(if *v { 1 } else { 0 });
                RpcParam::new(name, RpcTypeInfo::bit(), buf.freeze())
            }
            SqlValue::TinyInt(v) => {
                let mut buf = BytesMut::with_capacity(1);
                buf.put_u8(*v);
                RpcParam::new(name, RpcTypeInfo::tinyint(), buf.freeze())
            }
            SqlValue::SmallInt(v) => {
                let mut buf = BytesMut::with_capacity(2);
                buf.put_i16_le(*v);
                RpcParam::new(name, RpcTypeInfo::smallint(), buf.freeze())
            }
            SqlValue::Int(v) => RpcParam::int(name, *v),
            SqlValue::BigInt(v) => RpcParam::bigint(name, *v),
            SqlValue::Float(v) => {
                let mut buf = BytesMut::with_capacity(4);
                buf.put_f32_le(*v);
                RpcParam::new(name, RpcTypeInfo::real(), buf.freeze())
            }
            SqlValue::Double(v) => {
                let mut buf = BytesMut::with_capacity(8);
                buf.put_f64_le(*v);
                RpcParam::new(name, RpcTypeInfo::float(), buf.freeze())
            }
            SqlValue::String(s) => {
                if send_unicode {
                    RpcParam::nvarchar(name, s)
                } else {
                    RpcParam::varchar(name, s)
                }
            }
            SqlValue::Binary(b) => {
                RpcParam::new(name, RpcTypeInfo::varbinary(b.len() as u16), b.clone())
            }
            SqlValue::Xml(s) => RpcParam::nvarchar(name, s),
            #[cfg(feature = "uuid")]
            SqlValue::Uuid(u) => {
                let bytes = u.as_bytes();
                let mut buf = BytesMut::with_capacity(16);
                // SQL Server stores GUIDs in mixed-endian format
                buf.put_u32_le(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                buf.put_u16_le(u16::from_be_bytes([bytes[4], bytes[5]]));
                buf.put_u16_le(u16::from_be_bytes([bytes[6], bytes[7]]));
                buf.put_slice(&bytes[8..16]);
                RpcParam::new(name, RpcTypeInfo::uniqueidentifier(), buf.freeze())
            }
            #[cfg(feature = "decimal")]
            SqlValue::Decimal(d) => {
                let mut buf = BytesMut::with_capacity(17);
                mssql_types::encode::encode_decimal(*d, &mut buf);
                let scale = d.scale() as u8;
                RpcParam::new(name, RpcTypeInfo::decimal(38, scale), buf.freeze())
            }
            #[cfg(feature = "chrono")]
            SqlValue::Date(d) => {
                let mut buf = BytesMut::with_capacity(3);
                mssql_types::encode::encode_date(*d, &mut buf);
                RpcParam::new(name, RpcTypeInfo::date(), buf.freeze())
            }
            #[cfg(feature = "chrono")]
            SqlValue::Time(t) => {
                let mut buf = BytesMut::with_capacity(5);
                mssql_types::encode::encode_time(*t, &mut buf);
                RpcParam::new(name, RpcTypeInfo::time(7), buf.freeze())
            }
            #[cfg(feature = "chrono")]
            SqlValue::DateTime(dt) => {
                let mut buf = BytesMut::with_capacity(8);
                mssql_types::encode::encode_datetime2(*dt, &mut buf);
                RpcParam::new(name, RpcTypeInfo::datetime2(7), buf.freeze())
            }
            #[cfg(feature = "chrono")]
            SqlValue::DateTimeOffset(dto) => {
                let mut buf = BytesMut::with_capacity(10);
                mssql_types::encode::encode_datetimeoffset(*dto, &mut buf);
                RpcParam::new(name, RpcTypeInfo::datetimeoffset(7), buf.freeze())
            }
            #[cfg(feature = "json")]
            SqlValue::Json(j) => RpcParam::nvarchar(name, &j.to_string()),
            SqlValue::Tvp(tvp_data) => Self::encode_tvp_param(name, tvp_data)?,
            _ => {
                return Err(Error::Type(mssql_types::TypeError::UnsupportedConversion {
                    from: sql_value.type_name().to_string(),
                    to: "RPC parameter",
                }));
            }
        })
    }

    /// Convert a single `ToSql` value into an `RpcParam` with the given name.
    ///
    /// This is the shared conversion logic used by both `convert_params()`
    /// (for positional query parameters) and `ProcedureBuilder::input()`
    /// (for named procedure parameters).
    pub(crate) fn convert_single_param(
        name: &str,
        value: &(dyn crate::ToSql + Sync),
        send_unicode: bool,
    ) -> Result<RpcParam> {
        let sql_value = value.to_sql()?;
        Self::sql_value_to_rpc_param(name, &sql_value, send_unicode)
    }

    /// Convert ToSql parameters to RPC parameters with auto-generated names.
    pub(crate) fn convert_params(
        params: &[&(dyn crate::ToSql + Sync)],
        send_unicode: bool,
    ) -> Result<Vec<RpcParam>> {
        params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let name = format!("@p{}", i + 1);
                Self::convert_single_param(&name, *p, send_unicode)
            })
            .collect()
    }

    /// Convert named parameters to RPC parameters.
    ///
    /// Ensures each parameter name has an `@` prefix as required by
    /// `sp_executesql` parameter declarations.
    pub(crate) fn convert_named_params(
        params: &[crate::to_params::NamedParam],
        send_unicode: bool,
    ) -> Result<Vec<RpcParam>> {
        params
            .iter()
            .map(|p| {
                let name = if p.name.starts_with('@') {
                    p.name.clone()
                } else {
                    format!("@{}", p.name)
                };
                Self::sql_value_to_rpc_param(&name, &p.value, send_unicode)
            })
            .collect()
    }

    /// Encode a TVP parameter for RPC.
    ///
    /// This encodes the complete TVP structure including metadata and row data
    /// into the TDS wire format.
    fn encode_tvp_param(name: &str, tvp_data: &mssql_types::TvpData) -> Result<RpcParam> {
        // Convert mssql-types column definitions to wire format
        let wire_columns: Vec<TvpWireColumnDef> = tvp_data
            .columns
            .iter()
            .map(|col| {
                let wire_type = Self::convert_tvp_column_type(&col.column_type)?;
                Ok(if col.nullable {
                    TvpWireColumnDef::nullable(wire_type)
                } else {
                    TvpWireColumnDef::new(wire_type)
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Create encoder
        let encoder = TvpEncoder::new(&tvp_data.schema, &tvp_data.type_name, &wire_columns);

        // Encode to buffer
        let mut buf = BytesMut::with_capacity(256);

        // Encode metadata
        encoder.encode_metadata(&mut buf);

        // Encode each row
        for row in &tvp_data.rows {
            encoder.encode_row(&mut buf, |row_buf| {
                for (col_idx, value) in row.iter().enumerate() {
                    let wire_type = &wire_columns[col_idx].wire_type;
                    Self::encode_tvp_value(value, wire_type, row_buf);
                }
            });
        }

        // Encode end marker
        encoder.encode_end(&mut buf);

        // Build the full TVP type name (schema.TypeName)
        let full_type_name = if tvp_data.schema.is_empty() {
            tvp_data.type_name.clone()
        } else {
            format!("{}.{}", tvp_data.schema, tvp_data.type_name)
        };

        // Create RPC param with TVP type info
        // The type info includes the TVP type name for parameter declarations
        let type_info = RpcTypeInfo::tvp(&full_type_name);

        Ok(RpcParam {
            name: name.to_string(),
            flags: tds_protocol::rpc::ParamFlags::default(),
            type_info,
            value: Some(buf.freeze()),
        })
    }

    /// Convert mssql-types TvpColumnType to wire TvpWireType.
    fn convert_tvp_column_type(col_type: &mssql_types::TvpColumnType) -> Result<TvpWireType> {
        // TvpColumnType is #[non_exhaustive], so the wildcard arm is required
        // for forward compatibility even though all current variants are covered.
        #[allow(unreachable_patterns)]
        Ok(match col_type {
            mssql_types::TvpColumnType::Bit => TvpWireType::Bit,
            mssql_types::TvpColumnType::TinyInt => TvpWireType::Int { size: 1 },
            mssql_types::TvpColumnType::SmallInt => TvpWireType::Int { size: 2 },
            mssql_types::TvpColumnType::Int => TvpWireType::Int { size: 4 },
            mssql_types::TvpColumnType::BigInt => TvpWireType::Int { size: 8 },
            mssql_types::TvpColumnType::Real => TvpWireType::Float { size: 4 },
            mssql_types::TvpColumnType::Float => TvpWireType::Float { size: 8 },
            mssql_types::TvpColumnType::Decimal { precision, scale } => TvpWireType::Decimal {
                precision: *precision,
                scale: *scale,
            },
            mssql_types::TvpColumnType::NVarChar { max_length } => TvpWireType::NVarChar {
                max_length: *max_length,
            },
            mssql_types::TvpColumnType::VarChar { max_length } => TvpWireType::VarChar {
                max_length: *max_length,
            },
            mssql_types::TvpColumnType::VarBinary { max_length } => TvpWireType::VarBinary {
                max_length: *max_length,
            },
            mssql_types::TvpColumnType::UniqueIdentifier => TvpWireType::Guid,
            mssql_types::TvpColumnType::Date => TvpWireType::Date,
            mssql_types::TvpColumnType::Time { scale } => TvpWireType::Time { scale: *scale },
            mssql_types::TvpColumnType::DateTime2 { scale } => {
                TvpWireType::DateTime2 { scale: *scale }
            }
            mssql_types::TvpColumnType::DateTimeOffset { scale } => {
                TvpWireType::DateTimeOffset { scale: *scale }
            }
            mssql_types::TvpColumnType::Xml => TvpWireType::Xml,
            _ => {
                return Err(Error::Type(mssql_types::TypeError::UnsupportedConversion {
                    from: format!("{col_type:?}"),
                    to: "TVP wire type",
                }));
            }
        })
    }

    /// Encode a single TVP column value.
    fn encode_tvp_value(
        value: &mssql_types::SqlValue,
        wire_type: &TvpWireType,
        buf: &mut BytesMut,
    ) {
        use mssql_types::SqlValue;

        match value {
            SqlValue::Null => {
                encode_tvp_null(wire_type, buf);
            }
            SqlValue::Bool(v) => {
                encode_tvp_bit(*v, buf);
            }
            SqlValue::TinyInt(v) => {
                encode_tvp_int(*v as i64, 1, buf);
            }
            SqlValue::SmallInt(v) => {
                encode_tvp_int(*v as i64, 2, buf);
            }
            SqlValue::Int(v) => {
                encode_tvp_int(*v as i64, 4, buf);
            }
            SqlValue::BigInt(v) => {
                encode_tvp_int(*v, 8, buf);
            }
            SqlValue::Float(v) => {
                encode_tvp_float(*v as f64, 4, buf);
            }
            SqlValue::Double(v) => {
                encode_tvp_float(*v, 8, buf);
            }
            SqlValue::String(s) => {
                let max_len = match wire_type {
                    TvpWireType::NVarChar { max_length } => *max_length,
                    _ => 4000,
                };
                encode_tvp_nvarchar(s, max_len, buf);
            }
            SqlValue::Binary(b) => {
                let max_len = match wire_type {
                    TvpWireType::VarBinary { max_length } => *max_length,
                    _ => 8000,
                };
                encode_tvp_varbinary(b, max_len, buf);
            }
            #[cfg(feature = "decimal")]
            SqlValue::Decimal(d) => {
                let sign = if d.is_sign_negative() { 0u8 } else { 1u8 };
                let mantissa = d.mantissa().unsigned_abs();
                encode_tvp_decimal(sign, mantissa, buf);
            }
            #[cfg(feature = "uuid")]
            SqlValue::Uuid(u) => {
                let bytes = u.as_bytes();
                tds_protocol::tvp::encode_tvp_guid(bytes, buf);
            }
            #[cfg(feature = "chrono")]
            SqlValue::Date(d) => {
                // Calculate days since 0001-01-01
                let base =
                    chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
                let days = d.signed_duration_since(base).num_days() as u32;
                tds_protocol::tvp::encode_tvp_date(days, buf);
            }
            #[cfg(feature = "chrono")]
            SqlValue::Time(t) => {
                use chrono::Timelike;
                let nanos =
                    t.num_seconds_from_midnight() as u64 * 1_000_000_000 + t.nanosecond() as u64;
                let intervals = nanos / 100;
                let scale = match wire_type {
                    TvpWireType::Time { scale } => *scale,
                    _ => 7,
                };
                tds_protocol::tvp::encode_tvp_time(intervals, scale, buf);
            }
            #[cfg(feature = "chrono")]
            SqlValue::DateTime(dt) => {
                use chrono::Timelike;
                // Time component
                let nanos = dt.time().num_seconds_from_midnight() as u64 * 1_000_000_000
                    + dt.time().nanosecond() as u64;
                let intervals = nanos / 100;
                // Date component
                let base =
                    chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
                let days = dt.date().signed_duration_since(base).num_days() as u32;
                let scale = match wire_type {
                    TvpWireType::DateTime2 { scale } => *scale,
                    _ => 7,
                };
                tds_protocol::tvp::encode_tvp_datetime2(intervals, days, scale, buf);
            }
            #[cfg(feature = "chrono")]
            SqlValue::DateTimeOffset(dto) => {
                use chrono::{Offset, Timelike};
                // Time component (in 100-nanosecond intervals)
                let nanos = dto.time().num_seconds_from_midnight() as u64 * 1_000_000_000
                    + dto.time().nanosecond() as u64;
                let intervals = nanos / 100;
                // Date component (days since 0001-01-01)
                let base =
                    chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("epoch 0001-01-01 is valid");
                let days = dto.date_naive().signed_duration_since(base).num_days() as u32;
                // Timezone offset in minutes
                let offset_minutes = (dto.offset().fix().local_minus_utc() / 60) as i16;
                let scale = match wire_type {
                    TvpWireType::DateTimeOffset { scale } => *scale,
                    _ => 7,
                };
                tds_protocol::tvp::encode_tvp_datetimeoffset(
                    intervals,
                    days,
                    offset_minutes,
                    scale,
                    buf,
                );
            }
            #[cfg(feature = "json")]
            SqlValue::Json(j) => {
                // JSON is encoded as NVARCHAR
                encode_tvp_nvarchar(&j.to_string(), 0xFFFF, buf);
            }
            SqlValue::Xml(s) => {
                // XML is encoded as NVARCHAR for TVP
                encode_tvp_nvarchar(s, 0xFFFF, buf);
            }
            SqlValue::Tvp(_) => {
                // Nested TVPs are not supported
                encode_tvp_null(wire_type, buf);
            }
            // Handle future SqlValue variants as NULL
            _ => {
                encode_tvp_null(wire_type, buf);
            }
        }
    }
}
