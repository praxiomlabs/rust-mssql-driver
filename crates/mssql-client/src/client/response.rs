//! Response reading and token parsing for SQL Server query/execute results.
//!
//! This module handles reading TDS response messages from the server and
//! parsing the token stream into structured results (rows, columns, row counts).

use tds_protocol::token::{ColMetaData, EnvChangeType, Token, TokenParser};

use crate::error::{Error, Result};
use crate::state::ConnectionState;

use super::{Client, ConnectionHandle};

impl<S: ConnectionState> Client<S> {
    /// Read complete query response including columns and rows.
    pub(super) async fn read_query_response(
        &mut self,
    ) -> Result<(Vec<crate::row::Column>, Vec<crate::row::Row>)> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut columns: Vec<crate::row::Column> = Vec::new();
        let mut rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;

        loop {
            // Use next_token_with_metadata to properly parse Row/NbcRow tokens
            let token = parser.next_token_with_metadata(protocol_metadata.as_ref())?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ColMetaData(meta) => {
                    // New result set starting - clear previous rows
                    // This enables multi-statement batches to return the last result set
                    rows.clear();

                    columns = meta
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(i, col)| {
                            let type_name = format!("{:?}", col.type_id);
                            let mut column = crate::row::Column::new(&col.name, i, type_name)
                                .with_nullable(col.flags & 0x01 != 0);

                            if let Some(max_len) = col.type_info.max_length {
                                column = column.with_max_length(max_len);
                            }
                            if let (Some(prec), Some(scale)) =
                                (col.type_info.precision, col.type_info.scale)
                            {
                                column = column.with_precision_scale(prec, scale);
                            }
                            // Store collation for VARCHAR/CHAR types to enable
                            // collation-aware string decoding
                            if let Some(collation) = col.type_info.collation {
                                column = column.with_collation(collation);
                            }
                            column
                        })
                        .collect();

                    tracing::debug!(columns = columns.len(), "received column metadata");
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_raw_row(&raw_row, meta, &columns)?;
                        rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_nbc_row(&nbc_row, meta, &columns)?;
                        rows.push(row);
                    }
                }
                Token::Error(err) => {
                    return Err(Error::Server {
                        number: err.number,
                        state: err.state,
                        class: err.class,
                        message: err.message.clone(),
                        server: if err.server.is_empty() {
                            None
                        } else {
                            Some(err.server.clone())
                        },
                        procedure: if err.procedure.is_empty() {
                            None
                        } else {
                            Some(err.procedure.clone())
                        },
                        line: err.line as u32,
                    });
                }
                Token::Done(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "query failed (server set error flag in DONE token)".to_string(),
                        ));
                    }
                    tracing::debug!(
                        row_count = done.row_count,
                        has_more = done.status.more,
                        "query complete"
                    );
                    // Only break if there are no more result sets
                    // This enables multi-statement batches to process all results
                    if !done.status.more {
                        break;
                    }
                }
                Token::DoneProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure failed (server set error flag in DONEPROC token)"
                                .to_string(),
                        ));
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "statement within procedure failed (error flag in DONEINPROC token)"
                                .to_string(),
                        ));
                    }
                }
                Token::Info(info) => {
                    tracing::debug!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                Token::EnvChange(env) => {
                    // Process transaction-related EnvChange tokens.
                    // This allows BEGIN TRANSACTION, COMMIT, ROLLBACK via raw SQL
                    // to properly update the transaction descriptor.
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                _ => {}
            }
        }

        tracing::debug!(
            columns = columns.len(),
            rows = rows.len(),
            "query response parsed"
        );
        Ok((columns, rows))
    }

    /// Read execute result (row count) from the response.
    pub(super) async fn read_execute_result(&mut self) -> Result<u64> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut rows_affected = 0u64;
        let mut current_metadata: Option<ColMetaData> = None;

        loop {
            // Use metadata-aware parsing to handle Row tokens from SELECT statements
            let token = parser.next_token_with_metadata(current_metadata.as_ref())?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ColMetaData(meta) => {
                    // Store metadata for subsequent Row token parsing
                    current_metadata = Some(meta);
                }
                Token::Row(_) | Token::NbcRow(_) => {
                    // Skip row data for execute() - we only care about row count
                    // The rows are parsed but we don't process them
                }
                Token::Done(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "execute failed (server set error flag in DONE token)".to_string(),
                        ));
                    }
                    if done.status.count {
                        // Accumulate row counts from all statements in a batch
                        rows_affected += done.row_count;
                    }
                    // Only break if there are no more result sets
                    // This enables multi-statement batches to report total affected rows
                    if !done.status.more {
                        break;
                    }
                }
                Token::DoneProc(done) => {
                    if done.status.count {
                        rows_affected += done.row_count;
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.count {
                        rows_affected += done.row_count;
                    }
                }
                Token::Error(err) => {
                    return Err(Error::Server {
                        number: err.number,
                        state: err.state,
                        class: err.class,
                        message: err.message.clone(),
                        server: if err.server.is_empty() {
                            None
                        } else {
                            Some(err.server.clone())
                        },
                        procedure: if err.procedure.is_empty() {
                            None
                        } else {
                            Some(err.procedure.clone())
                        },
                        line: err.line as u32,
                    });
                }
                Token::Info(info) => {
                    tracing::info!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                Token::EnvChange(env) => {
                    // Process transaction-related EnvChange tokens.
                    // This allows BEGIN TRANSACTION, COMMIT, ROLLBACK via raw SQL
                    // to properly update the transaction descriptor.
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                _ => {}
            }
        }

        Ok(rows_affected)
    }

    /// Read the response from BEGIN TRANSACTION and extract the transaction descriptor.
    ///
    /// Per MS-TDS spec, the server sends a BeginTransaction EnvChange token containing
    /// the transaction descriptor (8-byte value) that must be included in subsequent
    /// ALL_HEADERS sections for requests within this transaction.
    pub(super) async fn read_transaction_begin_result(&mut self) -> Result<u64> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut transaction_descriptor: u64 = 0;

        loop {
            let token = parser.next_token()?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::EnvChange(env) => {
                    if env.env_type == EnvChangeType::BeginTransaction {
                        // Extract the transaction descriptor from the binary value
                        // Per MS-TDS spec, it's an 8-byte (ULONGLONG) value
                        if let tds_protocol::token::EnvChangeValue::Binary(ref data) = env.new_value
                        {
                            if data.len() >= 8 {
                                transaction_descriptor = u64::from_le_bytes([
                                    data[0], data[1], data[2], data[3], data[4], data[5], data[6],
                                    data[7],
                                ]);
                                tracing::debug!(
                                    transaction_descriptor =
                                        format!("0x{:016X}", transaction_descriptor),
                                    "transaction begun"
                                );
                            }
                        }
                    }
                }
                Token::Done(done) => {
                    if done.status.error {
                        return Err(Error::Query("BEGIN TRANSACTION failed".to_string()));
                    }
                    break;
                }
                Token::Error(err) => {
                    return Err(Error::Server {
                        number: err.number,
                        state: err.state,
                        class: err.class,
                        message: err.message.clone(),
                        server: if err.server.is_empty() {
                            None
                        } else {
                            Some(err.server.clone())
                        },
                        procedure: if err.procedure.is_empty() {
                            None
                        } else {
                            Some(err.procedure.clone())
                        },
                        line: err.line as u32,
                    });
                }
                Token::Info(info) => {
                    tracing::info!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                _ => {}
            }
        }

        Ok(transaction_descriptor)
    }

    /// Read stored procedure execution result with output parameters, result set, and row count.
    ///
    /// This method collects:
    /// - Output parameter values from ReturnStatus/ReturnValue tokens
    /// - Result set rows (if any)
    /// - Row count from DoneProc tokens
    pub(super) async fn read_stored_proc_result(
        &mut self,
    ) -> Result<crate::stream::ExecuteResult<'_>> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut output_params = Vec::new();
        let mut result_set_columns: Vec<crate::row::Column> = Vec::new();
        let mut result_set_rows: Vec<crate::row::Row> = Vec::new();
        let mut current_metadata: Option<ColMetaData> = None;
        let mut rows_affected: u64 = 0;

        loop {
            let token = parser
                .next_token_with_metadata(current_metadata.as_ref())
                .map_err(|e| Error::Protocol(e.to_string()))?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ReturnStatus(status) => {
                    // ReturnStatus contains the RETURN statement value
                    // Per SQL Server spec, every stored procedure has a return value (default: 0)
                    // We always include it to match the specification
                    output_params.push(crate::stream::OutputParam {
                        name: "return_value".to_string(), // Fixed name for RETURN value
                        value: mssql_types::SqlValue::Int(status),
                    });
                }
                Token::ReturnValue(ret_val) => {
                    // Convert ReturnValue to SqlValue using the correct type information
                    use mssql_types::SqlValue;

                    let sql_value = if ret_val.col_type == 0x00 {
                        // NULL type (0x00) - value is always NULL
                        SqlValue::Null
                    } else {
                        // Use mssql_types::decode to properly decode the value
                        use mssql_types::decode::{TypeInfo as DecodeTypeInfo, decode_value};

                        let mut value_bytes = ret_val.value;

                        // Convert tds_protocol::TypeInfo to mssql_types::TypeInfo
                        let decode_type_info = DecodeTypeInfo {
                            type_id: ret_val.col_type, // Use col_type field for type_id
                            length: ret_val.type_info.max_length,
                            scale: ret_val.type_info.scale,
                            precision: ret_val.type_info.precision,
                            collation: ret_val.type_info.collation.map(|c| {
                                mssql_types::decode::Collation {
                                    lcid: c.lcid,
                                    flags: c.sort_id,
                                }
                            }),
                        };

                        decode_value(&mut value_bytes, &decode_type_info).map_err(|e| {
                            Error::Protocol(format!("failed to decode output parameter: {e}"))
                        })?
                    };

                    // Use the actual parameter name from the ReturnValue token
                    // For OUTPUT parameters, this will be the parameter name (e.g., "@sum", "@result")
                    // For default RETURN value (0), the name might be empty - keep it as-is
                    output_params.push(crate::stream::OutputParam {
                        name: ret_val.param_name,
                        value: sql_value,
                    });
                }
                Token::ColMetaData(meta) => {
                    // New result set starting
                    result_set_columns = meta
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(i, col)| {
                            let type_name = format!("{:?}", col.type_id);
                            let mut column = crate::row::Column::new(&col.name, i, type_name)
                                .with_nullable(col.flags & 0x01 != 0);

                            if let Some(max_len) = col.type_info.max_length {
                                column = column.with_max_length(max_len);
                            }
                            if let (Some(prec), Some(scale)) =
                                (col.type_info.precision, col.type_info.scale)
                            {
                                column = column.with_precision_scale(prec, scale);
                            }
                            if let Some(collation) = col.type_info.collation {
                                column = column.with_collation(collation);
                            }
                            column
                        })
                        .collect();

                    tracing::debug!(
                        columns = result_set_columns.len(),
                        "received column metadata for result set"
                    );
                    current_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = current_metadata {
                        let row = crate::column_parser::convert_raw_row(
                            &raw_row,
                            meta,
                            &result_set_columns,
                        )?;
                        result_set_rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = current_metadata {
                        let row = crate::column_parser::convert_nbc_row(
                            &nbc_row,
                            meta,
                            &result_set_columns,
                        )?;
                        result_set_rows.push(row);
                    }
                }
                Token::DoneProc(done) => {
                    if done.status.count {
                        rows_affected += done.row_count;
                    }
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure failed (server set error flag in DONEPROC token)"
                                .to_string(),
                        ));
                    }
                    // DoneProc marks end of stored procedure execution
                    // Continue to process remaining tokens (return values may come after)
                }
                Token::DoneInProc(done) => {
                    if done.status.count {
                        rows_affected += done.row_count;
                    }
                    if done.status.error {
                        return Err(Error::Query(
                            "statement within procedure failed (error flag in DONEINPROC token)"
                                .to_string(),
                        ));
                    }
                    // DoneInProc may indicate end of a statement within the procedure
                    // Continue processing for more tokens
                }
                Token::Done(done) => {
                    if done.status.count {
                        rows_affected += done.row_count;
                    }
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure execution failed (error flag in DONE token)"
                                .to_string(),
                        ));
                    }
                    // Done marks the final completion
                    if !done.status.more {
                        break;
                    }
                }
                Token::Error(err) => {
                    return Err(Error::Server {
                        number: err.number,
                        state: err.state,
                        class: err.class,
                        message: err.message.clone(),
                        server: if err.server.is_empty() {
                            None
                        } else {
                            Some(err.server.clone())
                        },
                        procedure: if err.procedure.is_empty() {
                            None
                        } else {
                            Some(err.procedure.clone())
                        },
                        line: err.line as u32,
                    });
                }
                Token::Info(info) => {
                    tracing::debug!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                Token::EnvChange(env) => {
                    // Process transaction-related EnvChange tokens
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                _ => {}
            }
        }

        // If no RETURN value was received (shouldn't happen per SQL Server spec),
        // add a default one to ensure the vector is never empty
        if output_params.is_empty() || !output_params.iter().any(|p| p.name == "return_value") {
            output_params.insert(
                0,
                crate::stream::OutputParam {
                    name: "return_value".to_string(),
                    value: mssql_types::SqlValue::Int(0),
                },
            );
        }

        // Create result set if we have columns
        let result_set = if !result_set_columns.is_empty() {
            Some(crate::stream::QueryStream::new(
                result_set_columns,
                result_set_rows,
            ))
        } else {
            None
        };

        Ok(crate::stream::ExecuteResult::new(
            output_params,
            rows_affected,
            result_set,
        ))
    }
}

