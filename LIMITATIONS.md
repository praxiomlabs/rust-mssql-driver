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
| Protocol | Incremental result streaming (whole response is buffered) | Page with OFFSET/FETCH |
| Protocol | Server-Side Cursors | Use OFFSET/FETCH pagination |
| Data Types | NUMERIC/DECIMAL beyond 28-29 significant digits | CAST to narrower NUMERIC, FLOAT, or VARCHAR |
| Collations | OEM code pages CP437 / CP850 (legacy SQL collations) | Use a CP125x or UTF-8 collation, or CAST to NVARCHAR |
| Data Types | GEOMETRY, GEOGRAPHY | Use `STAsText()` or `STAsGeoJSON()` |
| Data Types | HIERARCHYID | Use `.ToString()` |
| Data Types | CLR UDTs | Cast to VARBINARY |
| Performance | Prepared statement cache (not wired) | `sp_executesql` server plan cache |
| Auth | Certificate auth and ADAL/MSAL workflows (FEDAUTH Phase 2) | Azure AD token / service principal / managed identity, SQL auth, or NTLM |
| Auth | Kerberos untested against live KDC | SQL auth or NTLM |
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

### Result Set Buffering (No Incremental Streaming)

**Status:** The full server response is buffered in memory; rows decode lazily

`query()` reads the entire server response into one buffer before returning.
The returned result set then *decodes* each row lazily as you iterate, so
peak memory tracks the raw response size (not the response plus a fully
typed `Vec<Row>`) — but it does **not** bound memory to a single row the way
true incremental network streaming would. A multi-GB result set means
multi-GB resident memory. There is currently no maximum-response-size guard.

**Workaround:** Page the query with `OFFSET`/`FETCH` so each fetch is
bounded:

```sql
SELECT ... FROM t ORDER BY id OFFSET @skip ROWS FETCH NEXT @take ROWS ONLY
```

---

### Large Object (LOB) Streaming

**Status:** Buffered only (no true streaming)

Large objects (VARCHAR(MAX), NVARCHAR(MAX), VARBINARY(MAX)) are fully buffered in memory
(this is a specific case of the whole-response buffering described above).

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

**Status:** Not wired — all parameterized queries use `sp_executesql`

An LRU statement cache exists in the codebase but is not consulted by any
query path: the driver never issues `sp_prepare`/`sp_execute`. Every
parameterized query goes through `sp_executesql`, which still benefits from
SQL Server's server-side plan cache, so repeated queries reuse plans —
there is just no client-side handle caching yet.

**Workaround:** None needed for plan reuse (`sp_executesql` provides it).
Client-side handle caching is planned.

---

### Certificate Authentication and ADAL/MSAL Workflows (FEDAUTH Phase 2)

**Status:** Azure AD logins implemented (SecurityToken workflow);
certificate credentials and server-directed token acquisition not wired

Azure AD / Entra credentials — pre-acquired access tokens, Managed
Identity, and Service Principals — complete logins via the LOGIN7 FEDAUTH
feature extension (SecurityToken workflow: the token is acquired
client-side before login; validated against live Azure SQL).

Still unwired:

- **Client certificate credentials** (`cert-auth`): token acquisition
  works, but `Client::connect` rejects the credential type with a clear
  configuration error.
- **ADAL/MSAL workflows** (`Authentication=ActiveDirectoryPassword`,
  `ActiveDirectoryInteractive`, …): these need the FEDAUTHINFO
  round-trip in which the server directs token acquisition; the
  connection-string parser rejects them with a pointer to the tracking
  issue.

