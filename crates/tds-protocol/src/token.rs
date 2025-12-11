//! TDS token stream definitions.
//!
//! Tokens are the fundamental units of TDS response data. The server sends
//! a stream of tokens that describe metadata, rows, errors, and other information.
//!
//! ## Token Structure
//!
//! Each token begins with a 1-byte token type identifier, followed by
//! token-specific data. Some tokens have fixed lengths, while others
//! have length prefixes.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use tds_protocol::token::{Token, TokenParser};
//! use bytes::Bytes;
//!
//! let data: Bytes = /* received from server */;
//! let mut parser = TokenParser::new(data);
//!
//! while let Some(token) = parser.next_token()? {
//!     match token {
//!         Token::Done(done) => println!("Rows affected: {}", done.row_count),
//!         Token::Error(err) => eprintln!("Error {}: {}", err.number, err.message),
//!         _ => {}
//!     }
//! }
//! ```

use bytes::{Buf, BufMut, Bytes};

use crate::codec::{read_b_varchar, read_us_varchar};
use crate::error::ProtocolError;

/// Token type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TokenType {
    /// Column metadata (COLMETADATA).
    ColMetaData = 0x81,
    /// Error message (ERROR).
    Error = 0xAA,
    /// Informational message (INFO).
    Info = 0xAB,
    /// Login acknowledgment (LOGINACK).
    LoginAck = 0xAD,
    /// Row data (ROW).
    Row = 0xD1,
    /// Null bitmap compressed row (NBCROW).
    NbcRow = 0xD2,
    /// Environment change (ENVCHANGE).
    EnvChange = 0xE3,
    /// SSPI authentication (SSPI).
    Sspi = 0xED,
    /// Done (DONE).
    Done = 0xFD,
    /// Done in procedure (DONEINPROC).
    DoneInProc = 0xFF,
    /// Done procedure (DONEPROC).
    DoneProc = 0xFE,
    /// Return status (RETURNSTATUS).
    ReturnStatus = 0x79,
    /// Return value (RETURNVALUE).
    ReturnValue = 0xAC,
    /// Order (ORDER).
    Order = 0xA9,
    /// Feature extension acknowledgment (FEATUREEXTACK).
    FeatureExtAck = 0xAE,
    /// Session state (SESSIONSTATE).
    SessionState = 0xE4,
    /// Federated authentication info (FEDAUTHINFO).
    FedAuthInfo = 0xEE,
    /// Column info (COLINFO).
    ColInfo = 0xA5,
    /// Table name (TABNAME).
    TabName = 0xA4,
    /// Offset (OFFSET).
    Offset = 0x78,
}

impl TokenType {
    /// Create a token type from a raw byte.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x81 => Some(Self::ColMetaData),
            0xAA => Some(Self::Error),
            0xAB => Some(Self::Info),
            0xAD => Some(Self::LoginAck),
            0xD1 => Some(Self::Row),
            0xD2 => Some(Self::NbcRow),
            0xE3 => Some(Self::EnvChange),
            0xED => Some(Self::Sspi),
            0xFD => Some(Self::Done),
            0xFF => Some(Self::DoneInProc),
            0xFE => Some(Self::DoneProc),
            0x79 => Some(Self::ReturnStatus),
            0xAC => Some(Self::ReturnValue),
            0xA9 => Some(Self::Order),
            0xAE => Some(Self::FeatureExtAck),
            0xE4 => Some(Self::SessionState),
            0xEE => Some(Self::FedAuthInfo),
            0xA5 => Some(Self::ColInfo),
            0xA4 => Some(Self::TabName),
            0x78 => Some(Self::Offset),
            _ => None,
        }
    }
}

