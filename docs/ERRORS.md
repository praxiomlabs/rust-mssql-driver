# Error Handling Guide

This document covers error types, categorization, and recommended retry strategies for rust-mssql-driver.

## Error Types

The driver uses a single `Error` enum that categorizes all possible failures:

```rust
pub enum Error {
    // Connection errors
    Connection(String),
    ConnectionClosed,
    ConnectTimeout,
    TlsTimeout,
    ConnectionTimeout,
    CommandTimeout,

    // Authentication
    Authentication(AuthError),

    // TLS
    Tls(String),

    // Protocol
    Protocol(String),
    Codec(CodecError),

    // Type conversion
    Type(TypeError),

    // Query execution
    Query(String),

    // SQL Server errors
    Server {
        number: i32,
        class: u8,
        state: u8,
        message: String,
        server: Option<String>,
        procedure: Option<String>,
        line: u32,
    },

    // Configuration
    Config(String),

    // Azure SQL routing
    Routing { host: String, port: u16 },
    TooManyRedirects { max: u8 },

    // I/O
    Io(Arc<std::io::Error>),

    // Validation
    InvalidIdentifier(String),

    // Pool
    PoolExhausted,
}
```

## Error Categorization

### Built-in Classification

The `Error` type provides built-in methods for classification:

```rust
// Check if error may succeed on retry
if error.is_transient() {
    // Implement retry logic
}

// Check if error will never succeed
if error.is_terminal() {
    // Don't retry, report to user
}

// Get server error severity (0-25)
if let Some(severity) = error.severity() {
    if severity >= 20 {
        // Connection-terminating error
    }
}
```

### Transient Errors (Retriable)

These errors may succeed if retried:

| Error | Description |
|-------|-------------|
| `ConnectTimeout` | TCP connection timed out |
| `TlsTimeout` | TLS handshake timed out |
| `ConnectionTimeout` | Connection timeout (general) |
| `CommandTimeout` | Query execution timed out |
| `ConnectionClosed` | Connection unexpectedly closed |
| `Routing` | Azure SQL redirect required |
| `PoolExhausted` | No available connections |
| `Io(...)` | Network I/O error |

**Transient Server Errors:**

| Error Number | Description |
|--------------|-------------|
| 1205 | Transaction deadlock victim |
| -2 | Query timeout |
| 10928, 10929 | Azure resource limit |
| 40197 | Azure service error |
| 40501 | Azure service busy |
| 40613 | Azure database unavailable |
| 49918, 49919, 49920 | Azure cannot process request |
| 4060 | Cannot open database (failover) |
| 18456 | Login failed (Azure failover) |

### Terminal Errors (Non-Retriable)

These errors will never succeed on retry:

| Error | Description |
|-------|-------------|
| `Config(...)` | Invalid configuration |
| `InvalidIdentifier(...)` | SQL injection attempt blocked |

**Terminal Server Errors:**

| Error Number | Description |
|--------------|-------------|
| 102 | Syntax error |
| 207 | Invalid column name |
| 208 | Invalid object name |
| 547 | Foreign key constraint violation |
| 2627 | Unique constraint violation |
| 2601 | Duplicate key |

### Error Severity Classes

SQL Server error classes (0-25):

| Class Range | Category | Connection State |
|-------------|----------|------------------|
| 0-10 | Informational | Connection OK |
| 11-16 | User errors | Connection OK |
| 17-19 | Resource/hardware | Connection OK |
| 20-25 | System errors | Connection terminated |

```rust
if let Some(severity) = error.severity() {
    match severity {
        0..=10 => println!("Informational"),
        11..=16 => println!("User error - check query"),
        17..=19 => println!("Resource issue"),
        20..=25 => println!("Fatal - connection closed"),
        _ => unreachable!(),
    }
}
```

## Retry Strategies

### Exponential Backoff

Recommended for transient errors:

```rust
use std::time::Duration;
use tokio::time::sleep;

async fn with_retry<T, F, Fut>(
    max_attempts: u32,
    operation: F,
) -> Result<T, Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let mut attempts = 0;
    let base_delay = Duration::from_millis(100);
    let max_delay = Duration::from_secs(30);

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_transient() && attempts < max_attempts => {
                attempts += 1;

                // Exponential backoff with jitter
                let delay = std::cmp::min(
                    base_delay * 2u32.pow(attempts),
                    max_delay,
                );
                let jitter = Duration::from_millis(rand::random::<u64>() % 100);

                tracing::warn!(
                    error = %e,
                    attempt = attempts,
                    delay = ?delay,
                    "Transient error, retrying"
                );

                sleep(delay + jitter).await;
            }
            Err(e) if e.is_terminal() => {
                tracing::error!(error = %e, "Terminal error, not retrying");
                return Err(e);
            }
            Err(e) => {
                tracing::error!(error = %e, "Non-retriable error");
                return Err(e);
            }
        }
    }
}
```

### Usage Example

```rust
let result = with_retry(3, || async {
    let mut conn = pool.get().await?;
    conn.query("SELECT * FROM users WHERE id = @p1", &[&user_id]).await
}).await?;
```

### Azure SQL Recommended Settings

Azure SQL has specific transient error patterns:

