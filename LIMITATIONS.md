# Known Limitations

This document describes known limitations of the rust-mssql-driver and recommended workarounds.

## Overview

The driver is designed for production use with SQL Server 2016+ and Azure SQL. Most common features are fully implemented, but some advanced features have limitations.

---

## Feature Limitations

### Multiple Active Result Sets (MARS)

**Status:** Not supported

**Description:** MARS allows multiple queries to be active simultaneously on a single connection. This driver does not support MARS.

**Workaround:** Use the built-in connection pool to execute concurrent queries:

```rust
use mssql_driver_pool::{Pool, PoolConfig};

// Create a pool with multiple connections
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

**Timeline:** MARS support may be considered for a future major version.

---

### Large Object (LOB) Streaming

**Status:** Buffered only (no true streaming)

**Description:** Large objects (VARCHAR(MAX), NVARCHAR(MAX), VARBINARY(MAX)) are fully buffered in memory before being returned to the application.

**Workaround:** For objects under 100MB, the current implementation is acceptable. For larger objects:

1. **Chunked Reads:** Read large data in chunks using SQL:
   ```sql
   SELECT SUBSTRING(large_column, @offset, @chunk_size) FROM table WHERE id = @id
   ```

2. **External Storage:** Store large binary data externally (Azure Blob Storage, S3) and keep only references in SQL Server.

3. **Memory Budget:** Ensure your application has sufficient memory headroom for the largest expected object.

**Timeline:** True streaming LOB support may be considered for a future version.

---

### Always Encrypted

**Status:** Fully Supported (v0.2.0+)

**Description:** SQL Server's Always Encrypted feature is fully implemented with client-side encryption and multiple key provider options.

**Implemented Features:**
- AEAD_AES_256_CBC_HMAC_SHA256 encryption/decryption
- RSA-OAEP key unwrapping for CEK decryption
- Column Encryption Key (CEK) caching with TTL
- Deterministic and randomized encryption types

**Available Key Providers:**
- **InMemoryKeyStore** - For development and testing
- **AzureKeyVaultProvider** - Azure Key Vault integration (`azure-identity` feature)
- **WindowsCertStoreProvider** - Windows Certificate Store (`sspi-auth` feature, Windows only)
- **Custom providers** - Implement the `KeyStoreProvider` trait

**Usage Example:**

```rust
use mssql_client::{Client, Config};
use mssql_auth::{AlwaysEncrypted, InMemoryKeyStore};

// Create a key store with your Column Master Key
let mut key_store = InMemoryKeyStore::new();
key_store.add_key("CMK1", &master_key_bytes);

// Configure Always Encrypted
let config = Config::from_connection_string(
    "Server=localhost;Database=mydb;User Id=sa;Password=secret;"
)?
.with_always_encrypted(AlwaysEncrypted::new(key_store));

let client = Client::connect(config).await?;

// Encrypted columns are automatically decrypted on read
// and encrypted on write
let rows = client.query(
    "SELECT SSN FROM employees WHERE id = @p1",
    &[&employee_id]
).await?;
```

**Important:** Do NOT use SQL Server's `ENCRYPTBYKEY` as an alternative. Always Encrypted provides client-side encryption where keys never leave the client, protecting data even from DBAs and server compromise. `ENCRYPTBYKEY` stores keys on the server and provides different (weaker) security guarantees.

See [SECURITY.md](SECURITY.md#always-encrypted-considerations) for threat model details.

---

## SQL Server Version Limitations

### SQL Server 2008/2008 R2/2012/2014 (TDS 7.3/7.4)

**Status:** Supported via TDS version configuration

**Description:** SQL Server 2008 and later versions are supported by configuring the TDS protocol version. The driver defaults to TDS 7.4 (SQL Server 2012+) for broad compatibility.

**Configuration:**

```rust
use mssql_client::Config;
use tds_protocol::version::TdsVersion;

// For SQL Server 2008
let config = Config::new()
    .host("legacy-server")
    .tds_version(TdsVersion::V7_3A);

// For SQL Server 2008 R2
let config = Config::new()
    .host("legacy-server")
    .tds_version(TdsVersion::V7_3B);

