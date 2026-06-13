//! Connection establishment for SQL Server.
//!
//! This module contains the `impl Client<Disconnected>` block, handling
//! TCP connection, TLS negotiation, PreLogin exchange, and Login7 authentication.

use std::marker::PhantomData;
use std::net::SocketAddr;

use bytes::BytesMut;
use mssql_codec::connection::Connection;
#[cfg(feature = "tls")]
use mssql_tls::{TlsConfig, TlsConnector, TlsNegotiationMode};
use tds_protocol::login7::Login7;
use tds_protocol::packet::DEFAULT_PACKET_SIZE;
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

/// Federated authentication parameters for a single LOGIN7 attempt.
///
/// `echo` mirrors the server's PRELOGIN FEDAUTHREQUIRED response, as required
/// for the `fFedAuthEcho` bit (MS-TDS §2.2.6.4).
#[derive(Clone, Copy)]
struct FedAuthLogin<'a> {
    token: &'a str,
    echo: bool,
}

impl Client<Disconnected> {
    /// Connect to SQL Server.
    ///
    /// This establishes a connection, performs TLS negotiation (if required),
    /// and authenticates with the server.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use mssql_client::Client;
    /// # async fn ex(config: mssql_client::Config) -> Result<(), mssql_client::Error> {
    /// let client = Client::connect(config).await?;
    /// # let _ = client;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(config: Config) -> Result<Client<Ready>> {
        Self::validate_credential_support(&config)?;

        // Azure AD / Entra credentials use the FEDAUTH SecurityToken workflow
        // (MS-TDS §2.2.6.4): the access token is acquired client-side before
        // any TDS traffic and sent in the LOGIN7 FEDAUTH feature extension.
        // Acquired once here so retries and Azure gateway redirects reuse it.
        let fed_auth_token = Self::resolve_fed_auth_token(&config).await?;

        let retry = config.retry.clone();
        let max_redirects = config.redirect.max_redirects;
        let follow_redirects = config.redirect.follow_redirects;
        // Overall timeout accounts for retries + redirects per attempt, capped at 5 min.
        let per_attempt = config.timeouts.connect_timeout
            + config.timeouts.tls_timeout
            + config.timeouts.login_timeout;
        let total_attempts = (retry.max_retries + 1) * (max_redirects as u32 + 1);
        let overall = (per_attempt * total_attempts).min(std::time::Duration::from_secs(300));
        let initial_host = config.host.clone();
        let initial_port = config.port;

