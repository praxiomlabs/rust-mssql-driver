# Connection Recovery

This document describes how rust-mssql-driver handles connection failures and recovery scenarios.

## Overview

Connection recovery is handled at multiple levels:

1. **Connection Pool Level** - Health checks, connection reaping, automatic reconnection
2. **Retry Policy Level** - Automatic retry for transient errors
3. **Application Level** - Error handling and manual recovery

## Current Capabilities

### Supported Recovery Scenarios

| Scenario | Recovery Method | Automatic? |
|----------|-----------------|------------|
| Idle connection timeout | Pool health check detects, creates new connection | Yes |
| Connection killed by DBA | Pool health check detects, creates new connection | Yes |
| SQL Server restart | Pool creates new connections after restart complete | Yes |
| Network blip during query | Retry policy retries transient errors | Yes |
| Deadlock victim | Retry policy retries automatically | Yes |
| Azure failover | Retry policy + redirect handling | Yes |
| Connection pool exhausted | Wait or timeout | Configurable |

### Not Supported (Requires Application Handling)

| Scenario | Why | Recommended Action |
|----------|-----|-------------------|
| Long network partition | Cannot maintain TCP state | Re-establish connection |
| Authentication token expiry | Requires new credentials | Refresh token, reconnect |
| In-transaction connection loss | Cannot recover transaction state | Rollback and retry at application level |
| SSL certificate change | TLS session invalidated | Reconnect |

## Pool-Based Recovery

The connection pool provides the primary recovery mechanism:

### Health Checks

```rust,no_run
use mssql_client::Config;
use mssql_driver_pool::{Pool, PoolConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool_config = PoolConfig::new()
        // How often to check idle connections
        .idle_timeout(Duration::from_secs(300))
        // Maximum connection lifetime (forces refresh)
        .max_lifetime(Duration::from_secs(3600))
        // Health check query executed before returning connection
        .health_check_query("SELECT 1");

    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let pool = Pool::new(pool_config, config).await?;
    let _ = pool;
    Ok(())
}
```

### How Pool Recovery Works

```text
Connection Requested
        │
        ▼
  ┌──────────────┐
  │ Idle Conn    │──Yes──▶┌────────────────┐
  │ Available?   │        │ Run Health     │
  └──────────────┘        │ Check Query    │
        │                 └───────┬────────┘
        No                        │
        │                    ┌────┴────┐
        ▼                    │         │
  ┌──────────────┐        Pass      Fail
  │ Under Max?   │           │         │
  └──────────────┘           ▼         ▼
        │              Return     Discard &
   ┌────┴────┐         Conn       Create New
   │         │                         │
  Yes        No                        │
   │         │                         │
   ▼         ▼                         ▼
Create    Wait for                Return New
New       Release                    Conn
```

### Connection Reset on Return

When a connection is returned to the pool:

```text
// Automatically executed by pool:
// 1. Reset session state
sp_reset_connection

// 2. Clear any in-flight transactions
IF @@TRANCOUNT > 0 ROLLBACK

// 3. Verify connection is healthy
SELECT 1
```

## Retry-Based Recovery

For transient errors during query execution, the retry policy handles recovery:

### Transient Error Detection

```text
impl Error {
    /// Check if this error is transient and should be retried.
    pub fn is_transient(&self) -> bool {
        match self {
            // Network errors
            Error::Io(_) => true,
            Error::ConnectionReset => true,
            Error::ConnectionClosed => true,

            // SQL Server transient errors
            Error::SqlServer { number, .. } => match number {
                1205 => true,  // Deadlock
                1222 => true,  // Lock timeout
                40501 => true, // Azure: Service busy
                40613 => true, // Azure: Database unavailable
                49918 => true, // Azure: Cannot process request
                _ => false,
            },

            // Other error types
            _ => false,
        }
    }
}
```

### Retry Configuration

