//! TLS configuration options.

use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Client authentication credentials for mutual TLS.
///
/// This is wrapped in an Arc because `PrivateKeyDer` doesn't implement Clone.
#[derive(Clone)]
pub struct ClientAuth {
    /// Client certificate chain.
    pub certificates: Vec<CertificateDer<'static>>,
    /// Client private key (wrapped in Arc as it doesn't implement Clone).
    pub key: Arc<PrivateKeyDer<'static>>,
}

impl ClientAuth {
    /// Create new client authentication credentials.
    pub fn new(certificates: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) -> Self {
        Self {
            certificates,
            key: Arc::new(key),
        }
    }
}

impl std::fmt::Debug for ClientAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientAuth")
            .field("certificates_count", &self.certificates.len())
            .field("has_key", &true)
            .finish()
    }
}

/// TLS configuration for SQL Server connections.
#[derive(Clone, Debug)]
pub struct TlsConfig {
    /// Whether to trust the server certificate without validation.
    ///
    /// **Warning:** This is insecure and should only be used for testing.
    pub trust_server_certificate: bool,

    /// Custom root certificates to trust.
    ///
    /// If empty, the system root certificates are used.
    pub root_certificates: Vec<CertificateDer<'static>>,

    /// Client authentication credentials for mutual TLS (TDS 8.0 client cert auth).
    pub client_auth: Option<ClientAuth>,

    /// Server hostname for certificate validation.
    ///
    /// If not set, the connection hostname is used.
    pub server_name: Option<String>,

    /// Minimum TLS version to accept.
    pub min_protocol_version: TlsVersion,

    /// Maximum TLS version to accept.
    pub max_protocol_version: TlsVersion,

    /// Whether to use TDS 8.0 strict mode (TLS before any TDS traffic).
    pub strict_mode: bool,

    /// Application-layer protocol negotiation (ALPN) protocols.
    pub alpn_protocols: Vec<Vec<u8>>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            trust_server_certificate: false,
            root_certificates: Vec::new(),
            client_auth: None,
            server_name: None,
            min_protocol_version: TlsVersion::Tls12,
            max_protocol_version: TlsVersion::Tls13,
            strict_mode: false,
            alpn_protocols: Vec::new(),
        }
    }
}

impl TlsConfig {
    /// Create a new TLS configuration with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Trust the server certificate without validation.
    ///
    /// **Warning:** This is insecure and should only be used for testing.
    #[must_use]
    pub fn trust_server_certificate(mut self, trust: bool) -> Self {
        self.trust_server_certificate = trust;
        self
    }

    /// Add a custom root certificate to trust.
    #[must_use]
    pub fn add_root_certificate(mut self, cert: CertificateDer<'static>) -> Self {
        self.root_certificates.push(cert);
        self
    }

    /// Set custom root certificates, replacing any existing ones.
    #[must_use]
    pub fn with_root_certificates(mut self, certs: Vec<CertificateDer<'static>>) -> Self {
        self.root_certificates = certs;
        self
    }

    /// Set client certificate and key for mutual TLS.
    #[must_use]
    pub fn with_client_auth(
        mut self,
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Self {
        self.client_auth = Some(ClientAuth::new(certs, key));
        self
    }

    /// Set the server name for certificate validation.
    #[must_use]
    pub fn with_server_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = Some(name.into());
        self
    }

    /// Set the minimum TLS version.
    #[must_use]
    pub fn min_protocol_version(mut self, version: TlsVersion) -> Self {
        self.min_protocol_version = version;
        self
    }

    /// Set the maximum TLS version.
    #[must_use]
    pub fn max_protocol_version(mut self, version: TlsVersion) -> Self {
        self.max_protocol_version = version;
        self
    }

    /// Enable TDS 8.0 strict mode.
    #[must_use]
    pub fn strict_mode(mut self, enabled: bool) -> Self {
        self.strict_mode = enabled;
        self
    }

    /// Set ALPN protocols.
    #[must_use]
    pub fn with_alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        self.alpn_protocols = protocols;
        self
    }

    /// Check if client certificate authentication is configured.
    #[must_use]
    pub fn has_client_auth(&self) -> bool {
        self.client_auth.is_some()
    }
}

/// TLS protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TlsVersion {
    /// TLS 1.2
    #[default]
    Tls12,
    /// TLS 1.3
    Tls13,
}

impl TlsVersion {
    /// Convert to rustls protocol version.
    #[must_use]
    pub fn to_rustls(&self) -> &'static rustls::SupportedProtocolVersion {
        match self {
            Self::Tls12 => &rustls::version::TLS12,
            Self::Tls13 => &rustls::version::TLS13,
        }
    }
}
