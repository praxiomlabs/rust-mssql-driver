//! SQL Server client implementation.

// Allow unwrap/expect for chrono date construction with known-valid constant dates
// and for regex patterns that are compile-time constants
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::needless_range_loop)]

mod connect;
mod params;
mod response;

use std::marker::PhantomData;

use mssql_codec::connection::Connection;
#[cfg(feature = "tls")]
use mssql_tls::TlsStream;
use tds_protocol::packet::PacketType;
use tds_protocol::rpc::RpcRequest;
use tds_protocol::token::{EnvChange, EnvChangeType};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::Config;
use crate::error::{Error, Result};
#[cfg(feature = "otel")]
use crate::instrumentation::InstrumentationContext;
use crate::state::{ConnectionState, InTransaction, Ready};
use crate::statement_cache::StatementCache;
use crate::stream::{MultiResultStream, QueryStream};
use crate::transaction::SavePoint;

/// SQL Server client with type-state connection management.
///
/// The generic parameter `S` represents the current connection state,
/// ensuring at compile time that certain operations are only available
/// in appropriate states.
pub struct Client<S: ConnectionState> {
    config: Config,
    _state: PhantomData<S>,
    /// The underlying connection (present only when connected)
    connection: Option<ConnectionHandle>,
    /// Server version from LoginAck (raw u32 TDS version)
    server_version: Option<u32>,
    /// Current database from EnvChange
    current_database: Option<String>,
    /// Server's default collation from SqlCollation EnvChange during login.
    /// Used when `SendStringParametersAsUnicode=false` to encode VARCHAR
    /// parameters with the correct character encoding and collation bytes.
    server_collation: Option<tds_protocol::token::Collation>,
    /// Prepared statement cache for query optimization
    statement_cache: StatementCache,
    /// Transaction descriptor from BeginTransaction EnvChange.
    /// Per MS-TDS spec, this value must be included in ALL_HEADERS for subsequent
    /// requests within an explicit transaction. 0 indicates auto-commit mode.
    transaction_descriptor: u64,
    /// Whether a request has been sent and the response has not yet been fully read.
    /// Used by the connection pool to detect dirty connections after cancel/timeout.
    in_flight: bool,
    /// Whether this connection needs a reset on next use.
    /// Set by connection pool on checkin, cleared after first query/execute.
    /// When true, the RESETCONNECTION flag is set on the first TDS packet.
    needs_reset: bool,
    /// OpenTelemetry instrumentation context (when otel feature is enabled)
    #[cfg(feature = "otel")]
    instrumentation: InstrumentationContext,
    /// Always Encrypted context for column decryption (when always-encrypted feature is enabled)
    #[cfg(feature = "always-encrypted")]
    pub(crate) encryption_context: Option<std::sync::Arc<crate::encryption::EncryptionContext>>,
}

/// Internal connection handle wrapping the actual connection.
///
/// This is an enum to support different connection types:
/// - TLS (TDS 8.0 strict mode) - requires `tls` feature
/// - TLS with PreLogin wrapping (TDS 7.x style) - requires `tls` feature
/// - Plain TCP (for internal networks or when `tls` feature is disabled)
#[allow(dead_code)] // Connection will be used once query execution is implemented
enum ConnectionHandle {
    /// TLS connection (TDS 8.0 strict mode - TLS before any TDS traffic)
    #[cfg(feature = "tls")]
    Tls(Connection<TlsStream<TcpStream>>),
    /// TLS connection with PreLogin wrapping (TDS 7.x style)
    #[cfg(feature = "tls")]
    TlsPrelogin(Connection<TlsStream<mssql_tls::TlsPreloginWrapper<TcpStream>>>),
    /// Plain TCP connection (for internal networks or when `tls` feature is disabled)
    Plain(Connection<TcpStream>),
}

// Private helper methods available to all connection states
impl<S: ConnectionState> Client<S> {
    /// Process transaction-related EnvChange tokens.
    ///
    /// This handles BeginTransaction, CommitTransaction, and RollbackTransaction
    /// EnvChange tokens, updating the transaction descriptor accordingly.
    ///
    /// This enables executing BEGIN TRANSACTION, COMMIT, and ROLLBACK via raw SQL
    /// while still having the transaction descriptor tracked correctly.
    fn process_transaction_env_change(env: &EnvChange, transaction_descriptor: &mut u64) {
        use tds_protocol::token::EnvChangeValue;

        match env.env_type {
            EnvChangeType::BeginTransaction => {
                if let EnvChangeValue::Binary(ref data) = env.new_value {
                    if data.len() >= 8 {
                        let descriptor = u64::from_le_bytes([
                            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                        ]);
                        tracing::debug!(descriptor = descriptor, "transaction started via raw SQL");
                        *transaction_descriptor = descriptor;
                    }
                }
            }
            EnvChangeType::CommitTransaction | EnvChangeType::RollbackTransaction => {
                tracing::debug!(
                    env_type = ?env.env_type,
                    "transaction ended via raw SQL"
                );
                *transaction_descriptor = 0;
            }
            _ => {}
        }
    }

