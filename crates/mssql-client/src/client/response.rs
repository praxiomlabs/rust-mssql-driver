//! Response reading and token parsing for SQL Server query/execute results.
//!
//! This module handles reading TDS response messages from the server and
//! parsing the token stream into structured results (rows, columns, row counts).

use tds_protocol::token::{ColMetaData, EnvChangeType, Token, TokenParser};

use crate::error::{Error, Result};
use crate::state::ConnectionState;

use super::{Client, ConnectionHandle};

impl<S: ConnectionState> Client<S> {
    /// Create a TokenParser with encryption awareness when configured.
    pub(super) fn create_parser(&self, payload: bytes::Bytes) -> TokenParser {
        let parser = TokenParser::new(payload);
        #[cfg(feature = "always-encrypted")]
        let parser = if self.encryption_context.is_some() {
            parser.with_encryption(true)
        } else {
            parser
        };
        parser
    }

    /// Resolve a ColumnDecryptor from ColMetaData if encryption is active.
    ///
    /// Returns `None` if encryption is not configured or the result set has
    /// no encrypted columns.
    #[cfg(feature = "always-encrypted")]
    async fn resolve_decryptor(
        &self,
        meta: &ColMetaData,
    ) -> Result<Option<crate::column_decryptor::ColumnDecryptor>> {
        if let Some(ref ctx) = self.encryption_context {
            if meta.cek_table.is_some() {
                return Ok(Some(
                    crate::column_decryptor::ColumnDecryptor::from_metadata(meta, ctx).await?,
                ));
            }
        }
        Ok(None)
    }

