//! TDS message reassembly.
//!
//! TDS messages can span multiple packets. This module handles reassembling
//! packets into complete messages based on the `END_OF_MESSAGE` status flag.

// Allow expect() on Option that is guaranteed to be Some based on prior logic
#![allow(clippy::expect_used)]

use bytes::{Bytes, BytesMut};
use tds_protocol::packet::{PacketStatus, PacketType};

use crate::packet_codec::Packet;

/// A complete TDS message reassembled from one or more packets.
#[derive(Debug, Clone)]
pub struct Message {
    /// The packet type of this message.
    pub packet_type: PacketType,
    /// The complete message payload (all packets combined).
    pub payload: Bytes,
}

impl Message {
    /// Create a new message from a single packet.
    #[must_use]
    pub fn from_packet(packet: Packet) -> Self {
        Self {
            packet_type: packet.header.packet_type,
            payload: packet.payload.freeze(),
        }
    }

    /// Get the message payload length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    /// Check if the message is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

/// Reassembles multiple TDS packets into complete messages.
///
/// TDS messages are framed with the `END_OF_MESSAGE` status flag on the final
/// packet. This assembler buffers packets until a complete message is received.
#[derive(Debug)]
pub struct MessageAssembler {
    /// Buffer for accumulating packet payloads.
    buffer: BytesMut,
    /// Packet type of the message being assembled.
    packet_type: Option<PacketType>,
    /// Number of packets accumulated.
    packet_count: usize,
}

impl MessageAssembler {
    /// Create a new message assembler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
            packet_type: None,
            packet_count: 0,
        }
    }

    /// Create a new message assembler with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
            packet_type: None,
            packet_count: 0,
        }
    }

    /// Push a packet into the assembler.
    ///
    /// Returns `Some(Message)` if this packet completes a message,
    /// `None` if more packets are needed.
    pub fn push(&mut self, packet: Packet) -> Option<Message> {
        // Record the packet type from the first packet
        if self.packet_type.is_none() {
            self.packet_type = Some(packet.header.packet_type);
        }

        // Append payload to buffer
        self.buffer.extend_from_slice(&packet.payload);
        self.packet_count += 1;

        tracing::trace!(
            packet_type = ?packet.header.packet_type,
            packet_count = self.packet_count,
            buffer_len = self.buffer.len(),
            is_eom = packet.header.status.contains(PacketStatus::END_OF_MESSAGE),
            "assembling message"
        );

        // Check if this is the last packet
        if packet.header.status.contains(PacketStatus::END_OF_MESSAGE) {
            let message = Message {
                packet_type: self.packet_type.take().expect("packet_type set above"),
                payload: self.buffer.split().freeze(),
            };
            self.packet_count = 0;
            Some(message)
        } else {
            None
        }
    }

    /// Check if the assembler has partial data buffered.
    #[must_use]
    pub fn has_partial(&self) -> bool {
        self.packet_type.is_some()
    }

    /// Get the number of packets accumulated so far.
    #[must_use]
    pub fn packet_count(&self) -> usize {
        self.packet_count
    }

    /// Get the current buffer length.
    #[must_use]
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Clear any partial message data.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.packet_type = None;
        self.packet_count = 0;
    }
}

impl Default for MessageAssembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tds_protocol::packet::PacketHeader;

    fn make_packet(is_eom: bool, payload: &[u8]) -> Packet {
        let status = if is_eom {
            PacketStatus::END_OF_MESSAGE
        } else {
            PacketStatus::NORMAL
        };
        let header = PacketHeader::new(PacketType::TabularResult, status, 0);
        Packet::new(header, BytesMut::from(payload))
    }

    #[test]
    fn test_single_packet_message() {
        let mut assembler = MessageAssembler::new();
        let packet = make_packet(true, b"hello");

        let message = assembler.push(packet).expect("should complete message");
        assert_eq!(message.packet_type, PacketType::TabularResult);
        assert_eq!(&message.payload[..], b"hello");
        assert!(!assembler.has_partial());
    }

    #[test]
    fn test_multi_packet_message() {
        let mut assembler = MessageAssembler::new();

        // First packet - not EOM
        let packet1 = make_packet(false, b"hello ");
        assert!(assembler.push(packet1).is_none());
        assert!(assembler.has_partial());
        assert_eq!(assembler.packet_count(), 1);

        // Second packet - not EOM
        let packet2 = make_packet(false, b"world");
        assert!(assembler.push(packet2).is_none());
        assert_eq!(assembler.packet_count(), 2);

        // Third packet - EOM
        let packet3 = make_packet(true, b"!");
        let message = assembler.push(packet3).expect("should complete message");

        assert_eq!(message.packet_type, PacketType::TabularResult);
        assert_eq!(&message.payload[..], b"hello world!");
        assert!(!assembler.has_partial());
        assert_eq!(assembler.packet_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut assembler = MessageAssembler::new();

        let packet = make_packet(false, b"partial");
        assembler.push(packet);
        assert!(assembler.has_partial());

        assembler.clear();
        assert!(!assembler.has_partial());
        assert_eq!(assembler.buffer_len(), 0);
    }
}
