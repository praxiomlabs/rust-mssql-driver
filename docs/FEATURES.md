# Feature Flags

This document describes all available feature flags in the rust-mssql-driver crates.

## Overview

Feature flags allow you to customize the driver's functionality based on your needs. Enabling only the features you need reduces compile time and binary size.

## mssql-client Features

The main client crate (`mssql-client`) provides these features:

| Feature | Default | Description |
|---------|---------|-------------|
| `chrono` | Yes | Date/time type support via the chrono crate |
| `uuid` | Yes | UUID type support |
| `decimal` | Yes | Decimal type support via rust_decimal |
| `json` | Yes | JSON type support via serde_json |
| `otel` | No | OpenTelemetry instrumentation for tracing |
| `zeroize` | No | Secure credential wiping from memory |
| `encoding` | No | Collation-aware VARCHAR decoding via encoding_rs |
| `filestream` | No | FILESTREAM BLOB access (Windows only) |

### chrono

**Default: Enabled**

Provides support for SQL Server date/time types using the `chrono` crate:

```rust
use chrono::{NaiveDate, NaiveDateTime, DateTime, Utc};

// Read date/time values
let date: NaiveDate = row.get("birth_date")?;
let datetime: NaiveDateTime = row.get("created_at")?;
let datetime_tz: DateTime<Utc> = row.get("updated_at")?;

// Use as parameters
client.query(
    "SELECT * FROM events WHERE event_date = @p1",
    &[&NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()]
).await?;
```

**Type Mappings:**

| SQL Server Type | Rust Type |
|-----------------|-----------|
| `DATE` | `chrono::NaiveDate` |
| `TIME` | `chrono::NaiveTime` |
| `DATETIME`, `DATETIME2`, `SMALLDATETIME` | `chrono::NaiveDateTime` |
| `DATETIMEOFFSET` | `chrono::DateTime<Utc>` |

**Disable if:** You don't use date/time types or prefer manual parsing.

```toml
mssql-client = { version = "0.5", default-features = false, features = ["uuid", "decimal", "json"] }
```

### uuid

**Default: Enabled**

Provides support for SQL Server `UNIQUEIDENTIFIER` type:

```rust
use uuid::Uuid;

// Read UUID values
let id: Uuid = row.get("user_id")?;

// Use as parameters
let new_id = Uuid::new_v4();
client.execute(
    "INSERT INTO users (id, name) VALUES (@p1, @p2)",
    &[&new_id, &"John"]
).await?;
```

**Disable if:** You don't use UUIDs or prefer to handle them as strings/bytes.

### decimal

**Default: Enabled**

Provides support for SQL Server `DECIMAL` and `NUMERIC` types:

```rust
use rust_decimal::Decimal;

// Read decimal values
let price: Decimal = row.get("price")?;
let balance: Decimal = row.get("account_balance")?;

// Use as parameters
let amount = Decimal::new(1999, 2);  // 19.99
client.execute(
    "UPDATE accounts SET balance = balance - @p1 WHERE id = @p2",
    &[&amount, &account_id]
).await?;
```

**Disable if:** You only use floating-point or integer types for numeric data.

### json

**Default: Enabled**

Provides support for JSON data via serde_json:

```rust
use serde_json::Value;

// Read JSON values
let config: Value = row.get("settings")?;

// Or deserialize to a typed struct
#[derive(Deserialize)]
struct UserSettings {
    theme: String,
    notifications: bool,
}
let settings: UserSettings = serde_json::from_value(row.get("settings")?)?;

// Use as parameters
let data = serde_json::json!({"key": "value"});
client.execute(
    "UPDATE users SET metadata = @p1 WHERE id = @p2",
    &[&data.to_string(), &user_id]
).await?;
```

**Disable if:** You don't store JSON data in SQL Server.

### otel

**Default: Disabled**

Enables OpenTelemetry instrumentation for distributed tracing:

```toml
mssql-client = { version = "0.5", features = ["otel"] }
```

When enabled, the driver automatically creates spans for:
- Connection establishment
- Query execution
- Transaction operations
- Pool operations

See [OPENTELEMETRY.md](OPENTELEMETRY.md) for detailed setup instructions.

**Enable if:** You use OpenTelemetry for observability in production.

### zeroize

**Default: Disabled**

Enables secure credential wiping using the `zeroize` crate:

```toml
mssql-client = { version = "0.5", features = ["zeroize"] }
```

When enabled:
- Passwords are securely wiped from memory when no longer needed
- Connection strings containing credentials are zeroed after parsing
- Reduces risk of credential leakage through memory dumps

**Enable if:** You have strict security requirements or handle highly sensitive data.

### filestream

**Default: Disabled**

Enables async read/write access to SQL Server FILESTREAM BLOBs (Windows only):

```toml
mssql-client = { version = "0.8", features = ["sspi-auth", "filestream"] }
```

When enabled:
- `FileStream` type implementing `AsyncRead + AsyncWrite` for FILESTREAM BLOBs
- `Client<InTransaction>::open_filestream()` convenience method
- Runtime dynamic loading of `OpenSqlFilestream` from the OLE DB Driver DLL
- Clear error messages if the OLE DB Driver is not installed

**Requirements:**
- Windows client machine
- Microsoft OLE DB Driver for SQL Server (`msoledbsql.dll`) installed at runtime
- SQL Server with FILESTREAM enabled (access level 2)
- Windows Authentication (FILESTREAM does not work with SQL auth)

