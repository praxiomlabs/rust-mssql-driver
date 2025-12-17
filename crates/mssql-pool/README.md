# mssql-driver-pool

Purpose-built connection pool for SQL Server with lifecycle management.

## Overview

Unlike generic connection pools, this implementation understands SQL Server specifics like `sp_reset_connection` for proper connection state cleanup. It provides efficient connection reuse while ensuring connections are in a clean state.

## Features

- **`sp_reset_connection`** - Proper connection state reset on return
- **Health checks** - Periodic validation via `SELECT 1`
- **Configurable sizing** - Min/max pool sizes
- **Timeout management** - Connection, idle, and checkout timeouts
- **Automatic reconnection** - Handles transient failures
- **Statement cache coordination** - Per-connection prepared statement management
- **Comprehensive metrics** - Observable pool statistics

## Usage

### Builder Pattern

```rust
use mssql_driver_pool::{Pool, PoolConfig};
use std::time::Duration;

let pool = Pool::builder()
    .min_connections(5)
    .max_connections(20)
    .idle_timeout(Duration::from_secs(300))
    .sp_reset_connection(true)
    .build()
    .await?;
```

### Using PoolConfig

```rust
let config = PoolConfig::new()
    .min_connections(5)
    .max_connections(20);

let pool = Pool::new(config).await?;
```

### Getting Connections

```rust
// Get a connection from the pool
let conn = pool.get().await?;

// Use the connection
let rows = conn.query("SELECT * FROM users", &[]).await?;

// Connection automatically returned to pool on drop
```

## Pool Status and Metrics

```rust
// Check pool status
let status = pool.status();
println!("Total: {}", status.pool_size);
println!("Available: {}", status.available);
println!("In use: {}", status.in_use);
println!("Utilization: {:.1}%", status.utilization());

// Get detailed metrics
let metrics = pool.metrics();
println!("Checkouts: {}", metrics.total_checkouts);
println!("Checkout success rate: {:.2}", metrics.checkout_success_rate());
println!("Avg checkout time: {:?}", metrics.avg_checkout_time);
```

## Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `min_connections` | 0 | Minimum connections to maintain |
| `max_connections` | 10 | Maximum connections allowed |
| `connection_timeout` | 30s | Timeout for establishing new connections |
| `idle_timeout` | 300s | Close connections idle longer than this |
| `max_lifetime` | None | Maximum lifetime of a connection |
| `checkout_timeout` | 30s | Timeout waiting for available connection |
| `health_check_interval` | 30s | Interval between health checks |
| `sp_reset_connection` | true | Execute `sp_reset_connection` on return |
| `test_on_acquire` | true | Validate connection before use |

## Connection Lifecycle

```text
1. Connection created or reused from pool
2. Health check (if test_on_acquire enabled)
3. Connection checked out to application
4. Application uses connection
5. Connection dropped/returned
6. sp_reset_connection executed (if enabled)
7. Connection returned to pool (or closed if max_lifetime exceeded)
```

## sp_reset_connection

SQL Server's `sp_reset_connection` resets connection state including:

- Clears temporary tables
- Resets session options (SET statements)
- Clears open transactions
- Drops temporary stored procedures
- Releases locks
- Resets the current database

This ensures each checkout receives a clean connection state.

## Modules

| Module | Description |
|--------|-------------|
| `config` | Pool configuration options |
| `pool` | Main `Pool` type and `PooledConnection` |
| `lifecycle` | Connection lifecycle management |
| `error` | Pool error types |

## Key Types

| Type | Description |
|------|-------------|
| `Pool` | Connection pool |
| `PoolBuilder` | Builder for pool configuration |
| `PoolConfig` | Pool configuration |
| `PooledConnection` | Checked-out connection wrapper |
| `PoolStatus` | Current pool state |
| `PoolMetrics` | Accumulated pool statistics |
| `ConnectionLifecycle` | Trait for custom lifecycle hooks |

## Error Handling

```rust
use mssql_driver_pool::PoolError;

match pool.get().await {
    Ok(conn) => { /* use connection */ }
    Err(PoolError::Exhausted) => {
        // Pool at max capacity, all connections in use
    }
    Err(PoolError::CheckoutTimeout) => {
        // Timed out waiting for available connection
    }
    Err(PoolError::ConnectionFailed(e)) => {
        // Failed to establish new connection
    }
    Err(e) => {
        // Other errors
    }
}
```

## Best Practices

1. **Size appropriately** - Set `max_connections` based on your workload and SQL Server limits
2. **Use min_connections** - Pre-warm the pool for latency-sensitive applications
3. **Enable health checks** - Detect stale connections before use
4. **Monitor metrics** - Track utilization and checkout times
5. **Set timeouts** - Prevent indefinite waits

## License

MIT OR Apache-2.0