// read_multi_result_response is on impl Client<Ready>, not the generic impl
use crate::state::Ready;

impl Client<Ready> {
    /// Read multiple result sets from a query response.
    pub(super) async fn read_multi_result_response(
        &mut self,
    ) -> Result<Vec<crate::stream::ResultSet>> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut result_sets: Vec<crate::stream::ResultSet> = Vec::new();
        let mut current_columns: Vec<crate::row::Column> = Vec::new();
        let mut current_rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;

        loop {
            let token = parser.next_token_with_metadata(protocol_metadata.as_ref())?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ColMetaData(meta) => {
                    // New result set starting - save the previous one if it has columns
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                    }

                    // Parse the new column metadata
                    current_columns = meta
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(i, col)| {
                            let type_name = format!("{:?}", col.type_id);
                            let mut column = crate::row::Column::new(&col.name, i, type_name)
                                .with_nullable(col.flags & 0x01 != 0);

                            if let Some(max_len) = col.type_info.max_length {
                                column = column.with_max_length(max_len);
                            }
                            if let (Some(prec), Some(scale)) =
                                (col.type_info.precision, col.type_info.scale)
                            {
                                column = column.with_precision_scale(prec, scale);
                            }
                            // Store collation for VARCHAR/CHAR types to enable
                            // collation-aware string decoding
                            if let Some(collation) = col.type_info.collation {
                                column = column.with_collation(collation);
                            }
                            column
                        })
                        .collect();

