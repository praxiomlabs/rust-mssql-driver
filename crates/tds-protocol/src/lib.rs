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

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod codec;
pub mod error;
pub mod login7;
pub mod packet;
pub mod prelogin;
pub mod rpc;
pub mod sql_batch;
pub mod token;
pub mod types;
pub mod version;

pub use error::ProtocolError;
pub use packet::{PacketHeader, PacketStatus, PacketType, DEFAULT_PACKET_SIZE, MAX_PACKET_SIZE, PACKET_HEADER_SIZE};
pub use prelogin::{EncryptionLevel, PreLogin, PreLoginOption};
pub use token::{
    ColMetaData, ColumnData, Collation, Done, DoneInProc, DoneProc, DoneStatus, EnvChange,
    EnvChangeType, EnvChangeValue, FeatureExtAck, FedAuthInfo, LoginAck, NbcRow, Order, RawRow,
    ReturnValue, ServerError, ServerInfo, SessionState, SspiToken, Token, TokenParser, TokenType,
    TypeInfo,
};
pub use types::{ColumnFlags, TypeId, Updateable};
pub use version::TdsVersion;
pub use login7::{FeatureExtension, FeatureId, Login7, OptionFlags1, OptionFlags2, OptionFlags3, TypeFlags};
pub use rpc::{ParamFlags, ProcId, RpcOptionFlags, RpcParam, RpcRequest, TypeInfo as RpcTypeInfo};
pub use sql_batch::{encode_sql_batch, encode_sql_batch_with_transaction, SqlBatch};
