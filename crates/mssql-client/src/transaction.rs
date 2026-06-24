//! Transaction support.
//!
//! This module provides transaction isolation levels and savepoint support
//! for SQL Server.

/// Transaction isolation level.
///
/// SQL Server supports these isolation levels for transaction management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
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
/// ```rust,no_run
/// # async fn ex(client: mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
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
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a savepoint should be used to rollback or it has no effect"]
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
