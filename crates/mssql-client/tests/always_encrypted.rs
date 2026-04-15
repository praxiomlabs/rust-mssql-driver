//! Always Encrypted integration tests.
//!
//! These tests verify the Always Encrypted cryptographic infrastructure.
//! Most tests run without a SQL Server connection, testing the crypto primitives.
//!
//! For live server tests with actual encrypted columns:
//!
//! ```bash
//! # Set connection details via environment variables
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourPassword
//!
//! # Run Always Encrypted tests
//! cargo test -p mssql-client --test always_encrypted --features always-encrypted -- --ignored
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use bytes::Bytes;
use tds_protocol::crypto::{CekTable, CekTableEntry, CekValue, CryptoMetadata, EncryptionTypeWire};

// =============================================================================
// Unit Tests for Crypto Infrastructure (no SQL Server required)
// =============================================================================

#[test]
fn test_cek_table_construction() {
    let mut table = CekTable::new();
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);

    let entry = CekTableEntry {
        database_id: 1,
        cek_id: 1,
        cek_version: 1,
        cek_md_version: 100,
        values: vec![CekValue {
            encrypted_value: Bytes::from_static(&[0xDE, 0xAD, 0xBE, 0xEF]),
            key_store_provider_name: "TEST_PROVIDER".to_string(),
            cmk_path: "/test/key/path".to_string(),
            encryption_algorithm: "RSA_OAEP".to_string(),
        }],
    };

    table.entries.push(entry);
    assert!(!table.is_empty());
    assert_eq!(table.len(), 1);

    let retrieved = table.get(0).unwrap();
    assert_eq!(retrieved.database_id, 1);
    assert_eq!(retrieved.cek_id, 1);
    assert_eq!(retrieved.cek_version, 1);
}

#[test]
fn test_cek_entry_primary_value() {
    let entry = CekTableEntry {
        database_id: 1,
        cek_id: 1,
        cek_version: 1,
        cek_md_version: 100,
        values: vec![
            CekValue {
                encrypted_value: Bytes::from_static(&[0x01]),
                key_store_provider_name: "PRIMARY".to_string(),
                cmk_path: "/primary".to_string(),
                encryption_algorithm: "RSA_OAEP".to_string(),
            },
            CekValue {
                encrypted_value: Bytes::from_static(&[0x02]),
                key_store_provider_name: "SECONDARY".to_string(),
                cmk_path: "/secondary".to_string(),
                encryption_algorithm: "RSA_OAEP".to_string(),
            },
        ],
    };

    let primary = entry.primary_value().unwrap();
    assert_eq!(primary.key_store_provider_name, "PRIMARY");
}

#[test]
fn test_crypto_metadata_methods() {
    let meta = CryptoMetadata {
        cek_table_ordinal: 0,
        base_user_type: 0,
        base_col_type: 0x26,
        base_type_info: tds_protocol::token::TypeInfo::default(),
        algorithm_id: 2, // AEAD_AES_256_CBC_HMAC_SHA256
        encryption_type: EncryptionTypeWire::Deterministic,
        normalization_version: 1,
    };

    assert!(meta.is_aead_aes_256());
    assert!(meta.is_deterministic());
    assert!(!meta.is_randomized());

    let meta_random = CryptoMetadata {
        cek_table_ordinal: 1,
        base_user_type: 0,
        base_col_type: 0x26,
        base_type_info: tds_protocol::token::TypeInfo::default(),
        algorithm_id: 2,
        encryption_type: EncryptionTypeWire::Randomized,
        normalization_version: 1,
    };

    assert!(meta_random.is_randomized());
    assert!(!meta_random.is_deterministic());
}

