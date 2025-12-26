# Unsupported Features

This document lists features that are explicitly **not supported** by rust-mssql-driver, along with the rationale and recommended alternatives.

## Quick Reference: Feature Status

For clarity, here's what **IS** implemented in v0.3.x:

### Authentication (Tier 1-4) ✅
- SQL Server authentication (username/password)
- Azure AD with pre-acquired token
- Azure Managed Identity (`azure-identity` feature)
- Azure Service Principal (`azure-identity` feature)
- Kerberos/GSSAPI (`integrated-auth` feature)
- Client Certificate (`cert-auth` feature)

### Connection Features ✅
- Per-query timeouts (`query_with_timeout()`, `execute_with_timeout()`)
- Query cancellation with ATTENTION packets (`cancel_handle()`)
- Connection pooling with metrics
- Statement caching with LRU eviction

### Observability ✅
- OpenTelemetry tracing spans (`otel` feature)
- OpenTelemetry metrics (`otel` feature, `DatabaseMetrics`)
- SQL statement sanitization for safe logging
- Pool metrics (via `pool.status()` and `pool.metrics()`)

---

## Explicit Non-Goals

The following features are intentionally not planned for implementation:

### 1. MARS (Multiple Active Result Sets)

**Status:** Not Planned

**Rationale:** MARS adds significant protocol complexity for limited benefit. It allows multiple active statements on a single connection, but:
- Increases connection state complexity
- Makes cancellation semantics ambiguous
- Most applications work fine with connection pooling instead

**Alternative:** Use connection pooling (`mssql-driver-pool`) to run concurrent queries on separate connections. This is simpler, more predictable, and often performs better.

### 2. Runtime Agnosticism

**Status:** Not Planned

**Rationale:** This driver is Tokio-native by design. Supporting multiple async runtimes (async-std, smol) would:
- Increase maintenance burden
- Prevent optimizations specific to Tokio
- Add conditional compilation complexity

**Alternative:** Use Tokio. It's the dominant async runtime in the Rust ecosystem.

### 3. SQL Server 2008 / 2012 Support

**Status:** Not Planned

**Rationale:** These versions are out of extended support from Microsoft. Supporting them would:
- Require TDS 7.1/7.2 protocol variants
- Prevent use of modern TDS features
- Increase testing matrix significantly

**Alternative:** Upgrade to SQL Server 2014+ (TDS 7.4) or SQL Server 2022 (TDS 8.0).

### 4. Windows SSPI Authentication

**Status:** ✅ Implemented (Cross-Platform)

The `sspi-auth` feature provides SSPI authentication via the sspi-rs crate, which works on:
- Windows (native SSPI)
- Linux/macOS (NTLM emulation via sspi-rs)

```rust
use mssql_auth::SspiAuth;

// Integrated auth (current user)
let auth = SspiAuth::new("sqlserver.example.com", 1433)?;

// Explicit credentials
let auth = SspiAuth::with_credentials(
    "sqlserver.example.com",
    1433,
    "DOMAIN\\user",
    "password",
)?;
```

**Also Supported:** Kerberos/GSSAPI authentication on Linux/macOS via the `integrated-auth` feature.

### 5. Named Pipes Transport

**Status:** Not Planned

**Rationale:** Named pipes are:
- Windows-only
- Rarely used in modern deployments
- TCP/IP is universally supported and performs similarly

**Alternative:** Use TCP/IP connections (the default).

### 6. Shared Memory Transport

**Status:** Not Planned

**Rationale:** Shared memory transport is:
- Windows-only (localhost connections)
- Undocumented in the public TDS specification
- Limited to same-machine scenarios where TCP/IP localhost works equally well

**Alternative:** Use TCP/IP connections to `localhost` or `127.0.0.1`.

### 7. Linked Servers / Distributed Queries

**Status:** Not Planned (Driver-Level)

**Rationale:** Linked server queries work at the SQL Server level, not the driver level. The driver executes whatever SQL you send.

**Alternative:** Your queries can use linked servers - the driver doesn't need special support.

---

## Unsupported Data Types

The following SQL Server data types are not currently supported:

### Spatial Types

