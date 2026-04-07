//! TLS support for the mock TDS server.
//!
//! Generates self-signed certificates at runtime using `rcgen` and provides
//! a `tokio-rustls` server acceptor for TLS handshakes.

use std::sync::Arc;

use rcgen::{CertifiedKey, KeyPair, generate_simple_self_signed};
use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

/// Generate a self-signed certificate and private key for testing.
///
/// The certificate is valid for `localhost` and `127.0.0.1`.
pub fn generate_test_certificate() -> CertifiedKey<KeyPair> {
    let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    generate_simple_self_signed(subject_alt_names)
        .expect("failed to generate self-signed certificate")
}

/// Create a `TlsAcceptor` from a self-signed certificate for mock server use.
pub fn create_tls_acceptor(cert_key: &CertifiedKey<KeyPair>) -> TlsAcceptor {
    // Ensure the ring crypto provider is installed (same as mssql-tls)
    ensure_crypto_provider();

    let cert_der = cert_key.cert.der().clone();
    let key_der = PrivateKeyDer::try_from(cert_key.signing_key.serialize_der())
        .expect("failed to parse private key DER");

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("failed to create TLS server config");

    // Disable TLS 1.3 session tickets to prevent post-handshake
    // NewSessionTicket messages from being sent through the PreLogin
    // wrapper while it's still in handshake mode.
    config.send_tls13_tickets = 0;

    TlsAcceptor::from(Arc::new(config))
}

/// Ensure the ring crypto provider is installed for rustls.
fn ensure_crypto_provider() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Ensure crypto provider is installed (public for test use).
pub fn ensure_crypto_provider_for_test() {
    ensure_crypto_provider();
}

/// Perform a server-side TLS handshake over a TDS PreLogin-wrapped stream.
///
/// In TDS 7.x, TLS handshake data is wrapped inside TDS PreLogin packets
/// (packet type 0x12). This function:
/// 1. Wraps the stream with `TlsPreloginWrapper` for the handshake
/// 2. Performs the TLS accept through the wrapper
/// 3. Returns the TLS stream with the wrapper in pass-through mode
///
/// After return, the wrapper is transparent and data flows directly
/// between TLS and the underlying stream.
pub async fn accept_tls_prelogin<S>(
    stream: S,
    acceptor: &TlsAcceptor,
) -> std::io::Result<TlsStream<TlsPreloginWrapper<S>>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let wrapper = TlsPreloginWrapper::new(stream);
    let mut tls_stream = acceptor.accept(wrapper).await?;
    // Mark handshake complete so the wrapper becomes pass-through
    tls_stream.get_mut().0.handshake_complete();
    Ok(tls_stream)
}

/// Perform a server-side TLS handshake directly (no PreLogin wrapping).
///
/// This bypasses TDS PreLogin packet wrapping and does a raw TLS handshake.
/// Used as a simpler alternative when the client also does raw TLS (e.g., for
/// TDS 8.0 strict mode testing).
pub async fn accept_tls_direct<S>(
    stream: S,
    acceptor: &TlsAcceptor,
) -> std::io::Result<TlsStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    acceptor.accept(stream).await
}

// =============================================================================
// Server-side PreLogin Wrapper
// =============================================================================
// This is a mirror of mssql_tls::TlsPreloginWrapper, duplicated here to
// avoid making mssql-tls a dependency of the test crate. The logic is
// identical: during handshake, TLS bytes are wrapped/unwrapped in TDS
// PreLogin packets (type 0x12); after handshake, it's transparent.

use std::cmp;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;

const HEADER_SIZE: usize = 8;
const PACKET_TYPE_PRELOGIN: u8 = 0x12;
const PACKET_STATUS_EOM: u8 = 0x01;

/// TDS PreLogin wrapper for server-side TLS handshake framing.
pub struct TlsPreloginWrapper<S> {
    stream: S,
    pending_handshake: bool,

    // Read state
    header_buf: [u8; HEADER_SIZE],
    header_pos: usize,
    read_remaining: usize,

    // Write state
    write_buf: Vec<u8>,
    write_pos: usize,
    header_written: bool,
}

impl<S> TlsPreloginWrapper<S> {
    /// Create a new wrapper around a stream.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            pending_handshake: true,
            header_buf: [0u8; HEADER_SIZE],
            header_pos: 0,
            read_remaining: 0,
            write_buf: vec![0u8; HEADER_SIZE],
            write_pos: HEADER_SIZE,
            header_written: false,
        }
    }

    /// Mark the handshake as complete; wrapper becomes transparent pass-through.
    pub fn handshake_complete(&mut self) {
        self.pending_handshake = false;
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for TlsPreloginWrapper<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        if !this.pending_handshake {
            return Pin::new(&mut this.stream).poll_read(cx, buf);
        }

        // Read TDS header first
        while this.header_pos < HEADER_SIZE {
            let mut header_buf = ReadBuf::new(&mut this.header_buf[this.header_pos..]);
            match Pin::new(&mut this.stream).poll_read(cx, &mut header_buf)? {
                Poll::Ready(()) => {
                    let n = header_buf.filled().len();
                    if n == 0 {
                        return Poll::Ready(Ok(()));
                    }
                    this.header_pos += n;
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        // Parse header to get payload length
        if this.read_remaining == 0 {
            let length = u16::from_be_bytes([this.header_buf[2], this.header_buf[3]]) as usize;
            this.read_remaining = length.saturating_sub(HEADER_SIZE);
        }

        // Read payload (TLS data)
        let max_read = cmp::min(this.read_remaining, buf.remaining());
        if max_read == 0 {
            return Poll::Ready(Ok(()));
        }

        let mut temp_buf = vec![0u8; max_read];
        let mut temp_read_buf = ReadBuf::new(&mut temp_buf);

        match Pin::new(&mut this.stream).poll_read(cx, &mut temp_read_buf)? {
            Poll::Ready(()) => {
                let n = temp_read_buf.filled().len();
                if n > 0 {
                    buf.put_slice(&temp_buf[..n]);
                    this.read_remaining -= n;
                    if this.read_remaining == 0 {
                        this.header_pos = 0;
                    }
                }
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for TlsPreloginWrapper<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();

        if !this.pending_handshake {
            return Pin::new(&mut this.stream).poll_write(cx, buf);
        }

        // Buffer TLS data; it will be wrapped and flushed in poll_flush
        this.write_buf.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        if this.pending_handshake && this.write_buf.len() > HEADER_SIZE {
            if !this.header_written {
                let total_length = this.write_buf.len();
                this.write_buf[0] = PACKET_TYPE_PRELOGIN;
                this.write_buf[1] = PACKET_STATUS_EOM;
                this.write_buf[2] = (total_length >> 8) as u8;
                this.write_buf[3] = total_length as u8;
                this.write_buf[4] = 0; // SPID
                this.write_buf[5] = 0; // SPID
                this.write_buf[6] = 1; // Packet ID
                this.write_buf[7] = 0; // Window

                this.header_written = true;
                this.write_pos = 0;
            }

            while this.write_pos < this.write_buf.len() {
                match Pin::new(&mut this.stream)
                    .poll_write(cx, &this.write_buf[this.write_pos..])?
                {
                    Poll::Ready(n) => {
                        this.write_pos += n;
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            this.write_buf.truncate(HEADER_SIZE);
            this.write_pos = HEADER_SIZE;
            this.header_written = false;
        }

        Pin::new(&mut this.stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}
