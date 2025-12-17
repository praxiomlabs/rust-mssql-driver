//! Transaction support.
//!
//! This module provides transaction isolation levels, savepoint support,
//! and transaction abstractions for SQL Server.

/// Transaction isolation level.
///
/// SQL Server supports these isolation levels for transaction management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    /// Read uncommitted (dirty reads allowed).
    ///
    /// Lowest isolation - transactions can read uncommitted changes from
    /// other transactions. Offers best performance but no consistency guarantees.
    ReadUncommitted,

    /// Read committed (default for SQL Server).
    ///
    /// Transactions can only read committed data. Prevents dirty reads
    /// but allows non-repeatable reads and phantom reads.
    #[default]
    ReadCommitted,

    /// Repeatable read.
    ///
    /// Ensures rows read by a transaction don't change during the transaction.
    /// Prevents dirty reads and non-repeatable reads, but allows phantom reads.
    RepeatableRead,

    /// Serializable (highest isolation).
    ///
    /// Strictest isolation - transactions are completely isolated from
    /// each other. Prevents all read phenomena but has highest lock contention.
    Serializable,

    /// Snapshot isolation.
    ///
    /// Uses row versioning to provide a point-in-time view of data.
    /// Requires snapshot isolation to be enabled on the database.
    Snapshot,
}

impl IsolationLevel {
    /// Get the SQL statement to set this isolation level.
    #[must_use]
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::ReadUncommitted => "SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED",
            Self::ReadCommitted => "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
            Self::RepeatableRead => "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ",
            Self::Serializable => "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
            Self::Snapshot => "SET TRANSACTION ISOLATION LEVEL SNAPSHOT",
        }
    }

    /// Get the isolation level name as used in SQL Server.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ReadUncommitted => "READ UNCOMMITTED",
            Self::ReadCommitted => "READ COMMITTED",
            Self::RepeatableRead => "REPEATABLE READ",
            Self::Serializable => "SERIALIZABLE",
            Self::Snapshot => "SNAPSHOT",
        }
    }
}

/// A savepoint within a transaction.
///
/// Savepoints allow partial rollbacks within a transaction.
/// The savepoint name is validated when created to prevent SQL injection.
///
/// # Example
///
/// ```rust,ignore
/// let mut tx = client.begin_transaction().await?;
///
/// tx.execute("INSERT INTO orders (customer_id) VALUES (@p1)", &[&42]).await?;
/// let sp = tx.save_point("before_items").await?;
///
/// tx.execute("INSERT INTO items (order_id, product_id) VALUES (@p1, @p2)", &[&1, &100]).await?;
///
/// // Oops, need to undo the items but keep the order
/// tx.rollback_to(&sp).await?;
///
/// // Continue with different items...
/// tx.commit().await?;
/// ```
#[derive(Debug, Clone)]
pub struct SavePoint {
    /// The validated savepoint name.
    pub(crate) name: String,
}

impl SavePoint {
    /// Create a new savepoint with a validated name.
    ///
    /// This is called internally after name validation.
    pub(crate) fn new(name: String) -> Self {
        Self { name }
    }

    /// Get the savepoint name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A database transaction abstraction.
///
/// This is a higher-level transaction wrapper that can be used
/// with closure-based APIs or as a standalone type.
pub struct Transaction<'a> {
    isolation_level: IsolationLevel,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl Transaction<'_> {
    /// Create a new transaction with default isolation level.
    #[allow(dead_code)] // Used when transaction begin is implemented
    pub(crate) fn new() -> Self {
        Self {
            isolation_level: IsolationLevel::default(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a new transaction with specified isolation level.
    #[allow(dead_code)] // Used when transaction begin is implemented
    pub(crate) fn with_isolation_level(level: IsolationLevel) -> Self {
        Self {
            isolation_level: level,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the isolation level of this transaction.
    #[must_use]
    pub fn isolation_level(&self) -> IsolationLevel {
        self.isolation_level
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_level_sql() {
        assert_eq!(
            IsolationLevel::ReadCommitted.as_sql(),
            "SET TRANSACTION ISOLATION LEVEL READ COMMITTED"
        );
        assert_eq!(
            IsolationLevel::Snapshot.as_sql(),
            "SET TRANSACTION ISOLATION LEVEL SNAPSHOT"
        );
    }

    #[test]
    fn test_isolation_level_name() {
        assert_eq!(IsolationLevel::ReadCommitted.name(), "READ COMMITTED");
        assert_eq!(IsolationLevel::Serializable.name(), "SERIALIZABLE");
    }

    #[test]
    fn test_savepoint_name() {
        let sp = SavePoint::new("my_savepoint".to_string());
        assert_eq!(sp.name(), "my_savepoint");
        // SavePoint now has no lifetime parameter
        assert_eq!(sp.name, "my_savepoint");
    }

    #[test]
    fn test_default_isolation_level() {
        let level = IsolationLevel::default();
        assert_eq!(level, IsolationLevel::ReadCommitted);
    }
}