| Type | Status | Rationale |
|------|--------|-----------|
| `GEOMETRY` | Not Supported | Complex binary format, requires geometric computation library |
| `GEOGRAPHY` | Not Supported | Complex binary format, requires geodetic computation library |

**Workaround:** Convert to WKT (Well-Known Text) or GeoJSON in SQL:

```sql
-- Return as WKT string
SELECT Location.STAsText() AS LocationWkt FROM Places;

-- Return as GeoJSON (SQL Server 2016+)
SELECT Location.STAsGeoJSON() AS LocationJson FROM Places;
```

### Hierarchical Types

| Type | Status | Rationale |
|------|--------|-----------|
| `HIERARCHYID` | Not Supported | Proprietary binary format with complex path operations |

**Workaround:** Convert to string representation in SQL:

```sql
SELECT OrgNode.ToString() AS OrgPath FROM OrgChart;
```

### User-Defined Types (UDT)

| Type | Status | Rationale |
|------|--------|-----------|
| CLR UDTs | ⚠️ Partial | Returns raw binary - CLR interpretation not supported |
| Alias Types | ✅ Supported | Treated as their underlying base type |

**Note:** CLR UDTs (including GEOMETRY, GEOGRAPHY, HIERARCHYID) are returned as raw binary data. The driver does not interpret the CLR binary format.

**Workaround:** Convert UDTs to standard types in your queries:

```sql
-- Spatial types
SELECT Location.STAsText() AS LocationWkt FROM Places;
SELECT Location.STAsGeoJSON() AS LocationJson FROM Places;

-- HierarchyID
SELECT OrgNode.ToString() AS OrgPath FROM OrgChart;

-- Custom CLR UDTs
SELECT CAST(MyUdt AS VARBINARY(MAX)) FROM MyTable;
```

### Sparse Columns

**Status:** Partial Support

Sparse columns are returned with their base data type. The `COLUMN_SET` XML representation is not automatically generated.

**Workaround:** Query the column set explicitly if needed:

```sql
SELECT *, SparseColumnSet FROM TableWithSparseColumns;
```

---

## Platform Limitations

### SQL Server Express LocalDB

**Status:** Not Tested

LocalDB uses a different connection mechanism (automatic instance management via Windows named pipes). While TCP/IP connections to LocalDB may work, this configuration is not tested.

**Alternative:** Use a full SQL Server Express instance with TCP/IP enabled, or use Docker containers for local development.

### 32-bit Platforms

**Status:** Not Supported

The driver is only tested on 64-bit platforms (x86_64, aarch64). 32-bit builds may work but are not part of the CI matrix.

**Rationale:** 32-bit systems are increasingly rare in production environments, and testing resources are limited.

---

## Connection Pool Limitations

### Thread Sharing

**Status:** Not Supported

`Client<S>` is single-owner by design. Connections cannot be shared between tasks without explicit synchronization.

**Rationale:** This simplifies the state machine and prevents race conditions in query execution.

**Alternative:** Use `Pool` to manage multiple connections, acquiring one per task as needed.

### Pool Metrics/Instrumentation

**Status:** ✅ Comprehensive Metrics Available

The pool exposes metrics via `pool.status()` and `pool.metrics()`:

| Metric | Status |
|--------|--------|
| Pool size (total connections) | ✅ Available via `status().total` |
| Available connections | ✅ Available via `status().available` |
| In-use connections | ✅ Available via `status().in_use` |
| Wait queue depth | ✅ Available via `status().wait_queue_depth` |
| Connections created | ✅ Available via `metrics().connections_created` |
| Connections closed | ✅ Available via `metrics().connections_closed` |
| Checkout success/failure | ✅ Available via `metrics().checkouts_*` |
| Health check stats | ✅ Available via `metrics().health_checks_*` |
| Reset stats | ✅ Available via `metrics().resets_*` |
| Uptime | ✅ Available via `metrics().uptime` |
| Connection acquisition time | ✅ Available via `metrics().avg_acquisition_time_us` |
| Idle timeout expirations | ✅ Available via `metrics().connections_idle_expired` |
| Lifetime expirations | ✅ Available via `metrics().connections_lifetime_expired` |
| Peak wait queue depth | ✅ Available via `metrics().peak_wait_queue_depth` |
| Reaper task runs | ✅ Available via `metrics().reaper_runs` |

