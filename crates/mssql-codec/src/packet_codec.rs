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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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

    /// Build an 8-byte TDS header with an arbitrary length field (which may be
    /// intentionally invalid for testing).
    fn header_with_length(len: u16) -> BytesMut {
        let mut data = BytesMut::new();
        data.put_u8(PacketType::SqlBatch as u8);
        data.put_u8(PacketStatus::END_OF_MESSAGE.bits());
        data.put_u16(len);
        data.put_u16(0); // spid
        data.put_u8(1); // packet_id
        data.put_u8(0); // window
        data
    }

    /// Issue #165: a length field smaller than the 8-byte header is malformed
    /// and must be rejected, not silently accepted or panicked on.
    #[test]
    fn test_decode_rejects_length_below_header_size() {
        let mut codec = TdsCodec::new();
        let mut data = header_with_length(4); // < PACKET_HEADER_SIZE (8)
        assert!(matches!(
            codec.decode(&mut data),
            Err(CodecError::InvalidHeader)
        ));
    }

    /// Issue #165: a declared length above the negotiated maximum must be
    /// rejected before any allocation/read of the claimed size.
    #[test]
    fn test_decode_rejects_packet_too_large() {
        let mut codec = TdsCodec::new().with_max_packet_size(16);
        // Header claims 20 bytes; only the 8-byte header is present, but the
        // size check must fire before the completeness check.
        let mut data = header_with_length(20);
        match codec.decode(&mut data) {
            Err(CodecError::PacketTooLarge { size, max }) => {
                assert_eq!(size, 20);
                assert_eq!(max, 16);
            }
            other => panic!("expected PacketTooLarge, got {other:?}"),
        }
    }

    /// Issue #165: encoding a packet whose total length exceeds the maximum
    /// must error rather than emit a truncated/overflowing length field.
    #[test]
    fn test_encode_rejects_packet_too_large() {
        let mut codec = TdsCodec::new().with_max_packet_size(16);
        let header = PacketHeader::new(PacketType::SqlBatch, PacketStatus::END_OF_MESSAGE, 0);
        // 8-byte header + 16-byte payload = 24 > 16.
        let payload = BytesMut::from(&[0u8; 16][..]);
        let mut dst = BytesMut::new();
        match codec.encode(Packet::new(header, payload), &mut dst) {
            Err(CodecError::PacketTooLarge { size, max }) => {
                assert_eq!(size, 24);
                assert_eq!(max, 16);
            }
            other => panic!("expected PacketTooLarge, got {other:?}"),
        }
    }

    /// Issue #165: the packet-id counter wraps 255 → 1, skipping 0 (TDS
    /// packet IDs are 1-based; a 0 would be misread by the server).
    #[test]
    fn test_packet_id_wraps_past_zero() {
        let mut codec = TdsCodec::new();
        let mut saw_zero = false;
        let mut saw_wrap_to_one = false;
        let mut prev = codec.next_packet_id(); // first id (1)
        for _ in 0..600 {
            let id = codec.next_packet_id();
            if id == 0 {
                saw_zero = true;
            }
            if prev == 255 {
                assert_eq!(id, 1, "after 255 the id must skip 0 and become 1");
                saw_wrap_to_one = true;
            }
            prev = id;
        }
        assert!(!saw_zero, "packet id 0 must never be emitted");
        assert!(saw_wrap_to_one, "the test must exercise the 255→1 wrap");
    }

    /// Issue #165: two complete packets concatenated in one buffer must both
    /// decode, with the buffer fully consumed.
    #[test]
    fn test_decode_two_packets_from_one_buffer() {
        let mut codec = TdsCodec::new();
        let mut data = BytesMut::new();
        for tag in [b"aaaa", b"bbbb"] {
            data.put_u8(PacketType::SqlBatch as u8);
            data.put_u8(PacketStatus::END_OF_MESSAGE.bits());
            data.put_u16(12);
            data.put_u16(0);
            data.put_u8(1);
            data.put_u8(0);
            data.put_slice(tag);
        }

        let p1 = codec.decode(&mut data).unwrap().expect("first packet");
        assert_eq!(&p1.payload[..], b"aaaa");
        let p2 = codec.decode(&mut data).unwrap().expect("second packet");
        assert_eq!(&p2.payload[..], b"bbbb");
        assert!(data.is_empty(), "buffer must be fully consumed");
        assert!(codec.decode(&mut data).unwrap().is_none());
    }

    /// Issue #165: a packet arriving in two reads (partial header, then the
    /// rest) must decode once the full packet is present.
    #[test]
    fn test_decode_incremental_feed() {
        let mut codec = TdsCodec::new();
        let mut full = header_with_length(12);
        full.put_slice(b"test");

        // Feed only the first 5 bytes (partial header).
        let mut data = BytesMut::new();
        data.put_slice(&full[..5]);
        assert!(codec.decode(&mut data).unwrap().is_none());

        // Feed the remaining bytes; now it decodes.
        data.put_slice(&full[5..]);
        let p = codec
            .decode(&mut data)
            .unwrap()
            .expect("packet after full feed");
        assert_eq!(&p.payload[..], b"test");
        assert!(data.is_empty());
    }
}