    /// Send a SQL batch to the server.
    ///
    /// Uses the client's current transaction descriptor in ALL_HEADERS.
    /// Per MS-TDS spec, when in an explicit transaction, the descriptor
    /// returned by BeginTransaction must be included.
    ///
    /// If `needs_reset` is set (from pool return), the RESETCONNECTION flag
    /// is included in the first packet to reset connection state.
    async fn send_sql_batch(&mut self, sql: &str) -> Result<()> {
        let payload =
            tds_protocol::encode_sql_batch_with_transaction(sql, self.transaction_descriptor);
        let max_packet = self.config.packet_size as usize;

        // Check if we need to reset the connection on this request
        let reset = self.needs_reset;
        if reset {
            self.needs_reset = false; // Clear flag before sending
            tracing::debug!("sending SQL batch with RESETCONNECTION flag");
        }

        self.in_flight = true;
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => {
                conn.send_message_with_reset(PacketType::SqlBatch, payload, max_packet, reset)
                    .await?;
            }
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => {
                conn.send_message_with_reset(PacketType::SqlBatch, payload, max_packet, reset)
                    .await?;
            }
            ConnectionHandle::Plain(conn) => {
                conn.send_message_with_reset(PacketType::SqlBatch, payload, max_packet, reset)
                    .await?;
            }
        }

