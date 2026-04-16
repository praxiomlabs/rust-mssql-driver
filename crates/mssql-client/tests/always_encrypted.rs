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
//
// ## Scope note
//
// Populating encrypted columns with ciphertext requires the driver to send
// encrypted RPC parameters (parameter encryption). That path is NOT yet
// implemented — see `docs/ALWAYS_ENCRYPTED.md § Limitations`. A naïve
// `INSERT ... VALUES (CONVERT(varbinary(max), 0x...))` is rejected by
// SQL Server with "Operand type clash: varbinary is incompatible with
// varbinary(N) encrypted with (...)" because the server strictly separates
// plain varbinary from its encrypted-column type, even for literals.
//
// Until parameter encryption lands, these live tests exercise the reachable
// slice of the pipeline end-to-end against a real server:
//
//   1. Generate an RSA-2048 keypair in the test.
//   2. Wrap a random 32-byte CEK with RSA-OAEP-SHA256 and build the
//      SQL Server envelope consumed by `RsaKeyUnwrapper::parse_encrypted_cek`:
//      `0x01 || u16le(path_len) || utf16le(path) || u16le(cipher_len) || cipher`.
//   3. `CREATE COLUMN MASTER KEY`, `CREATE COLUMN ENCRYPTION KEY`, and a
//      table with at least one encrypted column.
//   4. Populate rows through the only server-allowed path without parameter
//      encryption: INSERT leaving the encrypted column NULL.
//   5. Connect WITH `Column Encryption Setting=Enabled` + registered
//      `InMemoryKeyStore`, SELECT the row, and assert:
//       - the driver parses `CekTable` and per-column `CryptoMetadata`
//       - the driver async-resolves the encryptor via the key-store provider
//         (this is the path the `from_arc` bug in `EncryptionContext` broke —
//          see commit history for the fix)
//       - NULL encrypted values are surfaced as `SqlValue::Null`
//       - the plaintext base-type re-parse machinery is wired up (verified
//         by reading an unencrypted companion column from the same row)
//
// The full ciphertext round-trip through RPC parameters is tracked as a
// separate work item and will be added here once parameter encryption is
// implemented. See item 2.8 in .tmp/work-items.md.

#[cfg(feature = "always-encrypted")]
mod live_server {
    use mssql_auth::{AeadEncryptor, EncryptionType, InMemoryKeyStore};
    use mssql_client::{Client, Config, EncryptionConfig};
    use rand::RngCore;
    use rsa::{Oaep, RsaPrivateKey, pkcs8::EncodePrivateKey};
    use sha2::Sha256;

    /// Key path we register both in SQL Server's CMK metadata and in our
    /// `InMemoryKeyStore`. The value is arbitrary — SQL Server treats it
    /// as an opaque string; only the client looks it up.
    const KEY_PATH: &str = "rust-mssql-driver-test-key";

    /// Value we use for `KEY_STORE_PROVIDER_NAME`. Must match
    /// `InMemoryKeyStore::provider_name()` exactly.
    const PROVIDER_NAME: &str = "IN_MEMORY_KEY_STORE";

    /// Bundle everything a test needs after per-test setup completes.
    struct Fixture {
        /// The randomly-generated CEK. Unused today — parameter encryption
        /// (NYI) will pre-encrypt row values with it so INSERT tests can
        /// round-trip ciphertext. Kept on the fixture so the helpers remain
        /// wired once that feature lands.
        #[allow(dead_code)]
        cek_bytes: [u8; 32],
        /// CMK name in SQL Server (random per test to avoid collisions).
        cmk_name: String,
        /// CEK name in SQL Server.
        cek_name: String,
        /// Table name in SQL Server.
        table_name: String,
    }

    /// Encrypt `plaintext` with AEAD using the test CEK. Reserved for the
    /// ciphertext-roundtrip tests that land with parameter encryption.
    #[allow(dead_code)]
    fn aead_encrypt(cek: &[u8; 32], plaintext: &[u8], deterministic: bool) -> Vec<u8> {
        let enc = AeadEncryptor::new(cek).expect("AeadEncryptor::new");
        let mode = if deterministic {
            EncryptionType::Deterministic
        } else {
            EncryptionType::Randomized
        };
        enc.encrypt(plaintext, mode).expect("aead encrypt")
    }

    /// Convert bytes to an uppercase `0x...` hex literal for inline T-SQL use.
    #[allow(dead_code)]
    fn hex_literal(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(2 + bytes.len() * 2);
        s.push_str("0x");
        for b in bytes {
            s.push_str(&format!("{b:02X}"));
        }
        s
    }

    /// Build a SQL Server CEK envelope in the format `RsaKeyUnwrapper::parse_encrypted_cek`
    /// expects: `0x01 || u16_le(path_len) || utf16le(path) || u16_le(cipher_len) || cipher`.
    ///
    /// `encrypted_cek` is the raw RSA-OAEP output.
    fn build_cek_envelope(key_path: &str, encrypted_cek: &[u8]) -> Vec<u8> {
        let path_utf16: Vec<u8> = key_path
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let mut out = Vec::with_capacity(1 + 2 + path_utf16.len() + 2 + encrypted_cek.len());
        out.push(0x01);
        out.extend_from_slice(&(path_utf16.len() as u16).to_le_bytes());
        out.extend_from_slice(&path_utf16);
        out.extend_from_slice(&(encrypted_cek.len() as u16).to_le_bytes());
        out.extend_from_slice(encrypted_cek);
        out
    }

