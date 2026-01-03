//! # tds-protocol
//!
//! Pure implementation of the MS-TDS (Tabular Data Stream) protocol used by
//! Microsoft SQL Server.
//!
//! This crate provides `no_std` compatible packet structures, token parsing,
//! and serialization for TDS protocol versions 7.4 through 8.0.
//!
//! ## Features
//!
//! - `std` (default): Enable standard library support
//! - `alloc`: Enable allocation without full std (requires `alloc` crate)
//! - `encoding` (default): Enable collation-aware string encoding/decoding
//!   via the [`Collation::encoding()`] method. Uses the `encoding_rs` crate.
//!
//! ## Collation-Aware VARCHAR Decoding
//!
//! When the `encoding` feature is enabled (default), this crate provides
//! comprehensive support for decoding VARCHAR data with locale-specific
//! character encodings. This is essential for databases using non-ASCII
//! collations (e.g., Japanese, Chinese, Korean, Cyrillic, Arabic, etc.).
//!
//! ### Supported Encodings
//!
//! | Code Page | Encoding | Languages |
//! |-----------|----------|-----------|
//! | 874 | Windows-874 (TIS-620) | Thai |
//! | 932 | Shift_JIS | Japanese |
//! | 936 | GBK/GB18030 | Simplified Chinese |
//! | 949 | EUC-KR | Korean |
//! | 950 | Big5 | Traditional Chinese |
//! | 1250 | Windows-1250 | Central/Eastern European |
//! | 1251 | Windows-1251 | Cyrillic |
//! | 1252 | Windows-1252 | Western European (default) |
//! | 1253 | Windows-1253 | Greek |
//! | 1254 | Windows-1254 | Turkish |
//! | 1255 | Windows-1255 | Hebrew |
//! | 1256 | Windows-1256 | Arabic |
//! | 1257 | Windows-1257 | Baltic |
//! | 1258 | Windows-1258 | Vietnamese |
//! | UTF-8 | UTF-8 | SQL Server 2019+ collations |
//!
//! ### Example
//!
//! ```rust,ignore
//! use tds_protocol::Collation;
//!
//! // Japanese collation (LCID 0x0411 = Japanese_CI_AS)
//! let collation = Collation { lcid: 0x0411, sort_id: 0 };
//! if let Some(encoding) = collation.encoding() {
//!     // encoding is Shift_JIS
//!     let (decoded, _, _) = encoding.decode(varchar_bytes);
//! }
//!
//! // Check if UTF-8 collation (SQL Server 2019+)
//! let utf8_collation = Collation { lcid: 0x0800_0409, sort_id: 0 };
//! assert!(utf8_collation.is_utf8()); // true, no transcoding needed
//! ```
//!
//! ## Design Philosophy
//!
//! This crate is intentionally IO-agnostic. It contains no networking logic and
//! makes no assumptions about the async runtime. Higher-level crates build upon
//! this foundation to provide async I/O capabilities.
//!
//! ## Example
//!
//! ```rust,ignore
//! use tds_protocol::{PacketHeader, PacketType, PacketStatus};
//!
//! let header = PacketHeader {
//!     packet_type: PacketType::SqlBatch,
//!     status: PacketStatus::END_OF_MESSAGE,
//!     length: 100,
//!     spid: 0,
//!     packet_id: 1,
//!     window: 0,
//! };
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![deny(unsafe_code)]

// This crate requires heap allocation (String, Vec). When std is disabled,
// the alloc feature must be enabled to provide these types.
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
compile_error!(
    "tds-protocol requires either the `std` feature (default) or the `alloc` feature. \
     Enable at least one: `--features std` or `--features alloc`"
);

#[cfg(feature = "alloc")]
extern crate alloc;

// Internal prelude for no_std compatibility - provides String, Vec, Box, etc.
mod prelude;

pub mod codec;
pub mod collation;
pub mod crypto;
pub mod error;
pub mod login7;
pub mod packet;
pub mod prelogin;
pub mod rpc;
pub mod sql_batch;
pub mod token;
pub mod tvp;
pub mod types;
pub mod version;

pub use error::ProtocolError;
pub use login7::{
    FeatureExtension, FeatureId, Login7, OptionFlags1, OptionFlags2, OptionFlags3, TypeFlags,
};
pub use packet::{
    DEFAULT_PACKET_SIZE, MAX_PACKET_SIZE, PACKET_HEADER_SIZE, PacketHeader, PacketStatus,
    PacketType,
};
pub use prelogin::{EncryptionLevel, PreLogin, PreLoginOption};
pub use rpc::{ParamFlags, ProcId, RpcOptionFlags, RpcParam, RpcRequest, TypeInfo as RpcTypeInfo};
pub use sql_batch::{SqlBatch, encode_sql_batch, encode_sql_batch_with_transaction};
pub use token::{
    ColMetaData, Collation, ColumnData, Done, DoneInProc, DoneProc, DoneStatus, EnvChange,
    EnvChangeType, EnvChangeValue, FeatureExtAck, FedAuthInfo, LoginAck, NbcRow, Order, RawRow,
    ReturnValue, ServerError, ServerInfo, SessionState, SspiToken, Token, TokenParser, TokenType,
    TypeInfo,
};
pub use tvp::{
    TVP_END_TOKEN, TVP_ROW_TOKEN, TVP_TYPE_ID, TvpColumnDef as TvpWireColumnDef, TvpColumnFlags,
    TvpEncoder, TvpWireType, encode_tvp_bit, encode_tvp_date, encode_tvp_datetime2,
    encode_tvp_datetimeoffset, encode_tvp_decimal, encode_tvp_float, encode_tvp_guid,
    encode_tvp_int, encode_tvp_null, encode_tvp_nvarchar, encode_tvp_time, encode_tvp_varbinary,
};
pub use types::{ColumnFlags, TypeId, Updateable};
pub use version::TdsVersion;

// Always Encrypted metadata types
pub use crypto::{
    ALGORITHM_AEAD_AES_256_CBC_HMAC_SHA256, COLUMN_FLAG_ENCRYPTED, CekTable, CekTableEntry,
    CekValue, ColumnCryptoInfo, CryptoMetadata, ENCRYPTION_TYPE_DETERMINISTIC,
    ENCRYPTION_TYPE_RANDOMIZED, EncryptionTypeWire, NORMALIZATION_RULE_VERSION,
    is_column_encrypted,
};
