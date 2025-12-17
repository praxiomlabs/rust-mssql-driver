//! TDS PreLogin wrapper for TLS handshake.
//!
//! In TDS 7.x, the TLS handshake is wrapped inside TDS PreLogin packets.
//! This wrapper intercepts TLS traffic during the handshake and wraps/unwraps
//! the TDS packet framing.

use std::cmp;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// TDS packet header size.
const HEADER_SIZE: usize = 8;

/// TDS packet type for PreLogin.
const PACKET_TYPE_PRELOGIN: u8 = 0x12;

/// TDS packet status for end of message.
const PACKET_STATUS_EOM: u8 = 0x01;

/// Wrapper for TLS streams that handles TDS packet framing during handshake.
///
/// During the TLS handshake phase, this wrapper:
/// - Wraps outgoing TLS data in TDS PreLogin packets
/// - Unwraps incoming TDS PreLogin packets before passing to TLS
///
/// After handshake is complete, it becomes a transparent pass-through.
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
    /// Create a new TLS prelogin wrapper.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            pending_handshake: true,
            header_buf: [0u8; HEADER_SIZE],
            header_pos: 0,
            read_remaining: 0,
            write_buf: vec![0u8; HEADER_SIZE], // Pre-allocate header space
            write_pos: HEADER_SIZE, // Start after header
            header_written: false,
        }
    }

    /// Mark the handshake as complete.
    ///
    /// After this is called, the wrapper becomes a transparent pass-through.
    pub fn handshake_complete(&mut self) {
        self.pending_handshake = false;
    }

    /// Get a reference to the underlying stream.
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Get a mutable reference to the underlying stream.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Consume the wrapper and return the underlying stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for TlsPreloginWrapper<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        // After handshake, pass through directly
        if !this.pending_handshake {
            return Pin::new(&mut this.stream).poll_read(cx, buf);
        }

        // During handshake, we need to read and unwrap TDS packets

        // First, read the header if we haven't yet
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

        // Parse the header to get payload length
        if this.read_remaining == 0 {
            // Verify this is a PreLogin packet
            let packet_type = this.header_buf[0];
            if packet_type != PACKET_TYPE_PRELOGIN {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected PreLogin packet (0x12), got 0x{:02X}", packet_type),
                )));
            }

            // Length is big-endian u16 at bytes 2-3
            let length = u16::from_be_bytes([this.header_buf[2], this.header_buf[3]]) as usize;
            this.read_remaining = length.saturating_sub(HEADER_SIZE);

            tracing::trace!("TLS wrapper: reading {} bytes of payload", this.read_remaining);
        }

        // Read the payload (TLS data)
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

                    // If we've read all data for this packet, reset for next packet
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

        // After handshake, pass through directly
        if !this.pending_handshake {
            return Pin::new(&mut this.stream).poll_write(cx, buf);
        }

        // During handshake, buffer the data (we'll wrap it on flush)
        this.write_buf.extend_from_slice(buf);

        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        // If in handshake mode and we have buffered data, wrap it in a TDS packet
        if this.pending_handshake && this.write_buf.len() > HEADER_SIZE {
            if !this.header_written {
                // Write the TDS header at the beginning of the buffer
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

                tracing::trace!("TLS wrapper: sending {} bytes", total_length);
            }

            // Write all buffered data
            while this.write_pos < this.write_buf.len() {
                match Pin::new(&mut this.stream).poll_write(cx, &this.write_buf[this.write_pos..])? {
                    Poll::Ready(n) => {
                        this.write_pos += n;
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            // Reset for next write
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
