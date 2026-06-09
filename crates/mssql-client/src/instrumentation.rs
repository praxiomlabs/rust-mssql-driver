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
//! Build the driver with the `otel` feature (`cargo add mssql-client --features
//! otel`). Spans and metrics are then emitted automatically for connections,
//! queries, executes, transactions, and pool operations; with the feature off
//! the instrumentation compiles to no-ops at zero cost.
//!
//! This crate emits OpenTelemetry telemetry but does not configure an exporter —
//! install a tracer/meter provider in your application using the
//! `opentelemetry`, `opentelemetry_sdk`, and an exporter crate (e.g.
//! `opentelemetry-otlp` to an OTLP collector such as Jaeger), then drive the
//! `tracing` <-> OpenTelemetry bridge with `tracing-opentelemetry`. See those
//! crates' docs for provider setup.
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
//!
//! `db.statement` is sanitized by default ([`SanitizationConfig`]) so literal
//! parameter values are replaced with placeholders before being recorded; opt
//! out with [`SanitizationConfig::no_sanitization`] only when capturing raw SQL
//! is acceptable.
//!
//! ## Troubleshooting
//!
//! - **No spans appear** — confirm the `otel` feature is enabled and a tracer
//!   provider is installed before the first operation.
//! - **`db.rows_affected` missing** — it is only recorded on mutating
//!   statements, not on `SELECT`.
//! - **High attribute cardinality** — keep sanitization on and avoid adding
//!   per-row custom attributes.

