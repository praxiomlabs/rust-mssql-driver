//! SQL Server client implementation.
//!
//! ## DDL and statement routing
//!
//! [`Client::execute`] routes automatically by parameter count: with no
//! parameters it sends a SQL batch (which permits DDL such as `CREATE` / `ALTER`
//! / `DROP`); with parameters it uses `sp_executesql`, whose procedure context
//! SQL Server forbids DDL in. Run DDL with an empty parameter slice:
//!
//! ```rust,no_run
//! # async fn create_table(config: mssql_client::Config) -> Result<(), mssql_client::Error> {
//! # let mut client = mssql_client::Client::connect(config).await?;
//! client.execute("CREATE TABLE dbo.t (id INT)", &[]).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Use [`Client::simple_query`] for fire-and-forget batches (including
//! multi-statement, `;`-separated DDL) when you don't need the affected-row count.

// Allow unwrap/expect for chrono date construction with known-valid constant dates
// and for regex patterns that are compile-time constants
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::needless_range_loop)]

mod connect;
mod params;
pub(crate) mod response;

use std::marker::PhantomData;

use mssql_codec::connection::Connection;
#[cfg(feature = "tls")]
use mssql_tls::TlsStream;
use tds_protocol::packet::PacketType;
use tds_protocol::rpc::RpcRequest;
use tds_protocol::token::{EnvChange, EnvChangeType};
use tokio::net::TcpStream;

use crate::config::Config;
use crate::error::{Error, Result};
#[cfg(feature = "otel")]
use crate::instrumentation::InstrumentationContext;
use crate::state::{ConnectionState, InTransaction, Ready};
use crate::statement_cache::StatementCache;
use crate::stream::{MultiResultStream, QueryStream};
use crate::transaction::SavePoint;

/// How long to wait for the server to acknowledge an Attention packet after
/// a command timeout. SqlClient waits 5 seconds before dooming the
/// connection; we match it.
const ATTENTION_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Run a network future under an optional command deadline.
///
/// On timeout this sends an Attention packet via `canceller` and then awaits
/// the future so its own read loop drains the server's DONE_ATTN
/// acknowledgement, leaving the connection clean before returning
/// [`Error::CommandTimeout`]. This is the cancel-safe alternative to dropping
/// the future (e.g. via `tokio::time::timeout`), which would leave unconsumed
/// TDS data in the connection buffer and desync the next request.
///
/// The drain itself is bounded by [`ATTENTION_ACK_TIMEOUT`] — a hung server
/// that never acknowledges the attention must not turn the timeout into an
/// infinite wait. When the bound expires the connection is abandoned
/// mid-response: `in_flight` stays set, so the pool discards the connection
/// at check-in instead of reusing it.
pub(crate) async fn run_with_deadline<F, T>(
    fut: F,
    deadline: Option<std::time::Duration>,
    canceller: crate::cancel::CancelHandle,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    let Some(d) = deadline else {
        return fut.await;
    };
    tokio::pin!(fut);
    tokio::select! {
        biased;
        res = &mut fut => res,
        () = tokio::time::sleep(d) => {
            // Signal cancellation, then let the in-flight read consume the
            // server's attention acknowledgement so the connection stays usable.
            let drain = async {
                let _ = canceller.cancel().await;
                let _ = (&mut fut).await;
            };
            if tokio::time::timeout(ATTENTION_ACK_TIMEOUT, drain).await.is_err() {
                tracing::warn!(
                    timeout = ?ATTENTION_ACK_TIMEOUT,
                    "server did not acknowledge attention; abandoning the connection as dirty"
                );
            }
            Err(Error::CommandTimeout)
        }
    }
}

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

/// The parameter `TypeInfo` to declare a typed NULL ([`crate::null`]) with, from
/// its [`crate::ToSql::sql_type`] name. Returns `None` for an untyped NULL
/// (`Option::None`, type `"NULL"`), which falls back to the default param type.
#[cfg(feature = "always-encrypted")]
fn null_param_type_info(sql_type: &str) -> Option<tds_protocol::rpc::TypeInfo> {
    use tds_protocol::rpc::TypeInfo;
    Some(match sql_type {
        "BIT" => TypeInfo::bit(),
        "TINYINT" => TypeInfo::tinyint(),
        "SMALLINT" => TypeInfo::smallint(),
        "INT" => TypeInfo::int(),
        "BIGINT" => TypeInfo::bigint(),
        "REAL" => TypeInfo::real(),
        "FLOAT" => TypeInfo::float(),
        "NVARCHAR" => TypeInfo::nvarchar(1),
        "VARBINARY" => TypeInfo::varbinary(1),
        "UNIQUEIDENTIFIER" => TypeInfo::uuid(),
        "DATE" => TypeInfo::date(),
        _ => return None,
    })
}