    /// Build column metadata from ColMetaData, using base types for encrypted columns.
    fn build_columns(meta: &ColMetaData) -> Vec<crate::row::Column> {
        meta.columns
            .iter()
            .enumerate()
            .map(|(i, col)| {
                // For encrypted columns, use the base type info from CryptoMetadata
                // so users see the real column type (e.g., NVARCHAR) instead of BigVarBinary.
                let (effective_type_id, effective_type_info) =
                    if let Some(ref crypto) = col.crypto_metadata {
                        (crypto.base_type_id(), &crypto.base_type_info)
                    } else {
                        (col.type_id, &col.type_info)
                    };

                let type_name = format!("{effective_type_id:?}");
                let mut column = crate::row::Column::new(&col.name, i, type_name)
                    .with_nullable(col.flags & 0x01 != 0);

                if let Some(max_len) = effective_type_info.max_length {
                    column = column.with_max_length(max_len);
                }
                if let (Some(prec), Some(scale)) =
                    (effective_type_info.precision, effective_type_info.scale)
                {
                    column = column.with_precision_scale(prec, scale);
                }
                if let Some(collation) = effective_type_info.collation {
                    column = column.with_collation(collation);
                }
                column
            })
            .collect()
    }
}

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

        // Full response received from wire — connection is clean for next request
        self.in_flight = false;

        let mut parser = self.create_parser(message.payload);
        let mut columns: Vec<crate::row::Column> = Vec::new();
        let mut rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;
        #[cfg(feature = "always-encrypted")]
        let mut current_decryptor: Option<crate::column_decryptor::ColumnDecryptor> = None;

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

                    columns = Self::build_columns(&meta);

                    #[cfg(feature = "always-encrypted")]
                    {
                        current_decryptor = self.resolve_decryptor(&meta).await?;
                    }

                    tracing::debug!(columns = columns.len(), "received column metadata");
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        #[cfg(feature = "always-encrypted")]
                        let row = if let Some(ref dec) = current_decryptor {
                            crate::column_parser::convert_raw_row_decrypted(
                                &raw_row, meta, &columns, dec,
                            )?
                        } else {
                            crate::column_parser::convert_raw_row(&raw_row, meta, &columns)?
                        };
                        #[cfg(not(feature = "always-encrypted"))]
                        let row = crate::column_parser::convert_raw_row(&raw_row, meta, &columns)?;
                        rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        #[cfg(feature = "always-encrypted")]
                        let row = if let Some(ref dec) = current_decryptor {
                            crate::column_parser::convert_nbc_row_decrypted(
                                &nbc_row, meta, &columns, dec,
                            )?
                        } else {
                            crate::column_parser::convert_nbc_row(&nbc_row, meta, &columns)?
                        };
                        #[cfg(not(feature = "always-encrypted"))]
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
            // Note: execute() doesn't read row values, so no decryption needed.
            // But we still need the encryption-aware parser for ColMetaData/Row token parsing.
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        // Full response received from wire — connection is clean for next request
        self.in_flight = false;

        let mut parser = self.create_parser(message.payload);
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

        // Full response received from wire — connection is clean for next request
        self.in_flight = false;

        let mut parser = self.create_parser(message.payload);
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

    /// Read the complete response from a stored procedure RPC call.
    ///
    /// Parses the TDS token stream produced by an RPC request for a named
    /// stored procedure, collecting result sets, output parameters (from
    /// RETURNVALUE tokens), and the procedure return value (from RETURNSTATUS).
    ///
    /// Token handling is order-tolerant and accumulative:
    /// - `ColMetaData` / `Row` / `NbcRow` / `DoneInProc`: collect result sets
    /// - `ReturnValue`: decode value via `parse_column_value()`, push as `OutputParam`
    /// - `ReturnStatus`: store as `return_value`
    /// - `DoneProc`: final token, break when `!more`
    /// - `Error` / `Info` / `EnvChange`: standard handling
    pub(crate) async fn read_procedure_result(&mut self) -> Result<crate::stream::ProcedureResult> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;

        // Full response received from wire — connection is clean for next request
        self.in_flight = false;

        let mut parser = self.create_parser(message.payload);
        let mut result = crate::stream::ProcedureResult::new();

        // State for accumulating the current result set
        let mut current_columns: Vec<crate::row::Column> = Vec::new();
        let mut current_rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;
        #[cfg(feature = "always-encrypted")]
        let mut current_decryptor: Option<crate::column_decryptor::ColumnDecryptor> = None;

        loop {
            let token = parser.next_token_with_metadata(protocol_metadata.as_ref())?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ColMetaData(meta) => {
                    // New result set starting — save the previous one if it has columns
                    if !current_columns.is_empty() {
                        result.result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                    }

                    current_columns = Self::build_columns(&meta);

                    #[cfg(feature = "always-encrypted")]
                    {
                        current_decryptor = self.resolve_decryptor(&meta).await?;
                    }

                    tracing::debug!(
                        columns = current_columns.len(),
                        result_set = result.result_sets.len(),
                        "procedure: received column metadata"
                    );
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        #[cfg(feature = "always-encrypted")]
                        let row = if let Some(ref dec) = current_decryptor {
                            crate::column_parser::convert_raw_row_decrypted(
                                &raw_row,
                                meta,
                                &current_columns,
                                dec,
                            )?
                        } else {
                            crate::column_parser::convert_raw_row(&raw_row, meta, &current_columns)?
                        };
                        #[cfg(not(feature = "always-encrypted"))]
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
                        #[cfg(feature = "always-encrypted")]
                        let row = if let Some(ref dec) = current_decryptor {
                            crate::column_parser::convert_nbc_row_decrypted(
                                &nbc_row,
                                meta,
                                &current_columns,
                                dec,
                            )?
                        } else {
                            crate::column_parser::convert_nbc_row(&nbc_row, meta, &current_columns)?
                        };
                        #[cfg(not(feature = "always-encrypted"))]
                        let row = crate::column_parser::convert_nbc_row(
                            &nbc_row,
                            meta,
                            &current_columns,
                        )?;
                        current_rows.push(row);
                    }
                }
                Token::DoneInProc(done) => {
                    // Save current result set if we have columns
                    if !current_columns.is_empty() {
                        result.result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                        protocol_metadata = None;
                    }

                    if done.status.count {
                        result.rows_affected += done.row_count;
                    }
                    if done.status.error {
                        return Err(Error::Query(
                            "statement within procedure failed (error flag in DONEINPROC token)"
                                .to_string(),
                        ));
                    }
                }
                Token::ReturnValue(ret_val) => {
                    // Decode the return value bytes into a SqlValue using
                    // the same parser that handles column data in result rows.
                    use tds_protocol::token::ColumnData;
                    use tds_protocol::types::TypeId;

                    let type_id = TypeId::from_u8(ret_val.col_type).unwrap_or(TypeId::Null);
                    let col_data = ColumnData {
                        name: String::new(),
                        type_id,
                        col_type: ret_val.col_type,
                        flags: ret_val.flags,
                        user_type: ret_val.user_type,
                        type_info: ret_val.type_info.clone(),
                        crypto_metadata: None,
                    };
                    let mut buf = ret_val.value.as_ref();
                    let sql_value = crate::column_parser::parse_column_value(&mut buf, &col_data)?;

                    result.output_params.push(crate::stream::OutputParam {
                        name: ret_val.param_name,
                        value: sql_value,
                    });

                    tracing::debug!(
                        param_ordinal = ret_val.param_ordinal,
                        "procedure: received output parameter"
                    );
                }
                Token::ReturnStatus(status) => {
                    result.return_value = status;
                    tracing::debug!(return_value = status, "procedure: received return status");
                }
                Token::DoneProc(done) => {
                    // Save any remaining result set
                    if !current_columns.is_empty() {
                        result.result_sets.push(crate::stream::ResultSet::new(
                            std::mem::take(&mut current_columns),
                            std::mem::take(&mut current_rows),
                        ));
                    }

                    if done.status.count {
                        result.rows_affected += done.row_count;
                    }
                    if done.status.error {
                        return Err(Error::Query(
                            "stored procedure failed (server set error flag in DONEPROC token)"
                                .to_string(),
                        ));
                    }
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
                        "procedure: server info message"
                    );
                }
                Token::EnvChange(env) => {
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                other => {
                    tracing::trace!(token = ?std::mem::discriminant(&other), "procedure: unhandled token");
                }
            }
        }

        tracing::debug!(
            return_value = result.return_value,
            rows_affected = result.rows_affected,
            output_params = result.output_params.len(),
            result_sets = result.result_sets.len(),
            "procedure response parsed"
        );

        Ok(result)
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

        // Full response received from wire — connection is clean for next request
        self.in_flight = false;

        let mut parser = self.create_parser(message.payload);
        let mut result_sets: Vec<crate::stream::ResultSet> = Vec::new();
        let mut current_columns: Vec<crate::row::Column> = Vec::new();
        let mut current_rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;
        #[cfg(feature = "always-encrypted")]
        let mut current_decryptor: Option<crate::column_decryptor::ColumnDecryptor> = None;

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
                    current_columns = Self::build_columns(&meta);

                    #[cfg(feature = "always-encrypted")]
                    {
                        current_decryptor = self.resolve_decryptor(&meta).await?;
                    }

                    tracing::debug!(
                        columns = current_columns.len(),
                        result_set = result_sets.len(),
                        "received column metadata for result set"
                    );
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        #[cfg(feature = "always-encrypted")]
                        let row = if let Some(ref dec) = current_decryptor {
                            crate::column_parser::convert_raw_row_decrypted(
                                &raw_row,
                                meta,
                                &current_columns,
                                dec,
                            )?
                        } else {
                            crate::column_parser::convert_raw_row(&raw_row, meta, &current_columns)?
                        };
                        #[cfg(not(feature = "always-encrypted"))]
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
                        #[cfg(feature = "always-encrypted")]
                        let row = if let Some(ref dec) = current_decryptor {
                            crate::column_parser::convert_nbc_row_decrypted(
                                &nbc_row,
                                meta,
                                &current_columns,
                                dec,
                            )?
                        } else {
                            crate::column_parser::convert_nbc_row(&nbc_row, meta, &current_columns)?
                        };
                        #[cfg(not(feature = "always-encrypted"))]
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
