//! SQL Server client implementation.

use std::marker::PhantomData;
use std::sync::Arc;

use bytes::BytesMut;
use mssql_codec::connection::Connection;
use mssql_tls::{TlsConfig, TlsConnector, TlsNegotiationMode, TlsStream};
use tds_protocol::login7::Login7;
use tds_protocol::packet::{PacketType, MAX_PACKET_SIZE};
use tds_protocol::prelogin::{EncryptionLevel, PreLogin};
use tds_protocol::rpc::{RpcParam, RpcRequest, TypeInfo as RpcTypeInfo};
use tds_protocol::token::{
    ColMetaData, ColumnData, EnvChange, EnvChangeType, NbcRow, RawRow, Token, TokenParser,
};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::state::{ConnectionState, Disconnected, InTransaction, Ready};
use crate::statement_cache::StatementCache;
use crate::stream::QueryStream;
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
}

/// Internal connection handle wrapping the actual connection.
///
/// This is an enum to support both TLS and non-TLS connections,
/// though in practice TLS is almost always required for SQL Server.
#[allow(dead_code)]  // Connection will be used once query execution is implemented
enum ConnectionHandle {
    /// TLS connection
    Tls(Connection<TlsStream<TcpStream>>),
    /// Plain TCP connection (rare, for testing or internal networks)
    Plain(Connection<TcpStream>),
}

