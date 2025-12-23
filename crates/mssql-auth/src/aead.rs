//! AEAD_AES_256_CBC_HMAC_SHA256 encryption algorithm for Always Encrypted.
//!
//! This module implements the authenticated encryption scheme used by SQL Server's
//! Always Encrypted feature, following the IETF draft specification:
//! <https://tools.ietf.org/html/draft-mcgrew-aead-aes-cbc-hmac-sha2-05>
//!
//! ## Algorithm Overview
//!
//! The algorithm uses an Encrypt-then-MAC approach:
//! 1. Derive encryption, MAC, and IV keys from the Column Encryption Key (CEK)
//! 2. Generate IV (random for randomized, deterministic for deterministic encryption)
//! 3. Encrypt plaintext with AES-256-CBC + PKCS7 padding
//! 4. Compute MAC over version + IV + ciphertext
//! 5. Concatenate: version_byte + MAC + IV + ciphertext
//!
//! ## Ciphertext Format
//!
//! ```text
//! ┌──────────┬────────────┬────────────┬─────────────────────────┐
//! │ Version  │    MAC     │     IV     │   AES-256-CBC Cipher    │
//! │ (1 byte) │ (32 bytes) │ (16 bytes) │   (variable, min 16)    │
//! └──────────┴────────────┴────────────┴─────────────────────────┘
//! ```
//!
//! Minimum ciphertext size: 65 bytes (1 + 32 + 16 + 16)

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use crate::encryption::{EncryptionError, EncryptionType};

/// Version byte for the ciphertext format.
const VERSION_BYTE: u8 = 0x01;

/// AES block size in bytes.
const AES_BLOCK_SIZE: usize = 16;

/// AES-256 key size in bytes.
const AES_KEY_SIZE: usize = 32;

/// HMAC-SHA256 output size in bytes.
const MAC_SIZE: usize = 32;

/// IV size in bytes (128 bits).
const IV_SIZE: usize = 16;

/// Minimum ciphertext size: version(1) + MAC(32) + IV(16) + min_cipher(16).
const MIN_CIPHERTEXT_SIZE: usize = 1 + MAC_SIZE + IV_SIZE + AES_BLOCK_SIZE;

/// Key derivation labels as specified by Microsoft.
const ENCRYPTION_KEY_LABEL: &[u8] = b"Microsoft SQL Server cell encryption key";
const MAC_KEY_LABEL: &[u8] = b"Microsoft SQL Server cell MAC key";
const IV_KEY_LABEL: &[u8] = b"Microsoft SQL Server cell IV key";

/// Algorithm name used in key derivation.
const ALGORITHM_NAME: &[u8] = b"AEAD_AES_256_CBC_HMAC_SHA_256";

/// Type aliases for the crypto primitives.
type HmacSha256 = Hmac<Sha256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Derived encryption keys from a Column Encryption Key (CEK).
///
/// This structure holds the three derived keys used for AEAD encryption:
/// - `enc_key`: Used for AES-256-CBC encryption/decryption
/// - `mac_key`: Used for HMAC-SHA256 authentication
/// - `iv_key`: Used for deterministic IV generation
#[derive(Clone)]
pub struct DerivedKeys {
    /// Encryption key for AES-256-CBC.
    enc_key: [u8; AES_KEY_SIZE],
    /// MAC key for HMAC-SHA256.
    mac_key: [u8; AES_KEY_SIZE],
    /// IV key for deterministic IV derivation.
    iv_key: [u8; AES_KEY_SIZE],
}

