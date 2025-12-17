//! ToParams trait for automatic struct-to-parameters mapping.
//!
//! This module provides the `ToParams` trait which enables automatic conversion
//! from Rust structs to named query parameters.
//!
//! ## Derive Macro
//!
//! The recommended way to implement `ToParams` is via the derive macro from
//! `mssql-derive`:
//!
//! ```rust,ignore
//! use mssql_derive::ToParams;
//!
//! #[derive(ToParams)]
//! struct NewUser {
//!     name: String,
//!     #[mssql(rename = "email_address")]
//!     email: String,
//! }
//!
//! let user = NewUser {
//!     name: "Alice".into(),
//!     email: "alice@example.com".into(),
//! };
//!
//! // Use with named parameters in query
//! client.execute(
//!     "INSERT INTO users (name, email_address) VALUES (@name, @email_address)",
//!     &user.to_params(),
//! ).await?;
//! ```
//!
//! ## Supported Attributes
//!
//! - `#[mssql(rename = "param_name")]` - Use a different parameter name
//! - `#[mssql(skip)]` - Skip this field

use mssql_types::{SqlValue, ToSql, TypeError};

/// A named query parameter.
#[derive(Debug, Clone)]
pub struct NamedParam {
    /// Parameter name (without @ prefix).
    pub name: String,
    /// Parameter value.
    pub value: SqlValue,
}

impl NamedParam {
    /// Create a new named parameter.
    pub fn new<S: Into<String>>(name: S, value: SqlValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    /// Create a named parameter from a value implementing ToSql.
    pub fn from_value<S: Into<String>, T: ToSql>(name: S, value: &T) -> Result<Self, TypeError> {
        Ok(Self {
            name: name.into(),
            value: value.to_sql()?,
        })
    }
}

/// Trait for types that can be converted to named query parameters.
///
/// This trait is typically implemented via the `#[derive(ToParams)]` macro,
/// but can also be implemented manually for custom parameter handling.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_client::{ToParams, NamedParam};
/// use mssql_types::{ToSql, TypeError};
///
/// struct NewUser {
///     name: String,
///     email: String,
/// }
///
/// impl ToParams for NewUser {
///     fn to_params(&self) -> Result<Vec<NamedParam>, TypeError> {
///         Ok(vec![
///             NamedParam::from_value("name", &self.name)?,
///             NamedParam::from_value("email", &self.email)?,
///         ])
///     }
/// }
/// ```
pub trait ToParams {
    /// Convert this struct to a vector of named parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if any field value cannot be converted to a SQL value.
    fn to_params(&self) -> Result<Vec<NamedParam>, TypeError>;

    /// Get the number of parameters this struct produces.
    ///
    /// Returns `None` if the count is dynamic.
    fn param_count(&self) -> Option<usize> {
        None
    }
}

/// A list of named parameters that can be used in query execution.
///
/// This is a convenience wrapper around `Vec<NamedParam>` that implements
/// additional utility methods.
#[derive(Debug, Clone, Default)]
pub struct ParamList {
    params: Vec<NamedParam>,
}

impl ParamList {
    /// Create a new empty parameter list.
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Create a parameter list with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            params: Vec::with_capacity(capacity),
        }
    }

    /// Add a parameter to the list.
    pub fn push(&mut self, param: NamedParam) {
        self.params.push(param);
    }

    /// Add a parameter by name and value.
    pub fn add<S: Into<String>, T: ToSql>(&mut self, name: S, value: &T) -> Result<(), TypeError> {
        self.params.push(NamedParam::from_value(name, value)?);
        Ok(())
    }

    /// Get the parameters as a slice.
    pub fn as_slice(&self) -> &[NamedParam] {
        &self.params
    }

    /// Get the number of parameters.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Iterate over the parameters.
    pub fn iter(&self) -> impl Iterator<Item = &NamedParam> {
        self.params.iter()
    }
}

impl From<Vec<NamedParam>> for ParamList {
    fn from(params: Vec<NamedParam>) -> Self {
        Self { params }
    }
}

impl IntoIterator for ParamList {
    type Item = NamedParam;
    type IntoIter = std::vec::IntoIter<NamedParam>;

    fn into_iter(self) -> Self::IntoIter {
        self.params.into_iter()
    }
}

impl<'a> IntoIterator for &'a ParamList {
    type Item = &'a NamedParam;
    type IntoIter = std::slice::Iter<'a, NamedParam>;

    fn into_iter(self) -> Self::IntoIter {
        self.params.iter()
    }
}

impl FromIterator<NamedParam> for ParamList {
    fn from_iter<I: IntoIterator<Item = NamedParam>>(iter: I) -> Self {
        Self {
            params: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct TestParams {
        name: String,
        age: i32,
    }

    impl ToParams for TestParams {
        fn to_params(&self) -> Result<Vec<NamedParam>, TypeError> {
            Ok(vec![
                NamedParam::from_value("name", &self.name)?,
                NamedParam::from_value("age", &self.age)?,
            ])
        }

        fn param_count(&self) -> Option<usize> {
            Some(2)
        }
    }

    #[test]
    fn test_to_params_manual_impl() {
        let params = TestParams {
            name: "Alice".to_string(),
            age: 30,
        };

        let named_params = params.to_params().unwrap();
        assert_eq!(named_params.len(), 2);
        assert_eq!(named_params[0].name, "name");
        assert_eq!(named_params[1].name, "age");
    }

    #[test]
    fn test_named_param_creation() {
        let param = NamedParam::from_value("test", &42i32).unwrap();
        assert_eq!(param.name, "test");
        assert!(matches!(param.value, SqlValue::Int(42)));
    }

    #[test]
    fn test_param_list() {
        let mut list = ParamList::new();
        list.add("name", &"Alice").unwrap();
        list.add("age", &30i32).unwrap();

        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());

        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["name", "age"]);
    }

    #[test]
    fn test_param_list_from_iterator() {
        let params: ParamList = vec![
            NamedParam::new("a", SqlValue::Int(1)),
            NamedParam::new("b", SqlValue::Int(2)),
        ]
        .into_iter()
        .collect();

        assert_eq!(params.len(), 2);
    }
}