        let result = timeout(overall, async {
            let mut last_error: Option<Error> = None;

            for retry_attempt in 0..=retry.max_retries {
                if retry_attempt > 0 {
                    let backoff = retry.backoff_for_attempt(retry_attempt);
                    tracing::info!(
                        retry_attempt,
                        backoff_ms = backoff.as_millis() as u64,
                        "retrying connection after transient error"
                    );
                    tokio::time::sleep(backoff).await;
                }

                // Each retry starts fresh with original host/port
                let mut current_config = config.clone();
                let mut redirect_count: u8 = 0;

                let attempt_result = loop {
                    redirect_count += 1;
                    if redirect_count > max_redirects + 1 {
                        break Err(Error::TooManyRedirects { max: max_redirects });
                    }

                    match Self::try_connect(&current_config, fed_auth_token.as_deref()).await {
                        Ok(client) => break Ok(client),
                        Err(Error::Routing { host, port }) => {
                            if !follow_redirects {
                                break Err(Error::Routing { host, port });
                            }
                            tracing::info!(
                                host = %host,
                                port = port,
                                redirect = redirect_count,
                                max_redirects = max_redirects,
                                "following Azure SQL routing redirect"
                            );
                            current_config = current_config.with_host(&host).with_port(port);
                            continue;
                        }
                        Err(e) => break Err(e),
                    }
                };

                match attempt_result {
                    Ok(client) => return Ok(client),
                    Err(ref e) if e.is_transient() && retry.should_retry(retry_attempt) => {
                        tracing::warn!(
                            retry_attempt,
                            max_retries = retry.max_retries,
                            error = %e,
                            "transient connection error, will retry"
                        );
                        last_error = Some(attempt_result.unwrap_err());
                    }
                    Err(e) => return Err(e),
                }
            }

            // All retries exhausted — return last error
            Err(last_error.expect("at least one attempt was made"))
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_elapsed) => Err(Error::ConnectTimeout {
                host: initial_host,
                port: initial_port,
            }),
        }
    }

    /// Validate that the configured credentials can complete a login.
    ///
    /// Fails fast with an actionable error instead of sending a login the
    /// server would reject with an opaque error 18456 (or worse, leaking a
    /// bearer token over plaintext).
    fn validate_credential_support(config: &Config) -> Result<()> {
        match &config.credentials {
            mssql_auth::Credentials::SqlServer { .. } => Ok(()),
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            mssql_auth::Credentials::Integrated => Ok(()),
            creds if creds.is_azure_ad() => {
                // A FEDAUTH login carries a bearer token; sending it over a
                // plaintext connection would hand the token to any on-path
                // observer. Azure SQL always requires TLS anyway.
                #[cfg(not(feature = "tls"))]
                {
                    return Err(Error::Config(
                        "Azure AD / Entra (FEDAUTH) authentication requires TLS: \
                         enable the 'tls' feature."
                            .into(),
                    ));
                }
                #[cfg(feature = "tls")]
                {
                    if config.no_tls {
                        return Err(Error::Config(
                            "Azure AD / Entra (FEDAUTH) authentication cannot be combined \
                             with Encrypt=no_tls: the access token would be sent in \
                             plaintext. Use Encrypt=mandatory or Encrypt=strict."
                                .into(),
                        ));
                    }
                    if matches!(&config.credentials,
                        mssql_auth::Credentials::AzureAccessToken { token } if token.is_empty())
                    {
                        return Err(Error::Config(
                            "Azure AD access token is empty (the FEDAUTH token length \
                             must not be zero)"
                                .into(),
                        ));
                    }
                    if !config.strict_mode && !config.tds_version.supports_fed_auth() {
                        return Err(Error::Config(format!(
                            "Azure AD / Entra (FEDAUTH) authentication requires TDS 7.4 \
                             or later (configured: {})",
                            config.tds_version
                        )));
                    }
                    Ok(())
                }
            }
            // Remaining credential types (client certificate) cannot complete
            // a login yet: certificate-acquired tokens are not wired into the
            // login sequence. Tracked in issue #155.
            _ => Err(Error::Config(
                "client certificate (FEDAUTH) authentication is not yet supported \
                 (tracked in https://github.com/praxiomlabs/rust-mssql-driver/issues/155). \
                 Use SQL Server, integrated, or Azure AD / Entra authentication."
                    .into(),
            )),
        }
    }

    /// Resolve the federated authentication access token, if these
    /// credentials use FEDAUTH.
    ///
    /// Pre-acquired tokens are passed through; managed identity and service
    /// principal credentials acquire a token from Entra ID (network I/O).
    /// Returns `None` for non-FEDAUTH credentials.
    async fn resolve_fed_auth_token(config: &Config) -> Result<Option<String>> {
        match &config.credentials {
            mssql_auth::Credentials::AzureAccessToken { token } => Ok(Some(token.to_string())),
            #[cfg(feature = "azure-identity")]
            mssql_auth::Credentials::AzureManagedIdentity { client_id } => {
                let auth = match client_id {
                    Some(id) => {
                        mssql_auth::ManagedIdentityAuth::user_assigned_client_id(id.to_string())?
                    }
                    None => mssql_auth::ManagedIdentityAuth::system_assigned()?,
                };
                tracing::debug!("acquiring Azure SQL access token via managed identity");
                Ok(Some(auth.get_token().await?))
            }
            #[cfg(feature = "azure-identity")]
            mssql_auth::Credentials::AzureServicePrincipal {
                tenant_id,
                client_id,
                client_secret,
            } => {
                let auth = mssql_auth::ServicePrincipalAuth::new(
                    tenant_id.as_ref(),
                    client_id.to_string(),
                    client_secret.to_string(),
                )?;
                tracing::debug!(
                    client_id = %client_id,
                    "acquiring Azure SQL access token via service principal"
                );
                Ok(Some(auth.get_token().await?))
            }
            _ => Ok(None),
        }
    }

    async fn try_connect(config: &Config, fed_auth_token: Option<&str>) -> Result<Client<Ready>> {
        // If a named instance is specified, resolve the TCP port via SQL Browser
        let port = if let Some(ref instance) = config.instance {
            let resolved = crate::browser::resolve_instance(
                &config.host,
                instance,
                Some(config.timeouts.connect_timeout),
            )
            .await?;
            tracing::info!(
                host = %config.host,
                instance = %instance,
                resolved_port = resolved,
                database = ?config.database,
                "connecting to named SQL Server instance"
            );
            resolved
        } else {
            tracing::info!(
                host = %config.host,
                port = config.port,
                database = ?config.database,
                "connecting to SQL Server"
            );
            config.port
        };

        // Normalize "." and "(local)" to localhost for TCP.
        // These are standard ADO.NET aliases for the local machine.
        let host = if config.host == "." || config.host.eq_ignore_ascii_case("(local)") {
            "127.0.0.1"
        } else {
            &config.host
        };

        // Step 1: Establish TCP connection
        let tcp_stream = if config.multi_subnet_failover {
            Self::connect_parallel(host, port, config.timeouts.connect_timeout).await?
        } else {
            let addr = format!("{host}:{port}");
            tracing::debug!("establishing TCP connection to {}", addr);
            let stream = timeout(config.timeouts.connect_timeout, TcpStream::connect(&addr))
                .await
                .map_err(|_| Error::ConnectTimeout {
                    host: config.host.clone(),
                    port: config.port,
                })?
                .map_err(Error::from)?;
            stream.set_nodelay(true).map_err(Error::from)?;
            stream
        };

        #[cfg(feature = "tls")]
        {
            // Determine TLS negotiation mode
            let tls_mode = TlsNegotiationMode::from_encrypt_mode(config.strict_mode);

            // Step 2: Handle TDS 8.0 strict mode (TLS before any TDS traffic)
            if tls_mode.is_tls_first() {
                return Self::connect_tds_8(config, tcp_stream, fed_auth_token).await;
            }

            // Step 3: TDS 7.x flow - PreLogin first, then TLS, then Login7
            Self::connect_tds_7x(config, tcp_stream, fed_auth_token).await
        }

        #[cfg(not(feature = "tls"))]
        {
            // FEDAUTH credentials were rejected by validate_credential_support
            // (no TLS feature means no way to protect the bearer token).
            let _ = fed_auth_token;

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

    /// Resolve hostname to all IPs and race parallel TCP connections.
    ///
    /// Used when `MultiSubnetFailover=True` for AlwaysOn AG listeners that
    /// span multiple subnets. First successful TCP connection wins.
    async fn connect_parallel(
        host: &str,
        port: u16,
        connect_timeout: std::time::Duration,
    ) -> Result<TcpStream> {
        let addr_str = format!("{host}:{port}");
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&addr_str)
            .await
            .map_err(Error::from)?
            .collect();

        if addrs.is_empty() {
            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                format!("no addresses resolved for {host}:{port}"),
            )));
        }

        // Single address — no need to spawn tasks
        if addrs.len() == 1 {
            tracing::debug!(addr = %addrs[0], "MultiSubnetFailover: single address resolved");
            let stream = timeout(connect_timeout, TcpStream::connect(addrs[0]))
                .await
                .map_err(|_| Error::ConnectTimeout {
                    host: host.to_string(),
                    port,
                })?
                .map_err(Error::from)?;
            stream.set_nodelay(true).map_err(Error::from)?;
            return Ok(stream);
        }

        let addr_count = addrs.len();
        tracing::debug!(
            host = host,
            port = port,
            resolved_count = addr_count,
            "MultiSubnetFailover: racing parallel connections",
        );

        let mut join_set = tokio::task::JoinSet::new();

        for addr in addrs {
            let dur = connect_timeout;
            join_set.spawn(async move {
                let tcp = timeout(dur, TcpStream::connect(addr)).await.map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("connection to {addr} timed out"),
                    )
                })??;
                tcp.set_nodelay(true)?;
                Ok::<(TcpStream, SocketAddr), std::io::Error>((tcp, addr))
            });
        }

        let mut last_error: Option<std::io::Error> = None;

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok((stream, addr))) => {
                    tracing::debug!(addr = %addr, "MultiSubnetFailover: connected");
                    join_set.abort_all();
                    return Ok(stream);
                }
                Ok(Err(e)) => {
                    tracing::debug!(error = %e, "MultiSubnetFailover: attempt failed");
                    last_error = Some(e);
                }
                Err(join_err) => {
                    tracing::debug!(error = %join_err, "MultiSubnetFailover: task failed");
                    last_error = Some(std::io::Error::other(join_err.to_string()));
                }
            }
        }

        // All connections failed
        Err(Error::from(last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("all {addr_count} parallel connection attempts failed for {host}:{port}"),
            )
        })))
    }

    /// Connect using TDS 8.0 strict mode.
    ///
    /// Flow: TCP -> TLS -> PreLogin (encrypted) -> Login7 (encrypted)
    #[cfg(feature = "tls")]
    async fn connect_tds_8(
        config: &Config,
        tcp_stream: TcpStream,
        fed_auth_token: Option<&str>,
    ) -> Result<Client<Ready>> {
        tracing::debug!("using TDS 8.0 strict mode (TLS first)");

        // Build TLS configuration from the user's `config.tls` plus the
        // TDS 8.0 strict-mode requirements (see `connection_tls_config`).
        let tls_config = connection_tls_config(config, true);

        let tls_connector = TlsConnector::new(tls_config)?;

        // Perform TLS handshake before any TDS traffic
        let tls_stream = timeout(
            config.timeouts.tls_timeout,
            tls_connector.connect(tcp_stream, &config.host),
        )
        .await
        .map_err(|_| Error::TlsTimeout {
            host: config.host.clone(),
            port: config.port,
        })??;

        tracing::debug!("TLS handshake completed (strict mode)");

        // Create connection wrapper
        let mut connection = Connection::new(tls_stream);
        connection.set_max_message_size(config.max_response_size);

        // Send PreLogin (encrypted in strict mode)
        let prelogin = Self::build_prelogin(config, EncryptionLevel::Required);
        Self::send_prelogin(&mut connection, &prelogin).await?;
        let prelogin_response = Self::receive_prelogin(&mut connection).await?;

        // Create SSPI negotiator if integrated auth
        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
        let negotiator = Self::create_negotiator(config)?;
        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
        let sspi_token = match negotiator {
            Some(ref neg) => Some(neg.initialize()?),
            None => None,
        };
        #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
        let sspi_token: Option<Vec<u8>> = None;

        // Send Login7
        let fed_auth = fed_auth_token.map(|token| FedAuthLogin {
            token,
            echo: prelogin_response.fed_auth_required,
        });
        let login = Self::build_login7(config, sspi_token, fed_auth);
        Self::send_login7(&mut connection, &login).await?;

        // Process login response (with timeout to prevent hangs during redirect)
        let (server_version, current_database, routing, server_collation) = timeout(
            config.timeouts.login_timeout,
            Self::process_login_response(
                &mut connection,
                #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                negotiator.as_deref(),
            ),
        )
        .await
        .map_err(|_| Error::LoginTimeout {
            host: config.host.clone(),
            port: config.port,
        })??;

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
            server_collation,
            statement_cache: StatementCache::with_default_size(),
            transaction_descriptor: 0, // Auto-commit mode initially
            needs_reset: false,        // Fresh connection, no reset needed
            in_flight: false,          // No request pending
            #[cfg(feature = "otel")]
            instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                .with_database(current_database.clone().unwrap_or_default()),
            #[cfg(feature = "always-encrypted")]
            encryption_context: config.column_encryption.clone().map(|cfg| {
                std::sync::Arc::new(crate::encryption::EncryptionContext::from_arc(cfg))
            }),
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
    async fn connect_tds_7x(
        config: &Config,
        mut tcp_stream: TcpStream,
        fed_auth_token: Option<&str>,
    ) -> Result<Client<Ready>> {
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

        // FEDAUTH: echo the server's FEDAUTHREQUIRED response (fFedAuthEcho).
        let fed_auth = fed_auth_token.map(|token| FedAuthLogin {
            token,
            echo: prelogin_response.fed_auth_required,
        });

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
            // Upgrade to TLS with PreLogin wrapping (TDS 7.x style).
            // In TDS 7.x, the TLS handshake is wrapped inside TDS PreLogin
            // packets. Honor the user's `config.tls` (custom root certs,
            // client auth) without the TDS 8.0 strict ALPN.
            let tls_config = connection_tls_config(config, false);

            let tls_connector = TlsConnector::new(tls_config)?;

            // Use PreLogin-wrapped TLS connection for TDS 7.x
            let mut tls_stream = timeout(
                config.timeouts.tls_timeout,
                tls_connector.connect_with_prelogin(tcp_stream, &config.host),
            )
            .await
            .map_err(|_| Error::TlsTimeout {
                host: config.host.clone(),
                port: config.port,
            })??;

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

                // Create SSPI negotiator if integrated auth
                // Note: SSPI handshake over login-only encryption is limited —
                // the server response comes in plaintext, so multi-step SSPI
                // may not work. We include the initial token but don't loop.
                #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                let negotiator = Self::create_negotiator(config)?;
                #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                let sspi_token = match negotiator {
                    Some(ref neg) => Some(neg.initialize()?),
                    None => None,
                };
                #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
                let sspi_token: Option<Vec<u8>> = None;

                // Build and send Login7 directly through TLS
                let login = Self::build_login7(config, sspi_token, fed_auth);
                let login_payload = login.encode();

                // Create TDS packet manually for Login7. LOGIN7 is sent before
                // packet-size negotiation completes, so it MUST be split at the
                // 4096-byte TDS default — large FEDAUTH tokens (managed identity,
                // AAD tokens with many claims) push LOGIN7 over 4096 and the
                // server resets a single oversized packet.
                let max_packet = DEFAULT_PACKET_SIZE;
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
                connection.set_max_message_size(config.max_response_size);

                // Process login response (comes in plaintext, with timeout)
                let (server_version, current_database, routing, server_collation) = timeout(
                    config.timeouts.login_timeout,
                    Self::process_login_response(
                        &mut connection,
                        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                        negotiator.as_deref(),
                    ),
                )
                .await
                .map_err(|_| Error::LoginTimeout {
                    host: config.host.clone(),
                    port: config.port,
                })??;

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
                    server_collation,
                    statement_cache: StatementCache::with_default_size(),
                    transaction_descriptor: 0, // Auto-commit mode initially
                    needs_reset: false,        // Fresh connection, no reset needed
                    in_flight: false,          // No request pending
                    #[cfg(feature = "otel")]
                    instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                        .with_database(current_database.clone().unwrap_or_default()),
                    #[cfg(feature = "always-encrypted")]
                    encryption_context: config.column_encryption.clone().map(|cfg| {
                        std::sync::Arc::new(crate::encryption::EncryptionContext::from_arc(cfg))
                    }),
                })
            } else {
                // Full Encryption (ENCRYPT_ON per MS-TDS spec):
                // - All communication after TLS handshake goes through TLS
                let mut connection = Connection::new(tls_stream);
                connection.set_max_message_size(config.max_response_size);

                // Create SSPI negotiator if integrated auth
                #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                let negotiator = Self::create_negotiator(config)?;
                #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                let sspi_token = match negotiator {
                    Some(ref neg) => Some(neg.initialize()?),
                    None => None,
                };
                #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
                let sspi_token: Option<Vec<u8>> = None;

                // Send Login7
                let login = Self::build_login7(config, sspi_token, fed_auth);
                Self::send_login7(&mut connection, &login).await?;

                // Process login response (with timeout)
                let (server_version, current_database, routing, server_collation) = timeout(
                    config.timeouts.login_timeout,
                    Self::process_login_response(
                        &mut connection,
                        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                        negotiator.as_deref(),
                    ),
                )
                .await
                .map_err(|_| Error::LoginTimeout {
                    host: config.host.clone(),
                    port: config.port,
                })??;

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
                    server_collation,
                    statement_cache: StatementCache::with_default_size(),
                    transaction_descriptor: 0, // Auto-commit mode initially
                    needs_reset: false,        // Fresh connection, no reset needed
                    in_flight: false,          // No request pending
                    #[cfg(feature = "otel")]
                    instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                        .with_database(current_database.clone().unwrap_or_default()),
                    #[cfg(feature = "always-encrypted")]
                    encryption_context: config.column_encryption.clone().map(|cfg| {
                        std::sync::Arc::new(crate::encryption::EncryptionContext::from_arc(cfg))
                    }),
                })
            }
        } else {
            // Server does not require encryption and client doesn't either
            tracing::warn!(
                "Connecting without TLS encryption. This is insecure and should only be \
                 used for development/testing on trusted networks."
            );

            let mut connection = Connection::new(tcp_stream);

            connection.set_max_message_size(config.max_response_size);

            // Create SSPI negotiator if integrated auth
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            let negotiator = Self::create_negotiator(config)?;
            #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
            let sspi_token = match negotiator {
                Some(ref neg) => Some(neg.initialize()?),
                None => None,
            };
            #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
            let sspi_token: Option<Vec<u8>> = None;

            // Build and send Login7. `fed_auth` is provably None here: a
            // plaintext connection requires Encrypt=no_tls, which
            // validate_credential_support rejects for FEDAUTH credentials.
            let login = Self::build_login7(config, sspi_token, fed_auth);
            Self::send_login7(&mut connection, &login).await?;

            // Process login response (with timeout)
            let (server_version, current_database, routing, server_collation) = timeout(
                config.timeouts.login_timeout,
                Self::process_login_response(
                    &mut connection,
                    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                    negotiator.as_deref(),
                ),
            )
            .await
            .map_err(|_| Error::LoginTimeout {
                host: config.host.clone(),
                port: config.port,
            })??;

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
                server_collation,
                statement_cache: StatementCache::with_default_size(),
                transaction_descriptor: 0, // Auto-commit mode initially
                needs_reset: false,        // Fresh connection, no reset needed
                in_flight: false,          // No request pending
                #[cfg(feature = "otel")]
                instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                    .with_database(current_database.clone().unwrap_or_default()),
                #[cfg(feature = "always-encrypted")]
                encryption_context: config.column_encryption.clone().map(|cfg| {
                    std::sync::Arc::new(crate::encryption::EncryptionContext::from_arc(cfg))
                }),
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

        let mut connection = Connection::new(tcp_stream);

        connection.set_max_message_size(config.max_response_size);

        // Create SSPI negotiator if integrated auth
        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
        let negotiator = Self::create_negotiator(config)?;
        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
        let sspi_token = match negotiator {
            Some(ref neg) => Some(neg.initialize()?),
            None => None,
        };
        #[cfg(not(any(feature = "integrated-auth", feature = "sspi-auth")))]
        let sspi_token: Option<Vec<u8>> = None;

        // Build and send Login7 (FEDAUTH credentials were rejected by
        // validate_credential_support: no TLS feature, no token protection).
        let login = Self::build_login7(config, sspi_token, None);
        Self::send_login7(&mut connection, &login).await?;

        // Process login response (with timeout)
        let (server_version, current_database, routing, server_collation) = timeout(
            config.timeouts.login_timeout,
            Self::process_login_response(
                &mut connection,
                #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                negotiator.as_deref(),
            ),
        )
        .await
        .map_err(|_| Error::LoginTimeout {
            host: config.host.clone(),
            port: config.port,
        })??;

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
            server_collation,
            statement_cache: StatementCache::with_default_size(),
            transaction_descriptor: 0,
            needs_reset: false,
            in_flight: false,
            #[cfg(feature = "otel")]
            instrumentation: InstrumentationContext::new(config.host.clone(), config.port)
                .with_database(current_database.clone().unwrap_or_default()),
            #[cfg(feature = "always-encrypted")]
            encryption_context: config.column_encryption.clone().map(|cfg| {
                std::sync::Arc::new(crate::encryption::EncryptionContext::from_arc(cfg))
            }),
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

        // Advertise federated authentication so the server's response carries
        // the FEDAUTHREQUIRED value we must echo in LOGIN7 (fFedAuthEcho).
        if config.credentials.is_azure_ad() {
            prelogin = prelogin.with_fed_auth_required(true);
        }

        prelogin
    }

    /// Resolve the workstation ID for the LOGIN7 HostName field.
    ///
    /// Per MS-TDS, the LOGIN7 HostName field contains the client machine name
    /// (not the server name). Priority:
    /// 1. `Config::workstation_id` (explicit override)
    /// 2. Machine hostname from environment (`COMPUTERNAME` on Windows, `HOSTNAME` on Linux)
    /// 3. Empty string (fallback)
    fn resolve_workstation_id(config: &Config) -> String {
        if let Some(ref id) = config.workstation_id {
            return id.clone();
        }
        // COMPUTERNAME is set on Windows; HOSTNAME is set on most Linux systems.
        // This avoids adding a dependency for a simple lookup.
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_default()
    }

    /// Build a Login7 packet.
    ///
    /// When `sspi_token` is provided (integrated auth), the Login7 packet is
    /// built with the integrated security flag and the initial SSPI blob.
    ///
    /// When `fed_auth` is provided (Azure AD / Entra), the packet carries the
    /// FEDAUTH feature extension (SecurityToken workflow) and no username or
    /// password — per MS-TDS §2.2.6.4, `fIntSecurity` must be 0 and the
    /// credential fields stay empty.
    fn build_login7(
        config: &Config,
        sspi_token: Option<Vec<u8>>,
        fed_auth: Option<FedAuthLogin<'_>>,
    ) -> Login7 {
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
            .with_hostname(Self::resolve_workstation_id(config));

        if let Some(ref database) = config.database {
            login = login.with_database(database);
        }

        // ApplicationIntent → LOGIN7 TypeFlags READONLY_INTENT bit
        if config.application_intent == crate::config::ApplicationIntent::ReadOnly {
            login = login.with_read_only_intent(true);
        }

        // Session language → LOGIN7 Language field
        if let Some(ref lang) = config.language {
            login = login.with_language(lang);
        }

        // Set credentials
        if let Some(token) = sspi_token {
            // Integrated auth: set SSPI data and integrated security flag
            login = login.with_integrated_auth(token);
        } else if let Some(fed) = fed_auth {
            // Azure AD / Entra: FEDAUTH feature extension, SecurityToken
            // workflow. Username/password stay empty.
            login = login.with_feature(tds_protocol::login7::FeatureExtension {
                feature_id: tds_protocol::login7::FeatureId::FedAuth,
                data: mssql_auth::azure_ad::build_security_token_feature_data(fed.token, fed.echo),
            });
            tracing::debug!(
                fed_auth_echo = fed.echo,
                "Login7: adding FEDAUTH feature extension (SecurityToken workflow)"
            );
        } else if let mssql_auth::Credentials::SqlServer { username, password } =
            &config.credentials
        {
            login = login.with_sql_auth(username.as_ref(), password.as_ref());
        }

        // When Always Encrypted is configured, add the ColumnEncryption feature extension.
        // Version 1 = client supports column encryption without enclave computations.
        #[cfg(feature = "always-encrypted")]
        if config.column_encryption.is_some() {
            login = login.with_feature(tds_protocol::login7::FeatureExtension {
                feature_id: tds_protocol::login7::FeatureId::ColumnEncryption,
                data: bytes::Bytes::from_static(&[0x01]), // Version 1
            });
            tracing::debug!("Login7: adding ColumnEncryption feature extension (version 1)");
        }

        login
    }

    /// Create an SSPI/GSSAPI negotiator if integrated auth is configured.
    ///
    /// Returns `None` for non-integrated credential types.
    ///
    /// On Windows with `sspi-auth`, uses native Windows SSPI (`secur32.dll`) which
    /// supports all account types including Microsoft Accounts. Falls back to sspi-rs
    /// on non-Windows platforms.
    ///
    /// With `integrated-auth` (Linux/macOS), uses GSSAPI/Kerberos.
    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
    fn create_negotiator(config: &Config) -> Result<Option<Box<dyn mssql_auth::SspiNegotiator>>> {
        #[allow(clippy::match_like_matches_macro)]
        let is_integrated = match &config.credentials {
            mssql_auth::Credentials::Integrated => true,
            _ => false,
        };

        if !is_integrated {
            return Ok(None);
        }

        // On Windows: prefer native SSPI (secur32.dll) for integrated auth.
        // This handles all Windows account types including Microsoft Accounts,
        // domain accounts, and local accounts — unlike sspi-rs which requires
        // explicit credentials.
        #[cfg(all(windows, feature = "sspi-auth"))]
        let negotiator: Box<dyn mssql_auth::SspiNegotiator> =
            Box::new(mssql_auth::NativeSspiAuth::new(&config.host, config.port)?);

        // On non-Windows: use sspi-rs (pure Rust SSPI implementation)
        #[cfg(all(not(windows), feature = "sspi-auth"))]
        let negotiator: Box<dyn mssql_auth::SspiNegotiator> =
            Box::new(mssql_auth::SspiAuth::new(&config.host, config.port)?);

        #[cfg(all(feature = "integrated-auth", not(feature = "sspi-auth")))]
        let negotiator: Box<dyn mssql_auth::SspiNegotiator> =
            Box::new(mssql_auth::IntegratedAuth::new(&config.host, config.port));

        Ok(Some(negotiator))
    }

    /// Send a PreLogin packet (for use with Connection).
    #[cfg(feature = "tls")]
    async fn send_prelogin<T>(connection: &mut Connection<T>, prelogin: &PreLogin) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = prelogin.encode();
        // PRELOGIN is tiny and never approaches the packet limit; keep the
        // pre-fix behavior here (fully-qualified so the import stays lean for
        // no-default-features builds, matching the SSPI send site below).
        let max_packet = tds_protocol::packet::MAX_PACKET_SIZE;

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
    async fn send_login7<T>(connection: &mut Connection<T>, login: &Login7) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = login.encode();
        // LOGIN7 precedes packet-size negotiation, so it must be split at the
        // 4096-byte TDS default, not MAX_PACKET_SIZE: a large FEDAUTH token
        // makes LOGIN7 exceed 4096 and a single oversized packet is reset by
        // the server (a managed-identity token is ~1900 chars → ~4100 bytes).
        let max_packet = DEFAULT_PACKET_SIZE;

        connection
            .send_message(PacketType::Tds7Login, payload, max_packet)
            .await?;
        Ok(())
    }

    /// Process the login response tokens, handling SSPI challenge/response if needed.
    ///
    /// When a `negotiator` is provided and the server sends an SSPI challenge token,
    /// this method will automatically perform the multi-step SSPI handshake by:
    /// 1. Calling `negotiator.step(challenge)` to generate a response
    /// 2. Sending the response via an SSPI packet
    /// 3. Reading the next server message and continuing
    ///
    /// Returns: (server_version, database, routing_info)
    #[allow(clippy::never_loop)] // Loop is used when integrated-auth/sspi-auth features are enabled
    async fn process_login_response<T>(
        connection: &mut Connection<T>,
        #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))] negotiator: Option<
            &dyn mssql_auth::SspiNegotiator,
        >,
    ) -> Result<(
        Option<u32>,
        Option<String>,
        Option<(String, u16)>,
        Option<tds_protocol::token::Collation>,
    )>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let mut server_version = None;
        let mut database = None;
        let mut routing = None;
        let mut collation = None;

        'outer: loop {
            let message = connection
                .read_message()
                .await?
                .ok_or(Error::ConnectionClosed)?;

            let response_bytes = message.payload;
            let mut parser = TokenParser::new(response_bytes);

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
                        Self::process_env_change(&env, &mut database, &mut routing, &mut collation);
                    }
                    #[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
                    Token::Sspi(sspi_token) => {
                        let neg = negotiator.ok_or_else(|| {
                            Error::Protocol(
                                "server sent SSPI challenge but no negotiator is configured"
                                    .to_string(),
                            )
                        })?;

                        tracing::debug!(
                            challenge_len = sspi_token.data.len(),
                            "received SSPI challenge from server"
                        );

                        if let Some(response) = neg.step(&sspi_token.data)? {
                            tracing::debug!(response_len = response.len(), "sending SSPI response");
                            connection
                                .send_message(
                                    PacketType::Sspi,
                                    bytes::Bytes::from(response),
                                    tds_protocol::packet::MAX_PACKET_SIZE,
                                )
                                .await?;
                        }

                        // After sending the SSPI response, read the next server message
                        continue 'outer;
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
                    Token::FeatureExtAck(ack) => {
                        for feature in &ack.features {
                            tracing::debug!(
                                feature_id = feature.feature_id,
                                data_len = feature.data.len(),
                                "server acknowledged feature extension"
                            );
                        }
                    }
                    Token::Done(done) => {
                        if done.status.error {
                            return Err(Error::Protocol("login failed".to_string()));
                        }
                        break 'outer;
                    }
                    _ => {}
                }
            }

            // If we consumed all tokens without a Done or SSPI, break
            break;
        }

        Ok((server_version, database, routing, collation))
    }

    /// Process an EnvChange token.
    fn process_env_change(
        env: &EnvChange,
        database: &mut Option<String>,
        routing: &mut Option<(String, u16)>,
        collation: &mut Option<tds_protocol::token::Collation>,
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
            EnvChangeType::SqlCollation => {
                if let EnvChangeValue::Binary(ref data) = env.new_value {
                    if data.len() >= 5 {
                        let c = tds_protocol::token::Collation::from_bytes(
                            data[..5].try_into().unwrap(),
                        );
                        tracing::debug!(
                            lcid = c.lcid,
                            sort_id = c.sort_id,
                            "server collation received"
                        );
                        *collation = Some(c);
                    }
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

/// Build the TLS configuration for an outbound connection.
///
/// Starts from the user's [`Config::tls`] so custom root certificates, client
/// auth, and protocol-version bounds are honored, then layers the
/// connection-specific requirements. `trust_server_certificate` is taken from
/// the authoritative top-level [`Config`] field: both the builder and the
/// connection-string parser set it, but the parser does not mirror it into
/// `config.tls`, so reading it here is what keeps `TrustServerCertificate=...`
/// connection strings working.
///
/// `strict` selects TDS 8.0 strict mode (TLS-first) and adds the `tds/8.0`
/// ALPN protocol; TDS 7.x leaves both off (its TLS is wrapped in PreLogin).
///
/// Note the asymmetry: root certificates and client auth come from
/// `config.tls`, but `trust_server_certificate` is taken from the top-level
/// field and overrides whatever `config.tls` holds. So setting *only*
/// `config.tls = TlsConfig::new().trust_server_certificate(true)` while
/// leaving the top-level field at its `false` default does not trust the
/// server — set it via the connection string (`TrustServerCertificate=true`)
/// or `Config::trust_server_certificate(true)`, which is the supported path.
#[cfg(feature = "tls")]
fn connection_tls_config(config: &Config, strict: bool) -> TlsConfig {
    let tls = config
        .tls
        .clone()
        .trust_server_certificate(config.trust_server_certificate);
    if strict {
        tls.strict_mode(true)
            .with_alpn_protocols(vec![b"tds/8.0".to_vec()])
    } else {
        tls
    }
}

#[cfg(all(test, feature = "tls"))]
mod tls_config_tests {
    use super::*;
    use mssql_tls::CertificateDer;

    fn config_with_root(cert: Vec<u8>) -> Config {
        let mut config = Config::new();
        config.tls = config
            .tls
            .clone()
            .add_root_certificate(CertificateDer::from(cert));
        config
    }

    #[test]
    fn custom_root_certificate_reaches_connector_config() {
        // The bug: connect built a fresh TlsConfig and dropped config.tls,
        // so a custom CA was unreachable. Assert it survives into the
        // connection's TLS config, in both strict and non-strict paths.
        let config = config_with_root(vec![0xCA; 32]);

        for strict in [true, false] {
            let tls = connection_tls_config(&config, strict);
            assert_eq!(
                tls.root_certificates.len(),
                1,
                "custom root must reach the connector (strict={strict})"
            );
            assert_eq!(tls.root_certificates[0].as_ref(), &[0xCA; 32][..]);
        }
    }

    #[test]
    fn trust_server_certificate_taken_from_top_level_field() {
        // Mirrors the connection-string path, which sets the top-level field
        // without updating config.tls.
        let mut config = Config::new();
        config.trust_server_certificate = true;
        // config.tls still has the default (false) trust flag.
        assert!(!config.tls.trust_server_certificate);

        let tls = connection_tls_config(&config, false);
        assert!(
            tls.trust_server_certificate,
            "top-level trust flag must win"
        );
    }

    #[test]
    fn strict_mode_adds_tds8_alpn() {
        let config = Config::new();
        let strict = connection_tls_config(&config, true);
        assert!(strict.strict_mode);
        assert!(strict.alpn_protocols.iter().any(|p| p == b"tds/8.0"));

        let non_strict = connection_tls_config(&config, false);
        assert!(!non_strict.strict_mode);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod fed_auth_login_tests {
    use super::*;
    use tds_protocol::prelogin::EncryptionLevel;

    fn azure_config(token: &str) -> Config {
        Config::new().credentials(mssql_auth::Credentials::azure_token(token.to_string()))
    }

    /// Wire-exact assembly of the FEDAUTH feature extension inside the
    /// encoded LOGIN7, located through the ibExtension pointer indirection
    /// (MS-TDS §2.2.6.4): FeatureId 0x02, DWORD-LE data length, options byte
    /// `(SecurityToken << 1) | echo`, DWORD-LE token byte length, UTF-16LE
    /// token, 0xFF terminator. Username/password must stay empty and
    /// fIntSecurity clear.
    #[test]
    fn login7_fed_auth_feature_block_wire_exact() {
        let config = azure_config("AB");
        let login = Client::<Disconnected>::build_login7(
            &config,
            None,
            Some(FedAuthLogin {
                token: "AB",
                echo: true,
            }),
        );

        assert!(
            !login.option_flags2.integrated_security,
            "fIntSecurity MUST be 0 when FEDAUTH is present"
        );
        assert!(
            login.username.is_empty() && login.password.is_empty(),
            "FEDAUTH logins must not carry username/password"
        );

        let encoded = login.encode();

        // OptionFlags3 (byte 27) must have fExtension (0x10) set.
        assert_eq!(encoded[27] & 0x10, 0x10, "fExtension bit must be set");

        // ibExtension/cbExtension are the 6th (offset, length) pair in the
        // offset table starting at byte 36. The u32 it points to holds the
        // absolute offset of the FeatureExt block.
        const EXTENSION_SLOT: usize = 36 + 5 * 4;
        let ib_extension =
            u16::from_le_bytes([encoded[EXTENSION_SLOT], encoded[EXTENSION_SLOT + 1]]) as usize;
        let feature_off =
            u32::from_le_bytes(encoded[ib_extension..ib_extension + 4].try_into().unwrap())
                as usize;

        assert_eq!(
            encoded[feature_off], 0x02,
            "FeatureId must be FEDAUTH (0x02)"
        );
        let data_len = u32::from_le_bytes(
            encoded[feature_off + 1..feature_off + 5]
                .try_into()
                .unwrap(),
        ) as usize;
        // options(1) + token length DWORD(4) + "AB" as UTF-16LE(4)
        assert_eq!(data_len, 9, "FeatureDataLen must cover options + token");

        let data = &encoded[feature_off + 5..feature_off + 5 + data_len];
        assert_eq!(
            data,
            &[0x03, 0x04, 0x00, 0x00, 0x00, 0x41, 0x00, 0x42, 0x00],
            "options must be (SecurityToken << 1) | echo, then DWORD-LE \
             token byte length, then UTF-16LE token"
        );
        assert_eq!(
            encoded[feature_off + 5 + data_len],
            0xFF,
            "FeatureExt terminator must follow"
        );
    }

    /// The echo bit mirrors the server's PRELOGIN FEDAUTHREQUIRED response;
    /// when the server sent 0x00 the options byte must be 0x02 (echo clear).
    #[test]
    fn login7_fed_auth_echo_clear() {
        let config = azure_config("AB");
        let login = Client::<Disconnected>::build_login7(
            &config,
            None,
            Some(FedAuthLogin {
                token: "AB",
                echo: false,
            }),
        );
        let encoded = login.encode();

        const EXTENSION_SLOT: usize = 36 + 5 * 4;
        let ib_extension =
            u16::from_le_bytes([encoded[EXTENSION_SLOT], encoded[EXTENSION_SLOT + 1]]) as usize;
        let feature_off =
            u32::from_le_bytes(encoded[ib_extension..ib_extension + 4].try_into().unwrap())
                as usize;
        assert_eq!(encoded[feature_off], 0x02);
        assert_eq!(
            encoded[feature_off + 5],
            0x02,
            "options byte must have fFedAuthEcho clear"
        );
    }

    /// PRELOGIN must advertise FEDAUTHREQUIRED for Azure AD credentials and
    /// must not for SQL authentication.
    #[test]
    fn prelogin_advertises_fed_auth_for_azure_credentials() {
        let azure = azure_config("tok");
        let prelogin = Client::<Disconnected>::build_prelogin(&azure, EncryptionLevel::On);
        assert!(prelogin.fed_auth_required);

        let sql = Config::new().credentials(mssql_auth::Credentials::sql_server("u", "p"));
        let prelogin = Client::<Disconnected>::build_prelogin(&sql, EncryptionLevel::On);
        assert!(!prelogin.fed_auth_required);
    }

    /// Regression: a LOGIN7 carrying a large FEDAUTH token exceeds the 4096-byte
    /// TDS default packet size and MUST be split across multiple packets, each
    /// within 4096 bytes. Before the fix, `send_login7` passed MAX_PACKET_SIZE
    /// (65535) to `send_message` and emitted a single oversized packet, which
    /// Azure SQL reset — a managed-identity token (~1900 chars → ~4100-byte
    /// LOGIN7) tripped this every time, while smaller service-principal tokens
    /// stayed under 4096 and masked the bug. Verified live against Azure SQL.
    #[tokio::test]
    async fn login7_large_fed_auth_token_is_split_at_default_packet_size() {
        use tds_protocol::packet::PACKET_HEADER_SIZE;
        use tokio::io::AsyncReadExt;

        // ~2000-char token -> LOGIN7 comfortably over the 4096 default.
        let token = "A".repeat(2000);
        let config = azure_config(&token);
        let login = Client::<Disconnected>::build_login7(
            &config,
            None,
            Some(FedAuthLogin {
                token: &token,
                echo: true,
            }),
        );
        let encoded = login.encode();
        assert!(
            encoded.len() > DEFAULT_PACKET_SIZE,
            "precondition: LOGIN7 ({}) must exceed the default packet size to exercise splitting",
            encoded.len()
        );

        // Capture exactly what send_login7 writes to the transport.
        let (client_io, mut server_io) = tokio::io::duplex(64 * 1024);
        let mut connection = Connection::new(client_io);
        Client::<Disconnected>::send_login7(&mut connection, &login)
            .await
            .unwrap();
        drop(connection); // close the write half so read_to_end observes EOF
        let mut raw = Vec::new();
        server_io.read_to_end(&mut raw).await.unwrap();

        // Walk the TDS packets: 8-byte header, status at [1] (EOM = 0x01),
        // total length (incl. header) at [2..4] big-endian.
        let mut offset = 0;
        let mut packets = 0;
        let mut reassembled = Vec::new();
        let mut saw_eom = false;
        while offset < raw.len() {
            let status = raw[offset + 1];
            let len = u16::from_be_bytes([raw[offset + 2], raw[offset + 3]]) as usize;
            assert!(
                len <= DEFAULT_PACKET_SIZE,
                "packet {packets} length {len} exceeds the 4096-byte default"
            );
            assert!(!saw_eom, "no packet may follow the END_OF_MESSAGE packet");
            saw_eom = status & 0x01 == 0x01;
            reassembled.extend_from_slice(&raw[offset + PACKET_HEADER_SIZE..offset + len]);
            offset += len;
            packets += 1;
        }
        assert!(
            packets >= 2,
            "an oversized LOGIN7 must span multiple packets, got {packets}"
        );
        assert!(saw_eom, "the final packet must carry END_OF_MESSAGE");
        assert_eq!(
            reassembled,
            encoded.as_ref(),
            "reassembled packet payloads must equal the LOGIN7 encoding"
        );
    }
}
