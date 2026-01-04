//! TDS pre-login packet handling.
//!
//! The pre-login packet is the first message exchanged between client and server
//! in TDS 7.x connections. It negotiates protocol version, encryption, and other
//! connection parameters.
//!
//! Note: TDS 8.0 (strict mode) does not use pre-login negotiation; TLS is
//! established before any TDS traffic.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::ProtocolError;
use crate::prelude::*;
use crate::version::{SqlServerVersion, TdsVersion};

/// Pre-login option types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PreLoginOption {
    /// Version information.
    Version = 0x00,
    /// Encryption negotiation.
    Encryption = 0x01,
    /// Instance name (for named instances).
    Instance = 0x02,
    /// Thread ID.
    ThreadId = 0x03,
    /// MARS (Multiple Active Result Sets) support.
    Mars = 0x04,
    /// Trace ID for distributed tracing.
    TraceId = 0x05,
    /// Federated authentication required.
    FedAuthRequired = 0x06,
    /// Nonce for encryption.
    Nonce = 0x07,
    /// Terminator (end of options).
    Terminator = 0xFF,
}

impl PreLoginOption {
    /// Create from raw byte value.
    pub fn from_u8(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0x00 => Ok(Self::Version),
            0x01 => Ok(Self::Encryption),
            0x02 => Ok(Self::Instance),
            0x03 => Ok(Self::ThreadId),
            0x04 => Ok(Self::Mars),
            0x05 => Ok(Self::TraceId),
            0x06 => Ok(Self::FedAuthRequired),
            0x07 => Ok(Self::Nonce),
            0xFF => Ok(Self::Terminator),
            _ => Err(ProtocolError::InvalidPreloginOption(value)),
        }
    }
}

/// Encryption level for connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum EncryptionLevel {
    /// Encryption is off.
    Off = 0x00,
    /// Encryption is on.
    On = 0x01,
    /// Encryption is not supported.
    NotSupported = 0x02,
    /// Encryption is required.
    #[default]
    Required = 0x03,
    /// Client certificate authentication (TDS 8.0+).
    ClientCertAuth = 0x80,
}

impl EncryptionLevel {
    /// Create from raw byte value.
    pub fn from_u8(value: u8) -> Self {
        match value {
            0x00 => Self::Off,
            0x01 => Self::On,
            0x02 => Self::NotSupported,
            0x03 => Self::Required,
            0x80 => Self::ClientCertAuth,
            _ => Self::Off,
        }
    }

    /// Check if encryption is required.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        matches!(self, Self::On | Self::Required | Self::ClientCertAuth)
    }
}

/// Pre-login message builder and parser.
///
/// This struct is used for both client requests and server responses:
/// - **Client → Server**: Set `version` to the requested TDS version
/// - **Server → Client**: `server_version` contains the SQL Server product version
///
/// Note: The VERSION field has different semantics in each direction:
/// - Client sends: TDS protocol version (e.g., 7.4)
/// - Server sends: SQL Server product version (e.g., 13.0.6300 for SQL Server 2016)
#[derive(Debug, Clone, Default)]
pub struct PreLogin {
    /// TDS version (client request).
    ///
    /// This is the TDS protocol version the client requests. When sending a
    /// PreLogin, set this to the desired TDS version.
    pub version: TdsVersion,

    /// SQL Server product version (server response).
    ///
    /// When decoding a PreLogin response from the server, this contains the
    /// SQL Server product version (e.g., 13.0.6300 for SQL Server 2016).
    /// This is NOT the TDS version - the actual TDS version is negotiated
    /// in the LOGINACK token after login.
    pub server_version: Option<SqlServerVersion>,

    /// Sub-build version (legacy, now part of server_version).
    #[deprecated(since = "0.5.2", note = "Use server_version.sub_build instead")]
    pub sub_build: u16,

    /// Encryption level.
    pub encryption: EncryptionLevel,
    /// Instance name (for named instances).
    pub instance: Option<String>,
    /// Thread ID.
    pub thread_id: Option<u32>,
    /// MARS enabled.
    pub mars: bool,
    /// Trace ID (Activity ID and Sequence).
    pub trace_id: Option<TraceId>,
    /// Federated authentication required.
    pub fed_auth_required: bool,
    /// Nonce for encryption.
    pub nonce: Option<[u8; 32]>,
}

