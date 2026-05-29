# Production Deployment Guide

This guide covers best practices for deploying rust-mssql-driver in production environments.

## Table of Contents

- [Connection Configuration](#connection-configuration)
- [Connection Pooling](#connection-pooling)
- [Error Handling](#error-handling)
- [Monitoring & Observability](#monitoring--observability)
- [Security Hardening](#security-hardening)
- [High Availability](#high-availability)
- [Performance Tuning](#performance-tuning)
- [Troubleshooting](#troubleshooting)

---

## Connection Configuration

### Connection String Best Practices

```rust
// Production connection string
let conn_str = "Server=db.example.com,1433;\
    Database=production;\
    User Id=app_user;\
    Password=<secret>;\
    Encrypt=strict;\
    TrustServerCertificate=false;\
    Connect Timeout=30;\
    Command Timeout=60;\
    Application Name=MyApp-v1.2.3";
```

### Required Settings for Production

| Setting | Recommendation | Reason |
|---------|---------------|--------|
| `Encrypt` | `strict` or `true` | Always encrypt in production |
| `TrustServerCertificate` | `false` | Validate server certificates |
| `Connect Timeout` | 30 seconds | Fail fast on connection issues |
| `Command Timeout` | Based on queries | Prevent hung queries |
| `Application Name` | Set with version | Aids SQL Server monitoring |

### Environment-Specific Configuration

```rust
use std::env;

fn get_config() -> Result<Config, Error> {
    let host = env::var("DB_HOST").expect("DB_HOST required");
    let database = env::var("DB_NAME").expect("DB_NAME required");
    let user = env::var("DB_USER").expect("DB_USER required");
    let password = env::var("DB_PASSWORD").expect("DB_PASSWORD required");

    let encrypt = if env::var("DB_ENCRYPT_STRICT").is_ok() {
        "strict"
    } else {
        "true"
    };

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};Encrypt={};TrustServerCertificate=false",
        host, database, user, password, encrypt
    );

    Config::from_connection_string(&conn_str)
}
```

---

## Connection Pooling

### Sizing Guidelines

| Workload Type | Min Size | Max Size | Formula |
|---------------|----------|----------|---------|
| Web API | 2 | 20-50 | 2 × CPU cores |
| Background Jobs | 1 | 10 | Based on concurrency |
| High Throughput | 5 | 100 | Monitor and adjust |

### Recommended Configuration

```rust
use mssql_driver_pool::{Pool, PoolConfig};
use std::time::Duration;

let pool = Pool::builder()
    .client_config(config)
    // Size based on expected concurrency
    .min_connections(2)                       // Minimum warm connections
    .max_connections(20)                      // Maximum connections

    // Timeouts
    .acquire_timeout(Duration::from_secs(30)) // Max wait for connection
    .idle_timeout(Duration::from_secs(600))   // Close idle connections after 10 min
    .max_lifetime(Duration::from_secs(3600))  // Recycle connections after 1 hour

    // Health checking
    .test_on_borrow(true)                     // Verify before use

    .build()
    .await?;
```

### Pool Sizing Formula

For web applications:

```
max_connections = (average_concurrent_requests × average_query_time_ms) / 1000 + buffer

Example:
- 100 concurrent requests
- 50ms average query time
- Buffer of 10

max_connections = (100 × 50) / 1000 + 10 = 15 connections
```

### Avoiding Pool Exhaustion

```rust
// Always use timeouts to prevent pool exhaustion
match tokio::time::timeout(
    Duration::from_secs(5),
    pool.get()
).await {
    Ok(Ok(conn)) => {
        // Use connection
    }
    Ok(Err(e)) => {
        // Pool error (e.g., all connections unhealthy)
        tracing::error!("Pool error: {}", e);
    }
    Err(_) => {
        // Timeout waiting for connection
        tracing::warn!("Connection pool timeout - consider increasing pool size");
    }
}
```

---

## Error Handling

### Error Categories

```rust
use mssql_client::Error;

async fn execute_with_retry<T, F, Fut>(
    pool: &Pool,
    operation: F,
) -> Result<T, Error>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let mut attempts = 0;
    let max_attempts = 3;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if is_transient(&e) && attempts < max_attempts => {
                attempts += 1;
                let delay = Duration::from_millis(100 * 2u64.pow(attempts));
                tracing::warn!("Transient error, retrying in {:?}: {}", delay, e);
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_transient(error: &Error) -> bool {
    match error {
        Error::Io(_) => true,                    // Network errors
        Error::Protocol(msg) if msg.contains("timeout") => true,
        Error::Server { number, .. } => {
            matches!(number,
                // Transient SQL Server errors
                -2 |     // Timeout
                53 |     // Connection failure
                121 |    // Semaphore timeout
                233 |    // Connection init error
                10053 |  // Connection aborted
                10054 |  // Connection reset
                10060 |  // Connection timeout
                40143 |  // Azure SQL throttling
                40197 |  // Service error
                40501 |  // Service busy
                40613    // Database unavailable
            )
        }
        _ => false,
    }
}
```

### Graceful Degradation

```rust
async fn query_with_fallback(
    pool: &Pool,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<Vec<Row>, Error> {
    match pool.get().await {
        Ok(mut conn) => {
            conn.query(sql, params).await
        }
        Err(e) => {
            tracing::error!("Database unavailable: {}", e);
            // Return cached data, empty result, or propagate error
            // based on business requirements
            Err(e)
        }
    }
}
```

---

## Monitoring & Observability

### OpenTelemetry Integration

```toml
# Cargo.toml
[dependencies]
mssql-client = { version = "0.10", features = ["otel"] }
```

```rust
use opentelemetry::global;
use opentelemetry_sdk::trace::TracerProvider;

fn init_tracing() {
    let provider = TracerProvider::builder()
        .with_simple_exporter(opentelemetry_otlp::new_exporter().tonic())
        .build();
    global::set_tracer_provider(provider);
}

// Queries automatically generate spans with:
// - db.system = "mssql"
// - db.statement = <sanitized SQL>
// - db.operation = "query" | "execute"
// - Duration and error status
```

### Logging Configuration

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mssql_client=info,mssql_pool=info".into())
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
```

### Key Metrics to Monitor

| Metric | Source | Alert Threshold |
|--------|--------|-----------------|
| Pool connections in use | Pool metrics | > 80% of max |
| Connection wait time | Pool metrics | > 1 second |
| Query latency P99 | Application traces | Based on SLA |
| Error rate | Application logs | > 1% |
| Connection failures | Driver logs | Any |

---

## Security Hardening

### TLS Configuration

```rust
// Always use encryption in production
let config = Config::builder()
    .host("db.example.com")
    .encryption(Encryption::Required)  // Fail if TLS unavailable
    .trust_server_certificate(false)   // Validate certificates
    .build()?;
```

### Credential Management

```rust
// Never log passwords
let password = env::var("DB_PASSWORD")?;

// Use zeroize feature to wipe credentials from memory
#[cfg(feature = "zeroize")]
{
    use zeroize::Zeroize;
    let mut password = password;
    // ... use password ...
    password.zeroize();  // Secure wipe
}
```

### Network Security

1. **Use private endpoints** for Azure SQL
2. **Enable firewall rules** allowing only application IPs
3. **Use VPC/VNet** for database access
4. **Disable public endpoint** when possible

### Least Privilege Access

```sql
-- Create application-specific login
CREATE LOGIN app_readonly WITH PASSWORD = 'StrongPassword!';
CREATE USER app_readonly FOR LOGIN app_readonly;

-- Grant minimal permissions
GRANT SELECT ON SCHEMA::dbo TO app_readonly;
-- Only add INSERT/UPDATE/DELETE as needed
```

---

## High Availability

### Azure SQL Database

```rust
// Azure SQL handles failover automatically
// Connection string automatically follows redirects
let conn_str = "Server=your-server.database.windows.net;\
    Database=your-db;\
    User Id=user;\
    Password=pass;\
    Encrypt=strict";

let config = Config::from_connection_string(conn_str)?;
// Driver handles Azure SQL Gateway redirects automatically
```

### On-Premises Always On

```rust
// Connect to availability group listener
let conn_str = "Server=ag-listener.example.com;\
    Database=your-db;\
    User Id=user;\
    Password=pass;\
    ApplicationIntent=ReadOnly";  // For read replicas
```

### Connection Resilience

```rust
// Pool automatically handles connection failures
let pool = Pool::builder()
    .client_config(config)
    .max_connections(20)
    .test_on_borrow(true)        // Verify connection health
    .max_lifetime(Duration::from_secs(1800))  // Refresh connections
    .build()
    .await?;

// Implement retry logic for transient failures
let result = retry_with_backoff(|| async {
    let mut conn = pool.get().await?;
    conn.query(sql, params).await
}).await?;
```

---

## Performance Tuning

### Query Optimization

```rust
// Use parameterized queries (enables plan caching)
// GOOD
client.query(
    "SELECT * FROM users WHERE id = @p1",
    &[&user_id]
).await?;

// BAD (new plan for each query)
client.query(
    &format!("SELECT * FROM users WHERE id = {}", user_id),
    &[]
).await?;
```

### Batch Operations

```rust
// Batch multiple inserts in a transaction
let mut tx = client.begin_transaction().await?;

for chunk in records.chunks(1000) {
    for record in chunk {
        tx.execute(
            "INSERT INTO table (a, b) VALUES (@p1, @p2)",
            &[&record.a, &record.b]
        ).await?;
    }
}

tx.commit().await?;
```

### Memory Management

```rust
// For large result sets, process rows as they stream
let rows = client.query("SELECT * FROM large_table", &[]).await?;

// Process one row at a time (doesn't load all into memory)
for result in rows {
    let row = result?;
    process_row(&row)?;
}
```

---

## Troubleshooting

### Common Issues

#### Connection Timeout
```
Error: Connection timeout
```
**Causes:**
- Firewall blocking port 1433
- SQL Server not listening on TCP
- DNS resolution failure

**Resolution:**
1. Test connectivity: `telnet db.example.com 1433`
2. Verify SQL Server TCP/IP is enabled
3. Check firewall rules

#### TLS Handshake Failure
```
Error: TLS handshake failed
```
**Causes:**
- Certificate validation failure
- TLS version mismatch
- Server certificate expired

**Resolution:**
1. Verify server certificate is valid
2. Check if server supports TLS 1.2+
3. For development only: `TrustServerCertificate=true`

#### Pool Exhaustion
```
Error: Connection pool timeout
```
**Causes:**
- Pool size too small
- Connections not being returned
- Long-running queries blocking connections

**Resolution:**
1. Increase pool size
2. Add query timeouts
3. Check for connection leaks (forgotten await)

### Diagnostic Queries

```sql
-- Check active connections from application
SELECT
    program_name,
    COUNT(*) as connection_count,
    MAX(last_read) as last_activity
FROM sys.dm_exec_sessions
WHERE program_name = 'YourAppName'
GROUP BY program_name;

-- Check for blocking
SELECT * FROM sys.dm_exec_requests
WHERE blocking_session_id > 0;

-- Check query performance
SELECT TOP 10
    total_elapsed_time / execution_count as avg_elapsed,
    execution_count,
    SUBSTRING(text, 1, 100) as query
FROM sys.dm_exec_query_stats
CROSS APPLY sys.dm_exec_sql_text(sql_handle)
ORDER BY avg_elapsed DESC;
```
