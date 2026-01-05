//! SQL Server client implementation.

// Allow unwrap/expect for chrono date construction with known-valid constant dates
// and for regex patterns that are compile-time constants
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::needless_range_loop)]

use std::marker::PhantomData;
use std::sync::Arc;

use bytes::BytesMut;
use mssql_codec::connection::Connection;
use mssql_tls::{TlsConfig, TlsConnector, TlsNegotiationMode, TlsStream};
use tds_protocol::login7::Login7;
use tds_protocol::packet::{MAX_PACKET_SIZE, PacketType};
use tds_protocol::prelogin::{EncryptionLevel, PreLogin};
use tds_protocol::rpc::{RpcParam, RpcRequest, TypeInfo as RpcTypeInfo};
use tds_protocol::token::{
    ColMetaData, Collation, ColumnData, EnvChange, EnvChangeType, NbcRow, RawRow, Token,
    TokenParser,
};
#[cfg(feature = "decimal")]
use tds_protocol::tvp::encode_tvp_decimal;
use tds_protocol::tvp::{
    TvpColumnDef as TvpWireColumnDef, TvpColumnFlags, TvpEncoder, TvpWireType, encode_tvp_bit,
    encode_tvp_float, encode_tvp_int, encode_tvp_null, encode_tvp_nvarchar, encode_tvp_varbinary,
};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::Config;
use crate::error::{Error, Result};
#[cfg(feature = "otel")]
use crate::instrumentation::InstrumentationContext;
use crate::state::{ConnectionState, Disconnected, InTransaction, Ready};
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
    /// OpenTelemetry instrumentation context (when otel feature is enabled)
    #[cfg(feature = "otel")]
    instrumentation: InstrumentationContext,
}

/// Internal connection handle wrapping the actual connection.
///
/// This is an enum to support different connection types:
/// - TLS (TDS 8.0 strict mode)
/// - TLS with PreLogin wrapping (TDS 7.x style)
/// - Plain TCP (rare, for testing or internal networks)
#[allow(dead_code)] // Connection will be used once query execution is implemented
enum ConnectionHandle {
    /// TLS connection (TDS 8.0 strict mode - TLS before any TDS traffic)
    Tls(Connection<TlsStream<TcpStream>>),
    /// TLS connection with PreLogin wrapping (TDS 7.x style)
    TlsPrelogin(Connection<TlsStream<mssql_tls::TlsPreloginWrapper<TcpStream>>>),
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
        tcp_stream
            .set_nodelay(true)
            .map_err(|e| Error::Io(Arc::new(e)))?;

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

        let tls_connector = TlsConnector::new(tls_config).map_err(|e| Error::Tls(e.to_string()))?;

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
            current_database: current_database.clone(),
            statement_cache: StatementCache::with_default_size(),
            transaction_descriptor: 0, // Auto-commit mode initially
            #[cfg(feature = "otel")]
            instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                .with_database(current_database.unwrap_or_default()),
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
        use tds_protocol::packet::{PACKET_HEADER_SIZE, PacketHeader, PacketStatus};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        tracing::debug!("using TDS 7.x flow (PreLogin first)");

        // Build PreLogin packet
        // Determine client encryption level based on configuration
        let client_encryption = if config.no_tls {
            // no_tls: Completely disable TLS
            tracing::warn!(
                "⚠️  no_tls mode enabled. Connection will be UNENCRYPTED. \
                 Credentials and data will be transmitted in plaintext. \
                 This should only be used for development/testing with legacy SQL Server."
            );
            EncryptionLevel::NotSupported
        } else if config.encrypt {
            EncryptionLevel::On
        } else {
            EncryptionLevel::Off
        };
        let prelogin = Self::build_prelogin(config, client_encryption);
        tracing::debug!(encryption = ?client_encryption, "sending PreLogin");
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

