//! Query builder and prepared statement support.

use std::fmt::Write;

use mssql_types::ToSql;

/// A prepared query builder.
///
/// Queries can be built incrementally and reused with different parameters.
#[derive(Debug, Clone)]
pub struct Query {
    sql: String,
    // Placeholder for prepared statement handle and metadata
}

impl Query {
    /// Create a new query from SQL text.
    #[must_use]
    pub fn new(sql: impl Into<String>) -> Self {
        Self { sql: sql.into() }
    }

    /// Get the SQL text.
    #[must_use]
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// Extension trait for building parameterized queries.
pub trait QueryExt {
    /// Add a parameter to the query.
    fn bind<T: ToSql>(self, value: &T) -> BoundQuery<'_>;
}

/// A query with bound parameters.
pub struct BoundQuery<'a> {
    sql: &'a str,
    params: Vec<&'a dyn ToSql>,
}

impl<'a> BoundQuery<'a> {
    /// Create a new bound query.
    pub fn new(sql: &'a str) -> Self {
        Self {
            sql,
            params: Vec::new(),
        }
    }

    /// Add another parameter.
    pub fn bind<T: ToSql>(mut self, value: &'a T) -> Self {
        self.params.push(value);
        self
    }

    /// Get the SQL text.
    #[must_use]
    pub fn sql(&self) -> &str {
        self.sql
    }

    /// Get the bound parameters.
    #[must_use]
    pub fn params(&self) -> &[&dyn ToSql] {
        &self.params
    }
}

/// Generate an IN clause SQL fragment with positional parameters.
///
/// Returns a string like `(@p1, @p2, @p3)` for use in `WHERE column IN (...)`
/// queries. The `start` parameter controls where numbering begins (1-based),
/// allowing composition with other parameters in the same query.
///
/// # Panics
///
/// Panics if `count` is 0 (SQL Server rejects empty IN clauses).
///
/// # Examples
///
/// ```
/// use mssql_client::in_params;
///
/// // Simple: IN clause as the only parameterized part
/// let ids = vec![10i32, 20, 30];
/// let sql = format!("SELECT * FROM users WHERE id IN {}", in_params(1, ids.len()));
/// assert_eq!(sql, "SELECT * FROM users WHERE id IN (@p1, @p2, @p3)");
///
/// // Composed: other params before the IN clause
/// // WHERE status = @p1 AND id IN (@p2, @p3, @p4)
/// let fragment = in_params(2, 3);
/// assert_eq!(fragment, "(@p2, @p3, @p4)");
/// ```
pub fn in_params(start: usize, count: usize) -> String {
    assert!(count > 0, "IN clause requires at least one parameter");
    let mut s = String::with_capacity(count * 5);
    s.push('(');
    for i in 0..count {
        if i > 0 {
            s.push_str(", ");
        }
        // write! on String is infallible
        write!(s, "@p{}", start + i).unwrap();
    }
    s.push(')');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_new() {
        let query = Query::new("SELECT * FROM users");
        assert_eq!(query.sql(), "SELECT * FROM users");
    }

    #[test]
    fn test_query_new_from_string() {
        let sql = String::from("SELECT id FROM products");
        let query = Query::new(sql);
        assert_eq!(query.sql(), "SELECT id FROM products");
    }

    #[test]
    fn test_query_clone() {
        let query = Query::new("SELECT 1");
        let cloned = query.clone();
        assert_eq!(cloned.sql(), "SELECT 1");
    }

    #[test]
    fn test_query_debug() {
        let query = Query::new("SELECT 1");
        let debug = format!("{query:?}");
        assert!(debug.contains("SELECT 1"));
    }

    #[test]
    fn test_bound_query_new() {
        let bound = BoundQuery::new("SELECT * FROM users WHERE id = @p1");
        assert_eq!(bound.sql(), "SELECT * FROM users WHERE id = @p1");
        assert!(bound.params().is_empty());
    }

    #[test]
    fn test_bound_query_bind_single() {
        let id = 42i32;
        let bound = BoundQuery::new("SELECT * FROM users WHERE id = @p1").bind(&id);
        assert_eq!(bound.sql(), "SELECT * FROM users WHERE id = @p1");
        assert_eq!(bound.params().len(), 1);
    }

    #[test]
    fn test_bound_query_bind_multiple() {
        let id = 42i32;
        let name = "Alice";
        let bound = BoundQuery::new("SELECT * FROM users WHERE id = @p1 AND name = @p2")
            .bind(&id)
            .bind(&name);
        assert_eq!(bound.params().len(), 2);
    }

    #[test]
    fn test_bound_query_chained_binds() {
        let a = 1i32;
        let b = 2i32;
        let c = 3i32;
        let bound = BoundQuery::new("INSERT INTO t VALUES (@p1, @p2, @p3)")
            .bind(&a)
            .bind(&b)
            .bind(&c);
        assert_eq!(bound.params().len(), 3);
    }

    #[test]
    fn test_in_params_single() {
        assert_eq!(in_params(1, 1), "(@p1)");
    }

    #[test]
    fn test_in_params_multiple() {
        assert_eq!(in_params(1, 3), "(@p1, @p2, @p3)");
    }

    #[test]
    fn test_in_params_with_offset() {
        assert_eq!(in_params(4, 2), "(@p4, @p5)");
    }

    #[test]
    fn test_in_params_large() {
        let result = in_params(1, 5);
        assert_eq!(result, "(@p1, @p2, @p3, @p4, @p5)");
    }

    #[test]
    fn test_in_params_format_into_sql() {
        let sql = format!(
            "SELECT * FROM users WHERE status = @p1 AND id IN {}",
            in_params(2, 3)
        );
        assert_eq!(
            sql,
            "SELECT * FROM users WHERE status = @p1 AND id IN (@p2, @p3, @p4)"
        );
    }

    #[test]
    #[should_panic(expected = "IN clause requires at least one parameter")]
    fn test_in_params_zero_count_panics() {
        in_params(1, 0);
    }
}
