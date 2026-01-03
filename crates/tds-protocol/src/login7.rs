//! TDS LOGIN7 packet construction.
//!
//! The LOGIN7 packet is sent by the client to authenticate with SQL Server.
//! It contains client information, credentials, and feature negotiation data.
//!
//! ## Packet Structure
//!
//! The LOGIN7 packet has a complex structure with:
//! - Fixed-length header (94 bytes)
//! - Variable-length data section (strings are UTF-16LE)
//! - Optional feature extension block
//!
//! ## Security Note
//!
//! The password is obfuscated (not encrypted) using a simple XOR + bit rotation.
//! Always use TLS encryption for the connection.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::write_utf16_string;
use crate::prelude::*;
use crate::version::TdsVersion;

/// LOGIN7 packet header size (fixed portion).
pub const LOGIN7_HEADER_SIZE: usize = 94;

/// LOGIN7 option flags 1.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptionFlags1 {
    /// Use big-endian byte order.
    pub byte_order_be: bool,
    /// Character set (0 = ASCII, 1 = EBCDIC).
    pub char_ebcdic: bool,
    /// Floating point representation (0 = IEEE 754, 1 = VAX, 2 = ND5000).
    pub float_ieee: bool,
    /// Dump/load off.
    pub dump_load_off: bool,
    /// Use DB notification.
    pub use_db_notify: bool,
    /// Database is fatal.
    pub database_fatal: bool,
    /// Set language warning.
    pub set_lang_warn: bool,
}

impl OptionFlags1 {
    /// Convert to byte.
    ///
    /// Per MS-TDS 2.2.6.3 LOGIN7, OptionFlags1 layout:
    /// - bit 0: fByteOrder (0=little-endian, 1=big-endian)
    /// - bit 1: fChar (0=ASCII, 1=EBCDIC)
    /// - bits 2-3: fFloat (0=IEEE 754, 1=VAX, 2=ND5000)
    /// - bit 4: fDumpLoad
    /// - bit 5: fUseDB
    /// - bit 6: fDatabase
    /// - bit 7: fSetLang
    #[must_use]
    pub fn to_byte(&self) -> u8 {
        let mut flags = 0u8;
        if self.byte_order_be {
            flags |= 0x01; // bit 0
        }
        if self.char_ebcdic {
            flags |= 0x02; // bit 1
        }
        // Note: fFloat is bits 2-3, IEEE 754 = 0, so leave as 0 for IEEE
        // float_ieee being true means we use IEEE (which is 0, the default)
        if self.dump_load_off {
            flags |= 0x10; // bit 4
        }
        if self.use_db_notify {
            flags |= 0x20; // bit 5
        }
        if self.database_fatal {
            flags |= 0x40; // bit 6
        }
        if self.set_lang_warn {
            flags |= 0x80; // bit 7
        }
        flags
    }
}

/// LOGIN7 option flags 2.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptionFlags2 {
    /// Language is fatal.
    pub language_fatal: bool,
    /// ODBC driver.
    pub odbc: bool,
    /// Obsolete: transaction boundary.
    pub tran_boundary: bool,
    /// Obsolete: cache connect.
    pub cache_connect: bool,
    /// User type (0 = Normal, 1 = Server, 2 = DQ login, 3 = Replication).
    pub user_type: u8,
    /// Integrated security.
    pub integrated_security: bool,
}

impl OptionFlags2 {
    /// Convert to byte.
    #[must_use]
    pub fn to_byte(&self) -> u8 {
        let mut flags = 0u8;
        if self.language_fatal {
            flags |= 0x01;
        }
        if self.odbc {
            flags |= 0x02;
        }
        if self.tran_boundary {
            flags |= 0x04;
        }
        if self.cache_connect {
            flags |= 0x08;
        }
        flags |= (self.user_type & 0x07) << 4;
        if self.integrated_security {
            flags |= 0x80;
        }
        flags
    }
}

