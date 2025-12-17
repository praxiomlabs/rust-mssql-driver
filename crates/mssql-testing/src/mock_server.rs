//! Mock TDS server for unit testing.
//!
//! This module provides a mock SQL Server implementation that can be used
//! for unit testing without requiring a real database instance.
//!
//! ## Features
//!
//! - Simulates TDS protocol handshake (prelogin, login)
//! - Configurable responses for SQL queries
//! - Support for multiple concurrent connections
//! - Recorded packet replay for regression testing
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_testing::mock_server::{MockTdsServer, MockResponse};
//!
//! #[tokio::test]
//! async fn test_query() {
//!     let server = MockTdsServer::builder()
//!         .with_response("SELECT 1", MockResponse::scalar(1i32))
//!         .build()
//!         .await
//!         .unwrap();
//!
//!     let addr = server.addr();
//!     // Connect your client to addr...
//! }
//! ```

use bytes::{BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use tds_protocol::types::TypeId;
use tds_protocol::{
    DoneStatus, EnvChangeType, PACKET_HEADER_SIZE, PacketHeader, PacketStatus, PacketType,
    TokenType,
};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, broadcast};

/// Error type for mock server operations.
#[derive(Debug, Error)]
pub enum MockServerError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Protocol error.
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Server already stopped.
    #[error("Server already stopped")]
    Stopped,
}

/// Result type for mock server operations.
pub type Result<T> = std::result::Result<T, MockServerError>;

/// Mock response configuration.
#[derive(Clone)]
pub enum MockResponse {
    /// Return a single scalar value.
    Scalar(ScalarValue),

    /// Return multiple rows with columns.
    Rows {
        /// Column definitions.
        columns: Vec<MockColumn>,
        /// Row data.
        rows: Vec<Vec<ScalarValue>>,
    },

    /// Return an error.
    Error {
        /// Error number.
        number: i32,
        /// Error message.
        message: String,
        /// Severity class.
        severity: u8,
    },

    /// Return rows affected count (for INSERT/UPDATE/DELETE).
    RowsAffected(u64),

    /// Return raw pre-encoded TDS tokens.
    Raw(Bytes),

    /// Execute a custom handler.
    Custom(Arc<dyn Fn(&str) -> MockResponse + Send + Sync>),
}

impl fmt::Debug for MockResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(v) => f.debug_tuple("Scalar").field(v).finish(),
            Self::Rows { columns, rows } => f
                .debug_struct("Rows")
                .field("columns", columns)
                .field("rows", rows)
                .finish(),
            Self::Error {
                number,
                message,
                severity,
            } => f
                .debug_struct("Error")
                .field("number", number)
                .field("message", message)
                .field("severity", severity)
                .finish(),
            Self::RowsAffected(n) => f.debug_tuple("RowsAffected").field(n).finish(),
            Self::Raw(data) => f.debug_tuple("Raw").field(&data.len()).finish(),
            Self::Custom(_) => f.debug_tuple("Custom").field(&"<fn>").finish(),
        }
    }
}

impl MockResponse {
    /// Create a scalar integer response.
    pub fn scalar_int(value: i32) -> Self {
        Self::Scalar(ScalarValue::Int(value))
    }

    /// Create a scalar string response.
    pub fn scalar_string(value: impl Into<String>) -> Self {
        Self::Scalar(ScalarValue::String(value.into()))
    }

    /// Create an empty result response.
    pub fn empty() -> Self {
        Self::RowsAffected(0)
    }

    /// Create a rows affected response.
    pub fn affected(count: u64) -> Self {
        Self::RowsAffected(count)
    }

    /// Create an error response.
    pub fn error(number: i32, message: impl Into<String>) -> Self {
        Self::Error {
            number,
            message: message.into(),
            severity: 16,
        }
    }

    /// Create a multi-row response.
    pub fn rows(columns: Vec<MockColumn>, rows: Vec<Vec<ScalarValue>>) -> Self {
        Self::Rows { columns, rows }
    }
}