impl Client<Disconnected> {
    /// Connect to SQL Server.
    ///
    /// This establishes a connection, performs TLS negotiation (if required),
    /// and authenticates with the server.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client = Client::connect(config).await?;
    /// ```
    pub async fn connect(config: Config) -> Result<Client<Ready>> {
        let max_redirects = config.redirect.max_redirects;
        let follow_redirects = config.redirect.follow_redirects;
        let mut attempts = 0;
        let mut current_config = config;

        loop {
            attempts += 1;
            if attempts > max_redirects + 1 {
                return Err(Error::TooManyRedirects { max: max_redirects });
            }

            match Self::try_connect(&current_config).await {
                Ok(client) => return Ok(client),
                Err(Error::Routing { host, port }) => {
                    if !follow_redirects {
                        return Err(Error::Routing { host, port });
                    }
                    tracing::info!(
                        host = %host,
                        port = port,
                        attempt = attempts,
                        max_redirects = max_redirects,
                        "following Azure SQL routing redirect"
                    );
                    current_config = current_config.with_host(&host).with_port(port);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn try_connect(config: &Config) -> Result<Client<Ready>> {
        tracing::info!(
            host = %config.host,
            port = config.port,
            database = ?config.database,
            "connecting to SQL Server"
        );

        let addr = format!("{}:{}", config.host, config.port);

        // Step 1: Establish TCP connection
        tracing::debug!("establishing TCP connection to {}", addr);
        let tcp_stream = timeout(config.timeouts.connect_timeout, TcpStream::connect(&addr))
            .await
            .map_err(|_| Error::ConnectTimeout)?
            .map_err(|e| Error::Io(Arc::new(e)))?;

        // Enable TCP nodelay for better latency
        tcp_stream.set_nodelay(true).map_err(|e| Error::Io(Arc::new(e)))?;

        // Determine TLS negotiation mode
        let tls_mode = TlsNegotiationMode::from_encrypt_mode(config.strict_mode);

        // Step 2: Handle TDS 8.0 strict mode (TLS before any TDS traffic)
        if tls_mode.is_tls_first() {
            return Self::connect_tds_8(config, tcp_stream).await;
        }

        // Step 3: TDS 7.x flow - PreLogin first, then TLS, then Login7
        Self::connect_tds_7x(config, tcp_stream).await
    }

    /// Connect using TDS 8.0 strict mode.
    ///
    /// Flow: TCP -> TLS -> PreLogin (encrypted) -> Login7 (encrypted)
    async fn connect_tds_8(config: &Config, tcp_stream: TcpStream) -> Result<Client<Ready>> {
        tracing::debug!("using TDS 8.0 strict mode (TLS first)");

        // Build TLS configuration
        let tls_config = TlsConfig::new()
            .strict_mode(true)
            .trust_server_certificate(config.trust_server_certificate);

        let tls_connector = TlsConnector::new(tls_config)
            .map_err(|e| Error::Tls(e.to_string()))?;

        // Perform TLS handshake before any TDS traffic
        let tls_stream = timeout(
            config.timeouts.tls_timeout,
            tls_connector.connect(tcp_stream, &config.host),
        )
        .await
        .map_err(|_| Error::TlsTimeout)?
        .map_err(|e| Error::Tls(e.to_string()))?;

        tracing::debug!("TLS handshake completed (strict mode)");

        // Create connection wrapper
        let mut connection = Connection::new(tls_stream);

        // Send PreLogin (encrypted in strict mode)
        let prelogin = Self::build_prelogin(config, EncryptionLevel::Required);
        Self::send_prelogin(&mut connection, &prelogin).await?;
        let _prelogin_response = Self::receive_prelogin(&mut connection).await?;

        // Send Login7
        let login = Self::build_login7(config);
        Self::send_login7(&mut connection, &login).await?;

        // Process login response
        let (server_version, current_database, routing) =
            Self::process_login_response(&mut connection).await?;

        // Handle routing redirect
        if let Some((host, port)) = routing {
            return Err(Error::Routing { host, port });
        }

        Ok(Client {
            config: config.clone(),
            _state: PhantomData,
            connection: Some(ConnectionHandle::Tls(connection)),
            server_version,
            current_database,
            statement_cache: StatementCache::with_default_size(),
        })
    }

    /// Connect using TDS 7.x flow.
    ///
    /// Flow: TCP -> PreLogin (clear) -> TLS -> Login7 (encrypted)
    ///
    /// Note: For TDS 7.x, the PreLogin exchange happens over raw TCP before
    /// upgrading to TLS. We use low-level I/O for this initial exchange
    /// since the Connection struct splits the stream immediately.
    async fn connect_tds_7x(config: &Config, mut tcp_stream: TcpStream) -> Result<Client<Ready>> {
        use bytes::BufMut;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tds_protocol::packet::{PacketHeader, PacketStatus, PACKET_HEADER_SIZE};

        tracing::debug!("using TDS 7.x flow (PreLogin first)");

        // Build PreLogin packet
        let prelogin = Self::build_prelogin(config, EncryptionLevel::On);
        let prelogin_bytes = prelogin.encode();

        // Manually create and send the PreLogin packet over raw TCP
        let header = PacketHeader::new(
            PacketType::PreLogin,
            PacketStatus::END_OF_MESSAGE,
            (PACKET_HEADER_SIZE + prelogin_bytes.len()) as u16,
        );

        let mut packet_buf = BytesMut::with_capacity(PACKET_HEADER_SIZE + prelogin_bytes.len());
        header.encode(&mut packet_buf);
        packet_buf.put_slice(&prelogin_bytes);

        tcp_stream
            .write_all(&packet_buf)
            .await
            .map_err(|e| Error::Io(Arc::new(e)))?;

        // Read PreLogin response
        let mut header_buf = [0u8; PACKET_HEADER_SIZE];
        tcp_stream
            .read_exact(&mut header_buf)
            .await
            .map_err(|e| Error::Io(Arc::new(e)))?;

        let response_length = u16::from_be_bytes([header_buf[2], header_buf[3]]) as usize;
        let payload_length = response_length.saturating_sub(PACKET_HEADER_SIZE);

        let mut response_buf = vec![0u8; payload_length];
        tcp_stream
            .read_exact(&mut response_buf)
            .await
            .map_err(|e| Error::Io(Arc::new(e)))?;

        let prelogin_response =
            PreLogin::decode(&response_buf[..]).map_err(|e| Error::Protocol(e.to_string()))?;

        // Check server encryption response
        let server_encryption = prelogin_response.encryption;
        tracing::debug!(encryption = ?server_encryption, "server encryption level");

        // Now upgrade to TLS
        let tls_config = TlsConfig::new()
            .trust_server_certificate(config.trust_server_certificate);

        let tls_connector = TlsConnector::new(tls_config)
            .map_err(|e| Error::Tls(e.to_string()))?;

        let tls_stream = timeout(
            config.timeouts.tls_timeout,
            tls_connector.connect(tcp_stream, &config.host),
        )
        .await
        .map_err(|_| Error::TlsTimeout)?
        .map_err(|e| Error::Tls(e.to_string()))?;

        tracing::debug!("TLS handshake completed");

        // Create connection with TLS stream
        let mut connection = Connection::new(tls_stream);

        // Send Login7
        let login = Self::build_login7(config);
        Self::send_login7(&mut connection, &login).await?;

        // Process login response
        let (server_version, current_database, routing) =
            Self::process_login_response(&mut connection).await?;

        // Handle routing redirect
        if let Some((host, port)) = routing {
            return Err(Error::Routing { host, port });
        }

        Ok(Client {
            config: config.clone(),
            _state: PhantomData,
            connection: Some(ConnectionHandle::Tls(connection)),
            server_version,
            current_database,
            statement_cache: StatementCache::with_default_size(),
        })
    }

    /// Build a PreLogin packet.
    fn build_prelogin(config: &Config, encryption: EncryptionLevel) -> PreLogin {
        let mut prelogin = PreLogin::new().with_encryption(encryption);

        if config.mars {
            prelogin = prelogin.with_mars(true);
        }

        if let Some(ref instance) = config.instance {
            prelogin = prelogin.with_instance(instance);
        }

        prelogin
    }

    /// Build a Login7 packet.
    fn build_login7(config: &Config) -> Login7 {
        let mut login = Login7::new()
            .with_packet_size(config.packet_size as u32)
            .with_app_name(&config.application_name)
            .with_server_name(&config.host)
            .with_hostname(&config.host);

        if let Some(ref database) = config.database {
            login = login.with_database(database);
        }

        // Set credentials
        match &config.credentials {
            mssql_auth::Credentials::SqlServer { username, password } => {
                login = login.with_sql_auth(username.as_ref(), password.as_ref());
            }
            // Other credential types would be handled here
            _ => {}
        }

        login
    }

    /// Send a PreLogin packet (for use with Connection).
    async fn send_prelogin<T>(connection: &mut Connection<T>, prelogin: &PreLogin) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = prelogin.encode();
        let max_packet = MAX_PACKET_SIZE;

        connection
            .send_message(PacketType::PreLogin, payload, max_packet)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))
    }

    /// Receive a PreLogin response (for use with Connection).
    async fn receive_prelogin<T>(connection: &mut Connection<T>) -> Result<PreLogin>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let message = connection
            .read_message()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?
            .ok_or_else(|| Error::ConnectionClosed)?;

        PreLogin::decode(&message.payload[..]).map_err(|e| Error::Protocol(e.to_string()))
    }