/// LOGIN7 type flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeFlags {
    /// SQL type (0 = DFLT, 1 = TSQL).
    pub sql_type: u8,
    /// OLEDB driver.
    pub oledb: bool,
    /// Read-only intent.
    pub read_only_intent: bool,
}

impl TypeFlags {
    /// Convert to byte.
    #[must_use]
    pub fn to_byte(&self) -> u8 {
        let mut flags = 0u8;
        flags |= self.sql_type & 0x0F;
        if self.oledb {
            flags |= 0x10;
        }
        if self.read_only_intent {
            flags |= 0x20;
        }
        flags
    }
}

/// LOGIN7 option flags 3.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptionFlags3 {
    /// Change password.
    pub change_password: bool,
    /// User instance.
    pub user_instance: bool,
    /// Send YUKON binary XML.
    pub send_yukon_binary_xml: bool,
    /// Unknown collation handling.
    pub unknown_collation_handling: bool,
    /// Feature extension.
    pub extension: bool,
}

impl OptionFlags3 {
    /// Convert to byte.
    #[must_use]
    pub fn to_byte(&self) -> u8 {
        let mut flags = 0u8;
        if self.change_password {
            flags |= 0x01;
        }
        if self.user_instance {
            flags |= 0x02;
        }
        if self.send_yukon_binary_xml {
            flags |= 0x04;
        }
        if self.unknown_collation_handling {
            flags |= 0x08;
        }
        if self.extension {
            flags |= 0x10;
        }
        flags
    }
}

/// Feature extension types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FeatureId {
    /// Session recovery.
    SessionRecovery = 0x01,
    /// Federated authentication.
    FedAuth = 0x02,
    /// Column encryption.
    ColumnEncryption = 0x04,
    /// Global transactions.
    GlobalTransactions = 0x05,
    /// Azure SQL Support for DB.
    AzureSqlSupport = 0x08,
    /// Data classification.
    DataClassification = 0x09,
    /// UTF-8 support.
    Utf8Support = 0x0A,
    /// Azure SQL DNS Caching.
    AzureSqlDnsCaching = 0x0B,
    /// Terminator.
    Terminator = 0xFF,
}

/// LOGIN7 packet builder.
#[derive(Debug, Clone)]
pub struct Login7 {
    /// TDS version to request.
    pub tds_version: TdsVersion,
    /// Requested packet size.
    pub packet_size: u32,
    /// Client program version.
    pub client_prog_version: u32,
    /// Client process ID.
    pub client_pid: u32,
    /// Connection ID (for connection pooling).
    pub connection_id: u32,
    /// Option flags 1.
    pub option_flags1: OptionFlags1,
    /// Option flags 2.
    pub option_flags2: OptionFlags2,
    /// Type flags.
    pub type_flags: TypeFlags,
    /// Option flags 3.
    pub option_flags3: OptionFlags3,
    /// Client timezone offset in minutes.
    pub client_timezone: i32,
    /// Client LCID (locale ID).
    pub client_lcid: u32,
    /// Hostname (client machine name).
    pub hostname: String,
    /// Username for SQL authentication.
    pub username: String,
    /// Password for SQL authentication.
    pub password: String,
    /// Application name.
    pub app_name: String,
    /// Server name.
    pub server_name: String,
    /// Unused field.
    pub unused: String,
    /// Client library name.
    pub library_name: String,
    /// Language.
    pub language: String,
    /// Database name.
    pub database: String,
    /// Client ID (MAC address, typically zeros).
    pub client_id: [u8; 6],
    /// SSPI data for integrated authentication.
    pub sspi_data: Vec<u8>,
    /// Attach DB filename (for LocalDB).
    pub attach_db_file: String,
    /// New password (for password change).
    pub new_password: String,
    /// Feature extensions.
    pub features: Vec<FeatureExtension>,
}

/// Feature extension data.
#[derive(Debug, Clone)]
pub struct FeatureExtension {
    /// Feature ID.
    pub feature_id: FeatureId,
    /// Feature data.
    pub data: Bytes,
}