// Connection string syntax
let config = Config::from_connection_string(
    "Server=localhost;TDSVersion=7.3;User Id=sa;Password=secret;"
)?;
```

**Supported TDS Versions:**

| TDS Version | SQL Server Version | Configuration |
|-------------|-------------------|---------------|
| TDS 7.3A | SQL Server 2008 | `TdsVersion::V7_3A` or `TDSVersion=7.3` |
| TDS 7.3B | SQL Server 2008 R2 | `TdsVersion::V7_3B` or `TDSVersion=7.3B` |
| TDS 7.4 | SQL Server 2012+ (default) | `TdsVersion::V7_4` or `TDSVersion=7.4` |
| TDS 8.0 | SQL Server 2022+ strict mode | `TdsVersion::V8_0` or `Encrypt=strict` |

**Data Type Availability:**

| Feature | TDS 7.3+ | TDS 7.4+ |
|---------|----------|----------|
| DATE, TIME, DATETIME2, DATETIMEOFFSET | ✅ | ✅ |
| Session Recovery | ❌ | ✅ |
| Column Encryption (Always Encrypted) | ❌ | ✅ |
| UTF-8 Collations | ❌ | ✅ (SQL 2019+) |

**Note:** While TDS 7.3 is supported, SQL Server 2008/2008 R2 reached end of extended support. Consider upgrading when possible.

**TLS Compatibility:**

Legacy SQL Server instances (2012 and earlier) may only support TLS 1.0/1.1. Since this driver uses rustls (which requires TLS 1.2+), encrypted connections to these servers will fail with a TLS handshake error.

For legacy servers that cannot be upgraded, use the `no_tls` option to disable encryption entirely:

```rust
// Connection string
let config = Config::from_connection_string(
    "Server=legacy-server;User Id=sa;Password=secret;Encrypt=no_tls"
)?;

// Builder API
let config = Config::new()
    .host("legacy-server")
    .no_tls(true);
```

⚠️ **Security Warning:** `no_tls` transmits credentials and data in plaintext. Only use on isolated, trusted networks with legacy SQL Server instances that cannot support TLS 1.2+.

---

### SQL Server 2005 and Earlier (TDS 7.2 and Earlier)

**Status:** Not supported

**Description:** SQL Server 2005 and earlier use legacy TDS protocol versions (7.2 and earlier) that are not supported by this driver.

**Reason:** These versions are significantly past their extended support lifecycle and have different protocol behaviors.

**Workaround:** Upgrade to SQL Server 2008 or later and use TDS 7.3.

---

### SQL Server Express LocalDB

**Status:** Not tested

**Description:** LocalDB uses a different connection mechanism that has not been tested.

**Workaround:** Use a full SQL Server Express instance with TCP/IP enabled.

---

## Platform Limitations

### 32-bit Platforms

**Status:** Not supported

**Description:** The driver is only tested and supported on 64-bit platforms.

**Reason:** Modern SQL Server deployments are 64-bit. 32-bit support adds testing burden with minimal benefit.

---

### Named Pipes / Shared Memory

**Status:** Not supported

**Description:** Only TCP/IP connections are supported. Named Pipes and Shared Memory protocols are not implemented.

**Workaround:** Enable TCP/IP in SQL Server Configuration Manager:

1. Open SQL Server Configuration Manager
2. Navigate to SQL Server Network Configuration > Protocols
3. Enable TCP/IP
4. Restart SQL Server service

---

## Performance Considerations

### Prepared Statement Cache

**Status:** LRU cache only

**Description:** Prepared statements use an LRU cache with configurable size. There is no TTL-based expiration.

**Impact:** Long-running connections with varied query patterns may accumulate stale prepared statements.

**Workaround:**
- Configure appropriate cache size via connection settings
- Periodically recycle connections (the pool handles this automatically via `idle_timeout`)

---

## Concurrency Considerations

### Connection Thread Safety

**Status:** Single-owner only

**Description:** Each `Client` instance must be owned by a single task. The connection cannot be shared across threads simultaneously.

**Workaround:** Use the connection pool for concurrent access:

```rust
// Pool provides safe concurrent access
let pool = Pool::new(PoolConfig::new().max_connections(10), config).await?;

// Each task gets its own connection
tokio::spawn(async move {
    let mut conn = pool.get().await?;
    // Use connection
});
```

---

## Reporting Issues

If you encounter a limitation not documented here, please:

1. Check the [GitHub Issues](https://github.com/praxiomlabs/rust-mssql-driver/issues) for existing reports
2. Open a new issue with:
   - SQL Server version
   - Driver version
   - Minimal reproduction case
   - Expected vs actual behavior

---

## Feature Requests

For feature requests related to limitations:

1. Open a GitHub issue with the `enhancement` label
2. Describe the use case and business need
3. Reference this document if applicable

We prioritize features based on community demand and production impact.
