//! SQL identifier validation utilities.
//!
//! Shared validation for SQL identifiers (table names, procedure names,
//! savepoint names, column names) to prevent SQL injection.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Error;

/// Regex pattern for valid SQL Server identifiers.
///
/// Must start with a letter or underscore, followed by up to 127 characters
/// of alphanumerics, underscore, @, #, or $.
#[allow(clippy::expect_used)] // Static regex compilation with known-valid pattern
static IDENTIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_@#$]{0,127}$").expect("valid regex"));

/// Validate a single SQL identifier to prevent SQL injection.
///
/// Returns an error if the identifier is empty, starts with a digit,
/// contains invalid characters, or exceeds 128 characters.
///
/// # Examples
///
/// ```rust,ignore
/// validate_identifier("my_table")?;     // OK
/// validate_identifier("sp_test")?;      // OK
/// validate_identifier("123abc")?;       // Error: starts with digit
/// validate_identifier("table name")?;   // Error: contains space
/// ```
pub(crate) fn validate_identifier(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::InvalidIdentifier(
            "identifier cannot be empty".into(),
        ));
    }

    if !IDENTIFIER_RE.is_match(name) {
        return Err(Error::InvalidIdentifier(format!(
            "invalid identifier '{name}': must start with letter/underscore, \
             contain only alphanumerics/_/@/#/$, and be 1-128 characters"
        )));
    }

    Ok(())
}

/// Validate a potentially schema-qualified identifier.
///
/// Splits on `.` and validates each part individually. SQL Server allows
/// up to 4 parts: `server.catalog.schema.object`.
///
/// # Examples
///
/// ```rust,ignore
/// validate_qualified_identifier("dbo.Users")?;            // OK (2 parts)
/// validate_qualified_identifier("my_proc")?;              // OK (1 part)
/// validate_qualified_identifier("catalog.dbo.Users")?;    // OK (3 parts)
/// validate_qualified_identifier("a.b.c.d.e")?;            // Error: >4 parts
/// validate_qualified_identifier("dbo.123bad")?;           // Error: invalid part
/// ```
pub(crate) fn validate_qualified_identifier(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::InvalidIdentifier(
            "identifier cannot be empty".into(),
        ));
    }

    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() > 4 {
        return Err(Error::InvalidIdentifier(format!(
            "invalid qualified identifier '{name}': too many parts \
             (max 4: server.catalog.schema.object)"
        )));
    }

    for part in &parts {
        validate_identifier(part)?;
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_identifier_valid() {
        assert!(validate_identifier("my_table").is_ok());
        assert!(validate_identifier("Table123").is_ok());
        assert!(validate_identifier("_private").is_ok());
        assert!(validate_identifier("sp_test").is_ok());
        assert!(validate_identifier("column@name").is_ok());
        assert!(validate_identifier("temp#table").is_ok());
    }

    #[test]
    fn test_validate_identifier_invalid() {
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("123abc").is_err());
        assert!(validate_identifier("table-name").is_err());
        assert!(validate_identifier("table name").is_err());
        assert!(validate_identifier("table;DROP TABLE users").is_err());
    }

    #[test]
    fn test_validate_qualified_identifier_valid() {
        assert!(validate_qualified_identifier("dbo.Users").is_ok());
        assert!(validate_qualified_identifier("my_proc").is_ok());
        assert!(validate_qualified_identifier("catalog.dbo.Users").is_ok());
        assert!(validate_qualified_identifier("server.catalog.dbo.Users").is_ok());
    }

    #[test]
    fn test_validate_qualified_identifier_invalid() {
        assert!(validate_qualified_identifier("").is_err());
        assert!(validate_qualified_identifier("a.b.c.d.e").is_err());
        assert!(validate_qualified_identifier("dbo.123bad").is_err());
        assert!(validate_qualified_identifier("dbo.table;DROP").is_err());
    }
}