/// Parsed TDS token.
///
/// This enum represents all possible tokens that can be received from SQL Server.
/// Each variant contains the parsed token data.
#[derive(Debug, Clone)]
pub enum Token {
    /// Column metadata describing result set structure.
    ColMetaData(ColMetaData),
    /// Row data.
    Row(RawRow),
    /// Null bitmap compressed row.
    NbcRow(NbcRow),
    /// Completion of a SQL statement.
    Done(Done),
    /// Completion of a stored procedure.
    DoneProc(DoneProc),
    /// Completion within a stored procedure.
    DoneInProc(DoneInProc),
    /// Return status from stored procedure.
    ReturnStatus(i32),
    /// Return value from stored procedure.
    ReturnValue(ReturnValue),
    /// Error message from server.
    Error(ServerError),
    /// Informational message from server.
    Info(ServerInfo),
    /// Login acknowledgment.
    LoginAck(LoginAck),
    /// Environment change notification.
    EnvChange(EnvChange),
    /// Column ordering information.
    Order(Order),
    /// Feature extension acknowledgment.
    FeatureExtAck(FeatureExtAck),
    /// SSPI authentication data.
    Sspi(SspiToken),
    /// Session state information.
    SessionState(SessionState),
    /// Federated authentication info.
    FedAuthInfo(FedAuthInfo),
}

/// Column metadata token.
#[derive(Debug, Clone, Default)]
pub struct ColMetaData {
    /// Column definitions.
    pub columns: Vec<ColumnData>,
}

/// Column definition within metadata.
#[derive(Debug, Clone)]
pub struct ColumnData {
    /// Column name.
    pub name: String,
    /// Column data type.
    pub col_type: u8,
    /// Column flags.
    pub flags: u16,
    /// Type-specific metadata.
    pub type_info: TypeInfo,
}

/// Type-specific metadata.
#[derive(Debug, Clone, Default)]
pub struct TypeInfo {
    /// Maximum length for variable-length types.
    pub max_length: Option<u32>,
    /// Precision for numeric types.
    pub precision: Option<u8>,
    /// Scale for numeric types.
    pub scale: Option<u8>,
    /// Collation for string types.
    pub collation: Option<Collation>,
}

/// SQL Server collation.
#[derive(Debug, Clone, Copy, Default)]
pub struct Collation {
    /// Locale ID.
    pub lcid: u32,
    /// Sort ID.
    pub sort_id: u8,
}

/// Raw row data (not yet decoded).
#[derive(Debug, Clone)]
pub struct RawRow {
    /// Raw column values.
    pub data: bytes::Bytes,
}

/// Null bitmap compressed row.
#[derive(Debug, Clone)]
pub struct NbcRow {
    /// Null bitmap.
    pub null_bitmap: Vec<u8>,
    /// Raw non-null column values.
    pub data: bytes::Bytes,
}

/// Done token indicating statement completion.
#[derive(Debug, Clone, Copy)]
pub struct Done {
    /// Status flags.
    pub status: DoneStatus,
    /// Current command.
    pub cur_cmd: u16,
    /// Row count (if applicable).
    pub row_count: u64,
}

/// Done status flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct DoneStatus {
    /// More results follow.
    pub more: bool,
    /// Error occurred.
    pub error: bool,
    /// Transaction in progress.
    pub in_xact: bool,
    /// Row count is valid.
    pub count: bool,
    /// Attention acknowledgment.
    pub attn: bool,
    /// Server error caused statement termination.
    pub srverror: bool,
}

/// Done in procedure token.
#[derive(Debug, Clone, Copy)]
pub struct DoneInProc {
    /// Status flags.
    pub status: DoneStatus,
    /// Current command.
    pub cur_cmd: u16,
    /// Row count.
    pub row_count: u64,
}

/// Done procedure token.
#[derive(Debug, Clone, Copy)]
pub struct DoneProc {
    /// Status flags.
    pub status: DoneStatus,
    /// Current command.
    pub cur_cmd: u16,
    /// Row count.
    pub row_count: u64,
}

/// Return value from stored procedure.
#[derive(Debug, Clone)]
pub struct ReturnValue {
    /// Parameter ordinal.
    pub param_ordinal: u16,
    /// Parameter name.
    pub param_name: String,
    /// Status flags.
    pub status: u8,
    /// User type.
    pub user_type: u32,
    /// Type flags.
    pub flags: u16,
    /// Type info.
    pub type_info: TypeInfo,
    /// Value data.
    pub value: bytes::Bytes,
}

