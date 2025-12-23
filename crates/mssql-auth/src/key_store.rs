//! Key store providers and CEK caching for Always Encrypted.
//!
//! This module provides:
//! - [`InMemoryKeyStore`]: A simple key store for testing and development
//! - [`CekCache`]: A thread-safe cache for decrypted Column Encryption Keys
//!
//! ## Production Usage
//!
//! For production environments, implement the [`KeyStoreProvider`] trait
//! with a secure key storage solution such as:
//! - Azure Key Vault
//! - Windows Certificate Store
//! - Hardware Security Module (HSM)
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_auth::key_store::{InMemoryKeyStore, CekCache};
//!
//! // Create a key store with test keys
//! let mut key_store = InMemoryKeyStore::new();
//! key_store.add_key("TestKey", &private_key_pem)?;
//!
//! // Create a CEK cache for performance
//! let cek_cache = CekCache::new();
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use crate::aead::AeadEncryptor;
use crate::encryption::{EncryptionError, KeyStoreProvider};
use crate::key_unwrap::RsaKeyUnwrapper;

/// In-memory key store for testing and development.
///
/// **Security Warning**: This stores private keys in memory without hardware
/// protection. Use only for testing or development environments.
///
/// For production, use Azure Key Vault, Windows Certificate Store, or an HSM.
pub struct InMemoryKeyStore {
    /// Map of key path to RSA key unwrapper.
    keys: HashMap<String, RsaKeyUnwrapper>,
}

impl InMemoryKeyStore {
    /// Create a new empty in-memory key store.
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// Add a key to the store from PEM-encoded private key.
    ///
    /// # Arguments
    ///
    /// * `key_path` - The identifier/path for this key
    /// * `pem` - PEM-encoded RSA private key (PKCS#1 or PKCS#8)
    ///
    /// # Errors
    ///
    /// Returns an error if the PEM cannot be parsed.
    pub fn add_key(&mut self, key_path: &str, pem: &str) -> Result<(), EncryptionError> {
        let unwrapper = RsaKeyUnwrapper::from_pem(pem)?;
        self.keys.insert(key_path.to_string(), unwrapper);
        Ok(())
    }

    /// Add a key to the store from DER-encoded private key.
    ///
    /// # Arguments
    ///
    /// * `key_path` - The identifier/path for this key
    /// * `der` - DER-encoded RSA private key
    ///
    /// # Errors
    ///
    /// Returns an error if the DER cannot be parsed.
    pub fn add_key_der(&mut self, key_path: &str, der: &[u8]) -> Result<(), EncryptionError> {
        let unwrapper = RsaKeyUnwrapper::from_der(der)?;
        self.keys.insert(key_path.to_string(), unwrapper);
        Ok(())
    }

    /// Check if a key exists in the store.
    pub fn has_key(&self, key_path: &str) -> bool {
        self.keys.contains_key(key_path)
    }

    /// Remove a key from the store.
    pub fn remove_key(&mut self, key_path: &str) -> bool {
        self.keys.remove(key_path).is_some()
    }

    /// Get the number of keys in the store.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl Default for InMemoryKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl KeyStoreProvider for InMemoryKeyStore {
    fn provider_name(&self) -> &str {
        "IN_MEMORY_KEY_STORE"
    }

    async fn decrypt_cek(
        &self,
        cmk_path: &str,
        _algorithm: &str,
        encrypted_cek: &[u8],
    ) -> Result<Vec<u8>, EncryptionError> {
        let unwrapper = self.keys.get(cmk_path).ok_or_else(|| {
            EncryptionError::KeyStoreNotFound(format!("Key not found: {}", cmk_path))
        })?;

        unwrapper.decrypt_cek(encrypted_cek)
    }
}

/// Entry in the CEK cache.
struct CekCacheEntry {
    /// The decrypted CEK (stored for potential future use like re-keying).
    #[allow(dead_code)]
    cek: Vec<u8>,
    /// AEAD encryptor instance (pre-derived keys).
    encryptor: Arc<AeadEncryptor>,
    /// When this entry was created.
    created_at: Instant,
}

/// Thread-safe cache for decrypted Column Encryption Keys.
///
/// The cache stores decrypted CEKs and pre-computed AEAD encryptors
/// to avoid repeated RSA decryption and key derivation operations.
///
/// ## Cache Key
///
/// Entries are keyed by: `(database_id, cek_id, cek_version)`
///
/// ## Expiration
///
/// Entries expire after a configurable TTL (default: 2 hours).
/// Expired entries are lazily removed on access.
pub struct CekCache {
    /// Map of cache key to entry.
    entries: RwLock<HashMap<CekCacheKey, CekCacheEntry>>,
    /// Time-to-live for cache entries.
    ttl: Duration,
}

/// Key for CEK cache entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CekCacheKey {
    /// Database ID.
    pub database_id: u32,
    /// CEK ID within the database.
    pub cek_id: u32,
    /// CEK version (for key rotation).
    pub cek_version: u32,
}

impl CekCacheKey {
    /// Create a new cache key.
    pub fn new(database_id: u32, cek_id: u32, cek_version: u32) -> Self {
        Self {
            database_id,
            cek_id,
            cek_version,
        }
    }
}

