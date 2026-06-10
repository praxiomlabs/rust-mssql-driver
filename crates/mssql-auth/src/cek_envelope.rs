//! Canonical encrypted-CEK envelope codec for Always Encrypted.
//!
//! Every Microsoft Always Encrypted client (.NET, SSMS, JDBC, ODBC,
//! PowerShell) wraps Column Encryption Keys in the same envelope, defined by
//! the reference implementation (dotnet/SqlClient
//! `AlwaysEncrypted/EncryptedColumnEncryptionKeyParameters.cs`):
//!
//! ```text
//! version        (1 byte, 0x01)
//! key_path_len   (2 bytes, u16 LE)
//! ciphertext_len (2 bytes, u16 LE)
//! key_path       (UTF-16LE, informational)
//! ciphertext     (RSA-OAEP-wrapped CEK, length = RSA key size)
//! signature      (RSA-PKCS1 over SHA-256 of all preceding bytes,
//!                 length = RSA key size; verification is mandatory)
//! ```
//!
//! Both length fields precede both variable-length payloads, and the trailing
//! signature binds the envelope to the Column Master Key. All key store
//! providers must parse this exact layout and verify the signature before
//! unwrapping, or CEKs provisioned by standard tooling (SSMS, .NET,
//! `New-SqlColumnEncryptionKey`) cannot be used.

use crate::encryption::EncryptionError;
use sha2::{Digest, Sha256};

/// Version byte for the encrypted-CEK envelope.
const CEK_VERSION_BYTE: u8 = 0x01;

/// A parsed encrypted-CEK envelope.
#[derive(Debug)]
pub struct CekEnvelope<'a> {
    /// The RSA-wrapped CEK.
    pub ciphertext: &'a [u8],
    /// RSA-PKCS1-SHA256 signature over [`signed_portion`](Self::signed_portion).
    pub signature: &'a [u8],
    /// Every byte preceding the signature (what the signature covers).
    pub signed_portion: &'a [u8],
}

impl CekEnvelope<'_> {
    /// SHA-256 digest of the signed portion, the input to signature
    /// verification (providers sign/verify digests, not raw data).
    pub fn signed_digest(&self) -> [u8; 32] {
        Sha256::digest(self.signed_portion).into()
    }
}

/// Parse an encrypted-CEK envelope in the canonical Microsoft layout.
///
/// Bounds and version are validated here; ciphertext/signature length checks
/// against the RSA key size and signature verification are the caller's
/// responsibility (only the key store provider knows the key).
///
/// # Errors
///
/// Returns [`EncryptionError::CekDecryptionFailed`] if the envelope is
/// truncated, has an unknown version, or has no signature bytes.
pub fn parse(data: &[u8]) -> Result<CekEnvelope<'_>, EncryptionError> {
    if data.len() < 5 {
        return Err(EncryptionError::CekDecryptionFailed(
            "Encrypted CEK too short".into(),
        ));
    }

    if data[0] != CEK_VERSION_BYTE {
        return Err(EncryptionError::CekDecryptionFailed(format!(
            "Invalid CEK version: expected {:#04x}, got {:#04x}",
            CEK_VERSION_BYTE, data[0]
        )));
    }

    let key_path_len = u16::from_le_bytes([data[1], data[2]]) as usize;
    let ciphertext_len = u16::from_le_bytes([data[3], data[4]]) as usize;

    let ciphertext_offset = 5 + key_path_len;
    let signature_offset = ciphertext_offset + ciphertext_len;
    if data.len() <= signature_offset {
        return Err(EncryptionError::CekDecryptionFailed(format!(
            "Encrypted CEK truncated: {} bytes, need key path ({key_path_len}) + \
             ciphertext ({ciphertext_len}) + signature after a 5-byte header",
            data.len()
        )));
    }

    Ok(CekEnvelope {
        ciphertext: &data[ciphertext_offset..signature_offset],
        signature: &data[signature_offset..],
        signed_portion: &data[..signature_offset],
    })
}

/// Build the signed portion of an envelope (everything before the signature).
///
/// The complete envelope is this followed by an RSA-PKCS1 signature over its
/// SHA-256 digest, produced with the Column Master Key.
pub fn build_signed_portion(key_path: &str, ciphertext: &[u8]) -> Vec<u8> {
    let key_path_utf16: Vec<u8> = key_path
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();

    let mut out = Vec::with_capacity(5 + key_path_utf16.len() + ciphertext.len());
    out.push(CEK_VERSION_BYTE);
    out.extend_from_slice(&(key_path_utf16.len() as u16).to_le_bytes());
    out.extend_from_slice(&(ciphertext.len() as u16).to_le_bytes());
    out.extend_from_slice(&key_path_utf16);
    out.extend_from_slice(ciphertext);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample_envelope() -> Vec<u8> {
        let mut envelope = build_signed_portion("Test", &[0xAB; 256]);
        envelope.extend_from_slice(&[0xCD; 256]); // fake signature
        envelope
    }

    #[test]
    fn parse_extracts_fields() {
        let envelope = sample_envelope();
        let parsed = parse(&envelope).unwrap();
        assert_eq!(parsed.ciphertext, &[0xAB; 256][..]);
        assert_eq!(parsed.signature, &[0xCD; 256][..]);
        assert_eq!(parsed.signed_portion.len(), 5 + 8 + 256);
        assert_eq!(parsed.signed_portion, &envelope[..envelope.len() - 256]);
    }

    #[test]
    fn layout_puts_both_lengths_before_payloads() {
        let portion = build_signed_portion("Test", &[0xAB; 256]);
        assert_eq!(portion[0], 0x01);
        assert_eq!(u16::from_le_bytes([portion[1], portion[2]]), 8); // "Test" UTF-16LE
        assert_eq!(u16::from_le_bytes([portion[3], portion[4]]), 256);
        assert_eq!(&portion[5..7], &[b'T', 0x00]);
    }

    #[test]
    fn parse_rejects_bad_version() {
        let mut envelope = sample_envelope();
        envelope[0] = 0x02;
        let err = parse(&envelope).unwrap_err();
        assert!(err.to_string().contains("Invalid CEK version"));
    }

    #[test]
    fn parse_rejects_missing_signature() {
        let portion = build_signed_portion("Test", &[0xAB; 256]);
        assert!(parse(&portion).is_err()); // nothing after ciphertext
    }

    #[test]
    fn parse_rejects_truncation() {
        assert!(parse(&[0x01, 0x00]).is_err());
        let envelope = sample_envelope();
        assert!(parse(&envelope[..envelope.len() - 300]).is_err());
    }

    #[test]
    fn signed_digest_is_sha256_of_signed_portion() {
        let envelope = sample_envelope();
        let parsed = parse(&envelope).unwrap();
        let expected: [u8; 32] = Sha256::digest(&envelope[..envelope.len() - 256]).into();
        assert_eq!(parsed.signed_digest(), expected);
    }
}