/// Server error message.
#[derive(Debug, Clone)]
pub struct ServerError {
    /// Error number.
    pub number: i32,
    /// Error state.
    pub state: u8,
    /// Error severity class.
    pub class: u8,
    /// Error message text.
    pub message: String,
    /// Server name.
    pub server: String,
    /// Procedure name.
    pub procedure: String,
    /// Line number.
    pub line: i32,
}

/// Server informational message.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Info number.
    pub number: i32,
    /// Info state.
    pub state: u8,
    /// Info class (severity).
    pub class: u8,
    /// Info message text.
    pub message: String,
    /// Server name.
    pub server: String,
    /// Procedure name.
    pub procedure: String,
    /// Line number.
    pub line: i32,
}

/// Login acknowledgment token.
#[derive(Debug, Clone)]
pub struct LoginAck {
    /// Interface type.
    pub interface: u8,
    /// TDS version.
    pub tds_version: u32,
    /// Program name.
    pub prog_name: String,
    /// Program version.
    pub prog_version: u32,
}

/// Environment change token.
#[derive(Debug, Clone)]
pub struct EnvChange {
    /// Type of environment change.
    pub env_type: EnvChangeType,
    /// New value.
    pub new_value: EnvChangeValue,
    /// Old value.
    pub old_value: EnvChangeValue,
}

/// Environment change type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EnvChangeType {
    /// Database changed.
    Database = 1,
    /// Language changed.
    Language = 2,
    /// Character set changed.
    CharacterSet = 3,
    /// Packet size changed.
    PacketSize = 4,
    /// Unicode data sorting locale ID.
    UnicodeSortingLocalId = 5,
    /// Unicode comparison flags.
    UnicodeComparisonFlags = 6,
    /// SQL collation.
    SqlCollation = 7,
    /// Begin transaction.
    BeginTransaction = 8,
    /// Commit transaction.
    CommitTransaction = 9,
    /// Rollback transaction.
    RollbackTransaction = 10,
    /// Enlist DTC transaction.
    EnlistDtcTransaction = 11,
    /// Defect DTC transaction.
    DefectTransaction = 12,
    /// Real-time log shipping.
    RealTimeLogShipping = 13,
    /// Promote transaction.
    PromoteTransaction = 15,
    /// Transaction manager address.
    TransactionManagerAddress = 16,
    /// Transaction ended.
    TransactionEnded = 17,
    /// Reset connection completion acknowledgment.
    ResetConnectionCompletionAck = 18,
    /// User instance started.
    UserInstanceStarted = 19,
    /// Routing information.
    Routing = 20,
}

/// Environment change value.
#[derive(Debug, Clone)]
pub enum EnvChangeValue {
    /// String value.
    String(String),
    /// Binary value.
    Binary(bytes::Bytes),
    /// Routing information.
    Routing {
        /// Host name.
        host: String,
        /// Port number.
        port: u16,
    },
}

/// Column ordering information.
#[derive(Debug, Clone)]
pub struct Order {
    /// Ordered column indices.
    pub columns: Vec<u16>,
}

/// Feature extension acknowledgment.
#[derive(Debug, Clone)]
pub struct FeatureExtAck {
    /// Acknowledged features.
    pub features: Vec<FeatureAck>,
}

/// Individual feature acknowledgment.
#[derive(Debug, Clone)]
pub struct FeatureAck {
    /// Feature ID.
    pub feature_id: u8,
    /// Feature data.
    pub data: bytes::Bytes,
}

/// SSPI authentication token.
#[derive(Debug, Clone)]
pub struct SspiToken {
    /// SSPI data.
    pub data: bytes::Bytes,
}

/// Session state token.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Session state data.
    pub data: bytes::Bytes,
}

/// Federated authentication info.
#[derive(Debug, Clone)]
pub struct FedAuthInfo {
    /// STS URL.
    pub sts_url: String,
    /// Service principal name.
    pub spn: String,
}