impl DerivedKeys {
    /// Derive encryption keys from a Column Encryption Key (CEK).
    ///
    /// Uses HMAC-SHA256 with specific labels to derive three sub-keys:
    /// - `enc_key = HMAC-SHA256(CEK, label + algorithm + cek_length)`
    ///
    /// # Arguments
    ///
    /// * `cek` - The Column Encryption Key (must be 32 bytes for AES-256)
    ///
    /// # Errors
    ///
    /// Returns an error if the CEK is not exactly 32 bytes.
    pub fn derive(cek: &[u8]) -> Result<Self, EncryptionError> {
        if cek.len() != AES_KEY_SIZE {
            return Err(EncryptionError::ConfigurationError(format!(
                "CEK must be {} bytes, got {}",
                AES_KEY_SIZE,
                cek.len()
            )));
        }

        let cek_length = (cek.len() as u16).to_le_bytes();

        // Derive encryption key
        let enc_key = Self::derive_key(cek, ENCRYPTION_KEY_LABEL, &cek_length)?;

        // Derive MAC key
        let mac_key = Self::derive_key(cek, MAC_KEY_LABEL, &cek_length)?;

        // Derive IV key
        let iv_key = Self::derive_key(cek, IV_KEY_LABEL, &cek_length)?;

        Ok(Self {
            enc_key,
            mac_key,
            iv_key,
        })
    }

    /// Derive a single key using HMAC-SHA256.
    fn derive_key(
        cek: &[u8],
        label: &[u8],
        cek_length: &[u8],
    ) -> Result<[u8; AES_KEY_SIZE], EncryptionError> {
        let mut mac = HmacSha256::new_from_slice(cek)
            .map_err(|e| EncryptionError::EncryptionFailed(format!("HMAC init failed: {}", e)))?;

        mac.update(label);
        mac.update(ALGORITHM_NAME);
        mac.update(cek_length);

        let result = mac.finalize().into_bytes();
        let mut key = [0u8; AES_KEY_SIZE];
        key.copy_from_slice(&result);
        Ok(key)
    }

    /// Generate an IV for encryption.
    ///
    /// For randomized encryption, generates a cryptographically random IV.
    /// For deterministic encryption, derives the IV from the plaintext using HMAC-SHA256.
    pub fn generate_iv(
        &self,
        encryption_type: EncryptionType,
        plaintext: &[u8],
    ) -> Result<[u8; IV_SIZE], EncryptionError> {
        match encryption_type {
            EncryptionType::Randomized => {
                let mut iv = [0u8; IV_SIZE];
                rand::thread_rng().fill_bytes(&mut iv);
                Ok(iv)
            }
            EncryptionType::Deterministic => {
                // IV = HMAC-SHA256(iv_key, plaintext) truncated to 128 bits
                let mut mac = HmacSha256::new_from_slice(&self.iv_key).map_err(|e| {
                    EncryptionError::EncryptionFailed(format!("HMAC init failed: {}", e))
                })?;
                mac.update(plaintext);
                let result = mac.finalize().into_bytes();
                let mut iv = [0u8; IV_SIZE];
                iv.copy_from_slice(&result[..IV_SIZE]);
                Ok(iv)
            }
        }
    }
}

impl Drop for DerivedKeys {
    fn drop(&mut self) {
        // Zeroize keys on drop for security
        self.enc_key.fill(0);
        self.mac_key.fill(0);
        self.iv_key.fill(0);
    }
}

/// AEAD_AES_256_CBC_HMAC_SHA256 encryption context.
///
/// Provides encryption and decryption operations for Always Encrypted data.
pub struct AeadEncryptor {
    keys: DerivedKeys,
}

impl AeadEncryptor {
    /// Create a new encryptor from a Column Encryption Key.
    ///
    /// # Arguments
    ///
    /// * `cek` - The Column Encryption Key (32 bytes)
    ///
    /// # Errors
    ///
    /// Returns an error if key derivation fails.
    pub fn new(cek: &[u8]) -> Result<Self, EncryptionError> {
        let keys = DerivedKeys::derive(cek)?;
        Ok(Self { keys })
    }

