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
use crate::prelude::*;
use crate::types::TypeId;

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
    /// Column data type ID.
    pub type_id: TypeId,
    /// Column data type raw byte (for unknown types).
    pub col_type: u8,
    /// Column flags.
    pub flags: u16,
    /// User type ID.
    pub user_type: u32,
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
///
/// Collations in SQL Server define the character encoding and sorting rules
/// for string data. For `VARCHAR` columns, the collation determines which
/// code page (character encoding) is used to store the data.
///
/// # Encoding Support
///
/// When the `encoding` feature is enabled, the [`Collation::encoding()`] method
/// returns the appropriate [`encoding_rs::Encoding`] for decoding `VARCHAR` data.
///
/// # Example
///
/// ```rust,ignore
/// use tds_protocol::token::Collation;
///
/// let collation = Collation { lcid: 0x0804, sort_id: 0 }; // Chinese (PRC)
/// if let Some(encoding) = collation.encoding() {
///     let (decoded, _, _) = encoding.decode(raw_bytes);
///     // decoded is now proper Chinese text
/// }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Collation {
    /// Locale ID (LCID).
    ///
    /// The LCID encodes both the language and region. The lower 16 bits
    /// contain the primary language ID, and bits 16-19 contain the sort ID
    /// for some collations.
    ///
    /// For UTF-8 collations (SQL Server 2019+), bit 27 (0x0800_0000) is set.
    pub lcid: u32,
    /// Sort ID.
    ///
    /// Used with certain collations to specify sorting behavior.
    pub sort_id: u8,
}

impl Collation {
    /// Returns the character encoding for this collation.
    ///
    /// This method maps the collation's LCID to the appropriate character
    /// encoding from the `encoding_rs` crate.
    ///
    /// # Returns
    ///
    /// - `Some(&Encoding)` - The encoding to use for decoding `VARCHAR` data
    /// - `None` - If the collation uses UTF-8 (no transcoding needed) or
    ///   the LCID is not recognized (caller should use Windows-1252 fallback)
    ///
    /// # UTF-8 Collations
    ///
    /// SQL Server 2019+ supports UTF-8 collations (identified by the `_UTF8`
    /// suffix). These return `None` because no transcoding is needed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let collation = Collation { lcid: 0x0419, sort_id: 0 }; // Russian
    /// if let Some(encoding) = collation.encoding() {
    ///     // encoding is Windows-1251 for Cyrillic
    ///     let (text, _, had_errors) = encoding.decode(&raw_bytes);
    /// }
    /// ```
    #[cfg(feature = "encoding")]
    pub fn encoding(&self) -> Option<&'static encoding_rs::Encoding> {
        crate::collation::encoding_for_lcid(self.lcid)
    }

    /// Returns whether this collation uses UTF-8 encoding.
    ///
    /// UTF-8 collations were introduced in SQL Server 2019 and are
    /// identified by the `_UTF8` suffix in the collation name.
    #[cfg(feature = "encoding")]
    pub fn is_utf8(&self) -> bool {
        crate::collation::is_utf8_collation(self.lcid)
    }

    /// Returns the Windows code page number for this collation.
    ///
    /// Useful for error messages and debugging.
    ///
    /// # Returns
    ///
    /// The code page number (e.g., 1252 for Western European, 932 for Japanese).
    #[cfg(feature = "encoding")]
    pub fn code_page(&self) -> Option<u16> {
        crate::collation::code_page_for_lcid(self.lcid)
    }

    /// Returns the encoding name for this collation.
    ///
    /// Useful for error messages and debugging.
    #[cfg(feature = "encoding")]
    pub fn encoding_name(&self) -> &'static str {
        crate::collation::encoding_name_for_lcid(self.lcid)
    }
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
// ColMetaData and Row Parsing Implementation
// =============================================================================

impl ColMetaData {
    /// Special value indicating no metadata.
    pub const NO_METADATA: u16 = 0xFFFF;

    /// Decode a COLMETADATA token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let column_count = src.get_u16_le();

        // 0xFFFF means no metadata present
        if column_count == Self::NO_METADATA {
            return Ok(Self {
                columns: Vec::new(),
            });
        }

        let mut columns = Vec::with_capacity(column_count as usize);

        for _ in 0..column_count {
            let column = Self::decode_column(src)?;
            columns.push(column);
        }