    /// Build an admin Config (no column encryption — used for DDL setup/teardown).
    fn admin_config() -> Option<Config> {
        let host = std::env::var("MSSQL_HOST").ok()?;
        let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
        let password =
            std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
        let conn_str = format!(
            "Server={host};Database=master;User Id={user};Password={password};\
             TrustServerCertificate=true;Encrypt=true"
        );
        Config::from_connection_string(&conn_str).ok()
    }

    /// Build a Config with Always Encrypted enabled and `InMemoryKeyStore`
    /// pre-loaded with the caller-supplied PEM.
    fn encrypted_config(private_key_pem: &str) -> Option<Config> {
        let host = std::env::var("MSSQL_HOST").ok()?;
        let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
        let password =
            std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
        let conn_str = format!(
            "Server={host};Database=master;User Id={user};Password={password};\
             TrustServerCertificate=true;Encrypt=true"
        );
        let mut key_store = InMemoryKeyStore::new();
        key_store
            .add_key(KEY_PATH, private_key_pem)
            .expect("add_key");
        let enc = EncryptionConfig::new().with_provider(key_store);
        Config::from_connection_string(&conn_str)
            .ok()
            .map(|c| c.with_column_encryption(enc))
    }

    /// Seed per-test unique names to avoid collisions when the suite runs in
    /// parallel or re-runs after failure (the Docker server is shared).
    fn unique_suffix() -> String {
        let mut rng = rand::thread_rng();
        format!("{:08x}", rng.next_u32())
    }

    /// Create the CMK, CEK, and table. The caller must run teardown at the end.
    async fn setup(
        admin: &mut Client<mssql_client::Ready>,
        cek_bytes: &[u8; 32],
        rsa_private: &RsaPrivateKey,
        table_ddl: &str,
    ) -> Fixture {
        let suffix = unique_suffix();
        let cmk_name = format!("AE_TestCMK_{suffix}");
        let cek_name = format!("AE_TestCEK_{suffix}");
        let table_name = format!("AE_TestTbl_{suffix}");

        // Wrap the CEK with RSA-OAEP-SHA256 using the public half.
        let pub_key = rsa_private.to_public_key();
        let mut rng = rand::thread_rng();
        let padding = Oaep::new::<Sha256>();
        let rsa_ciphertext = pub_key
            .encrypt(&mut rng, padding, cek_bytes)
            .expect("rsa encrypt");
        let envelope = build_cek_envelope(KEY_PATH, &rsa_ciphertext);
        let envelope_hex = hex_literal(&envelope);

        // CREATE COLUMN MASTER KEY
        let cmk_sql = format!(
            "CREATE COLUMN MASTER KEY [{cmk_name}] WITH ( \
             KEY_STORE_PROVIDER_NAME = '{PROVIDER_NAME}', \
             KEY_PATH = '{KEY_PATH}' )"
        );
        admin.execute(&cmk_sql, &[]).await.expect("create cmk");

        // CREATE COLUMN ENCRYPTION KEY
        let cek_sql = format!(
            "CREATE COLUMN ENCRYPTION KEY [{cek_name}] WITH VALUES ( \
             COLUMN_MASTER_KEY = [{cmk_name}], \
             ALGORITHM = 'RSA_OAEP', \
             ENCRYPTED_VALUE = {envelope_hex} )"
        );
        admin.execute(&cek_sql, &[]).await.expect("create cek");

        // CREATE TABLE
        let tbl_sql = table_ddl
            .replace("{TABLE}", &table_name)
            .replace("{CEK}", &cek_name);
        admin.execute(&tbl_sql, &[]).await.expect("create table");

        Fixture {
            cek_bytes: *cek_bytes,
            cmk_name,
            cek_name,
            table_name,
        }
    }

    /// Drop everything we created, ignoring errors so teardown is idempotent.
    async fn teardown(admin: &mut Client<mssql_client::Ready>, fx: &Fixture) {
        let _ = admin
            .execute(&format!("DROP TABLE IF EXISTS [{}]", fx.table_name), &[])
            .await;
        let _ = admin
            .execute(
                &format!("DROP COLUMN ENCRYPTION KEY IF EXISTS [{}]", fx.cek_name),
                &[],
            )
            .await;
        let _ = admin
            .execute(
                &format!("DROP COLUMN MASTER KEY IF EXISTS [{}]", fx.cmk_name),
                &[],
            )
            .await;
    }

