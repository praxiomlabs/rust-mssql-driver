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
    /// Prepared statement cache for query optimization
    statement_cache: StatementCache,
    /// Transaction descriptor from BeginTransaction EnvChange.
    /// Per MS-TDS spec, this value must be included in ALL_HEADERS for subsequent
    /// requests within an explicit transaction. 0 indicates auto-commit mode.
    transaction_descriptor: u64,
    /// Whether this connection needs a reset on next use.
    /// Set by connection pool on checkin, cleared after first query/execute.
    /// When true, the RESETCONNECTION flag is set on the first TDS packet.
    needs_reset: bool,
    /// OpenTelemetry instrumentation context (when otel feature is enabled)
    #[cfg(feature = "otel")]
    instrumentation: InstrumentationContext,
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
    async fn send_rpc(&mut self, rpc: &RpcRequest) -> Result<()> {
        let payload = rpc.encode_with_transaction(self.transaction_descriptor);
        let max_packet = self.config.packet_size as usize;

        // Check if we need to reset the connection on this request
        let reset = self.needs_reset;
        if reset {
            self.needs_reset = false; // Clear flag before sending
            tracing::debug!("sending RPC with RESETCONNECTION flag");
        }

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
                let rpc_params = Self::convert_params(params)?;
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
            let rpc_params = Self::convert_params(params)?;
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
                let rpc_params = Self::convert_params(params)?;
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

    /// Execute a stored procedure with automatic OUTPUT parameter detection.
    ///
    /// This method provides comprehensive stored procedure execution including:
    /// - **Simplified API**: Only provide INPUT parameters, OUTPUT parameters are auto-detected
    /// - RETURN value handling (always included per SQL Server spec)
    /// - Result set processing (for SELECT statements within procedures)
    /// - Row count tracking
    ///
    /// # Simplified API (Recommended)
    ///
    /// **Only provide INPUT parameters - OUTPUT parameters are automatically detected:**
    ///
    /// ```rust,ignore
    /// // SQL: CREATE PROCEDURE dbo.CalculateSum
    /// //      @a INT, @b INT, @result INT OUTPUT
    /// //      AS SET @result = @a + @b;
    ///
    /// let result = client.execute_procedure(
    ///     "dbo.CalculateSum",
    ///     &[&10i32, &20i32]  // Only INPUT parameters needed!
    /// ).await?;
    ///
    /// // Access OUTPUT parameter (auto-detected from metadata)
    /// let sum: i32 = result.get_output("@result").unwrap().value.as_i32()?;
    /// println!("Sum: {}", sum);  // 30
    /// ```
    ///
    /// # Arguments
    ///
    /// * `proc_name` - Stored procedure name (optionally schema-qualified, e.g., "dbo.MyProc")
    /// * `params` - INPUT parameters only (OUTPUT parameters are auto-detected and filled)
    ///
    /// # Returns
    ///
    /// Returns an [`ExecuteResult`](crate::stream::ExecuteResult) containing:
    /// - `output_params`: Vec of output parameters (index 0 = RETURN value, index 1+ = OUTPUT params)
    /// - `rows_affected`: Number of rows modified
    /// - `result_set`: Optional result set from SELECT statements
    ///
    /// # How It Works
    ///
    /// 1. **Metadata Query**: Queries `sp_sproc_columns` to get parameter information
    /// 2. **Smart Detection**: Automatically identifies INPUT vs OUTPUT parameters
    /// 3. **Auto-Fill**: OUTPUT parameters are automatically filled with NULL values
    /// 4. **Type Safety**: Uses correct type information from metadata
    ///
    /// # Examples
    ///
    /// ## Basic OUTPUT Parameters
    ///
    /// ```rust,ignore
    /// // SQL: CREATE PROCEDURE dbo.sp_Calculate
    /// //      @input INT,
    /// //      @doubled INT OUTPUT,
    /// //      @tripled INT OUTPUT
    /// //      AS
    /// //      BEGIN
    /// //          SET @doubled = @input * 2;
    /// //          SET @tripled = @input * 3;
    /// //      END
    ///
    /// let result = client
    ///     .execute_procedure("dbo.sp_Calculate", &[&7i32])
    ///     .await?;
    ///
    /// let doubled = result.get_output("@doubled").unwrap();
    /// let tripled = result.get_output("@tripled").unwrap();
    ///
    /// println!("Doubled: {}", doubled.value.as_i32()?);  // 14
    /// println!("Tripled: {}", tripled.value.as_i32()?);  // 21
    /// ```
    ///
    /// ## Result Sets + OUTPUT Parameters
    ///
    /// ```rust,ignore
    /// // SQL:
    /// // CREATE PROCEDURE dbo.GetUserStats
    /// //     @min_score INT,
    /// //     @row_count INT OUTPUT
    /// // AS
    /// // BEGIN
    /// //     SELECT Id, Name, Score FROM Users WHERE Score >= @min_score;
    /// //     SET @row_count = @@ROWCOUNT;
    /// // END
    ///
    /// let result = client
    ///     .execute_procedure("dbo.GetUserStats", &[&90i32])
    ///     .await?;
    ///
    /// // Process result set
    /// if let Some(mut stream) = result.result_set {
    ///     while let Some(Ok(row)) = stream.next() {
    ///         let id: i32 = row.get(0)?;
    ///         let name: String = row.get(1)?;
    ///         let score: i32 = row.get(2)?;
    ///         println!("{}: {} (score: {})", id, name, score);
    ///     }
    /// }
    ///
    /// // Get OUTPUT parameter
    /// let count = result.get_output("@row_count").unwrap();
    /// println!("Total: {} rows", count.value.as_i32()?);
    /// ```
    ///
    /// ## RETURN Statement Support
    ///
    /// ```rust,ignore
    /// // SQL:
    /// // CREATE PROCEDURE dbo.CheckStatus
    /// //     @id INT
    /// // AS
    /// // BEGIN
    /// //     IF EXISTS (SELECT 1 FROM Users WHERE Id = @id)
    /// //         RETURN 1;
    /// //     RETURN 0;
    /// // END
    ///
    /// let result = client
    ///     .execute_procedure("dbo.CheckStatus", &[&123i32])
    ///     .await?;
    ///
    /// // RETURN value comes as first output param (always present)
    /// let status = result.get_return_value().unwrap();
    /// let value: i32 = status.value.as_i32()?;
    /// println!("Status: {}", value);
    /// ```
    ///
    /// ## Only OUTPUT Parameters (No INPUT)
    ///
    /// ```rust,ignore
    /// // SQL: CREATE PROCEDURE dbo.sp_GetConstant
    /// //      @result INT OUTPUT
    /// //      AS SET @result = 42;
    ///
    /// let params: &[&(dyn mssql_client::ToSql + Sync)] = &[];
    /// let result = client
    ///     .execute_procedure("dbo.sp_GetConstant", params)
    ///     .await?;
    ///
    /// let value = result.get_output("@result").unwrap();
    /// println!("Constant: {}", value.value.as_i32()?);  // 42
    /// ```
    ///
    /// # Error Handling
    ///
    /// ```rust,ignore
    /// use mssql_client::Error;
    ///
    /// match client.execute_procedure("dbo.MyProc", &[&input]).await {
    ///     Ok(result) => {
    ///         // Process result
    ///     }
    ///     Err(Error::Protocol(msg)) => {
    ///         // Parameter count mismatch or other protocol errors
    ///         eprintln!("Protocol error: {}", msg);
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Execution failed: {}", e);
    ///     }
    /// }
    /// ```
    pub async fn execute_procedure(
        &mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::stream::ExecuteResult<'_>> {
        tracing::debug!(
            proc_name = proc_name,
            params_count = params.len(),
            "executing stored procedure"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(proc_name);

        let result = async {
            // Convert ToSql parameters to RpcParam with automatic OUTPUT detection
            let rpc_params = self.convert_params_for_procedure(proc_name, params).await?;

            // Create RPC request for named procedure
            let mut rpc = tds_protocol::rpc::RpcRequest::named(proc_name);
            for param in rpc_params {
                rpc = rpc.param(param);
            }
            self.send_rpc(&rpc).await?;

            // Read response with output parameters, result set, and row count
            self.read_stored_proc_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(res) => InstrumentationContext::record_success(&mut span, Some(res.rows_affected)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        #[cfg(feature = "otel")]
        drop(span);

        result
    }

    /// Execute a stored procedure with a timeout.
    ///
    /// This is a convenience method that combines [`execute_procedure`](Self::execute_procedure)
    /// with a timeout. Uses the same simplified API - only provide INPUT parameters.
    ///
    /// # Arguments
    ///
    /// * `proc_name` - Stored procedure name (optionally schema-qualified)
    /// * `params` - INPUT parameters only (OUTPUT parameters are auto-detected)
    /// * `timeout_duration` - Maximum time to wait for the procedure to complete
    ///
    /// # Returns
    ///
    /// Returns [`Error::CommandTimeout`] if the timeout expires before completion.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// // Execute with a 5-second timeout
    /// let result = client
    ///     .execute_procedure_with_timeout(
    ///         "dbo.sp_LongRunning",
    ///         &[&input_value],
    ///         Duration::from_secs(5),
    ///     )
    ///     .await?;
    ///
    /// // Access OUTPUT parameters
    /// let output = result.get_output("@result").unwrap();
    /// ```
    pub async fn execute_procedure_with_timeout(
        &mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<crate::stream::ExecuteResult<'_>> {
        timeout(timeout_duration, self.execute_procedure(proc_name, params))
            .await
            .map_err(|_| Error::CommandTimeout)?
    }

    /// Execute a stored procedure that may return multiple result sets.
    ///
    /// This method is similar to [`execute_procedure`](Self::execute_procedure) but
    /// returns all result sets from the stored procedure, making it suitable for
    /// procedures that contain multiple SELECT statements.
    ///
    /// The method uses the **simplified API** - only provide INPUT parameters,
    /// OUTPUT parameters are automatically detected.
    ///
    /// # Arguments
    ///
    /// * `proc_name` - Stored procedure name (optionally schema-qualified, e.g., "dbo.MyProc")
    /// * `params` - INPUT parameters only (OUTPUT parameters are auto-detected)
    ///
    /// # Returns
    ///
    /// Returns a [`MultiExecuteResult`](crate::stream::MultiExecuteResult) containing:
    /// - `output_params`: Vec of output parameters (index 0 = RETURN value)
    /// - `rows_affected`: Number of rows modified
    /// - Multiple result sets (accessible via `next_row()` and `next_result()`)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // SQL: CREATE PROCEDURE dbo.sp_GetMultipleData
    /// //      @min_score INT,
    /// //      @row_count INT OUTPUT
    /// // AS
    /// // BEGIN
    /// //     -- First result set
    /// //     SELECT Id, Name FROM Users WHERE Score >= @min_score;
    /// //
    /// //     -- Second result set
    /// //     SELECT Id, Score FROM UserScores WHERE Score >= @min_score;
    /// //
    /// //     SET @row_count = @@ROWCOUNT;
    /// // END
    ///
    /// let mut result = client
    ///     .execute_procedure_multiple("dbo.sp_GetMultipleData", &[&90i32])
    ///     .await?;
    ///
    /// // Access OUTPUT parameters
    /// let row_count = result.get_output("@row_count").unwrap();
    /// println!("Total rows: {}", row_count.value.as_i32()?);
    ///
    /// // Process first result set
    /// while let Some(row) = result.next_row().await? {
    ///     let id: i32 = row.get(0)?;
    ///     let name: String = row.get(1)?;
    ///     println!("User: {} - {}", id, name);
    /// }
    ///
    /// // Move to second result set
    /// if result.next_result().await? {
    ///     while let Some(row) = result.next_row().await? {
    ///         let id: i32 = row.get(0)?;
    ///         let score: i32 = row.get(1)?;
    ///         println!("Score: {} - {}", id, score);
    ///     }
    /// }
    /// ```
    ///
    /// # When to Use
    ///
    /// - Use `execute_procedure` when you only need the first result set
    /// - Use `execute_procedure_multiple` when you need all result sets
    /// - Both methods support OUTPUT parameters and RETURN values
    pub async fn execute_procedure_multiple<'a>(
        &'a mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::stream::MultiExecuteResult<'a>> {
        tracing::debug!(
            proc_name = proc_name,
            params_count = params.len(),
            "executing stored procedure with multiple result sets"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(proc_name);

        let result = async {
            // Convert ToSql parameters to RpcParam with automatic OUTPUT detection
            let rpc_params = self.convert_params_for_procedure(proc_name, params).await?;

            // Create RPC request for named procedure
            let mut rpc = tds_protocol::rpc::RpcRequest::named(proc_name);
            for param in rpc_params {
                rpc = rpc.param(param);
            }
            self.send_rpc(&rpc).await?;

            // Read response with output parameters, multiple result sets, and row count
            self.read_stored_proc_multiple_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(res) => InstrumentationContext::record_success(&mut span, Some(res.rows_affected)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        #[cfg(feature = "otel")]
        drop(span);

        result
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
            statement_cache: self.statement_cache,
            transaction_descriptor, // Store the descriptor from server
            needs_reset: self.needs_reset,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
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
            statement_cache: self.statement_cache,
            transaction_descriptor,
            needs_reset: self.needs_reset,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
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
                let rpc_params = Self::convert_params(params)?;
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
                let rpc_params = Self::convert_params(params)?;
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

    /// Execute a stored procedure within the transaction with automatic OUTPUT parameter detection.
    ///
    /// This is the transaction-aware version of [`Client<Ready>::execute_procedure`].
    /// See that method for full documentation and examples.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut tx = client.begin_transaction().await?;
    ///
    /// // SQL: CREATE PROCEDURE dbo.UpdateUser
    /// //      @userId INT, @name NVARCHAR(100), @newId INT OUTPUT
    /// //      AS
    /// //      BEGIN
    /// //          UPDATE Users SET Name = @name WHERE Id = @userId;
    /// //          SET @newId = @userId;
    /// //      END
    ///
    /// let result = tx
    ///     .execute_procedure("dbo.UpdateUser", &[&123i32, &"John"])
    ///     .await?;
    ///
    /// let new_id: i32 = result.get_output("@newId").unwrap().value.as_i32()?;
    /// println!("New ID: {}", new_id);
    /// println!("Rows affected: {}", result.rows_affected);
    ///
    /// tx.commit().await?;
    /// ```
    pub async fn execute_procedure(
        &mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::stream::ExecuteResult<'_>> {
        tracing::debug!(
            proc_name = proc_name,
            params_count = params.len(),
            "executing stored procedure in transaction"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(proc_name);

        let result = async {
            // Convert ToSql parameters to RpcParam with automatic OUTPUT detection
            let rpc_params = self.convert_params_for_procedure(proc_name, params).await?;

            // Create RPC request for named procedure
            let mut rpc = tds_protocol::rpc::RpcRequest::named(proc_name);
            for param in rpc_params {
                rpc = rpc.param(param);
            }
            self.send_rpc(&rpc).await?;

            // Read response with output parameters, result set, and row count
            self.read_stored_proc_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(res) => InstrumentationContext::record_success(&mut span, Some(res.rows_affected)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        #[cfg(feature = "otel")]
        drop(span);

        result
    }

    /// Execute a stored procedure within the transaction with a timeout.
    ///
    /// This is a convenience method that combines [`execute_procedure`](Self::execute_procedure)
    /// with a timeout. Uses the same simplified API - only provide INPUT parameters.
    ///
    /// # Arguments
    ///
    /// * `proc_name` - Stored procedure name (optionally schema-qualified)
    /// * `params` - INPUT parameters only (OUTPUT parameters are auto-detected)
    /// * `timeout_duration` - Maximum time to wait for the procedure to complete
    ///
    /// # Returns
    ///
    /// Returns [`Error::CommandTimeout`] if the timeout expires before completion.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let mut tx = client.begin_transaction().await?;
    ///
    /// // Execute with a 5-second timeout
    /// let result = tx
    ///     .execute_procedure_with_timeout(
    ///         "dbo.sp_UpdateUser",
    ///         &[&user_id, &"John"],
    ///         Duration::from_secs(5),
    ///     )
    ///     .await?;
    ///
    /// tx.commit().await?;
    /// ```
    pub async fn execute_procedure_with_timeout(
        &mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<crate::stream::ExecuteResult<'_>> {
        timeout(timeout_duration, self.execute_procedure(proc_name, params))
            .await
            .map_err(|_| Error::CommandTimeout)?
    }

    /// Execute a stored procedure within the transaction that may return multiple result sets.
    ///
    /// This is the transaction-aware version of [`Client<Ready>::execute_procedure_multiple`].
    /// See that method for full documentation and examples.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut tx = client.begin_transaction().await?;
    ///
    /// // SQL: CREATE PROCEDURE dbo.sp_GetUserData
    /// //      @userId INT,
    /// //      @profile_count INT OUTPUT
    /// // AS
    /// // BEGIN
    /// //     -- First result set
    /// //     SELECT Id, Name FROM Users WHERE Id = @userId;
    /// //
    /// //     -- Second result set
    /// //     SELECT ProfileId, Bio FROM UserProfiles WHERE UserId = @userId;
    /// //
    /// //     SET @profile_count = @@ROWCOUNT;
    /// // END
    ///
    /// let mut result = tx
    ///     .execute_procedure_multiple("dbo.sp_GetUserData", &[&123i32])
    ///     .await?;
    ///
    /// // Access OUTPUT parameters
    /// let count = result.get_output("@profile_count").unwrap();
    /// println!("Profile count: {}", count.value.as_i32()?);
    ///
    /// // Process first result set
    /// while let Some(row) = result.next_row().await? {
    ///     let id: i32 = row.get(0)?;
    ///     let name: String = row.get(1)?;
    ///     println!("User: {} - {}", id, name);
    /// }
    ///
    /// // Move to second result set
    /// if result.next_result().await? {
    ///     while let Some(row) = result.next_row().await? {
    ///         let profile_id: i32 = row.get(0)?;
    ///         let bio: String = row.get(1)?;
    ///         println!("Profile: {} - {}", profile_id, bio);
    ///     }
    /// }
    ///
    /// tx.commit().await?;
    /// ```
    pub async fn execute_procedure_multiple<'a>(
        &'a mut self,
        proc_name: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::stream::MultiExecuteResult<'a>> {
        tracing::debug!(
            proc_name = proc_name,
            params_count = params.len(),
            "executing stored procedure with multiple result sets in transaction"
        );

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(proc_name);

        let result = async {
            // Convert ToSql parameters to RpcParam with automatic OUTPUT detection
            let rpc_params = self.convert_params_for_procedure(proc_name, params).await?;

            // Create RPC request for named procedure
            let mut rpc = tds_protocol::rpc::RpcRequest::named(proc_name);
            for param in rpc_params {
                rpc = rpc.param(param);
            }
            self.send_rpc(&rpc).await?;

            // Read response with output parameters, multiple result sets, and row count
            self.read_stored_proc_multiple_result().await
        }
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(res) => InstrumentationContext::record_success(&mut span, Some(res.rows_affected)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }

        #[cfg(feature = "otel")]
        drop(span);

        result
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
            statement_cache: self.statement_cache,
            transaction_descriptor: 0, // Reset to auto-commit mode
            needs_reset: self.needs_reset,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
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
            statement_cache: self.statement_cache,
            transaction_descriptor: 0, // Reset to auto-commit mode
            needs_reset: self.needs_reset,
            #[cfg(feature = "otel")]
            instrumentation: self.instrumentation,
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
        validate_identifier(name)?;
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

/// Validate an identifier (table name, savepoint name, etc.) to prevent SQL injection.
fn validate_identifier(name: &str) -> Result<()> {
    use once_cell::sync::Lazy;
    use regex::Regex;

    static IDENTIFIER_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_@#$]{0,127}$").unwrap());

    if name.is_empty() {
        return Err(Error::InvalidIdentifier(
            "identifier cannot be empty".into(),
        ));
    }

    if !IDENTIFIER_RE.is_match(name) {
        return Err(Error::InvalidIdentifier(format!(
            "invalid identifier '{name}': must start with letter/underscore, \
             contain only alphanumerics/_/@/#/$, and be 1-128 characters"
        )));
    }

    Ok(())
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_identifier_valid() {
        assert!(validate_identifier("my_table").is_ok());
        assert!(validate_identifier("Table123").is_ok());
        assert!(validate_identifier("_private").is_ok());
        assert!(validate_identifier("sp_test").is_ok());
    }

    #[test]
    fn test_validate_identifier_invalid() {
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("123abc").is_err());
        assert!(validate_identifier("table-name").is_err());
        assert!(validate_identifier("table name").is_err());
        assert!(validate_identifier("table;DROP TABLE users").is_err());
    }
}
