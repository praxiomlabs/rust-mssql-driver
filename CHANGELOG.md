# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Bulk Copy Protocol (BCP) support with packet type 0x07 for high-performance data loading
- OpenTelemetry instrumentation module with SQL sanitization and span attributes
- Comprehensive examples (basic, transactions, bulk_insert, derive_macros, streaming)

### Changed

- Simplified `SavePoint` struct to remove unnecessary lifetime parameter
- Row parsing now uses unified decode module from mssql-types for better performance

## [0.1.0] - 2025-01-XX

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

[Unreleased]: https://github.com/yourusername/rust-mssql-driver/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/rust-mssql-driver/releases/tag/v0.1.0