impl Default for Login7 {
    fn default() -> Self {
        #[cfg(feature = "std")]
        let client_pid = std::process::id();
        #[cfg(not(feature = "std"))]
        let client_pid = 0;

        Self {
            tds_version: TdsVersion::V7_4,
            packet_size: 4096,
            client_prog_version: 0,
            client_pid,
            connection_id: 0,
            // Match Tiberius/standard SQL Server client flags
            option_flags1: OptionFlags1 {
                use_db_notify: true,
                database_fatal: true,
                ..Default::default()
            },
            option_flags2: OptionFlags2 {
                language_fatal: true,
                odbc: true,
                ..Default::default()
            },
            type_flags: TypeFlags::default(), // TSQL type is in sql_type field
            option_flags3: OptionFlags3 {
                unknown_collation_handling: true,
                ..Default::default()
            },
            client_timezone: 0,
            client_lcid: 0x0409, // English (US)
            hostname: String::new(),
            username: String::new(),
            password: String::new(),
            app_name: String::from("rust-mssql-driver"),
            server_name: String::new(),
            unused: String::new(),
            library_name: String::from("rust-mssql-driver"),
            language: String::new(),
            database: String::new(),
            client_id: [0u8; 6],
            sspi_data: Vec::new(),
            attach_db_file: String::new(),
            new_password: String::new(),
            features: Vec::new(),
        }
    }
}

impl Login7 {
    /// Create a new Login7 packet builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the TDS version.
    #[must_use]
    pub fn with_tds_version(mut self, version: TdsVersion) -> Self {
        self.tds_version = version;
        self
    }