#[test]
fn test_encryption_type_wire_conversion() {
    assert_eq!(EncryptionTypeWire::Deterministic.to_u8(), 1);
    assert_eq!(EncryptionTypeWire::Randomized.to_u8(), 2);

    assert_eq!(
        EncryptionTypeWire::from_u8(1),
        Some(EncryptionTypeWire::Deterministic)
    );
    assert_eq!(
        EncryptionTypeWire::from_u8(2),
        Some(EncryptionTypeWire::Randomized)
    );
    assert_eq!(EncryptionTypeWire::from_u8(0), None);
    assert_eq!(EncryptionTypeWire::from_u8(99), None);
}

// =============================================================================
// Tests for mssql-client encryption types
// =============================================================================

use mssql_client::{
    EncryptionConfig, ParameterCryptoInfo, ParameterEncryptionInfo, ResultSetEncryptionInfo,
};

#[test]
fn test_encryption_config_builder() {
    let config = EncryptionConfig::new().with_cek_caching(false);

    assert!(config.enabled);
    assert!(!config.cache_ceks);
    assert!(!config.is_ready()); // No providers registered
}

#[test]
fn test_result_set_encryption_info_column_tracking() {
    let cek_table = CekTable::new();
    let mut info = ResultSetEncryptionInfo::new(cek_table, 5);

    // Initially no columns are encrypted
    for i in 0..5 {
        assert!(!info.is_column_encrypted(i));
        assert!(info.get_encryption_type(i).is_none());
    }

    // Mark column 2 as encrypted
    let metadata = CryptoMetadata {
        cek_table_ordinal: 0,
        base_user_type: 0,
        base_col_type: 0x26,
        base_type_info: tds_protocol::token::TypeInfo::default(),
        algorithm_id: 2,
        encryption_type: EncryptionTypeWire::Deterministic,
        normalization_version: 1,
    };
    info.set_column_crypto(2, metadata);

    assert!(!info.is_column_encrypted(0));
    assert!(!info.is_column_encrypted(1));
    assert!(info.is_column_encrypted(2));
    assert!(!info.is_column_encrypted(3));
    assert!(!info.is_column_encrypted(4));

    assert_eq!(
        info.get_encryption_type(2),
        Some(EncryptionTypeWire::Deterministic)
    );
}

#[test]
fn test_parameter_encryption_info_tracking() {
    let mut info = ParameterEncryptionInfo::new();

    assert!(!info.needs_encryption("@SSN"));
    assert!(!info.needs_encryption("@Name"));

    let ssn_crypto = ParameterCryptoInfo::new(0, EncryptionTypeWire::Deterministic, 2, 1, 1);
    info.add_parameter("@SSN".to_string(), ssn_crypto);

    assert!(info.needs_encryption("@SSN"));
    assert!(!info.needs_encryption("@Name"));

    let param = info.get_parameter("@SSN").unwrap();
    assert_eq!(param.cek_ordinal, 0);
    assert_eq!(param.encryption_type, EncryptionTypeWire::Deterministic);
}

// =============================================================================
// AEAD Encryption Tests (requires always-encrypted feature)
// =============================================================================

#[cfg(feature = "always-encrypted")]
mod aead_tests {
    use mssql_auth::{AeadEncryptor, EncryptionType};