                    tracing::debug!(
                        columns = current_columns.len(),
                        result_set = result_sets.len(),
                        "received column metadata for result set"
                    );
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_raw_row(
                            &raw_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_nbc_row(
                            &nbc_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::Error(err) => {
                    return Err(Error::Server {
                        number: err.number,
                        state: err.state,
                        class: err.class,
                        message: err.message.clone(),
                        server: if err.server.is_empty() {
                            None
                        } else {
                            Some(err.server.clone())
                        },
                        procedure: if err.procedure.is_empty() {
                            None
                        } else {
                            Some(err.procedure.clone())
                        },
                        line: err.line as u32,
                    });
                }
                Token::Done(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "multi-result query failed (server set error flag in DONE token)"
                                .to_string(),
                        ));
                    }

                    // Save the current result set if we have columns
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    // Check if there are more result sets
                    if !done.status.more {
                        tracing::debug!(result_sets = result_sets.len(), "all result sets parsed");
                        break;
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "statement within procedure failed (error flag in DONEINPROC token)"
                                .to_string(),
                        ));
                    }

                    // Save the current result set if we have columns (within stored proc)
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    // DoneInProc may indicate more results within the batch
                    if !done.status.more {
                        // No more results from this statement, but batch may continue
                    }
                }
                Token::DoneProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure failed (server set error flag in DONEPROC token)"
                                .to_string(),
                        ));
                    }
                    // DoneProc marks end of stored procedure, not necessarily end of results
                }
                Token::Info(info) => {
                    tracing::debug!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                _ => {}
            }
        }

        // Don't forget any remaining result set that wasn't followed by Done
        if !current_columns.is_empty() {
            result_sets.push(crate::stream::ResultSet::new(current_columns, current_rows));
        }

        Ok(result_sets)
    }
}

