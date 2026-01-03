# OpenTelemetry Integration

rust-mssql-driver provides optional OpenTelemetry instrumentation for distributed tracing and observability.

## Enabling OpenTelemetry

Add the `otel` feature to your dependency:

```toml
[dependencies]
mssql-client = { version = "0.5", features = ["otel"] }
```

## Automatic Instrumentation

When the `otel` feature is enabled, the driver automatically creates spans for:

- **Connection establishment** - `mssql.connect`
- **Query execution** - `mssql.query`
- **Execute statements** - `mssql.execute`
- **Transactions** - `mssql.transaction`
- **Pool operations** - `mssql.pool.acquire`

## Span Attributes

Each span includes semantic attributes following OpenTelemetry conventions:

| Attribute | Description | Example |
|-----------|-------------|---------|
| `db.system` | Database system identifier | `mssql` |
| `db.name` | Database name | `production` |
| `db.statement` | SQL statement (sanitized) | `SELECT * FROM users WHERE id = ?` |
| `db.operation` | Operation type | `query`, `execute`, `commit` |
| `db.user` | Database user | `app_user` |
| `server.address` | Server hostname | `db.example.com` |
| `server.port` | Server port | `1433` |
| `db.rows_affected` | Rows affected (for mutations) | `42` |

## SQL Sanitization

SQL statements are automatically sanitized to remove potentially sensitive parameter values:

**Original:**
```sql
SELECT * FROM users WHERE email = 'user@example.com' AND password = 'secret'
```

**Sanitized (in spans):**
```sql
SELECT * FROM users WHERE email = ? AND password = ?
```

## Setup Example

```rust
use mssql_client::{Client, Config};
use opentelemetry::global;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize OpenTelemetry
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://localhost:4317");

    let tracer_provider = TracerProvider::builder()
        .with_batch_exporter(exporter.build_span_exporter()?, opentelemetry_sdk::runtime::Tokio)
        .build();

    global::set_tracer_provider(tracer_provider);

    // Set up tracing subscriber with OpenTelemetry layer
    tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer())
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Use the driver - spans are created automatically
    let config = Config::from_connection_string(
        "Server=localhost;Database=test;User Id=sa;Password=Password123!"
    )?;

    let mut client = Client::connect(config).await?;

    // This query creates a span with db.statement attribute
    let rows = client.query("SELECT id, name FROM users WHERE active = @p1", &[&true]).await?;

    for result in rows {
        let row = result?;
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        println!("{}: {}", id, name);
    }

    client.close().await?;

    // Shut down tracer provider
    global::shutdown_tracer_provider();

    Ok(())
}
```

## Jaeger Setup

For local development with Jaeger:

```bash
# Start Jaeger with OTLP support
docker run -d --name jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p 4317:4317 \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest
```

Then access the Jaeger UI at http://localhost:16686.

## Metrics

Metrics instrumentation is available via the `DatabaseMetrics` struct when the `otel` feature is enabled.

### Available Metrics

| Metric Name | Type | Description |
|-------------|------|-------------|
| `db.client.connections.usage` | Gauge | Number of connections currently in use |
| `db.client.connections.idle` | Gauge | Number of idle connections available |
| `db.client.connections.max` | Gauge | Maximum connections allowed in the pool |
| `db.client.connections.create.total` | Counter | Total connections created |
| `db.client.connections.close.total` | Counter | Total connections closed |
| `db.client.operation.duration` | Histogram | Duration of database operations (seconds) |
| `db.client.operations.total` | Counter | Total operations performed |
| `db.client.errors.total` | Counter | Total operation errors |
| `db.client.connections.wait_time` | Histogram | Time spent waiting for a connection |

### Usage Example

```rust
use mssql_client::instrumentation::DatabaseMetrics;

// Create metrics collector (typically done once at pool creation)
let metrics = DatabaseMetrics::new(
    Some("main-pool"),  // Pool name for labeling
    "db.example.com",   // Server address
    1433                // Port
);

// Record pool status (called periodically or on change)
metrics.record_pool_status(5, 10, 20);  // in_use, idle, max

// Record operation timing
metrics.record_operation("SELECT", 0.025, true);  // operation, duration_secs, success

// Record connection wait time
metrics.record_connection_wait(0.003);  // seconds
```

## Performance Impact

When the `otel` feature is disabled (default), there is zero overhead from instrumentation code.

When enabled:
- Span creation: ~1-2μs per operation
- Attribute recording: ~0.5μs per attribute
- Async export: Minimal impact (batched, background)

For high-throughput scenarios, consider:
- Using sampling to reduce span volume
- Batching exports appropriately
- Monitoring trace collector capacity

## Integration with Existing Tracing

The driver uses the `tracing` crate, so it integrates seamlessly with existing tracing infrastructure:

```rust
use tracing::{info_span, Instrument};

async fn process_user(pool: &Pool, user_id: i32) -> Result<(), Error> {
    // Create a parent span
    let span = info_span!("process_user", user_id = user_id);

    async {
        let mut conn = pool.get().await?;

        // Database spans are automatically children of this span
        let rows = conn.query(
            "SELECT * FROM users WHERE id = @p1",
            &[&user_id]
        ).await?;

        // Process results...
        Ok(())
    }
    .instrument(span)
    .await
}
```

## Troubleshooting

### No Spans Appearing

1. Verify the `otel` feature is enabled in Cargo.toml
2. Check that the tracer provider is set before creating connections
3. Ensure the OTLP endpoint is reachable
4. Check collector logs for ingestion errors

### Missing Attributes

Some attributes are only available in certain contexts:
- `db.rows_affected` only appears on mutation operations
- `db.statement` may be truncated for very long queries

### High Cardinality

Avoid including user input directly in span names. The driver sanitizes SQL, but custom attributes should also be sanitized:

```rust
// Good: Low cardinality
span.record("user_type", user.role);

// Bad: High cardinality
span.record("user_email", user.email);  // Creates too many unique spans
```

## Configuration Reference

Environment variables for common collectors:

```bash
# OTLP/gRPC (default)
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# OTLP/HTTP
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318

# Service identification
export OTEL_SERVICE_NAME=my-app
export OTEL_RESOURCE_ATTRIBUTES=deployment.environment=production
```