**Enable if:** You need to read or write large binary data stored in SQL Server FILESTREAM columns.

See [`docs/FILESTREAM.md`](FILESTREAM.md) for setup instructions and usage examples.

## mssql-auth Features

The authentication crate (`mssql-auth`) provides these features:

| Feature | Default | Description |
|---------|---------|-------------|
| `azure-identity` | No | Azure Managed Identity and Service Principal authentication |
| `integrated-auth` | No | Kerberos/GSSAPI authentication (Linux/macOS) |
| `sspi-auth` | No | Windows SSPI authentication (cross-platform via sspi-rs) |
| `cert-auth` | No | Client certificate authentication (Azure AD with X.509) |
| `zeroize` | No | Secure credential zeroization on drop |
| `always-encrypted` | No | Always Encrypted transparent column decryption with key providers |

### azure-identity

**Default: Disabled**

Enables Azure authentication methods:

```toml
mssql-auth = { version = "0.5", features = ["azure-identity"] }
```

Provides:
- **Managed Identity** - For Azure VMs, Container Instances, App Services
- **Service Principal** - For application authentication with client ID + secret

### integrated-auth

**Default: Disabled**

Enables Kerberos/GSSAPI authentication on Linux and macOS:

```toml
mssql-auth = { version = "0.5", features = ["integrated-auth"] }
```

**Prerequisites:**
- Linux: `libkrb5-dev` (Debian/Ubuntu) or `krb5-devel` (RHEL/Fedora)
- macOS: Kerberos included with macOS

### sspi-auth

**Default: Disabled**

Enables Windows SSPI authentication via the cross-platform sspi-rs crate:

```toml
mssql-auth = { version = "0.5", features = ["sspi-auth"] }
```

Works on Windows natively and on other platforms when appropriate credentials are available.

### cert-auth

**Default: Disabled**

Enables client certificate authentication for Azure AD Service Principal:

```toml
mssql-auth = { version = "0.5", features = ["cert-auth"] }
```

Requires X.509 certificate and private key for authentication.

### always-encrypted

**Default: Disabled**

Enables Always Encrypted transparent column decryption:

```toml
mssql-auth = { version = "0.8", features = ["always-encrypted"] }
```

When `Column Encryption Setting=Enabled` is in the connection string, encrypted
columns are transparently decrypted in all query paths (`query()`, `call_procedure()`,
`query_multiple()`). See [`docs/ALWAYS_ENCRYPTED.md`](ALWAYS_ENCRYPTED.md) for the full guide.

Provides:
- Transparent column decryption in query results
- AEAD_AES_256_CBC_HMAC_SHA256 encryption/decryption
- RSA-OAEP key unwrapping for CEK decryption
- CEK caching with TTL expiration
- `InMemoryKeyStore` for development/testing
- `KeyStoreProvider` trait for custom implementations

**Production key providers:**
- `AzureKeyVaultProvider` - Azure Key Vault integration (`azure-identity` feature)
- `WindowsCertStoreProvider` - Windows Certificate Store (`sspi-auth` feature, Windows only)

## tds-protocol Features

The low-level protocol crate has these features:

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | Yes | Standard library support |
| `alloc` | No | Alloc-only support (no_std with allocator) |

### std

**Default: Enabled**

Provides full standard library support including I/O and error handling.

### alloc

**Default: Disabled**

Enables compilation without the standard library, using only the `alloc` crate. Useful for embedded or constrained environments.

```toml
[dependencies]
tds-protocol = { version = "0.5", default-features = false, features = ["alloc"] }
```

## Common Configurations

### Minimal (Smallest Binary)

```toml
[dependencies]
mssql-client = { version = "0.5", default-features = false }
```

Only basic types supported. No date/time, UUID, decimal, or JSON.

### Web Application (Typical)

```toml
[dependencies]
mssql-client = { version = "0.5" }
```

All default features enabled for comprehensive type support.

### Production with Observability

```toml
[dependencies]
mssql-client = { version = "0.5", features = ["otel"] }
```

Default features plus OpenTelemetry tracing.

### High-Security Environment

```toml
[dependencies]
mssql-client = { version = "0.5", features = ["zeroize", "otel"] }
```

All defaults plus secure credential handling and audit tracing.

### Numeric-Only Application

```toml
[dependencies]
mssql-client = { version = "0.5", default-features = false, features = ["decimal"] }
```

Only decimal support, no date/time or JSON.

## Feature Detection

You can detect enabled features at runtime:

```rust
#[cfg(feature = "chrono")]
fn handle_datetime(row: &Row) -> chrono::NaiveDateTime {
    row.get("created_at").unwrap()
}

#[cfg(not(feature = "chrono"))]
fn handle_datetime(row: &Row) -> String {
    row.get("created_at").unwrap()
}
```

## Dependency Impact

Each feature adds dependencies:

| Feature | Additional Dependencies |
|---------|------------------------|
| `chrono` | chrono |
| `uuid` | uuid |
| `decimal` | rust_decimal |
| `json` | serde_json, serde |
| `otel` | opentelemetry, opentelemetry_sdk, tracing-opentelemetry |
| `zeroize` | zeroize |

## Compile Time Impact

Approximate compile time impact (relative to minimal build):

| Configuration | Compile Time | Binary Size |
|---------------|--------------|-------------|
| Minimal | 1.0x | 1.0x |
| Default | 1.3x | 1.2x |
| All features | 1.5x | 1.4x |

*Actual times vary based on system and crate versions.*
