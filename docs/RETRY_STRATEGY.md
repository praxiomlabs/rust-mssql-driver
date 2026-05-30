# Retry and Backoff Strategy

This document describes how rust-mssql-driver handles transient errors through automatic retries with exponential backoff.

## Overview

The driver automatically retries operations that fail with transient errors. This behavior is configurable and follows best practices for cloud database connectivity.

## Default Retry Policy

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_retries` | 3 | Maximum retry attempts before giving up |
| `initial_backoff` | 100ms | Wait time before first retry |
| `max_backoff` | 30s | Maximum wait time between retries |
| `backoff_multiplier` | 2.0 | Exponential backoff factor |
| `jitter` | true | Add randomness to prevent thundering herd |

## Transient vs Non-Transient Errors

### Transient Errors (Retriable)

| Error Type | SQL Error Code | Description |
|------------|----------------|-------------|
| Deadlock | 1205 | Transaction was deadlocked |
| Lock timeout | 1222 | Lock request timeout |
| Service busy (Azure) | 40501 | Server too busy |
| Resource limit (Azure) | 10928, 10929 | Resource limit reached |
| Connection lost | — | Network interruption during query |
| TLS handshake failure | — | Transient TLS error |
| Database unavailable (Azure) | 40613 | Database temporarily unavailable |
| Connection throttled (Azure) | 10053, 10054 | Connection reset by server |

### Non-Transient Errors (Not Retriable)

| Error Type | Description |
|------------|-------------|
| Authentication failure | Invalid credentials |
| Permission denied | Access control violations |
| Invalid query syntax | SQL syntax errors |
| Constraint violation | Primary key, foreign key violations |
| Data conversion error | Type mismatch |
| Object not found | Table/column doesn't exist |

## Backoff Calculation

```text
backoff_ms = min(initial_backoff * (multiplier ^ (attempt - 1)), max_backoff)

if jitter:
    backoff_ms *= random(0.5, 1.5)
```

### Example Progression

| Attempt | Base Backoff | With Jitter (range) |
|---------|--------------|---------------------|
| 1 | 100ms | 50-150ms |
| 2 | 200ms | 100-300ms |
| 3 | 400ms | 200-600ms |
| 4 (if configured) | 800ms | 400-1200ms |

## Configuration

### Using Connection String

Not directly configurable via connection string. Use programmatic configuration.

### Programmatic Configuration

```rust,no_run
use mssql_client::{Config, RetryPolicy};
use std::time::Duration;

fn main() {
    // Custom retry policy
    let retry = RetryPolicy::new()
        .max_retries(5)
        .initial_backoff(Duration::from_millis(200))
        .max_backoff(Duration::from_secs(60))
        .backoff_multiplier(2.5)
        .jitter(true);

    let _config = Config::new()
        .host("server")
        .retry(retry);
}
```

### Disable Retries

```rust,no_run
use mssql_client::{Config, RetryPolicy};

fn main() {
    let _config = Config::new()
        .host("server")
        .retry(RetryPolicy::no_retry());
}
```

### Per-Operation Retry

```rust,no_run
use mssql_client::Config;

fn main() {
    let _config = Config::new()
        .host("server")
        .max_retries(5);  // Shorthand for default policy with different max
}
```

## Azure SQL Recommendations

Azure SQL has specific transient error patterns. Recommended settings:

```rust,no_run
use mssql_client::RetryPolicy;
use std::time::Duration;

fn main() {
    let _azure_retry = RetryPolicy::new()
        .max_retries(5)                           // More retries for cloud
        .initial_backoff(Duration::from_millis(100))
        .max_backoff(Duration::from_secs(60))     // Allow longer waits
        .backoff_multiplier(2.0)
        .jitter(true);                            // Essential for cloud
}
```

### Azure-Specific Error Codes

| Code | Error | Retry? | Notes |
|------|-------|--------|-------|
| 40501 | Service busy | Yes | Wait and retry |
| 40613 | Database unavailable | Yes | Failover in progress |
| 49918 | Cannot process request | Yes | Insufficient resources |
| 49919 | Cannot process create/update | Yes | Insufficient resources |
| 49920 | Cannot process request | Yes | Insufficient resources |
| 4060 | Cannot open database | Maybe | Check if database exists |
| 40197 | Processing request error | Yes | Service error |

## On-Premises Recommendations

For on-premises SQL Server with stable network:

```rust,no_run
use mssql_client::RetryPolicy;
use std::time::Duration;

