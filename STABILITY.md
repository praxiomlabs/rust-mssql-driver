# API Stability

This document describes the stability guarantees for the rust-mssql-driver project.

## Versioning Policy

This project follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html):

- **MAJOR** version for incompatible API changes
- **MINOR** version for backwards-compatible functionality additions
- **PATCH** version for backwards-compatible bug fixes

### Pre-1.0 Releases (0.x.y)

During the 0.x release series:

- **MINOR** version bumps (0.x.0) may include breaking changes
- **PATCH** version bumps (0.0.x) are backwards-compatible
- Breaking changes will be documented in the [CHANGELOG](CHANGELOG.md)

### Post-1.0 Releases

After 1.0.0 is released:

- Breaking changes require a major version bump
- Deprecated APIs will remain functional for at least one minor release
- Security fixes may be backported to supported versions

## Stable API Surface

The following APIs are considered stable and covered by semver guarantees:

### mssql-client

| API | Stability |
|-----|-----------|
| `Client::connect()` | Stable |
| `Client::query()` | Stable |
| `Client::execute()` | Stable |
| `Client::close()` | Stable |
| `Client::begin_transaction()` | Stable |
| `Config::from_connection_string()` | Stable |
| `Config::builder()` | Stable |
| `Row::get()` | Stable |
| `Row::try_get()` | Stable |
| `Transaction::commit()` | Stable |
| `Transaction::rollback()` | Stable |
| `Transaction::save_point()` | Stable |
| `Client::call_procedure()` | Stable |
| `Client::procedure()` | Stable |
| `ProcedureBuilder::input()` | Stable |
| `ProcedureBuilder::output_*()` | Stable |
| `ProcedureBuilder::execute()` | Stable |
| `ProcedureResult` fields | Stable |
| `Error` enum variants | Stable |
| `Client::open_filestream()` | Stable (`filestream` feature, Windows only) |
| `FileStream` / `FileStreamAccess` | Stable (`filestream` feature, Windows only) |
| `EncryptionContext` / `EncryptionConfig` | Stable (`always-encrypted` feature) |
| `KeyStoreProvider` trait | Stable (`always-encrypted` feature) |

### mssql-driver-pool

| API | Stability |
|-----|-----------|
| `Pool::builder()` | Stable |
| `Pool::get()` | Stable |
| `PoolConfig` fields | Stable |

### mssql-derive

| API | Stability |
|-----|-----------|
| `#[derive(FromRow)]` | Stable |
| `#[derive(ToParams)]` | Stable |
| `#[mssql(rename = "...")]` | Stable |
| `#[mssql(default)]` | Stable |
| `#[mssql(skip)]` | Stable |

### mssql-types

| API | Stability |
|-----|-----------|
| `FromSql` trait | Stable |
| `ToSql` trait | Stable |
| Built-in type conversions | Stable |

## Unstable API Surface

The following APIs are considered unstable and may change without a major version bump:

### Internal Modules

- Any module or type marked `#[doc(hidden)]`
- Types in `*::internal::*` modules
- Types with names ending in `Internal` or `Raw`

### Protocol Layer

- `tds-protocol` crate internals
- `mssql-codec` frame structures
- `mssql-tls` negotiation details

### Feature-Gated APIs with Platform Dependencies

APIs behind these feature flags depend on platform-specific libraries and may have different behavior across platforms:

- `azure-identity` - Azure authentication (requires Azure SDK)
- `integrated-auth` - Kerberos/GSSAPI auth (requires libgssapi on Unix)
- `sspi-auth` - Windows SSPI auth (requires sspi-rs)

### Unstable Functions

| API | Reason |
|-----|--------|
| `Client::with_raw_connection()` | Low-level access, may change |

## Deprecation Policy

When an API is deprecated:

1. The API will be marked with `#[deprecated]` with a message explaining:
   - Why it's deprecated
   - What to use instead
   - When it will be removed

2. Deprecated APIs will continue to work for at least:
   - **Pre-1.0**: One minor release
   - **Post-1.0**: One minor release (may be longer for widely-used APIs)

3. Removal will be announced in the CHANGELOG

### Example Deprecation

```rust
#[deprecated(
    since = "0.2.0",
    note = "Use `Config::builder()` instead. Will be removed in 0.4.0."
)]
pub fn Config::new() -> Config { ... }
```

## Minimum Supported Rust Version (MSRV)

- **Current MSRV**: 1.88 (Rust 2024 Edition)
- **MSRV Policy**: We support the latest stable Rust and may increase MSRV in minor releases

### MSRV Increase Policy

- MSRV increases are not considered breaking changes
- MSRV will only be increased when necessary for:
  - Security fixes
  - Critical bug fixes
  - Features requiring new language/stdlib capabilities
- MSRV increases will be documented in the CHANGELOG

## Platform Support

### Tier 1 (Fully Supported)

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

### Tier 2 (Best Effort)

- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`

### Not Supported

- 32-bit platforms
- Platforms without TLS support

## Testing and CI

Stability is enforced through:

1. **Comprehensive test suite** - Unit, integration, and property-based tests
2. **CI on all Tier 1 platforms** - Every PR tested
3. **cargo-deny** - Dependency auditing
4. **cargo-semver-checks** - Automated semver verification in CI

## Reporting Stability Issues

If you encounter an unexpected breaking change:

1. Check the [CHANGELOG](CHANGELOG.md) for documented changes
2. Open an issue describing:
   - The API that broke
   - Your previous working code
   - The error or behavior change
   - Versions involved

We take backwards compatibility seriously and will work to resolve issues promptly.

## Feature Flags and Stability

| Feature | Default | Stability |
|---------|---------|-----------|
| `chrono` | Yes | Stable |
| `uuid` | Yes | Stable |
| `decimal` | Yes | Stable |
| `json` | Yes | Stable |
| `otel` | No | Stable |
| `zeroize` | No | Stable |
| `azure-identity` | No | Stable (platform-dependent) |
| `integrated-auth` | No | Stable (platform-dependent) |
| `sspi-auth` | No | Stable (platform-dependent) |
| `cert-auth` | No | Stable |
| `always-encrypted` | No | Stable |
| `filestream` | No | Stable (Windows only) |
| `encoding` | No | Stable |

## SQL Server Compatibility

| SQL Server Version | Support Level |
|-------------------|---------------|
| 2022+ | Full (TDS 8.0) |
| 2019 | Full (TDS 7.4) |
| 2017 | Full (TDS 7.4) |
| 2016 | Full (TDS 7.4) |
| 2014 and earlier | Not supported |
| Azure SQL Database | Full |
| Azure SQL Managed Instance | Full |

SQL Server compatibility is considered part of our stability guarantee. Dropping support for a SQL Server version requires a major version bump (post-1.0).