        Ok(Self { columns })
    }

    /// Decode a single column from the metadata.
    fn decode_column(src: &mut impl Buf) -> Result<ColumnData, ProtocolError> {
        // UserType (4 bytes) + Flags (2 bytes) + TypeId (1 byte)
        if src.remaining() < 7 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let user_type = src.get_u32_le();
        let flags = src.get_u16_le();
        let col_type = src.get_u8();

        let type_id = TypeId::from_u8(col_type).unwrap_or(TypeId::Null); // Default to Null for unknown types

        // Parse type-specific metadata
        let type_info = Self::decode_type_info(src, type_id, col_type)?;

        // Read column name (B_VARCHAR format - 1 byte length in characters)
        let name = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        Ok(ColumnData {
            name,
            type_id,
            col_type,
            flags,
            user_type,
            type_info,
        })
    }

    /// Decode type-specific metadata based on the type ID.
    fn decode_type_info(
        src: &mut impl Buf,
        type_id: TypeId,
        col_type: u8,
    ) -> Result<TypeInfo, ProtocolError> {
        match type_id {
            // Fixed-length types have no additional metadata
            TypeId::Null => Ok(TypeInfo::default()),
            TypeId::Int1 | TypeId::Bit => Ok(TypeInfo::default()),
            TypeId::Int2 => Ok(TypeInfo::default()),
            TypeId::Int4 => Ok(TypeInfo::default()),
            TypeId::Int8 => Ok(TypeInfo::default()),
            TypeId::Float4 => Ok(TypeInfo::default()),
            TypeId::Float8 => Ok(TypeInfo::default()),
            TypeId::Money => Ok(TypeInfo::default()),
            TypeId::Money4 => Ok(TypeInfo::default()),
            TypeId::DateTime => Ok(TypeInfo::default()),
            TypeId::DateTime4 => Ok(TypeInfo::default()),

            // Variable length integer/float/money (1-byte max length)
            TypeId::IntN | TypeId::BitN | TypeId::FloatN | TypeId::MoneyN | TypeId::DateTimeN => {
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u8() as u32;
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    ..Default::default()
                })
            }

            // GUID has 1-byte length
            TypeId::Guid => {
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u8() as u32;
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    ..Default::default()
                })
            }

            // Decimal/Numeric types (1-byte length + precision + scale)
            TypeId::Decimal | TypeId::Numeric | TypeId::DecimalN | TypeId::NumericN => {
                if src.remaining() < 3 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u8() as u32;
                let precision = src.get_u8();
                let scale = src.get_u8();
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    precision: Some(precision),
                    scale: Some(scale),
                    ..Default::default()
                })
            }

            // Old-style byte-length strings (Char, VarChar, Binary, VarBinary)
            TypeId::Char | TypeId::VarChar | TypeId::Binary | TypeId::VarBinary => {
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u8() as u32;
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    ..Default::default()
                })
            }

            // Big varchar/binary with 2-byte length + collation for strings
            TypeId::BigVarChar | TypeId::BigChar => {
                if src.remaining() < 7 {
                    // 2 (length) + 5 (collation)
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u16_le() as u32;
                let collation = Self::decode_collation(src)?;
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    collation: Some(collation),
                    ..Default::default()
                })
            }

            // Big binary (2-byte length, no collation)
            TypeId::BigVarBinary | TypeId::BigBinary => {
                if src.remaining() < 2 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u16_le() as u32;
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    ..Default::default()
                })
            }

            // Unicode strings (NChar, NVarChar) - 2-byte length + collation
            TypeId::NChar | TypeId::NVarChar => {
                if src.remaining() < 7 {
                    // 2 (length) + 5 (collation)
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u16_le() as u32;
                let collation = Self::decode_collation(src)?;
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    collation: Some(collation),
                    ..Default::default()
                })
            }

            // Date type (no additional metadata)
            TypeId::Date => Ok(TypeInfo::default()),

            // Time, DateTime2, DateTimeOffset have scale
            TypeId::Time | TypeId::DateTime2 | TypeId::DateTimeOffset => {
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let scale = src.get_u8();
                Ok(TypeInfo {
                    scale: Some(scale),
                    ..Default::default()
                })
            }

            // Text/NText/Image (deprecated LOB types)
            TypeId::Text | TypeId::NText | TypeId::Image => {
                // These have complex metadata: length (4) + collation (5) + table name parts
                if src.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u32_le();

                // For Text/NText, read collation
                let collation = if type_id == TypeId::Text || type_id == TypeId::NText {
                    if src.remaining() < 5 {
                        return Err(ProtocolError::UnexpectedEof);
                    }
                    Some(Self::decode_collation(src)?)
                } else {
                    None
                };

                // Skip table name parts (variable length)
                // Format: numParts (1 byte) followed by us_varchar for each part
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let num_parts = src.get_u8();
                for _ in 0..num_parts {
                    // Read and discard table name part
                    let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;
                }

                Ok(TypeInfo {
                    max_length: Some(max_length),
                    collation,
                    ..Default::default()
                })
            }

            // XML type
            TypeId::Xml => {
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let schema_present = src.get_u8();

                if schema_present != 0 {
                    // Read schema info (3 us_varchar strings)
                    let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // db name
                    let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // owning schema
                    let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // xml schema collection
                }

                Ok(TypeInfo::default())
            }

            // UDT (User-defined type) - complex metadata
            TypeId::Udt => {
                // Max length (2 bytes)
                if src.remaining() < 2 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u16_le() as u32;

                // UDT metadata: db name, schema name, type name, assembly qualified name
                let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // db name
                let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // schema name
                let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // type name
                let _ = read_us_varchar(src).ok_or(ProtocolError::UnexpectedEof)?; // assembly qualified name

                Ok(TypeInfo {
                    max_length: Some(max_length),
                    ..Default::default()
                })
            }

            // Table-valued parameter - complex metadata (skip for now)
            TypeId::Tvp => {
                // TVP has very complex metadata, not commonly used
                // For now, we can't properly parse this
                Err(ProtocolError::InvalidTokenType(col_type))
            }

            // SQL Variant - 4-byte length
            TypeId::Variant => {
                if src.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let max_length = src.get_u32_le();
                Ok(TypeInfo {
                    max_length: Some(max_length),
                    ..Default::default()
                })
            }
        }
    }

    /// Decode collation information (5 bytes).
    fn decode_collation(src: &mut impl Buf) -> Result<Collation, ProtocolError> {
        if src.remaining() < 5 {
            return Err(ProtocolError::UnexpectedEof);
        }
        // Collation: LCID (4 bytes) + Sort ID (1 byte)
        let lcid = src.get_u32_le();
        let sort_id = src.get_u8();
        Ok(Collation { lcid, sort_id })
    }

    /// Get the number of columns.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Check if this represents no metadata.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
}

impl ColumnData {
    /// Check if this column is nullable.
    #[must_use]
    pub fn is_nullable(&self) -> bool {
        (self.flags & 0x0001) != 0
    }

    /// Get the fixed size in bytes for this column, if applicable.
    ///
    /// Returns `None` for variable-length types.
    #[must_use]
    pub fn fixed_size(&self) -> Option<usize> {
        match self.type_id {
            TypeId::Null => Some(0),
            TypeId::Int1 | TypeId::Bit => Some(1),
            TypeId::Int2 => Some(2),
            TypeId::Int4 => Some(4),
            TypeId::Int8 => Some(8),
            TypeId::Float4 => Some(4),
            TypeId::Float8 => Some(8),
            TypeId::Money => Some(8),
            TypeId::Money4 => Some(4),
            TypeId::DateTime => Some(8),
            TypeId::DateTime4 => Some(4),
            TypeId::Date => Some(3),
            _ => None,
        }
    }
}

// =============================================================================
// Row Parsing Implementation
// =============================================================================