// =============================================================================
// Token Parsing Implementation
// =============================================================================

/// Done token status flags bit positions.
mod done_status_bits {
    pub const DONE_MORE: u16 = 0x0001;
    pub const DONE_ERROR: u16 = 0x0002;
    pub const DONE_INXACT: u16 = 0x0004;
    pub const DONE_COUNT: u16 = 0x0010;
    pub const DONE_ATTN: u16 = 0x0020;
    pub const DONE_SRVERROR: u16 = 0x0100;
}

impl DoneStatus {
    /// Parse done status from raw bits.
    #[must_use]
    pub fn from_bits(bits: u16) -> Self {
        use done_status_bits::*;
        Self {
            more: (bits & DONE_MORE) != 0,
            error: (bits & DONE_ERROR) != 0,
            in_xact: (bits & DONE_INXACT) != 0,
            count: (bits & DONE_COUNT) != 0,
            attn: (bits & DONE_ATTN) != 0,
            srverror: (bits & DONE_SRVERROR) != 0,
        }
    }

    /// Convert to raw bits.
    #[must_use]
    pub fn to_bits(&self) -> u16 {
        use done_status_bits::*;
        let mut bits = 0u16;
        if self.more {
            bits |= DONE_MORE;
        }
        if self.error {
            bits |= DONE_ERROR;
        }
        if self.in_xact {
            bits |= DONE_INXACT;
        }
        if self.count {
            bits |= DONE_COUNT;
        }
        if self.attn {
            bits |= DONE_ATTN;
        }
        if self.srverror {
            bits |= DONE_SRVERROR;
        }
        bits
    }
}

impl Done {
    /// Size of the DONE token in bytes (excluding token type byte).
    pub const SIZE: usize = 12; // 2 (status) + 2 (curcmd) + 8 (rowcount)

    /// Decode a DONE token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < Self::SIZE {
            return Err(ProtocolError::IncompletePacket {
                expected: Self::SIZE,
                actual: src.remaining(),
            });
        }

        let status = DoneStatus::from_bits(src.get_u16_le());
        let cur_cmd = src.get_u16_le();
        let row_count = src.get_u64_le();

        Ok(Self {
            status,
            cur_cmd,
            row_count,
        })
    }

    /// Encode the DONE token to bytes.
    pub fn encode(&self, dst: &mut impl BufMut) {
        dst.put_u8(TokenType::Done as u8);
        dst.put_u16_le(self.status.to_bits());
        dst.put_u16_le(self.cur_cmd);
        dst.put_u64_le(self.row_count);
    }

    /// Check if more results follow this DONE token.
    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.status.more
    }

    /// Check if an error occurred.
    #[must_use]
    pub const fn has_error(&self) -> bool {
        self.status.error
    }

    /// Check if the row count is valid.
    #[must_use]
    pub const fn has_count(&self) -> bool {
        self.status.count
    }
}

impl DoneProc {
    /// Size of the DONEPROC token in bytes (excluding token type byte).
    pub const SIZE: usize = 12;

    /// Decode a DONEPROC token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < Self::SIZE {
            return Err(ProtocolError::IncompletePacket {
                expected: Self::SIZE,
                actual: src.remaining(),
            });
        }

        let status = DoneStatus::from_bits(src.get_u16_le());
        let cur_cmd = src.get_u16_le();
        let row_count = src.get_u64_le();

        Ok(Self {
            status,
            cur_cmd,
            row_count,
        })
    }

    /// Encode the DONEPROC token to bytes.
    pub fn encode(&self, dst: &mut impl BufMut) {
        dst.put_u8(TokenType::DoneProc as u8);
        dst.put_u16_le(self.status.to_bits());
        dst.put_u16_le(self.cur_cmd);
        dst.put_u64_le(self.row_count);
    }
}

impl DoneInProc {
    /// Size of the DONEINPROC token in bytes (excluding token type byte).
    pub const SIZE: usize = 12;