#[cfg(feature = "otel")]
use opentelemetry::{
    KeyValue, global,
    trace::{Span, SpanKind, Status, Tracer},
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
            KeyValue::new(attributes::SERVER_PORT, i64::from(self.server_port)),
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
        attrs.push(KeyValue::new(
            "db.connection_string.host",
            self.server_address.clone(),
        ));

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
        attrs.push(KeyValue::new(
            attributes::DB_OPERATION,
            operation.to_string(),
        ));

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

// =============================================================================
// OpenTelemetry Metrics Support
// =============================================================================

/// Metric names following OpenTelemetry semantic conventions.
pub mod metric_names {
    /// Gauge: Number of connections currently in use.
    pub const DB_CLIENT_CONNECTIONS_USAGE: &str = "db.client.connections.usage";
    /// Gauge: Number of idle connections in the pool.
    pub const DB_CLIENT_CONNECTIONS_IDLE: &str = "db.client.connections.idle";
    /// Gauge: Maximum connections allowed in the pool.
    pub const DB_CLIENT_CONNECTIONS_MAX: &str = "db.client.connections.max";
    /// Counter: Total number of connections created.
    pub const DB_CLIENT_CONNECTIONS_CREATE_TOTAL: &str = "db.client.connections.create.total";
    /// Counter: Total number of connections closed.
    pub const DB_CLIENT_CONNECTIONS_CLOSE_TOTAL: &str = "db.client.connections.close.total";
    /// Histogram: Duration of database operations (queries, executes).
    pub const DB_CLIENT_OPERATION_DURATION: &str = "db.client.operation.duration";
    /// Counter: Total number of operations performed.
    pub const DB_CLIENT_OPERATIONS_TOTAL: &str = "db.client.operations.total";
    /// Counter: Total number of operation errors.
    pub const DB_CLIENT_ERRORS_TOTAL: &str = "db.client.errors.total";
    /// Histogram: Time spent waiting for a connection from the pool.
    pub const DB_CLIENT_CONNECTIONS_WAIT_TIME: &str = "db.client.connections.wait_time";
}

/// Database metrics collector using OpenTelemetry.
#[cfg(feature = "otel")]
pub struct DatabaseMetrics {
    /// Connection usage gauge.
    connections_usage: opentelemetry::metrics::Gauge<u64>,
    /// Idle connections gauge.
    connections_idle: opentelemetry::metrics::Gauge<u64>,
    /// Max connections gauge.
    connections_max: opentelemetry::metrics::Gauge<u64>,
    /// Connections created counter.
    connections_create_total: opentelemetry::metrics::Counter<u64>,
    /// Connections closed counter.
    connections_close_total: opentelemetry::metrics::Counter<u64>,
    /// Operation duration histogram.
    operation_duration: opentelemetry::metrics::Histogram<f64>,
    /// Total operations counter.
    operations_total: opentelemetry::metrics::Counter<u64>,
    /// Error counter.
    errors_total: opentelemetry::metrics::Counter<u64>,
    /// Connection wait time histogram.
    connections_wait_time: opentelemetry::metrics::Histogram<f64>,
    /// Base attributes for all metrics.
    base_attributes: Vec<opentelemetry::KeyValue>,
}

#[cfg(feature = "otel")]
impl DatabaseMetrics {
    /// Create a new metrics collector.
    ///
    /// # Arguments
    ///
    /// * `pool_name` - Optional name to identify this pool in metrics
    /// * `server_address` - Server hostname
    /// * `server_port` - Server port
    pub fn new(pool_name: Option<&str>, server_address: &str, server_port: u16) -> Self {
        use opentelemetry::{KeyValue, global};

        let meter = global::meter("mssql-client");

        let connections_usage = meter
            .u64_gauge(metric_names::DB_CLIENT_CONNECTIONS_USAGE)
            .with_description("Number of connections currently in use")
            .with_unit("connections")
            .build();

        let connections_idle = meter
            .u64_gauge(metric_names::DB_CLIENT_CONNECTIONS_IDLE)
            .with_description("Number of idle connections available")
            .with_unit("connections")
            .build();

        let connections_max = meter
            .u64_gauge(metric_names::DB_CLIENT_CONNECTIONS_MAX)
            .with_description("Maximum number of connections allowed")
            .with_unit("connections")
            .build();

        let connections_create_total = meter
            .u64_counter(metric_names::DB_CLIENT_CONNECTIONS_CREATE_TOTAL)
            .with_description("Total number of connections created")
            .with_unit("connections")
            .build();

        let connections_close_total = meter
            .u64_counter(metric_names::DB_CLIENT_CONNECTIONS_CLOSE_TOTAL)
            .with_description("Total number of connections closed")
            .with_unit("connections")
            .build();

        let operation_duration = meter
            .f64_histogram(metric_names::DB_CLIENT_OPERATION_DURATION)
            .with_description("Duration of database operations")
            .with_unit("s")
            .build();

        let operations_total = meter
            .u64_counter(metric_names::DB_CLIENT_OPERATIONS_TOTAL)
            .with_description("Total number of database operations")
            .with_unit("operations")
            .build();

        let errors_total = meter
            .u64_counter(metric_names::DB_CLIENT_ERRORS_TOTAL)
            .with_description("Total number of operation errors")
            .with_unit("errors")
            .build();

        let connections_wait_time = meter
            .f64_histogram(metric_names::DB_CLIENT_CONNECTIONS_WAIT_TIME)
            .with_description("Time spent waiting for a connection")
            .with_unit("s")
            .build();

        let mut base_attributes = vec![
            KeyValue::new(attributes::DB_SYSTEM, DB_SYSTEM),
            KeyValue::new(attributes::SERVER_ADDRESS, server_address.to_string()),
            KeyValue::new(attributes::SERVER_PORT, i64::from(server_port)),
        ];

        if let Some(name) = pool_name {
            base_attributes.push(KeyValue::new("db.client.pool.name", name.to_string()));
        }

        Self {
            connections_usage,
            connections_idle,
            connections_max,
            connections_create_total,
            connections_close_total,
            operation_duration,
            operations_total,
            errors_total,
            connections_wait_time,
            base_attributes,
        }
    }

    /// Record pool connection status.
    pub fn record_pool_status(&self, in_use: u64, idle: u64, max: u64) {
        self.connections_usage.record(in_use, &self.base_attributes);
        self.connections_idle.record(idle, &self.base_attributes);
        self.connections_max.record(max, &self.base_attributes);
    }

    /// Record a connection being created.
    pub fn record_connection_created(&self) {
        self.connections_create_total.add(1, &self.base_attributes);
    }

    /// Record a connection being closed.
    pub fn record_connection_closed(&self) {
        self.connections_close_total.add(1, &self.base_attributes);
    }

    /// Record an operation duration.
    pub fn record_operation(&self, operation: &str, duration_seconds: f64, success: bool) {
        use opentelemetry::KeyValue;

        let mut attrs = self.base_attributes.clone();
        attrs.push(KeyValue::new(
            attributes::DB_OPERATION,
            operation.to_string(),
        ));
        attrs.push(KeyValue::new("db.operation.success", success));

        self.operations_total.add(1, &attrs);
        self.operation_duration.record(duration_seconds, &attrs);

        if !success {
            self.errors_total.add(1, &attrs);
        }
    }

    /// Record time spent waiting for a connection from the pool.
    pub fn record_connection_wait(&self, duration_seconds: f64) {
        self.connections_wait_time
            .record(duration_seconds, &self.base_attributes);
    }
}

/// No-op metrics collector when otel feature is disabled.
#[cfg(not(feature = "otel"))]
#[derive(Debug, Clone, Default)]
pub struct DatabaseMetrics;

#[cfg(not(feature = "otel"))]
impl DatabaseMetrics {
    /// Create a new no-op metrics collector.
    #[must_use]
    pub fn new(_pool_name: Option<&str>, _server_address: &str, _server_port: u16) -> Self {
        Self
    }

    /// Record pool status (no-op).
    pub fn record_pool_status(&self, _in_use: u64, _idle: u64, _max: u64) {}

    /// Record connection created (no-op).
    pub fn record_connection_created(&self) {}

    /// Record connection closed (no-op).
    pub fn record_connection_closed(&self) {}

    /// Record operation (no-op).
    pub fn record_operation(&self, _operation: &str, _duration_seconds: f64, _success: bool) {}

    /// Record connection wait time (no-op).
    pub fn record_connection_wait(&self, _duration_seconds: f64) {}
}

/// Helper for timing operations.
#[derive(Debug, Clone)]
pub struct OperationTimer {
    start: std::time::Instant,
    operation: &'static str,
}

impl OperationTimer {
    /// Start timing an operation.
    #[must_use]
    pub fn start(operation: &'static str) -> Self {
        Self {
            start: std::time::Instant::now(),
            operation,
        }
    }

    /// Get the elapsed time in seconds.
    #[must_use]
    pub fn elapsed_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Get the operation name.
    #[must_use]
    pub fn operation(&self) -> &'static str {
        self.operation
    }

    /// Finish timing and record the metric.
    #[cfg(feature = "otel")]
    pub fn finish(self, metrics: &DatabaseMetrics, success: bool) {
        metrics.record_operation(self.operation, self.elapsed_seconds(), success);
    }

    /// Finish timing (no-op when otel is disabled).
    #[cfg(not(feature = "otel"))]
    pub fn finish(self, _metrics: &DatabaseMetrics, _success: bool) {}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