impl RawRow {
    /// Decode a ROW token from bytes.
    ///
    /// This function requires the column metadata to know how to parse the row.
    /// The row data is stored as raw bytes for later parsing.
    pub fn decode(src: &mut impl Buf, metadata: &ColMetaData) -> Result<Self, ProtocolError> {
        let mut data = bytes::BytesMut::new();

        for col in &metadata.columns {
            Self::decode_column_value(src, col, &mut data)?;
        }

        Ok(Self {
            data: data.freeze(),
        })
    }

    /// Decode a single column value and append to the output buffer.
    fn decode_column_value(
        src: &mut impl Buf,
        col: &ColumnData,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), ProtocolError> {
        match col.type_id {
            // Fixed-length types
            TypeId::Null => {
                // No data
            }
            TypeId::Int1 | TypeId::Bit => {
                if src.remaining() < 1 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&[src.get_u8()]);
            }
            TypeId::Int2 => {
                if src.remaining() < 2 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u16_le().to_le_bytes());
            }
            TypeId::Int4 => {
                if src.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u32_le().to_le_bytes());
            }
            TypeId::Int8 => {
                if src.remaining() < 8 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u64_le().to_le_bytes());
            }
            TypeId::Float4 => {
                if src.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u32_le().to_le_bytes());
            }
            TypeId::Float8 => {
                if src.remaining() < 8 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u64_le().to_le_bytes());
            }
            TypeId::Money => {
                if src.remaining() < 8 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let hi = src.get_u32_le();
                let lo = src.get_u32_le();
                dst.extend_from_slice(&hi.to_le_bytes());
                dst.extend_from_slice(&lo.to_le_bytes());
            }
            TypeId::Money4 => {
                if src.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u32_le().to_le_bytes());
            }
            TypeId::DateTime => {
                if src.remaining() < 8 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let days = src.get_u32_le();
                let time = src.get_u32_le();
                dst.extend_from_slice(&days.to_le_bytes());
                dst.extend_from_slice(&time.to_le_bytes());
            }
            TypeId::DateTime4 => {
                if src.remaining() < 4 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                dst.extend_from_slice(&src.get_u32_le().to_le_bytes());
            }
            // DATE type uses 1-byte length prefix (can be NULL)
            TypeId::Date => {
                Self::decode_bytelen_type(src, dst)?;
            }

            // Variable-length nullable types (length-prefixed)
            TypeId::IntN | TypeId::BitN | TypeId::FloatN | TypeId::MoneyN | TypeId::DateTimeN => {
                Self::decode_bytelen_type(src, dst)?;
            }

            TypeId::Guid => {
                Self::decode_bytelen_type(src, dst)?;
            }

            TypeId::Decimal | TypeId::Numeric | TypeId::DecimalN | TypeId::NumericN => {
                Self::decode_bytelen_type(src, dst)?;
            }

            // Old-style byte-length strings
            TypeId::Char | TypeId::VarChar | TypeId::Binary | TypeId::VarBinary => {
                Self::decode_bytelen_type(src, dst)?;
            }

            // 2-byte length strings (or PLP for MAX types)
            TypeId::BigVarChar | TypeId::BigVarBinary => {
                // max_length == 0xFFFF indicates VARCHAR(MAX) or VARBINARY(MAX), which uses PLP
                if col.type_info.max_length == Some(0xFFFF) {
                    Self::decode_plp_type(src, dst)?;
                } else {
                    Self::decode_ushortlen_type(src, dst)?;
                }
            }

            // Fixed-length types that don't have MAX variants
            TypeId::BigChar | TypeId::BigBinary => {
                Self::decode_ushortlen_type(src, dst)?;
            }

            // Unicode strings (2-byte length in bytes, or PLP for NVARCHAR(MAX))
            TypeId::NVarChar => {
                // max_length == 0xFFFF indicates NVARCHAR(MAX), which uses PLP
                if col.type_info.max_length == Some(0xFFFF) {
                    Self::decode_plp_type(src, dst)?;
                } else {
                    Self::decode_ushortlen_type(src, dst)?;
                }
            }

            // Fixed-length NCHAR doesn't have MAX variant
            TypeId::NChar => {
                Self::decode_ushortlen_type(src, dst)?;
            }

            // Time types with scale
            TypeId::Time | TypeId::DateTime2 | TypeId::DateTimeOffset => {
                Self::decode_bytelen_type(src, dst)?;
            }

            // TEXT/NTEXT/IMAGE - deprecated LOB types using textptr format
            TypeId::Text | TypeId::NText | TypeId::Image => {
                Self::decode_textptr_type(src, dst)?;
            }

            // XML - uses actual PLP format
            TypeId::Xml => {
                Self::decode_plp_type(src, dst)?;
            }

            // Complex types
            TypeId::Variant => {
                Self::decode_intlen_type(src, dst)?;
            }

            TypeId::Udt => {
                // UDT uses PLP encoding
                Self::decode_plp_type(src, dst)?;
            }

            TypeId::Tvp => {
                // TVP not supported in row data
                return Err(ProtocolError::InvalidTokenType(col.col_type));
            }
        }

        Ok(())
    }

    /// Decode a 1-byte length-prefixed value.
    fn decode_bytelen_type(
        src: &mut impl Buf,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), ProtocolError> {
        if src.remaining() < 1 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let len = src.get_u8() as usize;
        if len == 0xFF {
            // NULL value - store as zero-length with NULL marker
            dst.extend_from_slice(&[0xFF]);
        } else if len == 0 {
            // Empty value
            dst.extend_from_slice(&[0x00]);
        } else {
            if src.remaining() < len {
                return Err(ProtocolError::UnexpectedEof);
            }
            dst.extend_from_slice(&[len as u8]);
            for _ in 0..len {
                dst.extend_from_slice(&[src.get_u8()]);
            }
        }
        Ok(())
    }

    /// Decode a 2-byte length-prefixed value.
    fn decode_ushortlen_type(
        src: &mut impl Buf,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), ProtocolError> {
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let len = src.get_u16_le() as usize;
        if len == 0xFFFF {
            // NULL value
            dst.extend_from_slice(&0xFFFFu16.to_le_bytes());
        } else if len == 0 {
            // Empty value
            dst.extend_from_slice(&0u16.to_le_bytes());
        } else {
            if src.remaining() < len {
                return Err(ProtocolError::UnexpectedEof);
            }
            dst.extend_from_slice(&(len as u16).to_le_bytes());
            for _ in 0..len {
                dst.extend_from_slice(&[src.get_u8()]);
            }
        }
        Ok(())
    }

    /// Decode a 4-byte length-prefixed value.
    fn decode_intlen_type(
        src: &mut impl Buf,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), ProtocolError> {
        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let len = src.get_u32_le() as usize;
        if len == 0xFFFFFFFF {
            // NULL value
            dst.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        } else if len == 0 {
            // Empty value
            dst.extend_from_slice(&0u32.to_le_bytes());
        } else {
            if src.remaining() < len {
                return Err(ProtocolError::UnexpectedEof);
            }
            dst.extend_from_slice(&(len as u32).to_le_bytes());
            for _ in 0..len {
                dst.extend_from_slice(&[src.get_u8()]);
            }
        }
        Ok(())
    }

    /// Decode a TEXT/NTEXT/IMAGE type (textptr format).
    ///
    /// These deprecated LOB types use a special format:
    /// - 1 byte: textptr_len (0 = NULL)
    /// - textptr_len bytes: textptr (if not NULL)
    /// - 8 bytes: timestamp (if not NULL)
    /// - 4 bytes: data length (if not NULL)
    /// - data_len bytes: the actual data (if not NULL)
    ///
    /// We convert this to PLP format for the client to parse:
    /// - 8 bytes: total length (0xFFFFFFFFFFFFFFFF = NULL)
    /// - 4 bytes: chunk length (= data length)
    /// - chunk data
    /// - 4 bytes: 0 (terminator)
    fn decode_textptr_type(
        src: &mut impl Buf,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), ProtocolError> {
        if src.remaining() < 1 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let textptr_len = src.get_u8() as usize;

        if textptr_len == 0 {
            // NULL value - write PLP NULL marker
            dst.extend_from_slice(&0xFFFFFFFFFFFFFFFFu64.to_le_bytes());
            return Ok(());
        }

        // Skip textptr bytes
        if src.remaining() < textptr_len {
            return Err(ProtocolError::UnexpectedEof);
        }
        src.advance(textptr_len);

        // Skip 8-byte timestamp
        if src.remaining() < 8 {
            return Err(ProtocolError::UnexpectedEof);
        }
        src.advance(8);

        // Read data length
        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let data_len = src.get_u32_le() as usize;

        if src.remaining() < data_len {
            return Err(ProtocolError::UnexpectedEof);
        }

        // Write in PLP format for client parsing:
        // - 8 bytes: total length
        // - 4 bytes: chunk length
        // - chunk data
        // - 4 bytes: 0 (terminator)
        dst.extend_from_slice(&(data_len as u64).to_le_bytes());
        dst.extend_from_slice(&(data_len as u32).to_le_bytes());
        for _ in 0..data_len {
            dst.extend_from_slice(&[src.get_u8()]);
        }
        dst.extend_from_slice(&0u32.to_le_bytes()); // PLP terminator

        Ok(())
    }

    /// Decode a PLP (Partially Length-Prefixed) value.
    ///
    /// PLP format:
    /// - 8 bytes: total length (0xFFFFFFFFFFFFFFFE = unknown, 0xFFFFFFFFFFFFFFFF = NULL)
    /// - If not NULL: chunks of (4 byte chunk length + data) until chunk length = 0
    fn decode_plp_type(src: &mut impl Buf, dst: &mut bytes::BytesMut) -> Result<(), ProtocolError> {
        if src.remaining() < 8 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let total_len = src.get_u64_le();

        // Store the total length marker
        dst.extend_from_slice(&total_len.to_le_bytes());

        if total_len == 0xFFFFFFFFFFFFFFFF {
            // NULL value - no more data
            return Ok(());
        }

        // Read chunks until terminator
        loop {
            if src.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }
            let chunk_len = src.get_u32_le() as usize;
            dst.extend_from_slice(&(chunk_len as u32).to_le_bytes());

            if chunk_len == 0 {
                // End of PLP data
                break;
            }

            if src.remaining() < chunk_len {
                return Err(ProtocolError::UnexpectedEof);
            }

            for _ in 0..chunk_len {
                dst.extend_from_slice(&[src.get_u8()]);
            }
        }

        Ok(())
    }
}