    /// Encrypt plaintext using AEAD_AES_256_CBC_HMAC_SHA256.
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt
    /// * `encryption_type` - Randomized or Deterministic encryption
    ///
    /// # Returns
    ///
    /// The ciphertext in the format: version_byte + MAC + IV + aes_ciphertext
    ///
    /// # Errors
    ///
    /// Returns an error if encryption fails.
    pub fn encrypt(
        &self,
        plaintext: &[u8],
        encryption_type: EncryptionType,
    ) -> Result<Vec<u8>, EncryptionError> {
        // Generate IV
        let iv = self.keys.generate_iv(encryption_type, plaintext)?;

        // Calculate ciphertext size with PKCS7 padding
        let padded_len = ((plaintext.len() / AES_BLOCK_SIZE) + 1) * AES_BLOCK_SIZE;
        let mut cipher_buf = vec![0u8; padded_len];
        cipher_buf[..plaintext.len()].copy_from_slice(plaintext);

        // Encrypt with AES-256-CBC
        let cipher = Aes256CbcEnc::new_from_slices(&self.keys.enc_key, &iv)
            .map_err(|e| EncryptionError::EncryptionFailed(format!("AES init failed: {}", e)))?;

        let ciphertext = cipher
            .encrypt_padded_mut::<Pkcs7>(&mut cipher_buf, plaintext.len())
            .map_err(|e| {
                EncryptionError::EncryptionFailed(format!("AES encryption failed: {}", e))
            })?;

        // Compute MAC: HMAC-SHA256(mac_key, version + IV + ciphertext + version_length)
        let mac = self.compute_mac(&iv, ciphertext)?;

        // Build output: version + MAC + IV + ciphertext
        let mut output = Vec::with_capacity(1 + MAC_SIZE + IV_SIZE + ciphertext.len());
        output.push(VERSION_BYTE);
        output.extend_from_slice(&mac);
        output.extend_from_slice(&iv);
        output.extend_from_slice(ciphertext);

        Ok(output)
    }

    /// Decrypt ciphertext using AEAD_AES_256_CBC_HMAC_SHA256.
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The encrypted data (version + MAC + IV + aes_ciphertext)
    ///
    /// # Returns
    ///
    /// The decrypted plaintext.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Ciphertext is too short
    /// - Version byte is invalid
    /// - MAC verification fails (data may be tampered)
    /// - Decryption fails
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        // Validate minimum length
        if ciphertext.len() < MIN_CIPHERTEXT_SIZE {
            return Err(EncryptionError::DecryptionFailed(format!(
                "Ciphertext too short: {} bytes, minimum {}",
                ciphertext.len(),
                MIN_CIPHERTEXT_SIZE
            )));
        }

        // Validate version byte
        if ciphertext[0] != VERSION_BYTE {
            return Err(EncryptionError::DecryptionFailed(format!(
                "Invalid version byte: expected {:#04x}, got {:#04x}",
                VERSION_BYTE, ciphertext[0]
            )));
        }

        // Extract components
        let stored_mac = &ciphertext[1..1 + MAC_SIZE];
        let iv = &ciphertext[1 + MAC_SIZE..1 + MAC_SIZE + IV_SIZE];
        let encrypted_data = &ciphertext[1 + MAC_SIZE + IV_SIZE..];

        // Verify MAC
        let computed_mac = self.compute_mac(iv, encrypted_data)?;
        if !constant_time_compare(stored_mac, &computed_mac) {
            return Err(EncryptionError::DecryptionFailed(
                "MAC verification failed: data may be tampered".into(),
            ));
        }

        // Decrypt with AES-256-CBC
        let cipher = Aes256CbcDec::new_from_slices(&self.keys.enc_key, iv)
            .map_err(|e| EncryptionError::DecryptionFailed(format!("AES init failed: {}", e)))?;

        let mut buf = encrypted_data.to_vec();
        let plaintext = cipher.decrypt_padded_mut::<Pkcs7>(&mut buf).map_err(|e| {
            EncryptionError::DecryptionFailed(format!("AES decryption failed: {}", e))
        })?;

        Ok(plaintext.to_vec())
    }

    /// Compute the MAC for authentication.
    ///
    /// MAC = HMAC-SHA256(mac_key, version + IV + ciphertext + version_length)
    fn compute_mac(&self, iv: &[u8], ciphertext: &[u8]) -> Result<[u8; MAC_SIZE], EncryptionError> {
        let mut mac = HmacSha256::new_from_slice(&self.keys.mac_key)
            .map_err(|e| EncryptionError::EncryptionFailed(format!("HMAC init failed: {}", e)))?;

        mac.update(&[VERSION_BYTE]);
        mac.update(iv);
        mac.update(ciphertext);
        mac.update(&[1u8]); // version_length = 1

        let result = mac.finalize().into_bytes();
        let mut output = [0u8; MAC_SIZE];
        output.copy_from_slice(&result);
        Ok(output)
    }
}

