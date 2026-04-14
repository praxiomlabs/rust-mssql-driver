# Operations Guide

This document covers operational best practices for deploying rust-mssql-driver in production.

## Graceful Shutdown

Proper shutdown handling prevents connection leaks and ensures transactions complete.

### Basic Shutdown

```rust
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = Pool::builder()
        .max_connections(10)
        .build(config)
        .await?;

    // Start your application
    let app_handle = tokio::spawn(run_application(pool.clone()));

    // Wait for shutdown signal
    signal::ctrl_c().await?;
    tracing::info!("shutdown signal received");

    // Graceful shutdown
    pool.close().await;
    tracing::info!("connection pool closed");

    // Wait for application tasks to complete
    app_handle.await?;

    Ok(())
}
```

### Shutdown with Timeout

```rust
use std::time::Duration;
use tokio::time::timeout;

async fn graceful_shutdown(pool: Pool, app_handle: JoinHandle<()>) {
    tracing::info!("initiating graceful shutdown");

    // Give in-flight requests time to complete
    let shutdown_timeout = Duration::from_secs(30);

    match timeout(shutdown_timeout, async {
        // Signal application to stop accepting new requests
        // ... your application-specific logic ...

        // Close pool (waits for connections to be returned)
        pool.close().await;
    }).await {
        Ok(()) => tracing::info!("graceful shutdown completed"),
        Err(_) => {
            tracing::warn!("shutdown timeout - forcing close");
            pool.force_close();
        }
    }
}
```

### Transaction Safety During Shutdown

```rust
// Use CancellationToken for coordinated shutdown
use tokio_util::sync::CancellationToken;

async fn process_request(
    pool: &Pool,
    cancel: CancellationToken,
) -> Result<(), Error> {
    let mut conn = pool.get().await?;

    let mut tx = conn.begin_transaction().await?;

    // Check cancellation before each operation
    if cancel.is_cancelled() {
        tx.rollback().await?;
        return Err(Error::Cancelled);
    }

    tx.execute("INSERT INTO ...", &[]).await?;

    if cancel.is_cancelled() {
        tx.rollback().await?;
        return Err(Error::Cancelled);
    }

    tx.execute("UPDATE ...", &[]).await?;

    // Commit only if not cancelled
    let _conn = tx.commit().await?;
    Ok(())
}
```

## Connection Pool Configuration

### Production Settings

```rust
use std::time::Duration;
use mssql_pool::{Pool, PoolConfig};

let pool = Pool::builder()
    // Size limits
    .max_connections(20)          // Max connections in pool
    .min_connections(5)           // Keep at least 5 warm connections

    // Timeouts
    .connect_timeout(Duration::from_secs(30))    // Connection establishment
    .acquire_timeout(Duration::from_secs(5))     // Wait for available connection
    .idle_timeout(Duration::from_secs(300))      // Close idle connections after 5 min
    .max_lifetime(Duration::from_secs(1800))     // Recycle connections every 30 min

    // Health checks
    .health_check_interval(Duration::from_secs(30))  // Check idle connections
    .test_on_checkout(true)        // Validate before use

    // Retry behavior
    .retry_policy(RetryPolicy {
        max_retries: 3,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_secs(10),
        backoff_multiplier: 2.0,
        jitter: true,
    })

    .build(config)
    .await?;
```

### Sizing Guidelines

| Application Type | Min Connections | Max Connections |
|-----------------|-----------------|-----------------|
| Low traffic API | 2-5 | 10-20 |
| High traffic API | 10-20 | 50-100 |
| Background workers | 1-2 | 5-10 |
| Batch processing | 2-5 | 20-50 |

**Formula:** Start with `max_connections = 2 * CPU_cores` and adjust based on load testing.

### Azure SQL Specific Settings

```rust
// Azure SQL has connection limits based on tier
let pool = Pool::builder()
    .max_connections(match azure_tier {
        "Basic" => 30,
        "S0" => 60,
        "S1" => 90,
        "S2" => 120,
        "P1" => 200,
        _ => 20,
    } / 2)  // Leave headroom for other apps
    .connect_timeout(Duration::from_secs(60))  // Azure can be slower
    .build(config)
    .await?;
```

## Observability

