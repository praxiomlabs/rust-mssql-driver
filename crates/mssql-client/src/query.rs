//! Query helper utilities.

use std::fmt::Write;

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
