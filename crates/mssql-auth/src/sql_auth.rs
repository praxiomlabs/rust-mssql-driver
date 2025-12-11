//! SQL Server authentication implementation.
//!
//! This module provides SQL Server username/password authentication,
//! which sends credentials via the TDS Login7 packet.

use std::borrow::Cow;

use crate::credentials::Credentials;
use crate::error::AuthError;
use crate::provider::{AuthData, AuthMethod, AuthProvider};

/// SQL Server authenticator for username/password authentication.
///
/// This provider handles traditional SQL Server authentication where
/// credentials are sent via the Login7 packet with password obfuscation.
///
/// # Security Note
///
/// The password is obfuscated (XOR + nibble swap), not encrypted.
/// Always use TLS encryption for the connection.
///
/// # Example
///
/// ```rust
/// use mssql_auth::SqlServerAuth;
///
/// let auth = SqlServerAuth::new("sa", "Password123!");
/// ```
#[derive(Clone)]
pub struct SqlServerAuth {
    username: Cow<'static, str>,
    password: Cow<'static, str>,
}

impl SqlServerAuth {
    /// Create a new SQL Server authenticator with credentials.
    pub fn new(
        username: impl Into<Cow<'static, str>>,
        password: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Create from existing credentials.
    ///
    /// Returns an error if the credentials are not SQL Server credentials.
    pub fn from_credentials(credentials: &Credentials) -> Result<Self, AuthError> {
        match credentials {
            Credentials::SqlServer { username, password } => Ok(Self {
                username: Cow::Owned(username.to_string()),
                password: Cow::Owned(password.to_string()),
            }),
            _ => Err(AuthError::UnsupportedMethod(
                "SqlServerAuth requires SQL Server credentials".into(),
            )),
        }
    }

    /// Get the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Encode a password for SQL Server Login7 packet.
    ///
    /// SQL Server uses a simple XOR-based obfuscation for passwords
    /// in Login7 packets. This is NOT encryption - it's just obfuscation.
    /// The connection should always be encrypted via TLS.
    ///
    /// # Algorithm
    ///
    /// For each UTF-16 code unit:
    /// 1. XOR each byte with 0xA5
    /// 2. Swap the high and low nibbles
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

impl AuthProvider for SqlServerAuth {
    fn method(&self) -> AuthMethod {
        AuthMethod::SqlServer
    }

    fn authenticate(&self) -> Result<AuthData, AuthError> {
        tracing::debug!(
            username = %self.username,
            "authenticating with SQL Server credentials"
        );

        let password_bytes = Self::encode_password(&self.password);

        Ok(AuthData::SqlServer {
            username: self.username.to_string(),
            password_bytes,
        })
    }
}

impl std::fmt::Debug for SqlServerAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlServerAuth")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

// Keep the old SqlAuthenticator for backward compatibility
/// SQL Server authenticator (legacy API).
///
/// This is kept for backward compatibility. Prefer using [`SqlServerAuth`] instead.
#[deprecated(since = "0.2.0", note = "Use SqlServerAuth instead")]
pub struct SqlAuthenticator;

#[allow(deprecated)]
impl SqlAuthenticator {
    /// Create a new SQL authenticator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Encode a password for SQL Server Login7 packet.
    #[must_use]
    pub fn encode_password(password: &str) -> Vec<u8> {
        SqlServerAuth::encode_password(password)
    }
}

#[allow(deprecated)]
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
        let encoded = SqlServerAuth::encode_password("test");
        assert!(!encoded.is_empty());
        assert_eq!(encoded.len(), 8); // 4 UTF-16 chars * 2 bytes each
    }

    #[test]
    fn test_password_encoding_known_value() {
        // Test against known encoded value
        // "a" in UTF-16LE is 0x61, 0x00
        // 0x61 ^ 0xA5 = 0xC4, nibble swap = 0x4C
        // 0x00 ^ 0xA5 = 0xA5, nibble swap = 0x5A
        let encoded = SqlServerAuth::encode_password("a");
        assert_eq!(encoded, vec![0x4C, 0x5A]);
    }

    #[test]
    fn test_sql_server_auth_provider() {
        let auth = SqlServerAuth::new("sa", "Password123!");

        assert_eq!(auth.method(), AuthMethod::SqlServer);
        assert_eq!(auth.username(), "sa");

        let data = auth.authenticate().unwrap();
        match data {
            AuthData::SqlServer {
                username,
                password_bytes,
            } => {
                assert_eq!(username, "sa");
                assert!(!password_bytes.is_empty());
            }
            _ => panic!("Expected SqlServer auth data"),
        }
    }

    #[test]
    fn test_from_credentials() {
        let creds = Credentials::sql_server("user", "pass");
        let auth = SqlServerAuth::from_credentials(&creds).unwrap();
        assert_eq!(auth.username(), "user");
    }

    #[test]
    fn test_from_credentials_wrong_type() {
        let creds = Credentials::azure_token("token");
        let result = SqlServerAuth::from_credentials(&creds);
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_redacts_password() {
        let auth = SqlServerAuth::new("sa", "secret");
        let debug = format!("{:?}", auth);
        assert!(debug.contains("sa"));
        assert!(!debug.contains("secret"));
        assert!(debug.contains("[REDACTED]"));
    }
}