**Note:** OpenTelemetry metrics integration is available via the `otel` feature (see `DatabaseMetrics`).

### TTL-Based Connection Expiration

**Status:** ✅ Implemented

The pool includes a background reaper task that:
- Removes connections exceeding `max_lifetime` (default: 30 minutes)
- Removes idle connections exceeding `idle_timeout` (default: 10 minutes)
- Maintains minimum connection count (`min_connections`)

Configuration:
```rust
let config = PoolConfig::new()
    .max_lifetime(Duration::from_secs(1800))  // 30 minutes
    .idle_timeout(Duration::from_secs(600));   // 10 minutes
```

### Custom Health Checks

**Status:** ✅ Implemented

The pool supports custom health check queries (default: `SELECT 1`):

```rust
use mssql_driver_pool::PoolConfig;

// Check specific database exists
let config = PoolConfig::new()
    .health_check_query("SELECT 1 FROM sys.databases WHERE name = 'mydb'");

// Check server can execute functions
let config = PoolConfig::new()
    .health_check_query("SELECT GETDATE()");
```

---

## Statement Cache Limitations

### Cross-Connection Statement Sharing

**Status:** Not Supported (By Design)

Prepared statement handles are connection-specific in SQL Server. Each connection maintains its own LRU statement cache.

**Rationale:** Sharing would require complex coordination and could lead to race conditions.

### TTL-Based Statement Expiration

**Status:** Not Supported

Cached statements are evicted based on LRU policy, not time-based expiration.

**Rationale:** LRU naturally evicts stale statements as new ones are prepared. Time-based expiration adds complexity without significant benefit.

---

## Administrative Features

The following administrative and diagnostic features are not directly exposed:

### Extended Events Integration

**Status:** Not Supported

There is no API for programmatic Extended Events session management.

**Workaround:** Use SQL commands to manage Extended Events sessions:

```sql
CREATE EVENT SESSION [MySession] ON SERVER ...
```

### Query Plan Retrieval

**Status:** Not Exposed

There is no direct API to retrieve query plans.

**Workaround:** Use SHOWPLAN options:

```sql
SET SHOWPLAN_XML ON;
-- Your query here
SET SHOWPLAN_XML OFF;
```

Or query the plan cache:

```sql
SELECT query_plan FROM sys.dm_exec_query_plan(plan_handle);
```

### Login Retry/Backoff Configuration

**Status:** Basic Only

Connection retry uses a simple fixed policy. Exponential backoff with jitter is not configurable.

**Planned:** Consider using a retry middleware or implementing custom retry logic in your application.

### Circuit Breaker Pattern

**Status:** Not Implemented

There is no built-in circuit breaker for failing connections.

**Alternative:** Implement circuit breaker logic in your application using crates like `failsafe` or `backoff`.

---

## Features Planned for Future Releases

The following features are planned but not yet implemented:

### Implemented in v0.3.0

| Feature | Description |
|---------|-------------|
| Always Encrypted (Key Providers) | Azure KeyVault via `azure-keyvault` feature, Windows CertStore via `windows-certstore` feature |
| Change Tracking Integration | `ChangeTrackingQuery` builder, `ChangeOperation` enum, helper SQL generators |
| LOB Streaming API | `Row::get_stream()` method returning `BlobReader` implementing `AsyncRead` |

### Implemented in v0.2.3

| Feature | Description |
|---------|-------------|
| TTL-Based Pool Expiration | Background reaper task with idle/lifetime expiration |
| Custom Health Checks | Configurable health check query |
| Wait Queue Metrics | Wait queue depth and peak tracking |
| Acquisition Time Metrics | Average connection acquisition time |

### Implemented in v0.2.0

| Feature | Description |
|---------|-------------|
| Table-Valued Parameters (TVP) | Pass structured data as parameters via `Tvp` type |
| Always Encrypted (Cryptography) | AEAD encryption, RSA-OAEP key unwrap via `always-encrypted` feature |
| OpenTelemetry Metrics | Counter/histogram metrics via `DatabaseMetrics` |
| Windows SSPI Authentication | Cross-platform SSPI support via `sspi-auth` feature |
| Query Cancellation | Mid-query cancel via `cancel_handle()` with ATTENTION packets |
| Per-Query Timeouts | `query_with_timeout()`, `execute_with_timeout()` methods |

