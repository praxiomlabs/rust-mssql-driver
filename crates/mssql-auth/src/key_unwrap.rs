//! RSA-OAEP key unwrapping for Always Encrypted.
//!
//! This module implements the RSA-OAEP algorithm used to decrypt Column Encryption Keys (CEKs)
//! that are encrypted with Column Master Keys (CMKs). The encrypted CEK arrives
//! in the canonical Microsoft envelope (see [`crate::cek_envelope`]); its
//! signature is verified against the CMK public key before unwrapping, as the
//! reference implementation requires.
//!
//! ## RSA-OAEP Parameters
//!
//! The `RSA_OAEP` algorithm in Always Encrypted CMK metadata means OAEP with
//! SHA-1 and MGF1-SHA-1 — the reference implementation wraps CEKs with
//! `RSAEncryptionPadding.OaepSHA1` (dotnet/SqlClient
//! `EncryptedColumnEncryptionKeyParameters.cs`). The envelope signature, by
//! contrast, uses PKCS#1 v1.5 over SHA-256.
//!
//! - **Hash function**: SHA-1
//! - **MGF**: MGF1-SHA-1
//! - **Label**: Empty

use rsa::{
    Oaep, Pkcs1v15Sign, RsaPrivateKey, pkcs1::DecodeRsaPrivateKey, pkcs8::DecodePrivateKey,
    traits::PublicKeyParts,
};
use sha1::Sha1;
use sha2::Sha256;

use crate::cek_envelope;
use crate::encryption::EncryptionError;

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
                EncryptionError::CmkError(format!("Failed to parse RSA private key: {e}"))
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
                EncryptionError::CmkError(format!("Failed to parse RSA private key: {e}"))
            })?;

        Ok(Self { private_key })
    }

    /// Create a new unwrapper from an existing RSA private key.
    pub fn from_key(private_key: RsaPrivateKey) -> Self {
        Self { private_key }
    }

    /// Decrypt a Column Encryption Key (CEK) using RSA-OAEP.
    ///
    /// Parses the canonical envelope, verifies its signature against this
    /// CMK's public key, and unwraps the CEK.
    ///
    /// # Arguments
    ///
    /// * `encrypted_cek` - The encrypted CEK envelope
    ///
    /// # Returns
    ///
    /// The decrypted CEK (32 bytes for AES-256).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The encrypted CEK envelope is invalid
    /// - The ciphertext or signature length does not match the RSA key size
    /// - Signature verification fails
    /// - RSA decryption fails
    pub fn decrypt_cek(&self, encrypted_cek: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let envelope = cek_envelope::parse(encrypted_cek)?;

        let key_size = self.private_key.size();
        if envelope.ciphertext.len() != key_size {
            return Err(EncryptionError::CekDecryptionFailed(format!(
                "CEK ciphertext length {} does not match RSA key size {key_size}",
                envelope.ciphertext.len()
            )));
        }
        if envelope.signature.len() != key_size {
            return Err(EncryptionError::CekDecryptionFailed(format!(
                "CEK signature length {} does not match RSA key size {key_size}",
                envelope.signature.len()
            )));
        }

        self.private_key
            .to_public_key()
            .verify(
                Pkcs1v15Sign::new::<Sha256>(),
                &envelope.signed_digest(),
                envelope.signature,
            )
            .map_err(|_| {
                EncryptionError::CekDecryptionFailed(
                    "CEK envelope signature verification failed".into(),
                )
            })?;

        // Decrypt using RSA-OAEP-SHA1 (what RSA_OAEP means in AE metadata)
        let padding = Oaep::new::<Sha1>();
        let decrypted = self
            .private_key
            .decrypt(padding, envelope.ciphertext)
            .map_err(|e| {
                EncryptionError::CekDecryptionFailed(format!("RSA-OAEP decryption failed: {e}"))
            })?;

        Ok(decrypted)
    }

    /// Decrypt raw RSA-OAEP ciphertext (without SQL Server header).
    ///
    /// Use this when you have just the RSA ciphertext without the SQL Server envelope.
    pub fn decrypt_raw(&self, ciphertext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let padding = Oaep::new::<Sha1>();
        self.private_key.decrypt(padding, ciphertext).map_err(|e| {
            EncryptionError::CekDecryptionFailed(format!("RSA-OAEP decryption failed: {e}"))
        })
    }

    /// Get the RSA key size in bits.
    pub fn key_bits(&self) -> usize {
        self.private_key.size() * 8
    }
}

/// Create a signed encrypted-CEK envelope for testing.
///
/// Builds the canonical envelope and signs it with the given CMK private key,
/// as standard provisioning tools do.
#[cfg(test)]
#[allow(clippy::expect_used)]
pub fn create_test_encrypted_cek(
    cmk: &RsaPrivateKey,
    key_path: &str,
    ciphertext: &[u8],
) -> Vec<u8> {
    use sha2::Digest;

    let mut envelope = cek_envelope::build_signed_portion(key_path, ciphertext);
    let digest: [u8; 32] = Sha256::digest(&envelope).into();
    let signature = cmk
        .sign(Pkcs1v15Sign::new::<Sha256>(), &digest)
        .expect("test CMK signs");
    envelope.extend_from_slice(&signature);
    envelope
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
        let padding = Oaep::new::<Sha1>();
        let mut rng = rand::thread_rng();
        let ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Decrypt and verify
        let decrypted = unwrapper.decrypt_raw(&ciphertext).unwrap();
        assert_eq!(decrypted, test_cek);
    }

    #[test]
    fn test_decrypt_cek_full_flow() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key.clone());

        // Generate a test CEK (32 bytes for AES-256)
        let test_cek = [0x55u8; 32];

        // Encrypt the CEK with RSA-OAEP
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha1>();
        let mut rng = rand::thread_rng();
        let rsa_ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Create a canonical signed envelope
        let encrypted_cek =
            create_test_encrypted_cek(&key, "CurrentUser/My/TestCert", &rsa_ciphertext);

        // Decrypt and verify
        let decrypted = unwrapper.decrypt_cek(&encrypted_cek).unwrap();
        assert_eq!(decrypted, test_cek);
    }

    #[test]
    fn test_decrypt_cek_rejects_tampered_envelope() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key.clone());

        let test_cek = [0x55u8; 32];
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha1>();
        let mut rng = rand::thread_rng();
        let rsa_ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        let mut encrypted_cek = create_test_encrypted_cek(&key, "Test", &rsa_ciphertext);
        // Flip one ciphertext bit: the signature must now fail to verify.
        encrypted_cek[20] ^= 0x01;

        let err = unwrapper.decrypt_cek(&encrypted_cek).unwrap_err();
        assert!(err.to_string().contains("signature verification failed"));
    }

    #[test]
    fn test_decrypt_cek_rejects_wrong_signer() {
        let key = generate_test_key();
        let unwrapper = RsaKeyUnwrapper::from_key(key.clone());

        let test_cek = [0x55u8; 32];
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha1>();
        let mut rng = rand::thread_rng();
        let rsa_ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Envelope signed by a DIFFERENT key than the CMK.
        let other_key = generate_test_key();
        let encrypted_cek = create_test_encrypted_cek(&other_key, "Test", &rsa_ciphertext);

        let err = unwrapper.decrypt_cek(&encrypted_cek).unwrap_err();
        assert!(err.to_string().contains("signature verification failed"));
    }
}