    #[test]
    fn test_aead_encrypt_decrypt_roundtrip() {
        // 32-byte key for AES-256
        let cek = [0x42u8; 32];
        let encryptor = AeadEncryptor::new(&cek).expect("Failed to create encryptor");

        let plaintext = b"Hello, Always Encrypted!";

        // Test deterministic encryption
        let ciphertext = encryptor
            .encrypt(plaintext, EncryptionType::Deterministic)
            .expect("Encryption failed");

        let decrypted = encryptor.decrypt(&ciphertext).expect("Decryption failed");

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_aead_deterministic_consistency() {
        let cek = [0x55u8; 32];
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Consistent value";

        let ct1 = encryptor
            .encrypt(plaintext, EncryptionType::Deterministic)
            .unwrap();
        let ct2 = encryptor
            .encrypt(plaintext, EncryptionType::Deterministic)
            .unwrap();

        // Deterministic encryption produces same ciphertext
        assert_eq!(ct1, ct2);
    }

    #[test]
    fn test_aead_randomized_uniqueness() {
        let cek = [0x66u8; 32];
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Random value";

        let ct1 = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();
        let ct2 = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        // Randomized encryption produces different ciphertext
        assert_ne!(ct1, ct2);

        // Both decrypt to same plaintext
        let pt1 = encryptor.decrypt(&ct1).unwrap();
        let pt2 = encryptor.decrypt(&ct2).unwrap();
        assert_eq!(pt1, pt2);
        assert_eq!(pt1, plaintext);
    }

    #[test]
    fn test_aead_tamper_detection() {
        let cek = [0x77u8; 32];
        let encryptor = AeadEncryptor::new(&cek).unwrap();

        let plaintext = b"Sensitive data";
        let mut ciphertext = encryptor
            .encrypt(plaintext, EncryptionType::Randomized)
            .unwrap();

        // Tamper with the ciphertext (flip a bit in the MAC)
        if ciphertext.len() > 10 {
            ciphertext[5] ^= 0x01;
        }

        let result = encryptor.decrypt(&ciphertext);
        assert!(
            result.is_err(),
            "Tampered ciphertext should fail to decrypt"
        );
    }

    #[test]
    fn test_aead_invalid_key_size() {
        let short_key = [0x42u8; 16]; // Should be 32 bytes
        let result = AeadEncryptor::new(&short_key);
        assert!(result.is_err(), "Should reject non-32-byte keys");
    }
}

// =============================================================================
// RSA Key Unwrapping Tests (requires always-encrypted feature)
// =============================================================================

#[cfg(feature = "always-encrypted")]
mod key_unwrap_tests {
    use mssql_auth::RsaKeyUnwrapper;
    use rsa::{Oaep, RsaPrivateKey, pkcs8::EncodePrivateKey};
    use sha2::Sha256;

    fn generate_test_key() -> RsaPrivateKey {
        let mut rng = rand::thread_rng();
        RsaPrivateKey::new(&mut rng, 2048).unwrap()
    }

    #[test]
    fn test_rsa_key_unwrap_roundtrip() {
        let key = generate_test_key();
        let pem = key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let unwrapper = RsaKeyUnwrapper::from_pem(&pem).expect("Failed to create unwrapper");

        // Encrypt a test CEK
        let test_cek = [0x42u8; 32];
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha256>();
        let mut rng = rand::thread_rng();
        let ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Decrypt and verify
        let decrypted = unwrapper.decrypt_raw(&ciphertext).unwrap();
        assert_eq!(&decrypted[..], &test_cek[..]);
    }

    #[test]
    fn test_rsa_key_bits() {
        let key = generate_test_key();
        let pem = key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let unwrapper = RsaKeyUnwrapper::from_pem(&pem).unwrap();

        assert_eq!(unwrapper.key_bits(), 2048);
    }
}

// =============================================================================
// Key Store Tests (requires always-encrypted feature)
// =============================================================================

#[cfg(feature = "always-encrypted")]
mod key_store_tests {
    use mssql_auth::{CekCache, CekCacheKey, InMemoryKeyStore, KeyStoreProvider};
    use rsa::{Oaep, RsaPrivateKey, pkcs8::EncodePrivateKey};
    use sha2::Sha256;
    use std::time::Duration;

