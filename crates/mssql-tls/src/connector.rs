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
/// This uses the Mozilla root certificate store for server validation
/// and requires no client authentication.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_tls::default_tls_config;
///
/// let config = default_tls_config()?;
/// ```
pub fn default_tls_config() -> Result<ClientConfig, TlsError> {
    // Ensure the crypto provider is installed before using rustls
    ensure_crypto_provider();

    let root_store = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(config)
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

        // Handle TrustServerCertificate mode (dangerous - development only)
        if config.trust_server_certificate {
            tracing::warn!(
                "TrustServerCertificate is enabled - certificate validation is DISABLED. \
                 This is insecure and should only be used for development/testing. \
                 Connections are vulnerable to man-in-the-middle attacks."
            );

            let client_config = ClientConfig::builder_with_protocol_versions(&versions)
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(DangerousServerCertVerifier))
                .with_no_client_auth();

            return Ok(client_config);
        }

        // Build root certificate store for normal validation
        let root_store = Self::build_root_store(config)?;

        // Build the client config with proper certificate validation
        let builder = ClientConfig::builder_with_protocol_versions(&versions)
            .with_root_certificates(root_store);

        let client_config = if let Some(client_auth) = &config.client_auth {
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

        Ok(client_config)
    }

    /// Build the root certificate store.
    fn build_root_store(config: &TlsConfig) -> Result<RootCertStore, TlsError> {
        let mut root_store = RootCertStore::empty();

        if config.trust_server_certificate {
            // When trusting all certificates, we still need a root store
            // but we'll use a custom verifier later
            // For now, add system roots as a fallback
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        } else if config.root_certificates.is_empty() {
            // Use system root certificates
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
}