impl Client<Ready> {
    /// Read stored procedure response with OUTPUT parameters and multiple result sets.
    ///
    /// This method handles stored procedures that return:
    /// - RETURN value (via ReturnStatus token)
    /// - OUTPUT parameters (via ReturnValue tokens)
    /// - Multiple result sets (via multiple ColMetaData tokens)
    /// - Row count information (via DoneProc token)
    pub(super) async fn read_stored_proc_multiple_result(
        &mut self,
    ) -> Result<crate::stream::MultiExecuteResult<'_>> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut output_params = Vec::new();
        let mut result_sets: Vec<crate::stream::ResultSet> = Vec::new();
        let mut current_columns: Vec<crate::row::Column> = Vec::new();
        let mut current_rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;
        let mut rows_affected: u64 = 0;

        loop {
            let token = parser
                .next_token_with_metadata(protocol_metadata.as_ref())
                .map_err(|e| Error::Protocol(e.to_string()))?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ReturnStatus(status) => {
                    // ReturnStatus contains the RETURN statement value
                    // Per SQL Server spec, every stored procedure has a return value (default: 0)
                    // We always include it to match the specification
                    output_params.push(crate::stream::OutputParam {
                        name: "return_value".to_string(), // Fixed name for RETURN value
                        value: mssql_types::SqlValue::Int(status),
                    });
                }
                Token::ReturnValue(ret_val) => {
                    // Convert ReturnValue to SqlValue using the correct type information
                    use mssql_types::SqlValue;

                    let sql_value = if ret_val.col_type == 0x00 {
                        // NULL type (0x00) - value is always NULL
                        SqlValue::Null
                    } else {
                        // Use mssql_types::decode to properly decode the value
                        use mssql_types::decode::{TypeInfo as DecodeTypeInfo, decode_value};

                        let mut value_bytes = ret_val.value;

                        // Convert tds_protocol::TypeInfo to mssql_types::TypeInfo
                        let decode_type_info = DecodeTypeInfo {
                            type_id: ret_val.col_type, // Use col_type field for type_id
                            length: ret_val.type_info.max_length,
                            scale: ret_val.type_info.scale,
                            precision: ret_val.type_info.precision,
                            collation: ret_val.type_info.collation.map(|c| {
                                mssql_types::decode::Collation {
                                    lcid: c.lcid,
                                    flags: c.sort_id,
                                }
                            }),
                        };

                        decode_value(&mut value_bytes, &decode_type_info)?
                    };

                    output_params.push(crate::stream::OutputParam {
                        name: ret_val.param_name,
                        value: sql_value,
                    });
                }
                Token::ColMetaData(meta) => {
                    // New result set starting - save the previous one if it has columns
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                    }

                    // Parse the new column metadata
                    current_columns = meta
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(i, col)| {
                            let type_name = format!("{:?}", col.type_id);
                            let mut column = crate::row::Column::new(&col.name, i, type_name)
                                .with_nullable(col.flags & 0x01 != 0);

                            if let Some(max_len) = col.type_info.max_length {
                                column = column.with_max_length(max_len);
                            }
                            if let (Some(prec), Some(scale)) =
                                (col.type_info.precision, col.type_info.scale)
                            {
                                column = column.with_precision_scale(prec, scale);
                            }
                            // Store collation for VARCHAR/CHAR types to enable
                            // collation-aware string decoding
                            if let Some(collation) = col.type_info.collation {
                                column = column.with_collation(collation);
                            }
                            column
                        })
                        .collect();