### Workarounds

#### Always Encrypted

Always Encrypted is fully supported with the `always-encrypted` feature:
- AEAD_AES_256_CBC_HMAC_SHA256 encryption/decryption
- RSA-OAEP key unwrapping for CEK decryption
- CEK caching with TTL expiration
- `InMemoryKeyStore` for testing/development
- `AzureKeyVaultProvider` for production (via `azure-keyvault` feature)
- `WindowsCertStoreProvider` for Windows environments (via `windows-certstore` feature)

Custom key stores can be implemented via the `KeyStoreProvider` trait.

**Important:** Do NOT use `ENCRYPTBYKEY` as a workaround - it does not provide the same security guarantees (keys are accessible to DBAs).

---

## Protocol Limitations

### Large Object (LOB) Streaming

**Current Status:** Streaming API Available

LOBs (VARBINARY(MAX), NVARCHAR(MAX), XML) are buffered from the network but provide a streaming API via `BlobReader`:

```rust
// Get streaming reader for a VARBINARY(MAX) column
let mut reader = row.get_stream(0)?;

// Stream to file without holding entire blob in user code
let mut file = tokio::fs::File::create("output.bin").await?;
tokio::io::copy(&mut reader, &mut file).await?;
```

**For LOBs over 100MB:** Consider chunking via SQL to reduce memory pressure:

```sql
-- Read in chunks
SELECT SUBSTRING(Data, @offset, @length) FROM Documents WHERE Id = @id;
```

### Cursor Support

**Status:** Not Implemented

Server-side cursors are not directly supported. However:
- Result set streaming is supported and efficient
- For large datasets, consider pagination with `OFFSET`/`FETCH`

---

## Why Not Support X?

If you're wondering why a specific feature isn't supported, the general principles are:

1. **Complexity vs. Value:** Features that add significant complexity for limited benefit are deprioritized
2. **Modern Practices:** Features obsoleted by modern alternatives are not implemented
3. **Cross-Platform:** Windows-only features are generally not supported
4. **Security:** Features with security implications receive extra scrutiny

## Feature Requests

If you need a feature not listed here:

1. Check if it's already tracked in [GitHub Issues](https://github.com/praxiomlabs/rust-mssql-driver/issues)
2. Open an issue with your use case
3. Consider whether a workaround exists

We prioritize features based on community need and alignment with the driver's goals.

---

## Appendix: Comprehensive Feature Matrix

### Authentication Matrix

| Method | Feature Flag | Status | Notes |
|--------|--------------|--------|-------|
| SQL Server (username/password) | default | ✅ Implemented | Login7 with password obfuscation |
| Azure AD Token | default | ✅ Implemented | Pre-acquired JWT token |
| Azure Managed Identity | `azure-identity` | ✅ Implemented | System/User-assigned identity |
| Azure Service Principal | `azure-identity` | ✅ Implemented | Client ID + Secret |
| Kerberos/GSSAPI | `integrated-auth` | ✅ Implemented | Linux/macOS via libgssapi |
| Client Certificate (mTLS) | `cert-auth` | ✅ Implemented | X.509 via Azure AD |
| Windows SSPI | `sspi-auth` | ✅ Implemented | Cross-platform via sspi-rs |
| Azure CLI Credentials | `azure-identity` | ✅ Implemented | Via azure_identity crate |

### Protocol Features Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| TDS 7.4 (SQL Server 2016+) | ✅ Implemented | Default protocol |
| TDS 8.0 (SQL Server 2022+) | ✅ Implemented | Strict encryption mode |
| Query Cancellation (ATTENTION) | ✅ Implemented | Mid-query cancel via `cancel_handle()` |
| Per-Query Timeouts | ✅ Implemented | `query_with_timeout()`, `execute_with_timeout()` |
| Prepared Statements | ✅ Implemented | Auto-cached with LRU eviction |
| Connection Pooling | ✅ Implemented | Built-in with metrics |
| Transaction Savepoints | ✅ Implemented | Validated identifiers |
| Azure SQL Redirect | ✅ Implemented | Automatic gateway redirect handling |
| MARS | ❌ Not Planned | Use pooling instead |
| Named Pipes | ❌ Not Planned | Windows-only |
| Shared Memory | ❌ Not Planned | Undocumented protocol |

