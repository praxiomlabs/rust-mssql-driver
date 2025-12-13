//! OpenTelemetry instrumentation for database operations.
//!
//! This module provides first-class OpenTelemetry tracing support when the
//! `otel` feature is enabled. It follows the OpenTelemetry semantic conventions
//! for database operations.
//!
//! ## Features
//!
//! When the `otel` feature is enabled, the following instrumentation is available:
//!
//! - **Connection spans**: Track connection establishment time and success/failure
//! - **Query spans**: Track SQL execution with sanitized statement attributes
//! - **Transaction spans**: Track transaction boundaries (begin, commit, rollback)
//! - **Error events**: Record errors with appropriate attributes
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_client::{Client, Config};
//! use mssql_client::instrumentation::Instrumented;
//!
//! // Wrap client for automatic instrumentation
//! let client = Client::connect(config).await?.instrumented();
//!
//! // All operations now emit spans
//! client.query("SELECT * FROM users").await?;
//! ```
//!
//! ## Semantic Conventions
//!
//! Follows OpenTelemetry database semantic conventions:
//! - `db.system`: "mssql"
//! - `db.name`: Database name
//! - `db.statement`: SQL statement (sanitized if configured)
//! - `db.operation`: Query operation type (SELECT, INSERT, etc.)
//! - `server.address`: Server hostname
//! - `server.port`: Server port

#[cfg(feature = "otel")]
use opentelemetry::{
    global,
    trace::{Span, SpanKind, Status, Tracer},
    KeyValue,
};

/// Database system identifier for MSSQL.
pub const DB_SYSTEM: &str = "mssql";

/// Span names for database operations.
pub mod span_names {
    /// Span name for connection establishment.
    pub const CONNECT: &str = "mssql.connect";
    /// Span name for query execution.
    pub const QUERY: &str = "mssql.query";
    /// Span name for command execution.
    pub const EXECUTE: &str = "mssql.execute";
    /// Span name for beginning a transaction.
    pub const BEGIN_TRANSACTION: &str = "mssql.begin_transaction";
    /// Span name for committing a transaction.
    pub const COMMIT: &str = "mssql.commit";
    /// Span name for rolling back a transaction.
    pub const ROLLBACK: &str = "mssql.rollback";
    /// Span name for savepoint operations.
    pub const SAVEPOINT: &str = "mssql.savepoint";
    /// Span name for bulk insert operations.
    pub const BULK_INSERT: &str = "mssql.bulk_insert";
}

/// Attribute keys following OpenTelemetry semantic conventions.
pub mod attributes {
    /// Database system type.
    pub const DB_SYSTEM: &str = "db.system";
    /// Database name.
    pub const DB_NAME: &str = "db.name";
    /// SQL statement (may be sanitized).
    pub const DB_STATEMENT: &str = "db.statement";
    /// Database operation type.
    pub const DB_OPERATION: &str = "db.operation";
    /// Server hostname.
    pub const SERVER_ADDRESS: &str = "server.address";
    /// Server port.
    pub const SERVER_PORT: &str = "server.port";
    /// Number of rows affected.
    pub const DB_ROWS_AFFECTED: &str = "db.rows_affected";
    /// Transaction isolation level.
    pub const DB_ISOLATION_LEVEL: &str = "db.mssql.isolation_level";
    /// Connection ID.
    pub const DB_CONNECTION_ID: &str = "db.connection_id";
    /// Error type.
    pub const ERROR_TYPE: &str = "error.type";
}

/// Configuration for SQL statement sanitization.
#[derive(Debug, Clone)]
pub struct SanitizationConfig {
    /// Whether to sanitize SQL statements.
    pub enabled: bool,
    /// Maximum length of statement to record.
    pub max_length: usize,
    /// Placeholder to use for sanitized values.
    pub placeholder: String,
}

impl Default for SanitizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_length: 2048,
            placeholder: "?".to_string(),
        }
    }
}

impl SanitizationConfig {
    /// Create a configuration that doesn't sanitize statements.
    #[must_use]
    pub fn no_sanitization() -> Self {
        Self {
            enabled: false,
            max_length: usize::MAX,
            placeholder: String::new(),
        }
    }

    /// Sanitize a SQL statement according to the configuration.
    #[must_use]
    pub fn sanitize(&self, sql: &str) -> String {
        if !self.enabled {
            return truncate_string(sql, self.max_length);
        }

        // Simple sanitization: replace string literals and numbers
        let sanitized = sanitize_sql(sql, &self.placeholder);
        truncate_string(&sanitized, self.max_length)
    }
}

