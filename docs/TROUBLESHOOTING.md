# Troubleshooting Guide

This guide helps diagnose and resolve common issues with rust-mssql-driver.

## Connection Issues

### Cannot Connect to SQL Server

**Symptoms:**
- `ConnectTimeout` error
- `Connection refused` error
- Hangs indefinitely

**Diagnostic Steps:**

1. **Verify SQL Server is running:**
   ```bash
   # Check if port is open
   nc -zv your-server 1433

   # Or use telnet
   telnet your-server 1433
   ```

2. **Check firewall rules:**
   ```bash
   # Linux
   sudo iptables -L -n | grep 1433

   # Windows
   netsh advfirewall firewall show rule name=all | findstr 1433
   ```

3. **Verify SQL Server is listening:**
   ```sql
   -- Run on SQL Server
   SELECT local_net_address, local_tcp_port
   FROM sys.dm_exec_connections
   WHERE session_id = @@SPID;
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Wrong port | Use comma syntax: `Server=host,1434` |
| Named instance | Use backslash: `Server=host\INSTANCE` |
| Firewall blocking | Open port 1433 (or custom port) |
| SQL Server not listening on TCP | Enable TCP/IP in SQL Server Configuration Manager |
| Azure SQL firewall | Add client IP to Azure firewall rules |

### TLS/SSL Errors

**Symptoms:**
- `TlsTimeout` error
- `certificate verify failed`
- `handshake failure`

**Diagnostic Steps:**

1. **Check TLS configuration:**
   ```rust
   // Test with certificate validation disabled (dev only!)
   let config = Config::from_connection_string(
       "Server=...;TrustServerCertificate=true"
   )?;
   ```

2. **Verify certificate:**
   ```bash
   # Check server certificate
   openssl s_client -connect your-server:1433 -starttls mssql
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Self-signed cert | Add CA cert or use `TrustServerCertificate=true` (dev only) |
| Hostname mismatch | Certificate CN must match server hostname |
| Expired certificate | Renew server certificate |
| TLS version mismatch | Check SQL Server supports TLS 1.2+ |
| TDS 8.0 with old server | Use `Encrypt=true` instead of `Encrypt=strict` |

### Authentication Failures

**Symptoms:**
- `Authentication` error
- `Login failed for user`
- Error 18456

**Diagnostic Steps:**

1. **Verify credentials work with other tools:**
   ```bash
   # Using sqlcmd
   sqlcmd -S your-server -U your-user -P 'your-password' -Q "SELECT 1"
   ```

2. **Check SQL Server error log:**
   ```sql
   EXEC xp_readerrorlog 0, 1, N'Login failed';
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Wrong password | Verify password, check for special characters |
| SQL auth disabled | Enable SQL Server authentication mode |
| User doesn't exist | Create login and user |
| Database access denied | Grant database access to user |
| Azure AD format | Use `user@server` format for Azure SQL |

**Error 18456 State Codes:**

| State | Meaning |
|-------|---------|
| 2 | Invalid user ID |
| 5 | Invalid user ID |
| 6 | Windows login attempted with SQL auth |
| 7 | Login disabled |
| 8 | Wrong password |
| 9 | Invalid password |
| 11 | Valid login, server access failure |
| 12 | Valid login, permission failure |
| 18 | Password change required |

## Query Execution Issues

### Query Timeout

**Symptoms:**
- `CommandTimeout` error
- Query hangs then fails

**Diagnostic Steps:**

1. **Check query execution time on server:**
   ```sql
   SET STATISTICS TIME ON;
   -- Run your query
   SET STATISTICS TIME OFF;
   ```

2. **Look for blocking:**
   ```sql
   SELECT blocking_session_id, wait_type, wait_time, wait_resource
   FROM sys.dm_exec_requests
   WHERE blocking_session_id <> 0;
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Query too slow | Optimize query, add indexes |
| Blocking | Identify and resolve blocking sessions |
| Timeout too short | Increase `Command Timeout` |
| Network latency | Check network path |

### Parameter Binding Errors

**Symptoms:**
- `Type` error
- `Conversion failed`
- Wrong data returned

**Diagnostic Steps:**

1. **Verify parameter types:**
   ```rust
   // Explicit type annotation
   let id: i32 = 123;
   client.query("SELECT * FROM t WHERE id = @p1", &[&id]).await?;
   ```

2. **Check SQL Server expected types:**
   ```sql
   SELECT name, system_type_name
   FROM sys.dm_exec_describe_first_result_set(
       N'SELECT * FROM your_table WHERE id = @p1',
       N'@p1 INT', 0
   );
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Type mismatch | Use correct Rust type for SQL type |
| NULL handling | Use `Option<T>` for nullable columns |
| String encoding | Strings are UTF-16 encoded automatically |
| Decimal precision | Use `rust_decimal::Decimal` for DECIMAL columns |

### Transaction Issues

**Symptoms:**
- `Transaction` error
- Unexpected rollback
- Deadlock (error 1205)

**Diagnostic Steps:**

1. **Check for deadlocks:**
   ```sql
   -- Enable deadlock graph capture
   DBCC TRACEON(1222, -1);

   -- Check error log for deadlocks
   EXEC xp_readerrorlog 0, 1, N'deadlock';
   ```

2. **Monitor active transactions:**
   ```sql
   SELECT session_id, transaction_id, name, transaction_begin_time
   FROM sys.dm_tran_active_transactions;
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Deadlock | Retry with backoff, use consistent access order |
| Uncommitted transaction | Ensure commit/rollback in all code paths |
| Isolation level | Consider `SNAPSHOT` isolation |
| Long transaction | Break into smaller transactions |

