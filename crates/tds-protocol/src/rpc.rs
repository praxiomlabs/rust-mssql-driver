//! RPC (Remote Procedure Call) request encoding.
//!
//! This module provides encoding for RPC requests (packet type 0x03).
//! RPC is used for calling stored procedures and sp_executesql for parameterized queries.
//!
//! ## sp_executesql
//!
//! The primary use case is `sp_executesql` for parameterized queries, which prevents
//! SQL injection and enables query plan caching.
//!
//! ## Wire Format
//!
//! ```text
//! RPC Request:
//! +-------------------+
//! | ALL_HEADERS       | (TDS 7.2+, optional)
//! +-------------------+
//! | ProcName/ProcID   | (procedure identifier)
//! +-------------------+
//! | Option Flags      | (2 bytes)
//! +-------------------+
//! | Parameters        | (repeated)
//! +-------------------+
//! ```

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::write_utf16_string;
use crate::prelude::*;

/// Well-known stored procedure IDs.
///
/// These are special procedure IDs that SQL Server recognizes
/// without requiring the procedure name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProcId {
    /// sp_cursor (0x0001)
    Cursor = 0x0001,
    /// sp_cursoropen (0x0002)
    CursorOpen = 0x0002,
    /// sp_cursorprepare (0x0003)
    CursorPrepare = 0x0003,
    /// sp_cursorexecute (0x0004)
    CursorExecute = 0x0004,
    /// sp_cursorprepexec (0x0005)
    CursorPrepExec = 0x0005,
    /// sp_cursorunprepare (0x0006)
    CursorUnprepare = 0x0006,
    /// sp_cursorfetch (0x0007)
    CursorFetch = 0x0007,
    /// sp_cursoroption (0x0008)
    CursorOption = 0x0008,
    /// sp_cursorclose (0x0009)
    CursorClose = 0x0009,
    /// sp_executesql (0x000A) - Primary method for parameterized queries
    ExecuteSql = 0x000A,
    /// sp_prepare (0x000B)
    Prepare = 0x000B,
    /// sp_execute (0x000C)
    Execute = 0x000C,
    /// sp_prepexec (0x000D) - Prepare and execute in one call
    PrepExec = 0x000D,
    /// sp_prepexecrpc (0x000E)
    PrepExecRpc = 0x000E,
    /// sp_unprepare (0x000F)
    Unprepare = 0x000F,
}

/// RPC option flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct RpcOptionFlags {
    /// Recompile the procedure.
    pub with_recompile: bool,
    /// No metadata in response.
    pub no_metadata: bool,
    /// Reuse metadata from previous call.
    pub reuse_metadata: bool,
}

impl RpcOptionFlags {
    /// Create new empty flags.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set with recompile flag.
    #[must_use]
    pub fn with_recompile(mut self, value: bool) -> Self {
        self.with_recompile = value;
        self
    }

    /// Encode to wire format (2 bytes).
    pub fn encode(&self) -> u16 {
        let mut flags = 0u16;
        if self.with_recompile {
            flags |= 0x0001;
        }
        if self.no_metadata {
            flags |= 0x0002;
        }
        if self.reuse_metadata {
            flags |= 0x0004;
        }
        flags
    }
}

/// RPC parameter status flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParamFlags {
    /// Parameter is passed by reference (OUTPUT parameter).
    pub by_ref: bool,
    /// Parameter has a default value.
    pub default: bool,
    /// Parameter is encrypted (Always Encrypted).
    pub encrypted: bool,
}

impl ParamFlags {
    /// Create new empty flags.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set as output parameter.
    #[must_use]
    pub fn output(mut self) -> Self {
        self.by_ref = true;
        self
    }

    /// Encode to wire format (1 byte).
    pub fn encode(&self) -> u8 {
        let mut flags = 0u8;
        if self.by_ref {
            flags |= 0x01;
        }
        if self.default {
            flags |= 0x02;
        }
        if self.encrypted {
            flags |= 0x08;
        }
        flags
    }
}

/// TDS type information for RPC parameters.
#[derive(Debug, Clone)]
pub struct TypeInfo {
    /// Type ID.
    pub type_id: u8,
    /// Maximum length for variable-length types.
    pub max_length: Option<u16>,
    /// Precision for numeric types.
    pub precision: Option<u8>,
    /// Scale for numeric types.
    pub scale: Option<u8>,
    /// Collation for string types.
    pub collation: Option<[u8; 5]>,
    /// TVP type name (e.g., "dbo.IntIdList") for Table-Valued Parameters.
    pub tvp_type_name: Option<String>,
}

