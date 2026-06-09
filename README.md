# rust-mssql-driver

An async Microsoft SQL Server driver for Rust.

[![Crates.io](https://img.shields.io/crates/v/mssql-client.svg)](https://crates.io/crates/mssql-client)
[![Documentation](https://docs.rs/mssql-client/badge.svg)](https://docs.rs/mssql-client)
[![License](https://img.shields.io/crates/l/mssql-client.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)

## Features

- **TDS 7.3 – 8.0 Support** - SQL Server 2008+ legacy support through SQL Server 2022+ strict encryption
- **Tokio-Native** - Designed for the Tokio async runtime with no compatibility layers
- **Type-State Connections** - Compile-time enforcement of valid connection states
- **Built-in Connection Pooling** - No external pooling crate required
- **Reduced-Copy Architecture** - `Arc<Bytes>` pattern minimizes allocation overhead
- **Pure Rust TLS** - Uses rustls, no OpenSSL dependency
- **Modern Rust** - 2024 Edition, MSRV 1.88

### Feature Status

| Feature | Status | Notes |
|---------|--------|-------|
| SQL Authentication | Yes | Username/password |
| Azure AD Token | Yes | Pre-acquired tokens |
| Queries & Parameters | Yes | Full support |
| Transactions | Yes | Commit, rollback, savepoints |
| Connection Pooling | Yes | Built-in via `mssql-driver-pool` |
| Bulk Insert | Yes | Batch loading via `Client::bulk_insert()` |
| `#[derive(FromRow)]` / `#[derive(ToParams)]` | Yes | Row-to-struct and struct-to-params mapping |
| TDS 7.3 (Legacy) | Yes | SQL Server 2008/2008 R2 |
| TDS 8.0 Strict Mode | Yes | SQL Server 2022+ |
| Azure Managed Identity | Yes | Via `azure-identity` |
| Kerberos/GSSAPI | Yes | Unix via `libgssapi` |
| Windows SSPI | Yes | Via `sspi-auth` feature |
| Table-Valued Parameters | Yes | Via `Tvp` type |
| OpenTelemetry Metrics | Yes | Query + pool lifecycle via `otel` feature |
| Always Encrypted (read) | Yes | Transparent decryption with Azure Key Vault and Windows CertStore providers |
| Always Encrypted (write) | Partial | `NULL` writes only; ciphertext write path pending — see [docs/ALWAYS_ENCRYPTED.md](docs/ALWAYS_ENCRYPTED.md#limitations) |
| Query Cancellation | Yes | ATTENTION signal + in-flight pool discard |
| Collation-Aware Decoding | Yes | 14+ character encodings |
| Collation-Aware VARCHAR Params | Yes | Via `SendStringParametersAsUnicode=false` |
| Stored Procedures | Yes | RPC-based with OUTPUT params and RETURN value |
| Named Instance Resolution | Yes | SQL Browser service (UDP 1434) |
| MultiSubnetFailover (AG) | Yes | Parallel TCP connect for listener failover |
| Connection Retry | Yes | `ConnectRetryCount` / `ConnectRetryInterval` |
| FILESTREAM BLOB Access | Yes | Windows only, via `filestream` feature |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mssql-client = "0.11"
tokio = { version = "1.48", features = ["full"] }
```

**Windows note:** The default TLS feature requires a C compiler (`ring`/`aws-lc-sys`). Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload, or any edition of Visual Studio with that workload. This is a one-time setup — see [CONTRIBUTING.md](CONTRIBUTING.md#4-platform-specific-windows-c-build-tools) for details.

## Quick Start

```rust,no_run
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

```text
Server=hostname,port;Database=dbname;User Id=user;Password=pass;Encrypt=strict;
```

### Supported Keywords

| Keyword | Aliases | Description |
|---------|---------|-------------|
| `Server` | `Data Source`, `Address` | Host and optional port (e.g., `localhost,1433`) |
| `Database` | `Initial Catalog` | Database name |
| `User Id` | `UID`, `User` | SQL authentication username |
| `Password` | `PWD` | SQL authentication password |
| `Encrypt` | | `true`, `false`, `strict`, `no_tls` |
| `TrustServerCertificate` | | Skip certificate validation (dev only) |
| `TDSVersion` | `ProtocolVersion` | TDS protocol version: `7.3`, `7.3A`, `7.3B`, `7.4`, `8.0` |
| `Application Name` | | Application identifier |
| `Connect Timeout` | | Connection timeout in seconds |
| `Command Timeout` | | Default command timeout |
| `SendStringParametersAsUnicode` | | `true`/`false` — when `false`, `String` params send as VARCHAR under the server collation (for index seeks) |
| `MultiSubnetFailover` | | `true` to race parallel TCP connects across all resolved addresses (Always On AG) |
| `ConnectRetryCount` | | Transient connect failures retried with exponential backoff |
| `Column Encryption Setting` | | `Enabled`/`Disabled` — transparent decryption of encrypted columns |

See [docs/CONNECTION_STRINGS.md](docs/CONNECTION_STRINGS.md) for the full ADO.NET-compatible keyword reference.

## Connection Pooling

Use the built-in connection pool for production applications:

```rust,no_run
use mssql_driver_pool::Pool;
use mssql_client::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("...")?;

    let pool = Pool::builder()
        .client_config(config)
        .max_connections(10)
        .min_connections(2)
        .build()
        .await?;

    // Get a connection from the pool
    let mut conn = pool.get().await?;

    let rows = conn.query("SELECT 1", &[]).await?;
    // Connection returned to pool when dropped

    Ok(())
}
```

## Transactions

```rust,no_run
use mssql_client::{Client, Config, IsolationLevel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=mydb;User Id=sa;Password=Password123!")?;
    let client = Client::connect(config).await?;

    // Begin transaction with a specific isolation level
    let mut tx = client.begin_transaction_with_isolation(IsolationLevel::Serializable).await?;

    tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = @p1", &[&1i32]).await?;
    tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = @p1", &[&2i32]).await?;

    // Commit (returns the client)
    tx.commit().await?;

    Ok(())
}
```

### Savepoints

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=mydb;User Id=sa;Password=Password123!")?;
    let client = Client::connect(config).await?;
    let mut tx = client.begin_transaction().await?;

    tx.execute("INSERT INTO orders DEFAULT VALUES", &[]).await?;

    // Create a savepoint
    let sp = tx.save_point("before_items").await?;

    tx.execute("INSERT INTO order_items DEFAULT VALUES", &[]).await?;

    // Rollback to savepoint if needed
    tx.rollback_to(&sp).await?;

    tx.commit().await?;
    Ok(())
}
```

## Derive Macros

Map rows to structs automatically:

```text
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
| `chrono` | Yes | Date/time type support via chrono |
| `uuid` | Yes | UUID type support |
| `decimal` | Yes | Decimal type support via rust_decimal |
| `encoding` | Yes | Collation-aware VARCHAR decoding |
| `json` | No | JSON type support via serde_json |
| `tls` | Yes | TLS/SSL encryption via rustls (disable for `Encrypt=no_tls` environments) |
| `otel` | No | OpenTelemetry tracing and metrics |
| `zeroize` | No | Secure credential wiping |
| `filestream` | No | FILESTREAM BLOB access (Windows only, requires OLE DB Driver) |

### Authentication Features (mssql-auth crate)

| Feature | Description |
|---------|-------------|
| `azure-identity` | Azure Managed Identity and Service Principal |
| `integrated-auth` | Kerberos/GSSAPI (Linux/macOS) |
| `sspi-auth` | Windows SSPI (cross-platform via sspi-rs) |
| `cert-auth` | Client certificate authentication |
| `zeroize` | Secure credential wiping from memory |
| `always-encrypted` | Transparent column decryption with Azure Key Vault and Windows CertStore key providers |

Enable optional features:

```toml
[dependencies]
mssql-client = { version = "0.11", features = ["otel"] }
mssql-auth = { version = "0.11", features = ["sspi-auth"] }
```

## SQL Server Compatibility

| SQL Server Version | Supported | TDS Version | Notes |
|-------------------|-----------|-------------|-------|
| 2008 | Yes | 7.3A | Legacy support |
| 2008 R2 | Yes | 7.3B | Legacy support |
| 2012 | Yes | 7.4 | |
| 2014 | Yes | 7.4 | |
| 2016 | Yes | 7.4 | |
| 2017 | Yes | 7.4 | Full TLS support |
| 2019 | Yes | 7.4 | |
| 2022+ | Yes | 8.0 | Strict TLS mode |
| Azure SQL Database | Yes | 7.4/8.0 | |
| Azure SQL Managed Instance | Yes | 7.4/8.0 | |

**Legacy Support (SQL Server 2008-2016):** Use `Encrypt=no_tls` for servers that don't support TLS 1.2. See [LIMITATIONS.md](LIMITATIONS.md) and [docs/SQL_SERVER_COMPATIBILITY.md](docs/SQL_SERVER_COMPATIBILITY.md) for details.

## API Stability

This project follows [Semantic Versioning](https://semver.org/).

- **0.x.y**: API may change between minor versions
- **1.0.0+**: Stable API with backward compatibility guarantees

See [STABILITY.md](STABILITY.md) for details on what's considered stable.

## Project Status

**Actively maintained**, currently pre-1.0. The driver is feature-complete for
the scenarios in the [Feature Status](#feature-status) table above, and an
integration suite runs against a real SQL Server instance in CI on every change.

Maintenance is **best-effort by a single maintainer** (see
[MAINTAINERS.md](MAINTAINERS.md)); the aim is a first response to issues and pull
requests within about a week. The road to **1.0** is about settling the public
API surface (see [STABILITY.md](STABILITY.md)) and closing the gaps in
[LIMITATIONS.md](LIMITATIONS.md) — once the API is proven in real deployments,
the 1.0 backward-compatibility guarantee follows.

## Comparison with Tiberius

| Feature | rust-mssql-driver | tiberius |
|---------|-------------------|----------|
| TDS 7.3 (SQL 2008) | Configurable | Supported |
| TDS 8.0 (strict mode) | Yes | Not supported |
| Connection pooling | Built-in | External (bb8/deadpool) |
| Runtime | Tokio-native | Runtime agnostic |
| Prepared statement cache | Automatic LRU | Per-execution |
| Azure SQL redirects | Automatic | Manual handling |
| Type-state connections | Yes | No |
| Stored procedures (RPC) | Dedicated API (OUTPUT params, RETURN value) | Via `EXEC` in SQL only; no dedicated API |
| Named instance resolution | Automatic (SQL Browser) | Yes (`SqlBrowser` trait) |

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
- [LIMITATIONS.md](LIMITATIONS.md) - Known limitations and explicit non-goals
- [docs/COMPARISON.md](docs/COMPARISON.md) - Feature comparison vs Tiberius, odbc-api, and sqlx-oldapi

### Feature & Usage Guides

- [docs/TYPE_STATE.md](docs/TYPE_STATE.md) - Type-state connection pattern
- [docs/CONNECTION_STRINGS.md](docs/CONNECTION_STRINGS.md) - ADO.NET connection string reference
- [docs/STORED_PROCEDURES.md](docs/STORED_PROCEDURES.md) - Stored procedure (RPC) calls
- [docs/DERIVE_MACROS.md](docs/DERIVE_MACROS.md) - `FromRow` / `ToParams` derive macros
- [docs/DDL.md](docs/DDL.md) - Executing DDL (CREATE/ALTER/DROP)
- [docs/LOB.md](docs/LOB.md) - Large object (MAX / XML) handling
- [docs/CANCEL_SAFETY.md](docs/CANCEL_SAFETY.md) - Query cancellation and cancel safety
- [docs/ALWAYS_ENCRYPTED.md](docs/ALWAYS_ENCRYPTED.md) - Always Encrypted (transparent decryption)
- [docs/FILESTREAM.md](docs/FILESTREAM.md) - FILESTREAM BLOB access (Windows)
- [docs/OPENTELEMETRY.md](docs/OPENTELEMETRY.md) - OpenTelemetry instrumentation
- [docs/FEATURES.md](docs/FEATURES.md) - Feature flag reference
- [docs/MEMORY.md](docs/MEMORY.md) - Memory and allocation design (`Arc<Bytes>`)

### Operational Docs

- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) - Production deployment guide
- [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) - Common issues and solutions
- [docs/CONNECTION_RECOVERY.md](docs/CONNECTION_RECOVERY.md) - Connection recovery and resilience
- [docs/ERRORS.md](docs/ERRORS.md) - Error codes and handling
- [docs/RETRY_STRATEGY.md](docs/RETRY_STRATEGY.md) - Retry policies and backoff
- [docs/TIMEOUTS.md](docs/TIMEOUTS.md) - Timeout configuration
- [docs/POOL_METRICS.md](docs/POOL_METRICS.md) - Pool metrics and monitoring
- [docs/MIGRATION_FROM_TIBERIUS.md](docs/MIGRATION_FROM_TIBERIUS.md) - Migration guide
- [docs/TLS.md](docs/TLS.md) - TLS configuration
- [docs/OPERATIONS.md](docs/OPERATIONS.md) - Operations and graceful shutdown
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md) - Performance benchmarks and targets

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

Contributions are welcome! A few quick pointers:

- **First time?** Read [CONTRIBUTING.md § First Contribution](CONTRIBUTING.md#first-contribution-quick-path) for the shortest path from clone to green CI.
- **Filing an issue?** Use the [issue templates](https://github.com/praxiomlabs/rust-mssql-driver/issues/new/choose) — they'll ask the right questions so reviewers can help faster.
- **Opening a PR?** The [PR template](.github/pull_request_template.md) walks you through what reviewers need to know.
- **Architecture changes?** Review [ARCHITECTURE.md](ARCHITECTURE.md) and the [ADR process](CONTRIBUTING.md#architecture-decision-records-adrs).
- **Code of Conduct**: We follow the [Rust Code of Conduct](CODE_OF_CONDUCT.md).
- **Current maintainers** and how to contact them: [MAINTAINERS.md](MAINTAINERS.md).

## Community

- **Questions and discussions**: [GitHub Discussions](https://github.com/praxiomlabs/rust-mssql-driver/discussions)
- **Bugs and feature requests**: [GitHub Issues](https://github.com/praxiomlabs/rust-mssql-driver/issues)
- **Security vulnerabilities**: [Private Security Advisory](https://github.com/praxiomlabs/rust-mssql-driver/security/advisories/new) — see [SECURITY.md](SECURITY.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Development

This project is built with heavy AI assistance, with a human maintainer
reviewing and accountable for every change. The public API is documented on
[docs.rs](https://docs.rs/mssql-client), the protocol layer has unit and
property tests, and an integration suite runs against a real SQL Server in CI
on every change. Known gaps are documented in [LIMITATIONS.md](LIMITATIONS.md);
if something doesn't hold up, please
[open an issue](https://github.com/praxiomlabs/rust-mssql-driver/issues).

## Acknowledgments

This project builds on learnings from [tiberius](https://github.com/prisma/tiberius) and the [MS-TDS protocol specification](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/).