                    tracing::debug!(
                        columns = current_columns.len(),
                        result_set = result_sets.len(),
                        "received column metadata for result set in stored procedure"
                    );
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_raw_row(
                            &raw_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    // NbcRow (null bitmap row) - similar to Row but with null bitmap
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_nbc_row(
                            &nbc_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::DoneProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure failed (server set error flag in DONEPROC token)"
                                .to_string(),
                        ));
                    }

                    // Save the current result set if we have columns
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    // Update rows_affected from DoneProc
                    rows_affected = done.row_count;

                    // Check if there are more result sets
                    if !done.status.more {
                        tracing::debug!(
                            result_sets = result_sets.len(),
                            "stored procedure completed, all result sets parsed"
                        );
                        break;
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "statement within procedure failed (error flag in DONEINPROC token)"
                                .to_string(),
                        ));
                    }

                    // Save the current result set if we have columns (within stored proc)
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    // Update rows_affected from DoneInProc
                    if done.row_count > rows_affected {
                        rows_affected = done.row_count;
                    }
                }
                Token::Info(info) => {
                    tracing::debug!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                Token::EnvChange(env) => {
                    // Process transaction-related EnvChange tokens
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                _ => {
                    // Ignore other tokens for stored procedure responses
                }
            }
        }

        // Don't forget any remaining result set that wasn't followed by Done tokens
        if !current_columns.is_empty() {
            result_sets.push(crate::stream::ResultSet::new(current_columns, current_rows));
        }

        // If no RETURN value was received (shouldn't happen per SQL Server spec),
        // add a default one to ensure the vector is never empty
        if output_params.is_empty() || !output_params.iter().any(|p| p.name == "return_value") {
            output_params.insert(
                0,
                crate::stream::OutputParam {
                    name: "return_value".to_string(),
                    value: mssql_types::SqlValue::Int(0),
                },
            );
        }

        Ok(crate::stream::MultiExecuteResult::new(
            output_params,
            rows_affected,
            crate::stream::MultiResultStream::new(result_sets),
        ))
    }
}

