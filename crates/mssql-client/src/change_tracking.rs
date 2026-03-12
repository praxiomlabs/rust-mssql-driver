//! SQL Server Change Tracking support.
//!
//! This module provides helper types and utilities for working with SQL Server's
//! built-in Change Tracking feature, which enables efficient incremental data
//! synchronization scenarios.
//!
//! ## Overview
//!
//! SQL Server Change Tracking automatically tracks changes (inserts, updates,
//! deletes) to table rows. Applications can query for changes since a specific
//! version to implement incremental sync patterns.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_client::change_tracking::{ChangeOperation, ChangeTrackingQuery};
//!
//! // Get current version for baseline
//! let current_version: i64 = client
//!     .query("SELECT CHANGE_TRACKING_CURRENT_VERSION()")
//!     .await?
//!     .first()
//!     .and_then(|r| r.try_get(0))
//!     .unwrap_or(0);
//!
//! // Later, query for changes since that version
//! let query = ChangeTrackingQuery::changes("Products", last_sync_version);
//! let changes: Vec<ChangedRow> = client.query(&query.to_sql()).await?
//!     .map(|row| ChangedRow::from_row(&row))
//!     .collect();
//!
//! for change in changes {
//!     match change.operation {
//!         ChangeOperation::Insert => println!("New row: {:?}", change.primary_key),
//!         ChangeOperation::Update => println!("Updated row: {:?}", change.primary_key),
//!         ChangeOperation::Delete => println!("Deleted row: {:?}", change.primary_key),
//!     }
//! }
//! ```
//!
//! ## Prerequisites
//!
//! Change Tracking must be enabled on the database and table:
//!
//! ```sql
//! -- Enable on database
//! ALTER DATABASE MyDB SET CHANGE_TRACKING = ON
//!     (CHANGE_RETENTION = 2 DAYS, AUTO_CLEANUP = ON);
//!
//! -- Enable on table
//! ALTER TABLE Products ENABLE CHANGE_TRACKING
//!     WITH (TRACK_COLUMNS_UPDATED = ON);
//! ```
//!
//! ## Key Concepts
//!
//! - **Version**: A monotonically increasing value representing a point in time
//! - **SYS_CHANGE_OPERATION**: I (Insert), U (Update), D (Delete)
//! - **SYS_CHANGE_VERSION**: The version when the row was last changed
//! - **SYS_CHANGE_CREATION_VERSION**: The version when the row was inserted
//!
//! ## References
//!
//! - [About Change Tracking](https://learn.microsoft.com/en-us/sql/relational-databases/track-changes/about-change-tracking-sql-server)
//! - [CHANGETABLE function](https://learn.microsoft.com/en-us/sql/relational-databases/system-functions/changetable-transact-sql)

use std::fmt;

use bytes::Bytes;

/// The type of change operation tracked by SQL Server Change Tracking.
///
/// This corresponds to the `SYS_CHANGE_OPERATION` column in `CHANGETABLE` results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChangeOperation {
    /// A new row was inserted (I).
    Insert,
    /// An existing row was updated (U).
    Update,
    /// A row was deleted (D).
    Delete,
}

impl ChangeOperation {
    /// Parse a change operation from its single-character SQL Server representation.
    ///
    /// # Arguments
    ///
    /// * `s` - A string containing 'I', 'U', or 'D'
    ///
    /// # Returns
    ///
    /// The parsed `ChangeOperation`, or `None` if the input is invalid.
    #[must_use]
    pub fn from_sql(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "I" => Some(Self::Insert),
            "U" => Some(Self::Update),
            "D" => Some(Self::Delete),
            _ => None,
        }
    }

    /// Get the SQL Server single-character representation.
    #[must_use]
    pub const fn as_sql(&self) -> &'static str {
        match self {
            Self::Insert => "I",
            Self::Update => "U",
            Self::Delete => "D",
        }
    }

    /// Check if this is an insert operation.
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        matches!(self, Self::Insert)
    }

    /// Check if this is an update operation.
    #[must_use]
    pub const fn is_update(&self) -> bool {
        matches!(self, Self::Update)
    }

    /// Check if this is a delete operation.
    #[must_use]
    pub const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete)
    }
}