```rust,no_run
use mssql_client::{Config, RetryPolicy};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new()
        .host("server")
        .retry(RetryPolicy::new()
            .max_retries(3)
            .initial_backoff(Duration::from_millis(100))
            .max_backoff(Duration::from_secs(30))
            .jitter(true));
    let _ = config;
    Ok(())
}
```

## Azure SQL-Specific Recovery

Azure SQL has additional recovery scenarios:

### Redirect Handling

Azure SQL Gateway may redirect connections to the actual database server:

```rust,no_run
use mssql_client::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Automatic redirect handling (default)
    let config = Config::new()
        .host("myserver.database.windows.net")
        .max_redirects(2); // Allow up to 2 redirect hops
    let _ = config;
    Ok(())
}
```

### Failover Recovery

During Azure SQL failover:

1. Active connections receive error
2. Retry policy waits with backoff
3. New connection established to replica
4. Application continues with minimal disruption

```rust,no_run
use mssql_client::{Config, RetryPolicy};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Recommended Azure SQL configuration
    let config = Config::new()
        .host("myserver.database.windows.net")
        .retry(RetryPolicy::new()
            .max_retries(5)
            .initial_backoff(Duration::from_millis(100))
            .max_backoff(Duration::from_secs(60))
            .jitter(true));
    let _ = config;
    Ok(())
}
```

## Application-Level Recovery

For scenarios not handled automatically, implement application-level recovery:

### Pattern: Retry with Fresh Connection

```text
use mssql_driver_pool::Pool;
use std::time::Duration;

async fn execute_with_recovery<F, T>(
    pool: &Pool,
    operation: F,
) -> Result<T, Error>
where
    F: Fn() -> BoxFuture<'static, Result<T, Error>>,
{
    let max_attempts = 3;

    for attempt in 1..=max_attempts {
        // Get fresh connection for each attempt
        let mut conn = match pool.get().await {
            Ok(c) => c,
            Err(e) if attempt < max_attempts => {
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        match operation(&mut conn).await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_transient() && attempt < max_attempts => {
                // Connection may be corrupted, let it go back to pool
                // for health check before reuse
                drop(conn);
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Err(Error::MaxRetriesExceeded)
}
```

### Pattern: Transaction Retry

```text
async fn transaction_with_retry<F, T>(
    pool: &Pool,
    operation: F,
) -> Result<T, Error>
where
    F: Fn(&mut Transaction) -> BoxFuture<'static, Result<T, Error>>,
{
    for attempt in 1..=3 {
        let mut conn = pool.get().await?;
        let mut tx = conn.begin_transaction().await?;

        match operation(&mut tx).await {
            Ok(result) => {
                tx.commit().await?;
                return Ok(result);
            }
            Err(e) if is_retriable_in_transaction(&e) && attempt < 3 => {
                // Rollback is automatic on drop
                // Wait before retry
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                continue;
            }
            Err(e) => {
                // Non-retriable error, rollback and return
                tx.rollback().await?;
                return Err(e);
            }
        }
    }

    Err(Error::MaxRetriesExceeded)
}

fn is_retriable_in_transaction(e: &Error) -> bool {
    // Only retry deadlocks in transactions
    // Other transient errors may have partial side effects
    matches!(e, Error::SqlServer { number: 1205, .. })
}
```

## Limitations

### Connection Recovery After Network Partition

The driver does **not** support automatic reconnection during a long network outage:

```text
Client ─────────X───────── SQL Server
                │
           Network Partition
                │
                ▼
After TCP timeout (~30s), connection is dead.
Application must handle reconnection.
```

**Reason**: TCP state cannot be maintained across network partition. Any in-flight queries are lost.

**Workaround**: Use pool with health checks. Dead connections are detected and replaced.

### Mid-Transaction Recovery

