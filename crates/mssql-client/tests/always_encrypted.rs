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

use mssql_client::EncryptionConfig;

#[test]
fn test_encryption_config_builder() {
    let config = EncryptionConfig::new().with_cek_caching(false);

    assert!(config.enabled);
    assert!(!config.cache_ceks);
    assert!(!config.is_ready()); // No providers registered
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
    use sha1::Sha1;

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
        let padding = Oaep::new::<Sha1>();
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
    use sha1::Sha1;
    use sha2::{Digest, Sha256};
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

        // Create encrypted CEK in the canonical signed envelope format
        let test_cek = [0x55u8; 32];
        let public_key = key.to_public_key();
        let padding = Oaep::new::<Sha1>();
        let ciphertext = public_key.encrypt(&mut rng, padding, &test_cek).unwrap();

        let mut envelope = mssql_auth::cek_envelope::build_signed_portion("TestKey", &ciphertext);
        let digest: [u8; 32] = Sha256::digest(&envelope).into();
        let signature = key
            .sign(rsa::Pkcs1v15Sign::new::<Sha256>(), &digest)
            .unwrap();
        envelope.extend_from_slice(&signature);

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
// Parameter (write) encryption is implemented for int/nvarchar/varbinary, so
// `test_parameter_encryption_round_trip` populates encrypted columns through the
// driver (encrypt on INSERT, decrypt on SELECT) and confirms the stored value is
// opaque ciphertext to a non-AE connection.
//
// `test_always_encrypted_metadata_and_null_roundtrip` remains a focused check of
// the metadata + NULL path: it inserts NULL into the encrypted column (a naïve
// plaintext literal is still rejected with "Operand type clash: varbinary is
// incompatible with varbinary(N) encrypted with (...)") and asserts:
//   - the driver parses `CekTable` and per-column `CryptoMetadata`,
//   - the encryptor is async-resolved via the key-store provider (the path the
//     `from_arc` bug in `EncryptionContext` broke — see commit history),
//   - NULL encrypted values surface as `SqlValue::Null`,
//   - the plaintext base-type re-parse machinery is wired (via an unencrypted
//     companion column).
//
// All live tests provision their own keys per the standard envelope flow:
//   1. Generate an RSA-2048 keypair in the test.
//   2. Wrap a random 32-byte CEK with RSA-OAEP-SHA1 (what `RSA_OAEP` means in AE
//      metadata) and build the canonical signed envelope
//      (`mssql_auth::cek_envelope`): `0x01 || u16le(path_len) || u16le(cipher_len)
//      || utf16le(path) || cipher || signature`.
//   3. `CREATE COLUMN MASTER KEY`, `CREATE COLUMN ENCRYPTION KEY`, and a table
//      with at least one encrypted column.

#[cfg(feature = "always-encrypted")]
mod live_server {
    use mssql_auth::{AeadEncryptor, EncryptionType, InMemoryKeyStore};
    use mssql_client::{Client, Config, EncryptionConfig};
    use rand::RngCore;
    use rsa::{Oaep, RsaPrivateKey, pkcs8::EncodePrivateKey};
    use sha1::Sha1;
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
        /// The randomly-generated CEK. The round-trip test encrypts through the
        /// driver's registered key store rather than reading the raw key here,
        /// so this stays unread; retained for symmetry with `setup`.
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

    /// Build a canonical signed CEK envelope (see `mssql_auth::cek_envelope`),
    /// exactly as standard provisioning tools (SSMS, .NET, JDBC) do.
    ///
    /// `encrypted_cek` is the raw RSA-OAEP output; `cmk` signs the envelope.
    fn build_cek_envelope(cmk: &RsaPrivateKey, key_path: &str, encrypted_cek: &[u8]) -> Vec<u8> {
        use sha2::Digest;

        let mut envelope = mssql_auth::cek_envelope::build_signed_portion(key_path, encrypted_cek);
        let digest: [u8; 32] = Sha256::digest(&envelope).into();
        let signature = cmk
            .sign(rsa::Pkcs1v15Sign::new::<Sha256>(), &digest)
            .expect("CMK signs envelope");
        envelope.extend_from_slice(&signature);
        envelope
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

        // Wrap the CEK with RSA-OAEP-SHA1 (what RSA_OAEP means in AE
        // metadata) using the public half.
        let pub_key = rsa_private.to_public_key();
        let mut rng = rand::thread_rng();
        let padding = Oaep::new::<Sha1>();
        let rsa_ciphertext = pub_key
            .encrypt(&mut rng, padding, cek_bytes)
            .expect("rsa encrypt");
        let envelope = build_cek_envelope(rsa_private, KEY_PATH, &rsa_ciphertext);
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

    /// Full Always Encrypted write→read round-trip: the driver encrypts
    /// deterministic INT, NVARCHAR, and VARBINARY parameters plus a randomized
    /// NVARCHAR parameter on INSERT, then decrypts them all back on SELECT. Also
    /// confirms the stored value is real ciphertext (opaque to a connection
    /// without Always Encrypted), proving the encryption happens client-side,
    /// not on the server.
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_parameter_encryption_round_trip() {
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
             EncInt INT ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL, \
             EncName NVARCHAR(50) COLLATE Latin1_General_BIN2 ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL, \
             EncData VARBINARY(50) ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL, \
             EncRand NVARCHAR(50) ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = RANDOMIZED, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL )",
        )
        .await;
        drop(admin);

        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let blob: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let insert = format!(
            "INSERT INTO [{}] (Id, EncInt, EncName, EncData, EncRand) \
             VALUES (@p1, @p2, @p3, @p4, @p5)",
            fx.table_name
        );
        // The driver encrypts @p2/@p3/@p4 (deterministic) and @p5 (randomized)
        // client-side; @p1 (Id) stays plaintext.
        let inserted = client
            .execute(
                &insert,
                &[&1i32, &42i32, &"Ada Lovelace", &blob, &"randomized secret"],
            )
            .await;

        // Read the encrypted columns back; the driver decrypts transparently.
        let round_trip: Result<(i32, String, Vec<u8>, String), String> = if inserted.is_ok() {
            let select = format!(
                "SELECT EncInt, EncName, EncData, EncRand FROM [{}] WHERE Id = @p1",
                fx.table_name
            );
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some((
                        r.get::<i32>(0).map_err(|e| format!("EncInt: {e}"))?,
                        r.get::<String>(1).map_err(|e| format!("EncName: {e}"))?,
                        r.get::<Vec<u8>>(2).map_err(|e| format!("EncData: {e}"))?,
                        r.get::<String>(3).map_err(|e| format!("EncRand: {e}"))?,
                    ));
                }
                found.ok_or_else(|| "no row returned".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        // A connection WITHOUT Always Encrypted sees only ciphertext.
        let opaque: Result<Vec<u8>, String> = {
            let mut plain = Client::connect(admin_config().expect("cfg"))
                .await
                .expect("plain connect");
            let sql = format!("SELECT EncInt FROM [{}] WHERE Id = 1", fx.table_name);
            async {
                let rows = plain
                    .query(&sql, &[])
                    .await
                    .map_err(|e| format!("plain select: {e}"))?;
                let mut bytes = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    bytes = Some(r.get::<Vec<u8>>(0).map_err(|e| format!("raw: {e}"))?);
                }
                bytes.ok_or_else(|| "no row".to_string())
            }
            .await
        };

        // Teardown before asserting so a failure never leaks server objects.
        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        let inserted = inserted.expect("encrypted INSERT should succeed");
        assert_eq!(inserted, 1, "exactly one row inserted");

        let (got_int, got_name, got_data, got_rand) = round_trip.expect("decrypt round-trip");
        assert_eq!(
            got_int, 42,
            "deterministic INT decrypts to the inserted value"
        );
        assert_eq!(
            got_name, "Ada Lovelace",
            "deterministic NVARCHAR decrypts to the inserted value"
        );
        assert_eq!(
            got_data,
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            "deterministic VARBINARY decrypts to the inserted value"
        );
        assert_eq!(
            got_rand, "randomized secret",
            "randomized NVARCHAR decrypts to the inserted value"
        );

        let ciphertext = opaque.expect("read raw ciphertext");
        assert!(
            ciphertext.len() > 4,
            "stored value is an AEAD blob, not a 4-byte int"
        );
        assert_ne!(
            ciphertext,
            42i32.to_le_bytes().to_vec(),
            "stored value must not be the plaintext int"
        );
    }

    /// Write→read round-trip for the fixed-width numeric types: bigint,
    /// smallint, tinyint, bit, real, and float. Each is encrypted on INSERT and
    /// decrypted back on SELECT, exercising both the 8-byte integer
    /// normalization and the 4/8-byte IEEE float forms through the live server.
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_numeric_parameter_encryption_round_trip() {
        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");

        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();

        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( \
             Id INT NOT NULL PRIMARY KEY, \
             EncBig BIGINT {enc} NULL, \
             EncSmall SMALLINT {enc} NULL, \
             EncTiny TINYINT {enc} NULL, \
             EncBit BIT {enc} NULL, \
             EncReal REAL {enc} NULL, \
             EncFloat FLOAT {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);

        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let big = 0x0102_0304_0506_0708_i64;
        let insert = format!(
            "INSERT INTO [{}] (Id, EncBig, EncSmall, EncTiny, EncBit, EncReal, EncFloat) \
             VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7)",
            fx.table_name
        );
        let inserted = client
            .execute(
                &insert,
                &[&1i32, &big, &258i16, &200u8, &true, &3.5f32, &3.5f64],
            )
            .await;

        let round_trip: Result<(i64, i16, u8, bool, f32, f64), String> = if inserted.is_ok() {
            let select = format!(
                "SELECT EncBig, EncSmall, EncTiny, EncBit, EncReal, EncFloat FROM [{}] WHERE Id = @p1",
                fx.table_name
            );
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some((
                        r.get::<i64>(0).map_err(|e| format!("EncBig: {e}"))?,
                        r.get::<i16>(1).map_err(|e| format!("EncSmall: {e}"))?,
                        r.get::<u8>(2).map_err(|e| format!("EncTiny: {e}"))?,
                        r.get::<bool>(3).map_err(|e| format!("EncBit: {e}"))?,
                        r.get::<f32>(4).map_err(|e| format!("EncReal: {e}"))?,
                        r.get::<f64>(5).map_err(|e| format!("EncFloat: {e}"))?,
                    ));
                }
                found.ok_or_else(|| "no row returned".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        let inserted = inserted.expect("encrypted numeric INSERT should succeed");
        assert_eq!(inserted, 1, "one row inserted");
        let (g_big, g_small, g_tiny, g_bit, g_real, g_float) =
            round_trip.expect("decrypt round-trip");
        assert_eq!(g_big, big, "BIGINT round-trips");
        assert_eq!(g_small, 258, "SMALLINT round-trips");
        assert_eq!(g_tiny, 200, "TINYINT round-trips");
        assert!(g_bit, "BIT round-trips");
        assert_eq!(g_real, 3.5, "REAL round-trips");
        assert_eq!(g_float, 3.5, "FLOAT round-trips");
    }

    /// Write→read round-trip for UNIQUEIDENTIFIER and DATE: each is encrypted on
    /// INSERT and decrypted back on SELECT, exercising the mixed-endian GUID
    /// byte order and the 3-byte day-count date form through the live server.
    #[cfg(all(feature = "uuid", feature = "chrono"))]
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_uuid_date_parameter_encryption_round_trip() {
        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");
        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();
        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( Id INT NOT NULL PRIMARY KEY, \
             EncUuid UNIQUEIDENTIFIER {enc} NULL, \
             EncDate DATE {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let uuid_val = uuid::Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
        let date_val = chrono::NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
        let sql = format!(
            "INSERT INTO [{}] (Id, EncUuid, EncDate) VALUES (@p1, @p2, @p3)",
            fx.table_name
        );
        let inserted = client.execute(&sql, &[&1i32, &uuid_val, &date_val]).await;

        let read: Result<(uuid::Uuid, chrono::NaiveDate), String> = if inserted.is_ok() {
            let select = format!(
                "SELECT EncUuid, EncDate FROM [{}] WHERE Id = @p1",
                fx.table_name
            );
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some((
                        r.get::<uuid::Uuid>(0)
                            .map_err(|e| format!("EncUuid: {e}"))?,
                        r.get::<chrono::NaiveDate>(1)
                            .map_err(|e| format!("EncDate: {e}"))?,
                    ));
                }
                found.ok_or_else(|| "no row".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(inserted.expect("uuid/date insert"), 1, "one row inserted");
        let (g_uuid, g_date) = read.expect("read back");
        assert_eq!(g_uuid, uuid_val, "UNIQUEIDENTIFIER round-trips");
        assert_eq!(g_date, date_val, "DATE round-trips");
    }

    /// Write→read round-trip for a DECIMAL column: a `numeric(value, precision,
    /// scale)` parameter declares `decimal(p, s)` so describe matches the
    /// encrypted column exactly, then the value is encrypted on INSERT and
    /// decrypted back on SELECT.
    #[cfg(feature = "decimal")]
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_decimal_parameter_encryption_round_trip() {
        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");
        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();
        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( Id INT NOT NULL PRIMARY KEY, \
             EncDecimal DECIMAL(18,4) {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        // Scale 2 input declared as decimal(18,4): exercises the rescale in
        // `numeric` (a value normalized at the wrong scale would decrypt wrong).
        let value = rust_decimal::Decimal::new(1_234_567, 2); // 12345.67
        let sql = format!(
            "INSERT INTO [{}] (Id, EncDecimal) VALUES (@p1, @p2)",
            fx.table_name
        );
        let inserted = client
            .execute(&sql, &[&1i32, &mssql_client::numeric(value, 18, 4)])
            .await;

        let read: Result<rust_decimal::Decimal, String> = if inserted.is_ok() {
            let select = format!("SELECT EncDecimal FROM [{}] WHERE Id = @p1", fx.table_name);
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some(
                        r.get::<rust_decimal::Decimal>(0)
                            .map_err(|e| format!("EncDecimal: {e}"))?,
                    );
                }
                found.ok_or_else(|| "no row".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(inserted.expect("decimal insert"), 1, "one row inserted");
        assert_eq!(read.expect("read back"), value, "DECIMAL round-trips");
    }

    /// Write→read round-trip for MONEY and SMALLMONEY: each is encrypted on
    /// INSERT and decrypted back on SELECT as a scale-4 decimal.
    #[cfg(feature = "decimal")]
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_money_parameter_encryption_round_trip() {
        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");
        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();
        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( Id INT NOT NULL PRIMARY KEY, \
             EncMoney MONEY {enc} NULL, \
             EncSmallMoney SMALLMONEY {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let money = rust_decimal::Decimal::new(123_400, 4); // 12.3400
        let sql = format!(
            "INSERT INTO [{}] (Id, EncMoney, EncSmallMoney) VALUES (@p1, @p2, @p3)",
            fx.table_name
        );
        let inserted = client
            .execute(
                &sql,
                &[
                    &1i32,
                    &mssql_client::Money(money),
                    &mssql_client::SmallMoney(money),
                ],
            )
            .await;

        let read: Result<(rust_decimal::Decimal, rust_decimal::Decimal), String> =
            if inserted.is_ok() {
                let select = format!(
                    "SELECT EncMoney, EncSmallMoney FROM [{}] WHERE Id = @p1",
                    fx.table_name
                );
                async {
                    let rows = client
                        .query(&select, &[&1i32])
                        .await
                        .map_err(|e| format!("select: {e}"))?;
                    let mut found = None;
                    for r in rows {
                        let r = r.map_err(|e| format!("row: {e}"))?;
                        found = Some((
                            r.get::<rust_decimal::Decimal>(0)
                                .map_err(|e| format!("EncMoney: {e}"))?,
                            r.get::<rust_decimal::Decimal>(1)
                                .map_err(|e| format!("EncSmallMoney: {e}"))?,
                        ));
                    }
                    found.ok_or_else(|| "no row".to_string())
                }
                .await
            } else {
                Err("insert failed".to_string())
            };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(inserted.expect("money insert"), 1, "one row inserted");
        let (g_money, g_smallmoney) = read.expect("read back");
        assert_eq!(g_money, money, "MONEY round-trips");
        assert_eq!(g_smallmoney, money, "SMALLMONEY round-trips");
    }

    /// Write→read round-trip for the typed temporal parameters: `time`,
    /// `datetime2` (both at scale 7 and scale 3, exercising the scale-aware
    /// byte length), `datetimeoffset`, legacy `datetime`, and `smalldatetime`.
    /// Values are chosen to land exactly on each column's resolution.
    #[cfg(feature = "chrono")]
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_temporal_parameter_encryption_round_trip() {
        use chrono::{FixedOffset, NaiveDate, TimeZone};
        use mssql_client::{SmallDateTime, datetime, datetime2, datetimeoffset, time};

        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");
        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();
        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( Id INT NOT NULL PRIMARY KEY, \
             EncTime TIME(7) {enc} NULL, \
             EncTime3 TIME(3) {enc} NULL, \
             EncDt2 DATETIME2(7) {enc} NULL, \
             EncDt2s DATETIME2(3) {enc} NULL, \
             EncDto DATETIMEOFFSET(7) {enc} NULL, \
             EncDt DATETIME {enc} NULL, \
             EncSdt SMALLDATETIME {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let day = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
        let t7 = day.and_hms_nano_opt(13, 14, 15, 123_456_700).unwrap();
        let t3 = day.and_hms_milli_opt(13, 14, 15, 123).unwrap();
        let dto = FixedOffset::east_opt(5 * 3600 + 30 * 60)
            .unwrap()
            .from_local_datetime(&t7)
            .single()
            .unwrap();
        let dt_legacy = day.and_hms_milli_opt(13, 14, 15, 123).unwrap();
        let sdt = day.and_hms_opt(13, 14, 0).unwrap();

        let sql = format!(
            "INSERT INTO [{}] (Id, EncTime, EncTime3, EncDt2, EncDt2s, EncDto, EncDt, EncSdt) \
             VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7, @p8)",
            fx.table_name
        );
        let inserted = client
            .execute(
                &sql,
                &[
                    &1i32,
                    &time(t7.time(), 7),
                    &time(t3.time(), 3),
                    &datetime2(t7, 7),
                    &datetime2(t3, 3),
                    &datetimeoffset(dto, 7),
                    &datetime(dt_legacy),
                    &SmallDateTime(sdt),
                ],
            )
            .await;

        type Row = (
            chrono::NaiveTime,
            chrono::NaiveTime,
            chrono::NaiveDateTime,
            chrono::NaiveDateTime,
            chrono::DateTime<FixedOffset>,
            chrono::NaiveDateTime,
            chrono::NaiveDateTime,
        );
        let read: Result<Row, String> = if inserted.is_ok() {
            let select = format!(
                "SELECT EncTime, EncTime3, EncDt2, EncDt2s, EncDto, EncDt, EncSdt \
                 FROM [{}] WHERE Id = @p1",
                fx.table_name
            );
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some((
                        r.get::<chrono::NaiveTime>(0)
                            .map_err(|e| format!("time: {e}"))?,
                        r.get::<chrono::NaiveTime>(1)
                            .map_err(|e| format!("time3: {e}"))?,
                        r.get::<chrono::NaiveDateTime>(2)
                            .map_err(|e| format!("dt2: {e}"))?,
                        r.get::<chrono::NaiveDateTime>(3)
                            .map_err(|e| format!("dt2s: {e}"))?,
                        r.get::<chrono::DateTime<FixedOffset>>(4)
                            .map_err(|e| format!("dto: {e}"))?,
                        r.get::<chrono::NaiveDateTime>(5)
                            .map_err(|e| format!("dt: {e}"))?,
                        r.get::<chrono::NaiveDateTime>(6)
                            .map_err(|e| format!("sdt: {e}"))?,
                    ));
                }
                found.ok_or_else(|| "no row".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(inserted.expect("temporal insert"), 1, "one row inserted");
        let (g_t7, g_t3, g_dt2, g_dt2s, g_dto, g_dt, g_sdt) = read.expect("read back");
        assert_eq!(g_t7, t7.time(), "TIME(7) round-trips");
        assert_eq!(g_t3, t3.time(), "TIME(3) round-trips");
        assert_eq!(g_dt2, t7, "DATETIME2(7) round-trips");
        assert_eq!(g_dt2s, t3, "DATETIME2(3) round-trips");
        assert_eq!(g_dto, dto, "DATETIMEOFFSET round-trips");
        assert_eq!(g_dt, dt_legacy, "DATETIME round-trips");
        assert_eq!(g_sdt, sdt, "SMALLDATETIME round-trips");
    }

    /// Write→read round-trip for the fixed-width `char`/`nchar`/`binary` types.
    /// AE requires `char`/`nchar` columns to use a `*_BIN2` collation for
    /// deterministic encryption. The normalized form is the value's bytes,
    /// unpadded, so the values read back at their original length.
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_fixed_width_parameter_encryption_round_trip() {
        use mssql_client::{binary, char, nchar};

        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");
        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();
        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( Id INT NOT NULL PRIMARY KEY, \
             EncChar CHAR(10) COLLATE Latin1_General_BIN2 {enc} NULL, \
             EncNChar NCHAR(10) COLLATE Latin1_General_BIN2 {enc} NULL, \
             EncBinary BINARY(10) {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let bin_val: Vec<u8> = vec![1, 2, 3, 4, 5];
        let sql = format!(
            "INSERT INTO [{}] (Id, EncChar, EncNChar, EncBinary) VALUES (@p1, @p2, @p3, @p4)",
            fx.table_name
        );
        let inserted = client
            .execute(
                &sql,
                &[
                    &1i32,
                    &char("Hello", 10),
                    &nchar("Hello", 10),
                    &binary(bin_val.clone(), 10),
                ],
            )
            .await;

        let read: Result<(String, String, Vec<u8>), String> = if inserted.is_ok() {
            let select = format!(
                "SELECT EncChar, EncNChar, EncBinary FROM [{}] WHERE Id = @p1",
                fx.table_name
            );
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some((
                        r.get::<String>(0).map_err(|e| format!("char: {e}"))?,
                        r.get::<String>(1).map_err(|e| format!("nchar: {e}"))?,
                        r.get::<Vec<u8>>(2).map_err(|e| format!("binary: {e}"))?,
                    ));
                }
                found.ok_or_else(|| "no row".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(inserted.expect("fixed-width insert"), 1, "one row inserted");
        let (g_char, g_nchar, g_binary) = read.expect("read back");
        // Fixed-width CHAR/NCHAR read back space-padded to the column width
        // (matching SQL Server / .NET), even though the encrypted form is unpadded.
        assert_eq!(g_char, "Hello     ", "CHAR(10) reads back space-padded");
        assert_eq!(g_nchar, "Hello     ", "NCHAR(10) reads back space-padded");
        assert_eq!(g_binary, bin_val, "BINARY round-trips (unpadded)");
    }

    /// Typed NULL (`null::<T>()`) into encrypted columns of several types: the
    /// typed NULL carries its SQL type, so the server accepts it against the
    /// target column (an untyped `Option::None` would be declared `nvarchar(1)`
    /// and rejected against, e.g., the `int` or `varbinary` columns). Each
    /// column reads back as NULL.
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_typed_null_encrypted_param_round_trip() {
        use mssql_client::null;

        let admin_cfg = match admin_config() {
            Some(c) => c,
            None => return,
        };
        let mut admin = Client::connect(admin_cfg).await.expect("admin connect");
        let (rsa_key, pem) = fresh_rsa_keypair();
        let cek = fresh_cek();
        let enc = "ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = [{CEK}], \
                   ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256')";
        let ddl = format!(
            "CREATE TABLE [{{TABLE}}] ( Id INT NOT NULL PRIMARY KEY, \
             EncInt INT {enc} NULL, \
             EncBig BIGINT {enc} NULL, \
             EncData VARBINARY(50) {enc} NULL, \
             EncName NVARCHAR(50) COLLATE Latin1_General_BIN2 {enc} NULL )"
        );
        let fx = setup(&mut admin, &cek, &rsa_key, &ddl).await;
        drop(admin);
        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let sql = format!(
            "INSERT INTO [{}] (Id, EncInt, EncBig, EncData, EncName) \
             VALUES (@p1, @p2, @p3, @p4, @p5)",
            fx.table_name
        );
        let inserted = client
            .execute(
                &sql,
                &[
                    &1i32,
                    &null::<i32>(),
                    &null::<i64>(),
                    &null::<Vec<u8>>(),
                    &null::<String>(),
                ],
            )
            .await;

        // Each flag is whether that column read back as NULL.
        let read: Result<(bool, bool, bool, bool), String> = if inserted.is_ok() {
            let select = format!(
                "SELECT EncInt, EncBig, EncData, EncName FROM [{}] WHERE Id = @p1",
                fx.table_name
            );
            async {
                let rows = client
                    .query(&select, &[&1i32])
                    .await
                    .map_err(|e| format!("select: {e}"))?;
                let mut found = None;
                for r in rows {
                    let r = r.map_err(|e| format!("row: {e}"))?;
                    found = Some((
                        r.try_get::<i32>(0)
                            .map_err(|e| format!("EncInt: {e}"))?
                            .is_none(),
                        r.try_get::<i64>(1)
                            .map_err(|e| format!("EncBig: {e}"))?
                            .is_none(),
                        r.try_get::<Vec<u8>>(2)
                            .map_err(|e| format!("EncData: {e}"))?
                            .is_none(),
                        r.try_get::<String>(3)
                            .map_err(|e| format!("EncName: {e}"))?
                            .is_none(),
                    ));
                }
                found.ok_or_else(|| "no row".to_string())
            }
            .await
        } else {
            Err("insert failed".to_string())
        };

        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        assert_eq!(inserted.expect("typed-NULL insert"), 1, "one row inserted");
        let (i, b, d, n) = read.expect("read back");
        assert!(i, "encrypted INT NULL round-trips");
        assert!(b, "encrypted BIGINT NULL round-trips");
        assert!(d, "encrypted VARBINARY NULL round-trips");
        assert!(n, "encrypted NVARCHAR NULL round-trips");
    }

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

    /// Multi-result-set streaming across an Always Encrypted boundary (#287).
    ///
    /// `query_stream` is fed a **two-SELECT batch with different schemas**, the
    /// second selecting encrypted columns. This is the one path #257 claimed
    /// "correctness is not the gap" by inspection with no test: on the second
    /// `ColMetaData`, `RowStream::switch_result_set` rebuilds `columns()` and
    /// **re-resolves the AE decryptor mid-stream** (from `None` for the plaintext
    /// first set to a resolved decryptor for the encrypted second set).
    ///
    /// Asserts the flat row sequence is correct across the boundary, `columns()`
    /// updates when the second set's metadata arrives, and the second set's
    /// encrypted values decrypt to the inserted plaintext.
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_multi_result_set_streaming_reresolves_ae_decryptor() {
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
             EncInt INT ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL, \
             EncName NVARCHAR(50) COLLATE Latin1_General_BIN2 ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL )",
        )
        .await;
        drop(admin);

        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        // Seed one row; the driver encrypts @p2/@p3 client-side.
        let insert = format!(
            "INSERT INTO [{}] (Id, EncInt, EncName) VALUES (@p1, @p2, @p3)",
            fx.table_name
        );
        let inserted = client
            .execute(&insert, &[&1i32, &42i32, &"Ada Lovelace"])
            .await;

        // A two-statement batch: a plaintext first result set (single INT column
        // named `marker`) followed by an encrypted second result set (two
        // columns). No parameters, so `query_stream` issues a plain SQL batch and
        // the server returns two distinct result sets in one response. The
        // `WHERE Id = 1` predicate is a literal to keep the batch parameterless.
        let batch = format!(
            "SELECT 7 AS marker; \
             SELECT EncInt, EncName FROM [{}] WHERE Id = 1;",
            fx.table_name
        );

        // Capture everything from the stream into owned values before teardown,
        // so the mutable borrow ends and a failure never leaks server objects.
        struct Streamed {
            marker: i32,
            cols_after_set1: Vec<String>,
            enc_int: i32,
            enc_name: String,
            cols_after_set2: Vec<String>,
            ended: bool,
        }

        let streamed: Result<Streamed, String> = if inserted.is_ok() {
            async {
                let mut stream = client
                    .query_stream(&batch, &[])
                    .await
                    .map_err(|e| format!("stream: {e}"))?;

                // First result set: the plaintext `marker` row. After it yields,
                // `columns()` still reflects the first set.
                let r1 = stream
                    .try_next()
                    .await
                    .map_err(|e| format!("set1 row: {e}"))?
                    .ok_or_else(|| "expected a first-result-set row".to_string())?;
                let cols_after_set1: Vec<String> =
                    stream.columns().iter().map(|c| c.name.clone()).collect();
                let marker = r1
                    .get_by_name::<i32>("marker")
                    .map_err(|e| format!("marker: {e}"))?;

                // Second result set: pulling the next row crosses the boundary —
                // `switch_result_set` rebuilds `columns()` and re-resolves the AE
                // decryptor. The encrypted values must decrypt transparently.
                let r2 = stream
                    .try_next()
                    .await
                    .map_err(|e| format!("set2 row: {e}"))?
                    .ok_or_else(|| "expected a second-result-set row".to_string())?;
                let cols_after_set2: Vec<String> =
                    stream.columns().iter().map(|c| c.name.clone()).collect();
                let enc_int = r2
                    .get_by_name::<i32>("EncInt")
                    .map_err(|e| format!("EncInt decrypt: {e}"))?;
                let enc_name = r2
                    .get_by_name::<String>("EncName")
                    .map_err(|e| format!("EncName decrypt: {e}"))?;

                let ended = stream
                    .try_next()
                    .await
                    .map_err(|e| format!("end: {e}"))?
                    .is_none();

                Ok(Streamed {
                    marker,
                    cols_after_set1,
                    enc_int,
                    enc_name,
                    cols_after_set2,
                    ended,
                })
            }
            .await
        } else {
            Err("seed insert failed".to_string())
        };

        // Teardown before asserting so a failure never leaks server objects.
        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        let inserted = inserted.expect("seed INSERT should succeed");
        assert_eq!(inserted, 1, "exactly one row seeded");

        let s = streamed.expect("multi-result-set stream");

        // Flat row sequence across the boundary.
        assert_eq!(s.marker, 7, "first result set's plaintext marker");
        assert_eq!(
            s.cols_after_set1,
            vec!["marker".to_string()],
            "columns() reflects the first (plaintext) result set"
        );

        // columns() updated on the second ColMetaData.
        assert_eq!(
            s.cols_after_set2,
            vec!["EncInt".to_string(), "EncName".to_string()],
            "columns() updates to the second result set's schema mid-stream"
        );

        // Second set decrypts after the mid-stream decryptor re-resolution.
        assert_eq!(
            s.enc_int, 42,
            "encrypted INT in the second set decrypts after re-resolution"
        );
        assert_eq!(
            s.enc_name, "Ada Lovelace",
            "encrypted NVARCHAR in the second set decrypts after re-resolution"
        );

        assert!(s.ended, "stream ends after the two flattened result sets");
    }

    /// Multi-result-set streaming, decryptor-cleared direction (#287).
    ///
    /// The mirror of `test_multi_result_set_streaming_reresolves_ae_decryptor`:
    /// here the **encrypted set comes first** and a **plaintext set second**, so
    /// `switch_result_set` must clear the decryptor (resolved → `None`) on the
    /// boundary. The decryptor's encrypted-column map is positional, so a stale
    /// decryptor would wrongly attempt to decrypt the plaintext second set's
    /// column 0 — this asserts that does not happen.
    #[tokio::test]
    #[ignore = "Requires SQL Server with Always Encrypted"]
    async fn test_multi_result_set_streaming_clears_ae_decryptor() {
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
             EncInt INT ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL, \
             EncName NVARCHAR(50) COLLATE Latin1_General_BIN2 ENCRYPTED WITH ( \
             COLUMN_ENCRYPTION_KEY = [{CEK}], \
             ENCRYPTION_TYPE = DETERMINISTIC, \
             ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA_256' ) NULL )",
        )
        .await;
        drop(admin);

        let mut client = Client::connect(encrypted_config(&pem).expect("cfg"))
            .await
            .expect("ae connect");

        let insert = format!(
            "INSERT INTO [{}] (Id, EncInt, EncName) VALUES (@p1, @p2, @p3)",
            fx.table_name
        );
        let inserted = client
            .execute(&insert, &[&1i32, &42i32, &"Ada Lovelace"])
            .await;

        // Encrypted first result set, plaintext second. The prelude resolves a
        // decryptor for set 1; crossing into set 2 must clear it.
        let batch = format!(
            "SELECT EncInt, EncName FROM [{}] WHERE Id = 1; \
             SELECT 99 AS plain;",
            fx.table_name
        );

        struct Streamed {
            enc_int: i32,
            enc_name: String,
            cols_after_set1: Vec<String>,
            plain: i32,
            cols_after_set2: Vec<String>,
            ended: bool,
        }

        let streamed: Result<Streamed, String> = if inserted.is_ok() {
            async {
                let mut stream = client
                    .query_stream(&batch, &[])
                    .await
                    .map_err(|e| format!("stream: {e}"))?;

                // First result set: encrypted columns decrypt transparently.
                let r1 = stream
                    .try_next()
                    .await
                    .map_err(|e| format!("set1 row: {e}"))?
                    .ok_or_else(|| "expected a first-result-set row".to_string())?;
                let cols_after_set1: Vec<String> =
                    stream.columns().iter().map(|c| c.name.clone()).collect();
                let enc_int = r1
                    .get_by_name::<i32>("EncInt")
                    .map_err(|e| format!("EncInt decrypt: {e}"))?;
                let enc_name = r1
                    .get_by_name::<String>("EncName")
                    .map_err(|e| format!("EncName decrypt: {e}"))?;

                // Second result set: pulling crosses the boundary; the decryptor
                // must clear so the plaintext column 0 is not treated as encrypted.
                let r2 = stream
                    .try_next()
                    .await
                    .map_err(|e| format!("set2 row: {e}"))?
                    .ok_or_else(|| "expected a second-result-set row".to_string())?;
                let cols_after_set2: Vec<String> =
                    stream.columns().iter().map(|c| c.name.clone()).collect();
                let plain = r2
                    .get_by_name::<i32>("plain")
                    .map_err(|e| format!("plain: {e}"))?;

                let ended = stream
                    .try_next()
                    .await
                    .map_err(|e| format!("end: {e}"))?
                    .is_none();

                Ok(Streamed {
                    enc_int,
                    enc_name,
                    cols_after_set1,
                    plain,
                    cols_after_set2,
                    ended,
                })
            }
            .await
        } else {
            Err("seed insert failed".to_string())
        };

        // Teardown before asserting so a failure never leaks server objects.
        let mut admin = Client::connect(admin_config().expect("cfg"))
            .await
            .expect("admin reconnect");
        teardown(&mut admin, &fx).await;

        let inserted = inserted.expect("seed INSERT should succeed");
        assert_eq!(inserted, 1, "exactly one row seeded");

        let s = streamed.expect("multi-result-set stream");

        // First set decrypts.
        assert_eq!(s.enc_int, 42, "encrypted INT in the first set decrypts");
        assert_eq!(
            s.enc_name, "Ada Lovelace",
            "encrypted NVARCHAR in the first set decrypts"
        );
        assert_eq!(
            s.cols_after_set1,
            vec!["EncInt".to_string(), "EncName".to_string()],
            "columns() reflects the first (encrypted) result set"
        );

        // Second set reads as plaintext after the decryptor is cleared.
        assert_eq!(
            s.plain, 99,
            "plaintext second set reads correctly after the decryptor clears"
        );
        assert_eq!(
            s.cols_after_set2,
            vec!["plain".to_string()],
            "columns() updates to the second (plaintext) result set"
        );

        assert!(s.ended, "stream ends after the two flattened result sets");
    }
}