        // Log PreLogin response
        // Note: The server sends its SQL Server product version in PreLogin,
        // NOT the TDS protocol version. The actual TDS version is negotiated
        // in the LOGINACK token after login.
        let client_tds_version = config.tds_version;
        if let Some(ref server_version) = prelogin_response.server_version {
            tracing::debug!(
                requested_tds_version = %client_tds_version,
                server_product_version = %server_version,
                server_product = server_version.product_name(),
                max_tds_version = %server_version.max_tds_version(),
                "PreLogin response received"
            );

            // Warn if the server's max TDS version is lower than requested
            let server_max_tds = server_version.max_tds_version();
            if server_max_tds < client_tds_version && !client_tds_version.is_tds_8() {
                tracing::warn!(
                    requested_tds_version = %client_tds_version,
                    server_max_tds_version = %server_max_tds,
                    server_product = server_version.product_name(),
                    "Server supports lower TDS version than requested. \
                     Connection will use server's maximum: {}",
                    server_max_tds
                );
            }

            // Warn about legacy SQL Server versions (2005 and earlier)
            if server_max_tds.is_legacy() {
                tracing::warn!(
                    server_product = server_version.product_name(),
                    server_max_tds_version = %server_max_tds,
                    "Server uses legacy TDS version. Some features may not be available."
                );
            }
        } else {
            tracing::debug!(
                requested_tds_version = %client_tds_version,
                "PreLogin response received (no version info)"
            );
        }

        // Check server encryption response
        let server_encryption = prelogin_response.encryption;
        tracing::debug!(encryption = ?server_encryption, "server encryption level");

        // Determine negotiated encryption level (follows TDS 7.x rules)
        // - NotSupported + NotSupported = NotSupported (no TLS at all)
        // - Off + Off = Off (TLS for login only, then plain)
        // - On + anything supported = On (full TLS)
        // - Required = On with failure if not possible
        let negotiated_encryption = match (client_encryption, server_encryption) {
            (EncryptionLevel::NotSupported, EncryptionLevel::NotSupported) => {
                EncryptionLevel::NotSupported
            }
            (EncryptionLevel::Off, EncryptionLevel::Off) => EncryptionLevel::Off,
            (EncryptionLevel::On, EncryptionLevel::Off)
            | (EncryptionLevel::On, EncryptionLevel::NotSupported) => {
                return Err(Error::Protocol(
                    "Server does not support requested encryption level".to_string(),
                ));
            }
            _ => EncryptionLevel::On,
        };

        // TLS is required unless negotiated encryption is NotSupported
        // Even with "Off", TLS is used to protect login credentials (per TDS 7.x spec)
        let use_tls = negotiated_encryption != EncryptionLevel::NotSupported;

