//! Framed packet stream for async I/O.
//!
//! This module provides both combined and split stream types:
//! - `PacketStream<T>` - Combined read/write stream for bidirectional I/O
//! - `PacketReader<T>` - Read-only stream for receiving packets
//! - `PacketWriter<T>` - Write-only sink for sending packets
//!
//! The split types are used by `Connection` for cancellation safety (ADR-005).

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;
use futures_core::Stream;
use futures_util::Sink;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedRead, FramedWrite};

use crate::error::CodecError;
use crate::packet_codec::{Packet, TdsCodec};

pin_project! {
    /// A framed packet stream over an async I/O transport.
    ///
    /// This wraps a tokio-util `Framed` codec and provides a higher-level
    /// interface for sending and receiving TDS packets.
    pub struct PacketStream<T> {
        #[pin]
        inner: Framed<T, TdsCodec>,
    }
}

impl<T> PacketStream<T>
where
    T: AsyncRead + AsyncWrite,
{
    /// Create a new packet stream over the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            inner: Framed::new(transport, TdsCodec::new()),
        }
    }

    /// Create a new packet stream with a custom codec.
    pub fn with_codec(transport: T, codec: TdsCodec) -> Self {
        Self {
            inner: Framed::new(transport, codec),
        }
    }

    /// Get a reference to the underlying transport.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the underlying transport.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Get a reference to the codec.
    pub fn codec(&self) -> &TdsCodec {
        self.inner.codec()
    }

    /// Get a mutable reference to the codec.
    pub fn codec_mut(&mut self) -> &mut TdsCodec {
        self.inner.codec_mut()
    }

    /// Consume the stream and return the underlying transport.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Get a reference to the read buffer.
    pub fn read_buffer(&self) -> &BytesMut {
        self.inner.read_buffer()
    }

    /// Get a mutable reference to the read buffer.
    pub fn read_buffer_mut(&mut self) -> &mut BytesMut {
        self.inner.read_buffer_mut()
    }
}

impl<T> Stream for PacketStream<T>
where
    T: AsyncRead + Unpin,
{
    type Item = Result<Packet, CodecError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl<T> Sink<Packet> for PacketStream<T>
where
    T: AsyncWrite + Unpin,
{
    type Error = CodecError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

impl<T> std::fmt::Debug for PacketStream<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PacketStream")
            .field("transport", self.inner.get_ref())
            .finish()
    }
}

// =============================================================================
// Split stream types for cancellation safety (ADR-005)
// =============================================================================

pin_project! {
    /// A read-only packet stream for receiving TDS packets.
    ///
    /// This is used for the read half of a split connection, enabling
    /// cancellation safety per ADR-005.
    pub struct PacketReader<T> {
        #[pin]
        inner: FramedRead<T, TdsCodec>,
    }
}

impl<T> PacketReader<T>
where
    T: AsyncRead,
{
    /// Create a new packet reader over the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            inner: FramedRead::new(transport, TdsCodec::new()),
        }
    }

    /// Create a new packet reader with a custom codec.
    pub fn with_codec(transport: T, codec: TdsCodec) -> Self {
        Self {
            inner: FramedRead::new(transport, codec),
        }
    }

    /// Get a reference to the underlying transport.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the underlying transport.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Get a reference to the codec.
    pub fn codec(&self) -> &TdsCodec {
        self.inner.decoder()
    }

    /// Get a mutable reference to the codec.
    pub fn codec_mut(&mut self) -> &mut TdsCodec {
        self.inner.decoder_mut()
    }

    /// Get a reference to the read buffer.
    pub fn read_buffer(&self) -> &BytesMut {
        self.inner.read_buffer()
    }

    /// Get a mutable reference to the read buffer.
    pub fn read_buffer_mut(&mut self) -> &mut BytesMut {
        self.inner.read_buffer_mut()
    }
}

impl<T> Stream for PacketReader<T>
where
    T: AsyncRead + Unpin,
{
    type Item = Result<Packet, CodecError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl<T> std::fmt::Debug for PacketReader<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PacketReader")
            .field("transport", self.inner.get_ref())
            .finish()
    }
}

pin_project! {
    /// A write-only packet sink for sending TDS packets.
    ///
    /// This is used for the write half of a split connection, enabling
    /// cancellation safety per ADR-005.
    pub struct PacketWriter<T> {
        #[pin]
        inner: FramedWrite<T, TdsCodec>,
    }
}

impl<T> PacketWriter<T>
where
    T: AsyncWrite,
{
    /// Create a new packet writer over the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            inner: FramedWrite::new(transport, TdsCodec::new()),
        }
    }

    /// Create a new packet writer with a custom codec.
    pub fn with_codec(transport: T, codec: TdsCodec) -> Self {
        Self {
            inner: FramedWrite::new(transport, codec),
        }
    }

    /// Get a reference to the underlying transport.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the underlying transport.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Get a reference to the codec.
    pub fn codec(&self) -> &TdsCodec {
        self.inner.encoder()
    }

    /// Get a mutable reference to the codec.
    pub fn codec_mut(&mut self) -> &mut TdsCodec {
        self.inner.encoder_mut()
    }
}

impl<T> Sink<Packet> for PacketWriter<T>
where
    T: AsyncWrite + Unpin,
{
    type Error = CodecError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

impl<T> std::fmt::Debug for PacketWriter<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PacketWriter")
            .field("transport", self.inner.get_ref())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tds_protocol::packet::{PacketHeader, PacketStatus, PacketType};

    /// Issue #165: `PacketStream` (Sink + Stream over `TdsCodec`) must
    /// round-trip a packet through a transport — the framing adapter that had
    /// no direct tests.
    #[tokio::test]
    async fn test_packet_stream_round_trip() {
        let (a, b) = tokio::io::duplex(4096);
        let mut writer = PacketStream::new(a);
        let mut reader = PacketStream::new(b);

        let header = PacketHeader::new(PacketType::SqlBatch, PacketStatus::END_OF_MESSAGE, 0);
        let sent = Packet::new(header, BytesMut::from(&b"hello"[..]));

        writer.send(sent).await.expect("send packet");

        let got = reader
            .next()
            .await
            .expect("a packet must arrive")
            .expect("decode must succeed");
        assert_eq!(got.header.packet_type, PacketType::SqlBatch);
        assert!(got.header.is_end_of_message());
        assert_eq!(&got.payload[..], b"hello");
    }

    /// Issue #165: the split `PacketWriter` → `PacketReader` halves used for
    /// cancellation-safe I/O (ADR-005) must also round-trip.
    #[tokio::test]
    async fn test_split_reader_writer_round_trip() {
        let (a, b) = tokio::io::duplex(4096);
        let mut writer = PacketWriter::new(a);
        let mut reader = PacketReader::new(b);

        let header = PacketHeader::new(PacketType::Rpc, PacketStatus::END_OF_MESSAGE, 0);
        writer
            .send(Packet::new(header, BytesMut::from(&b"world"[..])))
            .await
            .expect("send packet");

        let got = reader
            .next()
            .await
            .expect("a packet must arrive")
            .expect("decode must succeed");
        assert_eq!(got.header.packet_type, PacketType::Rpc);
        assert_eq!(&got.payload[..], b"world");
    }
}