/// Sanitize SQL by replacing literal values with placeholders.
fn sanitize_sql(sql: &str, placeholder: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut in_string = false;
    let mut string_char = ' ';

    while let Some(c) = chars.next() {
        if in_string {
            if c == string_char {
                // Check for escaped quote
                if chars.peek() == Some(&string_char) {
                    chars.next();
                    continue;
                }
                in_string = false;
                result.push_str(placeholder);
            }
            continue;
        }

        if c == '\'' || c == '"' {
            in_string = true;
            string_char = c;
            continue;
        }

        // Replace numeric literals (simplified)
        if c.is_ascii_digit() && !result.ends_with(|ch: char| ch.is_alphanumeric() || ch == '_') {
            // Skip the number
            while chars
                .peek()
                .is_some_and(|ch| ch.is_ascii_digit() || *ch == '.')
            {
                chars.next();
            }
            result.push_str(placeholder);
            continue;
        }

        result.push(c);
    }

    // If we ended in a string, close it
    if in_string {
        result.push_str(placeholder);
    }

    result
}

/// Truncate a string to a maximum length.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Extract the operation type from a SQL statement.
#[must_use]
pub fn extract_operation(sql: &str) -> &'static str {
    let sql_upper = sql.trim().to_uppercase();

    if sql_upper.starts_with("SELECT") {
        "SELECT"
    } else if sql_upper.starts_with("INSERT") {
        "INSERT"
    } else if sql_upper.starts_with("UPDATE") {
        "UPDATE"
    } else if sql_upper.starts_with("DELETE") {
        "DELETE"
    } else if sql_upper.starts_with("EXEC") || sql_upper.starts_with("EXECUTE") {
        "EXECUTE"
    } else if sql_upper.starts_with("BEGIN TRAN") {
        "BEGIN"
    } else if sql_upper.starts_with("COMMIT") {
        "COMMIT"
    } else if sql_upper.starts_with("ROLLBACK") {
        "ROLLBACK"
    } else if sql_upper.starts_with("CREATE") {
        "CREATE"
    } else if sql_upper.starts_with("ALTER") {
        "ALTER"
    } else if sql_upper.starts_with("DROP") {
        "DROP"
    } else {
        "OTHER"
    }
}

/// Instrumentation context for database operations.
#[cfg(feature = "otel")]
#[derive(Debug, Clone)]
pub struct InstrumentationContext {
    /// Server address.
    pub server_address: String,
    /// Server port.
    pub server_port: u16,
    /// Database name.
    pub database: Option<String>,
    /// Sanitization configuration.
    pub sanitization: SanitizationConfig,
}

#[cfg(feature = "otel")]
impl InstrumentationContext {
    /// Create a new instrumentation context.
    #[must_use]
    pub fn new(server_address: String, server_port: u16) -> Self {
        Self {
            server_address,
            server_port,
            database: None,
            sanitization: SanitizationConfig::default(),
        }
    }

    /// Set the database name.
    #[must_use]
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// Set the sanitization configuration.
    #[must_use]
    pub fn with_sanitization(mut self, config: SanitizationConfig) -> Self {
        self.sanitization = config;
        self
    }

    /// Get base attributes for spans.
    pub fn base_attributes(&self) -> Vec<KeyValue> {
        let mut attrs = vec![
            KeyValue::new(attributes::DB_SYSTEM, DB_SYSTEM),
            KeyValue::new(attributes::SERVER_ADDRESS, self.server_address.clone()),
            KeyValue::new(
                attributes::SERVER_PORT,
                i64::from(self.server_port),
            ),
        ];

        if let Some(ref db) = self.database {
            attrs.push(KeyValue::new(attributes::DB_NAME, db.clone()));
        }

        attrs
    }

    /// Create a connection span.
    pub fn connection_span(&self) -> impl Span {
        let tracer = global::tracer("mssql-client");
        let mut attrs = self.base_attributes();
        attrs.push(KeyValue::new("db.connection_string.host", self.server_address.clone()));

        tracer
            .span_builder(span_names::CONNECT)
            .with_kind(SpanKind::Client)
            .with_attributes(attrs)
            .start(&tracer)
    }

    /// Create a query span.
    pub fn query_span(&self, sql: &str) -> impl Span {
        let tracer = global::tracer("mssql-client");
        let mut attrs = self.base_attributes();

        let operation = extract_operation(sql);
        attrs.push(KeyValue::new(attributes::DB_OPERATION, operation));
        attrs.push(KeyValue::new(
            attributes::DB_STATEMENT,
            self.sanitization.sanitize(sql),
        ));

        tracer
            .span_builder(span_names::QUERY)
            .with_kind(SpanKind::Client)
            .with_attributes(attrs)
            .start(&tracer)
    }

