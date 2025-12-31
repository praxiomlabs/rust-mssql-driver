# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

## [0.2.0] - 2024-12-24

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

[Unreleased]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/praxiomlabs/rust-mssql-driver/releases/tag/v0.1.0