        if use_tls {
            // Upgrade to TLS with PreLogin wrapping (TDS 7.x style)
            // In TDS 7.x, the TLS handshake is wrapped inside TDS PreLogin packets
            let tls_config =
                TlsConfig::new().trust_server_certificate(config.trust_server_certificate);

            let tls_connector =
                TlsConnector::new(tls_config).map_err(|e| Error::Tls(e.to_string()))?;

            // Use PreLogin-wrapped TLS connection for TDS 7.x
            let mut tls_stream = timeout(
                config.timeouts.tls_timeout,
                tls_connector.connect_with_prelogin(tcp_stream, &config.host),
            )
            .await
            .map_err(|_| Error::TlsTimeout)?
            .map_err(|e| Error::Tls(e.to_string()))?;

            tracing::debug!("TLS handshake completed (PreLogin wrapped)");

            // Check if we need full encryption or login-only encryption
            let login_only_encryption = negotiated_encryption == EncryptionLevel::Off;

            if login_only_encryption {
                // Login-Only Encryption (ENCRYPT_OFF + ENCRYPT_OFF per MS-TDS spec):
                // - Login7 is sent through TLS to protect credentials
                // - Server responds in PLAINTEXT after receiving Login7
                // - All subsequent communication is plaintext
                //
                // We must NOT use Connection with TLS stream because Connection splits
                // the stream and we need to extract the underlying TCP afterward.
                use tokio::io::AsyncWriteExt;

                // Build and send Login7 directly through TLS
                let login = Self::build_login7(config);
                let login_payload = login.encode();

                // Create TDS packet manually for Login7
                let max_packet = MAX_PACKET_SIZE;
                let max_payload = max_packet - PACKET_HEADER_SIZE;
                let chunks: Vec<_> = login_payload.chunks(max_payload).collect();
                let total_chunks = chunks.len();

                for (i, chunk) in chunks.into_iter().enumerate() {
                    let is_last = i == total_chunks - 1;
                    let status = if is_last {
                        PacketStatus::END_OF_MESSAGE
                    } else {
                        PacketStatus::NORMAL
                    };

                    let header = PacketHeader::new(
                        PacketType::Tds7Login,
                        status,
                        (PACKET_HEADER_SIZE + chunk.len()) as u16,
                    );

                    let mut packet_buf = BytesMut::with_capacity(PACKET_HEADER_SIZE + chunk.len());
                    header.encode(&mut packet_buf);
                    packet_buf.put_slice(chunk);

                    tls_stream
                        .write_all(&packet_buf)
                        .await
                        .map_err(|e| Error::Io(Arc::new(e)))?;
                }

                // Flush TLS to ensure all data is sent
                tls_stream
                    .flush()
                    .await
                    .map_err(|e| Error::Io(Arc::new(e)))?;

                tracing::debug!("Login7 sent through TLS, switching to plaintext for response");

                // Extract the underlying TCP stream from the TLS layer
                // TlsStream::into_inner() returns (IO, ClientConnection)
                // where IO is our TlsPreloginWrapper<TcpStream>
                let (wrapper, _client_conn) = tls_stream.into_inner();
                let tcp_stream = wrapper.into_inner();

                // Create Connection from plain TCP for reading response
                let mut connection = Connection::new(tcp_stream);

                // Process login response (comes in plaintext)
                let (server_version, current_database, routing) =
                    Self::process_login_response(&mut connection).await?;

                // Handle routing redirect
                if let Some((host, port)) = routing {
                    return Err(Error::Routing { host, port });
                }

                // Store plain TCP connection for subsequent operations
                Ok(Client {
                    config: config.clone(),
                    _state: PhantomData,
                    connection: Some(ConnectionHandle::Plain(connection)),
                    server_version,
                    current_database: current_database.clone(),
                    statement_cache: StatementCache::with_default_size(),
                    transaction_descriptor: 0, // Auto-commit mode initially
                    #[cfg(feature = "otel")]
                    instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                        .with_database(current_database.unwrap_or_default()),
                })
            } else {
                // Full Encryption (ENCRYPT_ON per MS-TDS spec):
                // - All communication after TLS handshake goes through TLS
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
                    connection: Some(ConnectionHandle::TlsPrelogin(connection)),
                    server_version,
                    current_database: current_database.clone(),
                    statement_cache: StatementCache::with_default_size(),
                    transaction_descriptor: 0, // Auto-commit mode initially
                    #[cfg(feature = "otel")]
                    instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                        .with_database(current_database.unwrap_or_default()),
                })
            }
        } else {
            // Server does not require encryption and client doesn't either
            tracing::warn!(
                "Connecting without TLS encryption. This is insecure and should only be \
                 used for development/testing on trusted networks."
            );

            // Build Login7 packet
            let login = Self::build_login7(config);
            let login_bytes = login.encode();
            tracing::debug!("Login7 packet built: {} bytes", login_bytes.len(),);
            // Dump the fixed header (94 bytes)
            tracing::debug!(
                "Login7 fixed header (94 bytes): {:02X?}",
                &login_bytes[..login_bytes.len().min(94)]
            );
            // Dump variable data
            if login_bytes.len() > 94 {
                tracing::debug!(
                    "Login7 variable data ({} bytes): {:02X?}",
                    login_bytes.len() - 94,
                    &login_bytes[94..]
                );
            }

            // Send Login7 over raw TCP (like PreLogin)
            let login_header = PacketHeader::new(
                PacketType::Tds7Login,
                PacketStatus::END_OF_MESSAGE,
                (PACKET_HEADER_SIZE + login_bytes.len()) as u16,
            )
            .with_packet_id(1);
            let mut login_packet_buf =
                BytesMut::with_capacity(PACKET_HEADER_SIZE + login_bytes.len());
            login_header.encode(&mut login_packet_buf);
            login_packet_buf.put_slice(&login_bytes);

            tracing::debug!(
                "Sending Login7 packet: {} bytes total, header: {:02X?}",
                login_packet_buf.len(),
                &login_packet_buf[..PACKET_HEADER_SIZE]
            );
            tcp_stream
                .write_all(&login_packet_buf)
                .await
                .map_err(|e| Error::Io(Arc::new(e)))?;
            tcp_stream
                .flush()
                .await
                .map_err(|e| Error::Io(Arc::new(e)))?;
            tracing::debug!("Login7 sent and flushed over raw TCP");

            // Read login response header
            let mut response_header_buf = [0u8; PACKET_HEADER_SIZE];
            tcp_stream
                .read_exact(&mut response_header_buf)
                .await
                .map_err(|e| Error::Io(Arc::new(e)))?;

            let response_type = response_header_buf[0];
            let response_length =
                u16::from_be_bytes([response_header_buf[2], response_header_buf[3]]) as usize;
            tracing::debug!(
                "Response header: type={:#04X}, length={}",
                response_type,
                response_length
            );

            // Read response payload
            let payload_length = response_length.saturating_sub(PACKET_HEADER_SIZE);
            let mut response_payload = vec![0u8; payload_length];
            tcp_stream
                .read_exact(&mut response_payload)
                .await
                .map_err(|e| Error::Io(Arc::new(e)))?;
            tracing::debug!(
                "Response payload: {} bytes, first 32: {:02X?}",
                response_payload.len(),
                &response_payload[..response_payload.len().min(32)]
            );

            // Now create Connection for further communication
            let connection = Connection::new(tcp_stream);

            // Parse login response
            let response_bytes = bytes::Bytes::from(response_payload);
            let mut parser = TokenParser::new(response_bytes);
            let mut server_version = None;
            let mut current_database = None;
            let routing = None;

            while let Some(token) = parser
                .next_token()
                .map_err(|e| Error::Protocol(e.to_string()))?
            {
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
                        Self::process_env_change(&env, &mut current_database, &mut None);
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

            // Handle routing redirect
            if let Some((host, port)) = routing {
                return Err(Error::Routing { host, port });
            }

            Ok(Client {
                config: config.clone(),
                _state: PhantomData,
                connection: Some(ConnectionHandle::Plain(connection)),
                server_version,
                current_database: current_database.clone(),
                statement_cache: StatementCache::with_default_size(),
                transaction_descriptor: 0, // Auto-commit mode initially
                #[cfg(feature = "otel")]
                instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                    .with_database(current_database.unwrap_or_default()),
            })
        }
    }

    /// Build a PreLogin packet.
    fn build_prelogin(config: &Config, encryption: EncryptionLevel) -> PreLogin {
        // Use the configured TDS version (strict_mode overrides to V8_0)
        let version = if config.strict_mode {
            tds_protocol::version::TdsVersion::V8_0
        } else {
            config.tds_version
        };

        let mut prelogin = PreLogin::new()
            .with_version(version)
            .with_encryption(encryption);

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
        // Use the configured TDS version (strict_mode overrides to V8_0)
        let version = if config.strict_mode {
            tds_protocol::version::TdsVersion::V8_0
        } else {
            config.tds_version
        };

        let mut login = Login7::new()
            .with_tds_version(version)
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
            .ok_or(Error::ConnectionClosed)?;

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
            .ok_or(Error::ConnectionClosed)?;

        let response_bytes = message.payload;

        let mut parser = TokenParser::new(response_bytes);
        let mut server_version = None;
        let mut database = None;
        let mut routing = None;

        while let Some(token) = parser
            .next_token()
            .map_err(|e| Error::Protocol(e.to_string()))?
        {
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
    async fn send_sql_batch(&mut self, sql: &str) -> Result<()> {
        let payload =
            tds_protocol::encode_sql_batch_with_transaction(sql, self.transaction_descriptor);
        let max_packet = self.config.packet_size as usize;

        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        match connection {
            ConnectionHandle::Tls(conn) => {
                conn.send_message(PacketType::SqlBatch, payload, max_packet)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
            }
            ConnectionHandle::TlsPrelogin(conn) => {
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
    ///
    /// Uses the client's current transaction descriptor in ALL_HEADERS.
    async fn send_rpc(&mut self, rpc: &RpcRequest) -> Result<()> {
        let payload = rpc.encode_with_transaction(self.transaction_descriptor);
        let max_packet = self.config.packet_size as usize;

        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        match connection {
            ConnectionHandle::Tls(conn) => {
                conn.send_message(PacketType::Rpc, payload, max_packet)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
            }
            ConnectionHandle::TlsPrelogin(conn) => {
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
                TvpWireColumnDef {
                    wire_type,
                    flags: TvpColumnFlags {
                        nullable: col.nullable,
                    },
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

    /// Read complete query response including columns and rows.
    async fn read_query_response(
        &mut self,
    ) -> Result<(Vec<crate::row::Column>, Vec<crate::row::Row>)> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            ConnectionHandle::Tls(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::TlsPrelogin(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::Plain(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        }
        .ok_or(Error::ConnectionClosed)?;

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
                    // Only break if there are no more result sets
                    // This enables multi-statement batches to process all results
                    if !done.status.more {
                        break;
                    }
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
            TypeId::MoneyN => {
                if buf.remaining() < 1 {
                    return Err(Error::Protocol(
                        "unexpected EOF reading MoneyN length".into(),
                    ));
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
                            let mut decimal =
                                Decimal::from_i128_with_scale(mantissa as i128, scale);
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
                                let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
                                let date = base + chrono::Duration::days(days);
                                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
                                    minutes * 60,
                                    0,
                                )
                                .unwrap();
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
                                let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
                                let date = base + chrono::Duration::days(days);
                                // Convert 300ths of second to nanoseconds
                                let total_ms = (time_300ths * 1000) / 300;
                                let secs = (total_ms / 1000) as u32;
                                let nanos = ((total_ms % 1000) * 1_000_000) as u32;
                                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
                                    secs, nanos,
                                )
                                .unwrap();
                                SqlValue::DateTime(date.and_time(time))
                            }
                            #[cfg(not(feature = "chrono"))]
                            {
                                SqlValue::String(format!("DATETIME({days},{time_300ths})"))
                            }
                        }
                        _ => {
                            return Err(Error::Protocol(format!(
                                "invalid DateTimeN length: {len}"
                            )));
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
                    let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
                    let date = base + chrono::Duration::days(days);
                    let total_ms = (time_300ths * 1000) / 300;
                    let secs = (total_ms / 1000) as u32;
                    let nanos = ((total_ms % 1000) * 1_000_000) as u32;
                    let time =
                        chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos).unwrap();
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
                    let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
                    let date = base + chrono::Duration::days(days);
                    let time =
                        chrono::NaiveTime::from_num_seconds_from_midnight_opt(minutes * 60, 0)
                            .unwrap();
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
                        let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
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
                        let time = Self::intervals_to_time(intervals, scale);
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
                    let time_len = Self::time_bytes_for_scale(scale);

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
                        let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                        let date = base + chrono::Duration::days(days as i64);
                        let time = Self::intervals_to_time(intervals, scale);
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
                    let time_len = Self::time_bytes_for_scale(scale);

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
                        let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                        let date = base + chrono::Duration::days(days as i64);
                        let time = Self::intervals_to_time(intervals, scale);
                        let offset = chrono::FixedOffset::east_opt((offset_minutes as i32) * 60)
                            .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
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
            TypeId::Text => Self::parse_plp_varchar(buf, col.type_info.collation.as_ref())?,

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
                    let s = Self::decode_varchar_string(data, col.type_info.collation.as_ref());
                    buf.advance(len as usize);
                    SqlValue::String(s)
                }
            }

            // Variable-length string types (BigVarChar, BigChar)
            TypeId::BigVarChar | TypeId::BigChar => {
                // Check if this is a MAX type (uses PLP encoding)
                if col.type_info.max_length == Some(0xFFFF) {
                    // PLP format: 8-byte total length, then chunks
                    Self::parse_plp_varchar(buf, col.type_info.collation.as_ref())?
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
                        let s = Self::decode_varchar_string(data, col.type_info.collation.as_ref());
                        buf.advance(len as usize);
                        SqlValue::String(s)
                    }
                }
            }

            // NTEXT type - always uses PLP encoding (deprecated LOB type)
            TypeId::NText => Self::parse_plp_nvarchar(buf)?,

            // Variable-length Unicode string types (NVarChar, NChar)
            TypeId::NVarChar | TypeId::NChar => {
                // Check if this is a MAX type (uses PLP encoding)
                if col.type_info.max_length == Some(0xFFFF) {
                    // PLP format: 8-byte total length, then chunks
                    Self::parse_plp_nvarchar(buf)?
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
            TypeId::Image => Self::parse_plp_varbinary(buf)?,

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
                    Self::parse_plp_varbinary(buf)?
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
                match Self::parse_plp_nvarchar(buf)? {
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
            TypeId::Variant => Self::parse_sql_variant(buf)?,

            // UDT (User-Defined Type) - uses PLP encoding, return as binary
            TypeId::Udt => Self::parse_plp_varbinary(buf)?,

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
    fn parse_plp_nvarchar(buf: &mut &[u8]) -> Result<mssql_types::SqlValue> {
        use bytes::Buf;
        use mssql_types::SqlValue;

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
        // Try UTF-8 first (most common case and zero-cost for ASCII)
        if let Ok(s) = std::str::from_utf8(data) {
            return s.to_owned();
        }

        // If UTF-8 fails, try collation-aware decoding
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
    fn parse_plp_varchar(
        buf: &mut &[u8],
        collation: Option<&Collation>,
    ) -> Result<mssql_types::SqlValue> {
        use bytes::Buf;
        use mssql_types::SqlValue;

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
        let s = Self::decode_varchar_string(&all_data, collation);
        Ok(SqlValue::String(s))
    }

    /// Parse PLP-encoded VARBINARY(MAX) data.
    fn parse_plp_varbinary(buf: &mut &[u8]) -> Result<mssql_types::SqlValue> {
        use bytes::Buf;
        use mssql_types::SqlValue;

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
    fn parse_sql_variant(buf: &mut &[u8]) -> Result<mssql_types::SqlValue> {
        use bytes::Buf;
        use mssql_types::SqlValue;

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

                if money_len == 4 && data_len >= 4 {
                    let raw = buf.get_i32_le();
                    let value = raw as f64 / 10000.0;
                    Ok(SqlValue::Double(value))
                } else if data_len >= 8 {
                    let high = buf.get_i32_le() as i64;
                    let low = buf.get_u32_le() as i64;
                    let raw = (high << 32) | low;
                    let value = raw as f64 / 10000.0;
                    Ok(SqlValue::Double(value))
                } else {
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
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
                        let dt = base
                            + chrono::Duration::days(days)
                            + chrono::Duration::minutes(mins as i64);
                        Ok(SqlValue::DateTime(dt))
                    } else if data_len >= 8 {
                        // DATETIME
                        let days = buf.get_i32_le() as i64;
                        let ticks = buf.get_u32_le() as i64;
                        let base = NaiveDate::from_ymd_opt(1900, 1, 1)
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
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
                        let mut decimal =
                            Decimal::from_i128_with_scale(mantissa as i128, scale as u32);
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
                    let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
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
                let s = Self::decode_varchar_string(data, collation.as_ref());
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
                let s = String::from_utf16(&utf16).map_err(|_| {
                    Error::Protocol("invalid UTF-16 in SQL_VARIANT nvarchar".into())
                })?;
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
            .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
    }

    /// Read execute result (row count) from the response.
    async fn read_execute_result(&mut self) -> Result<u64> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            ConnectionHandle::Tls(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::TlsPrelogin(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::Plain(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut rows_affected = 0u64;
        let mut current_metadata: Option<ColMetaData> = None;

        loop {
            // Use metadata-aware parsing to handle Row tokens from SELECT statements
            let token = parser
                .next_token_with_metadata(current_metadata.as_ref())
                .map_err(|e| Error::Protocol(e.to_string()))?;

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
                        return Err(Error::Query("execution failed".to_string()));
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
    async fn read_transaction_begin_result(&mut self) -> Result<u64> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            ConnectionHandle::Tls(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::TlsPrelogin(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::Plain(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut transaction_descriptor: u64 = 0;

        loop {
            let token = parser
                .next_token()
                .map_err(|e| Error::Protocol(e.to_string()))?;

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

    /// Read multiple result sets from a batch response.
    async fn read_multi_result_response(&mut self) -> Result<Vec<crate::stream::ResultSet>> {
        let connection = self.connection.as_mut().ok_or(Error::ConnectionClosed)?;

        let message = match connection {
            ConnectionHandle::Tls(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::TlsPrelogin(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
            ConnectionHandle::Plain(conn) => conn
                .read_message()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        }
        .ok_or(Error::ConnectionClosed)?;

        let mut parser = TokenParser::new(message.payload);
        let mut result_sets: Vec<crate::stream::ResultSet> = Vec::new();
        let mut current_columns: Vec<crate::row::Column> = Vec::new();
        let mut current_rows: Vec<crate::row::Row> = Vec::new();
        let mut protocol_metadata: Option<ColMetaData> = None;

        loop {
            let token = parser
                .next_token_with_metadata(protocol_metadata.as_ref())
                .map_err(|e| Error::Protocol(e.to_string()))?;

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
                        let row = Self::convert_raw_row(&raw_row, meta, &current_columns)?;
                        current_rows.push(row);
                    }
                }
                Token::NbcRow(nbc_row) => {
                    if let Some(ref meta) = protocol_metadata {
                        let row = Self::convert_nbc_row(&nbc_row, meta, &current_columns)?;
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
                        return Err(Error::Query("query failed".to_string()));
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
                        return Err(Error::Query("query failed".to_string()));
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
                        return Err(Error::Query("query failed".to_string()));
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
            ConnectionHandle::Tls(conn) => {
                crate::cancel::CancelHandle::from_tls(conn.cancel_handle())
            }
            ConnectionHandle::TlsPrelogin(conn) => {
                crate::cancel::CancelHandle::from_tls_prelogin(conn.cancel_handle())
            }
            ConnectionHandle::Plain(conn) => {
                crate::cancel::CancelHandle::from_plain(conn.cancel_handle())
            }
        }
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
            ConnectionHandle::Tls(conn) => {
                crate::cancel::CancelHandle::from_tls(conn.cancel_handle())
            }
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

        let result = Client::<Ready>::parse_plp_nvarchar(&mut buf).unwrap();
        match result {
            mssql_types::SqlValue::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("expected String, got {:?}", result),
        }
    }

    #[test]
    fn test_parse_plp_nvarchar_null() {
        // NULL is indicated by total_len = 0xFFFFFFFFFFFFFFFF
        let plp = 0xFFFFFFFFFFFFFFFFu64.to_le_bytes();
        let mut buf: &[u8] = &plp;

        let result = Client::<Ready>::parse_plp_nvarchar(&mut buf).unwrap();
        assert!(matches!(result, mssql_types::SqlValue::Null));
    }

    #[test]
    fn test_parse_plp_nvarchar_empty() {
        // Empty string: total_len=0, single zero-length chunk
        let plp = make_plp_data(0, &[]);
        let mut buf: &[u8] = &plp;

        let result = Client::<Ready>::parse_plp_nvarchar(&mut buf).unwrap();
        match result {
            mssql_types::SqlValue::String(s) => assert_eq!(s, ""),
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

        let result = Client::<Ready>::parse_plp_nvarchar(&mut buf).unwrap();
        match result {
            mssql_types::SqlValue::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn test_parse_plp_varchar_simple() {
        let data = b"Hello World";
        let plp = make_plp_data(11, &[data]);
        let mut buf: &[u8] = &plp;

        let result = Client::<Ready>::parse_plp_varchar(&mut buf, None).unwrap();
        match result {
            mssql_types::SqlValue::String(s) => assert_eq!(s, "Hello World"),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn test_parse_plp_varchar_null() {
        let plp = 0xFFFFFFFFFFFFFFFFu64.to_le_bytes();
        let mut buf: &[u8] = &plp;

        let result = Client::<Ready>::parse_plp_varchar(&mut buf, None).unwrap();
        assert!(matches!(result, mssql_types::SqlValue::Null));
    }

    #[test]
    fn test_parse_plp_varbinary_simple() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        let plp = make_plp_data(5, &[&data]);
        let mut buf: &[u8] = &plp;

        let result = Client::<Ready>::parse_plp_varbinary(&mut buf).unwrap();
        match result {
            mssql_types::SqlValue::Binary(b) => assert_eq!(&b[..], &[0x01, 0x02, 0x03, 0x04, 0x05]),
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn test_parse_plp_varbinary_null() {
        let plp = 0xFFFFFFFFFFFFFFFFu64.to_le_bytes();
        let mut buf: &[u8] = &plp;

        let result = Client::<Ready>::parse_plp_varbinary(&mut buf).unwrap();
        assert!(matches!(result, mssql_types::SqlValue::Null));
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

        let result = Client::<Ready>::parse_plp_varbinary(&mut buf).unwrap();
        match result {
            mssql_types::SqlValue::Binary(b) => {
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

    use tds_protocol::token::{ColumnData, TypeInfo};
    use tds_protocol::types::TypeId;

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
        };

        let mut buf: &[u8] = &raw_data;

        // Parse column 0 (NVarChar)
        let value0 = Client::<Ready>::parse_column_value(&mut buf, &col0).unwrap();
        match value0 {
            mssql_types::SqlValue::String(s) => assert_eq!(s, "World"),
            _ => panic!("expected String, got {:?}", value0),
        }

        // Parse column 1 (IntN)
        let value1 = Client::<Ready>::parse_column_value(&mut buf, &col1).unwrap();
        match value1 {
            mssql_types::SqlValue::Int(i) => assert_eq!(i, 42),
            _ => panic!("expected Int, got {:?}", value1),
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
        };
        let col2 = col0.clone();
        let col3 = col1.clone();

        let mut buf: &[u8] = &data;

        // Parse all 4 columns
        let v0 = Client::<Ready>::parse_column_value(&mut buf, &col0).unwrap();
        assert!(
            matches!(v0, mssql_types::SqlValue::Null),
            "col0 should be Null"
        );

        let v1 = Client::<Ready>::parse_column_value(&mut buf, &col1).unwrap();
        assert!(
            matches!(v1, mssql_types::SqlValue::Int(123)),
            "col1 should be 123"
        );

        let v2 = Client::<Ready>::parse_column_value(&mut buf, &col2).unwrap();
        match v2 {
            mssql_types::SqlValue::String(s) => assert_eq!(s, "Test"),
            _ => panic!("col2 should be 'Test'"),
        }

        let v3 = Client::<Ready>::parse_column_value(&mut buf, &col3).unwrap();
        assert!(
            matches!(v3, mssql_types::SqlValue::Null),
            "col3 should be Null"
        );

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
        };

        let mut buf: &[u8] = &data;

        let v0 = Client::<Ready>::parse_column_value(&mut buf, &col0).unwrap();
        match v0 {
            mssql_types::SqlValue::String(s) => assert_eq!(s, test_str),
            _ => panic!("expected String"),
        }

        let v1 = Client::<Ready>::parse_column_value(&mut buf, &col1).unwrap();
        match v1 {
            mssql_types::SqlValue::BigInt(i) => assert_eq!(i, 9999999999),
            _ => panic!("expected BigInt"),
        }

        assert_eq!(buf.len(), 0, "buffer should be fully consumed");
    }
}