/// Scalar value for mock responses.
#[derive(Debug, Clone)]
pub enum ScalarValue {
    /// NULL value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// 32-bit integer.
    Int(i32),
    /// 64-bit integer.
    BigInt(i64),
    /// 32-bit float.
    Float(f32),
    /// 64-bit float.
    Double(f64),
    /// String value.
    String(String),
    /// Binary data.
    Binary(Vec<u8>),
}

impl ScalarValue {
    /// Get the TDS type ID for this value.
    fn type_id(&self) -> TypeId {
        match self {
            Self::Null => TypeId::Null,
            Self::Bool(_) => TypeId::BitN,
            Self::Int(_) => TypeId::IntN,
            Self::BigInt(_) => TypeId::IntN,
            Self::Float(_) => TypeId::FloatN,
            Self::Double(_) => TypeId::FloatN,
            Self::String(_) => TypeId::NVarChar,
            Self::Binary(_) => TypeId::BigVarBinary,
        }
    }

    /// Encode this value to TDS format.
    fn encode(&self, dst: &mut BytesMut) {
        match self {
            Self::Null => {
                dst.put_u8(0); // NULL length
            }
            Self::Bool(v) => {
                dst.put_u8(1); // length
                dst.put_u8(if *v { 1 } else { 0 });
            }
            Self::Int(v) => {
                dst.put_u8(4); // length
                dst.put_i32_le(*v);
            }
            Self::BigInt(v) => {
                dst.put_u8(8); // length
                dst.put_i64_le(*v);
            }
            Self::Float(v) => {
                dst.put_u8(4); // length
                dst.put_f32_le(*v);
            }
            Self::Double(v) => {
                dst.put_u8(8); // length
                dst.put_f64_le(*v);
            }
            Self::String(s) => {
                let utf16: Vec<u16> = s.encode_utf16().collect();
                let byte_len = utf16.len() * 2;
                if byte_len > 0xFFFF {
                    // PLP format for large strings
                    dst.put_u64_le(byte_len as u64);
                    dst.put_u32_le(byte_len as u32);
                    for c in utf16 {
                        dst.put_u16_le(c);
                    }
                    dst.put_u32_le(0); // terminator
                } else {
                    dst.put_u16_le(byte_len as u16);
                    for c in utf16 {
                        dst.put_u16_le(c);
                    }
                }
            }
            Self::Binary(data) => {
                if data.len() > 0xFFFF {
                    // PLP format
                    dst.put_u64_le(data.len() as u64);
                    dst.put_u32_le(data.len() as u32);
                    dst.extend_from_slice(data);
                    dst.put_u32_le(0); // terminator
                } else {
                    dst.put_u16_le(data.len() as u16);
                    dst.extend_from_slice(data);
                }
            }
        }
    }
}

/// Mock column definition.
#[derive(Debug, Clone)]
pub struct MockColumn {
    /// Column name.
    pub name: String,
    /// Column type.
    pub type_id: TypeId,
    /// Maximum length (for variable-length types).
    pub max_length: Option<u32>,
    /// Whether the column is nullable.
    pub nullable: bool,
}

impl MockColumn {
    /// Create a new column definition.
    pub fn new(name: impl Into<String>, type_id: TypeId) -> Self {
        Self {
            name: name.into(),
            type_id,
            max_length: None,
            nullable: true,
        }
    }

    /// Create an INT column.
    pub fn int(name: impl Into<String>) -> Self {
        Self::new(name, TypeId::IntN).with_max_length(4)
    }

    /// Create a BIGINT column.
    pub fn bigint(name: impl Into<String>) -> Self {
        Self::new(name, TypeId::IntN).with_max_length(8)
    }

    /// Create an NVARCHAR column.
    pub fn nvarchar(name: impl Into<String>, max_len: u32) -> Self {
        Self::new(name, TypeId::NVarChar).with_max_length(max_len * 2)
    }