fn main() {
    let _onprem_retry = RetryPolicy::new()
        .max_retries(3)                           // Fewer retries
        .initial_backoff(Duration::from_millis(50))
        .max_backoff(Duration::from_secs(10))
        .backoff_multiplier(2.0)
        .jitter(false);                           // Less critical on-prem
}
```

## Retry Flow

```text
Initial Request
       │
       ▼
  ┌─────────┐
  │ Execute │
  └────┬────┘
       │
       ▼
  ┌──────────────┐
  │ Check Result │
  └──────┬───────┘
         │
    ┌────┴────┐
    │         │
Success   Failure
    │         │
    ▼         ▼
  Done   ┌───────────────┐
         │ Transient?    │
         └───────┬───────┘
                 │
            ┌────┴────┐
            │         │
           No        Yes
            │         │
            ▼         ▼
       Return    ┌──────────────┐
       Error     │ Retry Left?  │
                 └──────┬───────┘
                        │
                   ┌────┴────┐
                   │         │
                  No        Yes
                   │         │
                   ▼         ▼
              Return    ┌─────────┐
              Error     │ Backoff │
                        └────┬────┘
                             │
                             ▼
                        Execute
                        (retry)
```

## Best Practices

### 1. Always Use Jitter for Cloud

```text
// Good: Prevents thundering herd
.jitter(true)

// Risk: All retries at same time can overwhelm server
.jitter(false)
```

### 2. Set Appropriate Max Backoff

```text
// Good: Reasonable maximum wait
.max_backoff(Duration::from_secs(60))

// Risk: Could wait too long for user-facing requests
.max_backoff(Duration::from_secs(300))
```

### 3. Log Retry Attempts

```text
// Enable at info level for retry visibility
RUST_LOG=mssql_client=info cargo run

// Log output includes retry information:
// INFO retry attempt 2/3 after deadlock, waiting 200ms
```

### 4. Consider Circuit Breaker for High Volume

For high-throughput systems, combine retries with circuit breaker:

```text
// Pseudocode - integrate with circuit breaker library
if circuit_breaker.is_open() {
    return Err(Error::CircuitOpen);
}

match client.query(sql, params).await {
    Ok(result) => {
        circuit_breaker.record_success();
        Ok(result)
    }
    Err(e) if e.is_transient() => {
        circuit_breaker.record_failure();
        // Driver will retry internally
        Err(e)
    }
    Err(e) => Err(e),
}
```

### 5. Handle Non-Retriable Errors Promptly

```text
match client.query(sql, params).await {
    Ok(result) => handle_result(result),
    Err(e) if e.is_transient() => {
        // Already retried by driver, log and handle
        tracing::error!("Query failed after retries: {:?}", e);
        handle_transient_failure(e)
    }
    Err(e) => {
        // Non-retriable: fix immediately
        tracing::error!("Query failed (non-transient): {:?}", e);
        handle_permanent_failure(e)
    }
}
```

## Timeout Interaction

Retries interact with timeouts. Ensure total retry time fits within acceptable latency:

```text
Total Time = sum(backoff[i] for i in 1..max_retries) + execution_time * (max_retries + 1)

Example with defaults:
- max_retries: 3
- backoffs: 100ms + 200ms + 400ms = 700ms
- command_timeout: 30s per attempt
- Worst case: 700ms + 30s * 4 = ~120s
```

### Recommended Settings for User-Facing APIs

```text
let api_retry = RetryPolicy::new()
    .max_retries(2)                          // Faster failure
    .initial_backoff(Duration::from_millis(50))
    .max_backoff(Duration::from_secs(5))
    .jitter(true);

let config = Config::new()
    .host("server")
    .retry(api_retry)
    .command_timeout(Duration::from_secs(10)); // Short timeout
```

## Monitoring Retries

Track retry metrics for observability:

| Metric | What to Watch |
|--------|---------------|
| `retries_total` | Total retry attempts |
| `retries_by_error_code` | Which errors cause retries |
| `retry_success_rate` | Do retries help? |
| `retry_latency_p99` | Total time including retries |

## Disabling Retries

In some scenarios, you may want to disable retries:

```text
// Disable for idempotency-sensitive operations
let config = Config::new()
    .host("server")
    .retry(RetryPolicy::no_retry());

// Or for specific operations (if supported)
client.execute_no_retry(sql, params).await?;
```

## Comparison with Other Drivers

| Feature | rust-mssql-driver | Tiberius | ADO.NET |
|---------|-------------------|----------|---------|
| Auto-retry | Yes | No | Yes (configurable) |
| Exponential backoff | Yes | N/A | Yes |
| Jitter | Yes | N/A | Partial |
| Per-error config | Planned | N/A | Yes |
| Circuit breaker | No (use external) | No | No |