    /// Send a Login7 packet.
    async fn send_login7<T>(connection: &mut Connection<T>, login: &Login7) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = login.encode();
        let max_packet = MAX_PACKET_SIZE;

        connection
            .send_message(PacketType::Tds7Login, payload, max_packet)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))
    }

    /// Process the login response tokens.
    ///
    /// Returns: (server_version, database, routing_info)
    async fn process_login_response<T>(
        connection: &mut Connection<T>,
    ) -> Result<(Option<u32>, Option<String>, Option<(String, u16)>)>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let message = connection
            .read_message()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?
            .ok_or_else(|| Error::ConnectionClosed)?;

        let response_bytes = message.payload;

        let mut parser = TokenParser::new(response_bytes);
        let mut server_version = None;
        let mut database = None;
        let mut routing = None;

        while let Some(token) = parser.next_token().map_err(|e| Error::Protocol(e.to_string()))? {
            match token {
                Token::LoginAck(ack) => {
                    tracing::info!(
                        version = ack.tds_version,
                        interface = ack.interface,
                        prog_name = %ack.prog_name,
                        "login acknowledged"
                    );
                    server_version = Some(ack.tds_version);
                }
                Token::EnvChange(env) => {
                    Self::process_env_change(&env, &mut database, &mut routing);
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
                Token::Done(done) => {
                    if done.status.error {
                        return Err(Error::Protocol("login failed".to_string()));
                    }
                    break;
                }
                _ => {}
            }
        }

        Ok((server_version, database, routing))
    }

    /// Process an EnvChange token.
    fn process_env_change(
        env: &EnvChange,
        database: &mut Option<String>,
        routing: &mut Option<(String, u16)>,
    ) {
        use tds_protocol::token::EnvChangeValue;

        match env.env_type {
            EnvChangeType::Database => {
                if let EnvChangeValue::String(ref new_value) = env.new_value {
                    tracing::debug!(database = %new_value, "database changed");
                    *database = Some(new_value.clone());
                }
            }
            EnvChangeType::Routing => {
                if let EnvChangeValue::Routing { ref host, port } = env.new_value {
                    tracing::info!(host = %host, port = port, "routing redirect received");
                    *routing = Some((host.clone(), port));
                }
            }
            _ => {
                if let EnvChangeValue::String(ref new_value) = env.new_value {
                    tracing::debug!(
                        env_type = ?env.env_type,
                        new_value = %new_value,
                        "environment change"
                    );
                }
            }
        }
    }
}