    /// Decode a DONEINPROC token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < Self::SIZE {
            return Err(ProtocolError::IncompletePacket {
                expected: Self::SIZE,
                actual: src.remaining(),
            });
        }

        let status = DoneStatus::from_bits(src.get_u16_le());
        let cur_cmd = src.get_u16_le();
        let row_count = src.get_u64_le();

        Ok(Self {
            status,
            cur_cmd,
            row_count,
        })
    }

    /// Encode the DONEINPROC token to bytes.
    pub fn encode(&self, dst: &mut impl BufMut) {
        dst.put_u8(TokenType::DoneInProc as u8);
        dst.put_u16_le(self.status.to_bits());
        dst.put_u16_le(self.cur_cmd);
        dst.put_u64_le(self.row_count);
    }
}

impl ServerError {
    /// Decode an ERROR token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        // ERROR token: length (2) + number (4) + state (1) + class (1) +
        //              message (us_varchar) + server (b_varchar) + procedure (b_varchar) + line (4)
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _length = src.get_u16_le();

        if src.remaining() < 6 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let number = src.get_i32_le();
        let state = src.get_u8();
        let class = src.get_u8();

        let message = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;
        let server = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;
        let procedure = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let line = src.get_i32_le();

        Ok(Self {
            number,
            state,
            class,
            message,
            server,
            procedure,
            line,
        })
    }

    /// Check if this is a fatal error (severity >= 20).
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        self.class >= 20
    }

    /// Check if this error indicates the batch was aborted (severity >= 16).
    #[must_use]
    pub const fn is_batch_abort(&self) -> bool {
        self.class >= 16
    }
}

impl ServerInfo {
    /// Decode an INFO token from bytes.
    ///
    /// INFO tokens have the same structure as ERROR tokens but with lower severity.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _length = src.get_u16_le();

        if src.remaining() < 6 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let number = src.get_i32_le();
        let state = src.get_u8();
        let class = src.get_u8();

        let message = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;
        let server = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;
        let procedure = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let line = src.get_i32_le();

        Ok(Self {
            number,
            state,
            class,
            message,
            server,
            procedure,
            line,
        })
    }
}

impl LoginAck {
    /// Decode a LOGINACK token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        // LOGINACK: length (2) + interface (1) + tds_version (4) + prog_name (b_varchar) + prog_version (4)
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _length = src.get_u16_le();

        if src.remaining() < 5 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let interface = src.get_u8();
        let tds_version = src.get_u32_le();
        let prog_name = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let prog_version = src.get_u32_le();

        Ok(Self {
            interface,
            tds_version,
            prog_name,
            prog_version,
        })
    }

    /// Get the TDS version as a `TdsVersion`.
    #[must_use]
    pub fn tds_version(&self) -> crate::version::TdsVersion {
        crate::version::TdsVersion::new(self.tds_version)
    }
}

impl EnvChangeType {
    /// Create from raw byte value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Database),
            2 => Some(Self::Language),
            3 => Some(Self::CharacterSet),
            4 => Some(Self::PacketSize),
            5 => Some(Self::UnicodeSortingLocalId),
            6 => Some(Self::UnicodeComparisonFlags),
            7 => Some(Self::SqlCollation),
            8 => Some(Self::BeginTransaction),
            9 => Some(Self::CommitTransaction),
            10 => Some(Self::RollbackTransaction),
            11 => Some(Self::EnlistDtcTransaction),
            12 => Some(Self::DefectTransaction),
            13 => Some(Self::RealTimeLogShipping),
            15 => Some(Self::PromoteTransaction),
            16 => Some(Self::TransactionManagerAddress),
            17 => Some(Self::TransactionEnded),
            18 => Some(Self::ResetConnectionCompletionAck),
            19 => Some(Self::UserInstanceStarted),
            20 => Some(Self::Routing),
            _ => None,
        }
    }
}

impl EnvChange {
    /// Decode an ENVCHANGE token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 3 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let length = src.get_u16_le() as usize;
        if src.remaining() < length {
            return Err(ProtocolError::IncompletePacket {
                expected: length,
                actual: src.remaining(),
            });
        }