    /// Create a transaction span.
    pub fn transaction_span(&self, operation: &str) -> impl Span {
        let tracer = global::tracer("mssql-client");
        let mut attrs = self.base_attributes();
        attrs.push(KeyValue::new(attributes::DB_OPERATION, operation.to_string()));

        let span_name = match operation {
            "BEGIN" => span_names::BEGIN_TRANSACTION,
            "COMMIT" => span_names::COMMIT,
            "ROLLBACK" => span_names::ROLLBACK,
            _ => span_names::SAVEPOINT,
        };

        tracer
            .span_builder(span_name)
            .with_kind(SpanKind::Client)
            .with_attributes(attrs)
            .start(&tracer)
    }

    /// Record an error on the current span.
    pub fn record_error(span: &mut impl Span, error: &crate::error::Error) {
        span.set_status(Status::error(error.to_string()));
        span.record_error(error);
    }

    /// Record success with optional row count.
    pub fn record_success(span: &mut impl Span, rows_affected: Option<u64>) {
        span.set_status(Status::Ok);
        if let Some(rows) = rows_affected {
            span.set_attribute(KeyValue::new(attributes::DB_ROWS_AFFECTED, rows as i64));
        }
    }
}

/// No-op instrumentation context when otel feature is disabled.
#[cfg(not(feature = "otel"))]
#[derive(Debug, Clone, Default)]
pub struct InstrumentationContext;

#[cfg(not(feature = "otel"))]
impl InstrumentationContext {
    /// Create a new instrumentation context (no-op).
    #[must_use]
    pub fn new(_server_address: String, _server_port: u16) -> Self {
        Self
    }

    /// Set the database name (no-op).
    #[must_use]
    pub fn with_database(self, _database: impl Into<String>) -> Self {
        self
    }

    /// Set the sanitization configuration (no-op).
    #[must_use]
    pub fn with_sanitization(self, _config: SanitizationConfig) -> Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_operation() {
        assert_eq!(extract_operation("SELECT * FROM users"), "SELECT");
        assert_eq!(extract_operation("  select id from users"), "SELECT");
        assert_eq!(extract_operation("INSERT INTO users VALUES (1)"), "INSERT");
        assert_eq!(extract_operation("UPDATE users SET name = 'foo'"), "UPDATE");
        assert_eq!(extract_operation("DELETE FROM users"), "DELETE");
        assert_eq!(extract_operation("EXEC sp_help"), "EXECUTE");
        assert_eq!(extract_operation("BEGIN TRANSACTION"), "BEGIN");
        assert_eq!(extract_operation("COMMIT"), "COMMIT");
        assert_eq!(extract_operation("ROLLBACK"), "ROLLBACK");
        assert_eq!(extract_operation("CREATE TABLE foo"), "CREATE");
        assert_eq!(extract_operation("unknown stuff"), "OTHER");
    }

    #[test]
    fn test_sanitize_sql() {
        let placeholder = "?";

        // String literals
        assert_eq!(
            sanitize_sql("SELECT * FROM users WHERE name = 'Alice'", placeholder),
            "SELECT * FROM users WHERE name = ?"
        );

        // Multiple strings
        assert_eq!(
            sanitize_sql("INSERT INTO t VALUES ('a', 'b')", placeholder),
            "INSERT INTO t VALUES (?, ?)"
        );

        // Escaped quotes
        assert_eq!(
            sanitize_sql("SELECT * WHERE name = 'O''Brien'", placeholder),
            "SELECT * WHERE name = ?"
        );

        // Numbers
        assert_eq!(
            sanitize_sql("SELECT * WHERE id = 123", placeholder),
            "SELECT * WHERE id = ?"
        );

        // Mixed
        assert_eq!(
            sanitize_sql("SELECT * WHERE id = 42 AND name = 'test'", placeholder),
            "SELECT * WHERE id = ? AND name = ?"
        );
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
    }

    #[test]
    fn test_sanitization_config_default() {
        let config = SanitizationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_length, 2048);
        assert_eq!(config.placeholder, "?");
    }

    #[test]
    fn test_sanitization_config_no_sanitization() {
        let config = SanitizationConfig::no_sanitization();
        assert!(!config.enabled);

        let sql = "SELECT * FROM users WHERE name = 'Alice'";
        assert_eq!(config.sanitize(sql), sql);
    }
}