// =============================================================================
// NbcRow Parsing Implementation
// =============================================================================

impl NbcRow {
    /// Decode an NBCROW token from bytes.
    ///
    /// NBCROW (Null Bitmap Compressed Row) stores a bitmap indicating which
    /// columns are NULL, followed by only the non-NULL values.
    pub fn decode(src: &mut impl Buf, metadata: &ColMetaData) -> Result<Self, ProtocolError> {
        let col_count = metadata.columns.len();
        let bitmap_len = (col_count + 7) / 8;

        if src.remaining() < bitmap_len {
            return Err(ProtocolError::UnexpectedEof);
        }

        // Read null bitmap
        let mut null_bitmap = vec![0u8; bitmap_len];
        for byte in &mut null_bitmap {
            *byte = src.get_u8();
        }

        // Read non-null values
        let mut data = bytes::BytesMut::new();

        for (i, col) in metadata.columns.iter().enumerate() {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let is_null = (null_bitmap[byte_idx] & (1 << bit_idx)) != 0;

            if !is_null {
                // Read the value - for NBCROW, we read without the length prefix
                // for fixed-length types, and with length prefix for variable types
                RawRow::decode_column_value(src, col, &mut data)?;
            }
        }

        Ok(Self {
            null_bitmap,
            data: data.freeze(),
        })
    }

    /// Check if a column at the given index is NULL.
    #[must_use]
    pub fn is_null(&self, column_index: usize) -> bool {
        let byte_idx = column_index / 8;
        let bit_idx = column_index % 8;
        if byte_idx < self.null_bitmap.len() {
            (self.null_bitmap[byte_idx] & (1 << bit_idx)) != 0
        } else {
            true // Out of bounds = NULL
        }
    }
}

// =============================================================================
// ReturnValue Parsing Implementation
// =============================================================================