        let env_type_byte = src.get_u8();
        let env_type = EnvChangeType::from_u8(env_type_byte)
            .ok_or(ProtocolError::InvalidTokenType(env_type_byte))?;

        let (new_value, old_value) = match env_type {
            EnvChangeType::Routing => {
                // Routing has special format
                let new_value = Self::decode_routing_value(src)?;
                let old_value = EnvChangeValue::Binary(Bytes::new());
                (new_value, old_value)
            }
            EnvChangeType::BeginTransaction
            | EnvChangeType::CommitTransaction
            | EnvChangeType::RollbackTransaction
            | EnvChangeType::EnlistDtcTransaction => {
                // Transaction tokens use binary format
                let new_len = src.get_u8() as usize;
                let new_value = if new_len > 0 && src.remaining() >= new_len {
                    EnvChangeValue::Binary(src.copy_to_bytes(new_len))
                } else {
                    EnvChangeValue::Binary(Bytes::new())
                };

                let old_len = src.get_u8() as usize;
                let old_value = if old_len > 0 && src.remaining() >= old_len {
                    EnvChangeValue::Binary(src.copy_to_bytes(old_len))
                } else {
                    EnvChangeValue::Binary(Bytes::new())
                };

                (new_value, old_value)
            }
            _ => {
                // String format for most env changes
                let new_value = read_b_varchar(src)
                    .map(EnvChangeValue::String)
                    .unwrap_or(EnvChangeValue::String(String::new()));

                let old_value = read_b_varchar(src)
                    .map(EnvChangeValue::String)
                    .unwrap_or(EnvChangeValue::String(String::new()));

                (new_value, old_value)
            }
        };

        Ok(Self {
            env_type,
            new_value,
            old_value,
        })
    }

    fn decode_routing_value(src: &mut impl Buf) -> Result<EnvChangeValue, ProtocolError> {
        // Routing format: length (2) + protocol (1) + port (2) + server_len (2) + server (utf16)
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _routing_len = src.get_u16_le();

        if src.remaining() < 5 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _protocol = src.get_u8();
        let port = src.get_u16_le();
        let server_len = src.get_u16_le() as usize;

        // Read UTF-16LE server name
        if src.remaining() < server_len * 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let mut chars = Vec::with_capacity(server_len);
        for _ in 0..server_len {
            chars.push(src.get_u16_le());
        }

        let host = String::from_utf16(&chars).map_err(|_| {
            ProtocolError::StringEncoding(
                #[cfg(feature = "std")]
                "invalid UTF-16 in routing hostname".to_string(),
                #[cfg(not(feature = "std"))]
                "invalid UTF-16 in routing hostname",
            )
        })?;

        Ok(EnvChangeValue::Routing { host, port })
    }

    /// Check if this is a routing redirect.
    #[must_use]
    pub fn is_routing(&self) -> bool {
        self.env_type == EnvChangeType::Routing
    }

    /// Get routing information if this is a routing change.
    #[must_use]
    pub fn routing_info(&self) -> Option<(&str, u16)> {
        if let EnvChangeValue::Routing { host, port } = &self.new_value {
            Some((host, *port))
        } else {
            None
        }
    }

    /// Get the new database name if this is a database change.
    #[must_use]
    pub fn new_database(&self) -> Option<&str> {
        if self.env_type == EnvChangeType::Database {
            if let EnvChangeValue::String(s) = &self.new_value {
                return Some(s);
            }
        }
        None
    }
}

impl Order {
    /// Decode an ORDER token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let length = src.get_u16_le() as usize;
        let column_count = length / 2;

        if src.remaining() < length {
            return Err(ProtocolError::IncompletePacket {
                expected: length,
                actual: src.remaining(),
            });
        }

        let mut columns = Vec::with_capacity(column_count);
        for _ in 0..column_count {
            columns.push(src.get_u16_le());
        }

        Ok(Self { columns })
    }
}

