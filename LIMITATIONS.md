# Known Limitations

This document describes known limitations of the rust-mssql-driver and recommended workarounds.

## Overview

The driver is designed for production use with SQL Server 2016+ and Azure SQL. Some advanced features are not yet implemented or have limitations.

---

## Feature Limitations

### Multiple Active Result Sets (MARS)

**Status:** Not supported in v0.x/v1.0

**Description:** MARS allows multiple queries to be active simultaneously on a single connection. This driver does not support MARS.

**Workaround:** Use the built-in connection pool to execute concurrent queries:

```rust
use mssql_driver_pool::{Pool, PoolConfig};

// Create a pool with multiple connections
let pool = Pool::builder()
    .max_size(10)  // 10 concurrent queries possible
    .build(config)
    .await?;

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

**Timeline:** MARS is planned for v2.0.

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

**Timeline:** True streaming LOB support is planned for v1.1.

---

### Always Encrypted

**Status:** Not supported

**Description:** SQL Server's Always Encrypted feature (client-side encryption of sensitive columns) is not implemented.

**Workaround:** Use application-layer encryption:

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::Aead;

// Encrypt before insert
let ciphertext = cipher.encrypt(nonce, plaintext)?;
client.execute("INSERT INTO users (encrypted_data) VALUES (@p1)", &[&ciphertext]).await?;

// Decrypt after select
let rows = client.query("SELECT encrypted_data FROM users WHERE id = @p1", &[&id]).await?;
let ciphertext: Vec<u8> = row.get(0)?;
let plaintext = cipher.decrypt(nonce, &ciphertext)?;
```

**Important:** Do NOT use SQL Server's `ENCRYPTBYKEY` as a workaround. It provides different security guarantees (keys stored on server, DBAs can access plaintext).

**Timeline:** Always Encrypted is planned for v2.0.

---

### Kerberos/Windows Authentication

**Status:** Not supported

**Description:** Kerberos (GSSAPI) and NTLM authentication for Windows domain environments are not implemented.

**Supported Authentication Methods:**
- SQL Server authentication (username/password)
- Azure Active Directory with access token

**Workaround:**

1. **SQL Authentication:** Create SQL logins for application access:
   ```sql
   CREATE LOGIN app_user WITH PASSWORD = 'StrongPassword!';
   CREATE USER app_user FOR LOGIN app_user;
   ```

2. **Azure AD Token:** For Azure SQL, use Azure AD authentication with a pre-obtained token:
   ```rust
   let config = Config::builder()
       .host("your-server.database.windows.net")
       .authentication(Authentication::AadToken(token))
       .build()?;
   ```

**Timeline:** Kerberos support is planned for v1.1.

---

### Query Cancellation

**Status:** Limited support

**Description:** Active queries cannot be cancelled mid-execution. The TDS ATTENTION signal is not implemented.

**Workaround:** Use query timeouts:

```rust
let config = Config::builder()
    .host("localhost")
    .query_timeout(Duration::from_secs(30))  // Query timeout
    .build()?;
```

For long-running queries, consider:
1. Breaking work into smaller batches
2. Using SQL Server's query governor
3. Implementing application-level polling patterns

**Timeline:** Query cancellation is planned for v1.1.

---

## SQL Server Version Limitations

### SQL Server 2014 and Earlier

**Status:** Not supported

**Description:** SQL Server versions before 2016 use TDS protocol versions that are not fully tested with this driver.

**Reason:** These versions are past their extended support lifecycle. Focus is on modern SQL Server.

**Workaround:** Upgrade to SQL Server 2016 or later.

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

## Performance Limitations

### Bulk Copy (BCP)

**Status:** In development (partial implementation)

**Description:** The bulk copy protocol for high-speed data loading is partially implemented.

**Workaround:** Use parameterized INSERT statements with batching:

```rust
// Batch inserts for better performance
for chunk in data.chunks(1000) {
    let mut tx = client.begin_transaction().await?;
    for record in chunk {
        tx.execute(
            "INSERT INTO table (col1, col2) VALUES (@p1, @p2)",
            &[&record.col1, &record.col2]
        ).await?;
    }
    tx.commit().await?;
}
```

**Timeline:** Full BCP support is planned for v1.0.

---

### Prepared Statement Memory

**Status:** LRU cache only

**Description:** Prepared statements use an LRU cache. There is no TTL-based expiration.

**Impact:** Long-running connections with varied query patterns may accumulate stale prepared statements.

**Workaround:**
- Configure appropriate cache size
- Periodically recycle connections (pool handles this automatically)

---

## Concurrency Limitations

### Connection Thread Safety

**Status:** Single-owner only

**Description:** Each `Client` instance must be owned by a single task. The connection cannot be shared across threads simultaneously.

**Workaround:** Use the connection pool for concurrent access:

```rust
// Pool provides safe concurrent access
let pool = Pool::builder().max_size(10).build(config).await?;

// Each task gets its own connection
tokio::spawn(async move {
    let conn = pool.get().await?;
    // Use connection
});
```

---

## Timeout Limitations

### No Per-Query Timeout Override

**Status:** Global only

**Description:** Query timeout is set at connection configuration time. Individual queries cannot override it.

**Workaround:** Use different connections with different timeout configurations, or implement application-level timeout logic:

```rust
match tokio::time::timeout(Duration::from_secs(5), client.query(sql, &[])).await {
    Ok(result) => result?,
    Err(_) => return Err(Error::Timeout),
}
```

---

## Reporting Issues

If you encounter a limitation not documented here, please:

1. Check the [GitHub Issues](https://github.com/rust-mssql-driver/rust-mssql-driver/issues) for existing reports
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
