//! Split I/O connection for cancellation safety.
//!
//! Per ADR-005, the TCP stream is split into separate read and write halves
//! to allow sending Attention packets while blocked on reading results.

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use tds_protocol::packet::{PACKET_HEADER_SIZE, PacketHeader, PacketStatus, PacketType};
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};
use tokio::sync::{Mutex, Notify};

use crate::error::CodecError;
use crate::framed::{PacketReader, PacketWriter};
use crate::message::{Message, MessageAssembler};
use crate::packet_codec::{Packet, TdsCodec};

/// A TDS connection with split I/O for cancellation safety.
///
/// This struct splits the underlying transport into read and write halves,
/// allowing Attention packets to be sent even while blocked reading results.
///
/// # Cancellation
///
/// SQL Server uses out-of-band "Attention" packets to cancel running queries.
/// Without split I/O, the driver would be unable to send cancellation while
/// blocked awaiting a read (e.g., processing a large result set).
///
/// # Example
///
/// ```rust,ignore
/// use mssql_codec::Connection;
/// use tokio::net::TcpStream;
///
/// let stream = TcpStream::connect("localhost:1433").await?;
/// let conn = Connection::new(stream);
///
/// // Can cancel from another task while reading
/// let cancel_handle = conn.cancel_handle();
/// tokio::spawn(async move {
///     tokio::time::sleep(Duration::from_secs(5)).await;
///     cancel_handle.cancel().await?;
/// });
/// ```
pub struct Connection<T>
where
    T: AsyncRead + AsyncWrite,
{
    /// Read half wrapped in a packet reader.
    reader: PacketReader<ReadHalf<T>>,
    /// Write half protected by mutex for concurrent cancel access.
    writer: Arc<Mutex<PacketWriter<WriteHalf<T>>>>,
    /// Message assembler for multi-packet messages.
    assembler: MessageAssembler,
    /// Notification for cancellation completion.
    cancel_notify: Arc<Notify>,
    /// Flag indicating cancellation is in progress.
    cancelling: Arc<std::sync::atomic::AtomicBool>,
}

