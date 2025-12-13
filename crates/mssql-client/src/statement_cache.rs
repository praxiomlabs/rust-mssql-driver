//! Prepared statement caching with LRU eviction.
//!
//! This module provides automatic caching of prepared statements to improve performance
//! for repeated query execution. The cache uses an LRU (Least Recently Used) eviction
//! policy to manage memory and server-side resources.
//!
//! ## Lifecycle
//!
//! 1. First execution of a parameterized query calls `sp_prepare`, returning a handle
//! 2. The handle is cached by SQL hash; subsequent executions use `sp_execute`
//! 3. When the cache is full, LRU eviction calls `sp_unprepare` for evicted handles
//! 4. Pool reset (`sp_reset_connection`) invalidates all handles, clearing the cache
//! 5. Connection close implicitly releases all server-side handles

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::time::Instant;

use lru::LruCache;

/// Default maximum number of prepared statements to cache per connection.
pub const DEFAULT_MAX_STATEMENTS: usize = 256;

/// A cached prepared statement.
///
/// Contains the server-assigned handle and metadata needed for execution.
#[derive(Debug, Clone)]
pub struct PreparedStatement {
    /// Server-assigned handle for this prepared statement.
    handle: i32,
    /// Hash of the SQL text for cache lookup.
    sql_hash: u64,
    /// The original SQL text (for debugging and logging).
    sql: String,
    /// Timestamp when this statement was prepared.
    created_at: Instant,
}

impl PreparedStatement {
    /// Create a new prepared statement.
    pub fn new(handle: i32, sql: String) -> Self {
        Self {
            handle,
            sql_hash: hash_sql(&sql),
            sql,
            created_at: Instant::now(),
        }
    }

    /// Get the server-assigned handle.
    #[must_use]
    pub fn handle(&self) -> i32 {
        self.handle
    }

    /// Get the SQL hash.
    #[must_use]
    pub fn sql_hash(&self) -> u64 {
        self.sql_hash
    }

    /// Get the SQL text.
    #[must_use]
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Get the creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// Get the age of this statement.
    #[must_use]
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

/// LRU cache for prepared statements.
///
/// This cache automatically evicts the least recently used statements when
/// the maximum capacity is reached. Evicted statements should have their
/// server-side handles released via `sp_unprepare`.
pub struct StatementCache {
    /// LRU cache of prepared statements keyed by SQL hash.
    cache: LruCache<u64, PreparedStatement>,
    /// Maximum number of cached statements.
    max_size: usize,
    /// Total number of cache hits (for metrics).
    hits: u64,
    /// Total number of cache misses (for metrics).
    misses: u64,
}

impl StatementCache {
    /// Create a new statement cache with the specified maximum size.
    ///
    /// # Panics
    ///
    /// Panics if `max_size` is 0.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        assert!(max_size > 0, "max_size must be greater than 0");
        Self {
            cache: LruCache::new(NonZeroUsize::new(max_size).expect("max_size > 0")),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    /// Create a new statement cache with the default maximum size.
    #[must_use]
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_MAX_STATEMENTS)
    }

    /// Look up a prepared statement by SQL text.
    ///
    /// Returns `Some(handle)` if the statement is cached, `None` otherwise.
    /// This updates the LRU order.
    pub fn get(&mut self, sql: &str) -> Option<i32> {
        let hash = hash_sql(sql);
        if let Some(stmt) = self.cache.get(&hash) {
            self.hits += 1;
            tracing::trace!(sql = sql, handle = stmt.handle, "statement cache hit");
            Some(stmt.handle)
        } else {
            self.misses += 1;
            tracing::trace!(sql = sql, "statement cache miss");
            None
        }
    }

    /// Peek at a prepared statement without updating LRU order.
    pub fn peek(&self, sql: &str) -> Option<&PreparedStatement> {
        let hash = hash_sql(sql);
        self.cache.peek(&hash)
    }

    /// Insert a prepared statement into the cache.
    ///
    /// Returns the evicted statement if one was removed due to capacity.
    pub fn insert(&mut self, stmt: PreparedStatement) -> Option<PreparedStatement> {
        let hash = stmt.sql_hash;
        tracing::debug!(
            sql = stmt.sql(),
            handle = stmt.handle,
            "caching prepared statement"
        );

        // Check if we need to evict
        let evicted = if self.cache.len() >= self.max_size {
            // Pop least recently used
            self.cache.pop_lru().map(|(_, stmt)| stmt)
        } else {
            None
        };

        self.cache.put(hash, stmt);
        evicted
    }

    /// Remove a prepared statement from the cache.
    ///
    /// Returns the removed statement if it was present.
    pub fn remove(&mut self, sql: &str) -> Option<PreparedStatement> {
        let hash = hash_sql(sql);
        self.cache.pop(&hash)
    }

