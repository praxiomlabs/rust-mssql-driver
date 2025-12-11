//! # mssql-codec
//!
//! Async framing layer for TDS packet handling.
//!
//! This crate transforms raw byte streams into high-level TDS packets,
//! handling packet reassembly across TCP segment boundaries and packet
//! continuation for large messages.
//!
//! ## Features
//!
//! - Packet reassembly across TCP segments
//! - Message reassembly from multiple packets
//! - IO splitting for cancellation safety (ADR-005)
//! - Integration with tokio-util's codec framework
//!
//! ## Architecture
//!
//! The codec layer sits between raw TCP streams and the higher-level client:
//!
//! ```text
//! TCP Stream → TdsCodec (packet framing) → MessageAssembler → Client
//! ```
//!
//! ### Cancellation Safety
//!
//! Per ADR-005, the connection splits the TCP stream into read and write halves.
//! This allows sending Attention packets for query cancellation even while
//! blocked reading a large result set.
//!
//! ```rust,ignore
//! use mssql_codec::Connection;
//!
//! let conn = Connection::new(tcp_stream);
//! let cancel = conn.cancel_handle();
//!
//! // Cancel from another task
//! tokio::spawn(async move {
//!     cancel.cancel().await?;
//! });
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod connection;
pub mod error;
pub mod framed;
pub mod message;
pub mod packet_codec;

pub use connection::{CancelHandle, Connection};
pub use error::CodecError;
pub use framed::{PacketReader, PacketStream, PacketWriter};
pub use message::{Message, MessageAssembler};
pub use packet_codec::{Packet, TdsCodec};