        Ok(())
    }

    /// Send an RPC request to the server.
    ///
    /// Uses the client's current transaction descriptor in ALL_HEADERS.
    ///
    /// If `needs_reset` is set (from pool return), the RESETCONNECTION flag
    /// is included in the first packet to reset connection state.
    pub(crate) async fn send_rpc(&mut self, rpc: &RpcRequest) -> Result<()> {
        let payload = rpc.encode_with_transaction(self.transaction_descriptor);
        let max_packet = self.config.packet_size as usize;

        // Check if we need to reset the connection on this request
        let reset = self.needs_reset;
        if reset {
            self.needs_reset = false; // Clear flag before sending
            tracing::debug!("sending RPC with RESETCONNECTION flag");
        }

        self.in_flight = true;
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => {
                conn.send_message_with_reset(PacketType::Rpc, payload, max_packet, reset)
                    .await?;
            }
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => {
                conn.send_message_with_reset(PacketType::Rpc, payload, max_packet, reset)
                    .await?;
            }
            ConnectionHandle::Plain(conn) => {
                conn.send_message_with_reset(PacketType::Rpc, payload, max_packet, reset)
                    .await?;
            }
        }

        Ok(())
    }

    /// Start building a stored procedure call with full control over parameters.
    ///
    /// Returns a [`crate::procedure::ProcedureBuilder`] that allows adding named input and output
    /// parameters before executing the call.
    ///
    /// The procedure name is validated to prevent SQL injection. It may be
    /// schema-qualified (e.g., `"dbo.MyProc"`).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = client.procedure("dbo.CalculateSum")?
    ///     .input("@a", &10i32)
    ///     .input("@b", &20i32)
    ///     .output_int("@result")
    ///     .execute().await?;
    ///
    /// let sum = result.get_output("@result").unwrap();
    /// ```
    pub fn procedure(
        &mut self,
        proc_name: &str,
    ) -> Result<crate::procedure::ProcedureBuilder<'_, S>> {
        crate::validation::validate_qualified_identifier(proc_name)?;
        Ok(crate::procedure::ProcedureBuilder::new(self, proc_name))
    }

    /// Execute a stored procedure with positional input parameters.
    ///
    /// This is a convenience method for the common case of calling a procedure
    /// with input-only parameters. For output parameters or named parameters,
    /// use [`procedure()`](Client::procedure) instead.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = client.call_procedure("dbo.GetUser", &[&1i32]).await?;
    /// assert_eq!(result.return_value, 0);
    ///
    /// if let Some(rs) = result.first_result_set() {
    ///     println!("columns: {:?}", rs.columns());
    /// }
    /// ```
    pub async fn call_procedure(
        &mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::stream::ProcedureResult> {
        crate::validation::validate_qualified_identifier(proc_name)?;

        tracing::debug!(
            proc_name = proc_name,
            params_count = params.len(),
            "executing stored procedure"
        );

        let rpc_params =
            Self::convert_params_positional(params, self.send_unicode(), self.server_collation())?;
        let mut rpc = RpcRequest::named(proc_name);
        for param in rpc_params {
            rpc = rpc.param(param);
        }

        self.send_rpc(&rpc).await?;
        self.read_procedure_result().await
    }

    /// Start a bulk insert operation for the specified table.
    ///
    /// Sends the `INSERT BULK` statement to the server and returns a
    /// [`crate::bulk::BulkWriter`] for streaming rows. The writer holds
    /// a mutable borrow on the client, preventing other operations while
    /// the bulk insert is in progress.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::{BulkInsertBuilder, BulkColumn};
    ///
    /// let builder = BulkInsertBuilder::new("dbo.Users")
    ///     .with_typed_columns(vec![
    ///         BulkColumn::new("id", "INT", 0)?,
    ///         BulkColumn::new("name", "NVARCHAR(100)", 1)?,
    ///     ]);
    ///
    /// let mut writer = client.bulk_insert(&builder).await?;
    /// writer.send_row(&[&1i32, &"Alice"])?;
    /// writer.send_row(&[&2i32, &"Bob"])?;
    /// let result = writer.finish().await?;
    /// println!("Inserted {} rows", result.rows_affected);
    /// ```
    pub async fn bulk_insert(
        &mut self,
        builder: &crate::bulk::BulkInsertBuilder,
    ) -> Result<crate::bulk::BulkWriter<'_, S>> {
        use tds_protocol::token::{ColMetaData, Token};

        tracing::debug!(
            table = builder.table_name(),
            columns = builder.columns().len(),
            "starting bulk insert"
        );

        // Step 1: Query the server for column metadata.
        // This gives us the exact type encoding the server expects for BulkLoad,
        // following the pattern established by Tiberius.
        let meta_query = format!("SELECT TOP 0 * FROM {}", builder.table_name());
        self.send_sql_batch(&meta_query).await?;

        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;
        let message = match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => conn.read_message().await?,
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => conn.read_message().await?,
            ConnectionHandle::Plain(conn) => conn.read_message().await?,
        }
        .ok_or(Error::ConnectionClosed)?;
        self.in_flight = false;

        // Capture both the raw COLMETADATA bytes and parsed column info
        let raw_payload = message.payload.clone();
        let mut parser = self.create_parser(message.payload);
        let mut server_metadata: Option<ColMetaData> = None;
        let mut meta_start: usize = 0;
        let mut meta_end: usize = 0;

        loop {
            let pos_before = raw_payload.len() - parser.remaining();
            let token = parser.next_token_with_metadata(server_metadata.as_ref())?;
            let pos_after = raw_payload.len() - parser.remaining();
            let Some(token) = token else { break };

            match token {
                Token::ColMetaData(meta) => {
                    meta_start = pos_before;
                    meta_end = pos_after;
                    server_metadata = Some(meta);
                }
                Token::Done(_) => break,
                _ => {}
            }
        }

        // Reject deprecated TEXT/NTEXT/IMAGE columns reported by the server.
        // These types require a legacy TEXTPTR wire format that this driver
        // does not support — users should migrate the column to VARCHAR(MAX) /
        // NVARCHAR(MAX) / VARBINARY(MAX).
        if let Some(ref meta) = server_metadata {
            use tds_protocol::types::TypeId;
            for col in meta.columns.iter() {
                let (rejected, replacement) = match col.type_id {
                    TypeId::Text => (Some("TEXT"), "VARCHAR(MAX)"),
                    TypeId::NText => (Some("NTEXT"), "NVARCHAR(MAX)"),
                    TypeId::Image => (Some("IMAGE"), "VARBINARY(MAX)"),
                    _ => (None, ""),
                };
                if let Some(sql_type) = rejected {
                    return Err(Error::from(mssql_types::TypeError::UnsupportedType {
                        sql_type: sql_type.to_string(),
                        reason: format!(
                            "column `{}` in table `{}` is {} — TEXT/NTEXT/IMAGE \
                             are not supported. Alter the column to {} instead \
                             (Microsoft deprecated TEXT/NTEXT/IMAGE in SQL \
                             Server 2005).",
                            col.name,
                            builder.table_name(),
                            sql_type,
                            replacement,
                        ),
                    }));
                }
            }
        }

        // Step 2: Send INSERT BULK statement to put server in bulk load mode
        let stmt = builder.build_insert_bulk_statement()?;
        self.send_sql_batch(&stmt).await?;
        self.read_execute_result().await?;

        // Step 3: Create bulk writer with server's metadata
        let raw_meta = if meta_end > meta_start {
            Some(raw_payload.slice(meta_start..meta_end))
        } else {
            None
        };

        let server_cols = server_metadata.as_ref().map(|m| m.columns.as_slice());
        let bulk = crate::bulk::BulkInsert::new_with_server_metadata(
            builder.columns().to_vec(),
            builder.options().batch_size,
            raw_meta,
            server_cols,
        );

        Ok(crate::bulk::BulkWriter::new(self, bulk))
    }

    /// Start a bulk insert without querying the server for column metadata.
    ///
    /// Unlike [`bulk_insert()`](Self::bulk_insert), this method does not send
    /// `SELECT TOP 0 * FROM table` to discover column types. Instead, the
    /// column metadata is constructed from the `BulkColumn` types provided
    /// on the builder. This saves a round-trip when the schema is known.
    ///
    /// # Caveats
    ///
    /// The caller must ensure `BulkColumn` entries match the target table's
    /// column definitions exactly. Mismatched types, lengths, precision/scale,
    /// or column ordering will cause the server to reject the BulkLoad packet.
    ///
    /// For most use cases, prefer [`bulk_insert()`](Self::bulk_insert) — the
    /// extra round-trip is usually negligible and the server-supplied metadata
    /// is guaranteed correct.
    pub async fn bulk_insert_without_schema_discovery(
        &mut self,
        builder: &crate::bulk::BulkInsertBuilder,
    ) -> Result<crate::bulk::BulkWriter<'_, S>> {
        tracing::debug!(
            table = builder.table_name(),
            columns = builder.columns().len(),
            "starting bulk insert (no schema discovery)"
        );

        // Send INSERT BULK statement to put server in bulk load mode
        let stmt = builder.build_insert_bulk_statement()?;
        self.send_sql_batch(&stmt).await?;
        self.read_execute_result().await?;

        // Create bulk writer with hand-crafted metadata
        let bulk = crate::bulk::BulkInsert::new(
            builder.columns().to_vec(),
            builder.options().batch_size,
        );

        Ok(crate::bulk::BulkWriter::new(self, bulk))
    }

    /// Send bulk load data as a BulkLoad (0x07) message and read the server response.
    ///
    /// Used internally by [`crate::bulk::BulkWriter::finish()`] to transmit accumulated
    /// row data after the `INSERT BULK` statement has been acknowledged.
    pub(crate) async fn send_and_read_bulk_load(&mut self, payload: bytes::Bytes) -> Result<u64> {
        let max_packet = self.config.packet_size as usize;

        self.in_flight = true;
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => {
                conn.send_message(PacketType::BulkLoad, payload, max_packet)
                    .await?;
            }
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => {
                conn.send_message(PacketType::BulkLoad, payload, max_packet)
                    .await?;
            }
            ConnectionHandle::Plain(conn) => {
                conn.send_message(PacketType::BulkLoad, payload, max_packet)
                    .await?;
            }
        }

        // Read the server's Done response with row count
        self.read_execute_result().await
    }

    /// Execute a query with named parameters and return a streaming result set.
    ///
    /// This method accepts [`NamedParam`](crate::to_params::NamedParam) values,
    /// making it compatible with the [`ToParams`](crate::to_params::ToParams) trait
    /// and the `#[derive(ToParams)]` macro.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::{NamedParam, ToParams};
    ///
    /// // With derive macro:
    /// #[derive(ToParams)]
    /// struct UserQuery { name: String }
    ///
    /// let q = UserQuery { name: "Alice".into() };
    /// let rows = client.query_named(
    ///     "SELECT * FROM users WHERE name = @name",
    ///     &q.to_params()?,
    /// ).await?;
    ///
    /// // Or manually:
    /// let params = vec![NamedParam::from_value("name", &"Alice")?];
    /// let rows = client.query_named(
    ///     "SELECT * FROM users WHERE name = @name",
    ///     &params,
    /// ).await?;
    /// ```
    pub async fn query_named<'a>(
        &'a mut self,
        sql: &str,
        params: &[crate::to_params::NamedParam],
    ) -> Result<QueryStream<'a>> {
        tracing::debug!(
            sql = sql,
            params_count = params.len(),
            "executing query with named parameters"
        );

        if params.is_empty() {
            self.send_sql_batch(sql).await?;
        } else {
            let rpc_params = Self::convert_named_params(params, self.send_unicode(), self.server_collation())?;
            let rpc = RpcRequest::execute_sql(sql, rpc_params);
            self.send_rpc(&rpc).await?;
        }

        let (columns, rows) = self.read_query_response().await?;
        Ok(QueryStream::new(columns, rows))
    }

    /// Execute a statement with named parameters.
    ///
    /// Returns the number of affected rows. This is the named-parameter
    /// counterpart of [`execute()`](Client::execute), compatible with the
    /// [`ToParams`](crate::to_params::ToParams) trait.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::NamedParam;
    ///
    /// let params = vec![
    ///     NamedParam::from_value("name", &"Alice")?,
    ///     NamedParam::from_value("email", &"alice@example.com")?,
    /// ];
    /// let rows_affected = client.execute_named(
    ///     "INSERT INTO users (name, email) VALUES (@name, @email)",
    ///     &params,
    /// ).await?;
    /// ```
    pub async fn execute_named(
        &mut self,
        sql: &str,
        params: &[crate::to_params::NamedParam],
    ) -> Result<u64> {
        tracing::debug!(
            sql = sql,
            params_count = params.len(),
            "executing statement with named parameters"
        );

        if params.is_empty() {
            self.send_sql_batch(sql).await?;
        } else {
            let rpc_params = Self::convert_named_params(params, self.send_unicode(), self.server_collation())?;
            let rpc = RpcRequest::execute_sql(sql, rpc_params);
            self.send_rpc(&rpc).await?;
        }

        self.read_execute_result().await
    }

    /// Whether string parameters are sent as NVARCHAR (Unicode).
    pub(crate) fn send_unicode(&self) -> bool {
        self.config.send_string_parameters_as_unicode
    }

    /// Server's default collation, captured from ENVCHANGE during login.
    pub(crate) fn server_collation(&self) -> Option<&tds_protocol::token::Collation> {
        self.server_collation.as_ref()
    }
}