Both are tracked in
[#155](https://github.com/praxiomlabs/rust-mssql-driver/issues/155)
(Phase 2).

**Workaround:** a service principal secret or a pre-acquired token covers
most certificate-credential scenarios; SQL authentication works on every
Azure SQL tier.

---

### Kerberos / Integrated Authentication (Untested Live)

**Status:** Implemented but never validated against a live KDC

The `integrated-auth` feature (Kerberos/GSSAPI via libgssapi) compiles and
has unit tests, but no end-to-end authentication against a real KDC or
domain-joined SQL Server has been performed. Treat it as experimental until
live validation lands.

**Workaround:** SQL authentication and cross-platform NTLM are the
validated paths.

---

### Legacy OEM Code Page Collations (CP437, CP850)

**Status:** Not decodable; the code page is reported but no transcoding occurs

SQL collations whose SortId maps to the OEM code pages 437 or 850 (e.g.
`SQL_Latin1_General_CP850_BIN`, `SQL_Latin1_General_CP437_CI_AS`) cannot be
transcoded: [`encoding_rs`] implements the WHATWG encoding set, which does
not include CP437 or CP850. `Collation::code_page()` still reports the true
code page (for diagnostics); `Collation::encoding()` returns `None`. All
CP125x SQL collations (1250–1257) and the CJK SQL collations are fully
supported via their SortId.

[`encoding_rs`]: https://docs.rs/encoding_rs

**Workaround:** Use a `CP125x` or `_UTF8` collation, or `CAST` the column to
`NVARCHAR` in the query so the value arrives as UTF-16.

---

### NUMERIC/DECIMAL Precision Beyond 28-29 Significant Digits

**Status:** Reads error with a descriptive message

SQL Server `NUMERIC`/`DECIMAL` supports up to 38 significant digits;
[`rust_decimal`] (the `decimal` feature's backing type) holds a 96-bit
mantissa with scale ≤ 28. Reading a value that exceeds that range returns a
`TypeError` naming the limitation. It is deliberately **not** a silent
fallback to `f64` (~15-16 significant digits), which would corrupt values
read, written back, or compared downstream.

[`rust_decimal`]: https://docs.rs/rust_decimal

**Workaround:** `CAST` the column in the query — to a `NUMERIC` precision
within range, to `FLOAT` if approximate semantics are acceptable, or to
`VARCHAR` to receive the exact digits as text.

---

### Always Encrypted Parameter Encryption (Write Path)

**Status:** Read path fully supported; parameter (write) encryption supported
for the common scalar types (see below)

Transparent decryption of encrypted columns works end-to-end: login-time
feature negotiation, `ColMetaData` / `CekTable` / `CryptoMetadata` parsing,
async CEK resolution through key store providers, and AEAD_AES_256_CBC_HMAC_SHA256
decryption in the row hot path.

Parameter (write) encryption is implemented for `int`, `tinyint`, `smallint`,
`bigint`, `bit`, `real`, `float`, `nvarchar`, `varbinary`, `uniqueidentifier`,
`date`, `money`, `smallmoney`, `decimal` (via `numeric(value, precision, scale)`),
the temporal types `time`/`datetime2`/`datetimeoffset` (via `time(v, scale)` /
`datetime2(v, scale)` / `datetimeoffset(v, scale)`), legacy `datetime` (via
`datetime(v)`), `smalldatetime` (via the `SmallDateTime` wrapper), and typed
`NULL` (via `null::<T>()`). With `Column Encryption Setting=Enabled`, a
parameterized query or `execute` automatically describes its parameters
(`sp_describe_parameter_encryption`), encrypts those bound to encrypted columns
client-side, and sends them as encrypted RPC parameters. Both deterministic and
randomized encryption are supported.

Bind a `decimal`/temporal parameter with its typed wrapper, not a bare value: an
encrypted column requires the declared type — precision/scale included — to match
exactly, so a plain `Decimal` (no precision) or `NaiveDateTime`/`NaiveTime` (no
scale, and ambiguous between `datetime` and `datetime2`) is rejected with
`Operand type clash` (Msg 206). `numeric` also rejects, client-side, a value whose
digits exceed the declared precision (the server cannot range-check an encrypted
value). The scale-7 temporal forms are validated byte-for-byte against
`Microsoft.Data.SqlClient`; lower scales are validated by live round-trip, because
Microsoft's client defaults temporal parameters to scale 7 and cannot emit
lower-scale forms for a byte-exact comparison.

Not yet implemented:
- Parameter encryption for the fixed-width `char`, `nchar`, and `binary` types:
  these require declaring an exact width (and, for `char`, a code page). Tracked
  in [#234](https://github.com/praxiomlabs/rust-mssql-driver/issues/234).
- Secure enclave operations.
- Caching of `sp_describe_parameter_encryption`: each parameterized statement
  currently incurs one extra describe round-trip when Always Encrypted is
  enabled (matching the uncached behaviour of other clients).

See the [`mssql-client` `encryption` module docs](https://docs.rs/mssql-client/latest/mssql_client/encryption/) for
the full rationale.

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

**Security Warning:** `no_tls` transmits credentials in plaintext.

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

### TEXT / NTEXT / IMAGE (deprecated since SQL Server 2005)

Not supported in bulk insert. Microsoft deprecated `TEXT` / `NTEXT` / `IMAGE`
in SQL Server 2005 and recommends `VARCHAR(MAX)` / `NVARCHAR(MAX)` /
`VARBINARY(MAX)` for all new development. Attempting to construct a
`BulkColumn` with `"TEXT"`, `"NTEXT"`, or `"IMAGE"`, or running
`Client::bulk_insert` against a table whose server metadata reports `TEXT` /
`NTEXT` / `IMAGE` columns, returns `TypeError::UnsupportedType` with a message
naming the correct replacement. Reading these columns in ordinary queries is
still supported.

To migrate:

```sql
ALTER TABLE MyTable ALTER COLUMN Body VARCHAR(MAX);   -- was TEXT
ALTER TABLE MyTable ALTER COLUMN Body NVARCHAR(MAX);  -- was NTEXT
ALTER TABLE MyTable ALTER COLUMN Blob VARBINARY(MAX); -- was IMAGE
```

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

Not implemented. Bound large reads by paging in SQL rather than holding a
server-side cursor; see "Result Set Buffering" above for why paging (not the
result set type) is what limits client memory.

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
3. **Cross-Platform First** - Core functionality works on all platforms; platform-specific features (SSPI, FILESTREAM, CertStore) are gated behind feature flags
4. **Security** - Features with security implications receive extra scrutiny

---

## Feature Requests

If you need a feature not listed here:

1. Check [GitHub Issues](https://github.com/praxiomlabs/rust-mssql-driver/issues)
2. Open an issue with your use case
3. Consider whether a workaround exists

---

*Last updated: June 2026.*
