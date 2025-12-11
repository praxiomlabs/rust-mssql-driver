//! TLS connector for establishing encrypted connections.

use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector as TokioTlsConnector;
use tokio_rustls::client::TlsStream;

use crate::config::{TlsConfig, TlsVersion};
use crate::error::TlsError;

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
        // Build root certificate store
        let root_store = Self::build_root_store(config)?;

        // Select protocol versions
        let versions: Vec<&'static rustls::SupportedProtocolVersion> =
            Self::select_versions(config);

        // Build the client config
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
