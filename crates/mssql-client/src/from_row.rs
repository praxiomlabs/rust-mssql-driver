//! FromRow trait for automatic row-to-struct mapping.
//!
//! This module provides the `FromRow` trait which enables automatic conversion
//! from database rows to Rust structs.
//!
//! ## Derive Macro
//!
//! The recommended way to implement `FromRow` is via the derive macro from
//! `mssql-derive`:
//!
//! ```rust,ignore
//! use mssql_derive::FromRow;
//!
//! #[derive(FromRow)]
//! struct User {
//!     id: i32,
//!     #[mssql(rename = "user_name")]
//!     name: String,
//!     email: Option<String>,
//! }
//! ```
//!
//! ## Supported Attributes
//!
//! - `#[mssql(rename = "column_name")]` - Map field to a different column name
//! - `#[mssql(skip)]` - Skip field, use Default value
//! - `#[mssql(default)]` - Use Default if column not found
//! - `#[mssql(flatten)]` - Flatten nested FromRow structs

use crate::error::Error;
use crate::row::Row;

/// Trait for types that can be constructed from a database row.
///
/// This trait is typically implemented via the `#[derive(FromRow)]` macro,
/// but can also be implemented manually for custom mapping logic.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_client::{FromRow, Row, Error};
///
/// struct User {
///     id: i32,
///     name: String,
/// }
///
/// impl FromRow for User {
///     fn from_row(row: &Row) -> Result<Self, Error> {
///         Ok(Self {
///             id: row.get_by_name("id")?,
///             name: row.get_by_name("name")?,
///         })
///     }
/// }
/// ```
pub trait FromRow: Sized {
    /// Construct an instance of this type from a database row.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A required column is missing
    /// - A column value cannot be converted to the expected Rust type
    /// - Any other mapping error occurs
    fn from_row(row: &Row) -> Result<Self, Error>;
}

/// Extension trait for iterating over query results as typed structs.
///
/// This trait is automatically implemented for any iterator of `Result<Row, Error>`.
pub trait RowIteratorExt: Iterator<Item = Result<Row, Error>> + Sized {
    /// Map each row to a struct implementing `FromRow`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use mssql_client::{FromRow, RowIteratorExt};
    ///
    /// #[derive(FromRow)]
    /// struct User { id: i32, name: String }
    ///
    /// let users: Vec<User> = client
    ///     .query("SELECT id, name FROM users", &[])
    ///     .await?
    ///     .map_rows::<User>()
    ///     .collect::<Result<Vec<_>, _>>()?;
    /// ```
    fn map_rows<T: FromRow>(self) -> MapRows<Self, T>;
}

impl<I: Iterator<Item = Result<Row, Error>>> RowIteratorExt for I {
    fn map_rows<T: FromRow>(self) -> MapRows<Self, T> {
        MapRows {
            inner: self,
            _marker: std::marker::PhantomData,
        }
    }
}

/// Iterator adapter that maps rows to typed structs.
pub struct MapRows<I, T> {
    inner: I,
    _marker: std::marker::PhantomData<T>,
}

impl<I, T> Iterator for MapRows<I, T>
where
    I: Iterator<Item = Result<Row, Error>>,
    T: FromRow,
{
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|result| result.and_then(|row| T::from_row(&row)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::row::Column;
    use mssql_types::SqlValue;

    struct TestUser {
        id: i32,
        name: String,
    }

    impl FromRow for TestUser {
        fn from_row(row: &Row) -> Result<Self, Error> {
            Ok(Self {
                id: row.get_by_name("id").map_err(Error::from)?,
                name: row.get_by_name("name").map_err(Error::from)?,
            })
        }
    }

    #[test]
    fn test_from_row_manual_impl() {
        let columns = vec![
            Column::new("id", 0, "INT".to_string()),
            Column::new("name", 1, "NVARCHAR".to_string()),
        ];
        let row = Row::from_values(
            columns,
            vec![SqlValue::Int(42), SqlValue::String("Alice".to_string())],
        );

        let user = TestUser::from_row(&row).unwrap();
        assert_eq!(user.id, 42);
        assert_eq!(user.name, "Alice");
    }

    #[test]
    fn test_map_rows_iterator() {
        let columns = vec![
            Column::new("id", 0, "INT".to_string()),
            Column::new("name", 1, "NVARCHAR".to_string()),
        ];

        let rows = vec![
            Ok(Row::from_values(
                columns.clone(),
                vec![SqlValue::Int(1), SqlValue::String("Alice".to_string())],
            )),
            Ok(Row::from_values(
                columns.clone(),
                vec![SqlValue::Int(2), SqlValue::String("Bob".to_string())],
            )),
        ];

        let users: Vec<TestUser> = rows
            .into_iter()
            .map_rows::<TestUser>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(users.len(), 2);
        assert_eq!(users[0].id, 1);
        assert_eq!(users[0].name, "Alice");
        assert_eq!(users[1].id, 2);
        assert_eq!(users[1].name, "Bob");
    }
}