impl FeatureExtAck {
    /// Feature terminator byte.
    pub const TERMINATOR: u8 = 0xFF;

    /// Decode a FEATUREEXTACK token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        let mut features = Vec::new();

        loop {
            if !src.has_remaining() {
                return Err(ProtocolError::UnexpectedEof);
            }

            let feature_id = src.get_u8();
            if feature_id == Self::TERMINATOR {
                break;
            }

            if src.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }

            let data_len = src.get_u32_le() as usize;

            if src.remaining() < data_len {
                return Err(ProtocolError::IncompletePacket {
                    expected: data_len,
                    actual: src.remaining(),
                });
            }

            let data = src.copy_to_bytes(data_len);
            features.push(FeatureAck { feature_id, data });
        }

        Ok(Self { features })
    }
}

impl SspiToken {
    /// Decode an SSPI token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let length = src.get_u16_le() as usize;

        if src.remaining() < length {
            return Err(ProtocolError::IncompletePacket {
                expected: length,
                actual: src.remaining(),
            });
        }

        let data = src.copy_to_bytes(length);
        Ok(Self { data })
    }
}

impl FedAuthInfo {
    /// Decode a FEDAUTHINFO token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _length = src.get_u32_le();

        if src.remaining() < 5 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let _count = src.get_u8();

        // Read option data
        let mut sts_url = String::new();
        let mut spn = String::new();

        // Parse info options until we have both
        while src.has_remaining() {
            if src.remaining() < 9 {
                break;
            }

            let info_id = src.get_u8();
            let info_len = src.get_u32_le() as usize;
            let _info_offset = src.get_u32_le();

            if src.remaining() < info_len {
                break;
            }

            // Read UTF-16LE string
            let char_count = info_len / 2;
            let mut chars = Vec::with_capacity(char_count);
            for _ in 0..char_count {
                chars.push(src.get_u16_le());
            }

            if let Ok(value) = String::from_utf16(&chars) {
                match info_id {
                    0x01 => spn = value,
                    0x02 => sts_url = value,
                    _ => {}
                }
            }
        }

        Ok(Self { sts_url, spn })
    }
}

// =============================================================================
// Token Parser
// =============================================================================

/// Token stream parser.
///
/// Parses a stream of TDS tokens from a byte buffer.
pub struct TokenParser {
    data: Bytes,
    position: usize,
}

impl TokenParser {
    /// Create a new token parser from bytes.
    #[must_use]
    pub fn new(data: Bytes) -> Self {
        Self { data, position: 0 }
    }

    /// Get remaining bytes in the buffer.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    /// Check if there are more bytes to parse.
    #[must_use]
    pub fn has_remaining(&self) -> bool {
        self.position < self.data.len()
    }