    /// Set SQL authentication credentials.
    #[must_use]
    pub fn with_sql_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = username.into();
        self.password = password.into();
        self.option_flags2.integrated_security = false;
        self
    }

    /// Enable integrated (Windows) authentication.
    #[must_use]
    pub fn with_integrated_auth(mut self, sspi_data: Vec<u8>) -> Self {
        self.sspi_data = sspi_data;
        self.option_flags2.integrated_security = true;
        self
    }

    /// Set the database to connect to.
    #[must_use]
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = database.into();
        self
    }

    /// Set the hostname (client machine name).
    #[must_use]
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }

    /// Set the application name.
    #[must_use]
    pub fn with_app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = app_name.into();
        self
    }

    /// Set the server name.
    #[must_use]
    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = server_name.into();
        self
    }

    /// Set the packet size.
    #[must_use]
    pub fn with_packet_size(mut self, packet_size: u32) -> Self {
        self.packet_size = packet_size;
        self
    }

    /// Enable read-only intent for readable secondary connections.
    #[must_use]
    pub fn with_read_only_intent(mut self, read_only: bool) -> Self {
        self.type_flags.read_only_intent = read_only;
        self
    }

    /// Add a feature extension.
    #[must_use]
    pub fn with_feature(mut self, feature: FeatureExtension) -> Self {
        self.option_flags3.extension = true;
        self.features.push(feature);
        self
    }

    /// Encode the LOGIN7 packet to bytes.
    #[must_use]
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(512);

        // Calculate variable data offsets
        // Variable data starts after the 94-byte fixed header
        let mut offset = LOGIN7_HEADER_SIZE as u16;

        // Pre-calculate all UTF-16 lengths
        let hostname_len = self.hostname.encode_utf16().count() as u16;
        let username_len = self.username.encode_utf16().count() as u16;
        let password_len = self.password.encode_utf16().count() as u16;
        let app_name_len = self.app_name.encode_utf16().count() as u16;
        let server_name_len = self.server_name.encode_utf16().count() as u16;
        let unused_len = self.unused.encode_utf16().count() as u16;
        let library_name_len = self.library_name.encode_utf16().count() as u16;
        let language_len = self.language.encode_utf16().count() as u16;
        let database_len = self.database.encode_utf16().count() as u16;
        let sspi_len = self.sspi_data.len() as u16;
        let attach_db_len = self.attach_db_file.encode_utf16().count() as u16;
        let new_password_len = self.new_password.encode_utf16().count() as u16;

        // Build variable data buffer
        let mut var_data = BytesMut::new();

        // Hostname
        let hostname_offset = offset;
        write_utf16_string(&mut var_data, &self.hostname);
        offset += hostname_len * 2;

        // Username
        let username_offset = offset;
        write_utf16_string(&mut var_data, &self.username);
        offset += username_len * 2;

        // Password (obfuscated)
        let password_offset = offset;
        Self::write_obfuscated_password(&mut var_data, &self.password);
        offset += password_len * 2;

        // App name
        let app_name_offset = offset;
        write_utf16_string(&mut var_data, &self.app_name);
        offset += app_name_len * 2;

        // Server name
        let server_name_offset = offset;
        write_utf16_string(&mut var_data, &self.server_name);
        offset += server_name_len * 2;

        // Unused / Feature extension pointer
        let extension_offset = if self.option_flags3.extension {
            // Calculate feature extension offset after all other data
            let base = offset
                + unused_len * 2
                + library_name_len * 2
                + language_len * 2
                + database_len * 2
                + sspi_len
                + attach_db_len * 2
                + new_password_len * 2;
            // Store the offset where feature extension will be
            var_data.put_u32_le(base as u32);
            offset += 4;
            base
        } else {
            let unused_offset = offset;
            write_utf16_string(&mut var_data, &self.unused);
            offset += unused_len * 2;
            unused_offset
        };

        // Library name
        let library_name_offset = offset;
        write_utf16_string(&mut var_data, &self.library_name);
        offset += library_name_len * 2;

        // Language
        let language_offset = offset;
        write_utf16_string(&mut var_data, &self.language);
        offset += language_len * 2;

        // Database
        let database_offset = offset;
        write_utf16_string(&mut var_data, &self.database);
        offset += database_len * 2;

        // Client ID (6 bytes)
        // (Already handled in fixed header)

        // SSPI
        let sspi_offset = offset;
        var_data.put_slice(&self.sspi_data);
        offset += sspi_len;

        // Attach DB file
        let attach_db_offset = offset;
        write_utf16_string(&mut var_data, &self.attach_db_file);
        offset += attach_db_len * 2;

        // Change password
        let new_password_offset = offset;
        if !self.new_password.is_empty() {
            Self::write_obfuscated_password(&mut var_data, &self.new_password);
        }
        #[allow(unused_assignments)]
        {
            offset += new_password_len * 2;
        }

        // Feature extensions (if any)
        if self.option_flags3.extension {
            for feature in &self.features {
                var_data.put_u8(feature.feature_id as u8);
                var_data.put_u32_le(feature.data.len() as u32);
                var_data.put_slice(&feature.data);
            }
            var_data.put_u8(FeatureId::Terminator as u8);
        }

        // Calculate total length
        let total_length = LOGIN7_HEADER_SIZE + var_data.len();

        // Write fixed header
        buf.put_u32_le(total_length as u32); // Length
        buf.put_u32_le(self.tds_version.raw()); // TDS version
        buf.put_u32_le(self.packet_size); // Packet size
        buf.put_u32_le(self.client_prog_version); // Client program version
        buf.put_u32_le(self.client_pid); // Client PID
        buf.put_u32_le(self.connection_id); // Connection ID

        // Option flags
        buf.put_u8(self.option_flags1.to_byte());
        buf.put_u8(self.option_flags2.to_byte());
        buf.put_u8(self.type_flags.to_byte());
        buf.put_u8(self.option_flags3.to_byte());

        buf.put_i32_le(self.client_timezone); // Client timezone
        buf.put_u32_le(self.client_lcid); // Client LCID

        // Variable length field offsets and lengths
        buf.put_u16_le(hostname_offset);
        buf.put_u16_le(hostname_len);
        buf.put_u16_le(username_offset);
        buf.put_u16_le(username_len);
        buf.put_u16_le(password_offset);
        buf.put_u16_le(password_len);
        buf.put_u16_le(app_name_offset);
        buf.put_u16_le(app_name_len);
        buf.put_u16_le(server_name_offset);
        buf.put_u16_le(server_name_len);

        // Extension offset (or unused)
        if self.option_flags3.extension {
            buf.put_u16_le(extension_offset as u16);
            buf.put_u16_le(4); // Size of offset pointer
        } else {
            buf.put_u16_le(extension_offset as u16);
            buf.put_u16_le(unused_len);
        }

        buf.put_u16_le(library_name_offset);
        buf.put_u16_le(library_name_len);
        buf.put_u16_le(language_offset);
        buf.put_u16_le(language_len);
        buf.put_u16_le(database_offset);
        buf.put_u16_le(database_len);

        // Client ID (6 bytes)
        buf.put_slice(&self.client_id);

        buf.put_u16_le(sspi_offset);
        buf.put_u16_le(sspi_len);
        buf.put_u16_le(attach_db_offset);
        buf.put_u16_le(attach_db_len);
        buf.put_u16_le(new_password_offset);
        buf.put_u16_le(new_password_len);

        // SSPI Long (4 bytes, for SSPI > 65535 bytes)
        buf.put_u32_le(0);

        // Append variable data
        buf.put_slice(&var_data);

        buf.freeze()
    }

    /// Write password with TDS obfuscation.
    ///
    /// Per MS-TDS spec: For every byte in the password buffer, the client SHOULD first
    /// swap the four high bits with the four low bits and then do a bit-XOR with 0xA5.
    fn write_obfuscated_password(dst: &mut impl BufMut, password: &str) {
        for c in password.encode_utf16() {
            let low = (c & 0xFF) as u8;
            let high = ((c >> 8) & 0xFF) as u8;

            // Step 1: Swap nibbles (rotate by 4 bits)
            // Step 2: XOR with 0xA5
            let low_enc = low.rotate_right(4) ^ 0xA5;
            let high_enc = high.rotate_right(4) ^ 0xA5;

            dst.put_u8(low_enc);
            dst.put_u8(high_enc);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_login7_default() {
        let login = Login7::new();
        assert_eq!(login.tds_version, TdsVersion::V7_4);
        assert_eq!(login.packet_size, 4096);
        assert!(login.option_flags2.odbc);
    }

    #[test]
    fn test_login7_encode() {
        let login = Login7::new()
            .with_hostname("TESTHOST")
            .with_sql_auth("testuser", "testpass")
            .with_database("testdb")
            .with_app_name("TestApp");

        let encoded = login.encode();

        // Check that the packet starts with a valid length
        assert!(encoded.len() >= LOGIN7_HEADER_SIZE);

        // Check TDS version at offset 4 (after length)
        let tds_version = u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
        assert_eq!(tds_version, TdsVersion::V7_4.raw());
    }

    #[test]
    fn test_password_obfuscation() {
        // Known test case: "a" should encode to specific bytes
        let mut buf = BytesMut::new();
        Login7::write_obfuscated_password(&mut buf, "a");

        // 'a' = 0x0061 in UTF-16LE
        // Per MS-TDS: swap nibbles FIRST, then XOR with 0xA5
        // Low byte: 0x61 swap nibbles = 0x16, XOR 0xA5 = 0xB3
        // High byte: 0x00 swap nibbles = 0x00, XOR 0xA5 = 0xA5
        assert_eq!(buf.len(), 2);
        assert_eq!(buf[0], 0xB3);
        assert_eq!(buf[1], 0xA5);
    }

    #[test]
    fn test_option_flags() {
        let flags1 = OptionFlags1::default();
        assert_eq!(flags1.to_byte(), 0x00);

        let flags2 = OptionFlags2 {
            odbc: true,
            integrated_security: true,
            ..Default::default()
        };
        assert_eq!(flags2.to_byte(), 0x82);

        let flags3 = OptionFlags3 {
            extension: true,
            ..Default::default()
        };
        assert_eq!(flags3.to_byte(), 0x10);
    }
}