/// Distributed tracing ID.
#[derive(Debug, Clone, Copy)]
pub struct TraceId {
    /// Activity ID (GUID).
    pub activity_id: [u8; 16],
    /// Activity sequence.
    pub activity_sequence: u32,
}

impl PreLogin {
    /// Create a new pre-login message with default values.
    #[must_use]
    #[allow(deprecated)] // sub_build is deprecated but we need to initialize it
    pub fn new() -> Self {
        Self {
            version: TdsVersion::V7_4,
            server_version: None,
            sub_build: 0,
            encryption: EncryptionLevel::Required,
            instance: None,
            thread_id: None,
            mars: false,
            trace_id: None,
            fed_auth_required: false,
            nonce: None,
        }
    }

    /// Set the TDS version.
    #[must_use]
    pub fn with_version(mut self, version: TdsVersion) -> Self {
        self.version = version;
        self
    }

    /// Set the encryption level.
    #[must_use]
    pub fn with_encryption(mut self, level: EncryptionLevel) -> Self {
        self.encryption = level;
        self
    }

    /// Enable MARS.
    #[must_use]
    pub fn with_mars(mut self, enabled: bool) -> Self {
        self.mars = enabled;
        self
    }

    /// Set the instance name.
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Encode the pre-login message to bytes.
    #[must_use]
    #[allow(deprecated)] // sub_build is deprecated but we still encode it
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);

        // Calculate option data offsets
        // Each option entry is 5 bytes: type (1) + offset (2) + length (2)
        // Plus 1 byte for terminator
        let mut option_count = 3; // Version, Encryption, MARS are always present
        if self.instance.is_some() {
            option_count += 1;
        }
        if self.thread_id.is_some() {
            option_count += 1;
        }
        if self.trace_id.is_some() {
            option_count += 1;
        }
        if self.fed_auth_required {
            option_count += 1;
        }
        if self.nonce.is_some() {
            option_count += 1;
        }

        let header_size = option_count * 5 + 1; // +1 for terminator
        let mut data_offset = header_size as u16;
        let mut data_buf = BytesMut::new();

        // VERSION option (6 bytes: 4 bytes version + 2 bytes sub-build)
        buf.put_u8(PreLoginOption::Version as u8);
        buf.put_u16(data_offset);
        buf.put_u16(6);
        let version_raw = self.version.raw();
        data_buf.put_u8((version_raw >> 24) as u8);
        data_buf.put_u8((version_raw >> 16) as u8);
        data_buf.put_u8((version_raw >> 8) as u8);
        data_buf.put_u8(version_raw as u8);
        data_buf.put_u16_le(self.sub_build);
        data_offset += 6;

        // ENCRYPTION option (1 byte)
        buf.put_u8(PreLoginOption::Encryption as u8);
        buf.put_u16(data_offset);
        buf.put_u16(1);
        data_buf.put_u8(self.encryption as u8);
        data_offset += 1;

        // INSTANCE option (if set)
        if let Some(ref instance) = self.instance {
            let instance_bytes = instance.as_bytes();
            let len = instance_bytes.len() as u16 + 1; // +1 for null terminator
            buf.put_u8(PreLoginOption::Instance as u8);
            buf.put_u16(data_offset);
            buf.put_u16(len);
            data_buf.put_slice(instance_bytes);
            data_buf.put_u8(0); // null terminator
            data_offset += len;
        }

        // THREADID option (if set)
        if let Some(thread_id) = self.thread_id {
            buf.put_u8(PreLoginOption::ThreadId as u8);
            buf.put_u16(data_offset);
            buf.put_u16(4);
            data_buf.put_u32(thread_id);
            data_offset += 4;
        }

        // MARS option (1 byte)
        buf.put_u8(PreLoginOption::Mars as u8);
        buf.put_u16(data_offset);
        buf.put_u16(1);
        data_buf.put_u8(if self.mars { 0x01 } else { 0x00 });
        data_offset += 1;

        // TRACEID option (if set)
        if let Some(ref trace_id) = self.trace_id {
            buf.put_u8(PreLoginOption::TraceId as u8);
            buf.put_u16(data_offset);
            buf.put_u16(36);
            data_buf.put_slice(&trace_id.activity_id);
            data_buf.put_u32_le(trace_id.activity_sequence);
            // Connection ID (16 bytes, typically zeros for client)
            data_buf.put_slice(&[0u8; 16]);
            data_offset += 36;
        }

        // FEDAUTHREQUIRED option (if set)
        if self.fed_auth_required {
            buf.put_u8(PreLoginOption::FedAuthRequired as u8);
            buf.put_u16(data_offset);
            buf.put_u16(1);
            data_buf.put_u8(0x01);
            data_offset += 1;
        }

        // NONCE option (if set)
        if let Some(ref nonce) = self.nonce {
            buf.put_u8(PreLoginOption::Nonce as u8);
            buf.put_u16(data_offset);
            buf.put_u16(32);
            data_buf.put_slice(nonce);
            let _ = data_offset; // Suppress unused warning
        }

        // Terminator
        buf.put_u8(PreLoginOption::Terminator as u8);

        // Append data section
        buf.put_slice(&data_buf);

        buf.freeze()
    }

    /// Decode a pre-login response from the server.
    ///
    /// Per MS-TDS spec 2.2.6.4, PreLogin message structure:
    /// - Option headers: each 5 bytes (type:1 + offset:2 + length:2)
    /// - Terminator: 1 byte (0xFF)
    /// - Option data: variable length, positioned at offsets specified in headers
    ///
    /// Offsets in headers are absolute from the start of the PreLogin packet payload.
    pub fn decode(mut src: impl Buf) -> Result<Self, ProtocolError> {
        let mut prelogin = Self::default();

        // Parse option headers first, collecting (option_type, offset, length)
        let mut options = Vec::new();
        loop {
            if src.remaining() < 1 {
                return Err(ProtocolError::UnexpectedEof);
            }

            let option_type = src.get_u8();
            if option_type == PreLoginOption::Terminator as u8 {
                break;
            }

            if src.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }

            let offset = src.get_u16();
            let length = src.get_u16();
            options.push((PreLoginOption::from_u8(option_type)?, offset, length));
        }

        // Get remaining data as bytes for random access
        let data = src.copy_to_bytes(src.remaining());

        // Calculate header size: each option is 5 bytes + 1 byte terminator
        let header_size = options.len() * 5 + 1;

        for (option, packet_offset, length) in options {
            let packet_offset = packet_offset as usize;
            let length = length as usize;

            // Convert absolute packet offset to offset within data buffer
            // The data buffer starts after the headers, so we subtract header_size
            if packet_offset < header_size {
                // Invalid: offset points inside the headers
                continue;
            }
            let data_offset = packet_offset - header_size;

            // Bounds check
            if data_offset + length > data.len() {
                continue;
            }

            #[allow(deprecated)] // We still populate sub_build for backward compatibility
            match option {
                PreLoginOption::Version if length >= 4 => {
                    // Per MS-TDS 2.2.6.4: The server sends its SQL Server product version
                    // in the VERSION field, NOT the TDS protocol version.
                    //
                    // Format: UL_VERSION (4 bytes big-endian) + US_SUBBUILD (2 bytes little-endian)
                    // UL_VERSION contains: [major][minor][build_hi][build_lo]
                    //
                    // For example, SQL Server 2016 sends 13.0.xxxx (major=13, minor=0)
                    let version_bytes = &data[data_offset..data_offset + 4];
                    let version_raw = u32::from_be_bytes([
                        version_bytes[0],
                        version_bytes[1],
                        version_bytes[2],
                        version_bytes[3],
                    ]);

                    // Extract sub_build if present
                    let sub_build = if length >= 6 {
                        let sub_build_bytes = &data[data_offset + 4..data_offset + 6];
                        u16::from_le_bytes([sub_build_bytes[0], sub_build_bytes[1]])
                    } else {
                        0
                    };

                    // Populate the new SqlServerVersion field (correct semantics)
                    prelogin.server_version =
                        Some(SqlServerVersion::from_raw(version_raw, sub_build));

                    // Also set deprecated fields for backward compatibility
                    prelogin.version = TdsVersion::new(version_raw);
                    prelogin.sub_build = sub_build;
                }
                PreLoginOption::Encryption if length >= 1 => {
                    prelogin.encryption = EncryptionLevel::from_u8(data[data_offset]);
                }
                PreLoginOption::Mars if length >= 1 => {
                    prelogin.mars = data[data_offset] != 0;
                }
                PreLoginOption::Instance if length > 0 => {
                    // Instance name is null-terminated string
                    let instance_data = &data[data_offset..data_offset + length];
                    if let Some(null_pos) = instance_data.iter().position(|&b| b == 0) {
                        if let Ok(s) = core::str::from_utf8(&instance_data[..null_pos]) {
                            if !s.is_empty() {
                                prelogin.instance = Some(s.to_string());
                            }
                        }
                    }
                }
                PreLoginOption::ThreadId if length >= 4 => {
                    let bytes = &data[data_offset..data_offset + 4];
                    prelogin.thread_id =
                        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                }
                PreLoginOption::FedAuthRequired if length >= 1 => {
                    prelogin.fed_auth_required = data[data_offset] != 0;
                }
                PreLoginOption::Nonce if length >= 32 => {
                    let mut nonce = [0u8; 32];
                    nonce.copy_from_slice(&data[data_offset..data_offset + 32]);
                    prelogin.nonce = Some(nonce);
                }
                _ => {}
            }
        }

        Ok(prelogin)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_prelogin_encode() {
        let prelogin = PreLogin::new()
            .with_version(TdsVersion::V7_4)
            .with_encryption(EncryptionLevel::Required);

        let encoded = prelogin.encode();
        assert!(!encoded.is_empty());
        // First byte should be VERSION option type
        assert_eq!(encoded[0], PreLoginOption::Version as u8);
    }

    #[test]
    fn test_encryption_level() {
        assert!(EncryptionLevel::Required.is_required());
        assert!(EncryptionLevel::On.is_required());
        assert!(!EncryptionLevel::Off.is_required());
        assert!(!EncryptionLevel::NotSupported.is_required());
    }

    #[test]
    fn test_prelogin_decode_roundtrip() {
        // Create a PreLogin with various options
        let original = PreLogin::new()
            .with_version(TdsVersion::V7_4)
            .with_encryption(EncryptionLevel::On)
            .with_mars(true);

        // Encode it
        let encoded = original.encode();

        // Decode it back
        let decoded = PreLogin::decode(encoded.as_ref()).unwrap();

        // Verify the critical fields match
        assert_eq!(decoded.version, original.version);
        assert_eq!(decoded.encryption, original.encryption);
        assert_eq!(decoded.mars, original.mars);
    }

    #[test]
    fn test_prelogin_decode_encryption_offset() {
        // Manually construct a PreLogin packet with options in non-standard order
        // to verify offset handling works correctly
        //
        // Structure:
        // - ENCRYPTION header at offset pointing to encryption data
        // - VERSION header at offset pointing to version data
        // - Terminator
        // - Data section

        use bytes::BufMut;

        let mut buf = bytes::BytesMut::new();

        // Header section: each option is 5 bytes (type:1 + offset:2 + length:2)
        // We'll have 2 options + terminator = 11 bytes header
        let header_size: u16 = 11;

        // ENCRYPTION option header (put this first to test that we read from correct offset)
        buf.put_u8(PreLoginOption::Encryption as u8);
        buf.put_u16(header_size); // offset to encryption data
        buf.put_u16(1); // length

        // VERSION option header
        buf.put_u8(PreLoginOption::Version as u8);
        buf.put_u16(header_size + 1); // offset to version data (after encryption)
        buf.put_u16(6); // length

        // Terminator
        buf.put_u8(PreLoginOption::Terminator as u8);

        // Data section
        // Encryption data (1 byte): ENCRYPT_ON = 0x01
        buf.put_u8(0x01);

        // Version data (6 bytes): TDS 7.4 = 0x74000004 big-endian + sub-build 0x0000 little-endian
        buf.put_u8(0x74);
        buf.put_u8(0x00);
        buf.put_u8(0x00);
        buf.put_u8(0x04);
        buf.put_u16_le(0x0000); // sub-build

        // Decode
        let decoded = PreLogin::decode(buf.freeze().as_ref()).unwrap();

        // Verify encryption was read from correct offset (not from index 0)
        assert_eq!(decoded.encryption, EncryptionLevel::On);
        assert_eq!(decoded.version, TdsVersion::V7_4);
    }
}