### Logging Configuration

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn setup_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_level(true)
            .json())  // JSON for log aggregation
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("mssql_client=info".parse().unwrap())
            .add_directive("mssql_pool=info".parse().unwrap())
            .add_directive("mssql_tls=warn".parse().unwrap()))
        .init();
}
```

### Log Levels

| Level | What's Logged |
|-------|---------------|
| `error` | Connection failures, server errors, protocol errors |
| `warn` | Transient failures, retries, insecure config warnings |
| `info` | Connection established/closed, transactions, routing |
| `debug` | Query execution, token parsing, pool operations |
| `trace` | Packet bytes, low-level protocol details |

### OpenTelemetry Integration

Enable the `otel` feature for distributed tracing:

```toml
[dependencies]
mssql-client = { version = "0.5", features = ["otel"] }
```

```rust
use opentelemetry::global;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_otlp::WithExportConfig;

fn setup_tracing() -> Result<(), Box<dyn std::error::Error>> {
    // Configure OTLP exporter
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
        )
        .install_batch(Tokio)?;

    // Set as global tracer
    global::set_tracer_provider(tracer);

    Ok(())
}
```

### Metrics Collection

```rust
use mssql_pool::PoolMetrics;

async fn export_metrics(pool: &Pool) {
    let metrics = pool.metrics();

    // Export to your metrics system (Prometheus, StatsD, etc.)
    gauge!("mssql.pool.size", metrics.pool_size as f64);
    gauge!("mssql.pool.available", metrics.available as f64);
    gauge!("mssql.pool.in_use", metrics.in_use as f64);
    counter!("mssql.pool.checkouts_total", metrics.total_checkouts);
    counter!("mssql.pool.timeouts_total", metrics.total_timeouts);
    histogram!("mssql.pool.checkout_duration_ms", metrics.avg_checkout_time_ms);
}
```

### Prometheus Metrics Endpoint

```rust
use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder, register_gauge, register_counter};

lazy_static! {
    static ref POOL_SIZE: Gauge = register_gauge!(
        "mssql_pool_size",
        "Current number of connections in pool"
    ).unwrap();
    static ref POOL_AVAILABLE: Gauge = register_gauge!(
        "mssql_pool_available",
        "Available connections in pool"
    ).unwrap();
    // ... more metrics
}

