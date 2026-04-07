//! Codec error types.

use thiserror::Error;

/// Errors that can occur during packet encoding/decoding.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CodecError {
    /// IO error during read/write operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Protocol-level error from tds-protocol.
    #[error("protocol error: {0}")]
    Protocol(#[from] tds_protocol::ProtocolError),

    /// Packet too large.
    #[error("packet too large: {size} bytes (max {max})")]
    PacketTooLarge {
        /// Actual packet size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// Incomplete packet data.
    #[error("incomplete packet: need {needed} more bytes")]
    IncompletePacket {
        /// Bytes needed to complete the packet.
        needed: usize,
    },

    /// Invalid packet header.
    #[error("invalid packet header")]
    InvalidHeader,

    /// Connection closed unexpectedly.
    #[error("connection closed")]
    ConnectionClosed,

    /// Encoding error.
    #[error("encoding error: {0}")]
    Encoding(String),

    /// Decoding error.
    #[error("decoding error: {0}")]
    Decoding(String),
}

impl CodecError {
    /// Check if this error is transient and may succeed on retry.
    ///
    /// IO errors and connection closures are typically transient.
    /// Protocol, encoding, and header errors are terminal.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Io(_) | Self::ConnectionClosed => true,
            Self::Protocol(e) => e.is_transient(),
            _ => false,
        }
    }

    /// Check if this error is terminal and will never succeed on retry.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        !self.is_transient()
    }
}
