//! TLS connector for establishing encrypted connections.

use std::sync::{Arc, Once};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector as TokioTlsConnector;
use tokio_rustls::client::TlsStream;

use crate::config::{TlsConfig, TlsVersion};
use crate::error::TlsError;

// =============================================================================
// Crypto Provider Initialization
// =============================================================================

/// Ensure the ring crypto provider is installed for rustls.
/// This is called automatically when creating a TLS connector.
static CRYPTO_PROVIDER_INIT: Once = Once::new();

fn ensure_crypto_provider() {
    CRYPTO_PROVIDER_INIT.call_once(|| {
        // Install the ring crypto provider as the process-wide default.
        // This is required for rustls 0.23+ which doesn't auto-select a provider.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

// =============================================================================
// Dangerous Certificate Verifier (for TrustServerCertificate=true)
// =============================================================================

/// A certificate verifier that accepts any server certificate.
///
/// **WARNING:** This is insecure and should only be used for development/testing.
/// Using this verifier exposes the connection to man-in-the-middle attacks.
#[derive(Debug)]
struct DangerousServerCertVerifier;

impl ServerCertVerifier for DangerousServerCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        // Accept any certificate without validation
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Support all common signature schemes
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

// =============================================================================
// Default TLS Configuration (per ARCHITECTURE.md §5.1)
// =============================================================================

/// Create a secure default TLS client configuration.
///
/// Server certificates are validated against the bundled Mozilla root
/// certificate store, and no client authentication is configured.
///
/// With the `native-certs` feature enabled, validation is delegated to the
/// OS/platform trust store instead, so servers chaining to an enterprise
/// internal CA installed in the OS store are accepted.
///
/// # Example
///
/// ```no_run
/// use mssql_tls::default_tls_config;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = default_tls_config()?;
/// # Ok(())
/// # }
/// ```
pub fn default_tls_config() -> Result<ClientConfig, TlsError> {
    // Ensure the crypto provider is installed before using rustls
    ensure_crypto_provider();

    #[cfg(feature = "native-certs")]
    {
        use rustls_platform_verifier::BuilderVerifierExt;
        ClientConfig::builder()
            .with_platform_verifier()
            .map(|builder| builder.with_no_client_auth())
            .map_err(|e| {
                TlsError::Configuration(format!("platform certificate verifier init failed: {e}"))
            })
    }

    #[cfg(not(feature = "native-certs"))]
    {
        let root_store = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };

        Ok(ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth())
    }
}

// =============================================================================
// TLS Connector
// =============================================================================

/// TLS connector for SQL Server connections.
///
/// This handles both TDS 7.x style (TLS after pre-login) and TDS 8.0
/// strict mode (TLS before any TDS traffic).
pub struct TlsConnector {
    config: TlsConfig,
    inner: TokioTlsConnector,
}

impl TlsConnector {
    /// Create a new TLS connector with the given configuration.
    pub fn new(config: TlsConfig) -> Result<Self, TlsError> {
        let client_config = Self::build_client_config(&config)?;
        let inner = TokioTlsConnector::from(Arc::new(client_config));

        Ok(Self { config, inner })
    }

    /// Build the rustls client configuration.
    fn build_client_config(config: &TlsConfig) -> Result<ClientConfig, TlsError> {
        // Ensure the crypto provider is installed before using rustls
        ensure_crypto_provider();

        // Select protocol versions
        let versions: Vec<&'static rustls::SupportedProtocolVersion> =
            Self::select_versions(config);

        // Reject TrustServerCertificate in strict mode — TDS 8.0 mandates
        // certificate validation to provide its security guarantees.
        if config.strict_mode && config.trust_server_certificate {
            return Err(TlsError::Configuration(
                "TrustServerCertificate=true is not allowed in TDS 8.0 strict mode. \
                 Strict mode requires server certificate validation to prevent \
                 man-in-the-middle attacks."
                    .into(),
            ));
        }

        // Handle TrustServerCertificate mode (dangerous - development only)
        if config.trust_server_certificate {
            tracing::warn!(
                "TrustServerCertificate is enabled - certificate validation is DISABLED. \
                 This is insecure and should only be used for development/testing. \
                 Connections are vulnerable to man-in-the-middle attacks."
            );

            let mut client_config = ClientConfig::builder_with_protocol_versions(&versions)
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(DangerousServerCertVerifier))
                .with_no_client_auth();

            if !config.alpn_protocols.is_empty() {
                client_config.alpn_protocols = config.alpn_protocols.clone();
            }

            return Ok(client_config);
        }

        // Select the server-certificate verifier. With the `native-certs`
        // feature and no explicit roots, this delegates to the OS/platform
        // trust store; otherwise it uses the configured (or bundled) roots.
        let builder = Self::builder_with_verifier(&versions, config)?;

        let mut client_config = if let Some(client_auth) = &config.client_auth {
            // Clone the key by matching on the Arc contents
            let key = match client_auth.key.as_ref() {
                rustls::pki_types::PrivateKeyDer::Pkcs1(key) => {
                    rustls::pki_types::PrivateKeyDer::Pkcs1(key.clone_key())
                }
                rustls::pki_types::PrivateKeyDer::Sec1(key) => {
                    rustls::pki_types::PrivateKeyDer::Sec1(key.clone_key())
                }
                rustls::pki_types::PrivateKeyDer::Pkcs8(key) => {
                    rustls::pki_types::PrivateKeyDer::Pkcs8(key.clone_key())
                }
                _ => {
                    return Err(TlsError::Configuration(
                        "unsupported private key format".into(),
                    ));
                }
            };

            builder
                .with_client_auth_cert(client_auth.certificates.clone(), key)
                .map_err(|e| TlsError::Configuration(format!("client auth setup failed: {e}")))?
        } else {
            builder.with_no_client_auth()
        };

        // Apply ALPN protocols (required for TDS 8.0 strict mode: "tds/8.0")
        if !config.alpn_protocols.is_empty() {
            client_config.alpn_protocols = config.alpn_protocols.clone();
        }

        Ok(client_config)
    }

    /// Build a rustls client-config builder with the server-certificate
    /// verifier installed, positioned for client-auth selection.
    ///
    /// With the `native-certs` feature enabled and no explicit
    /// `root_certificates` configured, server verification is delegated to the
    /// OS/platform trust store (so enterprise internal CAs are honored).
    /// Otherwise the configured roots — or the bundled Mozilla roots when none
    /// are given — are used. Explicit `root_certificates` always take
    /// precedence over the OS store.
    fn builder_with_verifier(
        versions: &[&'static rustls::SupportedProtocolVersion],
        config: &TlsConfig,
    ) -> Result<rustls::ConfigBuilder<ClientConfig, rustls::client::WantsClientCert>, TlsError>
    {
        let builder = ClientConfig::builder_with_protocol_versions(versions);

        #[cfg(feature = "native-certs")]
        if config.root_certificates.is_empty() {
            use rustls_platform_verifier::BuilderVerifierExt;
            return builder.with_platform_verifier().map_err(|e| {
                TlsError::Configuration(format!("platform certificate verifier init failed: {e}"))
            });
        }

        let root_store = Self::build_root_store(config)?;
        Ok(builder.with_root_certificates(root_store))
    }

    /// Build the root certificate store.
    fn build_root_store(config: &TlsConfig) -> Result<RootCertStore, TlsError> {
        let mut root_store = RootCertStore::empty();

        if config.trust_server_certificate {
            // When trusting all certificates, we still need a root store
            // but we'll use a custom verifier later
            // For now, add the bundled webpki (Mozilla) roots as a fallback
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        } else if config.root_certificates.is_empty() {
            // Use the bundled webpki (Mozilla) root certificates
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        } else {
            // Use custom root certificates
            for cert in &config.root_certificates {
                root_store
                    .add(cert.clone())
                    .map_err(|e| TlsError::InvalidCertificate(e.to_string()))?;
            }
        }

        Ok(root_store)
    }

    /// Select TLS protocol versions based on configuration.
    fn select_versions(config: &TlsConfig) -> Vec<&'static rustls::SupportedProtocolVersion> {
        let mut versions = Vec::new();

        if config.min_protocol_version <= TlsVersion::Tls12
            && config.max_protocol_version >= TlsVersion::Tls12
        {
            versions.push(&rustls::version::TLS12);
        }

        if config.min_protocol_version <= TlsVersion::Tls13
            && config.max_protocol_version >= TlsVersion::Tls13
        {
            versions.push(&rustls::version::TLS13);
        }

        if versions.is_empty() {
            // Fallback to TLS 1.2 if no versions match
            versions.push(&rustls::version::TLS12);
        }

        versions
    }

    /// Connect and perform TLS handshake over the given stream.
    ///
    /// # Arguments
    ///
    /// * `stream` - The underlying TCP stream
    /// * `server_name` - The server hostname for SNI and certificate validation
    pub async fn connect<S>(&self, stream: S, server_name: &str) -> Result<TlsStream<S>, TlsError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let server_name = self.config.server_name.as_deref().unwrap_or(server_name);

        let dns_name = ServerName::try_from(server_name.to_string()).map_err(|_| {
            TlsError::HostnameVerification {
                expected: server_name.to_string(),
                actual: "invalid DNS name".to_string(),
            }
        })?;

        tracing::debug!(server_name = %server_name, "performing TLS handshake");

        let tls_stream = self
            .inner
            .connect(dns_name, stream)
            .await
            .map_err(|e| TlsError::HandshakeFailed(e.to_string()))?;

        tracing::debug!("TLS handshake completed successfully");

        Ok(tls_stream)
    }

    /// Connect and perform TLS handshake with TDS PreLogin wrapping (TDS 7.x style).
    ///
    /// In TDS 7.x, the TLS handshake is wrapped inside TDS PreLogin packets.
    /// This method handles that wrapping automatically.
    ///
    /// # Arguments
    ///
    /// * `stream` - The underlying TCP stream
    /// * `server_name` - The server hostname for SNI and certificate validation
    ///
    /// # Returns
    ///
    /// A TLS stream wrapped around a PreLogin wrapper. After the handshake completes,
    /// the wrapper becomes a transparent pass-through.
    pub async fn connect_with_prelogin<S>(
        &self,
        stream: S,
        server_name: &str,
    ) -> Result<TlsStream<crate::TlsPreloginWrapper<S>>, TlsError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let server_name = self.config.server_name.as_deref().unwrap_or(server_name);

        let dns_name = ServerName::try_from(server_name.to_string()).map_err(|_| {
            TlsError::HostnameVerification {
                expected: server_name.to_string(),
                actual: "invalid DNS name".to_string(),
            }
        })?;

        tracing::debug!(server_name = %server_name, "performing TLS handshake (PreLogin wrapped)");

        // Wrap the stream in a PreLogin wrapper
        let wrapper = crate::TlsPreloginWrapper::new(stream);

        let mut tls_stream = self
            .inner
            .connect(dns_name, wrapper)
            .await
            .map_err(|e| TlsError::HandshakeFailed(e.to_string()))?;

        // Mark the handshake as complete so the wrapper becomes pass-through
        // get_mut() returns (&mut IO, &mut ClientConnection), so access .0 for the wrapper
        tls_stream.get_mut().0.handshake_complete();

        tracing::debug!("TLS handshake completed successfully (PreLogin wrapped)");

        Ok(tls_stream)
    }

    /// Check if this connector is configured for TDS 8.0 strict mode.
    #[must_use]
    pub fn is_strict_mode(&self) -> bool {
        self.config.strict_mode
    }

    /// Get the underlying configuration.
    #[must_use]
    pub fn config(&self) -> &TlsConfig {
        &self.config
    }
}

