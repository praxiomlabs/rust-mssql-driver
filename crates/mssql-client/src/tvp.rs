//! Table-Valued Parameters (TVP) support.
//!
//! TVPs allow passing collections of structured data to SQL Server stored procedures
//! as a parameter. This is more efficient than:
//! - Multiple INSERT statements
//! - String concatenation of values
//! - Temporary tables
//!
//! ## Usage
//!
//! First, create a table type in SQL Server:
//!
//! ```sql
//! CREATE TYPE dbo.UserIdList AS TABLE (
//!     UserId INT NOT NULL
//! );
//! ```
//!
//! Then use the `#[derive(Tvp)]` macro:
//!
//! ```rust,ignore
//! use mssql_derive::Tvp;
//!
//! #[derive(Tvp)]
//! #[mssql(type_name = "dbo.UserIdList")]
//! struct UserIdList {
//!     user_id: i32,
//! }
//!
//! // Create a collection of rows
//! let user_ids = vec![
//!     UserIdList { user_id: 1 },
//!     UserIdList { user_id: 2 },
//!     UserIdList { user_id: 3 },
//! ];
//!
//! // Pass to stored procedure
//! client.execute(
//!     "EXEC GetUserDetails @UserIds = @user_ids",
//!     &[&TvpValue::new(&user_ids)?],
//! ).await?;
//! ```
//!
//! ## Supported Attributes
//!
//! - `#[mssql(type_name = "schema.TypeName")]` - SQL Server TVP type name (required)
//! - `#[mssql(rename = "column_name")]` - Map field to different column name

use mssql_types::{SqlValue, ToSql, TypeError};

/// Metadata for a TVP column.
#[derive(Debug, Clone)]
pub struct TvpColumn {
    /// Column name.
    pub name: String,
    /// SQL type name (e.g., "INT", "NVARCHAR(100)").
    pub sql_type: String,
    /// Column ordinal (0-based).
    pub ordinal: usize,
}

impl TvpColumn {
    /// Create a new TVP column definition.
    pub fn new<S: Into<String>>(name: S, sql_type: S, ordinal: usize) -> Self {
        Self {
            name: name.into(),
            sql_type: sql_type.into(),
            ordinal,
        }
    }
}

/// A row in a table-valued parameter.
#[derive(Debug, Clone)]
pub struct TvpRow {
    /// Values for each column.
    pub values: Vec<SqlValue>,
}

impl TvpRow {
    /// Create a new TVP row from values.
    pub fn new(values: Vec<SqlValue>) -> Self {
        Self { values }
    }

    /// Get the value at the given index.
    pub fn get(&self, index: usize) -> Option<&SqlValue> {
        self.values.get(index)
    }

    /// Get the number of columns in this row.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the row is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Trait for types that can be used as table-valued parameters.
///
/// This trait is typically implemented via the `#[derive(Tvp)]` macro.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_client::{Tvp, TvpColumn, TvpRow};
/// use mssql_types::{SqlValue, TypeError, ToSql};
///
/// struct UserId {
///     user_id: i32,
/// }
///
/// impl Tvp for UserId {
///     fn type_name() -> &'static str {
///         "dbo.UserIdList"
///     }
///
///     fn columns() -> Vec<TvpColumn> {
///         vec![TvpColumn::new("UserId", "INT", 0)]
///     }
///
///     fn to_row(&self) -> Result<TvpRow, TypeError> {
///         Ok(TvpRow::new(vec![self.user_id.to_sql()?]))
///     }
/// }
/// ```
pub trait Tvp {
    /// Get the SQL Server type name for this TVP.
    ///
    /// This must match a user-defined table type in the database.
    fn type_name() -> &'static str;

    /// Get the column definitions for this TVP.
    fn columns() -> Vec<TvpColumn>;