impl ReturnValue {
    /// Decode a RETURNVALUE token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        // Length (2 bytes)
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let _length = src.get_u16_le();

        // Parameter ordinal (2 bytes)
        if src.remaining() < 2 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let param_ordinal = src.get_u16_le();

        // Parameter name (B_VARCHAR)
        let param_name = read_b_varchar(src).ok_or(ProtocolError::UnexpectedEof)?;

        // Status (1 byte)
        if src.remaining() < 1 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let status = src.get_u8();

        // User type (4 bytes) + flags (2 bytes) + type id (1 byte)
        if src.remaining() < 7 {
            return Err(ProtocolError::UnexpectedEof);
        }
        let user_type = src.get_u32_le();
        let flags = src.get_u16_le();
        let col_type = src.get_u8();

        let type_id = TypeId::from_u8(col_type).unwrap_or(TypeId::Null);

        // Parse type info
        let type_info = ColMetaData::decode_type_info(src, type_id, col_type)?;

        // Read the value data
        let mut value_buf = bytes::BytesMut::new();

        // Create a temporary column for value parsing
        let temp_col = ColumnData {
            name: String::new(),
            type_id,
            col_type,
            flags,
            user_type,
            type_info: type_info.clone(),
        };

        RawRow::decode_column_value(src, &temp_col, &mut value_buf)?;

        Ok(Self {
            param_ordinal,
            param_name,
            status,
            user_type,
            flags,
            type_info,
            value: value_buf.freeze(),
        })
    }
}

// =============================================================================
// SessionState Parsing Implementation
// =============================================================================