/// Map a typed-parameter wrapper's [`EncryptedParamType`] to the `TypeInfo` the
/// driver declares it as (for `sp_describe_parameter_encryption` and the
/// `CryptoMetadata` base type). Unknown future variants error rather than
/// silently declaring the wrong type.
#[cfg(feature = "always-encrypted")]
fn encrypted_param_type_info(
    ty: mssql_types::EncryptedParamType,
) -> Result<tds_protocol::rpc::TypeInfo> {
    use mssql_types::EncryptedParamType as E;
    use tds_protocol::rpc::TypeInfo;
    Ok(match ty {
        E::Decimal { precision, scale } => TypeInfo::decimal(precision, scale),
        E::Time { scale } => TypeInfo::time(scale),
        E::DateTime2 { scale } => TypeInfo::datetime2(scale),
        E::DateTimeOffset { scale } => TypeInfo::datetimeoffset(scale),
        E::DateTime => TypeInfo::datetime(),
        E::Char { length } => TypeInfo::char(length),
        E::NChar { length } => TypeInfo::nchar(length),
        E::Binary { length } => TypeInfo::binary(length),
        _ => {
            return Err(Error::Encryption(
                "unsupported Always Encrypted parameter type".to_string(),
            ));
        }
    })
}

// Private helper methods available to all connection states
impl<S: ConnectionState> Client<S> {
    /// The default per-command deadline from `command_timeout`.
    ///
    /// Returns `None` when `command_timeout` is zero, which means "no limit"
    /// (matching ADO.NET's `SqlCommand.CommandTimeout = 0`).
    pub(crate) fn command_deadline(&self) -> Option<std::time::Duration> {
        let t = self.config.command_timeout;
        if t.is_zero() { None } else { Some(t) }
    }