    fn generate_test_key_pem() -> String {
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_in_memory_key_store_basic() {
        let mut store = InMemoryKeyStore::new();
        assert!(store.is_empty());

        let pem = generate_test_key_pem();
        store.add_key("TestKey", &pem).unwrap();

        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
        assert!(store.has_key("TestKey"));
        assert!(!store.has_key("OtherKey"));
    }

    #[test]
    fn test_in_memory_key_store_provider_name() {
        let store = InMemoryKeyStore::new();
        assert_eq!(store.provider_name(), "IN_MEMORY_KEY_STORE");
    }

    #[tokio::test]
    async fn test_in_memory_key_store_decrypt_cek() {
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pem = key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string();

        let mut store = InMemoryKeyStore::new();
        store.add_key("TestKey", &pem).unwrap();

        // Create encrypted CEK in SQL Server format
        let test_cek = [0x55u8; 32];
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha256>();
        let ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        // Create SQL Server format envelope
        let key_path = "TestKey";
        let key_path_utf16: Vec<u8> = key_path
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();

        let mut envelope = vec![0x01]; // version
        envelope.extend_from_slice(&(key_path_utf16.len() as u16).to_le_bytes());
        envelope.extend_from_slice(&key_path_utf16);
        envelope.extend_from_slice(&(ciphertext.len() as u16).to_le_bytes());
        envelope.extend_from_slice(&ciphertext);

        // Decrypt
        let decrypted = store
            .decrypt_cek("TestKey", "RSA_OAEP", &envelope)
            .await
            .expect("Decryption failed");

        assert_eq!(&decrypted[..], &test_cek[..]);
    }

    #[test]
    fn test_cek_cache_basic() {
        let cache = CekCache::new();
        assert!(cache.is_empty());

        let key = CekCacheKey::new(1, 1, 1);
        let cek = vec![0x42u8; 32];

        let encryptor = cache.insert(key.clone(), cek).unwrap();
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get(&key).unwrap();
        assert!(std::sync::Arc::ptr_eq(&encryptor, &retrieved));
    }

    #[test]
    fn test_cek_cache_expiration() {
        let cache = CekCache::with_ttl(Duration::from_millis(10));
        let key = CekCacheKey::new(1, 1, 1);
        let cek = vec![0x42u8; 32];

        cache.insert(key.clone(), cek).unwrap();
        assert!(cache.get(&key).is_some());

        std::thread::sleep(Duration::from_millis(20));
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cek_cache_remove() {
        let cache = CekCache::new();
        let key = CekCacheKey::new(1, 1, 1);
        let cek = vec![0x42u8; 32];

        cache.insert(key.clone(), cek).unwrap();
        assert!(cache.remove(&key));
        assert!(cache.get(&key).is_none());
        assert!(!cache.remove(&key)); // Second remove returns false
    }
}

// =============================================================================
// Live Server Tests (require SQL Server with Always Encrypted configured)
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server with Always Encrypted"]
async fn test_always_encrypted_query() {
    // This test requires:
    // 1. SQL Server with Always Encrypted enabled
    // 2. A table with encrypted columns
    // 3. CMK configured in a supported key store
    //
    // Example setup SQL:
    // CREATE COLUMN MASTER KEY [TestCMK]
    // WITH (KEY_STORE_PROVIDER_NAME = 'MSSQL_CERTIFICATE_STORE',
    //       KEY_PATH = 'CurrentUser/My/TestCertificate');
    //
    // CREATE COLUMN ENCRYPTION KEY [TestCEK]
    // WITH VALUES (COLUMN_MASTER_KEY = [TestCMK],
    //              ALGORITHM = 'RSA_OAEP');
    //
    // CREATE TABLE EncryptedTest (
    //     Id INT PRIMARY KEY,
    //     SSN NVARCHAR(11) ENCRYPTED WITH (
    //         COLUMN_ENCRYPTION_KEY = [TestCEK],
    //         ENCRYPTION_TYPE = DETERMINISTIC,
    //         ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256'
    //     )
    // );

    // Test would connect and query encrypted data
    // let config = get_test_config().expect("SQL Server config required");
    // let client = Client::connect(config).await.expect("Failed to connect");
    // ...
}

#[tokio::test]
#[ignore = "Requires SQL Server with Always Encrypted"]
async fn test_always_encrypted_insert() {
    // Test inserting data into encrypted columns
    // This verifies parameter encryption works correctly
}

#[tokio::test]
#[ignore = "Requires SQL Server with Always Encrypted"]
async fn test_always_encrypted_comparison() {
    // Test deterministic encryption supports equality comparisons
    // WHERE SSN = @SSN should work when @SSN is encrypted the same way
}