### Data Types Matrix

| Type | Status | Notes |
|------|--------|-------|
| INT, BIGINT, SMALLINT, TINYINT | ✅ Implemented | Full range support |
| FLOAT, REAL | ✅ Implemented | IEEE 754 |
| DECIMAL, NUMERIC | ✅ Implemented | Via rust_decimal |
| VARCHAR, NVARCHAR | ✅ Implemented | Including MAX variants |
| VARBINARY | ✅ Implemented | Including MAX variants |
| DATE, TIME, DATETIME2 | ✅ Implemented | Via chrono crate |
| DATETIMEOFFSET | ✅ Implemented | Timezone-aware |
| UNIQUEIDENTIFIER (GUID) | ✅ Implemented | Via uuid crate |
| BIT | ✅ Implemented | Boolean mapping |
| XML | ✅ Implemented | As String |
| JSON (NVARCHAR) | ✅ Implemented | As String, parse in app |
| Table-Valued Parameters | ✅ Implemented | Via `Tvp` type |
| Geometry/Geography | ⚠️ Partial | Returns raw binary, use `STAsText()`/`STAsGeoJSON()` |
| HierarchyID | ⚠️ Partial | Returns raw binary, use `.ToString()` |
| User-Defined Types (CLR) | ⚠️ Partial | Returns raw binary (no CLR interpretation) |

### Connection Pool Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| Min/Max Connections | ✅ Implemented | Configurable |
| Connection Timeout | ✅ Implemented | Configurable |
| Idle Timeout | ✅ Implemented | Background reaper task |
| Max Lifetime | ✅ Implemented | Background reaper task |
| `sp_reset_connection` | ✅ Implemented | On connection return |
| Health Checks | ✅ Implemented | Configurable query (default: `SELECT 1`) |
| Pool Metrics | ✅ Implemented | Comprehensive via `pool.status()` and `pool.metrics()` |
| TTL-Based Eviction | ✅ Implemented | Background reaper with idle/lifetime expiration |
| Wait Queue Tracking | ✅ Implemented | Depth and peak metrics |
| Acquisition Time Tracking | ✅ Implemented | Average acquisition time |

### Observability Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| Tracing Spans | ✅ Implemented | `otel` feature, OpenTelemetry 0.31+ |
| SQL Sanitization | ✅ Implemented | Configurable for safe logging |
| Error Recording | ✅ Implemented | Via span events |
| Semantic Conventions | ✅ Implemented | Following OTel DB conventions |
| Metrics (Counters) | ✅ Implemented | `otel` feature, `DatabaseMetrics` |
| Metrics (Histograms) | ✅ Implemented | `otel` feature, `OperationTimer` |

### Security Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| TLS Encryption | ✅ Implemented | Via rustls |
| TLS 1.2/1.3 | ✅ Implemented | Configurable |
| Certificate Validation | ✅ Implemented | Configurable |
| Credential Zeroization | ✅ Implemented | `zeroize` feature |
| SQL Injection Prevention | ✅ Implemented | Parameterized queries |
| Always Encrypted (Cryptography) | ✅ Implemented | AEAD, RSA-OAEP, CEK caching |
| Always Encrypted (Key Providers) | ⏳ Planned v0.3.0 | Azure KeyVault, Windows CertStore |

### SQL Server Version Support

| Version | Status | Notes |
|---------|--------|-------|
| SQL Server 2022 | ✅ Supported | TDS 8.0 |
| SQL Server 2019 | ✅ Supported | TDS 7.4 |
| SQL Server 2017 | ✅ Supported | TDS 7.4 |
| SQL Server 2016 | ✅ Supported | TDS 7.4 (minimum) |
| SQL Server 2014 and earlier | ❌ Not Supported | Past EOL |
| Azure SQL Database | ✅ Supported | With all auth methods |
| Azure SQL Managed Instance | ✅ Supported | With redirect handling |

---

*Last updated: December 2024*