impl fmt::Display for ChangeOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Insert => write!(f, "INSERT"),
            Self::Update => write!(f, "UPDATE"),
            Self::Delete => write!(f, "DELETE"),
        }
    }
}

/// Metadata from a Change Tracking query result.
///
/// Contains the system columns returned by `CHANGETABLE(CHANGES ...)`.
#[derive(Debug, Clone)]
pub struct ChangeMetadata {
    /// The version when the row was last changed.
    pub version: i64,
    /// The version when the row was created (inserted).
    /// This is `None` for delete operations.
    pub creation_version: Option<i64>,
    /// The type of change (Insert, Update, Delete).
    pub operation: ChangeOperation,
    /// Binary mask of changed columns.
    /// Use `CHANGE_TRACKING_IS_COLUMN_IN_MASK()` to interpret.
    pub changed_columns: Option<Bytes>,
    /// Application-defined change context.
    pub context: Option<Bytes>,
}

impl ChangeMetadata {
    /// Create new change metadata.
    #[must_use]
    pub fn new(
        version: i64,
        creation_version: Option<i64>,
        operation: ChangeOperation,
        changed_columns: Option<Bytes>,
        context: Option<Bytes>,
    ) -> Self {
        Self {
            version,
            creation_version,
            operation,
            changed_columns,
            context,
        }
    }

    /// Create metadata for an insert operation.
    #[must_use]
    pub fn insert(version: i64) -> Self {
        Self {
            version,
            creation_version: Some(version),
            operation: ChangeOperation::Insert,
            changed_columns: None,
            context: None,
        }
    }

    /// Create metadata for an update operation.
    #[must_use]
    pub fn update(version: i64, creation_version: i64) -> Self {
        Self {
            version,
            creation_version: Some(creation_version),
            operation: ChangeOperation::Update,
            changed_columns: None,
            context: None,
        }
    }

    /// Create metadata for a delete operation.
    #[must_use]
    pub fn delete(version: i64) -> Self {
        Self {
            version,
            creation_version: None,
            operation: ChangeOperation::Delete,
            changed_columns: None,
            context: None,
        }
    }
}

/// Query builder for Change Tracking operations.
///
/// Helps construct proper SQL queries for common Change Tracking patterns.
///
/// # Example
///
/// ```rust
/// use mssql_client::change_tracking::ChangeTrackingQuery;
///
/// // Query for all changes since version 42
/// let query = ChangeTrackingQuery::changes("Products", 42);
/// assert!(query.to_sql().contains("CHANGETABLE"));
///
/// // Query with specific columns
/// let query = ChangeTrackingQuery::changes("Orders", 100)
///     .with_columns(&["OrderId", "CustomerId", "OrderDate"]);
/// let sql = query.to_sql();
/// assert!(sql.contains("OrderId"));
/// ```
#[derive(Debug, Clone)]
pub struct ChangeTrackingQuery {
    table_name: String,
    last_sync_version: i64,
    columns: Option<Vec<String>>,
    primary_keys: Option<Vec<String>>,
    alias: String,
    force_seek: bool,
}

impl ChangeTrackingQuery {
    /// Create a query for changes to a table since a specific version.
    ///
    /// This generates a `CHANGETABLE(CHANGES table_name, last_sync_version)` query.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The name of the table to query changes for
    /// * `last_sync_version` - The version from the previous sync (0 for initial)
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTrackingQuery;
    ///
    /// let query = ChangeTrackingQuery::changes("Products", 42);
    /// ```
    #[must_use]
    pub fn changes(table_name: impl Into<String>, last_sync_version: i64) -> Self {
        Self {
            table_name: table_name.into(),
            last_sync_version,
            columns: None,
            primary_keys: None,
            alias: "CT".into(),
            force_seek: false,
        }
    }