impl SessionState {
    /// Decode a SESSIONSTATE token from bytes.
    pub fn decode(src: &mut impl Buf) -> Result<Self, ProtocolError> {
        if src.remaining() < 4 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let length = src.get_u32_le() as usize;

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
            | EnvChangeType::EnlistDtcTransaction
            | EnvChangeType::SqlCollation => {
                // These use binary format per MS-TDS spec:
                // - Transaction tokens: transaction descriptor (8 bytes)
                // - SqlCollation: collation info (5 bytes: LCID + sort flags)
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
///
/// # Basic vs Context-Aware Parsing
///
/// Some tokens (like `Done`, `Error`, `LoginAck`) can be parsed without context.
/// Use [`next_token()`](TokenParser::next_token) for these.
///
/// Other tokens (like `ColMetaData`, `Row`, `NbcRow`) require column metadata
/// to parse correctly. Use [`next_token_with_metadata()`](TokenParser::next_token_with_metadata)
/// for these.
///
/// # Example
///
/// ```rust,ignore
/// let mut parser = TokenParser::new(data);
/// let mut metadata = None;
///
/// while let Some(token) = parser.next_token_with_metadata(metadata.as_ref())? {
///     match token {
///         Token::ColMetaData(meta) => {
///             metadata = Some(meta);
///         }
///         Token::Row(row) => {
///             // Process row using metadata
///         }
///         Token::Done(done) => {
///             if !done.has_more() {
///                 break;
///             }
///         }
///         _ => {}
///     }
/// }
/// ```
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

    /// Peek at the next token type without consuming it.
    #[must_use]
    pub fn peek_token_type(&self) -> Option<TokenType> {
        if self.position < self.data.len() {
            TokenType::from_u8(self.data[self.position])
        } else {
            None
        }
    }

    /// Parse the next token from the stream.
    ///
    /// This method can only parse context-independent tokens. For tokens that
    /// require column metadata (ColMetaData, Row, NbcRow), use
    /// [`next_token_with_metadata()`](TokenParser::next_token_with_metadata).
    ///
    /// Returns `None` if no more tokens are available.
    pub fn next_token(&mut self) -> Result<Option<Token>, ProtocolError> {
        self.next_token_with_metadata(None)
    }

    /// Parse the next token with optional column metadata context.
    ///
    /// When `metadata` is provided, this method can parse Row and NbcRow tokens.
    /// Without metadata, those tokens will return an error.
    ///
    /// Returns `None` if no more tokens are available.
    pub fn next_token_with_metadata(
        &mut self,
        metadata: Option<&ColMetaData>,
    ) -> Result<Option<Token>, ProtocolError> {
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
            Some(TokenType::ColMetaData) => {
                let col_meta = ColMetaData::decode(&mut buf)?;
                Token::ColMetaData(col_meta)
            }
            Some(TokenType::Row) => {
                let meta = metadata.ok_or_else(|| {
                    ProtocolError::StringEncoding(
                        #[cfg(feature = "std")]
                        "Row token requires column metadata".to_string(),
                        #[cfg(not(feature = "std"))]
                        "Row token requires column metadata",
                    )
                })?;
                let row = RawRow::decode(&mut buf, meta)?;
                Token::Row(row)
            }
            Some(TokenType::NbcRow) => {
                let meta = metadata.ok_or_else(|| {
                    ProtocolError::StringEncoding(
                        #[cfg(feature = "std")]
                        "NbcRow token requires column metadata".to_string(),
                        #[cfg(not(feature = "std"))]
                        "NbcRow token requires column metadata",
                    )
                })?;
                let row = NbcRow::decode(&mut buf, meta)?;
                Token::NbcRow(row)
            }
            Some(TokenType::ReturnValue) => {
                let ret_val = ReturnValue::decode(&mut buf)?;
                Token::ReturnValue(ret_val)
            }
            Some(TokenType::SessionState) => {
                let session = SessionState::decode(&mut buf)?;
                Token::SessionState(session)
            }
            Some(TokenType::ColInfo) | Some(TokenType::TabName) | Some(TokenType::Offset) => {
                // These tokens are rarely used and have complex formats.
                // Skip them by reading the length and advancing.
                if buf.remaining() < 2 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let length = buf.get_u16_le() as usize;
                if buf.remaining() < length {
                    return Err(ProtocolError::IncompletePacket {
                        expected: length,
                        actual: buf.remaining(),
                    });
                }
                // Skip the data
                buf.advance(length);
                // Recursively get the next token
                self.position = start_pos + (self.data.len() - start_pos - buf.remaining());
                return self.next_token_with_metadata(metadata);
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

    /// Skip the current token without fully parsing it.
    ///
    /// This is useful for skipping unknown or uninteresting tokens.
    pub fn skip_token(&mut self) -> Result<(), ProtocolError> {
        if !self.has_remaining() {
            return Ok(());
        }

        let token_type_byte = self.data[self.position];
        let token_type = TokenType::from_u8(token_type_byte);

        // Calculate how many bytes to skip based on token type
        let skip_amount = match token_type {
            // Fixed-size tokens
            Some(TokenType::Done) | Some(TokenType::DoneProc) | Some(TokenType::DoneInProc) => {
                1 + Done::SIZE // token type + 12 bytes
            }
            Some(TokenType::ReturnStatus) => {
                1 + 4 // token type + 4 bytes
            }
            // Variable-length tokens with 2-byte length prefix
            Some(TokenType::Error)
            | Some(TokenType::Info)
            | Some(TokenType::LoginAck)
            | Some(TokenType::EnvChange)
            | Some(TokenType::Order)
            | Some(TokenType::Sspi)
            | Some(TokenType::ColInfo)
            | Some(TokenType::TabName)
            | Some(TokenType::Offset)
            | Some(TokenType::ReturnValue) => {
                if self.remaining() < 3 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let length = u16::from_le_bytes([
                    self.data[self.position + 1],
                    self.data[self.position + 2],
                ]) as usize;
                1 + 2 + length // token type + length prefix + data
            }
            // Tokens with 4-byte length prefix
            Some(TokenType::SessionState) | Some(TokenType::FedAuthInfo) => {
                if self.remaining() < 5 {
                    return Err(ProtocolError::UnexpectedEof);
                }
                let length = u32::from_le_bytes([
                    self.data[self.position + 1],
                    self.data[self.position + 2],
                    self.data[self.position + 3],
                    self.data[self.position + 4],
                ]) as usize;
                1 + 4 + length
            }
            // FeatureExtAck has no length prefix - must parse
            Some(TokenType::FeatureExtAck) => {
                // Parse to find end
                let mut buf = &self.data[self.position + 1..];
                let _ = FeatureExtAck::decode(&mut buf)?;
                self.data.len() - self.position - buf.remaining()
            }
            // ColMetaData, Row, NbcRow require context and can't be easily skipped
            Some(TokenType::ColMetaData) | Some(TokenType::Row) | Some(TokenType::NbcRow) => {
                return Err(ProtocolError::InvalidTokenType(token_type_byte));
            }
            None => {
                return Err(ProtocolError::InvalidTokenType(token_type_byte));
            }
        };

        if self.remaining() < skip_amount {
            return Err(ProtocolError::UnexpectedEof);
        }

        self.position += skip_amount;
        Ok(())
    }

    /// Get the current position in the buffer.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// Reset the parser to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
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
        assert_eq!(EnvChangeType::from_u8(1), Some(EnvChangeType::Database));
        assert_eq!(EnvChangeType::from_u8(20), Some(EnvChangeType::Routing));
        assert_eq!(EnvChangeType::from_u8(100), None);
    }

    #[test]
    fn test_colmetadata_no_columns() {
        // No metadata marker (0xFFFF)
        let data = Bytes::from_static(&[0xFF, 0xFF]);
        let mut cursor: &[u8] = &data;
        let meta = ColMetaData::decode(&mut cursor).unwrap();
        assert!(meta.is_empty());
        assert_eq!(meta.column_count(), 0);
    }

    #[test]
    fn test_colmetadata_single_int_column() {
        // COLMETADATA with 1 INT column
        // Format: column_count (2) + [user_type (4) + flags (2) + type_id (1) + name (b_varchar)]
        let mut data = BytesMut::new();
        data.extend_from_slice(&[0x01, 0x00]); // 1 column
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user_type = 0
        data.extend_from_slice(&[0x01, 0x00]); // flags (nullable)
        data.extend_from_slice(&[0x38]); // TypeId::Int4
        // Column name "id" as B_VARCHAR (1 byte length + UTF-16LE)
        data.extend_from_slice(&[0x02]); // 2 characters
        data.extend_from_slice(&[b'i', 0x00, b'd', 0x00]); // "id" in UTF-16LE

        let mut cursor: &[u8] = &data;
        let meta = ColMetaData::decode(&mut cursor).unwrap();

        assert_eq!(meta.column_count(), 1);
        assert_eq!(meta.columns[0].name, "id");
        assert_eq!(meta.columns[0].type_id, TypeId::Int4);
        assert!(meta.columns[0].is_nullable());
    }

    #[test]
    fn test_colmetadata_nvarchar_column() {
        // COLMETADATA with 1 NVARCHAR(50) column
        let mut data = BytesMut::new();
        data.extend_from_slice(&[0x01, 0x00]); // 1 column
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user_type = 0
        data.extend_from_slice(&[0x01, 0x00]); // flags (nullable)
        data.extend_from_slice(&[0xE7]); // TypeId::NVarChar
        // Type info: max_length (2 bytes) + collation (5 bytes)
        data.extend_from_slice(&[0x64, 0x00]); // max_length = 100 (50 chars * 2)
        data.extend_from_slice(&[0x09, 0x04, 0xD0, 0x00, 0x34]); // collation
        // Column name "name"
        data.extend_from_slice(&[0x04]); // 4 characters
        data.extend_from_slice(&[b'n', 0x00, b'a', 0x00, b'm', 0x00, b'e', 0x00]);

        let mut cursor: &[u8] = &data;
        let meta = ColMetaData::decode(&mut cursor).unwrap();

        assert_eq!(meta.column_count(), 1);
        assert_eq!(meta.columns[0].name, "name");
        assert_eq!(meta.columns[0].type_id, TypeId::NVarChar);
        assert_eq!(meta.columns[0].type_info.max_length, Some(100));
        assert!(meta.columns[0].type_info.collation.is_some());
    }

    #[test]
    fn test_raw_row_decode_int() {
        // Create metadata for a single INT column
        let metadata = ColMetaData {
            columns: vec![ColumnData {
                name: "id".to_string(),
                type_id: TypeId::Int4,
                col_type: 0x38,
                flags: 0,
                user_type: 0,
                type_info: TypeInfo::default(),
            }],
        };

        // Row data: just 4 bytes for the int value 42
        let data = Bytes::from_static(&[0x2A, 0x00, 0x00, 0x00]); // 42 in little-endian
        let mut cursor: &[u8] = &data;
        let row = RawRow::decode(&mut cursor, &metadata).unwrap();

        // The raw data should contain the 4 bytes
        assert_eq!(row.data.len(), 4);
        assert_eq!(&row.data[..], &[0x2A, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_raw_row_decode_nullable_int() {
        // Create metadata for a nullable INT column (IntN)
        let metadata = ColMetaData {
            columns: vec![ColumnData {
                name: "id".to_string(),
                type_id: TypeId::IntN,
                col_type: 0x26,
                flags: 0x01, // nullable
                user_type: 0,
                type_info: TypeInfo {
                    max_length: Some(4),
                    ..Default::default()
                },
            }],
        };

        // Row data with value: 1 byte length + 4 bytes value
        let data = Bytes::from_static(&[0x04, 0x2A, 0x00, 0x00, 0x00]); // length=4, value=42
        let mut cursor: &[u8] = &data;
        let row = RawRow::decode(&mut cursor, &metadata).unwrap();

        assert_eq!(row.data.len(), 5);
        assert_eq!(row.data[0], 4); // length
        assert_eq!(&row.data[1..], &[0x2A, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_raw_row_decode_null_value() {
        // Create metadata for a nullable INT column (IntN)
        let metadata = ColMetaData {
            columns: vec![ColumnData {
                name: "id".to_string(),
                type_id: TypeId::IntN,
                col_type: 0x26,
                flags: 0x01, // nullable
                user_type: 0,
                type_info: TypeInfo {
                    max_length: Some(4),
                    ..Default::default()
                },
            }],
        };

        // NULL value: length = 0xFF (for bytelen types)
        let data = Bytes::from_static(&[0xFF]);
        let mut cursor: &[u8] = &data;
        let row = RawRow::decode(&mut cursor, &metadata).unwrap();

        assert_eq!(row.data.len(), 1);
        assert_eq!(row.data[0], 0xFF); // NULL marker
    }

    #[test]
    fn test_nbcrow_null_bitmap() {
        let row = NbcRow {
            null_bitmap: vec![0b00000101], // columns 0 and 2 are NULL
            data: Bytes::new(),
        };

        assert!(row.is_null(0));
        assert!(!row.is_null(1));
        assert!(row.is_null(2));
        assert!(!row.is_null(3));
    }

    #[test]
    fn test_token_parser_colmetadata() {
        // Build a COLMETADATA token with 1 INT column
        let mut data = BytesMut::new();
        data.extend_from_slice(&[0x81]); // COLMETADATA token type
        data.extend_from_slice(&[0x01, 0x00]); // 1 column
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user_type = 0
        data.extend_from_slice(&[0x01, 0x00]); // flags (nullable)
        data.extend_from_slice(&[0x38]); // TypeId::Int4
        data.extend_from_slice(&[0x02]); // column name length
        data.extend_from_slice(&[b'i', 0x00, b'd', 0x00]); // "id"

        let mut parser = TokenParser::new(data.freeze());
        let token = parser.next_token().unwrap().unwrap();

        match token {
            Token::ColMetaData(meta) => {
                assert_eq!(meta.column_count(), 1);
                assert_eq!(meta.columns[0].name, "id");
                assert_eq!(meta.columns[0].type_id, TypeId::Int4);
            }
            _ => panic!("Expected ColMetaData token"),
        }
    }

    #[test]
    fn test_token_parser_row_with_metadata() {
        // Build metadata
        let metadata = ColMetaData {
            columns: vec![ColumnData {
                name: "id".to_string(),
                type_id: TypeId::Int4,
                col_type: 0x38,
                flags: 0,
                user_type: 0,
                type_info: TypeInfo::default(),
            }],
        };

        // Build ROW token
        let mut data = BytesMut::new();
        data.extend_from_slice(&[0xD1]); // ROW token type
        data.extend_from_slice(&[0x2A, 0x00, 0x00, 0x00]); // value = 42

        let mut parser = TokenParser::new(data.freeze());
        let token = parser
            .next_token_with_metadata(Some(&metadata))
            .unwrap()
            .unwrap();

        match token {
            Token::Row(row) => {
                assert_eq!(row.data.len(), 4);
            }
            _ => panic!("Expected Row token"),
        }
    }

    #[test]
    fn test_token_parser_row_without_metadata_fails() {
        // Build ROW token
        let mut data = BytesMut::new();
        data.extend_from_slice(&[0xD1]); // ROW token type
        data.extend_from_slice(&[0x2A, 0x00, 0x00, 0x00]); // value = 42

        let mut parser = TokenParser::new(data.freeze());
        let result = parser.next_token(); // No metadata provided

        assert!(result.is_err());
    }

    #[test]
    fn test_token_parser_peek() {
        let data = Bytes::from_static(&[
            0xFD, // DONE token type
            0x10, 0x00, // status
            0xC1, 0x00, // cur_cmd
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_count
        ]);

        let parser = TokenParser::new(data);
        assert_eq!(parser.peek_token_type(), Some(TokenType::Done));
    }

    #[test]
    fn test_column_data_fixed_size() {
        let col = ColumnData {
            name: String::new(),
            type_id: TypeId::Int4,
            col_type: 0x38,
            flags: 0,
            user_type: 0,
            type_info: TypeInfo::default(),
        };
        assert_eq!(col.fixed_size(), Some(4));

        let col2 = ColumnData {
            name: String::new(),
            type_id: TypeId::NVarChar,
            col_type: 0xE7,
            flags: 0,
            user_type: 0,
            type_info: TypeInfo::default(),
        };
        assert_eq!(col2.fixed_size(), None);
    }

    // ========================================================================
    // End-to-End Decode Tests (Wire → Stored → Verification)
    // ========================================================================
    //
    // These tests verify that RawRow::decode_column_value correctly stores
    // column values in a format that can be parsed back.

    #[test]
    fn test_decode_nvarchar_then_intn_roundtrip() {
        // Simulate wire data for: "World" (NVarChar), 42 (IntN)
        // This tests the scenario from the MCP parameterized query

        // Build wire data (what the server sends)
        let mut wire_data = BytesMut::new();

        // Column 0: NVarChar "World" - 2-byte length prefix in bytes
        // "World" in UTF-16LE: W=0x0057, o=0x006F, r=0x0072, l=0x006C, d=0x0064
        let word = "World";
        let utf16: Vec<u16> = word.encode_utf16().collect();
        wire_data.put_u16_le((utf16.len() * 2) as u16); // byte length = 10
        for code_unit in &utf16 {
            wire_data.put_u16_le(*code_unit);
        }

        // Column 1: IntN 42 - 1-byte length prefix
        wire_data.put_u8(4); // 4 bytes for INT
        wire_data.put_i32_le(42);

        // Build column metadata
        let metadata = ColMetaData {
            columns: vec![
                ColumnData {
                    name: "greeting".to_string(),
                    type_id: TypeId::NVarChar,
                    col_type: 0xE7,
                    flags: 0x01,
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(10), // non-MAX
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                },
                ColumnData {
                    name: "number".to_string(),
                    type_id: TypeId::IntN,
                    col_type: 0x26,
                    flags: 0x01,
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(4),
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                },
            ],
        };

        // Decode the wire data into stored format
        let mut wire_cursor = wire_data.freeze();
        let raw_row = RawRow::decode(&mut wire_cursor, &metadata).unwrap();

        // Verify wire data was fully consumed
        assert_eq!(
            wire_cursor.remaining(),
            0,
            "wire data should be fully consumed"
        );

        // Now parse the stored data
        let mut stored_cursor: &[u8] = &raw_row.data;

        // Parse column 0 (NVarChar)
        // Stored format for non-MAX NVarChar: [2-byte len][data]
        assert!(
            stored_cursor.remaining() >= 2,
            "need at least 2 bytes for length"
        );
        let len0 = stored_cursor.get_u16_le() as usize;
        assert_eq!(len0, 10, "NVarChar length should be 10 bytes");
        assert!(
            stored_cursor.remaining() >= len0,
            "need {len0} bytes for data"
        );

        // Read UTF-16LE and convert to string
        let mut utf16_read = Vec::new();
        for _ in 0..(len0 / 2) {
            utf16_read.push(stored_cursor.get_u16_le());
        }
        let string0 = String::from_utf16(&utf16_read).unwrap();
        assert_eq!(string0, "World", "column 0 should be 'World'");

        // Parse column 1 (IntN)
        // Stored format for IntN: [1-byte len][data]
        assert!(
            stored_cursor.remaining() >= 1,
            "need at least 1 byte for length"
        );
        let len1 = stored_cursor.get_u8();
        assert_eq!(len1, 4, "IntN length should be 4");
        assert!(stored_cursor.remaining() >= 4, "need 4 bytes for INT data");
        let int1 = stored_cursor.get_i32_le();
        assert_eq!(int1, 42, "column 1 should be 42");

        // Verify stored data was fully consumed
        assert_eq!(
            stored_cursor.remaining(),
            0,
            "stored data should be fully consumed"
        );
    }

    #[test]
    fn test_decode_nvarchar_max_then_intn_roundtrip() {
        // Test NVARCHAR(MAX) followed by IntN - uses PLP encoding

        // Build wire data for PLP NVARCHAR(MAX) + IntN
        let mut wire_data = BytesMut::new();

        // Column 0: NVARCHAR(MAX) "Hello" - PLP format
        // PLP: 8-byte total length, then chunks
        let word = "Hello";
        let utf16: Vec<u16> = word.encode_utf16().collect();
        let byte_len = (utf16.len() * 2) as u64;

        wire_data.put_u64_le(byte_len); // total length = 10
        wire_data.put_u32_le(byte_len as u32); // chunk length = 10
        for code_unit in &utf16 {
            wire_data.put_u16_le(*code_unit);
        }
        wire_data.put_u32_le(0); // terminating zero-length chunk

        // Column 1: IntN 99
        wire_data.put_u8(4);
        wire_data.put_i32_le(99);

        // Build metadata with MAX type
        let metadata = ColMetaData {
            columns: vec![
                ColumnData {
                    name: "text".to_string(),
                    type_id: TypeId::NVarChar,
                    col_type: 0xE7,
                    flags: 0x01,
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(0xFFFF), // MAX indicator
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                },
                ColumnData {
                    name: "num".to_string(),
                    type_id: TypeId::IntN,
                    col_type: 0x26,
                    flags: 0x01,
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(4),
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                },
            ],
        };

        // Decode wire data
        let mut wire_cursor = wire_data.freeze();
        let raw_row = RawRow::decode(&mut wire_cursor, &metadata).unwrap();

        // Verify wire data was fully consumed
        assert_eq!(
            wire_cursor.remaining(),
            0,
            "wire data should be fully consumed"
        );

        // Parse stored PLP data for column 0
        let mut stored_cursor: &[u8] = &raw_row.data;

        // PLP stored format: [8-byte total][chunks...][4-byte 0]
        let total_len = stored_cursor.get_u64_le();
        assert_eq!(total_len, 10, "PLP total length should be 10");

        let chunk_len = stored_cursor.get_u32_le();
        assert_eq!(chunk_len, 10, "PLP chunk length should be 10");

        let mut utf16_read = Vec::new();
        for _ in 0..(chunk_len / 2) {
            utf16_read.push(stored_cursor.get_u16_le());
        }
        let string0 = String::from_utf16(&utf16_read).unwrap();
        assert_eq!(string0, "Hello", "column 0 should be 'Hello'");

        let terminator = stored_cursor.get_u32_le();
        assert_eq!(terminator, 0, "PLP should end with 0");

        // Parse IntN
        let len1 = stored_cursor.get_u8();
        assert_eq!(len1, 4);
        let int1 = stored_cursor.get_i32_le();
        assert_eq!(int1, 99, "column 1 should be 99");

        // Verify fully consumed
        assert_eq!(
            stored_cursor.remaining(),
            0,
            "stored data should be fully consumed"
        );
    }
}