    /// Generate an RSA-2048 keypair and its PEM encoding.
    fn fresh_rsa_keypair() -> (RsaPrivateKey, String) {
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).expect("rsa keygen");
        let pem = key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .expect("pem encode")
            .to_string();
        (key, pem)
    }

    /// Generate a random 32-byte CEK (the AES-256 key that AEAD uses).
    fn fresh_cek() -> [u8; 32] {
        let mut cek = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut cek);
        cek
    }

    // -------------------------------------------------------------------------
    // Tests
    // -------------------------------------------------------------------------

    /// Validate the Always Encrypted metadata path end-to-end against a live
    /// server. A row with NULL in the encrypted column is the only value we
    /// can populate without parameter encryption (any non-NULL plaintext
    /// literal is rejected with "Operand type clash" at INSERT time — see
    /// the scope note at the top of this section).
    ///
    /// Exercised here:
    ///   * LOGIN7 FeatureId::ColumnEncryption feature-extension round-trip.
    ///   * Server's CekTable parsing out of `ColMetaData`.
    ///   * Per-column `CryptoMetadata` parsing.
    ///   * `ColumnDecryptor::from_metadata` async resolution path (this is
    ///     where the `from_arc` provider-loss bug manifested — the context
    ///     would have an empty providers map and resolution would fail with
    ///     `KeyStoreNotFound`).
    ///   * NULL-in-encrypted-column handling in the column parser (returns
    ///     `SqlValue::Null` without attempting to decrypt).
    ///   * Plaintext base-type re-parse machinery (via the companion
    ///     unencrypted column `Description`).
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_always_encrypted_metadata_and_null_roundtrip() {
        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");

        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();

        let fx = setup(
            &mut admin,
            &cek,
            &rsa_key,
            "CREATE TABLE [{TABLE}] ( \
             Id INT NOT NULL PRIMARY KEY, \
             Description NVARCHAR(64) NOT NULL, \
             SSN NVARCHAR(32) COLLATE Latin1_General_BIN2 ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL )",
        )
        .await;

        // NULL is the only value we can populate through the current wire
        // path; parameter encryption is required for anything else.
        admin
            .execute(
                &format!(
                    "INSERT INTO [{}] (Id, Description, SSN) VALUES (1, N'row one', NULL)",
                    fx.table_name
                ),
                &[],
            )
            .await
            .expect("insert nulls");
        admin
            .execute(
                &format!(
                    "INSERT INTO [{}] (Id, Description, SSN) VALUES (2, N'row two', NULL)",
                    fx.table_name
                ),
                &[],
            )
            .await
            .expect("insert nulls 2");

        // Reconnect with AE enabled + provider registered. The reconnect
        // exercises the `from_arc` bug-fix path: Config gets cloned inside
        // `Client::connect` for retry/redirect handling, and the encryption
        // context must still see our `InMemoryKeyStore` after all those
        // clones. If the bug regressed, `get_encryptor` would fail with
        // `KeyStoreNotFound` on the first row because the CekTable entry
        // references the `IN_MEMORY_KEY_STORE` provider name.
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let rows = client
            .query(
                &format!(
                    "SELECT Id, Description, SSN FROM [{}] ORDER BY Id",
                    fx.table_name
                ),
                &[],
            )
            .await
            .expect("select rows");

        let mut got: Vec<(i32, String, Option<String>)> = Vec::new();
        for row_result in rows {
            let row = row_result.expect("row");
            let id: i32 = row.get(0).expect("id");
            let desc: String = row.get(1).expect("description");
            let ssn: Option<String> = row.get(2).expect("ssn (NULL-able decrypt)");
            got.push((id, desc, ssn));
        }

        // Teardown before assert so a later failure doesn't leak state.
        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(
            got,
            vec![
                (1, "row one".to_string(), None),
                (2, "row two".to_string(), None),
            ],
            "metadata round-trip + NULL decryption path must work"
        );
    }

    /// Verify the `EncryptionContext::from_arc` bug fix: when `Config` is
    /// cloned (as `Client::connect` does internally for retry handling), the
    /// `InMemoryKeyStore` providers must remain visible to the context.
    ///
    /// This is a direct in-process assertion that does not require the
    /// encrypted-table machinery — it probes the exact spot where the bug
    /// lived. Kept `#[ignore]` because it still builds a real `Client`
    /// against a server (which is where the Arc clone actually happens).
    #[tokio::test]
    #[ignore = "Requires SQL Server"]
    async fn test_encryption_context_keeps_providers_after_config_clone() {
        let (_rsa_key, pem) = fresh_rsa_keypair();
        let cfg = encrypted_config(&pem).expect("host/user/password env vars");

        // Cloning the Config the same way Client::connect does internally
        // was previously enough to strand providers — `try_unwrap` on the
        // inner Arc would fail and fall back to an empty provider map.
        let _clone_1 = cfg.clone();
        let _clone_2 = cfg.clone();

        // After three live Arc references, connect and invoke any read that
        // goes through the encryption context. For this we don't need an
        // encrypted table — the assertion is that the context reports the
        // provider as registered.
        let client = Client::connect(cfg).await.expect("ae connect");
        assert!(
            client.has_encryption_provider("IN_MEMORY_KEY_STORE"),
            "providers must survive Config clones (from_arc bug regression)"
        );
    }
}