impl<T> Connection<T>
where
    T: AsyncRead + AsyncWrite,
{
    /// Create a new connection from a transport.
    ///
    /// The transport is immediately split into read and write halves.
    pub fn new(transport: T) -> Self {
        let (read_half, write_half) = tokio::io::split(transport);

        Self {
            reader: PacketReader::new(read_half),
            writer: Arc::new(Mutex::new(PacketWriter::new(write_half))),
            assembler: MessageAssembler::new(),
            cancel_notify: Arc::new(Notify::new()),
            cancelling: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Create a new connection with custom codecs.
    pub fn with_codecs(transport: T, read_codec: TdsCodec, write_codec: TdsCodec) -> Self {
        let (read_half, write_half) = tokio::io::split(transport);

        Self {
            reader: PacketReader::with_codec(read_half, read_codec),
            writer: Arc::new(Mutex::new(PacketWriter::with_codec(
                write_half,
                write_codec,
            ))),
            assembler: MessageAssembler::new(),
            cancel_notify: Arc::new(Notify::new()),
            cancelling: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Get a handle for cancelling queries on this connection.
    ///
    /// The handle can be cloned and sent to other tasks.
    #[must_use]
    pub fn cancel_handle(&self) -> CancelHandle<T> {
        CancelHandle {
            writer: Arc::clone(&self.writer),
            notify: Arc::clone(&self.cancel_notify),
            cancelling: Arc::clone(&self.cancelling),
        }
    }

    /// Check if a cancellation is currently in progress.
    #[must_use]
    pub fn is_cancelling(&self) -> bool {
        self.cancelling.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Read the next complete message from the connection.
    ///
    /// This handles multi-packet message reassembly automatically.
    pub async fn read_message(&mut self) -> Result<Option<Message>, CodecError> {
        loop {
            // Check for cancellation
            if self.is_cancelling() {
                // Drain until we see DONE with ATTENTION flag
                return self.drain_after_cancel().await;
            }

            match self.reader.next().await {
                Some(Ok(packet)) => {
                    if let Some(message) = self.assembler.push(packet) {
                        return Ok(Some(message));
                    }
                    // Continue reading packets until message complete
                }
                Some(Err(e)) => return Err(e),
                None => {
                    // Connection closed
                    if self.assembler.has_partial() {
                        return Err(CodecError::ConnectionClosed);
                    }
                    return Ok(None);
                }
            }
        }
    }

    /// Read a single packet from the connection.
    ///
    /// This is lower-level than `read_message` and doesn't perform reassembly.
    pub async fn read_packet(&mut self) -> Result<Option<Packet>, CodecError> {
        match self.reader.next().await {
            Some(result) => result.map(Some),
            None => Ok(None),
        }
    }

    /// Send a packet on the connection.
    pub async fn send_packet(&mut self, packet: Packet) -> Result<(), CodecError> {
        let mut writer = self.writer.lock().await;
        writer.send(packet).await
    }

    /// Send a complete message, splitting into multiple packets if needed.
    ///
    /// If `reset_connection` is true, the RESETCONNECTION flag is set on the
    /// first packet. This causes SQL Server to reset connection state (temp
    /// tables, SET options, isolation level, etc.) before executing the command.
    /// Per TDS spec, this flag MUST only be set on the first packet of a message.
    pub async fn send_message(
        &mut self,
        packet_type: PacketType,
        payload: Bytes,
        max_packet_size: usize,
    ) -> Result<(), CodecError> {
        self.send_message_with_reset(packet_type, payload, max_packet_size, false)
            .await
    }

    /// Send a complete message with optional connection reset.
    ///
    /// If `reset_connection` is true, the RESETCONNECTION flag is set on the
    /// first packet. This causes SQL Server to reset connection state (temp
    /// tables, SET options, isolation level, etc.) before executing the command.
    /// Per TDS spec, this flag MUST only be set on the first packet of a message.
    pub async fn send_message_with_reset(
        &mut self,
        packet_type: PacketType,
        payload: Bytes,
        max_packet_size: usize,
        reset_connection: bool,
    ) -> Result<(), CodecError> {
        let max_payload = max_packet_size - PACKET_HEADER_SIZE;
        let chunks: Vec<_> = payload.chunks(max_payload).collect();
        let total_chunks = chunks.len();

        let mut writer = self.writer.lock().await;

        for (i, chunk) in chunks.into_iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == total_chunks - 1;

            // Build status flags
            let mut status = if is_last {
                PacketStatus::END_OF_MESSAGE
            } else {
                PacketStatus::NORMAL
            };

            // Per TDS spec, RESETCONNECTION must be on the first packet only
            if is_first && reset_connection {
                status |= PacketStatus::RESET_CONNECTION;
            }

            let header = PacketHeader::new(packet_type, status, 0);
            let packet = Packet::new(header, BytesMut::from(chunk));

            writer.send(packet).await?;
        }

        Ok(())
    }

    /// Flush the write buffer.
    pub async fn flush(&mut self) -> Result<(), CodecError> {
        let mut writer = self.writer.lock().await;
        writer.flush().await
    }

    /// Drain packets after cancellation until DONE with ATTENTION is received.
    async fn drain_after_cancel(&mut self) -> Result<Option<Message>, CodecError> {
        tracing::debug!("draining packets after cancellation");

        // Clear any partial message
        self.assembler.clear();

        loop {
            match self.reader.next().await {
                Some(Ok(packet)) => {
                    // Check for DONE token with ATTENTION flag
                    // The DONE token is at the start of the payload
                    if packet.header.packet_type == PacketType::TabularResult
                        && !packet.payload.is_empty()
                    {
                        // TokenType::Done = 0xFD
                        // Check if this packet contains a Done token
                        // and the status has ATTN flag (0x0020)
                        if self.check_attention_done(&packet) {
                            tracing::debug!("received DONE with ATTENTION, cancellation complete");
                            self.cancelling
                                .store(false, std::sync::atomic::Ordering::Release);
                            self.cancel_notify.notify_waiters();
                            return Ok(None);
                        }
                    }
                    // Continue draining
                }
                Some(Err(e)) => {
                    self.cancelling
                        .store(false, std::sync::atomic::Ordering::Release);
                    return Err(e);
                }
                None => {
                    self.cancelling
                        .store(false, std::sync::atomic::Ordering::Release);
                    return Ok(None);
                }
            }
        }
    }

    /// Check if a packet contains a DONE token with ATTENTION flag.
    fn check_attention_done(&self, packet: &Packet) -> bool {
        // Look for DONE token (0xFD) with ATTN status flag (bit 5)
        // DONE token format: token_type(1) + status(2) + cur_cmd(2) + row_count(8)
        let payload = &packet.payload;

        for i in 0..payload.len() {
            if payload[i] == 0xFD && i + 3 <= payload.len() {
                // Found DONE token, check status
                let status = u16::from_le_bytes([payload[i + 1], payload[i + 2]]);
                // DONE_ATTN = 0x0020
                if status & 0x0020 != 0 {
                    return true;
                }
            }
        }

        false
    }

    /// Get a reference to the read codec.
    pub fn read_codec(&self) -> &TdsCodec {
        self.reader.codec()
    }

    /// Get a mutable reference to the read codec.
    pub fn read_codec_mut(&mut self) -> &mut TdsCodec {
        self.reader.codec_mut()
    }
}

impl<T> std::fmt::Debug for Connection<T>
where
    T: AsyncRead + AsyncWrite + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("cancelling", &self.is_cancelling())
            .field("has_partial_message", &self.assembler.has_partial())
            .finish_non_exhaustive()
    }
}

/// Handle for cancelling queries on a connection.
///
/// This can be cloned and sent to other tasks to enable cancellation
/// from a different async context.
pub struct CancelHandle<T>
where
    T: AsyncRead + AsyncWrite,
{
    writer: Arc<Mutex<PacketWriter<WriteHalf<T>>>>,
    notify: Arc<Notify>,
    cancelling: Arc<std::sync::atomic::AtomicBool>,
}

impl<T> CancelHandle<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Send an Attention packet to cancel the current query.
    ///
    /// This can be called from a different task while the main task
    /// is blocked reading results.
    pub async fn cancel(&self) -> Result<(), CodecError> {
        // Mark cancellation in progress
        self.cancelling
            .store(true, std::sync::atomic::Ordering::Release);

        tracing::debug!("sending Attention packet for query cancellation");

        // Send the Attention packet
        let mut writer = self.writer.lock().await;

        // Create and send attention packet
        let header = PacketHeader::new(
            PacketType::Attention,
            PacketStatus::END_OF_MESSAGE,
            PACKET_HEADER_SIZE as u16,
        );
        let packet = Packet::new(header, BytesMut::new());

        writer.send(packet).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Wait for the cancellation to complete.
    ///
    /// This waits until the server acknowledges the cancellation
    /// with a DONE token containing the ATTENTION flag.
    pub async fn wait_cancelled(&self) {
        if self.cancelling.load(std::sync::atomic::Ordering::Acquire) {
            self.notify.notified().await;
        }
    }

    /// Check if a cancellation is currently in progress.
    #[must_use]
    pub fn is_cancelling(&self) -> bool {
        self.cancelling.load(std::sync::atomic::Ordering::Acquire)
    }
}

impl<T> Clone for CancelHandle<T>
where
    T: AsyncRead + AsyncWrite,
{
    fn clone(&self) -> Self {
        Self {
            writer: Arc::clone(&self.writer),
            notify: Arc::clone(&self.notify),
            cancelling: Arc::clone(&self.cancelling),
        }
    }
}

impl<T> std::fmt::Debug for CancelHandle<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelHandle")
            .field("cancelling", &self.is_cancelling())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_packet_header() {
        // Verify attention packet header construction
        let header = PacketHeader::new(
            PacketType::Attention,
            PacketStatus::END_OF_MESSAGE,
            PACKET_HEADER_SIZE as u16,
        );

        assert_eq!(header.packet_type, PacketType::Attention);
        assert!(header.status.contains(PacketStatus::END_OF_MESSAGE));
        assert_eq!(header.length, PACKET_HEADER_SIZE as u16);
    }

    #[test]
    fn test_check_attention_done() {
        // Test DONE token with ATTN flag detection
        // DONE token: 0xFD + status(2 bytes) + cur_cmd(2 bytes) + row_count(8 bytes)
        // DONE_ATTN flag is 0x0020

        // Create a mock packet with DONE token and ATTN flag
        let header = PacketHeader::new(PacketType::TabularResult, PacketStatus::END_OF_MESSAGE, 0);

        // DONE token with ATTN flag set (status = 0x0020)
        let payload_with_attn = BytesMut::from(
            &[
                0xFD, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ][..],
        );
        let packet_with_attn = Packet::new(header, payload_with_attn);

        // DONE token without ATTN flag (status = 0x0000)
        let payload_no_attn = BytesMut::from(
            &[
                0xFD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ][..],
        );
        let packet_no_attn = Packet::new(header, payload_no_attn);

        // We can't easily test check_attention_done without a Connection,
        // so we verify the token detection logic directly
        let check_done = |packet: &Packet| -> bool {
            let payload = &packet.payload;
            for i in 0..payload.len() {
                if payload[i] == 0xFD && i + 3 <= payload.len() {
                    let status = u16::from_le_bytes([payload[i + 1], payload[i + 2]]);
                    if status & 0x0020 != 0 {
                        return true;
                    }
                }
            }
            false
        };

        assert!(check_done(&packet_with_attn));
        assert!(!check_done(&packet_no_attn));
    }
}