/// Constant-time comparison to prevent timing attacks.
fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Test CEK for testing purposes (DO NOT USE IN PRODUCTION).
    fn test_cek() -> [u8; 32] {
        [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ]
    }

    #[test]
    fn test_key_derivation() {
        let cek = test_cek();
        let keys = DerivedKeys::derive(&cek).unwrap();

        // Keys should be derived (non-zero)
        assert!(!keys.enc_key.iter().all(|&b| b == 0));
        assert!(!keys.mac_key.iter().all(|&b| b == 0));
        assert!(!keys.iv_key.iter().all(|&b| b == 0));

        // Keys should be different from each other
        assert_ne!(keys.enc_key, keys.mac_key);
        assert_ne!(keys.mac_key, keys.iv_key);
        assert_ne!(keys.enc_key, keys.iv_key);
    }

    #[test]
    fn test_key_derivation_invalid_length() {
        let short_cek = [0u8; 16];
        let result = DerivedKeys::derive(&short_cek);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_decrypt_randomized() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Hello, SQL Server Always Encrypted!";
        let ciphertext = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        // Ciphertext should be longer than plaintext (version + MAC + IV + padding)
        assert!(ciphertext.len() >= MIN_CIPHERTEXT_SIZE);

        // Should start with version byte
        assert_eq!(ciphertext[0], VERSION_BYTE);

        // Decrypt and verify
        let decrypted = encryptor.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_deterministic() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Deterministic encryption test";

        // Encrypt twice
        let ciphertext1 = encryptor
            .encrypt(plaintext, EncryptionType::Deterministic)
            .unwrap();
        let ciphertext2 = encryptor
            .encrypt(plaintext, EncryptionType::Deterministic)
            .unwrap();

        // Deterministic encryption should produce same ciphertext
        assert_eq!(ciphertext1, ciphertext2);

        // Decrypt and verify
        let decrypted = encryptor.decrypt(&ciphertext1).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_randomized_produces_different_ciphertext() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Same plaintext";

        let ciphertext1 = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();
        let ciphertext2 = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        // Randomized encryption should produce different ciphertext
        assert_ne!(ciphertext1, ciphertext2);

        // Both should decrypt to the same plaintext
        assert_eq!(
            encryptor.decrypt(&ciphertext1).unwrap(),
            encryptor.decrypt(&ciphertext2).unwrap()
        );
    }

    #[test]
    fn test_decrypt_tampered_data() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Original data";
        let mut ciphertext = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        // Tamper with the ciphertext (modify a byte in the encrypted data)
        let last_idx = ciphertext.len() - 1;
        ciphertext[last_idx] ^= 0xFF;

        // Decryption should fail due to MAC verification
        let result = encryptor.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_version() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Test data";
        let mut ciphertext = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        // Change version byte
        ciphertext[0] = 0x02;

        let result = encryptor.decrypt(&ciphertext);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid version byte")
        );
    }

    #[test]
    fn test_decrypt_too_short() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let short_data = vec![0u8; 10];
        let result = encryptor.decrypt(&short_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn test_empty_plaintext() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"";
        let ciphertext = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        let decrypted = encryptor.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_large_plaintext() {
        let cek = test_cek();
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        // 10KB of data
        let plaintext: Vec<u8> = (0..10240).map(|i| (i % 256) as u8).collect();
        let ciphertext = encryptor
            .encrypt(&plaintext, EncryptionType::Randomized)
            .unwrap();

        let decrypted = encryptor.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_constant_time_compare() {
        let a = [1, 2, 3, 4, 5];
        let b = [1, 2, 3, 4, 5];
        let c = [1, 2, 3, 4, 6];

        assert!(constant_time_compare(&a, &b));
        assert!(!constant_time_compare(&a, &c));
        assert!(!constant_time_compare(&a, &[1, 2, 3]));
    }
}