    /// Set the maximum length.
    pub fn with_max_length(mut self, len: u32) -> Self {
        self.max_length = Some(len);
        self
    }

    /// Set nullable flag.
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }
}

/// Configuration for the mock TDS server.
#[derive(Default)]
pub struct MockServerConfig {
    /// Pre-configured responses for specific SQL queries.
    responses: HashMap<String, MockResponse>,
    /// Default response for unmatched queries.
    default_response: Option<MockResponse>,
    /// Server name to report in LoginAck.
    server_name: String,
    /// TDS version to report.
    tds_version: u32,
    /// Default database name.
    database: String,
}

/// Builder for `MockTdsServer`.
pub struct MockServerBuilder {
    config: MockServerConfig,
}

impl MockServerBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            config: MockServerConfig {
                responses: HashMap::new(),
                default_response: Some(MockResponse::empty()),
                server_name: "MockSQLServer".to_string(),
                tds_version: 0x74000004, // TDS 7.4
                database: "master".to_string(),
            },
        }
    }

    /// Add a response for a specific SQL query.
    pub fn with_response(mut self, sql: impl Into<String>, response: MockResponse) -> Self {
        self.config.responses.insert(sql.into(), response);
        self
    }

    /// Set the default response for unmatched queries.
    pub fn with_default_response(mut self, response: MockResponse) -> Self {
        self.config.default_response = Some(response);
        self
    }

    /// Set the server name reported in LoginAck.
    pub fn with_server_name(mut self, name: impl Into<String>) -> Self {
        self.config.server_name = name.into();
        self
    }

    /// Set the default database.
    pub fn with_database(mut self, db: impl Into<String>) -> Self {
        self.config.database = db.into();
        self
    }

    /// Build and start the mock server.
    pub async fn build(self) -> Result<MockTdsServer> {
        MockTdsServer::start(self.config).await
    }
}

impl Default for MockServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A mock TDS server for testing.
///
/// This server simulates a SQL Server instance for unit testing purposes.
/// It handles the TDS protocol handshake and responds to queries based on
/// pre-configured responses.
pub struct MockTdsServer {
    /// Server address.
    addr: SocketAddr,
    /// Shutdown signal sender.
    shutdown_tx: broadcast::Sender<()>,
    /// Server configuration (stored for potential introspection).
    #[allow(dead_code)]
    config: Arc<MockServerConfig>,
    /// Connection count.
    connection_count: Arc<Mutex<usize>>,
}

impl MockTdsServer {
    /// Create a new builder for the mock server.
    pub fn builder() -> MockServerBuilder {
        MockServerBuilder::new()
    }