    /// Build a cancel handle for the current connection, regardless of
    /// connection state. The public, documented surface is
    /// [`Client::<Ready>::cancel_handle`]; both state-specific methods
    /// delegate here.
    pub(crate) fn connection_cancel_handle(&self) -> crate::cancel::CancelHandle {
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

    /// Cancel an in-flight response that was abandoned without being drained —
    /// e.g. a [`RowStream`](crate::RowStream) dropped or cancelled mid-result.
    ///
    /// Sends an Attention and drains to the server's DONE_ATTN acknowledgement so
    /// the socket is clean and the connection reusable. A no-op when nothing is
    /// in flight. Bounded by [`ATTENTION_ACK_TIMEOUT`]: if the acknowledgement
    /// never arrives the connection is left marked in-flight (so the pool
    /// discards it on return) and an error is returned.
    pub(crate) async fn cancel_in_flight_response(&mut self) -> Result<()> {
        if !self.in_flight {
            return Ok(());
        }
        let canceller = self.connection_cancel_handle();
        let drain = async {
            canceller.cancel().await?;
            // With the cancelling flag set, `read_response_message` routes through
            // the codec's drain-after-cancel path and returns `Err(Cancelled)`
            // once the DONE_ATTN acknowledgement is consumed (clearing
            // `in_flight`). Any full messages that arrive before the ack are
            // discarded.
            loop {
                match self.read_response_message().await {
                    Err(Error::Cancelled) => return Ok(()),
                    Ok(_) => continue,
                    Err(e) => return Err(e),
                }
            }
        };
        match tokio::time::timeout(ATTENTION_ACK_TIMEOUT, drain).await {
            Ok(result) => result,
            Err(_) => {
                tracing::warn!(
                    timeout = ?ATTENTION_ACK_TIMEOUT,
                    "attention acknowledgement not received while cancelling an \
                     abandoned response; connection left dirty"
                );
                Err(Error::Cancelled)
            }
        }
    }

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

    /// Apply a transaction-related `ENVCHANGE` to this client's descriptor.
    ///
    /// Lets the streaming readers (which live in sibling modules) keep the
    /// transaction descriptor in sync with raw `BEGIN`/`COMMIT`/`ROLLBACK`
    /// batches seen mid-stream, exactly as the buffered readers do.
    pub(crate) fn apply_transaction_env_change(&mut self, env: &EnvChange) {
        Self::process_transaction_env_change(env, &mut self.transaction_descriptor);
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
        // If a previous streamed response was abandoned (a RowStream dropped
        // mid-result), drain it before issuing a new request so the next read
        // does not pick up the old response's bytes.
        self.cancel_in_flight_response().await?;

        let payload = tds_protocol::__private::encode_sql_batch_with_transaction(
            sql,
            self.transaction_descriptor,
        );
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
        // Drain an abandoned streamed response (see `send_sql_batch`) before
        // issuing this request.
        self.cancel_in_flight_response().await?;

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
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// let result = client.procedure("dbo.CalculateSum")?
    ///     .input("@a", &10i32)
    ///     .input("@b", &20i32)
    ///     .output_int("@result")
    ///     .execute().await?;
    ///
    /// let sum = result.get_output("@result").unwrap();
    /// # let _ = sum;
    /// # Ok(())
    /// # }
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
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// let result = client.call_procedure("dbo.GetUser", &[&1i32]).await?;
    /// assert_eq!(result.return_value, 0);
    ///
    /// if let Some(rs) = result.first_result_set() {
    ///     println!("columns: {:?}", rs.columns());
    /// }
    /// # Ok(())
    /// # }
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

        let deadline = self.command_deadline();
        let canceller = self.connection_cancel_handle();
        run_with_deadline(
            async {
                self.send_rpc(&rpc).await?;
                self.read_procedure_result().await
            },
            deadline,
            canceller,
        )
        .await
    }

    /// Ask the server how each parameter of a statement must be encrypted.
    ///
    /// Issues the `sp_describe_parameter_encryption` system RPC for the
    /// parameterized statement `tsql` with the parameter declaration `params`
    /// (e.g. `"@id int, @name nvarchar(64)"`), and parses the two result sets
    /// into a [`ParameterEncryptionInfo`](crate::encryption::ParameterEncryptionInfo): the
    /// CEK table, plus — for each parameter the server reports as encrypted —
    /// which CEK and whether deterministic or randomized. Parameters the server
    /// reports as plaintext are omitted.
    ///
    /// This is the first step of Always Encrypted parameter encryption; the
    /// connection must have negotiated it (`Column Encryption Setting=Enabled`).
    #[cfg(feature = "always-encrypted")]
    pub(crate) async fn describe_parameter_encryption(
        &mut self,
        tsql: &str,
        params: &str,
    ) -> Result<crate::encryption::ParameterEncryptionInfo> {
        let tsql_arg = tsql.to_string();
        let params_arg = params.to_string();
        let mut result = self
            .call_procedure(
                "sp_describe_parameter_encryption",
                &[&tsql_arg, &params_arg],
            )
            .await?;
        crate::encryption::ParameterEncryptionInfo::from_describe_result_sets(
            &mut result.result_sets,
        )
    }

    /// Build the `sp_executesql` request for a parameterized statement.
    ///
    /// When the connection has Always Encrypted enabled, parameters the server
    /// reports as encrypted are encrypted client-side first (an extra
    /// `sp_describe_parameter_encryption` round-trip). Otherwise this is the
    /// plain parameter conversion.
    pub(crate) async fn build_parameterized_rpc(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<RpcRequest> {
        #[cfg(feature = "always-encrypted")]
        if self.encryption_context.is_some() {
            return self.build_encrypted_sql_rpc(sql, params).await;
        }
        let rpc_params =
            Self::convert_params(params, self.send_unicode(), self.server_collation())?;
        Ok(RpcRequest::execute_sql(sql, rpc_params))
    }

    /// Encrypt the Always Encrypted parameters of a statement, then build its
    /// `sp_executesql` request.
    ///
    /// Asks the server which parameters are encrypted
    /// ([`describe_parameter_encryption`](Self::describe_parameter_encryption)),
    /// then for each one normalizes the value, resolves its column encryption
    /// key, encrypts, and emits an encrypted RPC parameter. Parameters the
    /// server reports as plaintext are sent unchanged.
    #[cfg(feature = "always-encrypted")]
    async fn build_encrypted_sql_rpc(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<RpcRequest> {
        use tds_protocol::rpc::RpcParam;

        let Some(ctx) = self.encryption_context.clone() else {
            let rpc_params =
                Self::convert_params(params, self.send_unicode(), self.server_collation())?;
            return Ok(RpcRequest::execute_sql(sql, rpc_params));
        };

        // Resolve each parameter's value once (AE normalization needs the typed
        // value, not the wire encoding) and build the plaintext RPC params.
        let send_unicode = self.send_unicode();
        let collation = self.server_collation().cloned();
        let mut values: Vec<mssql_types::SqlValue> = Vec::with_capacity(params.len());
        let mut plaintext: Vec<RpcParam> = Vec::with_capacity(params.len());
        let mut hints: Vec<Option<mssql_types::EncryptedParamType>> =
            Vec::with_capacity(params.len());
        for (i, p) in params.iter().enumerate() {
            let name = format!("@p{}", i + 1);
            let value = p.to_sql()?;
            let hint = p.encrypted_param_type();
            // A typed NULL (e.g. `null::<i32>()`) is declared by its SQL type so
            // describe accepts it against the target encrypted column; an untyped
            // NULL falls back to the default in `sql_value_to_rpc_param`.
            let rpc_param = match (&value, null_param_type_info(p.sql_type())) {
                (mssql_types::SqlValue::Null, Some(type_info)) => RpcParam::null(&name, type_info),
                _ => {
                    let mut param = Self::sql_value_to_rpc_param(
                        &name,
                        &value,
                        send_unicode,
                        collation.as_ref(),
                    )?;
                    // A typed-parameter wrapper (e.g. `numeric(v, p, s)`,
                    // `datetime2(v, scale)`) declares an explicit SQL type so
                    // describe matches the encrypted column exactly — the value
                    // alone cannot convey precision/scale or the legacy-`datetime`
                    // vs `datetime2` distinction.
                    if let Some(ty) = hint {
                        param.type_info = encrypted_param_type_info(ty)?;
                    }
                    param
                }
            };
            plaintext.push(rpc_param);
            values.push(value);
            hints.push(hint);
        }

        if plaintext.is_empty() {
            return Ok(RpcRequest::execute_sql(sql, plaintext));
        }

        // Ask the server which parameters need encryption.
        let declarations = RpcRequest::build_param_declarations(&plaintext);
        let info = self
            .describe_parameter_encryption(sql, &declarations)
            .await?;
        if info.parameters.is_empty() {
            return Ok(RpcRequest::execute_sql(sql, plaintext));
        }

        // Encrypt the flagged parameters; pass the rest through untouched.
        let mut final_params: Vec<RpcParam> = Vec::with_capacity(plaintext.len());
        for ((value, param), hint) in values.into_iter().zip(plaintext).zip(hints) {
            let Some(crypto) = info.get_parameter(&param.name) else {
                final_params.push(param);
                continue;
            };
            let entry = info.cek_table.get(crypto.cek_ordinal).ok_or_else(|| {
                Error::Protocol(format!(
                    "encrypted parameter {} references missing CEK ordinal {}",
                    param.name, crypto.cek_ordinal
                ))
            })?;
            let metadata = tds_protocol::rpc::EncryptedParamMetadata {
                base_type_info: param.type_info.clone(),
                algorithm_id: crypto.algorithm_id,
                encryption_type: crypto.encryption_type,
                database_id: entry.database_id,
                cek_id: entry.cek_id,
                cek_version: entry.cek_version,
                cek_md_version: entry.cek_md_version,
                normalization_rule_version: crypto.normalization_rule_version,
            };
            // A NULL value bound to an encrypted column is sent as an encrypted
            // NULL (the server rejects a plaintext parameter for an encrypted
            // column); there is nothing to encrypt.
            if matches!(value, mssql_types::SqlValue::Null) {
                final_params.push(RpcParam::encrypted_null(param.name, metadata));
                continue;
            }
            let normalized = crate::encryption::normalize_for_encryption(&value, hint)?;
            let ciphertext = ctx
                .encrypt_value(&normalized, entry, crypto.encryption_type)
                .await?;
            final_params.push(RpcParam::encrypted(
                param.name,
                bytes::Bytes::from(ciphertext),
                metadata,
            ));
        }

        Ok(RpcRequest::execute_sql(sql, final_params))
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
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// use mssql_client::{BulkInsertBuilder, BulkColumn, SqlValue};
    ///
    /// let builder = BulkInsertBuilder::new("dbo.Users")
    ///     .with_typed_columns(vec![
    ///         BulkColumn::new("id", "INT", 0)?,
    ///         BulkColumn::new("name", "NVARCHAR(100)", 1)?,
    ///     ]);
    ///
    /// let mut writer = client.bulk_insert(&builder).await?;
    /// writer.send_row_values(&[SqlValue::Int(1), SqlValue::String("Alice".into())])?;
    /// writer.send_row_values(&[SqlValue::Int(2), SqlValue::String("Bob".into())])?;
    /// let result = writer.finish().await?;
    /// println!("Inserted {} rows", result.rows_affected);
    /// # Ok(())
    /// # }
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
        let deadline = self.command_deadline();
        let canceller = self.connection_cancel_handle();
        let message = run_with_deadline(
            async {
                self.send_sql_batch(&meta_query).await?;
                self.read_response_message().await
            },
            deadline,
            canceller,
        )
        .await?;
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
        let deadline = self.command_deadline();
        let canceller = self.connection_cancel_handle();
        run_with_deadline(
            async {
                self.send_sql_batch(&stmt).await?;
                self.read_execute_result().await
            },
            deadline,
            canceller,
        )
        .await?;

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
        let deadline = self.command_deadline();
        let canceller = self.connection_cancel_handle();
        run_with_deadline(
            async {
                self.send_sql_batch(&stmt).await?;
                self.read_execute_result().await
            },
            deadline,
            canceller,
        )
        .await?;

        // Create bulk writer with hand-crafted metadata
        let bulk =
            crate::bulk::BulkInsert::new(builder.columns().to_vec(), builder.options().batch_size);

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
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// use mssql_client::{NamedParam, ToParams};
    ///
    /// // With derive macro:
    /// #[derive(mssql_derive::ToParams)]
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
    /// # let _ = rows;
    /// # Ok(())
    /// # }
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
            let rpc_params =
                Self::convert_named_params(params, self.send_unicode(), self.server_collation())?;
            let rpc = RpcRequest::execute_sql(sql, rpc_params);
            self.send_rpc(&rpc).await?;
        }

        let resp = self.read_query_response().await?;
        #[cfg(feature = "always-encrypted")]
        {
            Ok(QueryStream::from_raw(
                resp.columns,
                resp.pending_rows,
                resp.meta,
                resp.decryptor,
            ))
        }
        #[cfg(not(feature = "always-encrypted"))]
        {
            Ok(QueryStream::from_raw(
                resp.columns,
                resp.pending_rows,
                resp.meta,
            ))
        }
    }

    /// Execute a statement with named parameters.
    ///
    /// Returns the number of affected rows. This is the named-parameter
    /// counterpart of [`execute()`](Client::execute), compatible with the
    /// [`ToParams`](crate::to_params::ToParams) trait.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
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
    /// # let _ = rows_affected;
    /// # Ok(())
    /// # }
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

        let deadline = self.command_deadline();
        let canceller = self.connection_cancel_handle();
        run_with_deadline(
            async {
                if params.is_empty() {
                    self.send_sql_batch(sql).await?;
                } else {
                    let rpc_params = Self::convert_named_params(
                        params,
                        self.send_unicode(),
                        self.server_collation(),
                    )?;
                    let rpc = RpcRequest::execute_sql(sql, rpc_params);
                    self.send_rpc(&rpc).await?;
                }

                self.read_execute_result().await
            },
            deadline,
            canceller,
        )
        .await
    }

    /// Whether string parameters are sent as NVARCHAR (Unicode).
    pub(crate) fn send_unicode(&self) -> bool {
        self.config.send_string_parameters_as_unicode
    }

    /// Server's default collation, captured from ENVCHANGE during login.
    pub(crate) fn server_collation(&self) -> Option<&tds_protocol::token::Collation> {
        self.server_collation.as_ref()
    }

    /// Shared implementation behind `query_stream` for both `Ready` and
    /// `InTransaction`. Sends the request, then pulls packets until the first
    /// result set's `ColMetaData` (resolving columns and any Always Encrypted
    /// decryptor up front) before handing back a [`RowStream`].
    pub(crate) async fn query_stream_inner<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::row_stream::RowStream<'a, S>> {
        use crate::client::response::server_token_to_error;
        use crate::row_source::{Pull, RowSource};
        use tds_protocol::token::Token;

        tracing::debug!(sql = sql, params_count = params.len(), "streaming query");

        // Send the request (same wire format as the buffered path).
        if params.is_empty() {
            self.send_sql_batch(sql).await?;
        } else {
            let rpc = self.build_parameterized_rpc(sql, params).await?;
            self.send_rpc(&rpc).await?;
        }
        self.in_flight = true;

        #[cfg(feature = "always-encrypted")]
        let encryption_enabled = self.encryption_context.is_some();
        #[cfg(not(feature = "always-encrypted"))]
        let encryption_enabled = false;

        let mut source = RowSource::new(encryption_enabled);

        // Prelude: pull packets until the first result set's ColMetaData (so the
        // columns and any Always Encrypted decryptor are resolved up front), or
        // until a terminal Done/Error if there is no result set.
        loop {
            match source.pull()? {
                Pull::Token(Token::ColMetaData(meta)) => {
                    let columns = Self::build_columns(&meta);
                    #[cfg(feature = "always-encrypted")]
                    let decryptor = self
                        .resolve_decryptor(&meta)
                        .await?
                        .map(std::sync::Arc::new);
                    return Ok(crate::row_stream::RowStream::new(
                        self,
                        source,
                        columns,
                        meta,
                        #[cfg(feature = "always-encrypted")]
                        decryptor,
                    ));
                }
                Pull::Token(Token::Error(err)) => {
                    self.in_flight = false;
                    return Err(server_token_to_error(&err));
                }
                Pull::Token(Token::Done(done)) => {
                    if done.status.error {
                        self.in_flight = false;
                        return Err(Error::Query(
                            "query failed (server set error flag in DONE token)".to_string(),
                        ));
                    }
                    if !done.status.more {
                        // No result set (e.g. an INSERT) — an empty stream.
                        self.in_flight = false;
                        return Ok(crate::row_stream::RowStream::empty(self));
                    }
                    // More results may follow; keep looking for ColMetaData.
                }
                Pull::Token(Token::EnvChange(env)) => {
                    Self::process_transaction_env_change(&env, &mut self.transaction_descriptor);
                }
                Pull::Token(_) => {
                    // Info / Order / DoneProc / DoneInProc, etc. — keep pulling.
                }
                Pull::NeedMore => match self.read_response_packet().await? {
                    Some((payload, is_eom)) => source.push_packet(payload, is_eom),
                    None => {
                        self.in_flight = false;
                        return Err(Error::ConnectionClosed);
                    }
                },
                Pull::End => {
                    self.in_flight = false;
                    return Ok(crate::row_stream::RowStream::empty(self));
                }
            }
        }
    }