// Private helper methods available to all connection states
impl<S: ConnectionState> Client<S> {
    /// Send a SQL batch to the server.
    async fn send_sql_batch(&mut self, sql: &str) -> Result<()> {
        let payload = tds_protocol::encode_sql_batch(sql);
        let max_packet = self.config.packet_size as usize;

        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| Error::ConnectionClosed)?;

        match connection {
            ConnectionHandle::Tls(conn) => {
                conn.send_message(PacketType::SqlBatch, payload, max_packet)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
            }
            ConnectionHandle::Plain(conn) => {
                conn.send_message(PacketType::SqlBatch, payload, max_packet)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Send an RPC request to the server.
    async fn send_rpc(&mut self, rpc: &RpcRequest) -> Result<()> {
        let payload = rpc.encode();
        let max_packet = self.config.packet_size as usize;

        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| Error::ConnectionClosed)?;

        match connection {
            ConnectionHandle::Tls(conn) => {
                conn.send_message(PacketType::Rpc, payload, max_packet)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
            }
            ConnectionHandle::Plain(conn) => {
                conn.send_message(PacketType::Rpc, payload, max_packet)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Convert ToSql parameters to RPC parameters.
    fn convert_params(params: &[&(dyn crate::ToSql + Sync)]) -> Result<Vec<RpcParam>> {
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
                        buf.put_u32_le(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
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
                    SqlValue::Date(_) | SqlValue::Time(_) | SqlValue::DateTime(_) | SqlValue::DateTimeOffset(_) => {
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
                })
            })
            .collect()
    }

    /// Read complete query response including columns and rows.
    async fn read_query_response(
        &mut self,
    ) -> Result<(Vec<crate::row::Column>, Vec<crate::row::Row>)> {
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| Error::ConnectionClosed)?;

        let message = match connection {
            ConnectionHandle::Tls(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::Plain(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        }
        .ok_or_else(|| Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut columns: Vec<crate::row::Column> = Vec::new();
        let mut rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;

        loop {
            // Use next_token_with_metadata to properly parse Row/NbcRow tokens
            let token = parser
                .next_token_with_metadata(protocol_metadata.as_ref())
                .map_err(|e| Error::Protocol(e.to_string()))?;

            let Some(token) = token else {
                break;
            };

            match token {
                Token::ColMetaData(meta) => {
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
                            column
                        })
                        .collect();

                    tracing::debug!(columns = columns.len(), "received column metadata");
                    protocol_metadata = Some(meta);
                }
                Token::Row(raw_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = Self::convert_raw_row(&raw_row, meta, &columns)?;
                        rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = Self::convert_nbc_row(&nbc_row, meta, &columns)?;
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
                        return Err(Error::Query("query failed".to_string()));
                    }
                    tracing::debug!(
                        row_count = done.row_count,
                        has_more = done.status.more,
                        "query complete"
                    );
                    break;
                }
                Token::DoneProc(done) => {
                    if done.status.error {
                        return Err(Error::Query("query failed".to_string()));
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.error {
                        return Err(Error::Query("query failed".to_string()));
                    }
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

        tracing::debug!(
            columns = columns.len(),
            rows = rows.len(),
            "query response parsed"
        );
        Ok((columns, rows))
    }

    /// Convert a RawRow to a client Row.
    ///
    /// This parses the raw bytes back into SqlValue types based on column metadata.
    fn convert_raw_row(
        raw: &RawRow,
        meta: &ColMetaData,
        columns: &[crate::row::Column],
    ) -> Result<crate::row::Row> {
        let mut values = Vec::with_capacity(meta.columns.len());
        let mut buf = raw.data.as_ref();

        for col in &meta.columns {
            let value = Self::parse_column_value(&mut buf, col)?;
            values.push(value);
        }

        Ok(crate::row::Row::from_values(columns.to_vec(), values))
    }

    /// Convert an NbcRow to a client Row.
    ///
    /// NbcRow has a null bitmap followed by only non-null values.
    fn convert_nbc_row(
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
                let value = Self::parse_column_value(&mut buf, col)?;
                values.push(value);
            }
        }

        Ok(crate::row::Row::from_values(columns.to_vec(), values))
    }

    /// Parse a single column value from a buffer based on column metadata.
    fn parse_column_value(buf: &mut &[u8], col: &ColumnData) -> Result<mssql_types::SqlValue> {
        use bytes::Buf;
        use mssql_types::SqlValue;
        use tds_protocol::types::TypeId;

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
            TypeId::Money => {
                if buf.remaining() < 8 {
                    return Err(Error::Protocol("unexpected EOF reading MONEY".into()));
                }
                // MONEY is stored as 8 bytes, fixed-point with 4 decimal places
                let high = buf.get_i32_le();
                let low = buf.get_u32_le();
                let cents = ((high as i64) << 32) | (low as i64);
                let value = (cents as f64) / 10000.0;
                SqlValue::Double(value)
            }
            TypeId::Money4 => {
                if buf.remaining() < 4 {
                    return Err(Error::Protocol("unexpected EOF reading SMALLMONEY".into()));
                }
                let cents = buf.get_i32_le();
                let value = (cents as f64) / 10000.0;
                SqlValue::Double(value)
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
                    return Err(Error::Protocol("unexpected EOF reading FloatN length".into()));
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
            TypeId::MoneyN => {
                if buf.remaining() < 1 {
                    return Err(Error::Protocol("unexpected EOF reading MoneyN length".into()));
                }
                let len = buf.get_u8();
                match len {
                    0 => SqlValue::Null,
                    4 => {
                        let cents = buf.get_i32_le();
                        SqlValue::Double((cents as f64) / 10000.0)
                    }
                    8 => {
                        let high = buf.get_i32_le();
                        let low = buf.get_u32_le();
                        let cents = ((high as i64) << 32) | (low as i64);
                        SqlValue::Double((cents as f64) / 10000.0)
                    }
                    _ => {
                        return Err(Error::Protocol(format!("invalid MoneyN length: {len}")));
                    }
                }
            }
            TypeId::DateTimeN => {
                if buf.remaining() < 1 {
                    return Err(Error::Protocol(
                        "unexpected EOF reading DateTimeN length".into(),
                    ));
                }
                let len = buf.get_u8();
                if len == 0 {
                    SqlValue::Null
                } else {
                    // Skip date/time bytes for now - return as binary
                    if buf.remaining() < len as usize {
                        return Err(Error::Protocol("unexpected EOF reading DateTimeN".into()));
                    }
                    let bytes = bytes::Bytes::copy_from_slice(&buf[..len as usize]);
                    buf.advance(len as usize);
                    SqlValue::Binary(bytes)
                }
            }

            // Variable-length string types
            TypeId::BigVarChar | TypeId::BigChar | TypeId::Text => {
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
                    return Err(Error::Protocol("unexpected EOF reading varchar data".into()));
                } else {
                    let data = &buf[..len as usize];
                    let s = String::from_utf8_lossy(data).into_owned();
                    buf.advance(len as usize);
                    SqlValue::String(s)
                }
            }
            TypeId::NVarChar | TypeId::NChar | TypeId::NText => {
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
                    return Err(Error::Protocol("unexpected EOF reading nvarchar data".into()));
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

            // Variable-length binary types
            TypeId::BigVarBinary | TypeId::BigBinary | TypeId::Image => {
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

    /// Read execute result (row count) from the response.
    async fn read_execute_result(&mut self) -> Result<u64> {
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| Error::ConnectionClosed)?;

        let message = match connection {
            ConnectionHandle::Tls(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::Plain(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        }
        .ok_or_else(|| Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut rows_affected = 0u64;

        while let Some(token) = parser.next_token().map_err(|e| Error::Protocol(e.to_string()))? {
            match token {
                Token::Done(done) => {
                    if done.status.error {
                        return Err(Error::Query("execution failed".to_string()));
                    }
                    if done.status.count {
                        rows_affected = done.row_count;
                    }
                    break;
                }
                Token::DoneProc(done) => {
                    if done.status.count {
                        rows_affected = done.row_count;
                    }
                }
                Token::DoneInProc(done) => {
                    if done.status.count {
                        rows_affected = done.row_count;
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
                _ => {}
            }
        }

        Ok(rows_affected)
    }
}

impl Client<Ready> {
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
        let (columns, rows) = self.read_query_response().await?;

        Ok(QueryStream::new(columns, rows))
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
        let rows_affected = self.read_execute_result().await?;

        Ok(rows_affected)
    }

    /// Begin a transaction.
    ///
    /// This transitions the client from `Ready` to `InTransaction` state.
    pub async fn begin_transaction(mut self) -> Result<Client<InTransaction>> {
        tracing::debug!("beginning transaction");

        // Execute BEGIN TRANSACTION
        self.send_sql_batch("BEGIN TRANSACTION").await?;
        self.read_execute_result().await?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            statement_cache: self.statement_cache,
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
}

impl Client<InTransaction> {
    /// Execute a query within the transaction and return a streaming result set.
    ///
    /// See [`Client<Ready>::query`] for usage examples.
    pub async fn query<'a>(
        &'a mut self,
        sql: &str,
        params: &[&(dyn crate::ToSql + Sync)],
    ) -> Result<QueryStream<'a>> {
        tracing::debug!(sql = sql, params_count = params.len(), "executing query in transaction");

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
        let (columns, rows) = self.read_query_response().await?;

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
        let rows_affected = self.read_execute_result().await?;

        Ok(rows_affected)
    }

    /// Commit the transaction.
    ///
    /// This transitions the client back to `Ready` state.
    pub async fn commit(mut self) -> Result<Client<Ready>> {
        tracing::debug!("committing transaction");

        // Execute COMMIT TRANSACTION
        self.send_sql_batch("COMMIT TRANSACTION").await?;
        self.read_execute_result().await?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            statement_cache: self.statement_cache,
        })
    }

    /// Rollback the transaction.
    ///
    /// This transitions the client back to `Ready` state.
    pub async fn rollback(mut self) -> Result<Client<Ready>> {
        tracing::debug!("rolling back transaction");

        // Execute ROLLBACK TRANSACTION
        self.send_sql_batch("ROLLBACK TRANSACTION").await?;
        self.read_execute_result().await?;

        Ok(Client {
            config: self.config,
            _state: PhantomData,
            connection: self.connection,
            server_version: self.server_version,
            current_database: self.current_database,
            statement_cache: self.statement_cache,
        })
    }

    /// Create a savepoint and return a handle for later rollback.
    ///
    /// The returned `SavePoint` handle is lifetime-tied to the transaction,
    /// ensuring at compile time that savepoints cannot outlive their transaction.
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
    pub async fn save_point(&mut self, name: &str) -> Result<SavePoint<'_>> {
        validate_identifier(name)?;
        tracing::debug!(name = name, "creating savepoint");

        // Execute SAVE TRANSACTION <name>
        // Note: name is validated by validate_identifier() to prevent SQL injection
        let sql = format!("SAVE TRANSACTION {}", name);
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
    pub async fn rollback_to(&mut self, savepoint: &SavePoint<'_>) -> Result<()> {
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
    pub async fn release_savepoint(&mut self, savepoint: SavePoint<'_>) -> Result<()> {
        tracing::debug!(name = savepoint.name(), "releasing savepoint");

        // SQL Server doesn't require explicit savepoint release
        // The savepoint is implicitly released on commit/rollback
        // This method exists for API completeness
        drop(savepoint);
        Ok(())
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
            "invalid identifier '{}': must start with letter/underscore, \
             contain only alphanumerics/_/@/#/$, and be 1-128 characters",
            name
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
