//! TDS packet codec implementation.

use bytes::{BufMut, BytesMut};
use tds_protocol::packet::{MAX_PACKET_SIZE, PACKET_HEADER_SIZE, PacketHeader};
use tokio_util::codec::{Decoder, Encoder};

use crate::error::CodecError;

/// A TDS packet with header and payload.
#[derive(Debug, Clone)]
pub struct Packet {
    /// Packet header.
    pub header: PacketHeader,
    /// Packet payload (excluding header).
    pub payload: BytesMut,
}

impl Packet {
    /// Create a new packet with the given header and payload.
    #[must_use]
    pub fn new(header: PacketHeader, payload: BytesMut) -> Self {
        Self { header, payload }
    }

    /// Get the total packet size including header.
    #[must_use]
    pub fn total_size(&self) -> usize {
        PACKET_HEADER_SIZE + self.payload.len()
    }

    /// Check if this is the last packet in a message.
    #[must_use]
    pub fn is_end_of_message(&self) -> bool {
        self.header.is_end_of_message()
    }
}

/// TDS packet codec for tokio-util framing.
///
/// This codec handles the low-level encoding and decoding of TDS packets
/// over a byte stream.
pub struct TdsCodec {
    /// Maximum packet size to accept.
    max_packet_size: usize,
    /// Current packet sequence number for encoding.
    packet_id: u8,
}

impl TdsCodec {
    /// Create a new TDS codec with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_packet_size: MAX_PACKET_SIZE,
            packet_id: 1,
        }
    }

    /// Create a new TDS codec with a custom maximum packet size.
    #[must_use]
    pub fn with_max_packet_size(mut self, size: usize) -> Self {
        self.max_packet_size = size.min(MAX_PACKET_SIZE);
        self
    }

    /// Get the next packet ID and increment the counter.
    fn next_packet_id(&mut self) -> u8 {
        let id = self.packet_id;
        self.packet_id = self.packet_id.wrapping_add(1);
        if self.packet_id == 0 {
            self.packet_id = 1;
        }
        id
    }

    /// Reset the packet ID counter.
    pub fn reset_packet_id(&mut self) {
        self.packet_id = 1;
    }
}

impl Default for TdsCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for TdsCodec {
    type Item = Packet;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Need at least a header to proceed
        if src.len() < PACKET_HEADER_SIZE {
            return Ok(None);
        }

        // Peek at the header to get the length
        let length = u16::from_be_bytes([src[2], src[3]]) as usize;

        // Validate packet length
        if length < PACKET_HEADER_SIZE {
            return Err(CodecError::InvalidHeader);
        }
        if length > self.max_packet_size {
            return Err(CodecError::PacketTooLarge {
                size: length,
                max: self.max_packet_size,
            });
        }

        // Check if we have the complete packet
        if src.len() < length {
            // Reserve space for the full packet
            src.reserve(length - src.len());
            return Ok(None);
        }

        // Extract the packet bytes
        let packet_bytes = src.split_to(length);
        let mut cursor = packet_bytes.as_ref();

        // Parse the header
        let header = PacketHeader::decode(&mut cursor)?;

        // Extract payload
        let payload = BytesMut::from(&packet_bytes[PACKET_HEADER_SIZE..]);

        tracing::trace!(
            packet_type = ?header.packet_type,
            length = length,
            is_eom = header.is_end_of_message(),
            "decoded TDS packet"
        );

        Ok(Some(Packet::new(header, payload)))
    }
}

impl Encoder<Packet> for TdsCodec {
    type Error = CodecError;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let total_length = PACKET_HEADER_SIZE + item.payload.len();

        if total_length > self.max_packet_size {
            return Err(CodecError::PacketTooLarge {
                size: total_length,
                max: self.max_packet_size,
            });
        }

        // Reserve space
        dst.reserve(total_length);

        // Create header with correct length and packet ID
        let mut header = item.header;
        header.length = total_length as u16;
        header.packet_id = self.next_packet_id();

        // Encode header
        header.encode(dst);

        // Encode payload
        dst.put_slice(&item.payload);

        tracing::trace!(
            packet_type = ?header.packet_type,
            length = total_length,
            packet_id = header.packet_id,
            "encoded TDS packet"
        );

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tds_protocol::packet::{PacketStatus, PacketType};

    #[test]
    fn test_decode_packet() {
        let mut codec = TdsCodec::new();

        // Create a minimal packet: header (8 bytes) + 4 bytes payload
        let mut data = BytesMut::new();
        data.put_u8(PacketType::SqlBatch as u8); // type
        data.put_u8(PacketStatus::END_OF_MESSAGE.bits()); // status
        data.put_u16(12); // length (8 header + 4 payload)
        data.put_u16(0); // spid
        data.put_u8(1); // packet_id
        data.put_u8(0); // window
        data.put_slice(b"test"); // payload

        let packet = codec.decode(&mut data).unwrap().unwrap();
        assert_eq!(packet.header.packet_type, PacketType::SqlBatch);
        assert!(packet.header.is_end_of_message());
        assert_eq!(&packet.payload[..], b"test");
    }

    #[test]
    fn test_encode_packet() {
        let mut codec = TdsCodec::new();

        let header = PacketHeader::new(PacketType::SqlBatch, PacketStatus::END_OF_MESSAGE, 0);
        let payload = BytesMut::from(&b"test"[..]);
        let packet = Packet::new(header, payload);

        let mut dst = BytesMut::new();
        codec.encode(packet, &mut dst).unwrap();

        assert_eq!(dst.len(), 12); // 8 header + 4 payload
        assert_eq!(dst[0], PacketType::SqlBatch as u8);
    }

    #[test]
    fn test_incomplete_packet() {
        let mut codec = TdsCodec::new();

        // Only header, no payload
        let mut data = BytesMut::new();
        data.put_u8(PacketType::SqlBatch as u8);
        data.put_u8(PacketStatus::END_OF_MESSAGE.bits());
        data.put_u16(12); // Claims to be 12 bytes
        data.put_u16(0);
        data.put_u8(1);
        data.put_u8(0);
        // Missing 4 bytes of payload

        let result = codec.decode(&mut data).unwrap();
        assert!(result.is_none()); // Should return None for incomplete
    }
}