impl std::fmt::Debug for TlsConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConnector")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn setup_crypto_provider() {
        // Install the ring crypto provider for tests
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    #[test]
    fn test_default_config() {
        setup_crypto_provider();
        let config = TlsConfig::default();
        let connector = TlsConnector::new(config);
        assert!(connector.is_ok());
    }

    #[test]
    fn test_trust_server_certificate() {
        setup_crypto_provider();
        let config = TlsConfig::new().trust_server_certificate(true);
        let connector = TlsConnector::new(config).unwrap();
        assert!(!connector.is_strict_mode());
    }

    #[test]
    fn test_strict_mode() {
        setup_crypto_provider();
        let config = TlsConfig::new().strict_mode(true);
        let connector = TlsConnector::new(config).unwrap();
        assert!(connector.is_strict_mode());
    }

    /// #314: with the `native-certs` feature, the OS/platform trust verifier
    /// must initialize successfully on the host. These prove the wiring builds
    /// a usable config; they do NOT exercise end-to-end OS-trust validation
    /// (that needs a server chaining to an OS-installed internal CA, validated
    /// manually).
    #[cfg(feature = "native-certs")]
    mod native_certs {
        use super::*;

        #[test]
        fn default_tls_config_uses_platform_verifier() {
            setup_crypto_provider();
            // Construction succeeds → the platform verifier initialized against
            // the host OS trust store.
            assert!(default_tls_config().is_ok());
        }

        #[test]
        fn connector_with_empty_roots_uses_platform_verifier() {
            setup_crypto_provider();
            // Default config has no explicit roots → platform-verifier path.
            assert!(TlsConnector::new(TlsConfig::default()).is_ok());
        }

        #[test]
        fn strict_mode_builds_with_platform_verifier() {
            setup_crypto_provider();
            // Strict mode mandates real validation; the platform verifier path
            // must compose with it.
            let connector = TlsConnector::new(TlsConfig::new().strict_mode(true)).unwrap();
            assert!(connector.is_strict_mode());
        }
    }
}
