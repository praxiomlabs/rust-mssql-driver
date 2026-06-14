# rust-mssql-driver

An async Microsoft SQL Server driver for Rust.

[![Crates.io](https://img.shields.io/crates/v/mssql-client.svg)](https://crates.io/crates/mssql-client)
[![Documentation](https://docs.rs/mssql-client/badge.svg)](https://docs.rs/mssql-client)
[![CI](https://github.com/praxiomlabs/rust-mssql-driver/actions/workflows/ci.yml/badge.svg)](https://github.com/praxiomlabs/rust-mssql-driver/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/mssql-client.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)

- **TDS 7.3 – 8.0** — SQL Server 2008 through 2022+ strict encryption, plus Azure SQL
- **Tokio-native** — async from the ground up, no compatibility layers
- **Built-in connection pooling** — no external pooling crate required
- **Type-state connections** — invalid operations are compile errors
- **Pure-Rust TLS** — rustls; no OpenSSL, no system dependencies
- **Beyond queries** — transactions and savepoints, stored procedures with OUTPUT
  params, table-valued parameters, bulk insert, Always Encrypted (read + write),
  OpenTelemetry

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mssql-client = "0.16"
tokio = { version = "1.48", features = ["full"] }
```

**Windows note:** The default TLS feature requires a C compiler (`ring`/`aws-lc-sys`). Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload — a one-time setup; see [CONTRIBUTING.md](CONTRIBUTING.md#4-platform-specific-windows-c-build-tools) for details.

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

## Connection Strings

ADO.NET-style connection strings are supported, including the quoting rules
and the keywords you'd expect (`Server`, `Database`, `User Id`, `Password`,
`Encrypt`, `TrustServerCertificate`, timeouts, `Application Name`, and more):

```text
Server=hostname,port;Database=dbname;User Id=user;Password=pass;Encrypt=strict;
```

The full keyword reference lives in the
[`mssql-client` config docs](https://docs.rs/mssql-client/latest/mssql_client/config/).

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

    let mut tx = client.begin_transaction_with_isolation(IsolationLevel::Serializable).await?;

    tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = @p1", &[&1i32]).await?;

    // Savepoints let you roll back part of a transaction
    let sp = tx.save_point("transfer").await?;
    tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = @p1", &[&2i32]).await?;
    tx.rollback_to(&sp).await?;

    // Commit (returns the client)
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
| `azure-identity` | No | Azure AD / Entra logins with Managed Identity or Service Principal credentials (pre-acquired tokens work without it) |
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

```bash
cargo add mssql-client --features otel
cargo add mssql-auth --features sspi-auth
```

## SQL Server Compatibility

| Version | Notes |
|---------|-------|
| SQL Server 2008 – 2016 | TDS 7.3/7.4. These servers often lack TLS 1.2 (rustls requires it); use `Encrypt=no_tls` on a trusted network |
| SQL Server 2017 – 2019 | TDS 7.4, full TLS |
| SQL Server 2022+ | TDS 7.4 or 8.0 strict mode |
| Azure SQL Database / Managed Instance | Including automatic gateway redirects |

**How each version is validated:** SQL Server **2017, 2019, and 2022 are
CI-verified** — the integration suite runs the full ignored test suite against
all three on every change. SQL Server 2008–2016 and Azure SQL are **validated
manually** against real servers, not in CI.

Known quirks: SQL Server 2014 RTM reports `ProductMajorVersion` as NULL (the
driver falls back to parsing `ProductVersion`), and legacy servers negotiating
TLS 1.0/1.1 fail with "TLS handshake eof" under `Encrypt=true` — use
`Encrypt=no_tls` there. See [LIMITATIONS.md](LIMITATIONS.md) for the rest.

## Status

Pre-1.0 and actively maintained — by a single maintainer on a best-effort
basis (see [MAINTAINERS.md](MAINTAINERS.md)), with the aim of a first response
to issues and pull requests within about a week. The API may still change
between 0.x minors; [STABILITY.md](STABILITY.md) describes what is already
considered settled and the road to 1.0. An integration suite runs against a
real SQL Server in CI on every change.

Known gaps, so you don't have to discover them yourself:

- Kerberos/GSSAPI and FILESTREAM are implemented but not yet validated against
  live infrastructure.
- Always Encrypted reads are fully transparent; writes cover the common scalar
  types (some temporal and fixed-width types are pending — see LIMITATIONS.md).
- Rows decode lazily, but the response is buffered in memory — true streaming
  from the socket is planned.
- Parameterized queries run via `sp_executesql` (the server still reuses
  plans); the client-side prepared-statement cache is planned.
- No MARS (multiple active result sets on one connection).

The full list, with workarounds where they exist, is in
[LIMITATIONS.md](LIMITATIONS.md).

## Scope

This driver speaks TDS natively in pure Rust — no ODBC driver manager, no
OpenSSL, no C toolchain (outside the optional Windows SSPI feature) — and is
Tokio-only by design. It does not do compile-time query checking; if you want
sqlx-style checked queries, this is not that. Coming from Tiberius?
[MIGRATION.md](MIGRATION.md) maps the API differences.

## Examples

See the [`examples/`](crates/mssql-client/examples/) directory:

- [`basic.rs`](crates/mssql-client/examples/basic.rs) - Connection and queries
- [`transactions.rs`](crates/mssql-client/examples/transactions.rs) - Transaction handling
- [`streaming.rs`](crates/mssql-client/examples/streaming.rs) - Iterating large result sets (lazy row decoding)
- [`bulk_insert.rs`](crates/mssql-client/examples/bulk_insert.rs) - Bulk data loading
- [`derive_macros.rs`](crates/mssql-client/examples/derive_macros.rs) - Row mapping macros

## Documentation

- [API Documentation](https://docs.rs/mssql-client) - Full API reference on docs.rs
- [ARCHITECTURE.md](ARCHITECTURE.md) - Design decisions, ADRs, and internals
- [CHANGELOG.md](CHANGELOG.md) - Version history and release notes
- [STABILITY.md](STABILITY.md) - API stability guarantees and versioning policy
- [SECURITY.md](SECURITY.md) - Security policy, threat model, and best practices
- [LIMITATIONS.md](LIMITATIONS.md) - Known limitations and explicit non-goals
- [MIGRATION.md](MIGRATION.md) - Migrating from Tiberius

Feature and usage guides — connection strings, stored procedures, DDL, LOBs,
cancellation, Always Encrypted, FILESTREAM, OpenTelemetry, error handling, pool
metrics, and TLS — live in the crate rustdoc on
[docs.rs](https://docs.rs/mssql-client); see the relevant module on each crate's page.

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
