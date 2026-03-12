//! Connection establishment for SQL Server.
//!
//! This module contains the `impl Client<Disconnected>` block, handling
//! TCP connection, TLS negotiation, PreLogin exchange, and Login7 authentication.

use std::marker::PhantomData;

use bytes::BytesMut;
use mssql_codec::connection::Connection;
#[cfg(feature = "tls")]
use mssql_tls::{TlsConfig, TlsConnector, TlsNegotiationMode};
use tds_protocol::login7::Login7;
#[cfg(feature = "tls")]
use tds_protocol::packet::MAX_PACKET_SIZE;
use tds_protocol::packet::PacketType;
use tds_protocol::prelogin::{EncryptionLevel, PreLogin};
use tds_protocol::token::{EnvChange, EnvChangeType, Token, TokenParser};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::Config;
use crate::error::{Error, Result};
#[cfg(feature = "otel")]
use crate::instrumentation::InstrumentationContext;
use crate::state::{Disconnected, Ready};
use crate::statement_cache::StatementCache;

use super::{Client, ConnectionHandle};

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
            .map_err(Error::from)?;

        // Enable TCP nodelay for better latency
        tcp_stream.set_nodelay(true).map_err(Error::from)?;

        #[cfg(feature = "tls")]
        {
            // Determine TLS negotiation mode
            let tls_mode = TlsNegotiationMode::from_encrypt_mode(config.strict_mode);

            // Step 2: Handle TDS 8.0 strict mode (TLS before any TDS traffic)
            if tls_mode.is_tls_first() {
                return Self::connect_tds_8(config, tcp_stream).await;
            }

            // Step 3: TDS 7.x flow - PreLogin first, then TLS, then Login7
            Self::connect_tds_7x(config, tcp_stream).await
        }

        #[cfg(not(feature = "tls"))]
        {
            // When TLS feature is disabled, only no_tls connections are supported
            if config.strict_mode {
                return Err(Error::Config(
                    "TDS 8.0 strict mode requires TLS. Enable the 'tls' feature or use Encrypt=no_tls".into()
                ));
            }

            if !config.no_tls {
                return Err(Error::Config(
                    "TLS encryption requires the 'tls' feature. Either enable the 'tls' feature \
                     or use Encrypt=no_tls in your connection string for unencrypted connections."
                        .into(),
                ));
            }

            // Proceed with no-TLS connection
            Self::connect_no_tls(config, tcp_stream).await
        }
    }

    /// Connect using TDS 8.0 strict mode.
    ///
    /// Flow: TCP -> TLS -> PreLogin (encrypted) -> Login7 (encrypted)
    #[cfg(feature = "tls")]
    async fn connect_tds_8(config: &Config, tcp_stream: TcpStream) -> Result<Client<Ready>> {
        tracing::debug!("using TDS 8.0 strict mode (TLS first)");

        // Build TLS configuration
        let tls_config = TlsConfig::new()
            .strict_mode(true)
            .trust_server_certificate(config.trust_server_certificate);

        let tls_connector = TlsConnector::new(tls_config)?;

        // Perform TLS handshake before any TDS traffic
        let tls_stream = timeout(
            config.timeouts.tls_timeout,
            tls_connector.connect(tcp_stream, &config.host),
        )
        .await
        .map_err(|_| Error::TlsTimeout)??;

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

        // Process login response (with timeout to prevent hangs during redirect)
        let (server_version, current_database, routing) = timeout(
            config.timeouts.login_timeout,
            Self::process_login_response(&mut connection),
        )
        .await
        .map_err(|_| Error::ConnectionTimeout)??;

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
            needs_reset: false,        // Fresh connection, no reset needed
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
    #[cfg(feature = "tls")]
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
            .map_err(Error::from)?;

        // Read PreLogin response
        let mut header_buf = [0u8; PACKET_HEADER_SIZE];
        tcp_stream
            .read_exact(&mut header_buf)
            .await
            .map_err(Error::from)?;

        let response_length = u16::from_be_bytes([header_buf[2], header_buf[3]]) as usize;
        let payload_length = response_length.saturating_sub(PACKET_HEADER_SIZE);

        let mut response_buf = vec![0u8; payload_length];
        tcp_stream
            .read_exact(&mut response_buf)
            .await
            .map_err(Error::from)?;

        let prelogin_response = PreLogin::decode(&response_buf[..])?;

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

            let tls_connector = TlsConnector::new(tls_config)?;

            // Use PreLogin-wrapped TLS connection for TDS 7.x
            let mut tls_stream = timeout(
                config.timeouts.tls_timeout,
                tls_connector.connect_with_prelogin(tcp_stream, &config.host),
            )
            .await
            .map_err(|_| Error::TlsTimeout)??;

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
                        .map_err(Error::from)?;
                }

                // Flush TLS to ensure all data is sent
                tls_stream.flush().await.map_err(Error::from)?;

                tracing::debug!("Login7 sent through TLS, switching to plaintext for response");

                // Extract the underlying TCP stream from the TLS layer
                // TlsStream::into_inner() returns (IO, ClientConnection)
                // where IO is our TlsPreloginWrapper<TcpStream>
                let (wrapper, _client_conn) = tls_stream.into_inner();
                let tcp_stream = wrapper.into_inner();

                // Create Connection from plain TCP for reading response
                let mut connection = Connection::new(tcp_stream);

                // Process login response (comes in plaintext, with timeout)
                let (server_version, current_database, routing) = timeout(
                    config.timeouts.login_timeout,
                    Self::process_login_response(&mut connection),
                )
                .await
                .map_err(|_| Error::ConnectionTimeout)??;

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
                    needs_reset: false,        // Fresh connection, no reset needed
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

                // Process login response (with timeout)
                let (server_version, current_database, routing) = timeout(
                    config.timeouts.login_timeout,
                    Self::process_login_response(&mut connection),
                )
                .await
                .map_err(|_| Error::ConnectionTimeout)??;

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
                    needs_reset: false,        // Fresh connection, no reset needed
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
            // Note: Login7 packet hex dump intentionally omitted to prevent
            // credential leakage — the variable-data section contains the
            // obfuscated (trivially reversible) password.

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
                .map_err(Error::from)?;
            tcp_stream.flush().await.map_err(Error::from)?;
            tracing::debug!("Login7 sent and flushed over raw TCP");

            // Read login response header
            let mut response_header_buf = [0u8; PACKET_HEADER_SIZE];
            tcp_stream
                .read_exact(&mut response_header_buf)
                .await
                .map_err(Error::from)?;

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
                .map_err(Error::from)?;
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

            while let Some(token) = parser.next_token()? {
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
                needs_reset: false,        // Fresh connection, no reset needed
                #[cfg(feature = "otel")]
                instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                    .with_database(current_database.unwrap_or_default()),
            })
        }
    }

    /// Connect without TLS encryption (no_tls mode).
    ///
    /// This method is used when the `tls` feature is disabled and only supports
    /// unencrypted connections via `Encrypt=no_tls`.
    ///
    /// # Security Warning
    ///
    /// This transmits all data including credentials in plaintext. Only use this
    /// for development, testing, or on trusted internal networks where TLS is not
    /// required.
    #[cfg(not(feature = "tls"))]
    async fn connect_no_tls(config: &Config, mut tcp_stream: TcpStream) -> Result<Client<Ready>> {
        use bytes::BufMut;
        use tds_protocol::packet::{PACKET_HEADER_SIZE, PacketHeader, PacketStatus};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        tracing::warn!(
            "⚠️  Connecting without TLS (tls feature disabled). \
             Credentials and data will be transmitted in plaintext."
        );

        // Build PreLogin packet with NotSupported encryption
        let prelogin = Self::build_prelogin(config, EncryptionLevel::NotSupported);
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
            .map_err(Error::from)?;

        // Read PreLogin response
        let mut header_buf = [0u8; PACKET_HEADER_SIZE];
        tcp_stream
            .read_exact(&mut header_buf)
            .await
            .map_err(Error::from)?;

        let response_length = u16::from_be_bytes([header_buf[2], header_buf[3]]) as usize;
        let payload_length = response_length.saturating_sub(PACKET_HEADER_SIZE);

        let mut response_buf = vec![0u8; payload_length];
        tcp_stream
            .read_exact(&mut response_buf)
            .await
            .map_err(Error::from)?;

        let prelogin_response = PreLogin::decode(&response_buf[..])?;

        // Check server encryption response - must accept NotSupported
        let server_encryption = prelogin_response.encryption;
        if server_encryption != EncryptionLevel::NotSupported {
            return Err(Error::Config(format!(
                "Server requires encryption (level: {:?}) but TLS feature is disabled. \
                     Either enable the 'tls' feature or configure the server to allow unencrypted connections.",
                server_encryption
            )));
        }

        tracing::debug!("Server accepted unencrypted connection");

        // Build Login7 packet
        let login = Self::build_login7(config);
        let login_bytes = login.encode();

        // Send Login7 over raw TCP
        let login_header = PacketHeader::new(
            PacketType::Tds7Login,
            PacketStatus::END_OF_MESSAGE,
            (PACKET_HEADER_SIZE + login_bytes.len()) as u16,
        )
        .with_packet_id(1);
        let mut login_packet_buf = BytesMut::with_capacity(PACKET_HEADER_SIZE + login_bytes.len());
        login_header.encode(&mut login_packet_buf);
        login_packet_buf.put_slice(&login_bytes);

        tcp_stream
            .write_all(&login_packet_buf)
            .await
            .map_err(Error::from)?;
        tcp_stream.flush().await.map_err(Error::from)?;

        // Read login response header
        let mut response_header_buf = [0u8; PACKET_HEADER_SIZE];
        tcp_stream
            .read_exact(&mut response_header_buf)
            .await
            .map_err(Error::from)?;

        let response_length =
            u16::from_be_bytes([response_header_buf[2], response_header_buf[3]]) as usize;

        // Read response payload
        let payload_length = response_length.saturating_sub(PACKET_HEADER_SIZE);
        let mut response_payload = vec![0u8; payload_length];
        tcp_stream
            .read_exact(&mut response_payload)
            .await
            .map_err(Error::from)?;

        // Create Connection for further communication
        let connection = Connection::new(tcp_stream);

        // Parse login response
        let response_bytes = bytes::Bytes::from(response_payload);
        let mut parser = TokenParser::new(response_bytes);
        let mut server_version = None;
        let mut current_database = None;

        while let Some(token) = parser.next_token()? {
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

        Ok(Client {
            config: config.clone(),
            _state: PhantomData,
            connection: Some(ConnectionHandle::Plain(connection)),
            server_version,
            current_database: current_database.clone(),
            statement_cache: StatementCache::with_default_size(),
            transaction_descriptor: 0,
            needs_reset: false,
            #[cfg(feature = "otel")]
            instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                .with_database(current_database.unwrap_or_default()),
        })
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
    #[cfg(feature = "tls")]
    async fn send_prelogin<T>(connection: &mut Connection<T>, prelogin: &PreLogin) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = prelogin.encode();
        let max_packet = MAX_PACKET_SIZE;

        connection
            .send_message(PacketType::PreLogin, payload, max_packet)
            .await?;
        Ok(())
    }

    /// Receive a PreLogin response (for use with Connection).
    #[cfg(feature = "tls")]
    async fn receive_prelogin<T>(connection: &mut Connection<T>) -> Result<PreLogin>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let message = connection
            .read_message()
            .await?
            .ok_or(Error::ConnectionClosed)?;

        Ok(PreLogin::decode(&message.payload[..])?)
    }

    /// Send a Login7 packet.
    #[cfg(feature = "tls")]
    async fn send_login7<T>(connection: &mut Connection<T>, login: &Login7) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = login.encode();
        let max_packet = MAX_PACKET_SIZE;

        connection
            .send_message(PacketType::Tds7Login, payload, max_packet)
            .await?;
        Ok(())
    }

    /// Process the login response tokens.
    ///
    /// Returns: (server_version, database, routing_info)
    #[cfg(feature = "tls")]
    async fn process_login_response<T>(
        connection: &mut Connection<T>,
    ) -> Result<(Option<u32>, Option<String>, Option<(String, u16)>)>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let message = connection
            .read_message()
            .await?
            .ok_or(Error::ConnectionClosed)?;

        let response_bytes = message.payload;

        let mut parser = TokenParser::new(response_bytes);
        let mut server_version = None;
        let mut database = None;
        let mut routing = None;

        while let Some(token) = parser.next_token()? {
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