    /// Parse the next token from the stream.
    ///
    /// Returns `None` if no more tokens are available.
    pub fn next_token(&mut self) -> Result<Option<Token>, ProtocolError> {
        if !self.has_remaining() {
            return Ok(None);
        }

        let mut buf = &self.data[self.position..];
        let start_pos = self.position;

        let token_type_byte = buf.get_u8();
        let token_type = TokenType::from_u8(token_type_byte);

        let token = match token_type {
            Some(TokenType::Done) => {
                let done = Done::decode(&mut buf)?;
                Token::Done(done)
            }
            Some(TokenType::DoneProc) => {
                let done = DoneProc::decode(&mut buf)?;
                Token::DoneProc(done)
            }
            Some(TokenType::DoneInProc) => {
                let done = DoneInProc::decode(&mut buf)?;
                Token::DoneInProc(done)
            }
            Some(TokenType::Error) => {
                let error = ServerError::decode(&mut buf)?;
                Token::Error(error)
            }
            Some(TokenType::Info) => {
                let info = ServerInfo::decode(&mut buf)?;
                Token::Info(info)
            }
            Some(TokenType::LoginAck) => {
                let login_ack = LoginAck::decode(&mut buf)?;
                Token::LoginAck(login_ack)
            }
            Some(TokenType::EnvChange) => {
                let env_change = EnvChange::decode(&mut buf)?;
                Token::EnvChange(env_change)
            }
            Some(TokenType::Order) => {
                let order = Order::decode(&mut buf)?;
                Token::Order(order)
            }
            Some(TokenType::FeatureExtAck) => {
                let ack = FeatureExtAck::decode(&mut buf)?;
                Token::FeatureExtAck(ack)
            }
            Some(TokenType::Sspi) => {
                let sspi = SspiToken::decode(&mut buf)?;
                Token::Sspi(sspi)
            }
            Some(TokenType::FedAuthInfo) => {
                let info = FedAuthInfo::decode(&mut buf)?;
                Token::FedAuthInfo(info)
            }
            Some(TokenType::ReturnStatus) => {
                if buf.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let status = buf.get_i32_le();
                Token::ReturnStatus(status)
            }
            Some(TokenType::ColMetaData)
            | Some(TokenType::Row)
            | Some(TokenType::NbcRow)
            | Some(TokenType::ReturnValue)
            | Some(TokenType::SessionState)
            | Some(TokenType::ColInfo)
            | Some(TokenType::TabName)
            | Some(TokenType::Offset) => {
                // These tokens require additional context (column metadata) to parse.
                // Return an error indicating they need special handling.
                return Err(ProtocolError::InvalidTokenType(token_type_byte));
            }
            None => {
                return Err(ProtocolError::InvalidTokenType(token_type_byte));
            }
        };

        // Update position based on how much was consumed
        let consumed = self.data.len() - start_pos - buf.remaining();
        self.position = start_pos + consumed;

        Ok(Some(token))
    }
}

// =============================================================================
// no_std support
// =============================================================================

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_done_roundtrip() {
        let done = Done {
            status: DoneStatus {
                more: false,
                error: false,
                in_xact: false,
                count: true,
                attn: false,
                srverror: false,
            },
            cur_cmd: 193, // SELECT
            row_count: 42,
        };

        let mut buf = BytesMut::new();
        done.encode(&mut buf);

        // Skip the token type byte
        let mut cursor = &buf[1..];
        let decoded = Done::decode(&mut cursor).unwrap();

        assert_eq!(decoded.status.count, done.status.count);
        assert_eq!(decoded.cur_cmd, done.cur_cmd);
        assert_eq!(decoded.row_count, done.row_count);
    }

    #[test]
    fn test_done_status_bits() {
        let status = DoneStatus {
            more: true,
            error: true,
            in_xact: true,
            count: true,
            attn: false,
            srverror: false,
        };

        let bits = status.to_bits();
        let restored = DoneStatus::from_bits(bits);

        assert_eq!(status.more, restored.more);
        assert_eq!(status.error, restored.error);
        assert_eq!(status.in_xact, restored.in_xact);
        assert_eq!(status.count, restored.count);
    }

    #[test]
    fn test_token_parser_done() {
        // DONE token: type (1) + status (2) + curcmd (2) + rowcount (8)
        let data = Bytes::from_static(&[
            0xFD, // DONE token type
            0x10, 0x00, // status: DONE_COUNT
            0xC1, 0x00, // cur_cmd: 193 (SELECT)
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_count: 5
        ]);

        let mut parser = TokenParser::new(data);
        let token = parser.next_token().unwrap().unwrap();

        match token {
            Token::Done(done) => {
                assert!(done.status.count);
                assert!(!done.status.more);
                assert_eq!(done.cur_cmd, 193);
                assert_eq!(done.row_count, 5);
            }
            _ => panic!("Expected Done token"),
        }

        // No more tokens
        assert!(parser.next_token().unwrap().is_none());
    }

    #[test]
    fn test_env_change_type_from_u8() {
        assert_eq!(
            EnvChangeType::from_u8(1),
            Some(EnvChangeType::Database)
        );
        assert_eq!(
            EnvChangeType::from_u8(20),
            Some(EnvChangeType::Routing)
        );
        assert_eq!(EnvChangeType::from_u8(100), None);
    }
}
