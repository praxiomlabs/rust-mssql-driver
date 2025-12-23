//! RSA-OAEP key unwrapping for Always Encrypted.
//!
//! This module implements the RSA-OAEP algorithm used to decrypt Column Encryption Keys (CEKs)
//! that are encrypted with Column Master Keys (CMKs).
//!
//! ## SQL Server CEK Encryption Format
//!
//! SQL Server encrypts CEKs with the following format:
//!
//! ```text
//! encrypted_cek = version (1 byte) + key_path_length (2 bytes, LE) + key_path (UTF-16LE)
//!               + ciphertext_length (2 bytes, LE) + ciphertext
//! ```
//!
//! The `ciphertext` is the RSA-OAEP encrypted CEK.
//!
//! ## RSA-OAEP Parameters
//!
//! - **Hash function**: SHA-256 (for non-CNG providers)
//! - **MGF**: MGF1-SHA-256
//! - **Label**: Empty

use rsa::{
    Oaep, RsaPrivateKey, pkcs1::DecodeRsaPrivateKey, pkcs8::DecodePrivateKey,
    traits::PublicKeyParts,
};
use sha2::Sha256;

use crate::encryption::EncryptionError;

/// Version byte for SQL Server CEK encryption format.
const CEK_VERSION_BYTE: u8 = 0x01;

/// RSA-OAEP key unwrapper for decrypting Column Encryption Keys.
pub struct RsaKeyUnwrapper {
    private_key: RsaPrivateKey,
}

impl RsaKeyUnwrapper {
    /// Create a new unwrapper from a PEM-encoded RSA private key.
    ///
    /// Supports both PKCS#1 and PKCS#8 formats.
    ///
    /// # Arguments
    ///
    /// * `pem` - PEM-encoded RSA private key
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be parsed.
    pub fn from_pem(pem: &str) -> Result<Self, EncryptionError> {
        // Try PKCS#8 format first
        let private_key = RsaPrivateKey::from_pkcs8_pem(pem)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
            .map_err(|e| {
                EncryptionError::CmkError(format!("Failed to parse RSA private key: {}", e))
            })?;

        Ok(Self { private_key })
    }

    /// Create a new unwrapper from DER-encoded RSA private key bytes.
    ///
    /// # Arguments
    ///
    /// * `der` - DER-encoded RSA private key (PKCS#8 format)
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be parsed.
    pub fn from_der(der: &[u8]) -> Result<Self, EncryptionError> {
        let private_key = RsaPrivateKey::from_pkcs8_der(der)
            .or_else(|_| RsaPrivateKey::from_pkcs1_der(der))
            .map_err(|e| {
                EncryptionError::CmkError(format!("Failed to parse RSA private key: {}", e))
            })?;

        Ok(Self { private_key })
    }

    /// Create a new unwrapper from an existing RSA private key.
    pub fn from_key(private_key: RsaPrivateKey) -> Self {
        Self { private_key }
    }

    /// Decrypt a Column Encryption Key (CEK) using RSA-OAEP.
    ///
    /// # Arguments
    ///
    /// * `encrypted_cek` - The encrypted CEK in SQL Server format
    ///
    /// # Returns
    ///
    /// The decrypted CEK (32 bytes for AES-256).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The encrypted CEK format is invalid
    /// - RSA decryption fails
    pub fn decrypt_cek(&self, encrypted_cek: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        // Parse SQL Server CEK format
        let ciphertext = self.parse_encrypted_cek(encrypted_cek)?;

        // Decrypt using RSA-OAEP with SHA-256
        let padding = Oaep::new::<Sha256>();
        let decrypted = self.private_key.decrypt(padding, ciphertext).map_err(|e| {
            EncryptionError::CekDecryptionFailed(format!("RSA-OAEP decryption failed: {}", e))
        })?;

        Ok(decrypted)
    }

    /// Decrypt raw RSA-OAEP ciphertext (without SQL Server header).
    ///
    /// Use this when you have just the RSA ciphertext without the SQL Server envelope.
    pub fn decrypt_raw(&self, ciphertext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let padding = Oaep::new::<Sha256>();
        self.private_key.decrypt(padding, ciphertext).map_err(|e| {
            EncryptionError::CekDecryptionFailed(format!("RSA-OAEP decryption failed: {}", e))
        })
    }