    /// Shared implementation behind `query_stream_blob` for both `Ready` and
    /// `InTransaction`.
    pub(crate) async fn query_stream_blob_inner<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::blob_stream::BlobStream<'a, S>> {
        use crate::client::response::server_token_to_error;
        use crate::row_source::{Pull, RowSource};
        use tds_protocol::token::Token;

        if params.is_empty() {
            self.send_sql_batch(sql).await?;
        } else {
            let rpc = self.build_parameterized_rpc(sql, params).await?;
            self.send_rpc(&rpc).await?;
        }
        self.in_flight = true;

        #[cfg(feature = "always-encrypted")]
        let encryption_enabled = self.encryption_context.is_some();
        #[cfg(not(feature = "always-encrypted"))]
        let encryption_enabled = false;

        let mut source = RowSource::new(encryption_enabled);

        loop {
            match source.pull()? {
                Pull::Token(Token::ColMetaData(meta)) => {
                    let blob_index = Self::validate_blob_result_set(&meta)?;
                    let (buf, eom) = source.into_parts();
                    return Ok(crate::blob_stream::BlobStream::new(
                        self,
                        buf,
                        eom,
                        encryption_enabled,
                        meta,
                        blob_index,
                    ));
                }
                Pull::Token(Token::Error(err)) => {
                    self.in_flight = false;
                    return Err(server_token_to_error(&err));
                }
                Pull::Token(Token::Done(_)) => {
                    self.in_flight = false;
                    return Err(Error::Protocol(
                        "query_stream_blob: query produced no result set".to_string(),
                    ));
                }
                Pull::Token(_) => {}
                Pull::NeedMore => match self.read_response_packet().await? {
                    Some((payload, is_eom)) => source.push_packet(payload, is_eom),
                    None => {
                        self.in_flight = false;
                        return Err(Error::ConnectionClosed);
                    }
                },
                Pull::End => {
                    self.in_flight = false;
                    return Err(Error::Protocol(
                        "query_stream_blob: query produced no result set".to_string(),
                    ));
                }
            }
        }
    }

    /// Validate that a result set is shaped for [`query_stream_blob`] and return
    /// the index of its single trailing MAX column.
    fn validate_blob_result_set(meta: &tds_protocol::token::ColMetaData) -> Result<usize> {
        if meta.cek_table.is_some() {
            return Err(Error::Protocol(
                "query_stream_blob does not support Always Encrypted result sets".to_string(),
            ));
        }
        let max_cols: Vec<usize> = meta
            .columns
            .iter()
            .enumerate()
            .filter(|(_, c)| crate::blob_stream::is_plp_max(c))
            .map(|(i, _)| i)
            .collect();
        match max_cols.as_slice() {
            [] => Err(Error::Protocol(
                "query_stream_blob: result set has no MAX column — use query_stream".to_string(),
            )),
            [idx] if *idx == meta.columns.len() - 1 => Ok(*idx),
            [_] => Err(Error::Protocol(
                "query_stream_blob: the MAX column must be the last column".to_string(),
            )),
            _ => Err(Error::Protocol(
                "query_stream_blob: result set has more than one MAX column".to_string(),
            )),
        }
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

    /// Execute a query and return a result set with lazy per-row decoding.
    ///
    /// Per ADR-007 the full response is buffered in memory and each row is
    /// *decoded* on demand as you iterate — this is not incremental network
    /// streaming, so peak memory tracks the response size. Use
    /// `.collect_all()` if you want all rows materialized into a `Vec` up
    /// front.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use mssql_client::Row;
    /// # fn process(_: &Row) {}
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// // Streaming (synchronous iteration over the result set)
    /// let stream = client.query("SELECT * FROM users WHERE id = @p1", &[&1]).await?;
    /// for row in stream {
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
    /// # let _ = rows;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<QueryStream<'a>> {
        let deadline = self.command_deadline();
        self.query_inner(sql, params, deadline).await
    }

    /// Shared query implementation with an explicit command deadline.
    async fn query_inner<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        deadline: Option<std::time::Duration>,
    ) -> Result<QueryStream<'a>> {
        tracing::debug!(sql = sql, params_count = params.len(), "executing query");

        #[cfg(feature = "otel")]
        let instrumentation = self.instrumentation.clone();
        #[cfg(feature = "otel")]
        let mut span = instrumentation.query_span(sql);
        #[cfg(feature = "otel")]
        let timer = crate::instrumentation::OperationTimer::start(
            crate::instrumentation::extract_operation(sql),
        );

        let canceller = self.cancel_handle();
        let result = run_with_deadline(
            async {
                if params.is_empty() {
                    // Simple query without parameters - use SQL batch
                    self.send_sql_batch(sql).await?;
                } else {
                    // Parameterized query - sp_executesql (encrypts Always Encrypted params).
                    let rpc = self.build_parameterized_rpc(sql, params).await?;
                    self.send_rpc(&rpc).await?;
                }

                // Read complete response including columns and rows
                self.read_query_response().await
            },
            deadline,
            canceller,
        )
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }
        #[cfg(feature = "otel")]
        timer.finish(instrumentation.metrics(), result.is_ok());

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        let resp = result?;
        #[cfg(feature = "always-encrypted")]
        {
            Ok(QueryStream::from_raw(
                resp.columns,
                resp.pending_rows,
                resp.meta,
                resp.decryptor,
            ))
        }
        #[cfg(not(feature = "always-encrypted"))]
        {
            Ok(QueryStream::from_raw(
                resp.columns,
                resp.pending_rows,
                resp.meta,
            ))
        }
    }

    /// Execute a query and stream rows incrementally from the network.
    ///
    /// Unlike [`query`](Self::query) — which buffers the whole response in
    /// memory before returning — this reads TDS packets on demand as rows are
    /// pulled, so peak memory is roughly one packet plus one row regardless of
    /// result-set size. Use it for large result sets; use [`query`](Self::query)
    /// for the common small-result case where the buffered, synchronously
    /// iterable [`QueryStream`] is more convenient.
    ///
    /// The returned [`RowStream`](crate::RowStream) borrows the client for its
    /// lifetime, so no other request can run on this connection until the stream
    /// is consumed or dropped. Also available on `Client<InTransaction>` to
    /// stream within a transaction.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// let mut stream = client.query_stream("SELECT id FROM big_table", &[]).await?;
    /// while let Some(row) = stream.try_next().await? {
    ///     let id: i32 = row.get_by_name("id")?;
    ///     let _ = id;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_stream<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::row_stream::RowStream<'a, Ready>> {
        self.query_stream_inner(sql, params).await
    }

    /// Execute a query and stream a row's trailing MAX column from the network.
    ///
    /// For result sets whose last column is a single MAX type
    /// (`VARBINARY(MAX)`, `NVARCHAR(MAX)`, `VARCHAR(MAX)`, `XML`), this reads
    /// that column's bytes incrementally from the socket instead of
    /// materializing the cell — so a multi-GB BLOB can be streamed to a sink in
    /// bounded memory. The leading (scalar) columns are decoded eagerly into the
    /// per-row [`Row`](crate::Row).
    ///
    /// The MAX column must be the **last** column. The returned
    /// [`BlobStream`](crate::BlobStream) yields scalar [`Row`](crate::Row)s via
    /// [`next`](crate::BlobStream::next); read each row's blob with
    /// [`read_chunk`](crate::BlobStream::read_chunk) /
    /// [`copy_blob_to`](crate::BlobStream::copy_blob_to) before advancing. Also
    /// available on `Client<InTransaction>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the result set has no trailing MAX column, has more
    /// than one MAX column, the MAX column is not last, or the result set uses
    /// Always Encrypted (not yet supported on this path).
    pub async fn query_stream_blob<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::blob_stream::BlobStream<'a, Ready>> {
        self.query_stream_blob_inner(sql, params).await
    }

    /// Execute a query with a specific timeout.
    ///
    /// This overrides the default `command_timeout` from the connection configuration
    /// for this specific query. If the query does not complete within the specified
    /// duration, the driver sends an Attention packet to cancel it server-side,
    /// drains the acknowledgement, and returns [`Error::CommandTimeout`] with the
    /// connection left usable for the next request.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    /// * `params` - Query parameters
    /// * `timeout_duration` - Maximum time to wait for the query to complete
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
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
    /// # let _ = rows;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_with_timeout<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<QueryStream<'a>> {
        self.query_inner(sql, params, Some(timeout_duration)).await
    }

    /// Execute a batch that may return multiple result sets.
    ///
    /// This is useful for stored procedures or SQL batches that contain
    /// multiple SELECT statements.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// // Execute a batch with multiple SELECT statements
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
    /// # Ok(())
    /// # }
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

        let deadline = self.command_deadline();
        let canceller = self.connection_cancel_handle();
        let result_sets = run_with_deadline(
            async {
                if params.is_empty() {
                    // Simple batch without parameters - use SQL batch
                    self.send_sql_batch(sql).await?;
                } else {
                    // Parameterized query - sp_executesql (encrypts Always Encrypted params).
                    let rpc = self.build_parameterized_rpc(sql, params).await?;
                    self.send_rpc(&rpc).await?;
                }

                // Read all result sets
                self.read_multi_result_response().await
            },
            deadline,
            canceller,
        )
        .await?;
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
        let deadline = self.command_deadline();
        self.execute_inner(sql, params, deadline).await
    }

    /// Shared execute implementation with an explicit command deadline.
    async fn execute_inner(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        deadline: Option<std::time::Duration>,
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
        #[cfg(feature = "otel")]
        let timer = crate::instrumentation::OperationTimer::start(
            crate::instrumentation::extract_operation(sql),
        );

        let canceller = self.cancel_handle();
        let result = run_with_deadline(
            async {
                if params.is_empty() {
                    // Simple statement without parameters - use SQL batch
                    self.send_sql_batch(sql).await?;
                } else {
                    // Parameterized statement - sp_executesql (encrypts Always Encrypted params).
                    let rpc = self.build_parameterized_rpc(sql, params).await?;
                    self.send_rpc(&rpc).await?;
                }

                // Read response and get row count
                self.read_execute_result().await
            },
            deadline,
            canceller,
        )
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(rows) => InstrumentationContext::record_success(&mut span, Some(*rows)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }
        #[cfg(feature = "otel")]
        timer.finish(instrumentation.metrics(), result.is_ok());

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        result
    }

    /// Execute a statement with a specific timeout.
    ///
    /// This overrides the default `command_timeout` from the connection configuration
    /// for this specific statement. If the statement does not complete within the
    /// specified duration, the driver sends an Attention packet to cancel it
    /// server-side, drains the acknowledgement, and returns
    /// [`Error::CommandTimeout`] with the connection left usable.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL statement to execute
    /// * `params` - Statement parameters
    /// * `timeout_duration` - Maximum time to wait for the statement to complete
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
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
    /// # let _ = rows_affected;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_with_timeout(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        timeout_duration: std::time::Duration,
    ) -> Result<u64> {
        self.execute_inner(sql, params, Some(timeout_duration))
            .await
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
    /// ```rust,no_run
    /// # async fn ex(client: mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// use mssql_client::IsolationLevel;
    ///
    /// let tx = client.begin_transaction_with_isolation(IsolationLevel::Serializable).await?;
    /// // All operations in this transaction use SERIALIZABLE isolation
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
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
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// client.execute("BEGIN TRANSACTION", &[]).await?;
    /// assert!(client.is_in_transaction());
    ///
    /// client.execute("COMMIT", &[]).await?;
    /// assert!(!client.is_in_transaction());
    /// # Ok(())
    /// # }
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
    /// ```rust,no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
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
    /// # let _ = (handle, result);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn cancel_handle(&self) -> crate::cancel::CancelHandle {
        self.connection_cancel_handle()
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
/// ```rust,no_run
/// # async fn do_work(_: &mssql_client::Client<mssql_client::InTransaction>) -> Result<(), mssql_client::Error> { Ok(()) }
/// # async fn ex(client: mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
/// let tx = client.begin_transaction().await?;
/// match do_work(&tx).await {
///     Ok(_) => { tx.commit().await?; }
///     Err(e) => { tx.rollback().await?; return Err(e); }
/// }
/// # Ok(())
/// # }
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
        let deadline = self.command_deadline();
        self.query_inner(sql, params, deadline).await
    }

    /// Shared query implementation with an explicit command deadline.
    async fn query_inner<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        deadline: Option<std::time::Duration>,
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
        #[cfg(feature = "otel")]
        let timer = crate::instrumentation::OperationTimer::start(
            crate::instrumentation::extract_operation(sql),
        );

        let canceller = self.cancel_handle();
        let result = run_with_deadline(
            async {
                if params.is_empty() {
                    // Simple query without parameters - use SQL batch
                    self.send_sql_batch(sql).await?;
                } else {
                    // Parameterized query - sp_executesql (encrypts Always Encrypted params).
                    let rpc = self.build_parameterized_rpc(sql, params).await?;
                    self.send_rpc(&rpc).await?;
                }

                // Read complete response including columns and rows
                self.read_query_response().await
            },
            deadline,
            canceller,
        )
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(_) => InstrumentationContext::record_success(&mut span, None),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }
        #[cfg(feature = "otel")]
        timer.finish(instrumentation.metrics(), result.is_ok());

        // Drop the span before returning
        #[cfg(feature = "otel")]
        drop(span);

        let resp = result?;
        #[cfg(feature = "always-encrypted")]
        {
            Ok(QueryStream::from_raw(
                resp.columns,
                resp.pending_rows,
                resp.meta,
                resp.decryptor,
            ))
        }
        #[cfg(not(feature = "always-encrypted"))]
        {
            Ok(QueryStream::from_raw(
                resp.columns,
                resp.pending_rows,
                resp.meta,
            ))
        }
    }

    /// Stream rows incrementally from the network within the transaction.
    ///
    /// Identical to [`Client<Ready>::query_stream`] except the query runs inside
    /// the open transaction. The returned [`RowStream`](crate::RowStream)
    /// borrows the transaction client for its lifetime, so the stream must be
    /// consumed or dropped before the transaction can be committed or rolled
    /// back.
    pub async fn query_stream<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::row_stream::RowStream<'a, InTransaction>> {
        self.query_stream_inner(sql, params).await
    }

    /// Stream a row's trailing MAX column from the network within the
    /// transaction.
    ///
    /// See [`Client<Ready>::query_stream_blob`] for semantics and constraints;
    /// the only difference is that the query runs inside the open transaction.
    pub async fn query_stream_blob<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<crate::blob_stream::BlobStream<'a, InTransaction>> {
        self.query_stream_blob_inner(sql, params).await
    }

    /// Execute a statement within the transaction.
    ///
    /// Returns the number of affected rows.
    pub async fn execute(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<u64> {
        let deadline = self.command_deadline();
        self.execute_inner(sql, params, deadline).await
    }

    /// Shared execute implementation with an explicit command deadline.
    async fn execute_inner(
        &mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
        deadline: Option<std::time::Duration>,
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
        #[cfg(feature = "otel")]
        let timer = crate::instrumentation::OperationTimer::start(
            crate::instrumentation::extract_operation(sql),
        );

        let canceller = self.cancel_handle();
        let result = run_with_deadline(
            async {
                if params.is_empty() {
                    // Simple statement without parameters - use SQL batch
                    self.send_sql_batch(sql).await?;
                } else {
                    // Parameterized statement - sp_executesql (encrypts Always Encrypted params).
                    let rpc = self.build_parameterized_rpc(sql, params).await?;
                    self.send_rpc(&rpc).await?;
                }

                // Read response and get row count
                self.read_execute_result().await
            },
            deadline,
            canceller,
        )
        .await;

        #[cfg(feature = "otel")]
        match &result {
            Ok(rows) => InstrumentationContext::record_success(&mut span, Some(*rows)),
            Err(e) => InstrumentationContext::record_error(&mut span, e),
        }
        #[cfg(feature = "otel")]
        timer.finish(instrumentation.metrics(), result.is_ok());

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
        self.query_inner(sql, params, Some(timeout_duration)).await
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
        self.execute_inner(sql, params, Some(timeout_duration))
            .await
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
    /// ```text
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
    /// ```rust,no_run
    /// # async fn ex(client: mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// let mut tx = client.begin_transaction().await?;
    /// tx.execute("INSERT INTO orders ...", &[]).await?;
    /// let sp = tx.save_point("before_items").await?;
    /// tx.execute("INSERT INTO items ...", &[]).await?;
    /// // Oops, rollback just the items
    /// tx.rollback_to(&sp).await?;
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
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
    /// ```rust,no_run
    /// # async fn ex(mut tx: mssql_client::Client<mssql_client::InTransaction>) -> Result<(), mssql_client::Error> {
    /// let sp = tx.save_point("checkpoint").await?;
    /// // ... do some work ...
    /// tx.rollback_to(&sp).await?;  // Undo changes since checkpoint
    /// // Transaction is still active, savepoint is still valid
    /// # Ok(())
    /// # }
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
        self.connection_cancel_handle()
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