```rust
async fn azure_retry<T, F, Fut>(operation: F) -> Result<T, Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let mut attempts = 0;
    let max_attempts = 5;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(Error::Server { number, .. })
                if Error::is_transient_server_error(number) && attempts < max_attempts =>
            {
                attempts += 1;

                // Azure recommends longer delays
                let delay = Duration::from_secs(attempts as u64 * 5);
                tracing::warn!(
                    error_number = number,
                    attempt = attempts,
                    "Azure transient error, retrying in {:?}",
                    delay
                );

                sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Circuit Breaker Pattern

For sustained failures:

```rust
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure: AtomicU64,  // Unix timestamp millis
    threshold: u32,
    reset_timeout: Duration,
}

impl CircuitBreaker {
    fn new(threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure: AtomicU64::new(0),
            threshold,
            reset_timeout,
        }
    }

    fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.last_failure.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            Ordering::Relaxed,
        );
    }

    fn is_open(&self) -> bool {
        let count = self.failure_count.load(Ordering::Relaxed);
        if count < self.threshold {
            return false;
        }

        // Check if reset timeout has passed
        let last = self.last_failure.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        now - last < self.reset_timeout.as_millis() as u64
    }
}
```

## Error Handling Patterns

### Match by Category

```rust
match result {
    Ok(rows) => process_rows(rows),

    // Transient - retry
    Err(e) if e.is_transient() => {
        schedule_retry();
    }

    // Terminal - report error
    Err(e) if e.is_terminal() => {
        report_error_to_user(&e);
    }

    // Server error - handle by number
    Err(Error::Server { number, message, .. }) => {
        match number {
            547 => handle_constraint_violation(&message),
            2627 | 2601 => handle_duplicate_key(&message),
            _ => log_server_error(number, &message),
        }
    }

    // Connection issues
    Err(Error::ConnectionClosed | Error::ConnectionTimeout) => {
        reconnect_and_retry();
    }

    // Unknown
    Err(e) => {
        tracing::error!(error = %e, "Unexpected error");
    }
}
```

### Structured Logging

```rust
fn log_error(error: &Error) {
    match error {
        Error::Server { number, class, message, procedure, line, .. } => {
            tracing::error!(
                error_number = number,
                severity = class,
                message = %message,
                procedure = ?procedure,
                line = line,
                "SQL Server error"
            );
        }
        _ => {
            tracing::error!(
                error_type = std::any::type_name_of_val(error),
                error = %error,
                is_transient = error.is_transient(),
                is_terminal = error.is_terminal(),
                "Database error"
            );
        }
    }
}
```

### Error Response Mapping

For web applications:

```rust
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self.0 {
            // Client errors
            Error::Config(_) | Error::InvalidIdentifier(_) => {
                (StatusCode::BAD_REQUEST, self.0.to_string()).into_response()
            }

            // Server busy - tell client to retry
            e if e.is_transient() => {
                (StatusCode::SERVICE_UNAVAILABLE, "Please retry").into_response()
            }

            // Constraint violation
            Error::Server { number: 547 | 2627 | 2601, .. } => {
                (StatusCode::CONFLICT, "Resource conflict").into_response()
            }

            // Everything else - internal error
            _ => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
            }
        }
    }
}
```

## Common Error Scenarios

### Connection Failures

```rust
match client.connect(config).await {
    Ok(client) => client,
    Err(Error::ConnectTimeout) => {
        // Network unreachable or firewall blocking
        panic!("Cannot reach database - check network/firewall");
    }
    Err(Error::TlsTimeout) => {
        // TLS handshake failed
        panic!("TLS handshake failed - check certificates");
    }
    Err(Error::Authentication(e)) => {
        // Wrong credentials
        panic!("Authentication failed: {}", e);
    }
    Err(e) => panic!("Connection failed: {}", e),
}
```

### Query Execution Failures

```rust
match client.query(sql, params).await {
    Ok(rows) => Ok(rows),
    Err(Error::CommandTimeout) => {
        // Query took too long
        Err(AppError::QueryTimeout)
    }
    Err(Error::Server { number: 102, message, .. }) => {
        // SQL syntax error
        Err(AppError::InvalidQuery(message))
    }
    Err(Error::Server { number: 207, message, .. }) => {
        // Invalid column
        Err(AppError::InvalidColumn(message))
    }
    Err(e) => Err(AppError::Database(e)),
}
```

### Transaction Failures

```rust
match tx.commit().await {
    Ok(()) => Ok(()),
    Err(Error::Server { number: 1205, .. }) => {
        // Deadlock - retry the entire transaction
        Err(AppError::Deadlock)
    }
    Err(Error::Server { number: 547, message, .. }) => {
        // Constraint violation
        Err(AppError::ConstraintViolation(message))
    }
    Err(e) => Err(AppError::TransactionFailed(e)),
}
```

## Best Practices

1. **Always check `is_transient()`** before retrying
2. **Never retry terminal errors** - they will always fail
3. **Use exponential backoff** with jitter for retries
4. **Limit retry attempts** - 3-5 is typical
5. **Log error details** including server error numbers
6. **Don't expose internal errors** to end users
7. **Monitor error rates** by category
8. **Set appropriate timeouts** per operation type
9. **Use circuit breakers** for sustained failures
10. **Handle Azure-specific errors** for cloud deployments