    /// Specify which data columns to include (in addition to change tracking columns).
    ///
    /// If not specified, only change tracking system columns are returned.
    ///
    /// # Arguments
    ///
    /// * `columns` - Column names to include in the result
    #[must_use]
    pub fn with_columns(mut self, columns: &[&str]) -> Self {
        self.columns = Some(columns.iter().map(|&s| s.to_string()).collect());
        self
    }

    /// Specify the primary key columns for the table.
    ///
    /// This is needed when you want to join change tracking results
    /// with the original table to get current row data.
    #[must_use]
    pub fn with_primary_keys(mut self, keys: &[&str]) -> Self {
        self.primary_keys = Some(keys.iter().map(|&s| s.to_string()).collect());
        self
    }

    /// Set the table alias for the CHANGETABLE result.
    #[must_use]
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = alias.into();
        self
    }

    /// Enable FORCESEEK hint for the query.
    ///
    /// This can improve performance in some scenarios.
    #[must_use]
    pub fn with_force_seek(mut self) -> Self {
        self.force_seek = true;
        self
    }

    /// Generate the SQL query string.
    ///
    /// This returns a query that can be executed directly.
    #[must_use]
    pub fn to_sql(&self) -> String {
        let force_seek = if self.force_seek { ", FORCESEEK" } else { "" };

        // Build the SELECT column list
        let select_cols = self.build_select_columns();

        format!(
            "SELECT {} FROM CHANGETABLE(CHANGES {}, {}{})",
            select_cols, self.table_name, self.last_sync_version, force_seek
        )
    }

    /// Generate a SQL query that joins with the original table.
    ///
    /// This is useful when you need both the change tracking metadata
    /// and the current row data (for inserts and updates).
    ///
    /// # Arguments
    ///
    /// * `data_columns` - Columns from the data table to include
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTrackingQuery;
    ///
    /// let query = ChangeTrackingQuery::changes("Products", 42)
    ///     .with_primary_keys(&["ProductId"]);
    /// let sql = query.to_sql_with_data(&["Name", "Price", "Stock"]);
    /// assert!(sql.contains("LEFT OUTER JOIN"));
    /// ```
    #[must_use]
    pub fn to_sql_with_data(&self, data_columns: &[&str]) -> String {
        let force_seek = if self.force_seek { ", FORCESEEK" } else { "" };
        let alias = &self.alias;

        // Build change tracking columns
        let ct_cols = format!(
            "{alias}.SYS_CHANGE_VERSION, {alias}.SYS_CHANGE_CREATION_VERSION, \
             {alias}.SYS_CHANGE_OPERATION, {alias}.SYS_CHANGE_COLUMNS, {alias}.SYS_CHANGE_CONTEXT"
        );

        // Build data columns (prefixed with table alias)
        let data_cols: String = data_columns
            .iter()
            .map(|c| format!("T.{c}"))
            .collect::<Vec<_>>()
            .join(", ");

        // Build primary key columns from change tracking
        let pk_cols: String = self
            .primary_keys
            .as_ref()
            .map(|pks| {
                pks.iter()
                    .map(|pk| format!("{alias}.{pk}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        // Build join condition
        let join_condition: String = self
            .primary_keys
            .as_ref()
            .map(|pks| {
                pks.iter()
                    .map(|pk| format!("{alias}.{pk} = T.{pk}"))
                    .collect::<Vec<_>>()
                    .join(" AND ")
            })
            .unwrap_or_else(|| "1=1".into());

        let select_cols = if pk_cols.is_empty() {
            format!("{ct_cols}, {data_cols}")
        } else {
            format!("{ct_cols}, {pk_cols}, {data_cols}")
        };

        format!(
            "SELECT {select_cols} \
             FROM CHANGETABLE(CHANGES {table}, {version}{force_seek}) AS {alias} \
             LEFT OUTER JOIN {table} AS T ON {join_condition}",
            table = self.table_name,
            version = self.last_sync_version,
        )
    }

    fn build_select_columns(&self) -> String {
        let alias = &self.alias;

        // Always include change tracking system columns
        let mut cols = vec![
            format!("{alias}.SYS_CHANGE_VERSION"),
            format!("{alias}.SYS_CHANGE_CREATION_VERSION"),
            format!("{alias}.SYS_CHANGE_OPERATION"),
            format!("{alias}.SYS_CHANGE_COLUMNS"),
            format!("{alias}.SYS_CHANGE_CONTEXT"),
        ];

        // Add primary key columns if specified
        if let Some(ref pks) = self.primary_keys {
            for pk in pks {
                cols.push(format!("{alias}.{pk}"));
            }
        }

        // Add data columns if specified
        if let Some(ref data_cols) = self.columns {
            for col in data_cols {
                cols.push(format!("{alias}.{col}"));
            }
        }

        cols.join(", ")
    }
}

/// Helper functions for Change Tracking operations.
pub struct ChangeTracking;

impl ChangeTracking {
    /// Generate SQL to get the current change tracking version.
    ///
    /// Returns the global change tracking version number.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTracking;
    ///
    /// let sql = ChangeTracking::current_version_sql();
    /// assert_eq!(sql, "SELECT CHANGE_TRACKING_CURRENT_VERSION()");
    /// ```
    #[must_use]
    pub const fn current_version_sql() -> &'static str {
        "SELECT CHANGE_TRACKING_CURRENT_VERSION()"
    }

    /// Generate SQL to get the minimum valid version for a table.
    ///
    /// If a client's last sync version is less than this, it must
    /// perform a full re-sync instead of incremental sync.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The name of the table
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTracking;
    ///
    /// let sql = ChangeTracking::min_valid_version_sql("Products");
    /// assert!(sql.contains("CHANGE_TRACKING_MIN_VALID_VERSION"));
    /// ```
    #[must_use]
    pub fn min_valid_version_sql(table_name: &str) -> String {
        format!("SELECT CHANGE_TRACKING_MIN_VALID_VERSION(OBJECT_ID(N'{table_name}'))")
    }

    /// Generate SQL to check if a column is in a change mask.
    ///
    /// Used to determine which specific columns changed in an update operation.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name
    /// * `column_name` - The column to check
    /// * `mask_variable` - The name of the variable holding the change mask
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTracking;
    ///
    /// let sql = ChangeTracking::column_in_mask_sql("Products", "Price", "@mask");
    /// assert!(sql.contains("CHANGE_TRACKING_IS_COLUMN_IN_MASK"));
    /// ```
    #[must_use]
    pub fn column_in_mask_sql(table_name: &str, column_name: &str, mask_variable: &str) -> String {
        format!(
            "SELECT CHANGE_TRACKING_IS_COLUMN_IN_MASK(\
             COLUMNPROPERTY(OBJECT_ID(N'{table_name}'), N'{column_name}', 'ColumnId'), \
             {mask_variable})"
        )
    }

    /// Generate SQL to enable change tracking on a database.
    ///
    /// # Arguments
    ///
    /// * `database_name` - The database name
    /// * `retention_days` - How long to retain change data
    /// * `auto_cleanup` - Whether to automatically clean up old data
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTracking;
    ///
    /// let sql = ChangeTracking::enable_database_sql("MyDB", 2, true);
    /// assert!(sql.contains("SET CHANGE_TRACKING = ON"));
    /// ```
    #[must_use]
    pub fn enable_database_sql(
        database_name: &str,
        retention_days: u32,
        auto_cleanup: bool,
    ) -> String {
        let cleanup = if auto_cleanup { "ON" } else { "OFF" };
        format!(
            "ALTER DATABASE [{database_name}] SET CHANGE_TRACKING = ON \
             (CHANGE_RETENTION = {retention_days} DAYS, AUTO_CLEANUP = {cleanup})"
        )
    }

    /// Generate SQL to enable change tracking on a table.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name
    /// * `track_columns_updated` - Whether to track which columns were updated
    ///
    /// # Example
    ///
    /// ```rust
    /// use mssql_client::change_tracking::ChangeTracking;
    ///
    /// let sql = ChangeTracking::enable_table_sql("Products", true);
    /// assert!(sql.contains("ENABLE CHANGE_TRACKING"));
    /// ```
    #[must_use]
    pub fn enable_table_sql(table_name: &str, track_columns_updated: bool) -> String {
        let track_cols = if track_columns_updated { "ON" } else { "OFF" };
        format!(
            "ALTER TABLE [{table_name}] ENABLE CHANGE_TRACKING \
             WITH (TRACK_COLUMNS_UPDATED = {track_cols})"
        )
    }

    /// Generate SQL to disable change tracking on a table.
    #[must_use]
    pub fn disable_table_sql(table_name: &str) -> String {
        format!("ALTER TABLE [{table_name}] DISABLE CHANGE_TRACKING")
    }

    /// Generate SQL to disable change tracking on a database.
    #[must_use]
    pub fn disable_database_sql(database_name: &str) -> String {
        format!("ALTER DATABASE [{database_name}] SET CHANGE_TRACKING = OFF")
    }
}

/// Result of checking if a sync version is still valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SyncVersionStatus {
    /// The sync version is valid and incremental sync can proceed.
    Valid,
    /// The sync version is too old; a full re-sync is required.
    TooOld,
    /// Change tracking is not enabled or the table doesn't exist.
    NotEnabled,
}

impl SyncVersionStatus {
    /// Check sync version validity from the min_valid_version result.
    ///
    /// # Arguments
    ///
    /// * `last_sync_version` - The client's last synchronized version
    /// * `min_valid_version` - Result from `CHANGE_TRACKING_MIN_VALID_VERSION()`
    ///
    /// # Returns
    ///
    /// The sync status indicating whether incremental sync is possible.
    #[must_use]
    pub fn check(last_sync_version: i64, min_valid_version: Option<i64>) -> Self {
        match min_valid_version {
            None => Self::NotEnabled,
            Some(min) if last_sync_version >= min => Self::Valid,
            Some(_) => Self::TooOld,
        }
    }

    /// Check if incremental sync is possible.
    #[must_use]
    pub const fn can_sync_incrementally(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Check if a full re-sync is required.
    #[must_use]
    pub const fn requires_full_sync(&self) -> bool {
        matches!(self, Self::TooOld)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_operation_from_sql() {
        assert_eq!(
            ChangeOperation::from_sql("I"),
            Some(ChangeOperation::Insert)
        );
        assert_eq!(
            ChangeOperation::from_sql("U"),
            Some(ChangeOperation::Update)
        );
        assert_eq!(
            ChangeOperation::from_sql("D"),
            Some(ChangeOperation::Delete)
        );
        assert_eq!(
            ChangeOperation::from_sql("i"),
            Some(ChangeOperation::Insert)
        );
        assert_eq!(
            ChangeOperation::from_sql(" U "),
            Some(ChangeOperation::Update)
        );
        assert_eq!(ChangeOperation::from_sql("X"), None);
        assert_eq!(ChangeOperation::from_sql(""), None);
    }

    #[test]
    fn test_change_operation_as_sql() {
        assert_eq!(ChangeOperation::Insert.as_sql(), "I");
        assert_eq!(ChangeOperation::Update.as_sql(), "U");
        assert_eq!(ChangeOperation::Delete.as_sql(), "D");
    }

    #[test]
    fn test_change_operation_predicates() {
        assert!(ChangeOperation::Insert.is_insert());
        assert!(!ChangeOperation::Insert.is_update());
        assert!(!ChangeOperation::Insert.is_delete());

        assert!(!ChangeOperation::Update.is_insert());
        assert!(ChangeOperation::Update.is_update());
        assert!(!ChangeOperation::Update.is_delete());

        assert!(!ChangeOperation::Delete.is_insert());
        assert!(!ChangeOperation::Delete.is_update());
        assert!(ChangeOperation::Delete.is_delete());
    }

    #[test]
    fn test_change_metadata_constructors() {
        let insert = ChangeMetadata::insert(42);
        assert_eq!(insert.version, 42);
        assert_eq!(insert.creation_version, Some(42));
        assert_eq!(insert.operation, ChangeOperation::Insert);

        let update = ChangeMetadata::update(50, 42);
        assert_eq!(update.version, 50);
        assert_eq!(update.creation_version, Some(42));
        assert_eq!(update.operation, ChangeOperation::Update);

        let delete = ChangeMetadata::delete(60);
        assert_eq!(delete.version, 60);
        assert_eq!(delete.creation_version, None);
        assert_eq!(delete.operation, ChangeOperation::Delete);
    }

    #[test]
    fn test_change_tracking_query_basic() {
        let query = ChangeTrackingQuery::changes("Products", 42);
        let sql = query.to_sql();

        assert!(sql.contains("CHANGETABLE(CHANGES Products, 42)"));
        assert!(sql.contains("SYS_CHANGE_VERSION"));
        assert!(sql.contains("SYS_CHANGE_OPERATION"));
    }

    #[test]
    fn test_change_tracking_query_with_columns() {
        let query = ChangeTrackingQuery::changes("Products", 42).with_columns(&["Name", "Price"]);
        let sql = query.to_sql();

        assert!(sql.contains("CT.Name"));
        assert!(sql.contains("CT.Price"));
    }

    #[test]
    fn test_change_tracking_query_with_primary_keys() {
        let query = ChangeTrackingQuery::changes("Products", 42).with_primary_keys(&["ProductId"]);
        let sql = query.to_sql();

        assert!(sql.contains("CT.ProductId"));
    }

    #[test]
    fn test_change_tracking_query_force_seek() {
        let query = ChangeTrackingQuery::changes("Products", 42).with_force_seek();
        let sql = query.to_sql();

        assert!(sql.contains("FORCESEEK"));
    }

    #[test]
    fn test_change_tracking_query_with_data() {
        let query = ChangeTrackingQuery::changes("Products", 42).with_primary_keys(&["ProductId"]);
        let sql = query.to_sql_with_data(&["Name", "Price"]);

        assert!(sql.contains("LEFT OUTER JOIN Products AS T"));
        assert!(sql.contains("CT.ProductId = T.ProductId"));
        assert!(sql.contains("T.Name"));
        assert!(sql.contains("T.Price"));
    }

    #[test]
    fn test_change_tracking_helper_sql() {
        assert_eq!(
            ChangeTracking::current_version_sql(),
            "SELECT CHANGE_TRACKING_CURRENT_VERSION()"
        );

        let min_sql = ChangeTracking::min_valid_version_sql("Products");
        assert!(min_sql.contains("CHANGE_TRACKING_MIN_VALID_VERSION"));
        assert!(min_sql.contains("Products"));

        let mask_sql = ChangeTracking::column_in_mask_sql("Products", "Price", "@mask");
        assert!(mask_sql.contains("CHANGE_TRACKING_IS_COLUMN_IN_MASK"));
        assert!(mask_sql.contains("Price"));
        assert!(mask_sql.contains("@mask"));
    }

    #[test]
    fn test_change_tracking_enable_sql() {
        let db_sql = ChangeTracking::enable_database_sql("MyDB", 7, true);
        assert!(db_sql.contains("[MyDB]"));
        assert!(db_sql.contains("7 DAYS"));
        assert!(db_sql.contains("AUTO_CLEANUP = ON"));

        let table_sql = ChangeTracking::enable_table_sql("Products", true);
        assert!(table_sql.contains("[Products]"));
        assert!(table_sql.contains("TRACK_COLUMNS_UPDATED = ON"));
    }

    #[test]
    fn test_sync_version_status() {
        // Valid case
        assert!(SyncVersionStatus::check(100, Some(50)).can_sync_incrementally());
        assert!(SyncVersionStatus::check(50, Some(50)).can_sync_incrementally());

        // Too old case
        assert!(SyncVersionStatus::check(40, Some(50)).requires_full_sync());

        // Not enabled case
        let status = SyncVersionStatus::check(100, None);
        assert_eq!(status, SyncVersionStatus::NotEnabled);
        assert!(!status.can_sync_incrementally());
    }
}
