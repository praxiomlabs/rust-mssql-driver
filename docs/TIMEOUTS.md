# Timeout Configuration

This document explains all timeout settings in rust-mssql-driver and how to configure them for different scenarios.

## Timeout Overview

The driver supports multiple timeout types:

| Timeout | Default | Scope | Description |
|---------|---------|-------|-------------|
| Connect Timeout | 30s | Connection | TCP connection establishment |
| TLS Timeout | 30s | Connection | TLS handshake completion |
| Login Timeout | 30s | Connection | SQL Server login sequence |
| Command Timeout | None | Query | Individual query execution |
| Pool Acquire Timeout | 30s | Pool | Waiting for available connection |

## Connection String Timeouts

Set timeouts via connection string keywords:

```
Server=localhost;Connect Timeout=15;Command Timeout=60
```

### Connect Timeout

Time limit for establishing the TCP connection to SQL Server.

```rust
// Connection string
"Server=localhost;Connect Timeout=15"

// Builder API
Config::builder()
    .host("localhost")
    .connect_timeout(Duration::from_secs(15))
    .build()?
```

**Recommendations:**
- Local/LAN: 5-15 seconds
- Remote/WAN: 15-30 seconds
- Azure SQL: 30-60 seconds (cold start scenarios)

### Command Timeout

Default timeout for query and execute operations.

```rust
// Connection string
"Server=localhost;Command Timeout=60"

// Builder API
Config::builder()
    .host("localhost")
    .command_timeout(Duration::from_secs(60))
    .build()?
```

**Value 0 = no timeout** (not recommended for production)

**Recommendations:**
- Web API queries: 5-30 seconds
- Reporting queries: 60-300 seconds
- Background jobs: 300-600 seconds

## Pool Timeouts

Configure pool-level timeouts:

```rust
use mssql_driver_pool::{Pool, PoolConfig};
use std::time::Duration;

let pool = Pool::builder()
    // Time to wait for a connection from the pool
    .acquire_timeout(Duration::from_secs(30))

    // Close connections idle longer than this
    .idle_timeout(Duration::from_secs(600))

    // Maximum connection lifetime (recycle)
    .max_lifetime(Duration::from_secs(3600))

    .build(config)
    .await?;
```

### acquire_timeout

Maximum time to wait for a connection from the pool.

**Behavior:**
- If a connection is available immediately, returns instantly
- If pool is exhausted, waits up to `acquire_timeout`
- If timeout expires, returns `Error::PoolTimeout`

**Recommendations:**
- Web requests: 5-10 seconds (fail fast)
- Background jobs: 30-60 seconds (can wait)

### idle_timeout

Close connections that have been idle longer than this duration.

**Purpose:**
- Prevents stale connections
- Reduces resource usage during low traffic
- Azure SQL closes idle connections after ~30 minutes

**Recommendations:**
- General: 300-600 seconds (5-10 minutes)
- Azure SQL: 60-300 seconds (before Azure kills them)

### max_lifetime

Maximum time a connection can be reused before being closed.

**Purpose:**
- Prevents issues with long-lived connections
- Ensures connection rotation for load balancing
- Clears accumulated state on the connection

**Recommendations:**
- General: 1800-3600 seconds (30-60 minutes)
- High-availability: 900-1800 seconds (for faster failover detection)

## Application-Level Timeouts

For fine-grained control, use Tokio timeouts:

```rust
use tokio::time::{timeout, Duration};

// Per-query timeout (overrides command timeout)
let result = timeout(
    Duration::from_secs(5),
    client.query("SELECT * FROM users", &[])
).await;

match result {
    Ok(Ok(rows)) => { /* Success */ }
    Ok(Err(db_error)) => { /* Database error */ }
    Err(_) => { /* Timeout exceeded */ }
}
```

### Timeout Wrapper Function

```rust
async fn query_with_timeout<T, F, Fut>(
    timeout_secs: u64,
    operation: F,
) -> Result<T, Error>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    match timeout(Duration::from_secs(timeout_secs), operation()).await {
        Ok(result) => result,
        Err(_) => Err(Error::Timeout(format!(
            "Operation timed out after {} seconds",
            timeout_secs
        ))),
    }
}

// Usage
let rows = query_with_timeout(10, || async {
    client.query("SELECT * FROM users", &[]).await
}).await?;
```

## Timeout Scenarios

### Web Application

Fast failure for responsive UX:

```rust
let config = Config::from_connection_string(
    "Server=db;Connect Timeout=5;Command Timeout=10"
)?;

let pool = Pool::builder()
    .acquire_timeout(Duration::from_secs(5))
    .build(config)
    .await?;
```

### Background Job Processing

Longer timeouts for batch operations:

```rust
let config = Config::from_connection_string(
    "Server=db;Connect Timeout=30;Command Timeout=300"
)?;

let pool = Pool::builder()
    .acquire_timeout(Duration::from_secs(60))
    .max_lifetime(Duration::from_secs(7200))  // 2 hours
    .build(config)
    .await?;
```

### Azure SQL Database

Account for cold starts and throttling:

```rust
let config = Config::from_connection_string(
    "Server=x.database.windows.net;Connect Timeout=60;Command Timeout=60"
)?;

let pool = Pool::builder()
    .acquire_timeout(Duration::from_secs(60))
    .idle_timeout(Duration::from_secs(300))  // Before Azure kills it
    .max_lifetime(Duration::from_secs(1800))
    .build(config)
    .await?;
```

### Reporting / OLAP

Long-running analytical queries:

```rust
let config = Config::from_connection_string(
    "Server=reporting-db;Connect Timeout=15;Command Timeout=3600"
)?;

// Or no default timeout, control per-query
let config = Config::from_connection_string(
    "Server=reporting-db;Connect Timeout=15;Command Timeout=0"
)?;

// Then timeout at application level
let result = timeout(
    Duration::from_secs(3600),
    client.query(&complex_report_sql, &[])
).await??;
```

## Timeout Error Handling

```rust
use mssql_client::Error;

match client.query(sql, params).await {
    Ok(rows) => handle_rows(rows),
    Err(Error::Timeout(msg)) => {
        // Query exceeded command timeout
        tracing::warn!("Query timeout: {}", msg);
        return Err(AppError::QueryTimeout);
    }
    Err(Error::ConnectTimeout { ref host, port }) => {
        // TCP connection timed out
        tracing::error!("TCP connect timeout to {}:{}", host, port);
        return Err(AppError::ConnectionTimeout);
    }
    Err(Error::TlsTimeout { ref host, port }) => {
        // TLS handshake timed out
        tracing::error!("TLS handshake timeout to {}:{}", host, port);
        return Err(AppError::ConnectionTimeout);
    }
    Err(Error::LoginTimeout { ref host, port }) => {
        // Login/authentication phase timed out
        tracing::error!("Login timeout to {}:{}", host, port);
        return Err(AppError::ConnectionTimeout);
    }
    Err(e) => {
        // Other database error
        return Err(AppError::Database(e));
    }
}
```

## Timeout Best Practices

### 1. Always Set Timeouts

Never run production workloads without timeouts:

```rust
// BAD: No timeouts - query can hang forever
let config = Config::from_connection_string(
    "Server=db;Command Timeout=0"
)?;

// GOOD: Explicit timeouts
let config = Config::from_connection_string(
    "Server=db;Connect Timeout=15;Command Timeout=30"
)?;
```

### 2. Timeout Hierarchy

Set timeouts at multiple levels for defense in depth:

```
Application Timeout (shortest)
    └── Command Timeout (medium)
        └── Pool Acquire Timeout (longest or equal)
            └── Connect Timeout (initial connection)
```

### 3. Monitor Timeout Metrics

Track timeout occurrences:

```rust
static TIMEOUT_COUNTER: AtomicU64 = AtomicU64::new(0);

async fn query_with_metrics(client: &mut Client, sql: &str) -> Result<Vec<Row>, Error> {
    match client.query(sql, &[]).await {
        Ok(rows) => Ok(rows),
        Err(e) if matches!(e, Error::Timeout(_)) => {
            TIMEOUT_COUNTER.fetch_add(1, Ordering::Relaxed);
            Err(e)
        }
        Err(e) => Err(e),
    }
}
```

### 4. Adjust Based on Query Complexity

Different queries need different timeouts:

```rust
async fn lookup_user(client: &mut Client, id: i32) -> Result<User, Error> {
    // Fast indexed lookup
    timeout(Duration::from_secs(2), async {
        client.query("SELECT * FROM users WHERE id = @p1", &[&id]).await
    }).await?
}

async fn generate_report(client: &mut Client) -> Result<Report, Error> {
    // Complex aggregation
    timeout(Duration::from_secs(300), async {
        client.query(&report_sql, &[]).await
    }).await?
}
```

## Troubleshooting Timeouts

### Frequent Connection Timeouts

- Check network connectivity to SQL Server
- Verify firewall allows port 1433
- Check DNS resolution speed
- Consider increasing `Connect Timeout`

### Frequent Query Timeouts

- Review query execution plans
- Add appropriate indexes
- Consider query optimization
- Check for blocking/locking issues

### Pool Acquire Timeouts

- Increase pool `max_size`
- Reduce connection hold time in application
- Check for connection leaks (unreturned connections)
- Review application concurrency patterns