use crate::state::InTransaction;

impl Client<InTransaction> {
    /// Read stored procedure response with OUTPUT parameters and multiple result sets.
    ///
    /// This is the transaction-aware version of the method in `Client<Ready>`.
    /// See that method for full documentation.
    pub(super) async fn read_stored_proc_multiple_result(
        &mut self,
    ) -> Result<crate::stream::MultiExecuteResult<'_>> {
        // Call the Ready implementation by delegating to the inner Client
        // We need to be careful about lifetimes here
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut output_params = Vec::new();
        let mut result_sets: Vec<crate::stream::ResultSet> = Vec::new();
        let mut current_columns: Vec<crate::row::Column> = Vec::new();
        let mut current_rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;
        let mut rows_affected: u64 = 0;

        loop {
            let token = parser
                .next_token_with_metadata(protocol_metadata.as_ref())
                .map_err(|e| Error::Protocol(e.to_string()))?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ReturnStatus(status) => {
                    output_params.push(crate::stream::OutputParam {
                        name: "return_value".to_string(),
                        value: mssql_types::SqlValue::Int(status),
                    });
                }
                Token::ReturnValue(ret_val) => {
                    use mssql_types::SqlValue;

                    let sql_value = if ret_val.col_type == 0x00 {
                        SqlValue::Null
                    } else {
                        use mssql_types::decode::{TypeInfo as DecodeTypeInfo, decode_value};

                        let mut value_bytes = ret_val.value;

                        let decode_type_info = DecodeTypeInfo {
                            type_id: ret_val.col_type,
                            length: ret_val.type_info.max_length,
                            scale: ret_val.type_info.scale,
                            precision: ret_val.type_info.precision,
                            collation: ret_val.type_info.collation.map(|c| {
                                mssql_types::decode::Collation {
                                    lcid: c.lcid,
                                    flags: c.sort_id,
                                }
                            }),
                        };

                        decode_value(&mut value_bytes, &decode_type_info)?
                    };

                    output_params.push(crate::stream::OutputParam {
                        name: ret_val.param_name,
                        value: sql_value,
                    });
                }
                Token::ColMetaData(meta) => {
                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                    }

                    current_columns = meta
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(i, col)| {
                            let type_name = format!("{:?}", col.type_id);
                            let mut column = crate::row::Column::new(&col.name, i, type_name)
                                .with_nullable(col.flags & 0x01 != 0);

                            if let Some(max_len) = col.type_info.max_length {
                                column = column.with_max_length(max_len);
                            }
                            if let (Some(prec), Some(scale)) =
                                (col.type_info.precision, col.type_info.scale)
                            {
                                column = column.with_precision_scale(prec, scale);
                            }
                            if let Some(collation) = col.type_info.collation {
                                column = column.with_collation(collation);
                            }
                            column
                        })
                        .collect();