    /// Start the mock server on an available port.
    pub async fn start(config: MockServerConfig) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, _) = broadcast::channel(1);
        let config = Arc::new(config);
        let connection_count = Arc::new(Mutex::new(0usize));

        let server = Self {
            addr,
            shutdown_tx: shutdown_tx.clone(),
            config: config.clone(),
            connection_count: connection_count.clone(),
        };

        // Spawn the accept loop
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _peer_addr)) => {
                                let config = config.clone();
                                let count = connection_count.clone();
                                tokio::spawn(async move {
                                    {
                                        let mut c = count.lock().await;
                                        *c += 1;
                                    }
                                    if let Err(e) = handle_connection(stream, config).await {
                                        tracing::debug!("Connection error: {}", e);
                                    }
                                    {
                                        let mut c = count.lock().await;
                                        *c = c.saturating_sub(1);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!("Accept error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(server)
    }

    /// Get the server's listening address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the host string for connection configuration.
    pub fn host(&self) -> String {
        self.addr.ip().to_string()
    }

    /// Get the port number.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Get the current connection count.
    pub async fn connection_count(&self) -> usize {
        *self.connection_count.lock().await
    }

    /// Stop the server.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

impl Drop for MockTdsServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Handle a single client connection.
async fn handle_connection(mut stream: TcpStream, config: Arc<MockServerConfig>) -> Result<()> {
    // Step 1: Handle PRELOGIN
    let prelogin_request = read_packet(&mut stream).await?;
    if prelogin_request.packet_type != PacketType::PreLogin {
        return Err(MockServerError::Protocol(format!(
            "Expected PreLogin, got {:?}",
            prelogin_request.packet_type
        )));
    }
    send_prelogin_response(&mut stream).await?;

    // Step 2: Handle LOGIN7
    let login_request = read_packet(&mut stream).await?;
    if login_request.packet_type != PacketType::Tds7Login {
        return Err(MockServerError::Protocol(format!(
            "Expected Tds7Login, got {:?}",
            login_request.packet_type
        )));
    }
    send_login_response(&mut stream, &config).await?;

    // Step 3: Handle SQL batches and RPC requests
    loop {
        let packet = match read_packet(&mut stream).await {
            Ok(p) => p,
            Err(MockServerError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Client disconnected
                break;
            }
            Err(e) => return Err(e),
        };

        match packet.packet_type {
            PacketType::SqlBatch => {
                let sql = decode_sql_batch(&packet.payload)?;
                let response = find_response(&sql, &config);
                send_query_response(&mut stream, response).await?;
            }
            PacketType::Rpc => {
                // For RPC requests (sp_executesql, sp_prepare, etc.)
                // Extract the SQL from the RPC payload and handle similarly
                let response = config
                    .default_response
                    .clone()
                    .unwrap_or(MockResponse::empty());
                send_query_response(&mut stream, response).await?;
            }
            PacketType::Attention => {
                // Client sent attention/cancel signal
                send_attention_ack(&mut stream).await?;
            }
            _ => {
                tracing::debug!("Unexpected packet type: {:?}", packet.packet_type);
            }
        }
    }

    Ok(())
}

/// Parsed TDS packet.
struct Packet {
    packet_type: PacketType,
    payload: Bytes,
}

/// Read a complete TDS packet from the stream.
async fn read_packet(stream: &mut TcpStream) -> Result<Packet> {
    let mut header_buf = [0u8; PACKET_HEADER_SIZE];
    stream.read_exact(&mut header_buf).await?;

    let mut cursor = &header_buf[..];
    let header =
        PacketHeader::decode(&mut cursor).map_err(|e| MockServerError::Protocol(e.to_string()))?;

    let payload_len = header.payload_length();
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream.read_exact(&mut payload).await?;
    }

    // Handle multi-packet messages
    let mut full_payload = BytesMut::from(&payload[..]);

    if !header.is_end_of_message() {
        loop {
            let mut next_header_buf = [0u8; PACKET_HEADER_SIZE];
            stream.read_exact(&mut next_header_buf).await?;

            let mut cursor = &next_header_buf[..];
            let next_header = PacketHeader::decode(&mut cursor)
                .map_err(|e| MockServerError::Protocol(e.to_string()))?;

            let next_payload_len = next_header.payload_length();
            let mut next_payload = vec![0u8; next_payload_len];
            if next_payload_len > 0 {
                stream.read_exact(&mut next_payload).await?;
            }

            full_payload.extend_from_slice(&next_payload);

            if next_header.is_end_of_message() {
                break;
            }
        }
    }

    Ok(Packet {
        packet_type: header.packet_type,
        payload: full_payload.freeze(),
    })
}

/// Write a TDS packet to the stream.
async fn write_packet(
    stream: &mut TcpStream,
    packet_type: PacketType,
    payload: &[u8],
) -> Result<()> {
    let total_len = PACKET_HEADER_SIZE + payload.len();
    let header = PacketHeader {
        packet_type,
        status: PacketStatus::END_OF_MESSAGE,
        length: total_len as u16,
        spid: 0,
        packet_id: 1,
        window: 0,
    };

    let mut buf = BytesMut::with_capacity(total_len);
    header.encode(&mut buf);
    buf.extend_from_slice(payload);

    stream.write_all(&buf).await?;
    stream.flush().await?;
    Ok(())
}

/// Send PRELOGIN response.
async fn send_prelogin_response(stream: &mut TcpStream) -> Result<()> {
    // PRELOGIN response format:
    // Option tokens (5 bytes each: type + offset + length) followed by data
    // VERSION (0x00), ENCRYPTION (0x01)
    //
    // Options section layout:
    //   VERSION token:    1 + 2 + 2 = 5 bytes
    //   ENCRYPTION token: 1 + 2 + 2 = 5 bytes
    //   Terminator:       1 byte
    //   Total header:     11 bytes
    //
    // Data section layout:
    //   VERSION data at offset 11: 6 bytes
    //   ENCRYPTION data at offset 17: 1 byte

    let mut response = BytesMut::new();

    // Option header area (offsets are big-endian per TDS spec)
    // VERSION token
    response.put_u8(0x00); // VERSION
    response.put_u16(11); // offset (header size)
    response.put_u16(6); // length

    // ENCRYPTION token
    response.put_u8(0x01); // ENCRYPTION
    response.put_u16(17); // offset (11 + 6)
    response.put_u16(1); // length

    // Terminator
    response.put_u8(0xFF);

    // VERSION data (at offset 11)
    response.put_u8(16); // major version
    response.put_u8(0); // minor version
    response.put_u16_le(0); // build number
    response.put_u16_le(0); // sub-build number

    // ENCRYPTION data (at offset 17)
    response.put_u8(0x00); // ENCRYPT_OFF (no encryption)

    write_packet(stream, PacketType::PreLogin, &response).await
}

/// Send LOGIN7 response (LoginAck + EnvChange + Done).
async fn send_login_response(stream: &mut TcpStream, config: &MockServerConfig) -> Result<()> {
    let mut response = BytesMut::new();

    // EnvChange: Database
    encode_env_change(&mut response, EnvChangeType::Database, &config.database, "");

    // EnvChange: PacketSize
    encode_env_change(&mut response, EnvChangeType::PacketSize, "4096", "4096");

    // LoginAck
    encode_login_ack(&mut response, &config.server_name, config.tds_version);

    // Done
    encode_done(&mut response, 0, false);

    write_packet(stream, PacketType::TabularResult, &response).await
}

/// Encode an EnvChange token.
fn encode_env_change(dst: &mut BytesMut, env_type: EnvChangeType, new_val: &str, old_val: &str) {
    let new_utf16: Vec<u16> = new_val.encode_utf16().collect();
    let old_utf16: Vec<u16> = old_val.encode_utf16().collect();

    let data_len = 1 + 1 + new_utf16.len() * 2 + 1 + old_utf16.len() * 2;

    dst.put_u8(TokenType::EnvChange as u8);
    dst.put_u16_le(data_len as u16);
    dst.put_u8(env_type as u8);

    // New value (B_VARCHAR format)
    dst.put_u8(new_utf16.len() as u8);
    for c in &new_utf16 {
        dst.put_u16_le(*c);
    }

    // Old value (B_VARCHAR format)
    dst.put_u8(old_utf16.len() as u8);
    for c in &old_utf16 {
        dst.put_u16_le(*c);
    }
}

/// Encode a LoginAck token.
fn encode_login_ack(dst: &mut BytesMut, server_name: &str, tds_version: u32) {
    let name_utf16: Vec<u16> = server_name.encode_utf16().collect();

    // LoginAck: interface (1) + tds_version (4) + prog_name (b_varchar) + prog_version (4)
    let data_len = 1 + 4 + 1 + name_utf16.len() * 2 + 4;

    dst.put_u8(TokenType::LoginAck as u8);
    dst.put_u16_le(data_len as u16);
    dst.put_u8(1); // interface: SQL
    dst.put_u32_le(tds_version);

    // Program name (B_VARCHAR)
    dst.put_u8(name_utf16.len() as u8);
    for c in &name_utf16 {
        dst.put_u16_le(*c);
    }

    // Program version
    dst.put_u32_le(0x10000000); // 16.0.0.0
}

/// Encode a Done token.
fn encode_done(dst: &mut BytesMut, row_count: u64, more: bool) {
    dst.put_u8(TokenType::Done as u8);

    let status = DoneStatus {
        count: row_count > 0,
        more,
        ..Default::default()
    };

    dst.put_u16_le(status.to_bits());
    dst.put_u16_le(0xC1); // cur_cmd: SELECT
    dst.put_u64_le(row_count);
}

/// Decode SQL from a SQL_BATCH packet payload.
fn decode_sql_batch(payload: &Bytes) -> Result<String> {
    // SQL Batch format: ALL_HEADERS (optional) + SQL text in UTF-16LE
    // For simplicity, assume no ALL_HEADERS (check first 4 bytes)

    let mut cursor = payload.as_ref();

    // Check if ALL_HEADERS is present
    if cursor.len() >= 4 {
        let total_len = u32::from_le_bytes([cursor[0], cursor[1], cursor[2], cursor[3]]) as usize;

        // If total_len looks like a header length (reasonable size), skip headers
        if total_len >= 4 && total_len < cursor.len() && total_len < 1000 {
            cursor = &cursor[total_len..];
        }
    }

    // Read UTF-16LE SQL text
    if cursor.len() % 2 != 0 {
        return Err(MockServerError::Protocol(
            "Invalid UTF-16 SQL text length".to_string(),
        ));
    }

    let char_count = cursor.len() / 2;
    let mut chars = Vec::with_capacity(char_count);
    for i in 0..char_count {
        let c = u16::from_le_bytes([cursor[i * 2], cursor[i * 2 + 1]]);
        chars.push(c);
    }

    String::from_utf16(&chars)
        .map_err(|_| MockServerError::Protocol("Invalid UTF-16 SQL text".to_string()))
}

/// Find the response for a SQL query.
fn find_response(sql: &str, config: &MockServerConfig) -> MockResponse {
    // Normalize SQL for matching
    let normalized = sql.trim().to_uppercase();

    // Check exact match first
    if let Some(response) = config.responses.get(&normalized) {
        return response.clone();
    }

    // Check case-insensitive match
    for (key, response) in &config.responses {
        if key.trim().to_uppercase() == normalized {
            return response.clone();
        }
    }

    // Use default response
    config
        .default_response
        .clone()
        .unwrap_or(MockResponse::empty())
}

/// Send a query response based on the MockResponse.
async fn send_query_response(stream: &mut TcpStream, response: MockResponse) -> Result<()> {
    let mut buf = BytesMut::new();

    match response {
        MockResponse::Scalar(value) => {
            // Single column, single row result
            encode_colmetadata(&mut buf, &[MockColumn::new("", value.type_id())]);
            encode_row(&mut buf, &[value.clone()]);
            encode_done(&mut buf, 1, false);
        }
        MockResponse::Rows { columns, rows } => {
            encode_colmetadata(&mut buf, &columns);
            for row in &rows {
                encode_row(&mut buf, row);
            }
            encode_done(&mut buf, rows.len() as u64, false);
        }
        MockResponse::Error {
            number,
            message,
            severity,
        } => {
            encode_error(&mut buf, number, &message, severity);
            encode_done(&mut buf, 0, false);
        }
        MockResponse::RowsAffected(count) => {
            encode_done(&mut buf, count, false);
        }
        MockResponse::Raw(data) => {
            buf.extend_from_slice(&data);
        }
        MockResponse::Custom(_handler) => {
            // For custom handlers, we'd need the SQL here
            // For now, just send empty result
            encode_done(&mut buf, 0, false);
        }
    }

    write_packet(stream, PacketType::TabularResult, &buf).await
}

/// Encode COLMETADATA token.
fn encode_colmetadata(dst: &mut BytesMut, columns: &[MockColumn]) {
    dst.put_u8(TokenType::ColMetaData as u8);
    dst.put_u16_le(columns.len() as u16);

    for col in columns {
        // UserType (4 bytes)
        dst.put_u32_le(0);

        // Flags (2 bytes) - nullable = 0x01
        dst.put_u16_le(if col.nullable { 0x01 } else { 0x00 });

        // Type ID (1 byte)
        dst.put_u8(col.type_id as u8);

        // Type-specific metadata
        match col.type_id {
            TypeId::IntN | TypeId::BitN | TypeId::FloatN | TypeId::MoneyN | TypeId::DateTimeN => {
                dst.put_u8(col.max_length.unwrap_or(4) as u8);
            }
            TypeId::NVarChar | TypeId::NChar => {
                dst.put_u16_le(col.max_length.unwrap_or(8000) as u16);
                // Collation (5 bytes)
                dst.put_u32_le(0x0904D000); // LCID
                dst.put_u8(0x34); // Sort ID
            }
            TypeId::BigVarBinary | TypeId::BigBinary => {
                dst.put_u16_le(col.max_length.unwrap_or(8000) as u16);
            }
            _ => {
                // Fixed-length types have no additional metadata
            }
        }

        // Column name (B_VARCHAR)
        let name_utf16: Vec<u16> = col.name.encode_utf16().collect();
        dst.put_u8(name_utf16.len() as u8);
        for c in &name_utf16 {
            dst.put_u16_le(*c);
        }
    }
}

/// Encode ROW token.
fn encode_row(dst: &mut BytesMut, values: &[ScalarValue]) {
    dst.put_u8(TokenType::Row as u8);
    for value in values {
        value.encode(dst);
    }
}

/// Encode ERROR token.
fn encode_error(dst: &mut BytesMut, number: i32, message: &str, severity: u8) {
    let msg_utf16: Vec<u16> = message.encode_utf16().collect();
    let server_utf16: Vec<u16> = "MockServer".encode_utf16().collect();

    // ERROR: number (4) + state (1) + class (1) + message (us_varchar) +
    //        server (b_varchar) + procedure (b_varchar) + line (4)
    let data_len = (4 + 1 + 1 + 2 + msg_utf16.len() * 2 + 1 + server_utf16.len() * 2 + 1) + 4;

    dst.put_u8(TokenType::Error as u8);
    dst.put_u16_le(data_len as u16);
    dst.put_i32_le(number);
    dst.put_u8(1); // state
    dst.put_u8(severity); // class

    // Message (US_VARCHAR)
    dst.put_u16_le(msg_utf16.len() as u16);
    for c in &msg_utf16 {
        dst.put_u16_le(*c);
    }

    // Server name (B_VARCHAR)
    dst.put_u8(server_utf16.len() as u8);
    for c in &server_utf16 {
        dst.put_u16_le(*c);
    }

    // Procedure name (B_VARCHAR) - empty
    dst.put_u8(0);

    // Line number
    dst.put_i32_le(1);
}

/// Send attention acknowledgment.
async fn send_attention_ack(stream: &mut TcpStream) -> Result<()> {
    let mut buf = BytesMut::new();

    // DONE with ATTN flag
    buf.put_u8(TokenType::Done as u8);
    let status = DoneStatus {
        attn: true,
        ..Default::default()
    };
    buf.put_u16_le(status.to_bits());
    buf.put_u16_le(0);
    buf.put_u64_le(0);

    write_packet(stream, PacketType::TabularResult, &buf).await
}

/// Recorded packet for replay testing.
#[derive(Debug, Clone)]
pub struct RecordedPacket {
    /// Packet direction (true = server to client).
    pub from_server: bool,
    /// Raw packet data including header.
    pub data: Bytes,
}

/// Packet recorder for capturing and replaying TDS sessions.
#[derive(Debug, Default)]
pub struct PacketRecorder {
    packets: Vec<RecordedPacket>,
}

impl PacketRecorder {
    /// Create a new packet recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a packet.
    pub fn record(&mut self, from_server: bool, data: Bytes) {
        self.packets.push(RecordedPacket { from_server, data });
    }

    /// Get all recorded packets.
    pub fn packets(&self) -> &[RecordedPacket] {
        &self.packets
    }

    /// Save recorded packets to a file.
    pub async fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let mut file = File::create(path).await?;

        for packet in &self.packets {
            // Direction (1 byte) + length (4 bytes) + data
            file.write_u8(if packet.from_server { 1 } else { 0 })
                .await?;
            file.write_u32_le(packet.data.len() as u32).await?;
            file.write_all(&packet.data).await?;
        }

        Ok(())
    }

    /// Load recorded packets from a file.
    pub async fn load(path: &std::path::Path) -> std::io::Result<Self> {
        use tokio::fs::File;
        use tokio::io::AsyncReadExt;

        let mut file = File::open(path).await?;
        let mut recorder = Self::new();

        loop {
            let from_server = match file.read_u8().await {
                Ok(b) => b != 0,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            let len = file.read_u32_le().await? as usize;
            let mut data = vec![0u8; len];
            file.read_exact(&mut data).await?;

            recorder.record(from_server, Bytes::from(data));
        }

        Ok(recorder)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_starts() {
        let server = MockTdsServer::builder()
            .with_server_name("TestServer")
            .build()
            .await
            .unwrap();

        assert!(server.port() > 0);
        assert_eq!(server.host(), "127.0.0.1");
    }

    #[tokio::test]
    async fn test_mock_response_scalar() {
        let response = MockResponse::scalar_int(42);
        match response {
            MockResponse::Scalar(ScalarValue::Int(v)) => assert_eq!(v, 42),
            _ => panic!("Expected scalar int"),
        }
    }

    #[tokio::test]
    async fn test_mock_response_error() {
        let response = MockResponse::error(50000, "Test error");
        match response {
            MockResponse::Error {
                number,
                message,
                severity,
            } => {
                assert_eq!(number, 50000);
                assert_eq!(message, "Test error");
                assert_eq!(severity, 16);
            }
            _ => panic!("Expected error response"),
        }
    }

    #[test]
    fn test_scalar_value_encode_int() {
        let value = ScalarValue::Int(42);
        let mut buf = BytesMut::new();
        value.encode(&mut buf);

        assert_eq!(buf.len(), 5); // 1 byte length + 4 bytes value
        assert_eq!(buf[0], 4); // length
        assert_eq!(i32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]), 42);
    }

    #[test]
    fn test_scalar_value_encode_string() {
        let value = ScalarValue::String("test".to_string());
        let mut buf = BytesMut::new();
        value.encode(&mut buf);

        // 2 bytes length + 8 bytes UTF-16
        assert_eq!(buf.len(), 10);
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 8);
    }

    #[test]
    fn test_mock_column_int() {
        let col = MockColumn::int("id");
        assert_eq!(col.name, "id");
        assert_eq!(col.type_id, TypeId::IntN);
        assert_eq!(col.max_length, Some(4));
    }

    #[test]
    fn test_mock_column_nvarchar() {
        let col = MockColumn::nvarchar("name", 50);
        assert_eq!(col.name, "name");
        assert_eq!(col.type_id, TypeId::NVarChar);
        assert_eq!(col.max_length, Some(100)); // 50 chars * 2 bytes
    }

    #[test]
    fn test_done_status_encoding() {
        let mut buf = BytesMut::new();
        encode_done(&mut buf, 5, false);

        assert_eq!(buf[0], TokenType::Done as u8);
        // Status should have COUNT flag set
        let status = u16::from_le_bytes([buf[1], buf[2]]);
        assert_eq!(status & 0x0010, 0x0010); // DONE_COUNT
    }
}