## Connection Pool Issues

### Pool Exhausted

**Symptoms:**
- `PoolExhausted` error
- Long wait times for connections
- Application hangs

**Diagnostic Steps:**

1. **Check pool metrics:**
   ```rust
   let metrics = pool.metrics();
   println!("Size: {}, Available: {}, In use: {}",
       metrics.pool_size, metrics.available, metrics.in_use);
   ```

2. **Look for connection leaks:**
   ```rust
   // Ensure connections are returned
   {
       let conn = pool.get().await?;
       // conn dropped here, returned to pool
   }
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Pool too small | Increase `max_connections` |
| Connection leak | Ensure connections are dropped/returned |
| Slow queries | Optimize queries, add timeouts |
| Too many concurrent requests | Add request queuing |

### Stale Connections

**Symptoms:**
- Random `ConnectionClosed` errors
- Errors after idle period
- `Connection reset by peer`

**Diagnostic Steps:**

1. **Check connection lifetime:**
   ```rust
   let pool = Pool::builder()
       .max_lifetime(Duration::from_secs(300))  // 5 minutes
       .idle_timeout(Duration::from_secs(60))   // 1 minute
       .build(config)
       .await?;
   ```

2. **Enable health checks:**
   ```rust
   let pool = Pool::builder()
       .test_on_checkout(true)
       .health_check_interval(Duration::from_secs(30))
       .build(config)
       .await?;
   ```

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Idle timeout by server | Reduce `idle_timeout` |
| Firewall timeout | Enable TCP keepalives |
| Server restart | Enable health checks |
| Azure SQL idle disconnect | Use shorter `max_lifetime` |

## Azure SQL Specific Issues

### Redirect Errors

**Symptoms:**
- `TooManyRedirects` error
- Connection loops

**Explanation:** Azure SQL Gateway redirects to actual database node. The driver handles this automatically, but issues can occur.

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Too many redirects | Check for DNS/networking issues |
| Redirect to wrong region | Verify connection string server name |
| Gateway issues | Retry with backoff |

### Resource Limits

**Symptoms:**
- Error 10928/10929 (Resource limit)
- Error 40501 (Service busy)
- Error 40613 (Database unavailable)

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Too many connections | Reduce pool size, share pool |
| DTU/vCore exhausted | Scale up Azure tier |
| Throttling | Implement retry with backoff |
| Maintenance | Wait and retry |

### Retry Strategy for Azure

```rust
use std::time::Duration;
use tokio::time::sleep;

async fn with_azure_retry<T, F, Fut>(operation: F) -> Result<T, Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let mut attempts = 0;
    let max_attempts = 5;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_transient() && attempts < max_attempts => {
                attempts += 1;
                let delay = Duration::from_secs(attempts as u64 * 5);
                tracing::warn!(
                    error = %e,
                    attempt = attempts,
                    "Transient error, retrying in {:?}",
                    delay
                );
                sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Type Conversion Issues

### DateTime Handling

**Symptoms:**
- Wrong dates/times
- Timezone issues
- Precision loss

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Timezone mismatch | Use `DATETIMEOFFSET` or store as UTC |
| Precision loss | Use `DATETIME2` instead of `DATETIME` |
| Date out of range | Check SQL Server date limits |

### Decimal Precision

**Symptoms:**
- Rounding errors
- Precision loss
- Conversion errors

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Precision loss | Enable `decimal` feature, use `rust_decimal::Decimal` |
| Scale mismatch | Specify correct scale in SQL type |
| Overflow | Use appropriate precision in SQL definition |

### NULL Handling

**Symptoms:**
- Unexpected NULL errors
- `unwrap()` panics
- Wrong default values

**Common Fixes:**

| Issue | Solution |
|-------|----------|
| Not handling NULL | Use `Option<T>` for nullable columns |
| Default on NULL | Use `.unwrap_or_default()` |
| NULL in non-nullable | Check data, use `Option<T>` |

## Debug Logging

Enable detailed logging for troubleshooting:

```bash
# Basic info
RUST_LOG=mssql_client=info cargo run

# Detailed debug
RUST_LOG=mssql_client=debug,mssql_pool=debug cargo run

# Protocol-level trace (very verbose)
RUST_LOG=mssql_client=trace,tds_protocol=trace cargo run
```

### Log Levels

| Level | What's Logged |
|-------|---------------|
| `error` | Connection failures, server errors |
| `warn` | Transient failures, retries, insecure config |
| `info` | Connection events, transactions |
| `debug` | Query execution, token parsing |
| `trace` | Packet bytes, protocol details |

## Getting Help

If you can't resolve your issue:

1. **Check existing issues:** [GitHub Issues](https://github.com/praxiomlabs/rust-mssql-driver/issues)

2. **Gather diagnostic information:**
   - Rust version (`rustc --version`)
   - Driver version (from Cargo.toml)
   - SQL Server version
   - Connection string (redact password!)
   - Full error message and backtrace
   - Minimal reproducible example

3. **Open an issue** with the above information

4. **For security issues:** See [SECURITY.md](../SECURITY.md)