impl TypeInfo {
    /// Create type info for INT.
    pub fn int() -> Self {
        Self {
            type_id: 0x26, // INTNTYPE (variable-length int)
            max_length: Some(4),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for BIGINT.
    pub fn bigint() -> Self {
        Self {
            type_id: 0x26, // INTNTYPE
            max_length: Some(8),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for SMALLINT.
    pub fn smallint() -> Self {
        Self {
            type_id: 0x26, // INTNTYPE
            max_length: Some(2),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for TINYINT.
    pub fn tinyint() -> Self {
        Self {
            type_id: 0x26, // INTNTYPE
            max_length: Some(1),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for BIT.
    pub fn bit() -> Self {
        Self {
            type_id: 0x68, // BITNTYPE
            max_length: Some(1),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for FLOAT.
    pub fn float() -> Self {
        Self {
            type_id: 0x6D, // FLTNTYPE
            max_length: Some(8),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for REAL.
    pub fn real() -> Self {
        Self {
            type_id: 0x6D, // FLTNTYPE
            max_length: Some(4),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for NVARCHAR with max length.
    pub fn nvarchar(max_len: u16) -> Self {
        Self {
            type_id: 0xE7,                 // NVARCHARTYPE
            max_length: Some(max_len * 2), // UTF-16, so double the char count
            precision: None,
            scale: None,
            // Default collation (Latin1_General_CI_AS equivalent)
            collation: Some([0x09, 0x04, 0xD0, 0x00, 0x34]),
            tvp_type_name: None,
        }
    }

    /// Create type info for NVARCHAR(MAX).
    pub fn nvarchar_max() -> Self {
        Self {
            type_id: 0xE7,            // NVARCHARTYPE
            max_length: Some(0xFFFF), // MAX indicator
            precision: None,
            scale: None,
            collation: Some([0x09, 0x04, 0xD0, 0x00, 0x34]),
            tvp_type_name: None,
        }
    }

    /// Create type info for VARBINARY with max length.
    pub fn varbinary(max_len: u16) -> Self {
        Self {
            type_id: 0xA5, // BIGVARBINTYPE
            max_length: Some(max_len),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for UNIQUEIDENTIFIER.
    pub fn uniqueidentifier() -> Self {
        Self {
            type_id: 0x24, // GUIDTYPE
            max_length: Some(16),
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for DATE.
    pub fn date() -> Self {
        Self {
            type_id: 0x28, // DATETYPE
            max_length: None,
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for DATETIME2.
    pub fn datetime2(scale: u8) -> Self {
        Self {
            type_id: 0x2A, // DATETIME2TYPE
            max_length: None,
            precision: None,
            scale: Some(scale),
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for DECIMAL.
    pub fn decimal(precision: u8, scale: u8) -> Self {
        Self {
            type_id: 0x6C,        // DECIMALNTYPE
            max_length: Some(17), // Max decimal size
            precision: Some(precision),
            scale: Some(scale),
            collation: None,
            tvp_type_name: None,
        }
    }

    /// Create type info for a Table-Valued Parameter.
    ///
    /// # Arguments
    /// * `type_name` - The fully qualified table type name (e.g., "dbo.IntIdList")
    pub fn tvp(type_name: impl Into<String>) -> Self {
        Self {
            type_id: 0xF3, // TVP type
            max_length: None,
            precision: None,
            scale: None,
            collation: None,
            tvp_type_name: Some(type_name.into()),
        }
    }

    /// Encode type info to buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        // TVP (0xF3) has type_id embedded in the value data itself
        // (written by TvpEncoder::encode_metadata), so don't write it here
        if self.type_id != 0xF3 {
            buf.put_u8(self.type_id);
        }

        // Variable-length types need max length
        match self.type_id {
            0x26 | 0x68 | 0x6D => {
                // INTNTYPE, BITNTYPE, FLTNTYPE
                if let Some(len) = self.max_length {
                    buf.put_u8(len as u8);
                }
            }
            0xE7 | 0xA5 | 0xEF => {
                // NVARCHARTYPE, BIGVARBINTYPE, NCHARTYPE
                if let Some(len) = self.max_length {
                    buf.put_u16_le(len);
                }
                // Collation for string types
                if let Some(collation) = self.collation {
                    buf.put_slice(&collation);
                }
            }
            0x24 => {
                // GUIDTYPE
                if let Some(len) = self.max_length {
                    buf.put_u8(len as u8);
                }
            }
            0x29..=0x2B => {
                // DATETIME2TYPE, TIMETYPE, DATETIMEOFFSETTYPE
                if let Some(scale) = self.scale {
                    buf.put_u8(scale);
                }
            }
            0x6C | 0x6A => {
                // DECIMALNTYPE, NUMERICNTYPE
                if let Some(len) = self.max_length {
                    buf.put_u8(len as u8);
                }
                if let Some(precision) = self.precision {
                    buf.put_u8(precision);
                }
                if let Some(scale) = self.scale {
                    buf.put_u8(scale);
                }
            }
            _ => {}
        }
    }
}

/// An RPC parameter.
#[derive(Debug, Clone)]
pub struct RpcParam {
    /// Parameter name (can be empty for positional params).
    pub name: String,
    /// Status flags.
    pub flags: ParamFlags,
    /// Type information.
    pub type_info: TypeInfo,
    /// Parameter value (raw bytes).
    pub value: Option<Bytes>,
}

impl RpcParam {
    /// Create a new parameter with a value.
    pub fn new(name: impl Into<String>, type_info: TypeInfo, value: Bytes) -> Self {
        Self {
            name: name.into(),
            flags: ParamFlags::default(),
            type_info,
            value: Some(value),
        }
    }

    /// Create a NULL parameter.
    pub fn null(name: impl Into<String>, type_info: TypeInfo) -> Self {
        Self {
            name: name.into(),
            flags: ParamFlags::default(),
            type_info,
            value: None,
        }
    }

    /// Create an INT parameter.
    pub fn int(name: impl Into<String>, value: i32) -> Self {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_i32_le(value);
        Self::new(name, TypeInfo::int(), buf.freeze())
    }

    /// Create a BIGINT parameter.
    pub fn bigint(name: impl Into<String>, value: i64) -> Self {
        let mut buf = BytesMut::with_capacity(8);
        buf.put_i64_le(value);
        Self::new(name, TypeInfo::bigint(), buf.freeze())
    }

    /// Create an NVARCHAR parameter.
    pub fn nvarchar(name: impl Into<String>, value: &str) -> Self {
        let mut buf = BytesMut::new();
        // Encode as UTF-16LE
        for code_unit in value.encode_utf16() {
            buf.put_u16_le(code_unit);
        }
        let char_len = value.chars().count();
        let type_info = if char_len > 4000 {
            TypeInfo::nvarchar_max()
        } else {
            TypeInfo::nvarchar(char_len.max(1) as u16)
        };
        Self::new(name, type_info, buf.freeze())
    }

    /// Mark as output parameter.
    #[must_use]
    pub fn as_output(mut self) -> Self {
        self.flags = self.flags.output();
        self
    }

    /// Encode the parameter to buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        // Parameter name (B_VARCHAR - length-prefixed)
        let name_len = self.name.encode_utf16().count() as u8;
        buf.put_u8(name_len);
        if name_len > 0 {
            for code_unit in self.name.encode_utf16() {
                buf.put_u16_le(code_unit);
            }
        }

        // Status flags
        buf.put_u8(self.flags.encode());

        // Type info
        self.type_info.encode(buf);

        // Value
        if let Some(ref value) = self.value {
            // Length prefix based on type
            match self.type_info.type_id {
                0x26 => {
                    // INTNTYPE
                    buf.put_u8(value.len() as u8);
                    buf.put_slice(value);
                }
                0x68 | 0x6D => {
                    // BITNTYPE, FLTNTYPE
                    buf.put_u8(value.len() as u8);
                    buf.put_slice(value);
                }
                0xE7 | 0xA5 => {
                    // NVARCHARTYPE, BIGVARBINTYPE
                    if self.type_info.max_length == Some(0xFFFF) {
                        // MAX type - use PLP format
                        // For simplicity, send as single chunk
                        let total_len = value.len() as u64;
                        buf.put_u64_le(total_len);
                        buf.put_u32_le(value.len() as u32);
                        buf.put_slice(value);
                        buf.put_u32_le(0); // Terminator
                    } else {
                        buf.put_u16_le(value.len() as u16);
                        buf.put_slice(value);
                    }
                }
                0x24 => {
                    // GUIDTYPE
                    buf.put_u8(value.len() as u8);
                    buf.put_slice(value);
                }
                0x28 => {
                    // DATETYPE (fixed 3 bytes)
                    buf.put_slice(value);
                }
                0x2A => {
                    // DATETIME2TYPE
                    buf.put_u8(value.len() as u8);
                    buf.put_slice(value);
                }
                0x6C => {
                    // DECIMALNTYPE
                    buf.put_u8(value.len() as u8);
                    buf.put_slice(value);
                }
                0xF3 => {
                    // TVP (Table-Valued Parameter)
                    // TVP values are self-delimiting: they contain complete metadata,
                    // row data, and end token (TVP_END_TOKEN = 0x00). No length prefix.
                    buf.put_slice(value);
                }
                _ => {
                    // Generic: assume length-prefixed
                    buf.put_u8(value.len() as u8);
                    buf.put_slice(value);
                }
            }
        } else {
            // NULL value
            match self.type_info.type_id {
                0xE7 | 0xA5 => {
                    // Variable-length types use 0xFFFF for NULL
                    if self.type_info.max_length == Some(0xFFFF) {
                        buf.put_u64_le(0xFFFFFFFFFFFFFFFF); // PLP NULL
                    } else {
                        buf.put_u16_le(0xFFFF);
                    }
                }
                _ => {
                    buf.put_u8(0); // Zero-length for NULL
                }
            }
        }
    }
}

/// RPC request builder.
#[derive(Debug, Clone)]
pub struct RpcRequest {
    /// Procedure name (if using named procedure).
    proc_name: Option<String>,
    /// Procedure ID (if using well-known procedure).
    proc_id: Option<ProcId>,
    /// Option flags.
    options: RpcOptionFlags,
    /// Parameters.
    params: Vec<RpcParam>,
}

impl RpcRequest {
    /// Create a new RPC request for a named procedure.
    pub fn named(proc_name: impl Into<String>) -> Self {
        Self {
            proc_name: Some(proc_name.into()),
            proc_id: None,
            options: RpcOptionFlags::default(),
            params: Vec::new(),
        }
    }

    /// Create a new RPC request for a well-known procedure.
    pub fn by_id(proc_id: ProcId) -> Self {
        Self {
            proc_name: None,
            proc_id: Some(proc_id),
            options: RpcOptionFlags::default(),
            params: Vec::new(),
        }
    }

    /// Create an sp_executesql request.
    ///
    /// This is the primary method for parameterized queries.
    ///
    /// # Example
    ///
    /// ```
    /// use tds_protocol::rpc::{RpcRequest, RpcParam};
    ///
    /// let rpc = RpcRequest::execute_sql(
    ///     "SELECT * FROM users WHERE id = @p1 AND name = @p2",
    ///     vec![
    ///         RpcParam::int("@p1", 42),
    ///         RpcParam::nvarchar("@p2", "Alice"),
    ///     ],
    /// );
    /// ```
    pub fn execute_sql(sql: &str, params: Vec<RpcParam>) -> Self {
        let mut request = Self::by_id(ProcId::ExecuteSql);

        // First parameter: the SQL statement (NVARCHAR(MAX))
        request.params.push(RpcParam::nvarchar("", sql));

        // Second parameter: parameter declarations
        if !params.is_empty() {
            let declarations = Self::build_param_declarations(&params);
            request.params.push(RpcParam::nvarchar("", &declarations));
        }

        // Add the actual parameters
        request.params.extend(params);

        request
    }

    /// Build parameter declaration string for sp_executesql.
    fn build_param_declarations(params: &[RpcParam]) -> String {
        params
            .iter()
            .map(|p| {
                let name = if p.name.starts_with('@') {
                    p.name.clone()
                } else if p.name.is_empty() {
                    // Generate positional name
                    format!(
                        "@p{}",
                        params.iter().position(|x| x.name == p.name).unwrap_or(0) + 1
                    )
                } else {
                    format!("@{}", p.name)
                };

                let type_name: String = match p.type_info.type_id {
                    0x26 => match p.type_info.max_length {
                        Some(1) => "tinyint".to_string(),
                        Some(2) => "smallint".to_string(),
                        Some(4) => "int".to_string(),
                        Some(8) => "bigint".to_string(),
                        _ => "int".to_string(),
                    },
                    0x68 => "bit".to_string(),
                    0x6D => match p.type_info.max_length {
                        Some(4) => "real".to_string(),
                        _ => "float".to_string(),
                    },
                    0xE7 => {
                        if p.type_info.max_length == Some(0xFFFF) {
                            "nvarchar(max)".to_string()
                        } else {
                            let len = p.type_info.max_length.unwrap_or(4000) / 2;
                            format!("nvarchar({})", len)
                        }
                    }
                    0xA5 => {
                        if p.type_info.max_length == Some(0xFFFF) {
                            "varbinary(max)".to_string()
                        } else {
                            let len = p.type_info.max_length.unwrap_or(8000);
                            format!("varbinary({})", len)
                        }
                    }
                    0x24 => "uniqueidentifier".to_string(),
                    0x28 => "date".to_string(),
                    0x2A => {
                        let scale = p.type_info.scale.unwrap_or(7);
                        format!("datetime2({})", scale)
                    }
                    0x6C => {
                        let precision = p.type_info.precision.unwrap_or(18);
                        let scale = p.type_info.scale.unwrap_or(0);
                        format!("decimal({}, {})", precision, scale)
                    }
                    0xF3 => {
                        // TVP - Table-Valued Parameter
                        // Must be declared with the table type name and READONLY
                        if let Some(ref tvp_name) = p.type_info.tvp_type_name {
                            format!("{} READONLY", tvp_name)
                        } else {
                            // Fallback if type name is missing (shouldn't happen)
                            "sql_variant".to_string()
                        }
                    }
                    _ => "sql_variant".to_string(),
                };

                format!("{} {}", name, type_name)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Create an sp_prepare request.
    pub fn prepare(sql: &str, params: &[RpcParam]) -> Self {
        let mut request = Self::by_id(ProcId::Prepare);

        // OUT: handle (INT)
        request
            .params
            .push(RpcParam::null("@handle", TypeInfo::int()).as_output());

        // Param declarations
        let declarations = Self::build_param_declarations(params);
        request
            .params
            .push(RpcParam::nvarchar("@params", &declarations));

        // SQL statement
        request.params.push(RpcParam::nvarchar("@stmt", sql));

        // Options (1 = WITH RECOMPILE)
        request.params.push(RpcParam::int("@options", 1));

        request
    }

    /// Create an sp_execute request.
    pub fn execute(handle: i32, params: Vec<RpcParam>) -> Self {
        let mut request = Self::by_id(ProcId::Execute);

        // Handle from sp_prepare
        request.params.push(RpcParam::int("@handle", handle));

        // Add parameters
        request.params.extend(params);

        request
    }

    /// Create an sp_unprepare request.
    pub fn unprepare(handle: i32) -> Self {
        let mut request = Self::by_id(ProcId::Unprepare);
        request.params.push(RpcParam::int("@handle", handle));
        request
    }

    /// Set option flags.
    #[must_use]
    pub fn with_options(mut self, options: RpcOptionFlags) -> Self {
        self.options = options;
        self
    }

    /// Add a parameter.
    #[must_use]
    pub fn param(mut self, param: RpcParam) -> Self {
        self.params.push(param);
        self
    }

    /// Encode the RPC request to bytes (auto-commit mode).
    ///
    /// For requests within an explicit transaction, use [`Self::encode_with_transaction`].
    #[must_use]
    pub fn encode(&self) -> Bytes {
        self.encode_with_transaction(0)
    }

    /// Encode the RPC request with a transaction descriptor.
    ///
    /// Per MS-TDS spec, when executing within an explicit transaction:
    /// - The `transaction_descriptor` MUST be the value returned by the server
    ///   in the BeginTransaction EnvChange token.
    /// - For auto-commit mode (no explicit transaction), use 0.
    ///
    /// # Arguments
    ///
    /// * `transaction_descriptor` - The transaction descriptor from BeginTransaction EnvChange,
    ///   or 0 for auto-commit mode.
    #[must_use]
    pub fn encode_with_transaction(&self, transaction_descriptor: u64) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);

        // ALL_HEADERS - TDS 7.2+ requires this section
        // Total length placeholder (will be filled in)
        let all_headers_start = buf.len();
        buf.put_u32_le(0); // Total length placeholder

        // Transaction descriptor header (required for RPC)
        // Per MS-TDS 2.2.5.3: HeaderLength (4) + HeaderType (2) + TransactionDescriptor (8) + OutstandingRequestCount (4)
        buf.put_u32_le(18); // Header length
        buf.put_u16_le(0x0002); // Header type: transaction descriptor
        buf.put_u64_le(transaction_descriptor); // Transaction descriptor from BeginTransaction EnvChange
        buf.put_u32_le(1); // Outstanding request count (1 for non-MARS connections)

        // Fill in ALL_HEADERS total length
        let all_headers_len = buf.len() - all_headers_start;
        let len_bytes = (all_headers_len as u32).to_le_bytes();
        buf[all_headers_start..all_headers_start + 4].copy_from_slice(&len_bytes);

        // Procedure name or ID
        if let Some(proc_id) = self.proc_id {
            // Use PROCID format
            buf.put_u16_le(0xFFFF); // Name length = 0xFFFF indicates PROCID follows
            buf.put_u16_le(proc_id as u16);
        } else if let Some(ref proc_name) = self.proc_name {
            // Use procedure name
            let name_len = proc_name.encode_utf16().count() as u16;
            buf.put_u16_le(name_len);
            write_utf16_string(&mut buf, proc_name);
        }

        // Option flags
        buf.put_u16_le(self.options.encode());

        // Parameters
        for param in &self.params {
            param.encode(&mut buf);
        }

        buf.freeze()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_proc_id_values() {
        assert_eq!(ProcId::ExecuteSql as u16, 0x000A);
        assert_eq!(ProcId::Prepare as u16, 0x000B);
        assert_eq!(ProcId::Execute as u16, 0x000C);
        assert_eq!(ProcId::Unprepare as u16, 0x000F);
    }

    #[test]
    fn test_option_flags_encode() {
        let flags = RpcOptionFlags::new().with_recompile(true);
        assert_eq!(flags.encode(), 0x0001);
    }

    #[test]
    fn test_param_flags_encode() {
        let flags = ParamFlags::new().output();
        assert_eq!(flags.encode(), 0x01);
    }

    #[test]
    fn test_int_param() {
        let param = RpcParam::int("@p1", 42);
        assert_eq!(param.name, "@p1");
        assert_eq!(param.type_info.type_id, 0x26);
        assert!(param.value.is_some());
    }

    #[test]
    fn test_nvarchar_param() {
        let param = RpcParam::nvarchar("@name", "Alice");
        assert_eq!(param.name, "@name");
        assert_eq!(param.type_info.type_id, 0xE7);
        // UTF-16 encoded "Alice" = 10 bytes
        assert_eq!(param.value.as_ref().unwrap().len(), 10);
    }

    #[test]
    fn test_execute_sql_request() {
        let rpc = RpcRequest::execute_sql(
            "SELECT * FROM users WHERE id = @p1",
            vec![RpcParam::int("@p1", 42)],
        );

        assert_eq!(rpc.proc_id, Some(ProcId::ExecuteSql));
        // SQL statement + param declarations + actual params
        assert_eq!(rpc.params.len(), 3);
    }

    #[test]
    fn test_param_declarations() {
        let params = vec![
            RpcParam::int("@p1", 42),
            RpcParam::nvarchar("@name", "Alice"),
        ];

        let decls = RpcRequest::build_param_declarations(&params);
        assert!(decls.contains("@p1 int"));
        assert!(decls.contains("@name nvarchar"));
    }

    #[test]
    fn test_rpc_encode_not_empty() {
        let rpc = RpcRequest::execute_sql("SELECT 1", vec![]);
        let encoded = rpc.encode();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_prepare_request() {
        let rpc = RpcRequest::prepare(
            "SELECT * FROM users WHERE id = @p1",
            &[RpcParam::int("@p1", 0)],
        );

        assert_eq!(rpc.proc_id, Some(ProcId::Prepare));
        // handle (output), params, stmt, options
        assert_eq!(rpc.params.len(), 4);
        assert!(rpc.params[0].flags.by_ref); // handle is OUTPUT
    }

    #[test]
    fn test_execute_request() {
        let rpc = RpcRequest::execute(123, vec![RpcParam::int("@p1", 42)]);

        assert_eq!(rpc.proc_id, Some(ProcId::Execute));
        assert_eq!(rpc.params.len(), 2); // handle + param
    }

    #[test]
    fn test_unprepare_request() {
        let rpc = RpcRequest::unprepare(123);

        assert_eq!(rpc.proc_id, Some(ProcId::Unprepare));
        assert_eq!(rpc.params.len(), 1); // just the handle
    }
}
