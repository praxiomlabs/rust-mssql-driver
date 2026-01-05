# Limitations & Non-Goals

This document describes known limitations and explicit non-goals of rust-mssql-driver.

For supported features, see [README.md](README.md).

---

## Quick Reference

| Category | Limitation | Alternative |
|----------|------------|-------------|
| Protocol | MARS | Use connection pooling |
| Protocol | Named Pipes / Shared Memory | Use TCP/IP |
| Protocol | True LOB Streaming | Chunked reads via SQL |
| Protocol | Server-Side Cursors | Use OFFSET/FETCH pagination |
| Data Types | GEOMETRY, GEOGRAPHY | Use `STAsText()` or `STAsGeoJSON()` |
| Data Types | HIERARCHYID | Use `.ToString()` |
| Data Types | CLR UDTs | Cast to VARBINARY |
| Platforms | SQL Server 2005 and earlier | Upgrade to SQL Server 2008+ |
| Platforms | 32-bit systems | Use 64-bit |
| Runtime | Non-Tokio runtimes | Use Tokio |

---

## Current Limitations

These are limitations with workarounds for users who need the functionality.

### Multiple Active Result Sets (MARS)

**Status:** Not supported

MARS allows multiple queries to be active simultaneously on a single connection.

**Workaround:** Use the built-in connection pool:

```rust
use mssql_driver_pool::{Pool, PoolConfig};

let pool = Pool::new(
    PoolConfig::new().max_connections(10),
    config
).await?;

// Execute queries concurrently using different connections
let (result1, result2) = tokio::join!(
    async {
        let mut conn = pool.get().await?;
        conn.query("SELECT 1", &[]).await
    },
    async {
        let mut conn = pool.get().await?;
        conn.query("SELECT 2", &[]).await
    }
);
```

---

### Large Object (LOB) Streaming

**Status:** Buffered only (no true streaming)

Large objects (VARCHAR(MAX), NVARCHAR(MAX), VARBINARY(MAX)) are fully buffered in memory.

**Workaround:** For objects over 100MB, chunk via SQL:

```sql
SELECT SUBSTRING(large_column, @offset, @chunk_size) FROM table WHERE id = @id
```

Or store large binary data externally (Azure Blob Storage, S3).

---

### Connection Thread Safety

**Status:** Single-owner only

Each `Client` instance must be owned by a single task.

**Workaround:** Use the connection pool for concurrent access:

```rust
let pool = Pool::new(PoolConfig::new().max_connections(10), config).await?;

tokio::spawn(async move {
    let mut conn = pool.get().await?;
    // Use connection
});
```

---

### Prepared Statement Cache

**Status:** LRU only (no TTL)

Cached statements are evicted by LRU policy, not time-based expiration.

**Workaround:** Configure appropriate cache size or periodically recycle connections via `idle_timeout`.

---

## SQL Server Compatibility

### Supported Versions

| TDS Version | SQL Server Version | Configuration |
|-------------|-------------------|---------------|
| TDS 7.3A | SQL Server 2008 | `TdsVersion::V7_3A` or `TDSVersion=7.3` |
| TDS 7.3B | SQL Server 2008 R2 | `TdsVersion::V7_3B` or `TDSVersion=7.3B` |
| TDS 7.4 | SQL Server 2012+ (default) | `TdsVersion::V7_4` or `TDSVersion=7.4` |
| TDS 8.0 | SQL Server 2022+ strict mode | `TdsVersion::V8_0` or `Encrypt=strict` |

### TLS Compatibility

Legacy SQL Server (2016 and earlier) may only support TLS 1.0/1.1. Since rustls requires TLS 1.2+, use `Encrypt=no_tls` for unencrypted connections on trusted networks:

```rust
let config = Config::from_connection_string(
    "Server=legacy-server;User Id=sa;Password=secret;Encrypt=no_tls"
)?;
```

⚠️ **Security Warning:** `no_tls` transmits credentials in plaintext.

### Not Supported

- **SQL Server 2005 and earlier** - TDS 7.2 protocol not implemented
- **LocalDB** - Not tested (use SQL Server Express with TCP/IP)

---

## Unsupported Data Types

### Spatial Types (GEOMETRY, GEOGRAPHY)

Returns raw CLR binary. Convert in SQL:

```sql
SELECT Location.STAsText() AS LocationWkt FROM Places;
SELECT Location.STAsGeoJSON() AS LocationJson FROM Places;  -- SQL Server 2016+
```

### HIERARCHYID

Returns raw binary. Convert in SQL:

```sql
SELECT OrgNode.ToString() AS OrgPath FROM OrgChart;
```

### CLR User-Defined Types

Returns raw binary without CLR interpretation. Cast to standard types in queries.

### Sparse Columns

Returned as base data type. Query `COLUMN_SET` explicitly if needed.

---

## Explicit Non-Goals

These features are intentionally not planned for implementation.

### Runtime Agnosticism

This driver is Tokio-native by design. Supporting multiple async runtimes (async-std, smol) would increase maintenance burden and prevent Tokio-specific optimizations.

**Alternative:** Use Tokio.

### Named Pipes / Shared Memory Transport

Windows-only protocols with limited use in modern deployments.

**Alternative:** Use TCP/IP (the default).

### Server-Side Cursors

Result set streaming is efficient; cursors add complexity without significant benefit.

**Alternative:** Use `OFFSET`/`FETCH` for pagination.

### Circuit Breaker Pattern

Not built into the driver.

**Alternative:** Use crates like `failsafe` or `backoff` in your application.

---

## Administrative Features

These SQL Server administrative features are not directly exposed:

| Feature | Status | Workaround |
|---------|--------|------------|
| Extended Events | Not exposed | Use SQL commands directly |
| Query Plans | Not exposed | Use `SET SHOWPLAN_XML ON` |
| Login Retry/Backoff | Basic only | Implement custom retry logic |

---

## Design Principles

When evaluating feature requests:

1. **Complexity vs. Value** - Features with high complexity for limited benefit are deprioritized
2. **Modern Practices** - Features obsoleted by modern alternatives are not implemented
3. **Cross-Platform** - Windows-only features are generally not supported
4. **Security** - Features with security implications receive extra scrutiny

---

## Feature Requests

If you need a feature not listed here:

1. Check [GitHub Issues](https://github.com/praxiomlabs/rust-mssql-driver/issues)
2. Open an issue with your use case
3. Consider whether a workaround exists

---

*Last updated: January 2026*
