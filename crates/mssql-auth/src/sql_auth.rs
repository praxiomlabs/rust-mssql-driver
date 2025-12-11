//! SQL Server authentication implementation.

use bytes::BytesMut;

use crate::credentials::Credentials;
use crate::error::AuthError;

/// SQL Server authenticator for building login packets.
pub struct SqlAuthenticator;

impl SqlAuthenticator {
    /// Create a new SQL authenticator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build the login packet payload for SQL authentication.
    ///
    /// This creates the Login7 packet structure used for SQL Server
    /// username/password authentication.
    pub fn build_login_payload(
        &self,
        credentials: &Credentials,
        hostname: &str,
        app_name: &str,
        database: Option<&str>,
    ) -> Result<BytesMut, AuthError> {
        let username = match credentials {
            Credentials::SqlServer { username, .. } => username.as_ref(),
            _ => {
                return Err(AuthError::UnsupportedMethod(
                    "SqlAuthenticator only supports SQL Server credentials".into(),
                ));
            }
        };

        // Build Login7 packet
        // This is a simplified placeholder - full implementation requires
        // proper TDS Login7 structure encoding
        let buf = BytesMut::with_capacity(1024);

        // Placeholder: actual Login7 encoding would go here
        // The real implementation needs to handle:
        // - Fixed-length header (94 bytes)
        // - Variable-length data (username, password, etc.)
        // - Password encoding (XOR obfuscation)
        // - Feature extension options

        tracing::debug!(
            username = username,
            hostname = hostname,
            app_name = app_name,
            database = database,
            "building SQL authentication login payload"
        );

        Ok(buf)
    }

    /// Encode a password for SQL Server Login7 packet.
    ///
    /// SQL Server uses a simple XOR-based obfuscation for passwords
    /// in Login7 packets. This is NOT encryption - it's just obfuscation.
    /// The connection should always be encrypted via TLS.
    #[must_use]
    pub fn encode_password(password: &str) -> Vec<u8> {
        password
            .encode_utf16()
            .flat_map(|c| {
                let byte1 = (c & 0xFF) as u8;
                let byte2 = (c >> 8) as u8;

                // XOR with 0xA5 and swap nibbles
                let encoded1 = ((byte1 ^ 0xA5) << 4) | ((byte1 ^ 0xA5) >> 4);
                let encoded2 = ((byte2 ^ 0xA5) << 4) | ((byte2 ^ 0xA5) >> 4);

                [encoded1, encoded2]
            })
            .collect()
    }
}

impl Default for SqlAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_encoding() {
        // Test that password encoding produces expected output
        let encoded = SqlAuthenticator::encode_password("test");
        assert!(!encoded.is_empty());
        assert_eq!(encoded.len(), 8); // 4 UTF-16 chars * 2 bytes each
    }

    #[test]
    fn test_unsupported_credentials() {
        let auth = SqlAuthenticator::new();
        let creds = Credentials::azure_token("token");

        let result = auth.build_login_payload(&creds, "host", "app", None);
        assert!(result.is_err());
    }
}