    /// Convert this struct to a TVP row.
    ///
    /// # Errors
    ///
    /// Returns an error if any field value cannot be converted to a SQL value.
    fn to_row(&self) -> Result<TvpRow, TypeError>;
}

/// A table-valued parameter value that can be passed to a stored procedure.
///
/// This wraps a collection of `Tvp` items and provides the necessary metadata
/// for the TDS protocol.
#[derive(Debug, Clone)]
pub struct TvpValue {
    /// The SQL Server type name.
    pub type_name: String,
    /// Column definitions.
    pub columns: Vec<TvpColumn>,
    /// The rows of data.
    pub rows: Vec<TvpRow>,
}

impl TvpValue {
    /// Create a TVP value from a slice of items implementing `Tvp`.
    ///
    /// # Errors
    ///
    /// Returns an error if any item cannot be converted to a row.
    pub fn new<T: Tvp>(items: &[T]) -> Result<Self, TypeError> {
        let rows: Result<Vec<TvpRow>, TypeError> = items.iter().map(|item| item.to_row()).collect();

        Ok(Self {
            type_name: T::type_name().to_string(),
            columns: T::columns(),
            rows: rows?,
        })
    }

    /// Create an empty TVP value with the given type name and columns.
    pub fn empty<T: Tvp>() -> Self {
        Self {
            type_name: T::type_name().to_string(),
            columns: T::columns(),
            rows: Vec::new(),
        }
    }

    /// Get the number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if the TVP is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Iterate over the rows.
    pub fn iter(&self) -> impl Iterator<Item = &TvpRow> {
        self.rows.iter()
    }
}

impl ToSql for TvpValue {
    fn to_sql(&self) -> Result<SqlValue, TypeError> {
        // For now, TVP values are represented as a special binary type
        // that will be handled specially during parameter encoding.
        // The actual encoding will be done in the RPC layer.

        // We serialize the TVP metadata and rows into a structured format
        // that can be later decoded by the RPC encoder.
        //
        // Format: JSON-like structure (for now, will be optimized)
        // This is a placeholder - real implementation will use TDS-specific encoding
        let serialized = format!(
            "TVP:{}:{}",
            self.type_name,
            self.rows.len()
        );

        Ok(SqlValue::String(serialized))
    }

    fn sql_type(&self) -> &'static str {
        "TVP"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestUserId {
        user_id: i32,
    }

    impl Tvp for TestUserId {
        fn type_name() -> &'static str {
            "dbo.UserIdList"
        }

        fn columns() -> Vec<TvpColumn> {
            vec![TvpColumn::new("UserId", "INT", 0)]
        }

        fn to_row(&self) -> Result<TvpRow, TypeError> {
            Ok(TvpRow::new(vec![self.user_id.to_sql()?]))
        }
    }

    #[test]
    fn test_tvp_trait_impl() {
        assert_eq!(TestUserId::type_name(), "dbo.UserIdList");

        let columns = TestUserId::columns();
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].name, "UserId");
        assert_eq!(columns[0].sql_type, "INT");
    }

    #[test]
    fn test_tvp_row_creation() {
        let item = TestUserId { user_id: 42 };
        let row = item.to_row().unwrap();

        assert_eq!(row.len(), 1);
        assert!(matches!(row.get(0), Some(SqlValue::Int(42))));
    }

    #[test]
    fn test_tvp_value_creation() {
        let items = vec![
            TestUserId { user_id: 1 },
            TestUserId { user_id: 2 },
            TestUserId { user_id: 3 },
        ];

        let tvp = TvpValue::new(&items).unwrap();

        assert_eq!(tvp.type_name, "dbo.UserIdList");
        assert_eq!(tvp.columns.len(), 1);
        assert_eq!(tvp.len(), 3);
    }

    #[test]
    fn test_tvp_value_empty() {
        let tvp: TvpValue = TvpValue::empty::<TestUserId>();

        assert_eq!(tvp.type_name, "dbo.UserIdList");
        assert!(tvp.is_empty());
    }

    #[test]
    fn test_tvp_column() {
        let col = TvpColumn::new("TestCol", "NVARCHAR(100)", 0);

        assert_eq!(col.name, "TestCol");
        assert_eq!(col.sql_type, "NVARCHAR(100)");
        assert_eq!(col.ordinal, 0);
    }
}