impl Client<Ready> {
    /// Mark this connection as needing a reset on next use.
    ///
    /// Called by the connection pool when a connection is returned.
    /// The next SQL batch or RPC will include the RESETCONNECTION flag
    /// in the TDS packet header, causing SQL Server to reset connection
    /// state (temp tables, SET options, transaction isolation level, etc.)
    /// before executing the command.
    ///
    /// This is more efficient than calling `sp_reset_connection` as a
    /// separate command because it's handled at the TDS protocol level.
    pub fn mark_needs_reset(&mut self) {
        self.needs_reset = true;
    }

    /// Check if this connection needs a reset.
    ///
    /// Returns true if `mark_needs_reset()` was called and the reset
    /// hasn't been performed yet.
    #[must_use]
    pub fn needs_reset(&self) -> bool {
        self.needs_reset
    }

    /// Execute a query and return a streaming result set.
    ///
    /// Per ADR-007, results are streamed by default for memory efficiency.
    /// Use `.collect_all()` on the stream if you need all rows in memory.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// // Streaming (memory-efficient)
    /// let mut stream = client.query("SELECT * FROM users WHERE id = @p1", &[&1]).await?;
    /// while let Some(row) = stream.next().await {
    ///     let row = row?;
    ///     process(&row);
    /// }
    ///
    /// // Buffered (loads all into memory)
    /// let rows: Vec<Row> = client
    ///     .query("SELECT * FROM small_table", &[])
    ///     .await?
    ///     .collect_all()
    ///     .await?;
    /// ```
    pub async fn query<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<QueryStream<'a>> {
        tracing::debug!(sql = sql, params_count = params.len(), "executing query");

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(sql);

