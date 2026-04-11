//! Parameter conversion for SQL Server RPC calls.
//!
//! This module converts Rust types (via `ToSql`) into TDS wire-format
//! RPC parameters, including Table-Valued Parameter (TVP) encoding.

use bytes::{BufMut, BytesMut};
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

/// Metadata for a stored procedure parameter.
#[derive(Debug, Clone)]
struct ProcParamMetadata {
    name: String,
    position: usize,
    is_output: bool,
    max_length: Option<usize>,
    precision: Option<u8>,
    scale: Option<u8>,
    type_name: String,
}

impl<S: ConnectionState> Client<S> {
    /// Convert ToSql parameters to RPC parameters.
    pub(crate) fn convert_params(params: &[&(dyn crate::ToSql + Sync)]) -> Result<Vec<RpcParam>> {
        use bytes::{BufMut, BytesMut};
        use mssql_types::SqlValue;

        params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let sql_value = p.to_sql()?;
                let name = format!("@p{}", i + 1);

                Ok(match sql_value {
                    SqlValue::Null => RpcParam::null(&name, RpcTypeInfo::nvarchar(1)),
                    SqlValue::Bool(v) => {
                        let mut buf = BytesMut::with_capacity(1);
                        buf.put_u8(if v { 1 } else { 0 });
                        RpcParam::new(&name, RpcTypeInfo::bit(), buf.freeze())
                    }
                    SqlValue::TinyInt(v) => {
                        let mut buf = BytesMut::with_capacity(1);
                        buf.put_u8(v);
                        RpcParam::new(&name, RpcTypeInfo::tinyint(), buf.freeze())
                    }
                    SqlValue::SmallInt(v) => {
                        let mut buf = BytesMut::with_capacity(2);
                        buf.put_i16_le(v);
                        RpcParam::new(&name, RpcTypeInfo::smallint(), buf.freeze())
                    }
                    SqlValue::Int(v) => RpcParam::int(&name, v),
                    SqlValue::BigInt(v) => RpcParam::bigint(&name, v),
                    SqlValue::Float(v) => {
                        let mut buf = BytesMut::with_capacity(4);
                        buf.put_f32_le(v);
                        RpcParam::new(&name, RpcTypeInfo::real(), buf.freeze())
                    }
                    SqlValue::Double(v) => {
                        let mut buf = BytesMut::with_capacity(8);
                        buf.put_f64_le(v);
                        RpcParam::new(&name, RpcTypeInfo::float(), buf.freeze())
                    }
                    SqlValue::String(ref s) => RpcParam::nvarchar(&name, s),
                    SqlValue::Binary(ref b) => {
                        RpcParam::new(&name, RpcTypeInfo::varbinary(b.len() as u16), b.clone())
                    }
                    SqlValue::Xml(ref s) => RpcParam::nvarchar(&name, s),
                    #[cfg(feature = "uuid")]
                    SqlValue::Uuid(u) => {
                        // UUID is stored in a specific byte order for SQL Server
                        let bytes = u.as_bytes();
                        let mut buf = BytesMut::with_capacity(16);
                        // SQL Server stores GUIDs in mixed-endian format
                        buf.put_u32_le(u32::from_be_bytes([
                            bytes[0], bytes[1], bytes[2], bytes[3],
                        ]));
                        buf.put_u16_le(u16::from_be_bytes([bytes[4], bytes[5]]));
                        buf.put_u16_le(u16::from_be_bytes([bytes[6], bytes[7]]));
                        buf.put_slice(&bytes[8..16]);
                        RpcParam::new(&name, RpcTypeInfo::uniqueidentifier(), buf.freeze())
                    }
                    #[cfg(feature = "decimal")]
                    SqlValue::Decimal(d) => {
                        // Decimal encoding is complex; use string representation for now
                        RpcParam::nvarchar(&name, &d.to_string())
                    }
                    #[cfg(feature = "chrono")]
                    SqlValue::Date(_)
                    | SqlValue::Time(_)
                    | SqlValue::DateTime(_)
                    | SqlValue::DateTimeOffset(_) => {
                        // For date/time types, use string representation for simplicity
                        // A full implementation would encode these properly
                        let s = match &sql_value {
                            SqlValue::Date(d) => d.to_string(),
                            SqlValue::Time(t) => t.to_string(),
                            SqlValue::DateTime(dt) => dt.to_string(),
                            SqlValue::DateTimeOffset(dto) => dto.to_rfc3339(),
                            _ => unreachable!(),
                        };
                        RpcParam::nvarchar(&name, &s)
                    }
                    #[cfg(feature = "json")]
                    SqlValue::Json(ref j) => RpcParam::nvarchar(&name, &j.to_string()),
                    SqlValue::Tvp(ref tvp_data) => {
                        // Encode TVP using the wire format
                        Self::encode_tvp_param(&name, tvp_data)?
                    }
                    // Handle future SqlValue variants
                    _ => {
                        return Err(Error::Type(mssql_types::TypeError::UnsupportedConversion {
                            from: sql_value.type_name().to_string(),
                            to: "RPC parameter",
                        }));
                    }
                })
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
                let wire_type = Self::convert_tvp_column_type(&col.column_type);
                if col.nullable {
                    TvpWireColumnDef::nullable(wire_type)
                } else {
                    TvpWireColumnDef::new(wire_type)
                }
            })
            .collect();

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
    fn convert_tvp_column_type(col_type: &mssql_types::TvpColumnType) -> TvpWireType {
        // TvpColumnType is #[non_exhaustive], so the wildcard arm is required
        // for forward compatibility even though all current variants are covered.
        #[allow(unreachable_patterns)]
        match col_type {
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
            _ => unreachable!("unknown TvpColumnType variant"),
        }
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
                let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
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
                let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
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
                let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
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

    /// Fetch stored procedure parameter metadata from the database.
    ///
    /// This uses `sp_sproc_columns` which provides better version compatibility
    /// (SQL Server 2000+) compared to direct system catalog queries.
    ///
    /// This retrieves parameter information including name, position, type,
    /// and whether it's an OUTPUT parameter.
    async fn fetch_procedure_params_metadata(
        &mut self,
        proc_name: &str,
    ) -> Result<Vec<ProcParamMetadata>> {
        // Parse procedure name to extract schema and name
        let (schema, name) = if proc_name.contains('.') {
            let parts: Vec<&str> = proc_name.split('.').collect();
            (parts[0], parts[1])
        } else {
            ("dbo", proc_name)
        };

        // Escape single quotes in both parts
        let escaped_schema = schema.replace('\'', "''");
        let escaped_name = name.replace('\'', "''");

        // Try different approaches for sp_sproc_columns
        // Approach 1: With owner parameter
        self.send_sql_batch(&format!(
            "sp_sproc_columns @procedure_name = '{escaped_name}', @procedure_owner = '{escaped_schema}'"
        )).await?;
        let (_columns, rows) = self.read_query_response().await?;

        // If approach 1 returns no rows, try approach 2: Use simple name without schema
        let rows = if rows.is_empty() {
            tracing::debug!("No rows with owner parameter, trying without owner");
            self.send_sql_batch(&format!(
                "sp_sproc_columns @procedure_name = '{escaped_name}'"
            ))
            .await?;
            let (_, rows2) = self.read_query_response().await?;
            rows2
        } else {
            rows
        };

        tracing::debug!(
            proc_name = %proc_name,
            row_count = rows.len(),
            "sp_sproc_columns returned {} rows",
            rows.len()
        );

        let mut params = Vec::new();

        // Process rows from sp_sproc_columns
        // According to SQL Server documentation, sp_sproc_columns returns:
        // PROCEDURE_QUALIFIER(1), PROCEDURE_OWNER(2), PROCEDURE_NAME(3),
        // COLUMN_NAME(4), COLUMN_TYPE(5), DATA_TYPE(6), TYPE_NAME(7),
        // PRECISION(8), LENGTH(9), SCALE(10), RADIX(11), NULLABLE(12),
        // REMARKS(13), COLUMN_DEF(14), SQL_DATA_TYPE(15), SQL_DATETIME_SUB(16),
        // CHAR_OCTET_LENGTH(17), ORDINAL_POSITION(18), IS_NULLABLE(19)
        // Note: SQL Server returns 1-based column positions, so we subtract 1
        for (row_idx, row) in rows.iter().enumerate() {
            // Get parameter name (with @ prefix) - COLUMN_NAME is at position 4 -> index 3
            let name: String = match row.get(3) {
                Ok(n) => n,
                Err(e) => {
                    tracing::error!("Failed to get column 3 (name): {:?}", e);
                    return Err(Error::Type(e));
                }
            };

            // COLUMN_TYPE is at position 5 -> index 4 (as smallint)
            // But returns as string in some SQL Server versions
            let column_type_str: String = match row.get(4) {
                Ok(ct) => ct,
                Err(_) => {
                    // Try as i16
                    let ct: i16 = row.get(4)?;
                    ct.to_string()
                }
            };

            // Parse column type: 1 = INPUT, 2 = INPUT/OUTPUT, 4/5 = RETURN_VALUE
            let column_type: i16 = match column_type_str.trim().parse::<i16>() {
                Ok(ct) => ct,
                Err(_) => {
                    tracing::warn!(
                        row = row_idx,
                        column_type = %column_type_str,
                        "invalid COLUMN_TYPE, defaulting to INPUT"
                    );
                    1 // Default to INPUT
                }
            };

            // Skip RETURN_VALUE parameters (they're not actual parameters)
            if column_type == 4 || column_type == 5 {
                tracing::debug!(
                    name = %name,
                    "skipping RETURN_VALUE parameter"
                );
                continue;
            }

            // TYPE_NAME is at position 7 -> index 6
            let type_name: String = match row.get(6) {
                Ok(tn) => tn,
                Err(e) => {
                    tracing::error!("Failed to get column 6 (type_name): {:?}", e);
                    return Err(Error::Type(e));
                }
            };

            // PRECISION is at position 8 -> index 7
            let precision: Option<i32> = row.get(7).ok();

            // SCALE is at position 10 -> index 9
            let scale: Option<i16> = row.get(9).ok();

            // LENGTH is at position 9 -> index 8
            let length: Option<i32> = row.get(8).ok();

            // ORDINAL_POSITION is at position 18 -> index 17
            let position: i32 = match row.get(17) {
                Ok(pos) => pos,
                Err(e) => {
                    tracing::error!("Failed to get column 17 (position): {:?}", e);
                    return Err(Error::Type(e));
                }
            };

            // Determine max_length from precision or length
            let max_length = length.or(precision).map(|l| l as usize);

            // Determine if OUTPUT parameter
            let is_output = column_type == 2; // INPUT/OUTPUT

            tracing::debug!(
                name = %name,
                position = position,
                type_name = %type_name,
                is_output = is_output,
                "parameter metadata"
            );

            params.push(ProcParamMetadata {
                name,
                position: (position - 1) as usize, // Convert to 0-based
                is_output,
                max_length,
                precision: precision.map(|p| p as u8),
                scale: scale.map(|s| s as u8),
                type_name,
            });
        }

        // Sort by position to ensure correct order
        params.sort_by_key(|p| p.position);

        Ok(params)
    }

    /// Convert ToSql parameters to RPC parameters for stored procedure execution.
    ///
    /// This function:
    /// 1. Queries the database for stored procedure parameter metadata
    /// 2. Matches input parameters to procedure parameters by position
    /// 3. Automatically marks OUTPUT parameters
    /// 4. Converts ToSql values to RpcParam with correct type info
    ///
    /// **API Simplification**: Users only need to provide INPUT parameters.
    /// OUTPUT parameters are automatically detected and filled with NULL values.
    ///
    /// # Example
    ///
    /// For a procedure with:
    /// - 1 INPUT parameter: `@input INT`
    /// - 3 OUTPUT parameters: `@out1 INT OUTPUT`, `@out2 INT OUTPUT`, `@out3 INT OUTPUT`
    ///
    /// Users can provide just the input value:
    /// ```rust,ignore
    /// client.execute_procedure("dbo.MyProc", &[&42i32]).await?
    /// ```
    ///
    /// Or provide all parameters explicitly (backward compatible):
    /// ```rust,ignore
    /// client.execute_procedure("dbo.MyProc", &[&42i32, &None::<i32>, &None::<i32>, &None::<i32>]).await?
    /// ```
    pub(crate) async fn convert_params_for_procedure(
        &mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<Vec<RpcParam>> {
        // Fetch parameter metadata from database
        let metadata = self.fetch_procedure_params_metadata(proc_name).await?;

        // Count INPUT vs OUTPUT parameters
        let input_count = metadata.iter().filter(|p| !p.is_output).count();
        let output_count = metadata.iter().filter(|p| p.is_output).count();
        let total_count = metadata.len();

        tracing::debug!(
            proc_name = %proc_name,
            params_provided = params.len(),
            input_params = input_count,
            output_params = output_count,
            total_params = total_count,
            "procedure parameter summary"
        );

        // Determine if user provided only INPUT parameters or all parameters
        let user_provided_only_inputs = params.len() == input_count;

        if user_provided_only_inputs {
            tracing::debug!(
                "User provided only INPUT parameters ({}), OUTPUT parameters will be auto-filled",
                input_count
            );
        } else if params.len() != total_count {
            return Err(Error::Protocol(format!(
                "Parameter count mismatch for procedure {}: expected {} (all parameters) or {} (INPUT only), got {}",
                proc_name,
                total_count,
                input_count,
                params.len()
            )));
        }

        // Convert each metadata entry to RpcParam
        let mut rpc_params = Vec::new();
        let mut input_index = 0;

        for (meta_idx, param_meta) in metadata.iter().enumerate() {
            let param_name = &param_meta.name;

            // Determine if this parameter should use user-provided value or auto-generated NULL
            let rpc_param = if param_meta.is_output {
                // OUTPUT parameter: use NULL value
                tracing::debug!(
                    name = %param_name,
                    "auto-filling OUTPUT parameter with NULL"
                );
                let type_info = Self::type_info_from_metadata(param_meta);
                let mut rpc_param = RpcParam::null(param_name, type_info);
                rpc_param.flags = rpc_param.flags.output();
                rpc_param
            } else if user_provided_only_inputs {
                // INPUT parameter with simplified API: use user-provided value
                let param_value = params.get(input_index).ok_or_else(|| {
                    Error::Protocol(format!(
                        "Missing INPUT parameter at position {} for procedure {}",
                        input_index + 1,
                        proc_name
                    ))
                })?;

                let sql_value = param_value.to_sql()?;
                Self::sql_value_to_rpc_param(param_name, param_meta, sql_value)?
            } else {
                // Traditional API: use parameter at same position
                let param_value = params.get(meta_idx).ok_or_else(|| {
                    Error::Protocol(format!(
                        "Missing parameter at position {} for procedure {}",
                        meta_idx + 1,
                        proc_name
                    ))
                })?;

                let sql_value = param_value.to_sql()?;
                let mut rpc_param =
                    Self::sql_value_to_rpc_param(param_name, param_meta, sql_value)?;

                // Mark as OUTPUT if needed (for traditional API)
                if param_meta.is_output {
                    rpc_param.flags = rpc_param.flags.output();
                    tracing::debug!(name = %param_name, "marked parameter as OUTPUT");
                }

                rpc_param
            };

            rpc_params.push(rpc_param);

            // Only increment input_index for INPUT parameters in simplified API
            if !param_meta.is_output && user_provided_only_inputs {
                input_index += 1;
            }
        }

        Ok(rpc_params)
    }

    /// Convert a SQL value to RpcParam with type info from metadata.
    fn sql_value_to_rpc_param(
        param_name: &str,
        param_meta: &ProcParamMetadata,
        sql_value: mssql_types::SqlValue,
    ) -> Result<RpcParam> {
        use mssql_types::SqlValue;

        Ok(match sql_value {
            SqlValue::Null => {
                // Use type info from metadata to create NULL parameter
                let type_info = Self::type_info_from_metadata(param_meta);
                RpcParam::null(param_name, type_info)
            }
            SqlValue::Bool(v) => {
                let mut buf = BytesMut::with_capacity(1);
                buf.put_u8(if v { 1 } else { 0 });
                RpcParam::new(param_name, RpcTypeInfo::bit(), buf.freeze())
            }
            SqlValue::TinyInt(v) => {
                let mut buf = BytesMut::with_capacity(1);
                buf.put_u8(v);
                RpcParam::new(param_name, RpcTypeInfo::tinyint(), buf.freeze())
            }
            SqlValue::SmallInt(v) => {
                let mut buf = BytesMut::with_capacity(2);
                buf.put_i16_le(v);
                RpcParam::new(param_name, RpcTypeInfo::smallint(), buf.freeze())
            }
            SqlValue::Int(v) => RpcParam::int(param_name, v),
            SqlValue::BigInt(v) => RpcParam::bigint(param_name, v),
            SqlValue::Float(v) => {
                let mut buf = BytesMut::with_capacity(4);
                buf.put_f32_le(v);
                RpcParam::new(param_name, RpcTypeInfo::real(), buf.freeze())
            }
            SqlValue::Double(v) => {
                let mut buf = BytesMut::with_capacity(8);
                buf.put_f64_le(v);
                RpcParam::new(param_name, RpcTypeInfo::float(), buf.freeze())
            }
            SqlValue::String(ref s) => RpcParam::nvarchar(param_name, s),
            SqlValue::Binary(ref b) => RpcParam::new(
                param_name,
                RpcTypeInfo::varbinary(b.len() as u16),
                b.clone(),
            ),
            SqlValue::Xml(ref s) => RpcParam::nvarchar(param_name, s),
            #[cfg(feature = "uuid")]
            SqlValue::Uuid(u) => {
                // UUID is stored in a specific byte order for SQL Server
                let bytes = u.as_bytes();
                let mut buf = BytesMut::with_capacity(16);
                // SQL Server stores GUIDs in mixed-endian format
                buf.put_u32_le(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                buf.put_u16_le(u16::from_be_bytes([bytes[4], bytes[5]]));
                buf.put_u16_le(u16::from_be_bytes([bytes[6], bytes[7]]));
                buf.put_slice(&bytes[8..16]);
                RpcParam::new(param_name, RpcTypeInfo::uniqueidentifier(), buf.freeze())
            }
            #[cfg(feature = "decimal")]
            SqlValue::Decimal(d) => {
                // Decimal encoding is complex; use string representation for now
                RpcParam::nvarchar(param_name, &d.to_string())
            }
            #[cfg(feature = "chrono")]
            SqlValue::Date(_)
            | SqlValue::Time(_)
            | SqlValue::DateTime(_)
            | SqlValue::DateTimeOffset(_) => {
                // For date/time types, use string representation for simplicity
                // A full implementation would encode these properly
                let s = match &sql_value {
                    SqlValue::Date(d) => d.to_string(),
                    SqlValue::Time(t) => t.to_string(),
                    SqlValue::DateTime(dt) => dt.to_string(),
                    SqlValue::DateTimeOffset(dto) => dto.to_rfc3339(),
                    _ => unreachable!(),
                };
                RpcParam::nvarchar(param_name, &s)
            }
            #[cfg(feature = "json")]
            SqlValue::Json(ref j) => RpcParam::nvarchar(param_name, &j.to_string()),
            SqlValue::Tvp(_) => {
                return Err(Error::Protocol(
                    "Table-Valued Parameters are not supported for stored procedures".to_string(),
                ));
            }
            // Handle future SqlValue variants
            _ => {
                return Err(Error::Type(mssql_types::TypeError::UnsupportedConversion {
                    from: sql_value.type_name().to_string(),
                    to: "RPC parameter",
                }));
            }
        })
    }

    /// Create RpcTypeInfo from parameter metadata.
    fn type_info_from_metadata(meta: &ProcParamMetadata) -> RpcTypeInfo {
        use tds_protocol::rpc::TypeInfo;

        // Map SQL Server type names to RpcTypeInfo
        match meta.type_name.to_uppercase().as_str() {
            "INT" => RpcTypeInfo::int(),
            "BIGINT" => RpcTypeInfo::bigint(),
            "SMALLINT" => RpcTypeInfo::smallint(),
            "TINYINT" => RpcTypeInfo::tinyint(),
            "BIT" => RpcTypeInfo::bit(),
            "FLOAT" => RpcTypeInfo::float(),
            "REAL" => RpcTypeInfo::real(),
            "VARCHAR" | "NVARCHAR" => {
                let max_len = meta.max_length.unwrap_or(4000);
                RpcTypeInfo::nvarchar(max_len as u16)
            }
            "CHAR" | "NCHAR" => {
                let max_len = meta.max_length.unwrap_or(1);
                RpcTypeInfo::nvarchar(max_len as u16)
            }
            "VARBINARY" => {
                let max_len = meta.max_length.unwrap_or(8000);
                RpcTypeInfo::varbinary(max_len as u16)
            }
            "DATETIME" => {
                // Use DATETIME2 for DATETIME (SQL Server 2008+)
                TypeInfo {
                    type_id: 0x2A, // DATETIME2TYPE
                    max_length: None,
                    precision: None,
                    scale: Some(3), // DATETIME has precision of 300ms (3 decimal places)
                    collation: None,
                    tvp_type_name: None,
                }
            }
            "DATETIME2" => RpcTypeInfo::datetime2(7),
            "DATE" => RpcTypeInfo::date(),
            "TIME" => {
                let scale = meta.scale.unwrap_or(7);
                TypeInfo {
                    type_id: 0x29, // TIMETYPE
                    max_length: None,
                    precision: None,
                    scale: Some(scale),
                    collation: None,
                    tvp_type_name: None,
                }
            }
            "UNIQUEIDENTIFIER" => RpcTypeInfo::uniqueidentifier(),
            "DECIMAL" | "NUMERIC" => {
                let precision = meta.precision.unwrap_or(18);
                let scale = meta.scale.unwrap_or(0);
                RpcTypeInfo::decimal(precision, scale)
            }
            "XML" => RpcTypeInfo::nvarchar(0xFFFF),
            _ => {
                // Default to NVARCHAR for unknown types
                tracing::warn!(
                    type_name = %meta.type_name,
                    "unknown type, defaulting to NVARCHAR"
                );
                RpcTypeInfo::nvarchar(4000)
            }
        }
    }
}
