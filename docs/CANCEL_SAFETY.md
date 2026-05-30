# Cancel Safety

## Summary

- `CancelHandle::cancel()` is the **safe** way to cancel a running query
- Dropping a query future mid-flight (e.g., via `tokio::select!`) is **NOT cancel-safe** and can corrupt the connection
- The pool's `test_on_checkout = true` (default) provides a safety net

## The Safe Path: CancelHandle

Obtain a `CancelHandle` before starting a query, then cancel from another task:

```rust,no_run
use mssql_client::{Client, Config};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;

    let cancel = client.cancel_handle();

    // Spawn the query (consume the stream inside the task so it stays 'static)
    let query_handle = tokio::spawn(async move {
        let rows = client.query("SELECT * FROM large_table", &[]).await?;
        let count = rows.into_iter().count();
        Ok::<_, mssql_client::Error>(count)
    });

    // Cancel after timeout
    tokio::time::sleep(Duration::from_secs(5)).await;
    cancel.cancel().await?;
    let _ = query_handle.await;
    Ok(())
}
```

When `cancel()` is called:

1. An **attention packet** is sent to SQL Server via the connection's write half
2. SQL Server acknowledges with a `DONE_ATTN` token
3. The driver drains all pending response data from the TCP stream
4. The connection is left **clean** and ready for the next query

## The Unsafe Path: Future Drops

```rust,no_run
use mssql_client::{Client, Config};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;

    // WARNING: NOT cancel-safe
    tokio::select! {
        result = client.query("SELECT 1", &[]) => { let _ = result; }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            // The query future is DROPPED here.
            // The TCP stream has partially-consumed response data.
            // The connection is now dirty.
        }
    }
    Ok(())
}
```

When a query future is dropped mid-flight:

1. The TCP receive buffer contains unconsumed TDS response data
2. There is no `Drop` impl to trigger cleanup (async drop is not stable in Rust)
3. The next query on this connection will read garbage from the previous response
4. If using a pool, the dirty connection may be returned and handed to another caller

### Why Can't We Fix This Automatically?

Rust does not support `async fn drop()`. The cleanup (draining response data until `DONE_ATTN`) requires async I/O, which cannot run in a synchronous `Drop` impl. This is a fundamental limitation shared by all async Rust database drivers.

## Pool Protection

The connection pool detects dirty connections and discards them automatically:

1. **In-flight detection (primary):** The client tracks an `in_flight` flag that is set before sending a request and cleared after the full response is read from the wire. When a `PooledConnection` is dropped, if `in_flight` is true, the connection is discarded instead of returned to the pool.

2. **Health checks (secondary safety net):** `test_on_checkout = true` (default) runs `SELECT 1` before handing out a connection. This catches edge cases where a connection becomes dirty through other means.

| Setting | Default | Effect |
|---|---|---|
| `in_flight` detection | Always on | Discards connections with pending responses |
| `test_on_checkout` | `true` | Runs `SELECT 1` before handing out a connection |
| `test_on_checkin` | `false` | Marks connection for health check on next checkout |

## Recommendations

| Pattern | Safety | Recommendation |
|---|---|---|
| `cancel_handle.cancel()` | Safe | Preferred for all query cancellation |
| `tokio::spawn` + `abort()` | Unsafe (drops future) | Use `cancel_handle` instead |
| `tokio::select!` with query | Unsafe (drops loser) | Use `cancel_handle` in a separate branch |
| `tokio::time::timeout` | Unsafe (drops on timeout) | Use `cancel_handle` with a timer task |

### Safe Timeout Pattern

```rust,no_run
use mssql_client::{Client, Config, Error};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let cancel = client.cancel_handle();

    let result = tokio::select! {
        result = client.query("SELECT 1", &[]) => result,
        _ = async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let _ = cancel.cancel().await;
        } => {
            Err(Error::CommandTimeout)
        }
    };
    let _ = result;
    Ok(())
}
```

In this pattern, the cancel branch sends an attention packet *before* the query future is dropped, ensuring the connection drains cleanly.