impl CekCache {
    /// Create a new CEK cache with default TTL (2 hours).
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(2 * 60 * 60))
    }

    /// Create a new CEK cache with custom TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    /// Get a cached encryptor for a CEK.
    ///
    /// Returns `None` if the entry doesn't exist or has expired.
    pub fn get(&self, key: &CekCacheKey) -> Option<Arc<AeadEncryptor>> {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(key) {
            if entry.created_at.elapsed() < self.ttl {
                return Some(Arc::clone(&entry.encryptor));
            }
        }
        None
    }

    /// Insert a CEK into the cache.
    ///
    /// Creates an AEAD encryptor from the CEK for future use.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    /// * `cek` - The decrypted Column Encryption Key
    ///
    /// # Returns
    ///
    /// The AEAD encryptor for the CEK.
    pub fn insert(
        &self,
        key: CekCacheKey,
        cek: Vec<u8>,
    ) -> Result<Arc<AeadEncryptor>, EncryptionError> {
        let encryptor = Arc::new(AeadEncryptor::new(&cek)?);

        let entry = CekCacheEntry {
            cek,
            encryptor: Arc::clone(&encryptor),
            created_at: Instant::now(),
        };

        let mut entries = self.entries.write();
        entries.insert(key, entry);

        Ok(encryptor)
    }

    /// Get or insert a CEK.
    ///
    /// If the CEK is cached, returns the cached encryptor.
    /// Otherwise, calls the provided function to get the CEK
    /// and caches it.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    /// * `get_cek` - Function to get the CEK if not cached
    pub async fn get_or_insert<F, Fut>(
        &self,
        key: CekCacheKey,
        get_cek: F,
    ) -> Result<Arc<AeadEncryptor>, EncryptionError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<u8>, EncryptionError>>,
    {
        // Try to get from cache first
        if let Some(encryptor) = self.get(&key) {
            return Ok(encryptor);
        }

        // Not in cache, fetch and insert
        let cek = get_cek().await?;
        self.insert(key, cek)
    }

    /// Remove a CEK from the cache.
    ///
    /// Call this when a CEK is rotated or invalidated.
    pub fn remove(&self, key: &CekCacheKey) -> bool {
        let mut entries = self.entries.write();
        entries.remove(key).is_some()
    }

    /// Clear all expired entries from the cache.
    pub fn cleanup_expired(&self) {
        let mut entries = self.entries.write();
        entries.retain(|_, entry| entry.created_at.elapsed() < self.ttl);
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        let mut entries = self.entries.write();
        entries.clear();
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

impl Default for CekCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use rsa::{RsaPrivateKey, pkcs8::EncodePrivateKey};

    fn generate_test_key_pem() -> String {
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_in_memory_key_store_new() {
        let store = InMemoryKeyStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_in_memory_key_store_add_key() {
        let mut store = InMemoryKeyStore::new();
        let pem = generate_test_key_pem();

        store.add_key("TestKey", &pem).unwrap();
        assert!(store.has_key("TestKey"));
        assert!(!store.has_key("OtherKey"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_in_memory_key_store_remove_key() {
        let mut store = InMemoryKeyStore::new();
        let pem = generate_test_key_pem();

        store.add_key("TestKey", &pem).unwrap();
        assert!(store.remove_key("TestKey"));
        assert!(!store.has_key("TestKey"));
        assert!(!store.remove_key("TestKey"));
    }

    #[test]
    fn test_in_memory_key_store_provider_name() {
        let store = InMemoryKeyStore::new();
        assert_eq!(store.provider_name(), "IN_MEMORY_KEY_STORE");
    }

    #[test]
    fn test_cek_cache_key() {
        let key1 = CekCacheKey::new(1, 2, 3);
        let key2 = CekCacheKey::new(1, 2, 3);
        let key3 = CekCacheKey::new(1, 2, 4);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cek_cache_insert_and_get() {
        let cache = CekCache::new();
        let key = CekCacheKey::new(1, 1, 1);
        let cek = vec![0x42u8; 32];

        // Insert
        let encryptor = cache.insert(key.clone(), cek).unwrap();
        assert_eq!(cache.len(), 1);

        // Get
        let retrieved = cache.get(&key);
        assert!(retrieved.is_some());
        assert!(Arc::ptr_eq(&encryptor, &retrieved.unwrap()));
    }

    #[test]
    fn test_cek_cache_miss() {
        let cache = CekCache::new();
        let key = CekCacheKey::new(1, 1, 1);

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cek_cache_expiration() {
        let cache = CekCache::with_ttl(Duration::from_millis(10));
        let key = CekCacheKey::new(1, 1, 1);
        let cek = vec![0x42u8; 32];

        cache.insert(key.clone(), cek).unwrap();
        assert!(cache.get(&key).is_some());

        // Wait for expiration
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
    }

    #[test]
    fn test_cek_cache_clear() {
        let cache = CekCache::new();

        for i in 0..5 {
            let key = CekCacheKey::new(i, 1, 1);
            let cek = vec![0x42u8; 32];
            cache.insert(key, cek).unwrap();
        }

        assert_eq!(cache.len(), 5);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cek_cache_cleanup_expired() {
        let cache = CekCache::with_ttl(Duration::from_millis(50));

        // Insert first entry
        let key1 = CekCacheKey::new(1, 1, 1);
        cache.insert(key1.clone(), vec![0x42u8; 32]).unwrap();

        // Wait a bit, then insert second entry
        std::thread::sleep(Duration::from_millis(30));
        let key2 = CekCacheKey::new(2, 1, 1);
        cache.insert(key2.clone(), vec![0x43u8; 32]).unwrap();

        assert_eq!(cache.len(), 2);

        // Wait for first entry to expire
        std::thread::sleep(Duration::from_millis(30));
        cache.cleanup_expired();

        // First entry should be removed, second should remain
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&key1).is_none());
        assert!(cache.get(&key2).is_some());
    }
}
