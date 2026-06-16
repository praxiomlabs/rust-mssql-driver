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

### Road to 1.0

We are deliberately staying in the `0.x` series until the criteria below hold.
Pre-1.0, a breaking change is a minor bump (see above) — this is the correct,
expressive way to evolve an API that is still settling, and a premature 1.0 we
then break would be worse than honest `0.x`. 1.0 is the *end* of a deliberate
API-stabilisation process, not a milestone to rush.

Criteria for 1.0:

1. **All public dependencies are themselves ≥1.0.** Per the
   [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/necessities.html)
   (C-STABLE), a crate cannot be stable while a type from a pre-1.0 dependency
   appears in its public API. This is currently the hard blocker (tracked under
   the `1.0-blocker` label).
2. **The public API surface is frozen and future-proofed** — error enums
   `#[non_exhaustive]` (done), public structs with private fields or
   `#[non_exhaustive]`, and `#[must_use]` on builders and futures. The SQL↔Rust
   type- and row-mapping traits stay *open* (see the sealing posture below) and
   evolve via additive default methods, not by sealing.
3. **A published support pledge** — once 1.0 ships we commit to maintaining the
   1.0 line and a minimum interval before any 2.0, paired with the existing
   rolling MSRV policy. We will not declare 1.0 until we can make and honour
   such a pledge.

Progress toward these is tracked in the **Road to 1.0** milestone.

### Security Support

The latest released minor version receives security fixes. Pre-1.0, older minor
versions are end-of-life and do not receive backports — upgrade to the latest
minor to stay covered.

## Stable API Surface

The following APIs are considered stable and covered by semver guarantees:

### mssql-client

| API | Stability |
|-----|-----------|
| `Client::connect()` | Stable |
| `Client::query()` | Stable |
| `Client::query_stream()` | Stable |
| `RowStream` (`try_next` / `collect_all` / `cancel` / `columns`) | Stable |
| `Client::query_stream_blob()` | Stable |
| `BlobStream` (`next` / `read_chunk` / `copy_blob_to` / `columns`) | Stable |
| `Client::execute()` | Stable |
| `Client::close()` | Stable |
| `Client::begin_transaction()` | Stable |
| `Config::from_connection_string()` | Stable |
| `Config::new()` + `with_*` builders | Stable |
| `Row::get()` | Stable |
| `Row::try_get()` | Stable |
| `Client<InTransaction>::commit()` | Stable |
| `Client<InTransaction>::rollback()` | Stable |
| `Client::save_point()` | Stable |
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
| `ApplicationIntent` enum | Stable |
| `Config::application_intent()` | Stable |
| `Config::workstation_id()` | Stable |
| `Config::language()` | Stable |

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

### Trait extensibility (sealing posture)

The type-mapping and row-mapping traits — `ToSql`, `FromSql`, `SqlTyped`,
`ToParams`, `Tvp`, `FromRow`, `KeyStoreProvider`, `AsyncAuthProvider`,
`ConnectionLifecycle` — are intentionally **open** (user-implementable): binding
custom Rust types, mapping rows by hand, and plugging in custom key stores are
supported features, and every comparable driver (`tokio-postgres`, `sqlx`,
`diesel`) leaves the equivalents open. **Do not seal them.** They evolve via
additive default methods, not by closing the trait.

Only the type-state markers (`ConnectionState` and its states) are sealed — that
is a soundness requirement (an unsealed state set would let downstream code forge
a `Client<FakeState>`), not an evolution lever. Extension traits with a universal
blanket impl (e.g. `RowIteratorExt`) are already effectively closed and gain
nothing from a `Sealed` supertrait.

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

None currently. APIs considered unstable will be listed here with the
reason; expect additions as low-level access points are exposed.

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

A hypothetical illustration of the format:

```rust
#[deprecated(
    since = "0.2.0",
    note = "Use `connect_with_config()` instead. Will be removed in 0.4.0."
)]
pub async fn connect_legacy(/* ... */) { /* ... */ }
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
5. **Public-API snapshots** - `scripts/check-public-api.sh` diffs committed
   snapshots of every crate's public surface, catching changes
   `cargo-semver-checks` can miss. Platform-gated API (FILESTREAM, the Windows
   certificate store CMK provider, SSPI auth) is frozen by a dedicated Windows
   CI leg against `public-api/<crate>.windows.txt`, since the Linux
   `--all-features` baseline cannot see `cfg(windows)` items.

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
| `json` | No | Stable |
| `otel` | No | Stable |
| `zeroize` | No | Stable |
| `azure-identity` | No | Stable (platform-dependent) |
| `integrated-auth` | No | Stable (platform-dependent) |
| `sspi-auth` | No | Stable (platform-dependent) |
| `cert-auth` | No | Stable |
| `always-encrypted` | No | Stable |
| `filestream` | No | Stable (Windows only) |
| `encoding` | Yes | Stable |
| `tls` | Yes | Stable |

## SQL Server Compatibility

| SQL Server Version | Support Level |
|-------------------|---------------|
| 2022+ | Full (TDS 8.0) |
| 2019 | Full (TDS 7.4) |
| 2017 | Full (TDS 7.4) |
| 2016 | Full (TDS 7.4) |
| 2012 / 2014 | Supported (TDS 7.4) |
| 2008 / 2008 R2 | Legacy (TDS 7.3; usually needs `Encrypt=no_tls`, see LIMITATIONS.md) |
| 2005 and earlier | Not supported |
| Azure SQL Database | Full |
| Azure SQL Managed Instance | Full |

**How each version is validated:** "Full"/"Supported"/"Legacy" describe protocol
capability, not test cadence. **SQL Server 2017, 2019, and 2022 are CI-verified**
— the integration suite runs the full ignored test suite against all three on
every change. SQL Server 2008–2016 and Azure SQL are validated manually against
real servers, not in CI.

SQL Server compatibility is considered part of our stability guarantee. Dropping support for a SQL Server version requires a major version bump (post-1.0).