                    tracing::debug!(
                        columns = current_columns.len(),
                        result_set = result_sets.len(),
                        "received column metadata for result set in stored procedure"
                    );
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_raw_row(
                            &raw_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = crate::column_parser::convert_nbc_row(
                            &nbc_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::DoneProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure failed (server set error flag in DONEPROC token)"
                                .to_string(),
                        ));
                    }

                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    rows_affected = done.row_count;

                    if !done.status.more {
                        tracing::debug!(
                            result_sets = result_sets.len(),
                            "stored procedure completed, all result sets parsed"
                        );
                        break;
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.error {
                        return Err(Error::Query(
                            "statement within procedure failed (error flag in DONEINPROC token)"
                                .to_string(),
                        ));
                    }

                    if !current_columns.is_empty() {
                        result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    if done.row_count > rows_affected {
                        rows_affected = done.row_count;
                    }
                }
                Token::Info(info) => {
                    tracing::debug!(
                        number = info.number,
                        message = %info.message,
                        "server info message"
                    );
                }
                Token::EnvChange(env) => {
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                _ => {}
            }
        }

        if !current_columns.is_empty() {
            result_sets.push(crate::stream::ResultSet::new(current_columns, current_rows));
        }

        if output_params.is_empty() || !output_params.iter().any(|p| p.name == "return_value") {
            output_params.insert(
                0,
                crate::stream::OutputParam {
                    name: "return_value".to_string(),
                    value: mssql_types::SqlValue::Int(0),
                },
            );
        }

        Ok(crate::stream::MultiExecuteResult::new(
            output_params,
            rows_affected,
            crate::stream::MultiResultStream::new(result_sets),
        ))
    }
}
