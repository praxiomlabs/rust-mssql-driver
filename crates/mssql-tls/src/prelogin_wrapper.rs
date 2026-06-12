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

/// Packet size cap before login completes (MS-TDS: 4096 until a larger size
/// is negotiated, which cannot happen during the PRELOGIN TLS handshake).
const HANDSHAKE_PACKET_SIZE: usize = 4096;

/// TLS payload per handshake packet.
const MAX_HANDSHAKE_PAYLOAD: usize = HANDSHAKE_PACKET_SIZE - HEADER_SIZE;

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
            write_pos: HEADER_SIZE,            // Start after header
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
                    format!("Expected PreLogin packet (0x12), got 0x{packet_type:02X}"),
                )));
            }

            // Length is big-endian u16 at bytes 2-3
            let length = u16::from_be_bytes([this.header_buf[2], this.header_buf[3]]) as usize;
            this.read_remaining = length.saturating_sub(HEADER_SIZE);

            tracing::trace!(
                "TLS wrapper: reading {} bytes of payload",
                this.read_remaining
            );
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

        // If in handshake mode and we have buffered data, wrap it in TDS packets
        if this.pending_handshake && this.write_buf.len() > HEADER_SIZE {
            if !this.header_written {
                // The buffered TLS flight can exceed one packet (e.g. a
                // client-certificate chain). Pre-login packets are capped at
                // 4096 bytes — and the header length field is a u16, so a
                // single oversized packet would silently truncate the length
                // (flight mod 65536) and corrupt the framing. Split the
                // flight into as many PRELOGIN packets as needed, each a
                // complete EOM message.
                let payload = this.write_buf.split_off(HEADER_SIZE);
                let packets = payload.len().div_ceil(MAX_HANDSHAKE_PAYLOAD);
                let mut framed = Vec::with_capacity(payload.len() + packets * HEADER_SIZE);
                for (i, chunk) in payload.chunks(MAX_HANDSHAKE_PAYLOAD).enumerate() {
                    let total = HEADER_SIZE + chunk.len();
                    framed.push(PACKET_TYPE_PRELOGIN);
                    framed.push(PACKET_STATUS_EOM);
                    framed.push((total >> 8) as u8);
                    framed.push(total as u8);
                    framed.push(0); // SPID
                    framed.push(0); // SPID
                    framed.push((i as u8).wrapping_add(1)); // Packet ID (mod 256)
                    framed.push(0); // Window
                    framed.extend_from_slice(chunk);
                }
                this.write_buf = framed;
                this.write_pos = 0;
                this.header_written = true;

                tracing::trace!(
                    payload_bytes = payload.len(),
                    packets,
                    "TLS wrapper: sending handshake flight"
                );
            }

            // Write all buffered data
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

            // Reset for the next flight: restore the reserved header prefix
            // that poll_write appends payload after.
            this.write_buf.clear();
            this.write_buf.resize(HEADER_SIZE, 0);
            this.write_pos = HEADER_SIZE;
            this.header_written = false;
        }

        Pin::new(&mut this.stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Parse `(type, status, packet_id, payload)` for each TDS packet.
    fn parse_packets(mut bytes: &[u8]) -> Vec<(u8, u8, u8, Vec<u8>)> {
        let mut packets = Vec::new();
        while !bytes.is_empty() {
            let total = usize::from(bytes[2]) << 8 | usize::from(bytes[3]);
            let (packet, rest) = bytes.split_at(total);
            packets.push((
                packet[0],
                packet[1],
                packet[6],
                packet[HEADER_SIZE..].to_vec(),
            ));
            bytes = rest;
        }
        packets
    }

    #[tokio::test]
    async fn small_flight_is_one_prelogin_packet() {
        let (client, mut server) = tokio::io::duplex(1 << 20);
        let mut wrapper = TlsPreloginWrapper::new(client);
        let payload: Vec<u8> = (0..100u8).collect();
        wrapper.write_all(&payload).await.unwrap();
        wrapper.flush().await.unwrap();

        let mut received = vec![0u8; payload.len() + HEADER_SIZE];
        server.read_exact(&mut received).await.unwrap();
        let packets = parse_packets(&received);
        assert_eq!(packets.len(), 1);
        let (ptype, status, id, data) = &packets[0];
        assert_eq!(*ptype, PACKET_TYPE_PRELOGIN);
        assert_eq!(*status, PACKET_STATUS_EOM);
        assert_eq!(*id, 1);
        assert_eq!(*data, payload);
    }

    /// Issue #167 regression: a handshake flight larger than 65535 bytes
    /// (realistic with TLS client-certificate chains) used to be framed as
    /// ONE packet whose u16 length field silently truncated to
    /// `flight mod 65536`, corrupting the stream. The flight must be split
    /// at the 4096-byte pre-login packet cap.
    #[tokio::test]
    async fn oversized_flight_splits_at_packet_cap() {
        let (client, mut server) = tokio::io::duplex(1 << 20);
        let mut wrapper = TlsPreloginWrapper::new(client);
        let payload: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
        wrapper.write_all(&payload).await.unwrap();
        wrapper.flush().await.unwrap();

        let expected_packets = payload.len().div_ceil(MAX_HANDSHAKE_PAYLOAD);
        let mut received = vec![0u8; payload.len() + expected_packets * HEADER_SIZE];
        server.read_exact(&mut received).await.unwrap();

        let packets = parse_packets(&received);
        assert_eq!(packets.len(), expected_packets);
        let mut reassembled = Vec::new();
        for (i, (ptype, status, id, data)) in packets.iter().enumerate() {
            assert_eq!(*ptype, PACKET_TYPE_PRELOGIN);
            assert_eq!(
                *status, PACKET_STATUS_EOM,
                "each chunk is its own complete EOM message"
            );
            assert_eq!(*id, (i as u8).wrapping_add(1));
            assert!(data.len() + HEADER_SIZE <= HANDSHAKE_PACKET_SIZE);
            reassembled.extend_from_slice(data);
        }
        assert_eq!(reassembled, payload, "no bytes lost or reordered");
    }

    #[tokio::test]
    async fn consecutive_flights_reuse_the_wrapper_cleanly() {
        // The reset path after a flight must leave the buffer ready for the
        // next poll_write (header space reserved), or the second flight's
        // framing corrupts.
        let (client, mut server) = tokio::io::duplex(1 << 20);
        let mut wrapper = TlsPreloginWrapper::new(client);

        for round in 0..3u8 {
            let payload = vec![round; 50];
            wrapper.write_all(&payload).await.unwrap();
            wrapper.flush().await.unwrap();

            let mut received = vec![0u8; payload.len() + HEADER_SIZE];
            server.read_exact(&mut received).await.unwrap();
            let packets = parse_packets(&received);
            assert_eq!(packets.len(), 1);
            assert_eq!(packets[0].3, payload, "round {round} payload intact");
        }
    }
}