        let result = async {
            if params.is_empty() {
                // Simple query without parameters - use SQL batch
                self.send_sql_batch(sql).await?;
            } else {
                // Parameterized query - use sp_executesql via RPC
                let rpc_params = Self::convert_params(params, self.send_unicode(), self.server_collation())?;
                let rpc = RpcRequest::execute_sql(sql, rpc_params);
                self.send_rpc(&rpc).await?;
            }

            // Read complete response including columns and rows
            self.read_query_response().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        let (columns, rows) = result?;
        Ok(QueryStream::new(columns, rows))
    }

    /// Execute a query with a specific timeout.
    ///
    /// This overrides the default `command_timeout` from the connection configuration
    /// for this specific query. If the query does not complete within the specified
    /// duration, an error is returned.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    /// * `params` - Query parameters
    /// * `timeout_duration` - Maximum time to wait for the query to complete
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// // Execute with a 5-second timeout
    /// let rows = client
    ///     .query_with_timeout(
    ///         "SELECT * FROM large_table",
    ///         &[],
    ///         Duration::from_secs(5),
    ///     )
    ///     .await?;
    /// ```
    pub async fn query_with_timeout<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<QueryStream<'a>> {
        timeout(timeout_duration, self.query(sql, params))
            .await
            .map_err(|_| Error::CommandTimeout)?
    }

    /// Execute a batch that may return multiple result sets.
    ///
    /// This is useful for stored procedures or SQL batches that contain
    /// multiple SELECT statements.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Execute a batch with multiple SELECTs
    /// let mut results = client.query_multiple(
    ///     "SELECT 1 AS a; SELECT 2 AS b, 3 AS c;",
    ///     &[]
    /// ).await?;
    ///
    /// // Process first result set
    /// while let Some(row) = results.next_row().await? {
    ///     println!("Result 1: {:?}", row);
    /// }
    ///
    /// // Move to second result set
    /// if results.next_result().await? {
    ///     while let Some(row) = results.next_row().await? {
    ///         println!("Result 2: {:?}", row);
    ///     }
    /// }
    /// ```
    pub async fn query_multiple<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<MultiResultStream<'a>> {
        tracing::debug!(
            sql = sql,
            params_count = params.len(),
            "executing multi-result query"
        );

        if params.is_empty() {
            // Simple batch without parameters - use SQL batch
            self.send_sql_batch(sql).await?;
        } else {
            // Parameterized query - use sp_executesql via RPC
            let rpc_params = Self::convert_params(params, self.send_unicode(), self.server_collation())?;
            let rpc = RpcRequest::execute_sql(sql, rpc_params);
            self.send_rpc(&rpc).await?;
        }

        // Read all result sets
        let result_sets = self.read_multi_result_response().await?;
        Ok(MultiResultStream::new(result_sets))
    }

    /// Execute a query that doesn't return rows.
    ///
    /// Returns the number of affected rows.
    pub async fn execute(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<u64> {
        tracing::debug!(
            sql = sql,
            params_count = params.len(),
            "executing statement"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(sql);

        let result = async {
            if params.is_empty() {
                // Simple statement without parameters - use SQL batch
                self.send_sql_batch(sql).await?;
            } else {
                // Parameterized statement - use sp_executesql via RPC
                let rpc_params = Self::convert_params(params, self.send_unicode(), self.server_collation())?;
                let rpc = RpcRequest::execute_sql(sql, rpc_params);
                self.send_rpc(&rpc).await?;
            }

            // Read response and get row count
            self.read_execute_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(rows) => InstrumentationContext::record_success(&mut span, Some(*rows)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        result
    }

    /// Execute a statement with a specific timeout.
    ///
    /// This overrides the default `command_timeout` from the connection configuration
    /// for this specific statement. If the statement does not complete within the
    /// specified duration, an error is returned.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL statement to execute
    /// * `params` - Statement parameters
    /// * `timeout_duration` - Maximum time to wait for the statement to complete
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// // Execute with a 10-second timeout
    /// let rows_affected = client
    ///     .execute_with_timeout(
    ///         "UPDATE large_table SET status = @p1",
    ///         &[&"processed"],
    ///         Duration::from_secs(10),
    ///     )
    ///     .await?;
    /// ```
    pub async fn execute_with_timeout(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<u64> {
        timeout(timeout_duration, self.execute(sql, params))
            .await
            .map_err(|_| Error::CommandTimeout)?
    }

    /// Begin a transaction.
    ///
    /// This transitions the client from `Ready` to `InTransaction` state.
    /// Per MS-TDS spec, the server returns a transaction descriptor in the
    /// BeginTransaction EnvChange token that must be included in subsequent
    /// ALL_HEADERS sections.
    pub async fn begin_transaction(mut self) -> Result<Client<InTransaction>> {
        tracing::debug!("beginning transaction");

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.transaction_span("BEGIN");

        // Execute BEGIN TRANSACTION and extract the transaction descriptor
        let result = async {
            self.send_sql_batch("BEGIN TRANSACTION").await?;
            self.read_transaction_begin_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before moving instrumentation
        #[cfg(feature = "otel")]
        drop(span);

        let transaction_descriptor = result?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            server_collation: self.server_collation,
            statement_cache: self.statement_cache,
            transaction_descriptor, // Store the descriptor from server
            needs_reset: self.needs_reset,
            in_flight: self.in_flight,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
            #[cfg(feature = "always-encrypted")]
            encryption_context: self.encryption_context,
        })
    }

    /// Begin a transaction with a specific isolation level.
    ///
    /// This transitions the client from `Ready` to `InTransaction` state
    /// with the specified isolation level.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::IsolationLevel;
    ///
    /// let tx = client.begin_transaction_with_isolation(IsolationLevel::Serializable).await?;
    /// // All operations in this transaction use SERIALIZABLE isolation
    /// tx.commit().await?;
    /// ```
    pub async fn begin_transaction_with_isolation(
        mut self,
        isolation_level: crate::transaction::IsolationLevel,
    ) -> Result<Client<InTransaction>> {
        tracing::debug!(
            isolation_level = %isolation_level.name(),
            "beginning transaction with isolation level"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.transaction_span("BEGIN");

        // First set the isolation level
        let result = async {
            self.send_sql_batch(isolation_level.as_sql()).await?;
            self.read_execute_result().await?;

            // Then begin the transaction
            self.send_sql_batch("BEGIN TRANSACTION").await?;
            self.read_transaction_begin_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        #[cfg(feature = "otel")]
        drop(span);

        let transaction_descriptor = result?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            server_collation: self.server_collation,
            statement_cache: self.statement_cache,
            transaction_descriptor,
            needs_reset: self.needs_reset,
            in_flight: self.in_flight,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
            #[cfg(feature = "always-encrypted")]
            encryption_context: self.encryption_context,
        })
    }

    /// Execute a simple query without parameters.
    ///
    /// This is useful for DDL statements and simple queries where you
    /// don't need to retrieve the affected row count.
    pub async fn simple_query(&mut self, sql: &str) -> Result<()> {
        tracing::debug!(sql = sql, "executing simple query");

        // Send SQL batch
        self.send_sql_batch(sql).await?;

        // Read and discard response
        let _ = self.read_execute_result().await?;

        Ok(())
    }

    /// Close the connection gracefully.
    pub async fn close(self) -> Result<()> {
        tracing::debug!("closing connection");
        Ok(())
    }

    /// Get the current database name.
    #[must_use]
    pub fn database(&self) -> Option<&str> {
        self.config.database.as_deref()
    }

    /// Get the server host.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.config.host
    }

    /// Get the server port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Check if the connection is currently in a transaction.
    ///
    /// This returns `true` if a transaction was started via raw SQL
    /// (`BEGIN TRANSACTION`) and has not yet been committed or rolled back.
    ///
    /// Note: This only tracks transactions started via raw SQL. Transactions
    /// started via the type-state API (`begin_transaction()`) result in a
    /// `Client<InTransaction>` which is a different type.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// client.execute("BEGIN TRANSACTION", &[]).await?;
    /// assert!(client.is_in_transaction());
    ///
    /// client.execute("COMMIT", &[]).await?;
    /// assert!(!client.is_in_transaction());
    /// ```
    #[must_use]
    pub fn is_in_transaction(&self) -> bool {
        self.transaction_descriptor != 0
    }

    /// Check if a request is in-flight (sent but response not fully read).
    ///
    /// Used by the connection pool to detect dirty connections that were
    /// interrupted mid-query (e.g., by `tokio::select!` or a timeout).
    /// A connection with an in-flight request has unread data in the TCP
    /// buffer and must be discarded rather than returned to the pool.
    #[must_use]
    pub fn is_in_flight(&self) -> bool {
        self.in_flight
    }

    /// Report whether an Always Encrypted key-store provider with the given
    /// name is currently reachable through this client's encryption context.
    ///
    /// Returns `false` when the `always-encrypted` feature isn't enabled, when
    /// the connection was opened without `column_encryption` configured, or
    /// when no matching provider was registered.
    #[cfg(feature = "always-encrypted")]
    #[must_use]
    pub fn has_encryption_provider(&self, name: &str) -> bool {
        self.encryption_context
            .as_ref()
            .is_some_and(|ctx| ctx.has_provider(name))
    }

    /// Get a handle for cancelling the current query.
    ///
    /// The cancel handle can be cloned and sent to other tasks, enabling
    /// cancellation of long-running queries from a separate async context.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let cancel_handle = client.cancel_handle();
    ///
    /// // Spawn a task to cancel after 10 seconds
    /// let handle = tokio::spawn(async move {
    ///     tokio::time::sleep(Duration::from_secs(10)).await;
    ///     let _ = cancel_handle.cancel().await;
    /// });
    ///
    /// // This query will be cancelled if it runs longer than 10 seconds
    /// let result = client.query("SELECT * FROM very_large_table", &[]).await;
    /// ```
    #[must_use]
    pub fn cancel_handle(&self) -> crate::cancel::CancelHandle {
        let connection = self
            .connection
            .as_ref()
            .expect("connection should be present");
        match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => {
                crate::cancel::CancelHandle::from_tls(conn.cancel_handle())
            }
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => {
                crate::cancel::CancelHandle::from_tls_prelogin(conn.cancel_handle())
            }
            ConnectionHandle::Plain(conn) => {
                crate::cancel::CancelHandle::from_plain(conn.cancel_handle())
            }
        }
    }
}

/// # Drop Behavior
///
/// **`Client<InTransaction>` has no automatic rollback on drop.** If the client is
/// dropped without calling [`commit()`](Client::commit) or [`rollback()`](Client::rollback),
/// the transaction remains open on the server until the TCP connection closes
/// (at which point SQL Server automatically rolls back).
///
/// This is because `Drop` is synchronous and cannot perform the async I/O needed
/// to send a `ROLLBACK TRANSACTION` command.
///
/// ## Consequences of dropping without commit/rollback
///
/// - **Direct connections:** The transaction leaks until the OS TCP timeout
///   (potentially 30+ minutes), holding locks on any modified rows.
/// - **Pooled connections:** The pool detects the active transaction descriptor
///   and discards the connection rather than returning it to the idle pool
///   (see `PooledConnection::drop` in `mssql-driver-pool`).
///
/// ## Best practice
///
/// Always ensure `commit()` or `rollback()` is called. Use helper patterns
/// for error paths:
///
/// ```rust,ignore
/// let tx = client.begin_transaction().await?;
/// match do_work(&tx).await {
///     Ok(_) => { tx.commit().await?; }
///     Err(e) => { tx.rollback().await?; return Err(e); }
/// }
/// ```
impl Client<InTransaction> {
    /// Execute a query within the transaction and return a streaming result set.
    ///
    /// See [`Client<Ready>::query`] for usage examples.
    pub async fn query<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<QueryStream<'a>> {
        tracing::debug!(
            sql = sql,
            params_count = params.len(),
            "executing query in transaction"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(sql);

        let result = async {
            if params.is_empty() {
                // Simple query without parameters - use SQL batch
                self.send_sql_batch(sql).await?;
            } else {
                // Parameterized query - use sp_executesql via RPC
                let rpc_params = Self::convert_params(params, self.send_unicode(), self.server_collation())?;
                let rpc = RpcRequest::execute_sql(sql, rpc_params);
                self.send_rpc(&rpc).await?;
            }

            // Read complete response including columns and rows
            self.read_query_response().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        let (columns, rows) = result?;
        Ok(QueryStream::new(columns, rows))
    }

    /// Execute a statement within the transaction.
    ///
    /// Returns the number of affected rows.
    pub async fn execute(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<u64> {
        tracing::debug!(
            sql = sql,
            params_count = params.len(),
            "executing statement in transaction"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(sql);

        let result = async {
            if params.is_empty() {
                // Simple statement without parameters - use SQL batch
                self.send_sql_batch(sql).await?;
            } else {
                // Parameterized statement - use sp_executesql via RPC
                let rpc_params = Self::convert_params(params, self.send_unicode(), self.server_collation())?;
                let rpc = RpcRequest::execute_sql(sql, rpc_params);
                self.send_rpc(&rpc).await?;
            }

            // Read response and get row count
            self.read_execute_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(rows) => InstrumentationContext::record_success(&mut span, Some(*rows)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        result
    }

    /// Execute a query within the transaction with a specific timeout.
    ///
    /// See [`Client<Ready>::query_with_timeout`] for details.
    pub async fn query_with_timeout<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<QueryStream<'a>> {
        timeout(timeout_duration, self.query(sql, params))
            .await
            .map_err(|_| Error::CommandTimeout)?
    }

    /// Execute a statement within the transaction with a specific timeout.
    ///
    /// See [`Client<Ready>::execute_with_timeout`] for details.
    pub async fn execute_with_timeout(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<u64> {
        timeout(timeout_duration, self.execute(sql, params))
            .await
            .map_err(|_| Error::CommandTimeout)?
    }

    /// Open a FILESTREAM BLOB for async reading and/or writing.
    ///
    /// This method queries the server for the transaction context, then opens
    /// the FILESTREAM handle using the native Win32 `OpenSqlFilestream` API.
    ///
    /// # Arguments
    ///
    /// * `path` — The UNC path obtained from the T-SQL `column.PathName()` function.
    ///   Query this yourself before calling `open_filestream`:
    ///   ```sql
    ///   SELECT Content.PathName() FROM dbo.Documents WHERE Id = @p1
    ///   ```
    /// * `access` — Read, write, or read/write access mode.
    ///
    /// # Requirements
    ///
    /// - SQL Server must have FILESTREAM enabled (`sp_configure 'filestream access level', 2`)
    /// - The Microsoft OLE DB Driver for SQL Server must be installed on the client
    /// - The `FileStream` must be dropped before calling [`commit`] or [`rollback`]
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::FileStreamAccess;
    /// use tokio::io::AsyncReadExt;
    ///
    /// let mut tx = client.begin_transaction().await?;
    ///
    /// // Get the FILESTREAM path
    /// let rows = tx.query(
    ///     "SELECT Content.PathName() FROM dbo.Documents WHERE Id = @p1",
    ///     &[&doc_id],
    /// ).await?;
    /// let path: String = rows.into_iter().next().unwrap()?.get(0)?;
    ///
    /// // Open and read the BLOB
    /// let mut stream = tx.open_filestream(&path, FileStreamAccess::Read).await?;
    /// let mut data = Vec::new();
    /// stream.read_to_end(&mut data).await?;
    /// drop(stream);
    ///
    /// tx.commit().await?;
    /// ```
    #[cfg(all(windows, feature = "filestream"))]
    pub async fn open_filestream(
        &mut self,
        path: &str,
        access: crate::filestream::FileStreamAccess,
    ) -> Result<crate::filestream::FileStream> {
        tracing::debug!(path = path, ?access, "opening FILESTREAM BLOB");

        // Get the transaction context from SQL Server.
        // This binds the file access to the current SQL transaction.
        let txn_context: Vec<u8> = {
            let rows = self
                .query("SELECT GET_FILESTREAM_TRANSACTION_CONTEXT()", &[])
                .await?;
            let mut ctx = None;
            for result in rows {
                let row = result?;
                ctx = Some(row.get::<Vec<u8>>(0)?);
            }
            ctx.ok_or_else(|| {
                Error::FileStream("GET_FILESTREAM_TRANSACTION_CONTEXT() returned no rows".into())
            })?
        };

        crate::filestream::FileStream::open(path, access, &txn_context)
    }

    /// Commit the transaction.
    ///
    /// This transitions the client back to `Ready` state.
    pub async fn commit(mut self) -> Result<Client<Ready>> {
        tracing::debug!("committing transaction");

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.transaction_span("COMMIT");

        // Execute COMMIT TRANSACTION
        let result = async {
            self.send_sql_batch("COMMIT TRANSACTION").await?;
            self.read_execute_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before moving instrumentation
        #[cfg(feature = "otel")]
        drop(span);

        result?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            server_collation: self.server_collation,
            statement_cache: self.statement_cache,
            transaction_descriptor: 0, // Reset to auto-commit mode
            needs_reset: self.needs_reset,
            in_flight: self.in_flight,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
            #[cfg(feature = "always-encrypted")]
            encryption_context: self.encryption_context,
        })
    }

    /// Rollback the transaction.
    ///
    /// This transitions the client back to `Ready` state.
    pub async fn rollback(mut self) -> Result<Client<Ready>> {
        tracing::debug!("rolling back transaction");

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.transaction_span("ROLLBACK");

        // Execute ROLLBACK TRANSACTION
        let result = async {
            self.send_sql_batch("ROLLBACK TRANSACTION").await?;
            self.read_execute_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        // Drop the span before moving instrumentation
        #[cfg(feature = "otel")]
        drop(span);

        result?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            server_collation: self.server_collation,
            statement_cache: self.statement_cache,
            transaction_descriptor: 0, // Reset to auto-commit mode
            needs_reset: self.needs_reset,
            in_flight: self.in_flight,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
            #[cfg(feature = "always-encrypted")]
            encryption_context: self.encryption_context,
        })
    }

    /// Create a savepoint and return a handle for later rollback.
    ///
    /// The returned `SavePoint` handle contains the validated savepoint name.
    /// Use it with `rollback_to()` to partially undo transaction work.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tx = client.begin_transaction().await?;
    /// tx.execute("INSERT INTO orders ...").await?;
    /// let sp = tx.save_point("before_items").await?;
    /// tx.execute("INSERT INTO items ...").await?;
    /// // Oops, rollback just the items
    /// tx.rollback_to(&sp).await?;
    /// tx.commit().await?;
    /// ```
    pub async fn save_point(&mut self, name: &str) -> Result<SavePoint> {
        crate::validation::validate_identifier(name)?;
        tracing::debug!(name = name, "creating savepoint");

        // Execute SAVE TRANSACTION <name>
        // Note: name is validated by validate_identifier() to prevent SQL injection
        let sql = format!("SAVE TRANSACTION {name}");
        self.send_sql_batch(&sql).await?;
        self.read_execute_result().await?;

        Ok(SavePoint::new(name.to_string()))
    }

    /// Rollback to a savepoint.
    ///
    /// This rolls back all changes made after the savepoint was created,
    /// but keeps the transaction active. The savepoint remains valid and
    /// can be rolled back to again.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let sp = tx.save_point("checkpoint").await?;
    /// // ... do some work ...
    /// tx.rollback_to(&sp).await?;  // Undo changes since checkpoint
    /// // Transaction is still active, savepoint is still valid
    /// ```
    pub async fn rollback_to(&mut self, savepoint: &SavePoint) -> Result<()> {
        tracing::debug!(name = savepoint.name(), "rolling back to savepoint");

        // Execute ROLLBACK TRANSACTION <name>
        // Note: savepoint name was validated during creation
        let sql = format!("ROLLBACK TRANSACTION {}", savepoint.name());
        self.send_sql_batch(&sql).await?;
        self.read_execute_result().await?;

        Ok(())
    }

    /// Release a savepoint (optional cleanup).
    ///
    /// Note: SQL Server doesn't have explicit savepoint release, but this
    /// method is provided for API completeness. The savepoint is automatically
    /// released when the transaction commits or rolls back.
    pub async fn release_savepoint(&mut self, savepoint: SavePoint) -> Result<()> {
        tracing::debug!(name = savepoint.name(), "releasing savepoint");

        // SQL Server doesn't require explicit savepoint release
        // The savepoint is implicitly released on commit/rollback
        // This method exists for API completeness
        drop(savepoint);
        Ok(())
    }

    /// Get a handle for cancelling the current query within the transaction.
    ///
    /// See [`Client<Ready>::cancel_handle`] for usage examples.
    #[must_use]
    pub fn cancel_handle(&self) -> crate::cancel::CancelHandle {
        let connection = self
            .connection
            .as_ref()
            .expect("connection should be present");
        match connection {
            #[cfg(feature = "tls")]
            ConnectionHandle::Tls(conn) => {
                crate::cancel::CancelHandle::from_tls(conn.cancel_handle())
            }
            #[cfg(feature = "tls")]
            ConnectionHandle::TlsPrelogin(conn) => {
                crate::cancel::CancelHandle::from_tls_prelogin(conn.cancel_handle())
            }
            ConnectionHandle::Plain(conn) => {
                crate::cancel::CancelHandle::from_plain(conn.cancel_handle())
            }
        }
    }
}

impl<S: ConnectionState> std::fmt::Debug for Client<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("host", &self.config.host)
            .field("port", &self.config.port)
            .field("database", &self.config.database)
            .finish()
    }
}
