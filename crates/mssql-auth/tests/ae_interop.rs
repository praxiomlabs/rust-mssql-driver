//! Cross-implementation interop fixtures for AEAD_AES_256_CBC_HMAC_SHA256.
//!
//! The fixture blob below was produced by `generate_ae_fixtures.py`, an
//! independent implementation of the Always Encrypted cell-encryption
//! algorithm transcribed from the .NET reference
//! (dotnet/SqlClient `SqlAeadAes256CbcHmac256{EncryptionKey,Algorithm}.cs`).
//! Decrypting it proves key derivation, MAC layout, IV derivation, and the
//! cell-blob format all match the spec, i.e. that this driver interoperates
//! with data encrypted by .NET/SSMS/JDBC. Self-roundtrip tests cannot catch
//! a derivation that is internally consistent but non-conformant.
//!
//! The blob was additionally cross-checked out-of-band against Microsoft's
//! shipped binary — Microsoft.Data.SqlClient 5.2.2
//! (`SqlAeadAes256CbcHmac256Algorithm`, invoked via reflection) produced the
//! same bytes for this CEK/plaintext and decrypted it back. That harness is not
//! committed, so the in-repo guarantee rests on the transcription above; the
//! reflection check is a manual, out-of-band confirmation. End-to-end
//! validation against a live server (the wire-metadata path) is tracked in
//! issue #87.

#![cfg(feature = "always-encrypted")]
#![allow(clippy::expect_used)]

use mssql_auth::AeadEncryptor;
use mssql_auth::encryption::EncryptionType;

/// Matches `test_cek()` in `src/aead.rs` and `CEK` in the generator script.
const CEK: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

const PLAINTEXT: &[u8] = b"Hello, SQL Server Always Encrypted!";

/// Deterministic encryption of PLAINTEXT under CEK, per the spec.
const SPEC_DETERMINISTIC_BLOB: [u8; 97] = [
    0x01, 0x89, 0x53, 0x43, 0x28, 0xff, 0x31, 0x74, 0xba, 0x3d, 0x9a, 0x8b, 0x5c, 0x05, 0x62, 0x48,
    0x73, 0x35, 0xed, 0xca, 0x1e, 0x45, 0x26, 0x9d, 0x65, 0x74, 0xa3, 0x30, 0x53, 0xab, 0x5f, 0x89,
    0x5d, 0x98, 0x05, 0xdb, 0xec, 0x33, 0x62, 0x2f, 0x02, 0x1c, 0xcc, 0xe7, 0xe4, 0x26, 0x71, 0x1e,
    0xa9, 0x0e, 0x0e, 0x2c, 0x8d, 0x78, 0x9a, 0xda, 0xe8, 0x1e, 0xf4, 0xde, 0x18, 0x59, 0x6f, 0x66,
    0x6a, 0x80, 0x7e, 0xdd, 0x67, 0x4d, 0xd0, 0x1b, 0x45, 0x17, 0xeb, 0x8e, 0xcb, 0xde, 0x74, 0x60,
    0xe2, 0xa4, 0x21, 0xbd, 0x3e, 0xfc, 0x83, 0x08, 0xfa, 0x70, 0x50, 0x99, 0x29, 0x08, 0xb8, 0x3d,
    0x06,
];

#[test]
fn decrypts_externally_encrypted_blob() {
    let encryptor = AeadEncryptor::new(&CEK).expect("CEK is 32 bytes");
    let plaintext = encryptor
        .decrypt(&SPEC_DETERMINISTIC_BLOB)
        .expect("spec-conformant blob must decrypt (MAC must verify)");
    assert_eq!(plaintext, PLAINTEXT);
}

#[test]
fn deterministic_encryption_matches_spec_blob() {
    let encryptor = AeadEncryptor::new(&CEK).expect("CEK is 32 bytes");
    let blob = encryptor
        .encrypt(PLAINTEXT, EncryptionType::Deterministic)
        .expect("encryption succeeds");
    assert_eq!(
        blob,
        SPEC_DETERMINISTIC_BLOB.to_vec(),
        "deterministic ciphertext must be byte-identical to the spec fixture"
    );
}