    /// Clear all cached statements.
    ///
    /// Returns an iterator over all removed statements.
    /// The caller should call `sp_unprepare` for each returned statement.
    pub fn clear(&mut self) -> impl Iterator<Item = PreparedStatement> + '_ {
        let mut statements = Vec::with_capacity(self.cache.len());
        while let Some((_, stmt)) = self.cache.pop_lru() {
            statements.push(stmt);
        }
        tracing::debug!(count = statements.len(), "cleared statement cache");
        statements.into_iter()
    }

    /// Get the number of cached statements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get the maximum cache size.
    #[must_use]
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Get the number of cache hits.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Get the number of cache misses.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Get the cache hit ratio (0.0 to 1.0).
    #[must_use]
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Reset cache statistics.
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

impl Default for StatementCache {
    fn default() -> Self {
        Self::with_default_size()
    }
}

impl std::fmt::Debug for StatementCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatementCache")
            .field("len", &self.cache.len())
            .field("max_size", &self.max_size)
            .field("hits", &self.hits)
            .field("misses", &self.misses)
            .finish()
    }
}

/// Hash SQL text for cache lookup.
///
/// Uses a stable hash algorithm to ensure consistent lookups.
#[must_use]
pub fn hash_sql(sql: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    sql.hash(&mut hasher);
    hasher.finish()
}

/// Configuration for statement caching.
#[derive(Debug, Clone)]
pub struct StatementCacheConfig {
    /// Whether statement caching is enabled.
    pub enabled: bool,
    /// Maximum number of statements to cache.
    pub max_size: usize,
}

impl Default for StatementCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: DEFAULT_MAX_STATEMENTS,
        }
    }
}

impl StatementCacheConfig {
    /// Create a new configuration with caching disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_size: 0,
        }
    }

    /// Create a new configuration with a custom max size.
    #[must_use]
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            enabled: true,
            max_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statement_cache_new() {
        let cache = StatementCache::new(10);
        assert_eq!(cache.max_size(), 10);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_statement_cache_insert_and_get() {
        let mut cache = StatementCache::new(10);

        let stmt = PreparedStatement::new(1, "SELECT * FROM users".to_string());
        cache.insert(stmt);

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("SELECT * FROM users"), Some(1));
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 0);
    }

    #[test]
    fn test_statement_cache_miss() {
        let mut cache = StatementCache::new(10);

        assert_eq!(cache.get("SELECT 1"), None);
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 0);
    }

    #[test]
    fn test_statement_cache_lru_eviction() {
        let mut cache = StatementCache::new(2);

        // Insert 2 statements
        cache.insert(PreparedStatement::new(1, "SELECT 1".to_string()));
        cache.insert(PreparedStatement::new(2, "SELECT 2".to_string()));
        assert_eq!(cache.len(), 2);

        // Access the first statement to make it recently used
        cache.get("SELECT 1");

        // Insert a third statement - should evict "SELECT 2"
        let evicted = cache.insert(PreparedStatement::new(3, "SELECT 3".to_string()));

        assert!(evicted.is_some());
        assert_eq!(evicted.unwrap().handle(), 2);
        assert_eq!(cache.len(), 2);

        // Verify "SELECT 1" is still cached (was accessed recently)
        assert_eq!(cache.get("SELECT 1"), Some(1));
        // Verify "SELECT 2" was evicted
        assert_eq!(cache.get("SELECT 2"), None);
        // Verify "SELECT 3" is cached
        assert_eq!(cache.get("SELECT 3"), Some(3));
    }

    #[test]
    fn test_statement_cache_clear() {
        let mut cache = StatementCache::new(10);

        cache.insert(PreparedStatement::new(1, "SELECT 1".to_string()));
        cache.insert(PreparedStatement::new(2, "SELECT 2".to_string()));

        let cleared: Vec<_> = cache.clear().collect();
        assert_eq!(cleared.len(), 2);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_statement_cache_remove() {
        let mut cache = StatementCache::new(10);

        cache.insert(PreparedStatement::new(1, "SELECT 1".to_string()));
        assert_eq!(cache.len(), 1);

        let removed = cache.remove("SELECT 1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().handle(), 1);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_statement_cache_hit_ratio() {
        let mut cache = StatementCache::new(10);

        cache.insert(PreparedStatement::new(1, "SELECT 1".to_string()));

        // 2 hits, 1 miss
        cache.get("SELECT 1");
        cache.get("SELECT 1");
        cache.get("SELECT 2");

        assert_eq!(cache.hits(), 2);
        assert_eq!(cache.misses(), 1);
        assert!((cache.hit_ratio() - 0.666666).abs() < 0.001);
    }

    #[test]
    fn test_hash_sql_consistency() {
        let sql = "SELECT * FROM users WHERE id = @p1";
        let hash1 = hash_sql(sql);
        let hash2 = hash_sql(sql);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_sql_different() {
        let hash1 = hash_sql("SELECT 1");
        let hash2 = hash_sql("SELECT 2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_prepared_statement_age() {
        let stmt = PreparedStatement::new(1, "SELECT 1".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(stmt.age().as_millis() >= 10);
    }

    #[test]
    fn test_statement_cache_config_default() {
        let config = StatementCacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_size, DEFAULT_MAX_STATEMENTS);
    }

    #[test]
    fn test_statement_cache_config_disabled() {
        let config = StatementCacheConfig::disabled();
        assert!(!config.enabled);
    }
}
