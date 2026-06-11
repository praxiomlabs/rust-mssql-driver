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
    ///
    /// Returns [`CodecError::Cancelled`] when the in-flight request was
    /// cancelled via Attention and the server's DONE_ATTN acknowledgement has
    /// been consumed — the connection is then clean for the next request.
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
                        // The cancel flag may have been set while this read was
                        // parked in `next()`. In that case the message belongs
                        // to the request being cancelled (the server discards
                        // it and acknowledges with DONE_ATTN), so it must not
                        // be surfaced as a response — otherwise `cancelling`
                        // stays latched and a later drain eats the *next*
                        // request's response.
                        if self.is_cancelling() {
                            if Self::payload_ends_with_attention_done(&message.payload) {
                                tracing::debug!(
                                    "received DONE with ATTENTION, cancellation complete"
                                );
                                self.finish_cancel();
                                return Err(CodecError::Cancelled);
                            }
                            tracing::debug!("discarding message from cancelled request");
                            continue;
                        }
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
        // An empty payload must still produce one header-only EOM packet:
        // `[]chunks()` yields zero chunks, which would send nothing at all and
        // leave the caller waiting for a response that never comes (issue
        // #165). A zero-length-payload message is valid TDS framing.
        let chunks: Vec<&[u8]> = if payload.is_empty() {
            vec![&[]]
        } else {
            payload.chunks(max_payload).collect()
        };
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

    /// Drain messages after cancellation until DONE with ATTENTION is received.
    ///
    /// Returns [`CodecError::Cancelled`] once the acknowledgement is consumed;
    /// the connection is then clean for the next request.
    async fn drain_after_cancel(&mut self) -> Result<Option<Message>, CodecError> {
        tracing::debug!("draining packets after cancellation");

        // Clear any partial message
        self.assembler.clear();

        loop {
            match self.reader.next().await {
                Some(Ok(packet)) => {
                    // Assemble complete messages so the acknowledgement check
                    // runs on the message trailer — a per-packet check would
                    // miss a DONE token straddling a packet boundary.
                    if let Some(message) = self.assembler.push(packet) {
                        if message.packet_type == PacketType::TabularResult
                            && Self::payload_ends_with_attention_done(&message.payload)
                        {
                            tracing::debug!("received DONE with ATTENTION, cancellation complete");
                            self.finish_cancel();
                            return Err(CodecError::Cancelled);
                        }
                        tracing::debug!("discarding message from cancelled request");
                    }
                    // Continue draining
                }
                Some(Err(e)) => {
                    self.cancelling
                        .store(false, std::sync::atomic::Ordering::Release);
                    return Err(e);
                }
                None => {
                    // EOF while waiting for the acknowledgement: the
                    // connection really is gone.
                    self.cancelling
                        .store(false, std::sync::atomic::Ordering::Release);
                    return Err(CodecError::ConnectionClosed);
                }
            }
        }
    }

    /// Mark the in-flight cancellation as acknowledged and wake waiters.
    fn finish_cancel(&self) {
        self.cancelling
            .store(false, std::sync::atomic::Ordering::Release);
        self.cancel_notify.notify_waiters();
    }

    /// Check whether a message payload terminates in a DONE token carrying
    /// the ATTN status flag (the attention acknowledgement).
    ///
    /// Every tabular response message ends with a fixed 13-byte DONE-family
    /// token (token(1) + status(2) + cur_cmd(2) + row_count(8)), and per
    /// MS-TDS 2.2.7.6 the acknowledgement is a DONE (0xFD) with DONE_ATTN as
    /// the final token of the cancelled stream. Anchoring the check to the
    /// trailer means row bytes that happen to contain `0xFD, 0x20` (entirely
    /// possible in binary/integer cell data arriving during the cancel
    /// window) cannot be mistaken for the acknowledgement — an interior byte
    /// scan was proven to clear the cancel flag early and leak the real
    /// acknowledgement into the next request.
    fn payload_ends_with_attention_done(payload: &[u8]) -> bool {
        let Some(start) = payload.len().checked_sub(13) else {
            return false;
        };
        // DONE token type = 0xFD; DONE_ATTN = 0x0020 in the LE status word.
        payload[start] == 0xFD
            && u16::from_le_bytes([payload[start + 1], payload[start + 2]]) & 0x0020 != 0
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Issue #165: sending a message with an empty payload must still emit
    /// exactly one header-only EOM packet. Previously `chunks()` on an empty
    /// payload yielded zero chunks, so nothing was sent and the caller would
    /// hang waiting for a response.
    #[tokio::test]
    async fn test_send_empty_payload_emits_one_eom_packet() {
        use tokio::io::AsyncReadExt;

        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let mut conn = Connection::new(client_io);

        conn.send_message(PacketType::SqlBatch, Bytes::new(), 4096)
            .await
            .expect("empty message should send");

        // Exactly one header-only packet (8 bytes) must arrive.
        let mut header = [0u8; PACKET_HEADER_SIZE];
        server_io
            .read_exact(&mut header)
            .await
            .expect("one header-only packet must be sent");
        assert_eq!(header[0], PacketType::SqlBatch as u8);
        assert!(
            PacketStatus::from_bits_truncate(header[1]).contains(PacketStatus::END_OF_MESSAGE),
            "the single packet must be flagged END_OF_MESSAGE"
        );
        let length = u16::from_be_bytes([header[2], header[3]]);
        assert_eq!(
            length as usize, PACKET_HEADER_SIZE,
            "length must be header-only (no payload)"
        );

        // And nothing more.
        drop(conn);
        let mut rest = Vec::new();
        server_io.read_to_end(&mut rest).await.expect("read rest");
        assert!(rest.is_empty(), "no second packet may follow");
    }

    /// Issue #165: across a multi-packet send, RESET_CONNECTION must be set on
    /// the first packet only (per MS-TDS), and END_OF_MESSAGE on the last only.
    #[tokio::test]
    async fn test_reset_flag_on_first_packet_only_across_multi_packet_send() {
        use tokio::io::AsyncReadExt;

        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let mut conn = Connection::new(client_io);

        // max_packet_size 16 → max_payload 8; a 12-byte payload spans 2 packets.
        let payload = Bytes::from(vec![0xABu8; 12]);
        conn.send_message_with_reset(PacketType::SqlBatch, payload, 16, true)
            .await
            .expect("multi-packet send should succeed");
        drop(conn);

        let mut all = Vec::new();
        server_io.read_to_end(&mut all).await.expect("read packets");

        // Packet 1: header(8) + payload(8) = 16 bytes.
        let s0 = PacketStatus::from_bits_truncate(all[1]);
        assert!(
            s0.contains(PacketStatus::RESET_CONNECTION),
            "first packet must carry RESET_CONNECTION"
        );
        assert!(
            !s0.contains(PacketStatus::END_OF_MESSAGE),
            "first packet of two must not be END_OF_MESSAGE"
        );

        // Packet 2 starts at offset 16: header(8) + payload(4).
        let s1 = PacketStatus::from_bits_truncate(all[16 + 1]);
        assert!(
            !s1.contains(PacketStatus::RESET_CONNECTION),
            "RESET_CONNECTION must not repeat on later packets"
        );
        assert!(
            s1.contains(PacketStatus::END_OF_MESSAGE),
            "last packet must be END_OF_MESSAGE"
        );
        assert_eq!(all.len(), 16 + 8 + 4, "exactly two packets must be sent");
    }

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

        assert!(
            Connection::<tokio::io::DuplexStream>::payload_ends_with_attention_done(
                &packet_with_attn.payload
            )
        );
        assert!(
            !Connection::<tokio::io::DuplexStream>::payload_ends_with_attention_done(
                &packet_no_attn.payload
            )
        );

        // Interior 0xFD,0x20 bytes (e.g. row data) must not register: only
        // the trailing token position counts.
        let mut interior = vec![0xD1, 0x08, 0xFD, 0x20, 0xAA, 0xBB];
        interior.extend_from_slice(&[
            0xFD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        assert!(
            !Connection::<tokio::io::DuplexStream>::payload_ends_with_attention_done(&interior)
        );
    }

    /// Build a raw single-packet TabularResult TDS message around `payload`.
    fn raw_message(payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x04, 0x01]; // TabularResult, END_OF_MESSAGE
        v.extend_from_slice(&((payload.len() + 8) as u16).to_be_bytes());
        v.extend_from_slice(&[0, 0, 1, 0]); // spid, packet id, window
        v.extend_from_slice(payload);
        v
    }

    /// DONE token bytes with the given status.
    fn done_token(status: u16) -> [u8; 13] {
        let s = status.to_le_bytes();
        [
            0xFD, s[0], s[1], 0xC1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    }

    /// Regression test for the cancel-mid-read race.
    ///
    /// When `cancel()` fires while `read_message()` is already parked on the
    /// socket, the cancelled request's response stream (here: DONE(ERROR)
    /// followed by the DONE(ATTN) acknowledgement) arrives through the
    /// *normal* read path. It must be discarded — not surfaced as a query
    /// response — and the read must end in `CodecError::Cancelled` with the
    /// `cancelling` flag cleared, so the next request's response is delivered
    /// intact. Before the fix, the first DONE was returned as the response,
    /// the flag stayed latched, and a later drain ate the next response.
    #[tokio::test]
    async fn test_cancel_mid_read_discards_cancelled_stream() {
        use std::task::{Context, Poll};
        use tokio::io::AsyncWriteExt;

        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let mut conn = Connection::new(client_io);
        let cancel = conn.cancel_handle();

        // Park a read with nothing to deliver yet (mimics waiting on a slow
        // query). A noop waker is fine: the future is re-polled via `.await`
        // below after data is written.
        let mut read_fut = Box::pin(conn.read_message());
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        assert!(matches!(read_fut.as_mut().poll(&mut cx), Poll::Pending));

        // Cancel while the read is parked, then deliver the cancelled
        // request's stream plus the next request's response.
        cancel.cancel().await.expect("send attention");
        server_io
            .write_all(&raw_message(&done_token(0x0002))) // DONE_ERROR
            .await
            .unwrap();
        server_io
            .write_all(&raw_message(&done_token(0x0020))) // DONE_ATTN ack
            .await
            .unwrap();
        server_io
            .write_all(&raw_message(&done_token(0x0010))) // next response
            .await
            .unwrap();

        let result = read_fut.await;
        assert!(
            matches!(result, Err(CodecError::Cancelled)),
            "parked read must consume the cancelled stream and report \
             Cancelled, got {result:?}"
        );
        assert!(!conn.is_cancelling(), "cancel flag must be cleared");

        // The next request's response must come through untouched.
        let message = conn
            .read_message()
            .await
            .expect("next read")
            .expect("next message");
        assert_eq!(message.payload[0], 0xFD);
        assert_eq!(
            u16::from_le_bytes([message.payload[1], message.payload[2]]),
            0x0010,
            "next response must not be eaten by a stale drain"
        );
    }

    /// Cancellation requested before the read starts takes the drain path and
    /// must behave identically to the mid-read race.
    #[tokio::test]
    async fn test_cancel_before_read_drains_to_attention_ack() {
        use tokio::io::AsyncWriteExt;

        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let mut conn = Connection::new(client_io);
        let cancel = conn.cancel_handle();

        cancel.cancel().await.expect("send attention");
        server_io
            .write_all(&raw_message(&done_token(0x0022))) // ERROR | ATTN ack
            .await
            .unwrap();
        server_io
            .write_all(&raw_message(&done_token(0x0010))) // next response
            .await
            .unwrap();

        let result = conn.read_message().await;
        assert!(matches!(result, Err(CodecError::Cancelled)));
        assert!(!conn.is_cancelling());

        let message = conn
            .read_message()
            .await
            .expect("next read")
            .expect("next message");
        assert_eq!(
            u16::from_le_bytes([message.payload[1], message.payload[2]]),
            0x0010
        );
    }

    /// PR #143 review, Blocker 1: row bytes that happen to contain
    /// `0xFD, 0x20` must NOT be mistaken for the DONE_ATTN acknowledgement.
    ///
    /// During the cancel window the cancelled request's *data* (rows already
    /// in flight) can arrive before the real acknowledgement. A byte-scan
    /// for any interior 0xFD with bit 5 set false-positives on such data,
    /// clears the cancel flag early, and the genuine ack then poisons the
    /// next request — the exact failure the cancellation fix claims to
    /// eliminate.
    #[tokio::test]
    async fn test_cancel_race_row_bytes_do_not_fake_the_attention_ack() {
        use std::task::{Context, Poll};
        use tokio::io::AsyncWriteExt;

        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let mut conn = Connection::new(client_io);
        let cancel = conn.cancel_handle();

        // Park a read, then cancel while it waits (the realistic ordering).
        let mut read_fut = Box::pin(conn.read_message());
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        assert!(matches!(read_fut.as_mut().poll(&mut cx), Poll::Pending));
        cancel.cancel().await.expect("send attention");

        // Message 1: the cancelled request's data — row-ish bytes whose
        // *interior* contains 0xFD followed by a byte with bit 5 set (e.g. a
        // BIGINT cell value), terminated by a DONE with MORE and no ATTN.
        let mut row_data = vec![0xD1, 0x08]; // ROW token, length-ish prefix
        row_data.extend_from_slice(&[0xFD, 0x20, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        row_data.extend_from_slice(&done_token(0x0001)); // DONE_MORE, no ATTN
        server_io.write_all(&raw_message(&row_data)).await.unwrap();

        // Message 2: the genuine acknowledgement.
        server_io
            .write_all(&raw_message(&done_token(0x0020)))
            .await
            .unwrap();

        // Message 3: the next request's response.
        server_io
            .write_all(&raw_message(&done_token(0x0010)))
            .await
            .unwrap();

        let result = read_fut.await;
        assert!(
            matches!(result, Err(CodecError::Cancelled)),
            "cancelled read must end in Cancelled, got {result:?}"
        );
        assert!(!conn.is_cancelling());

        // The next read must deliver message 3 — not the stale ack from
        // message 2.
        let message = conn
            .read_message()
            .await
            .expect("next read")
            .expect("next message");
        let status = u16::from_le_bytes([message.payload[1], message.payload[2]]);
        assert_eq!(
            status, 0x0010,
            "next request's response must come through intact; 0x0020 means \
             the interior row bytes were mistaken for the ack and the real \
             ack leaked into the next request"
        );
    }
}