async fn metrics_handler(pool: Pool) -> String {
    let metrics = pool.metrics();

    POOL_SIZE.set(metrics.pool_size as f64);
    POOL_AVAILABLE.set(metrics.available as f64);

    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
```

## Health Checks

### Basic Health Check

```rust
async fn health_check(pool: &Pool) -> Result<(), Error> {
    let mut conn = pool.get().await?;
    conn.simple_query("SELECT 1").await?;
    Ok(())
}
```

### Detailed Health Check

```rust
#[derive(serde::Serialize)]
struct HealthStatus {
    status: &'static str,
    pool_size: usize,
    available: usize,
    database: String,
    latency_ms: u64,
    version: String,
}

async fn detailed_health_check(pool: &Pool) -> HealthStatus {
    let start = std::time::Instant::now();

    let (database, version) = match pool.get().await {
        Ok(mut conn) => {
            let db = conn.database().unwrap_or("unknown").to_string();
            let version = match conn.query("SELECT @@VERSION", &[]).await {
                Ok(mut stream) => {
                    if let Some(Ok(row)) = stream.next().await {
                        row.get::<String>(0).unwrap_or_default()
                    } else {
                        "unknown".to_string()
                    }
                }
                Err(_) => "error".to_string(),
            };
            (db, version)
        }
        Err(_) => ("error".to_string(), "error".to_string()),
    };

    let metrics = pool.metrics();
    let latency = start.elapsed().as_millis() as u64;

    HealthStatus {
        status: if metrics.available > 0 { "healthy" } else { "degraded" },
        pool_size: metrics.pool_size,
        available: metrics.available,
        database,
        latency_ms: latency,
        version,
    }
}
```

### Kubernetes Probes

```rust
use axum::{routing::get, Router, response::IntoResponse, http::StatusCode};

async fn liveness() -> impl IntoResponse {
    // Simple liveness - is the process running?
    StatusCode::OK
}

async fn readiness(pool: Pool) -> impl IntoResponse {
    // Readiness - can we serve requests?
    match pool.get().await {
        Ok(mut conn) => {
            match conn.simple_query("SELECT 1").await {
                Ok(_) => StatusCode::OK,
                Err(_) => StatusCode::SERVICE_UNAVAILABLE,
            }
        }
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

let app = Router::new()
    .route("/health/live", get(liveness))
    .route("/health/ready", get(|| readiness(pool.clone())));
```

## Troubleshooting

### Common Issues

#### Connection Timeouts

**Symptoms:** `ConnectTimeout` errors during peak load.

**Causes:**
- Pool exhausted (all connections in use)
- Network issues / firewall blocking
- SQL Server under load

**Solutions:**
```rust
// Increase pool size
.max_connections(50)

// Increase timeouts
.connect_timeout(Duration::from_secs(60))
.acquire_timeout(Duration::from_secs(10))

// Enable connection health checks
.test_on_checkout(true)
```

#### Connection Resets

**Symptoms:** `ConnectionReset` errors sporadically.

**Causes:**
- Idle connections closed by server/firewall
- Network instability
- Azure SQL maintenance

**Solutions:**
```rust
// Reduce idle timeout
.idle_timeout(Duration::from_secs(120))

// Enable keepalives
.keepalive_interval(Duration::from_secs(30))

// Recycle connections regularly
.max_lifetime(Duration::from_secs(900))
```

#### Deadlocks

**Symptoms:** `Error::Server { number: 1205 }` (deadlock victim).

**Solutions:**
```rust
// Retry deadlocks automatically
if error.is_transient() {
    // Safe to retry
    return retry_operation().await;
}

// Use shorter transactions
// Use consistent ordering of table access
// Consider SNAPSHOT isolation
```

#### Memory Growth

**Symptoms:** Process memory increases over time.

**Causes:**
- Connections not returned to pool
- Large result sets buffered
- Statement cache growing

**Solutions:**
```rust
// Ensure connections are returned (use RAII)
{
    let conn = pool.get().await?;
    // conn returned to pool when dropped
}

// Stream large results
while let Some(row) = stream.next().await {
    // Process row immediately
}

// Limit statement cache
.statement_cache_size(50)
```

### Debug Logging

Enable detailed logging for troubleshooting:

```bash
RUST_LOG=mssql_client=debug,mssql_pool=debug,mssql_codec=debug cargo run
```

For protocol-level debugging:

```bash
RUST_LOG=mssql_client=trace,tds_protocol=trace cargo run
```

## Production Checklist

### Security

- [ ] `Encrypt=true` or `Encrypt=strict` in connection string
- [ ] `TrustServerCertificate=false` (validate certificates)
- [ ] Credentials loaded from secrets manager (not env vars in logs)
- [ ] `zeroize` feature enabled for credential wiping
- [ ] Parameterized queries used everywhere (no string interpolation)

### Reliability

- [ ] Connection pool configured with appropriate limits
- [ ] Retry policy configured for transient errors
- [ ] Timeouts set for connect, command, and acquire
- [ ] Health check endpoint implemented
- [ ] Graceful shutdown handling implemented

### Observability

- [ ] Logging configured with appropriate levels
- [ ] Metrics exported (pool size, checkouts, errors)
- [ ] Tracing enabled for distributed systems (`otel` feature)
- [ ] Alerts configured for error rates and pool exhaustion

### Performance

- [ ] Connection pool sized appropriately for load
- [ ] Statement caching enabled (default)
- [ ] Large results streamed (not buffered)
- [ ] Indexes exist for query patterns

### Operations

- [ ] Runbook documented for common issues
- [ ] Database connection string in secrets management
- [ ] Monitoring dashboard created
- [ ] On-call rotation aware of SQL Server dependencies

## Performance Tuning

### Query Optimization

```rust
// Bad: Fetching all rows then filtering
let all_rows = conn.query("SELECT * FROM large_table", &[])
    .await?
    .collect_all()
    .await?;
let filtered: Vec<_> = all_rows.iter().filter(|r| r.get::<i32>(0) > 100).collect();

// Good: Filter in SQL
let rows = conn.query(
    "SELECT * FROM large_table WHERE id > @p1",
    &[&100i32]
).await?;
```

### Batch Operations

```rust
// Bad: Individual inserts
for item in items {
    conn.execute("INSERT INTO items VALUES (@p1)", &[&item]).await?;
}

// Good: Batch insert
let mut tx = conn.begin_transaction().await?;
for item in items {
    tx.execute("INSERT INTO items VALUES (@p1)", &[&item]).await?;
}
tx.commit().await?;

// Better: Bulk insert
let bulk = conn.bulk_insert("items")
    .with_columns(&["value"])
    .build()
    .await?;
for item in items {
    bulk.send_row(&[&item]).await?;
}
bulk.finish().await?;
```

### Connection Reuse

```rust
// Bad: New connection per request
async fn handle_request(config: Config) {
    let conn = Client::connect(config).await?;
    // ... use conn
    // Connection closed
}

// Good: Pool connections
async fn handle_request(pool: &Pool) {
    let conn = pool.get().await?;
    // ... use conn
    // Connection returned to pool
}
```
