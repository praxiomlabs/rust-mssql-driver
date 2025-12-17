//! TDS packet header definitions.

use bitflags::bitflags;
use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::ProtocolError;

/// TDS packet header size in bytes.
pub const PACKET_HEADER_SIZE: usize = 8;

/// Maximum TDS packet size (64KB - 1).
pub const MAX_PACKET_SIZE: usize = 65535;

/// Default TDS packet size.
pub const DEFAULT_PACKET_SIZE: usize = 4096;

/// TDS packet type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PacketType {
    /// SQL batch request.
    SqlBatch = 0x01,
    /// Pre-TDS7 login packet.
    PreTds7Login = 0x02,
    /// Remote procedure call.
    Rpc = 0x03,
    /// Tabular response.
    TabularResult = 0x04,
    /// Attention signal.
    Attention = 0x06,
    /// Bulk load data.
    BulkLoad = 0x07,
    /// Federated authentication token.
    FedAuthToken = 0x08,
    /// Transaction manager request.
    TransactionManager = 0x0E,
    /// TDS7+ login packet.
    Tds7Login = 0x10,
    /// SSPI authentication.
    Sspi = 0x11,
    /// Pre-login packet.
    PreLogin = 0x12,
}

impl PacketType {
    /// Create a packet type from a raw byte value.
    pub fn from_u8(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0x01 => Ok(Self::SqlBatch),
            0x02 => Ok(Self::PreTds7Login),
            0x03 => Ok(Self::Rpc),
            0x04 => Ok(Self::TabularResult),
            0x06 => Ok(Self::Attention),
            0x07 => Ok(Self::BulkLoad),
            0x08 => Ok(Self::FedAuthToken),
            0x0E => Ok(Self::TransactionManager),
            0x10 => Ok(Self::Tds7Login),
            0x11 => Ok(Self::Sspi),
            0x12 => Ok(Self::PreLogin),
            _ => Err(ProtocolError::InvalidPacketType(value)),
        }
    }
}

bitflags! {
    /// TDS packet status flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PacketStatus: u8 {
        /// Normal packet, more packets to follow.
        const NORMAL = 0x00;
        /// End of message (last packet).
        const END_OF_MESSAGE = 0x01;
        /// Ignore this event (used for attention acknowledgment).
        const IGNORE_EVENT = 0x02;
        /// Reset connection (SQL Server 2000+).
        const RESET_CONNECTION = 0x08;
        /// Reset connection but keep transaction state.
        const RESET_CONNECTION_KEEP_TRANSACTION = 0x10;
    }
}

/// TDS packet header.
///
/// Every TDS packet begins with an 8-byte header that describes
/// the packet type, status, and length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    /// Type of packet.
    pub packet_type: PacketType,
    /// Status flags.
    pub status: PacketStatus,
    /// Total packet length including header.
    pub length: u16,
    /// Server process ID (SPID).
    pub spid: u16,
    /// Packet sequence number (wraps at 255).
    pub packet_id: u8,
    /// Window (unused, should be 0).
    pub window: u8,
}

impl PacketHeader {
    /// Create a new packet header.
    #[must_use]
    pub const fn new(packet_type: PacketType, status: PacketStatus, length: u16) -> Self {
        Self {
            packet_type,
            status,
            length,
            spid: 0,
            packet_id: 0,
            window: 0,
        }
    }

    /// Parse a packet header from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < PACKET_HEADER_SIZE {
            return Err(ProtocolError::IncompletePacket {
                expected: PACKET_HEADER_SIZE,
                actual: src.remaining(),
            });
        }

        let packet_type = PacketType::from_u8(src.get_u8())?;
        let status_byte = src.get_u8();
        let status = PacketStatus::from_bits(status_byte)
            .ok_or(ProtocolError::InvalidPacketStatus(status_byte))?;
        let length = src.get_u16();
        let spid = src.get_u16();
        let packet_id = src.get_u8();
        let window = src.get_u8();

        Ok(Self {
            packet_type,
            status,
            length,
            spid,
            packet_id,
            window,
        })
    }

    /// Encode the packet header to bytes.
    pub fn encode(&self, dst: &mut impl BufMut) {
        dst.put_u8(self.packet_type as u8);
        dst.put_u8(self.status.bits());
        dst.put_u16(self.length);
        dst.put_u16(self.spid);
        dst.put_u8(self.packet_id);
        dst.put_u8(self.window);
    }

    /// Encode the packet header to a new `Bytes` buffer.
    #[must_use]
    pub fn encode_to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_HEADER_SIZE);
        self.encode(&mut buf);
        buf.freeze()
    }

    /// Get the payload length (total length minus header).
    #[must_use]
    pub const fn payload_length(&self) -> usize {
        self.length.saturating_sub(PACKET_HEADER_SIZE as u16) as usize
    }

    /// Check if this is the last packet in a message.
    #[must_use]
    pub const fn is_end_of_message(&self) -> bool {
        self.status.contains(PacketStatus::END_OF_MESSAGE)
    }

    /// Set the packet ID (sequence number).
    #[must_use]
    pub const fn with_packet_id(mut self, id: u8) -> Self {
        self.packet_id = id;
        self
    }

    /// Set the SPID.
    #[must_use]
    pub const fn with_spid(mut self, spid: u16) -> Self {
        self.spid = spid;
        self
    }
}

impl Default for PacketHeader {
    fn default() -> Self {
        Self {
            packet_type: PacketType::SqlBatch,
            status: PacketStatus::END_OF_MESSAGE,
            length: PACKET_HEADER_SIZE as u16,
            spid: 0,
            packet_id: 1,
            window: 0,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = PacketHeader {
            packet_type: PacketType::SqlBatch,
            status: PacketStatus::END_OF_MESSAGE,
            length: 100,
            spid: 54,
            packet_id: 1,
            window: 0,
        };

        let bytes = header.encode_to_bytes();
        assert_eq!(bytes.len(), PACKET_HEADER_SIZE);

        let mut cursor = bytes.as_ref();
        let decoded = PacketHeader::decode(&mut cursor).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_payload_length() {
        let header = PacketHeader::new(PacketType::SqlBatch, PacketStatus::END_OF_MESSAGE, 100);
        assert_eq!(header.payload_length(), 92);
    }

    #[test]
    fn test_packet_type_from_u8() {
        assert_eq!(PacketType::from_u8(0x01).unwrap(), PacketType::SqlBatch);
        assert_eq!(PacketType::from_u8(0x12).unwrap(), PacketType::PreLogin);
        assert!(PacketType::from_u8(0xFF).is_err());
    }
}