    /// Parse the SQL Server encrypted CEK format.
    ///
    /// Format:
    /// - Version (1 byte): Must be 0x01
    /// - Key path length (2 bytes, little-endian)
    /// - Key path (UTF-16LE encoded string)
    /// - Ciphertext length (2 bytes, little-endian)
    /// - Ciphertext (RSA-OAEP encrypted CEK)
    fn parse_encrypted_cek<'a>(&self, data: &'a [u8]) -> Result<&'a [u8], EncryptionError> {
        if data.len() < 5 {
            return Err(EncryptionError::CekDecryptionFailed(
                "Encrypted CEK too short".into(),
            ));
        }

        // Check version
        if data[0] != CEK_VERSION_BYTE {
            return Err(EncryptionError::CekDecryptionFailed(format!(
                "Invalid CEK version: expected {:#04x}, got {:#04x}",
                CEK_VERSION_BYTE, data[0]
            )));
        }

        // Read key path length (2 bytes, little-endian)
        let key_path_len = u16::from_le_bytes([data[1], data[2]]) as usize;

        // Calculate offset after key path
        let ciphertext_len_offset = 3 + key_path_len;
        if data.len() < ciphertext_len_offset + 2 {
            return Err(EncryptionError::CekDecryptionFailed(
                "Encrypted CEK truncated: missing ciphertext length".into(),
            ));
        }

        // Read ciphertext length (2 bytes, little-endian)
        let ciphertext_len =
            u16::from_le_bytes([data[ciphertext_len_offset], data[ciphertext_len_offset + 1]])
                as usize;

        // Calculate ciphertext offset
        let ciphertext_offset = ciphertext_len_offset + 2;
        if data.len() < ciphertext_offset + ciphertext_len {
            return Err(EncryptionError::CekDecryptionFailed(format!(
                "Encrypted CEK truncated: expected {} bytes of ciphertext, got {}",
                ciphertext_len,
                data.len() - ciphertext_offset
            )));
        }

        Ok(&data[ciphertext_offset..ciphertext_offset + ciphertext_len])
    }

    /// Get the RSA key size in bits.
    pub fn key_bits(&self) -> usize {
        self.private_key.size() * 8
    }
}

/// Create an encrypted CEK in SQL Server format for testing.
///
/// This is useful for testing the parsing logic.
#[cfg(test)]
pub fn create_test_encrypted_cek(key_path: &str, ciphertext: &[u8]) -> Vec<u8> {
    // Convert key path to UTF-16LE
    let key_path_utf16: Vec<u8> = key_path
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();

    let mut result = Vec::new();

    // Version byte
    result.push(CEK_VERSION_BYTE);

    // Key path length (2 bytes, LE)
    let path_len = key_path_utf16.len() as u16;
    result.extend_from_slice(&path_len.to_le_bytes());

    // Key path (UTF-16LE)
    result.extend_from_slice(&key_path_utf16);

    // Ciphertext length (2 bytes, LE)
    let cipher_len = ciphertext.len() as u16;
    result.extend_from_slice(&cipher_len.to_le_bytes());

    // Ciphertext
    result.extend_from_slice(ciphertext);

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use rsa::{RsaPrivateKey, pkcs8::EncodePrivateKey};

    fn generate_test_key() -> RsaPrivateKey {
        let mut rng = rand::thread_rng();
        RsaPrivateKey::new(&mut rng, 2048).unwrap()
    }

    #[test]
    fn test_key_unwrapper_from_pem_pkcs8() {
        let key = generate_test_key();
        let pem = key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();

        let unwrapper = RsaKeyUnwrapper::from_pem(&pem).unwrap();
        assert_eq!(unwrapper.key_bits(), 2048);
    }

    #[test]
    fn test_decrypt_raw() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key.clone());

        // Encrypt a test CEK
        let test_cek = [0x42u8; 32]; // Test CEK
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha256>();
        let mut rng = rand::thread_rng();
        let ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Decrypt and verify
        let decrypted = unwrapper.decrypt_raw(&ciphertext).unwrap();
        assert_eq!(decrypted, test_cek);
    }

    #[test]
    fn test_parse_encrypted_cek() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key.clone());

        // Create a test encrypted CEK with SQL Server format
        let test_ciphertext = vec![0xAB; 256]; // Fake ciphertext
        let encrypted_cek = create_test_encrypted_cek("TestKeyPath", &test_ciphertext);

        // Parse should extract the ciphertext
        let extracted = unwrapper.parse_encrypted_cek(&encrypted_cek).unwrap();
        assert_eq!(extracted, &test_ciphertext[..]);
    }

    #[test]
    fn test_parse_encrypted_cek_invalid_version() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key);

        let mut data = create_test_encrypted_cek("Test", &[0u8; 32]);
        data[0] = 0x02; // Invalid version

        let result = unwrapper.parse_encrypted_cek(&data);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid CEK version")
        );
    }

    #[test]
    fn test_parse_encrypted_cek_too_short() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key);

        let result = unwrapper.parse_encrypted_cek(&[0x01, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_cek_full_flow() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key.clone());

        // Generate a test CEK (32 bytes for AES-256)
        let test_cek = [0x55u8; 32];

        // Encrypt the CEK with RSA-OAEP
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha256>();
        let mut rng = rand::thread_rng();
        let rsa_ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Create SQL Server format encrypted CEK
        let encrypted_cek = create_test_encrypted_cek("CurrentUser/My/TestCert", &rsa_ciphertext);

        // Decrypt and verify
        let decrypted = unwrapper.decrypt_cek(&encrypted_cek).unwrap();
        assert_eq!(decrypted, test_cek);
    }

    #[test]
    fn test_create_test_encrypted_cek() {
        let ciphertext = vec![0x12, 0x34, 0x56, 0x78];
        let encrypted = create_test_encrypted_cek("Test", &ciphertext);

        // Version byte
        assert_eq!(encrypted[0], 0x01);

        // Key path length (8 bytes for "Test" in UTF-16LE)
        let path_len = u16::from_le_bytes([encrypted[1], encrypted[2]]);
        assert_eq!(path_len, 8); // "Test" = 4 chars * 2 bytes

        // Ciphertext should be at the end
        assert_eq!(&encrypted[encrypted.len() - 4..], &ciphertext[..]);
    }
}
