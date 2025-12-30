# rust-mssql-driver

A high-performance, async Microsoft SQL Server driver for Rust.

[![Crates.io](https://img.shields.io/crates/v/mssql-client.svg)](https://crates.io/crates/mssql-client)
[![Documentation](https://docs.rs/mssql-client/badge.svg)](https://docs.rs/mssql-client)
[![License](https://img.shields.io/crates/l/mssql-client.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)

## Features

- **TDS 7.3 – 8.0 Support** - SQL Server 2008+ legacy support through SQL Server 2022+ strict encryption
- **Tokio-Native** - Designed for the Tokio async runtime with no compatibility layers
- **Type-State Connections** - Compile-time enforcement of valid connection states
- **Built-in Connection Pooling** - No external pooling crate required
- **Reduced-Copy Architecture** - `Arc<Bytes>` pattern minimizes allocation overhead
- **Pure Rust TLS** - Uses rustls, no OpenSSL dependency
- **Modern Rust** - 2024 Edition, MSRV 1.85

### Feature Status (v0.3.x)

| Feature | Status | Notes |
|---------|--------|-------|
| SQL Authentication | ✅ | Username/password |
| Azure AD Token | ✅ | Pre-acquired tokens |
| Queries & Parameters | ✅ | Full support |
| Transactions | ✅ | Commit, rollback, savepoints |
| Connection Pooling | ✅ | Built-in via `mssql-driver-pool` |
| Bulk Insert | ✅ | High-performance batch loading |
| `#[derive(FromRow)]` | ✅ | Row-to-struct mapping |
| TDS 7.3 (Legacy) | ✅ | SQL Server 2008/2008 R2 |
| TDS 8.0 Strict Mode | ✅ | SQL Server 2022+ |
| Azure Managed Identity | ✅ | Via `azure-identity` |
| Kerberos/GSSAPI | ✅ | Unix via `libgssapi` |
| Windows SSPI | ✅ | Via `sspi-auth` feature |
| Table-Valued Parameters | ✅ | Via `Tvp` type |
| OpenTelemetry Metrics | ✅ | Via `otel` feature |
| Always Encrypted | ⏳ | Cryptography implemented, key providers planned |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mssql-client = "0.3"
tokio = { version = "1.48", features = ["full"] }
```

## Quick Start

```rust
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), mssql_client::Error> {
    // Connect using a connection string
    let config = Config::from_connection_string(
        "Server=localhost;Database=mydb;User Id=sa;Password=Password123!;TrustServerCertificate=true"
    )?;

    let mut client = Client::connect(config).await?;

    // Execute a query
    let rows = client.query("SELECT id, name FROM users WHERE active = @p1", &[&true]).await?;

    for result in rows {
        let row = result?;
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        println!("{}: {}", id, name);
    }

    client.close().await?;
    Ok(())
}
```

## Connection String Format

The driver supports ADO.NET-compatible connection strings:

```
Server=hostname,port;Database=dbname;User Id=user;Password=pass;Encrypt=strict;
```

### Supported Keywords

| Keyword | Aliases | Description |
|---------|---------|-------------|
| `Server` | `Data Source`, `Address` | Host and optional port (e.g., `localhost,1433`) |
| `Database` | `Initial Catalog` | Database name |
| `User Id` | `UID`, `User` | SQL authentication username |
| `Password` | `PWD` | SQL authentication password |
| `Encrypt` | | `true`, `false`, `strict` (TDS 8.0) |
| `TrustServerCertificate` | | Skip certificate validation (dev only) |
| `TDSVersion` | `ProtocolVersion` | TDS protocol version: `7.3`, `7.3A`, `7.3B`, `7.4`, `8.0` |
| `Application Name` | | Application identifier |
| `Connect Timeout` | | Connection timeout in seconds |
| `Command Timeout` | | Default command timeout |

## Connection Pooling

Use the built-in connection pool for production applications:

```rust
use mssql_driver_pool::{Pool, PoolConfig};
use mssql_client::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("...")?;

    let pool = Pool::builder()
        .max_size(10)
        .min_size(2)
        .build(config)
        .await?;

    // Get a connection from the pool
    let mut conn = pool.get().await?;

    let rows = conn.query("SELECT 1", &[]).await?;
    // Connection returned to pool when dropped

    Ok(())
}
```

## Transactions

```rust
use mssql_client::{Client, Config, IsolationLevel};

async fn transfer_funds(client: &mut Client) -> Result<(), mssql_client::Error> {
    // Begin transaction with isolation level
    let mut tx = client.begin_transaction()
        .isolation_level(IsolationLevel::Serializable)
        .await?;

    tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = @p1", &[&1i32]).await?;
    tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = @p1", &[&2i32]).await?;

    // Commit (returns the client)
    tx.commit().await?;

    Ok(())
}
```

### Savepoints

```rust
let mut tx = client.begin_transaction().await?;

tx.execute("INSERT INTO orders ...", &[]).await?;

// Create a savepoint
let sp = tx.save_point("before_items").await?;

tx.execute("INSERT INTO order_items ...", &[]).await?;

// Rollback to savepoint if needed
tx.rollback_to(&sp).await?;

tx.commit().await?;
```

## Derive Macros

Map rows to structs automatically:

```rust
use mssql_derive::FromRow;

#[derive(FromRow)]
struct User {
    id: i32,
    #[mssql(rename = "user_name")]
    name: String,
    #[mssql(default)]
    email: Option<String>,
}

let rows = client.query("SELECT id, user_name, email FROM users", &[]).await?;
for result in rows {
    let user: User = result?.try_into()?;
    println!("{}: {}", user.id, user.name);
}
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `otel` | No | OpenTelemetry tracing and metrics |
| `zeroize` | No | Secure credential wiping |
| `chrono` | Yes | Date/time type support via chrono |
| `uuid` | Yes | UUID type support |
| `decimal` | Yes | Decimal type support via rust_decimal |
| `json` | Yes | JSON type support via serde_json |

### Authentication Features (mssql-auth crate)

| Feature | Description |
|---------|-------------|
| `azure-identity` | Azure Managed Identity and Service Principal |
| `integrated-auth` | Kerberos/GSSAPI (Linux/macOS) |
| `sspi-auth` | Windows SSPI (cross-platform via sspi-rs) |
| `cert-auth` | Client certificate authentication |
| `zeroize` | Secure credential wiping from memory |
| `always-encrypted` | Client-side encryption (cryptography implemented) |

Enable optional features:

```toml
[dependencies]
mssql-client = { version = "0.3", features = ["otel"] }
mssql-auth = { version = "0.3", features = ["sspi-auth"] }
```

## SQL Server Compatibility

| SQL Server Version | Supported | TDS Version | Notes |
|-------------------|-----------|-------------|-------|
| 2008 | ✅ | 7.3A | Legacy support |
| 2008 R2 | ✅ | 7.3B | Legacy support |
| 2012 | ✅ | 7.4 | |
| 2014 | ✅ | 7.4 | |
| 2016 | ✅ | 7.4 | |
| 2017 | ✅ | 7.4 | Recommended minimum |
| 2019 | ✅ | 7.4 | |
| 2022+ | ✅ | 8.0 | Strict TLS mode |
| Azure SQL Database | ✅ | 7.4/8.0 | |
| Azure SQL Managed Instance | ✅ | 7.4/8.0 | |

**Legacy Support (SQL Server 2008/2008 R2):** Configure via connection string (`TDSVersion=7.3`) or builder pattern. See [LIMITATIONS.md](LIMITATIONS.md) for details.

## API Stability

This project follows [Semantic Versioning](https://semver.org/).

- **0.x.y**: API may change between minor versions
- **1.0.0+**: Stable API with backward compatibility guarantees

See [STABILITY.md](STABILITY.md) for details on what's considered stable.

## Comparison with Tiberius

| Feature | rust-mssql-driver | tiberius |
|---------|-------------------|----------|
| TDS 7.3 (SQL 2008) | Configurable | Supported |
| TDS 8.0 (strict mode) | First-class | Not supported |
| Connection pooling | Built-in | External (bb8/deadpool) |
| Runtime | Tokio-native | Runtime agnostic |
| Prepared statement cache | Automatic LRU | Per-execution |
| Azure SQL redirects | Automatic | Manual handling |
| Type-state connections | Yes | No |

## Examples

See the [`examples/`](crates/mssql-client/examples/) directory:

- [`basic.rs`](crates/mssql-client/examples/basic.rs) - Connection and queries
- [`transactions.rs`](crates/mssql-client/examples/transactions.rs) - Transaction handling
- [`streaming.rs`](crates/mssql-client/examples/streaming.rs) - Streaming large results
- [`bulk_insert.rs`](crates/mssql-client/examples/bulk_insert.rs) - Bulk data loading
- [`derive_macros.rs`](crates/mssql-client/examples/derive_macros.rs) - Row mapping macros

## Documentation

### API & Reference

- [API Documentation](https://docs.rs/mssql-client) - Full API reference on docs.rs
- [ARCHITECTURE.md](ARCHITECTURE.md) - Design decisions, ADRs, and internals
- [CHANGELOG.md](CHANGELOG.md) - Version history and release notes

### Guides & Policies

- [STABILITY.md](STABILITY.md) - API stability guarantees and versioning policy
- [SECURITY.md](SECURITY.md) - Security policy, threat model, and best practices
- [LIMITATIONS.md](LIMITATIONS.md) - Known limitations and workarounds
- [UNSUPPORTED.md](UNSUPPORTED.md) - Explicitly unsupported features with rationale
- [PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) - Production readiness checklist

### Crate-Specific Documentation

Each crate has its own README with crate-specific documentation:

| Crate | Description |
|-------|-------------|
| [`mssql-client`](crates/mssql-client/README.md) | Main client API |
| [`mssql-driver-pool`](crates/mssql-pool/README.md) | Connection pooling |
| [`mssql-derive`](crates/mssql-derive/README.md) | Derive macros |
| [`mssql-types`](crates/mssql-types/README.md) | Type conversions |
| [`mssql-auth`](crates/mssql-auth/README.md) | Authentication providers |
| [`mssql-tls`](crates/mssql-tls/README.md) | TLS negotiation |
| [`tds-protocol`](crates/tds-protocol/README.md) | TDS protocol layer |
| [`mssql-codec`](crates/mssql-codec/README.md) | Async framing |
| [`mssql-testing`](crates/mssql-testing/README.md) | Test infrastructure |

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

This project builds on learnings from [tiberius](https://github.com/prisma/tiberius) and the [MS-TDS protocol specification](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/).
