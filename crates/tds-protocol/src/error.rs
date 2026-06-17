//! Protocol-level error types.

use thiserror::Error;

/// Errors that can occur during TDS protocol parsing or encoding.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// Packet data is truncated or incomplete.
    #[error("incomplete packet: expected {expected} bytes, got {actual}")]
    IncompletePacket {
        /// Expected number of bytes.
        expected: usize,
        /// Actual number of bytes available.
        actual: usize,
    },

    /// Invalid packet type value.
    #[error("invalid packet type: {0:#x}")]
    InvalidPacketType(u8),

    /// Invalid token type value.
    #[error("invalid token type: {0:#x}")]
    InvalidTokenType(u8),

    /// Invalid data type value.
    #[error("invalid data type: {0:#x}")]
    InvalidDataType(u8),

    /// Invalid prelogin option.
    #[error("invalid prelogin option: {0:#x}")]
    InvalidPreloginOption(u8),

    /// Invalid PRELOGIN encryption level byte.
    #[error("invalid encryption level: {0:#x}")]
    InvalidEncryptionLevel(u8),

    /// Invalid TDS version.
    #[error("invalid TDS version: {0:#x}")]
    InvalidTdsVersion(u32),

    /// String encoding error.
    #[error("string encoding error: {0}")]
    StringEncoding(
        #[cfg(feature = "std")] String,
        #[cfg(not(feature = "std"))] &'static str,
    ),

    /// Packet length exceeds maximum allowed.
    #[error("packet too large: {length} bytes (max {max})")]
    PacketTooLarge {
        /// Actual packet length.
        length: usize,
        /// Maximum allowed length.
        max: usize,
    },

    /// Invalid packet status flags.
    #[error("invalid packet status: {0:#x}")]
    InvalidPacketStatus(u8),

    /// Buffer overflow during encoding.
    #[error("buffer overflow: needed {needed} bytes, capacity {capacity}")]
    BufferOverflow {
        /// Bytes needed.
        needed: usize,
        /// Buffer capacity.
        capacity: usize,
    },

    /// Unexpected end of stream.
    #[error("unexpected end of stream")]
    UnexpectedEof,

    /// Protocol version mismatch.
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u32),

    /// Invalid field value in a protocol structure.
    #[error("invalid {field} value: {value}")]
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Invalid value.
        value: u32,
    },
}

impl ProtocolError {
    /// Check if this error is transient and may succeed on retry.
    ///
    /// Protocol errors are always terminal — they indicate malformed data or
    /// driver bugs. In particular `UnexpectedEof` is produced when a token
    /// inside a fully-received message is truncated or misparsed; retrying
    /// deterministically fails again. Genuine connection loss surfaces at the
    /// transport layer (`CodecError::Io` / `CodecError::ConnectionClosed`),
    /// which remains transient.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        false
    }

    /// Check if this error is terminal and will never succeed on retry.
    ///
    /// Most protocol errors indicate a fundamental mismatch between client
    /// and server, a driver bug, or corrupted data.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        !self.is_transient()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #160 regression: `UnexpectedEof` is produced for malformed
    /// tokens inside a fully-received buffer (dozens of parse sites), so it
    /// must not be classified retryable — retry layers honoring
    /// `is_transient` would re-run a deterministically failing parse.
    #[test]
    fn unexpected_eof_is_terminal_not_transient() {
        assert!(!ProtocolError::UnexpectedEof.is_transient());
        assert!(ProtocolError::UnexpectedEof.is_terminal());
    }
}