Transactions cannot be recovered after connection loss:

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let mut tx = client.begin_transaction().await?;

    tx.execute("INSERT INTO orders DEFAULT VALUES", &[]).await?;

    // Network interruption here — transaction is LOST
    // Cannot recover to complete or rollback

    // After reconnection, previous transaction state is gone
    tx.commit().await?;
    Ok(())
}
```

**Reason**: Transaction state is held on the server. Connection loss means server-side rollback.

**Workaround**: Implement idempotent operations or use application-level saga pattern.

### Connection State Preservation

Some connection state is **not** preserved after recovery:

| State | Preserved? | Notes |
|-------|------------|-------|
| Session variables | No | Use `sp_reset_connection` |
| Temporary tables | No | Tied to session |
| Prepared statements | No | Re-prepared automatically via cache |
| Current database | Yes | Set in connection string |
| Transaction isolation | No | Reset to default |
| CONTEXT_INFO | No | Session-specific |

## Best Practices

### 1. Use Connection Pool

Always use the pool instead of direct connections:

```rust,no_run
use mssql_client::{Client, Config};
use mssql_driver_pool::{Pool, PoolConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let pool_config = PoolConfig::new();

    // Good: Pool handles recovery
    let pool = Pool::new(pool_config, config.clone()).await?;
    let _conn = pool.get().await?;

    // Avoid: No automatic recovery
    let _client = Client::connect(config).await?;
    Ok(())
}
```

### 2. Configure Appropriate Timeouts

```rust,no_run
use mssql_client::{Config, TimeoutConfig};
use mssql_driver_pool::PoolConfig;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new()
        .host("server")
        .connect_timeout(Duration::from_secs(30))
        .timeouts(TimeoutConfig::new().command_timeout(Duration::from_secs(60)));

    let pool_config = PoolConfig::new()
        .connection_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300));
    let _ = (config, pool_config);
    Ok(())
}
```

### 3. Handle Connection Errors Explicitly

```text
match pool.get().await {
    Ok(conn) => { /* use connection */ }
    Err(PoolError::Timeout) => {
        // Pool exhausted - scale or wait
        metrics.record_pool_exhaustion();
    }
    Err(PoolError::Connection(e)) => {
        // Connection creation failed
        metrics.record_connection_failure();
        // Consider alerting if persistent
    }
    Err(PoolError::PoolClosed) => {
        // Pool shutdown - likely application shutdown
    }
}
```

### 4. Monitor Recovery Metrics

Track these metrics for observability:

| Metric | What It Indicates |
|--------|-------------------|
| `pool_connections_created` | How often new connections are needed |
| `pool_connections_closed_stale` | Idle timeout triggering |
| `pool_health_check_failures` | Dead connections being detected |
| `query_retries_total` | Transient errors being retried |
| `query_retries_exhausted` | Retry limit hit - needs attention |

### 5. Test Recovery Scenarios

Include these in your test suite:

```text
#[tokio::test]
async fn test_recovery_after_connection_kill() {
    let pool = create_test_pool().await;
    let conn = pool.get().await?;

    // Kill the connection from another session
    // (requires DBA privileges in test setup)

    // Next get should work (pool detects and replaces)
    let conn2 = pool.get().await?;
    let result = conn2.query("SELECT 1", &[]).await?;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_recovery_after_server_restart() {
    let pool = create_test_pool().await;

    // Restart SQL Server (Docker container stop/start)

    // Wait for server to be ready
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Pool should create new connection
    let conn = pool.get().await?;
    let result = conn.query("SELECT 1", &[]).await?;
    assert!(result.is_ok());
}
```

## Comparison with Other Drivers

| Feature | rust-mssql-driver | Tiberius | ADO.NET |
|---------|-------------------|----------|---------|
| Pool-based recovery | Yes | External | Yes |
| Automatic retry | Yes | No | Yes |
| Azure failover | Yes | No | Yes |
| Connection reset | Yes | Manual | Yes |
| Transaction recovery | No | No | No |
| State preservation | Limited | No | Limited |

## Future Improvements

The following improvements are planned or implemented:

- [x] Connection warm-up after pool creation (v0.5.1)
- [x] Health check on checkout with automatic reconnection (v0.5.1)
- [x] Configurable health check query (v0.5.0)
- [ ] Connection monitoring with proactive refresh
- [ ] More granular retry policies per error type
- [ ] Circuit breaker integration
