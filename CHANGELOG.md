# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Always Encrypted decryption integration** — wired CryptoMetadata parsing and AEAD_AES_256_CBC_HMAC_SHA256 decryption into query execution. Encrypted columns are transparently decrypted when `Column Encryption Setting=Enabled` is set in the connection string. Decryption is supported across all response readers: `query()`, `call_procedure()`, and `query_multiple()`. CEK resolution is performed asynchronously at ColMetaData time; per-row decryption is synchronous in the hot path.
- **Native Windows SSPI authentication** — integrated auth (`Integrated Security=true`) now uses the native Windows SSPI subsystem (`secur32.dll`) instead of sspi-rs on Windows, supporting all account types including Microsoft Accounts, domain accounts, and local accounts without explicit credentials (closes #65)
- **FILESTREAM BLOB access** (Windows only, `filestream` feature) — async read/write access to SQL Server FILESTREAM data via `OpenSqlFilestream`. `FileStream` implements `AsyncRead + AsyncWrite` for tokio compatibility. Accessed via `Client<InTransaction>::open_filestream()` or the low-level `FileStream::open()` API. Requires the Microsoft OLE DB Driver for SQL Server at runtime. (closes #67)
- **34 new unit tests** for tds-protocol token parsing (ReturnValue, ReturnStatus, DoneProc, DoneInProc, ServerError, multi-token streams) and mssql-auth error/provider classification

### Changed

- **unwrap() audit** — replaced ~20 production `unwrap()` calls with `expect()` containing descriptive context strings across library code
- **panic! audit** — audited all panic-family macros (`panic!`, `unreachable!`, `unimplemented!`, `todo!`) in library code; converted one unjustified `unreachable!` to proper error propagation
- Updated LIMITATIONS.md to reflect v0.8.0+ features (stored procedures, SQL Browser, pool health checks, Always Encrypted)

### Fixed

- **Always Encrypted in procedures and multi-result queries** — decryption was missing from `read_procedure_result()` and `read_multi_result_response()`, causing encrypted columns to return raw ciphertext instead of plaintext when accessed via `call_procedure()` or `query_multiple()`
- **windows-certstore compilation errors** — resolved 9 compilation errors in the Always Encrypted Windows Certificate Store provider caused by API changes in the `windows` 0.62 crate (#83)
- **Silent error swallowing** — replaced `filter_map(|r| r.ok())` in test code with explicit `unwrap()` so failures are visible; documented intentional best-effort parsing in bulk insert type resolution

## [0.8.0] - 2026-04-13

### Added

- **Stored procedure support** with two-tier API:
  - `client.call_procedure("dbo.MyProc", &[&1i32])` — simple convenience for input-only calls
  - `client.procedure("dbo.MyProc")?.input("@a", &val).output_int("@result").execute()` — full builder with named input/output parameters
  - `ProcedureResult` type with return value, rows affected, output parameters, and result sets
  - `ProcedureBuilder` with typed output methods: `output_int`, `output_bigint`, `output_nvarchar`, `output_bit`, `output_float`, `output_decimal`, `output_raw`
  - Works in both `Ready` and `InTransaction` states
  - All procedure names validated to prevent SQL injection
  - Feature scope, API design, and documentation structure informed by PR #71 from @c5soft
- **SQL Browser instance resolution** for named instances (`host\SQLEXPRESS`):
  - Automatic TCP port discovery via SQL Server Browser service (UDP 1434)
  - Transparent integration into `Client::connect()` — no API changes needed
  - Supports `.` as localhost (e.g., `Server=.\SQLEXPRESS`)
  - Requested by @tracker1 in #66
- **Pool `test_on_checkin` health check** — connections returned to the pool are now health-checked before reuse when enabled (closes #29)
- `ProcedureResult` and `ResultSet` now implement `Clone`
- Added `col_type: u8` field and `#[non_exhaustive]` to protocol-level `ReturnValue` struct

### Fixed

- **Mock TLS cross-platform race** — fixed `TlsPreloginWrapper` handshake-to-passthrough race condition that caused test failures on macOS and Windows (closes #70). Root cause: client sends raw TLS before server-side wrapper transitions from PreLogin framing
- **RUSTSEC-2026-0097** (rand 0.8.5 unsoundness) — added ignore with justification (log feature not enabled, blocked on upstream rsa 0.10 stable)
- Resolved stale advisory ignore for RUSTSEC-2026-0066 (fixed by testcontainers 0.27 bump)
- Updated RUSTSEC-2025-0134 ignore reason (rustls-pemfile is a direct dep of mssql-auth, not just transitive via bollard)

### Changed

- **Azure SDK bump**: azure_core/azure_identity 0.30 → 0.34, azure_security_keyvault_keys 0.9 → 0.13
  - `ClientCertificateCredential::new()` now takes `SecretBytes` instead of `Secret`
  - Key Vault `unwrap_key()`/`sign()`/`verify()` now require `key_version` as a method parameter
- Bumped dev dependencies: testcontainers 0.25 → 0.27, criterion 0.7 → 0.8, rustls 0.23.37 → 0.23.38, tokio 1.51.0 → 1.51.1
- Bumped CI actions: codecov-action v5 → v6, action-gh-release v2 → v3, github-script v8 → v9
- Extracted `validate_identifier()` and `validate_qualified_identifier()` to shared `validation` module
## [0.7.0] - 2026-04-07

This is a **security + API hardening release**. It resolves seven RUSTSEC
advisories, wires SSPI integrated authentication into the client login flow,
hardens the public API surface with `#[non_exhaustive]` on 33 public enums,
removes deprecated items ahead of 1.0, and bumps MSRV to Rust 1.88 to unblock
the security fixes.

### Security

- **Resolved 7 RUSTSEC advisories** (closes #63):
  - RUSTSEC-2026-0044: aws-lc X.509 Name Constraints Bypass via Wildcard/Unicode CN
  - RUSTSEC-2026-0045: aws-lc AES-CCM Timing Side-Channel
  - RUSTSEC-2026-0046: aws-lc PKCS7 Certificate Chain Validation Bypass
  - RUSTSEC-2026-0047: aws-lc PKCS7 Signature Validation Bypass
  - RUSTSEC-2026-0048: aws-lc CRL Distribution Point Scope Check Logic Error
  - RUSTSEC-2026-0049: rustls-webpki CRL matching logic flaw
  - RUSTSEC-2026-0009: time crate DoS via stack exhaustion (unblocked by MSRV bump)

  Fixed via `cargo update`: aws-lc-sys 0.35 → 0.39, rustls 0.23.36 → 0.23.37,
  rustls-webpki 0.103.8 → 0.103.10, time 0.3.45 → 0.3.47, plus 100+ other
  transitive patch bumps.

- **SQL injection hardening in bulk insert**: Validate identifiers in generated
  SQL to close a remaining injection vector; remove any residual credential
  logging from bulk insert code paths.

- **Savepoint identifier validation**: Identifiers passed to savepoint/release
  operations now pass through the same `validate_identifier()` regex guard used
  for other SQL identifiers.

### Added

- **SSPI / integrated authentication is now functional end-to-end** (closes #64):
  - `SspiAuth` (Windows via sspi-rs) and `IntegratedAuth` (Linux/macOS via GSSAPI)
    are now invoked during `Client::connect()`. Previously they compiled but
    were never called, producing SQL Server error 18456.
  - New `SspiNegotiator` trait abstracts the `initialize` / `step` / `is_complete`
    handshake. Both providers implement it; the client login loop drives the
    SPNEGO challenge/response over TDS SSPI packets (type 0x11).
  - `Credentials::integrated()` constructor for API consistency with
    `Credentials::sql_server()` / `Credentials::azure_token()`.
  - Connection-string support for `Integrated Security=true` / `sspi` / `yes` /
    `1` and `Trusted_Connection=true` (ADO.NET-compatible keywords).
  - `Credentials::Integrated` variant is now gated on
    `#[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]` so Windows
    users with only `sspi-auth` can construct it.
  - `mssql-driver-pool` now forwards `integrated-auth` and `sspi-auth` features
    through to `mssql-client`, so pool users can enable SSPI via the pool crate
    directly.

- **`#[non_exhaustive]` on 33 public enums** for forward-compatible evolution
  post-1.0. Users matching on these enums must now include a wildcard arm.
  Consult the Breaking Changes section for the full list and migration notes.

- **Error classification API**: `Error::is_transient()`, `is_terminal()`, and
  related predicates now cover the complete set of error variants, enabling
  programmatic retry decisions without pattern matching on internal shapes.

- **TLS: rustls PKI type re-exports** and DER convenience methods in `mssql-tls`
  so downstream code can construct `TlsConfig` without pulling in `rustls_pki_types`
  directly.

- **Testing: TLS support in the mock TDS server** used by integration tests,
  enabling coverage of the PreLogin-wrapped TLS handshake path.

- **Tooling**:
  - `cargo xtask ci-local` mirrors GitHub Actions locally for faster feedback.
  - `cargo xtask release` automates version bumping across the workspace.
  - CI feature-combination validation now uses `cargo-hack` instead of a
    hand-maintained matrix.

### Changed

- **BREAKING (pre-1.0 minor bump allowance)**: 33 public enums across the
  workspace are now `#[non_exhaustive]`. See "Breaking Changes" below for the
  full list and migration guide.

- **BREAKING**: Deprecated items scheduled for removal before 1.0 have been
  removed. See "Breaking Changes" below.

- **Error handling**: Error chains are now preserved end-to-end (`source()` works
  correctly across layer boundaries), and `Error::Server` display output is
  significantly improved with structured server/procedure/line context.

- **Timeout errors**: `Error::ConnectTimeout`, `Error::TlsTimeout`, and
  `Error::LoginTimeout` now carry `host: String` and `port: u16` fields so
  diagnostics show which endpoint timed out. Previously these variants were
  anonymous and required the caller to remember which host was being dialed.

- **TLS ALPN**: ALPN protocols (`tds/8.0` etc.) are now applied in all TLS paths,
  including the PreLogin-wrapped handshake. Previously they were only set in the
  TDS 8.0 strict mode path, causing servers to fall back to TLS defaults.

- **TDS 8.0 strict mode now rejects `TrustServerCertificate=true`** at
  connect time. Strict mode's entire security premise requires certificate
  validation; silently honoring the flag was misleading. Use `Encrypt=true`
  (non-strict) if you need to disable cert validation for legacy servers.

- **OpenTelemetry crate versions aligned at 0.32** (from the previous mix of
  0.31 / 0.32). All `opentelemetry*` dependencies now move together.

- **Internal refactors** (no API impact):
  - `mssql-client/src/config.rs` split into a directory module with submodules.
  - `mssql-derive` split from a monolithic `lib.rs` into per-macro modules.
  - `mssql-client/src/client.rs` split into `client/connect.rs`, `client/params.rs`,
    `client/response.rs`, and `client/mod.rs`.
  - Column value parsing extracted into `mssql-client/src/column_parser.rs`.
  - `workspace-hack` crate and `cargo-hakari` configuration removed (dead code).
  - `tokio` dependency tightened from `features = ["full"]` to the minimal feature
    set actually used, reducing build time and binary size.
  - `syn` features in `mssql-derive` reduced to the minimum needed for our macros.
  - Platform dependency installation centralized in a reusable GitHub Actions
    composite action.

### Fixed

- **SSPI login with empty credentials** (#64): `Client::connect()` now drives
  the SSPI handshake when `Credentials::Integrated` is configured. Before, the
  Login7 packet was sent with an empty username and password, producing
  `error 18456: Login failed for user ''`.
- **Error chain preservation**: `Error::Io` now correctly exposes the underlying
  `std::io::Error` via `source()`, enabling downstream code to inspect causes.
- **`mssql-testing` accidentally published** to crates.io: the crate now has
  `publish = false` and is excluded from the publish workflow. Published versions
  of `mssql-testing` are cosmetic and should not be depended on.
- **CI MSRV drift**: the MSRV job now reads the required Rust version from
  `Cargo.toml` dynamically instead of hardcoding a version that could drift.
- **Release workflow resilience**: publish steps now use exponential retry with
  a sane backoff to tolerate crates.io index propagation delays.
- **CI xtask tool checks**: the `xtask` CLI now verifies tool availability
  (cargo-deny, cargo-nextest, etc.) and prints install instructions rather than
  failing with opaque errors.

### Removed

- **Deprecated APIs removed before 1.0** — see "Breaking Changes" for the list
  and migration notes.
- **`workspace-hack` crate and `cargo-hakari` configuration** (dead code; had
  no effect on build performance).

### Breaking Changes

Per STABILITY.md § Pre-1.0 Releases, breaking changes are permitted in pre-1.0
minor bumps. All breaking changes are documented here with migration notes.

#### 1. MSRV bumped to Rust 1.88

- **What changed**: Minimum Supported Rust Version is now **1.88** (up from 1.85).
- **Why**: Required to pull in the `time 0.3.47` patch that fixes RUSTSEC-2026-0009
  (DoS via stack exhaustion in RFC 2822 parsing). Rust 1.88 has been stable since
  June 2025.
- **Migration**: Update your toolchain. The project follows a rolling 6-month
  MSRV window aligned with Tokio's policy (see ARCHITECTURE.md §6.6).
- **Per STABILITY.md § MSRV Increase Policy**, MSRV bumps are not considered
  breaking changes in the semver sense. Documented here for completeness.

#### 2. Public enums marked `#[non_exhaustive]`

- **What changed**: 33 public enums across `mssql-client`, `mssql-auth`,
  `mssql-types`, `mssql-pool`, and `tds-protocol` now carry `#[non_exhaustive]`.
  This prevents downstream code from exhaustively matching on them, which would
  block us from adding new variants without a breaking change.
- **Affected enums include**: `Error` (and its internal variants), `Credentials`,
  `AuthMethod`, `EncryptionLevel`, `TdsVersion`, `TokenType`, `ColumnType`,
  `PacketType`, and many more.
- **Why**: Necessary hardening before we stabilize the 1.0 API. Without this,
  every new `Error` variant or `Credentials` kind would be a breaking change
  post-1.0.
- **Migration**:
  ```rust
  // Before:
  match err {
      Error::Io(_) => ...,
      Error::Protocol(_) => ...,
      Error::Server { .. } => ...,
      // Missing variants cause compile error
  }

  // After:
  match err {
      Error::Io(_) => ...,
      Error::Protocol(_) => ...,
      Error::Server { .. } => ...,
      _ => ...,  // wildcard now required
  }
  ```
  If you were exhaustively matching, add a wildcard arm. Consider whether the
  wildcard should map to a sensible default (e.g., "unknown error, retry once")
  rather than a panic.

#### 3. Deprecated items removed

- **What changed**: Items marked `#[deprecated]` in prior 0.x releases have been
  removed. This includes old `Config::new()` forms, pre-builder API entry points,
  and internal-but-`pub` items that were kept for backwards compatibility.
- **Migration**: If `cargo build` fails on `0.7.0` with "cannot find function X",
  check the deprecation notice in your previous `0.6.x` build output — it will
  name the replacement API. All replacements are `Config::builder()`-style
  fluent APIs.

#### 4. `Error` variant shape changes

- **What changed**: `Error::ConnectTimeout`, `Error::TlsTimeout`, `Error::LoginTimeout`
  now carry `{ host: String, port: u16 }` fields instead of being unit variants.
- **Migration**:
  ```rust
  // Before:
  Err(Error::ConnectTimeout) => ...

  // After:
  Err(Error::ConnectTimeout { host, port }) => ...
  // Or, if you don't care about the context:
  Err(Error::ConnectTimeout { .. }) => ...
  ```

#### 5. TDS 8.0 strict mode rejects `TrustServerCertificate=true`

- **What changed**: Configurations that combine `Encrypt=strict` and
  `TrustServerCertificate=true` now return `Error::Config` at connect time.
- **Why**: Strict mode's security guarantees depend on full certificate chain
  validation. Silently honoring `TrustServerCertificate=true` was a footgun.
- **Migration**: If you actually need to skip cert validation, use
  `Encrypt=true` (non-strict TLS). If you're using strict mode because you need
  TDS 8.0 features, your server should have a valid certificate.

### MSRV

- **MSRV**: Rust **1.88** (up from 1.85, see Breaking Changes #1).

### Internal

- Dropped the unused `workspace-hack` crate and `cargo-hakari` config.
- `mssql-testing` is no longer published to crates.io (`publish = false`).
- Pre-commit hooks now run fmt + clippy + typecheck on every commit.
- Documentation additions: test taxonomy, xtask command reference, feature
  matrix, module graphs, crate taxonomy, SAFETY comments on all `unsafe` FFI
  blocks (Windows cert store, etc.).

## [0.6.0] - 2026-01-12

### Added

- **PEM certificate support**: `CertificateAuth::from_pem()` constructor for users with PEM-formatted certificates (common in Linux/Kubernetes environments)
- **Decimal support for Money types**: Money, SmallMoney, and MoneyN columns now return `rust_decimal::Decimal` when the `decimal` feature is enabled, preventing precision loss in financial applications
- **Optional TLS feature**: TLS dependencies are now behind the `tls` feature flag (enabled by default). Disable for `Encrypt=no_tls` connections to reduce binary size (~2-3 MB) and speed up compilation. Useful for enterprise internal networks, Kubernetes clusters, and legacy SQL Server environments

### Fixed

- **VARCHAR decoding for non-UTF8 encodings**: Fixed incorrect decoding of VARCHAR columns with legacy encodings (GBK, Shift-JIS, etc.) where the UTF-8 fast-path would incorrectly accept valid UTF-8 byte sequences that were actually non-UTF8 encoded data
- **Security vulnerabilities**: Updated dependencies to resolve RUSTSEC-2026-0001 (rkyv) and RUSTSEC-2026-0002 (lru)

### Changed

- Money type parsing consolidated into single `parse_money_value()` helper function

## [0.5.2] - 2026-01-04

### Added

- `Encrypt=no_tls` connection string option to disable TLS entirely for legacy SQL Server instances
- `Config::no_tls()` builder method for programmatic configuration
- `SqlServerVersion` type for proper SQL Server product version representation

### Fixed

- PreLogin VERSION field now correctly identified as SQL Server product version, not TDS protocol version
- Logging now shows correct product names (e.g., "SQL Server 2012") instead of "Unknown SQL Server version"
- TVP (Table-Valued Parameter) RPC declarations now correctly use table type names (e.g., `@p1 dbo.IntIdList READONLY`) instead of falling through to `sql_variant`

## [0.5.1] - 2026-01-03

### Added

- CI: Feature flag validation job to catch no_std/alloc compatibility issues early
- Tests: Unit tests for credentials module and query builder

### Fixed

- `tds-protocol` no_std + alloc support: Added internal prelude module for consistent type imports
- Benchmark CI workflow: Fixed output format compatibility with github-action-benchmark
- Benchmark CI workflow: Graceful handling when no baseline exists for PR comparisons

### Changed

- Documentation: Updated production readiness and contributing guidelines

## [0.5.0] - 2026-01-01

### Added

#### Collation-Aware VARCHAR Decoding
- New `encoding` feature for proper decoding of VARCHAR columns with non-UTF-8 collations
- `Collation::encoding()` method returns the `encoding_rs::Encoding` for a collation's LCID
- `Collation::encoding_name()` method returns human-readable encoding name
- `Column.collation` field exposes collation metadata for string columns
- `Column.encoding_name()` convenience method (requires `encoding` feature)
- Support for Windows code pages: 1252 (Latin1), 1251 (Cyrillic), 1250 (Central European),
  932 (Shift_JIS), 936 (GB18030), 949 (Korean), 950 (Big5), and more
- Example: `collation_encoding.rs` demonstrating collation-aware decoding

### Changed

- **BREAKING**: `Column` struct marked `#[non_exhaustive]`
  - Use `Column::new()` or builder methods to construct instances
  - Allows future field additions without breaking changes

### Fixed

- VARCHAR columns with non-UTF-8 collations (e.g., `SQL_Latin1_General_CP1_CI_AS`) now decode correctly
- Previously, characters like "Café" would display as "Caf�" when using Windows-1252 encoding

## [0.4.0] - 2025-12-31

### Added

#### TDS 7.3 Protocol Support (SQL Server 2008/2008 R2)
- `TdsVersion::V7_3A` constant for SQL Server 2008
- `TdsVersion::V7_3B` constant for SQL Server 2008 R2
- Connection string option `TDSVersion` (or `ProtocolVersion`) to specify TDS version
- `Config::tds_version()` builder method for programmatic configuration
- `TdsVersion::parse()` for parsing version strings ("7.3", "7.3A", "7.3B", "7.4", "8.0")
- `TdsVersion::sql_server_version_name()` for human-readable SQL Server version names
- `TdsVersion::is_tds_7_3()` and `TdsVersion::is_tds_7_4()` helper methods
- `TdsVersion::is_legacy()` to detect TDS 7.2 and earlier
- `TdsVersion::supports_date_time_types()` for feature detection
- Version negotiation logging during connection

#### Testing & CI
- Comprehensive version compatibility test suite (19 tests)
- SQL Server version detection tests

### Changed

- **BREAKING**: `Config` struct marked `#[non_exhaustive]`
  - Use `Config::new()`, `Config::default()`, or builder methods to construct
  - Allows future field additions without breaking changes
- `TdsVersion::Display` now shows correct format (e.g., "TDS 7.3A" instead of "TDS 7.10")
- Default TDS version is `V7_4` for broad compatibility with SQL Server 2012+
- Setting `TdsVersion::V8_0` automatically enables `strict_mode`

### Fixed

- Justfile `confirm` syntax compatibility with just 1.45+
- Semver check is now advisory for pre-1.0 releases (breaking changes allowed in 0.x.y)

## [0.3.0] - 2025-12-24

### Added

#### Always Encrypted Key Providers
- Azure Key Vault CMK provider (`azure-identity` feature)
  - RSA-OAEP and RSA-OAEP-256 key unwrapping
  - Key versioning support
  - Automatic credential management via Azure Identity SDK
- Windows Certificate Store CMK provider (`sspi-auth` feature, Windows only)
  - NCRYPT API integration for secure key operations
  - Certificate thumbprint-based key lookup

#### LOB Streaming
- `Row::get_stream(index)` and `Row::get_stream_by_name(name)` methods
- `BlobReader` integration for streaming LOB data via `AsyncRead`
- Improved memory efficiency for large binary/text columns

#### Change Tracking
- `ChangeTrackingQuery` builder for generating CHANGETABLE queries
- `ChangeOperation` enum (Insert, Update, Delete)
- `ChangeMetadata` struct for tracking version info
- `ChangeTracking` helper with SQL generation utilities
- `SyncVersionStatus` for validating sync state

#### Pool Improvements
- `PoolMetrics` extended with:
  - `connections_idle_expired`: Connections closed due to idle timeout
  - `connections_lifetime_expired`: Connections closed due to max lifetime
  - `reaper_runs`: Number of reaper task executions
  - `peak_wait_queue_depth`: Peak wait queue observed
  - `avg_acquisition_time_us`: Average acquisition time
- `PoolStatus.wait_queue_depth`: Current wait queue depth
- `PoolConfig.health_check_query`: Configurable health check SQL

### Changed

- **BREAKING**: `PoolConfig`, `PoolStatus`, and `PoolMetrics` are now `#[non_exhaustive]`
  - Use builder pattern or `Default::default()` to construct
  - Allows future field additions without breaking changes
- Updated `azure_identity` SDK to 0.30 (API changes for `ClientCertificateCredential`)
- Updated `azure_security_keyvault_keys` SDK to 0.9.0

### Fixed

- Azure Identity SDK compatibility: Updated `ClientCertificateCredential` usage for 0.30 API

## [0.2.0] - 2025-12-24

### Added

#### Authentication
- Azure Managed Identity authentication (`azure-identity` feature)
- Azure Service Principal authentication (`azure-identity` feature)
- Kerberos/GSSAPI authentication for Linux/macOS (`integrated-auth` feature)
- Windows SSPI/Kerberos authentication (`sspi-auth` feature)
- Client certificate authentication for Azure AD (`cert-auth` feature)

#### Table-Valued Parameters (TVP)
- Full TVP support with `#[derive(Tvp)]` macro
- DateTimeOffset encoding for TVP columns
- All SQL Server types supported in TVP rows

#### Always Encrypted
- Client-side encryption infrastructure
- Column Encryption Key (CEK) management
- AEAD_AES_256_CBC_HMAC_SHA256 algorithm support
- RSA-OAEP key unwrapping

#### Query Execution
- Explicit query cancellation via `CancelHandle`
- Per-query timeout configuration
- Secure credential handling with `zeroize` feature

#### Observability
- OpenTelemetry Metrics integration (`otel` feature)
- SQL sanitization for span attributes
- Comprehensive instrumentation module

#### Other
- Bulk Copy Protocol (BCP) support for high-performance data loading
- Comprehensive examples (basic, transactions, bulk_insert, derive_macros, streaming)

### Changed

- Simplified `SavePoint` struct to remove unnecessary lifetime parameter
- Row parsing now uses unified decode module from mssql-types for better performance
- Added `#[non_exhaustive]` to public enums for semver safety
- Updated dependencies: sspi 0.18, criterion 0.7, testcontainers 0.25, webpki-roots 1.0

### Fixed

- Cargo.lock consistency for webpki-roots versions
- CI semver-checks configuration for Kerberos headers

## [0.1.0] - 2025-12-16

Initial release of the rust-mssql-driver project.

### Added

#### Core Features
- Type-state pattern for compile-time connection state enforcement
- Async/await support built on Tokio 1.48+
- TDS 7.4 and TDS 8.0 (strict encryption mode) protocol support
- TLS negotiation via rustls (pure Rust, no OpenSSL dependency)

#### Connection Management
- Connection configuration via connection strings
- Azure SQL redirect handling with automatic failover
- Configurable timeouts for connect, TLS, and query operations
- MARS (Multiple Active Result Sets) support flag

#### Query Execution
- Simple queries without parameters (SQL batch)
- Parameterized queries via sp_executesql RPC
- Streaming result sets with memory-efficient iteration
- QueryStream supporting both Iterator and async Stream patterns

#### Transaction Support
- BEGIN/COMMIT/ROLLBACK transaction management
- Savepoint creation and rollback
- Transaction isolation level configuration
- Type-state transitions for transaction safety

#### Type System
- Comprehensive SQL Server type mapping (INT, BIGINT, VARCHAR, NVARCHAR, etc.)
- Feature-gated support for chrono (date/time), uuid, decimal, and json types
- Zero-copy byte access via Arc<Bytes> pattern
- FromSql/ToSql traits for type conversion

#### Connection Pooling (mssql-pool)
- Semaphore-based connection management
- Configurable pool size and timeout settings
- Connection health checking and recycling
- Pool metrics collection

#### Derive Macros (mssql-derive)
- `#[derive(FromRow)]` for automatic row-to-struct mapping
- `#[derive(ToParams)]` for struct-to-parameter conversion
- `#[derive(Tvp)]` for table-valued parameter support
- Field renaming via `#[mssql(rename = "...")]` attribute

#### Protocol Layer (tds-protocol)
- Full TDS packet framing (header + payload)
- PreLogin and LOGIN7 packet construction
- Token stream parsing (COLMETADATA, ROW, NBCROW, DONE, ERROR, etc.)
- RPC request encoding for stored procedure calls

#### Build Infrastructure
- cargo-xtask automation (test, lint, fmt, coverage, fuzz, etc.)
- Mock TDS server for unit testing (mssql-testing)
- GitHub Actions CI workflow
- cargo-deny for dependency auditing

### Security
- Savepoint name validation to prevent SQL injection
- TLS certificate validation with trust_server_certificate option
- Secure credential handling for SQL authentication

### Crate Structure
- `tds-protocol` - Pure TDS protocol implementation (no_std compatible)
- `mssql-tls` - TLS negotiation layer
- `mssql-codec` - Async framing and message reassembly
- `mssql-types` - SQL ↔ Rust type mapping
- `mssql-auth` - Authentication strategies
- `mssql-pool` - Connection pooling
- `mssql-client` - High-level client API
- `mssql-derive` - Procedural macros
- `mssql-testing` - Test infrastructure

[Unreleased]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/praxiomlabs/rust-mssql-driver/releases/tag/v0.1.0
