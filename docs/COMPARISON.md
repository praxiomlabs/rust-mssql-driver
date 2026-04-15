# Comparison With Other Rust MSSQL Drivers

This page compares `mssql-driver` with the other Rust options for SQL Server connectivity, based on source code analysis and public GitHub issues as of April 2026.

## Feature Matrix

| Feature | mssql-driver | Tiberius | odbc-api | sqlx-oldapi |
|---|---|---|---|---|
| TDS 8.0 (strict TLS) | Yes | No (Tiberius [#412]) | N/A | No |
| Always Encrypted | Yes | No (Tiberius [#54], 6yr open) | Via ODBC driver | No |
| Built-in connection pool | Yes | No (Tiberius [#146]) | No | No |
| Prepared statement cache | Yes (LRU) | No (Tiberius [#30], 6yr open) | Via ODBC | Yes |
| Table-valued parameters | Yes | No | Via ODBC driver | No (sqlx-oldapi [#46]) |
| Bulk insert (BCP) | Partial (packet encoding; transport pending) | Partial (Tiberius [#322], [#358]) | Yes | No |
| Query cancellation | Yes (attention) | No (Tiberius [#79], [#300]) | Via ODBC | No |
| Azure AD / Managed Identity | Yes | No (Tiberius [#175], 32 comments) | Via ODBC driver | No |
| Cross-platform NTLM | Yes | Windows only (Tiberius [#97]) | Via ODBC driver | No (sqlx-oldapi [#13]) |
| Kerberos / SPNEGO | Yes | Unix only | Via ODBC driver | No |
| Service Principal auth | Yes | No | Via ODBC driver | No |
| Client certificate auth | Yes | No | Via ODBC driver | No |
| Credential zeroization | Yes (`zeroize` feature) | No | No | No |
| Named instance (SQL Browser) | Yes | Yes | N/A | Yes |
| ADO.NET connection strings | Yes | Yes | N/A | No (sqlx [#411], [#605]) |
| ApplicationIntent (ReadOnly) | Yes | No | Via ODBC driver | No |
| Azure SQL redirect | Yes | Yes | N/A | No |
| Output parameters | Yes | Limited | Yes | No |
| JSON type | Yes | No | Via ODBC driver | Via NVARCHAR |
| `deny(unsafe_code)` | Yes (workspace-wide) | No (4 unsafe blocks) | No (372 unsafe) | No |
| Fuzz testing | 11 targets | No | No | No |
| Property testing | Yes (proptest) | No | No | No |
| Compile-time query checking | No | No | No | Yes (limited) |
| Runtime agnostic | No (Tokio only) | Yes | N/A (sync) | Yes |
| MARS | No | No | Via ODBC driver | No |

## Protocol Coverage

`mssql-driver` implements TDS 7.3 through 8.0 from scratch in pure Rust. Key protocol capabilities:

- **TDS 8.0 strict mode** — TLS-before-handshake, required by modern Azure SQL and SQL Server 2022+ strict configurations
- **PreLogin/Login7** — full negotiation including SSPI, Azure AD token, FeatureExt
- **SQL Batch and RPC** — automatic routing based on parameter presence
- **Attention packets** — clean query cancellation without connection corruption
- **ENVCHANGE routing** — transparent Azure SQL Gateway redirect handling

## Authentication

| Method | mssql-driver | Tiberius | odbc-api | sqlx-oldapi |
|---|---|---|---|---|
| SQL Authentication | Yes | Yes | Yes | Yes |
| Windows / NTLM | Yes (cross-platform) | Windows only | Via driver | No |
| Kerberos / SPNEGO | Yes (`integrated-auth`) | Unix (libgssapi) | Via driver | No |
| Azure AD Token | Yes | User-provided only | Via driver | No |
| Managed Identity | Yes (`azure-identity`) | No | Via driver | No |
| Service Principal | Yes (`azure-identity`) | No | Via driver | No |
| Client Certificate | Yes (`cert-auth`) | No | Via driver | No |
| Windows SSPI | Yes (`sspi-auth`) | No | Via driver | No |

sqlx-oldapi's lack of Windows authentication was called a "blocker" by enterprise users (sqlx-oldapi [#13]).

## Architecture

| Property | mssql-driver | Tiberius | odbc-api | sqlx-oldapi |
|---|---|---|---|---|
| Crate structure | 9-crate workspace | Monolith | 2-crate workspace | 7-crate workspace |
| `no_std` support | Yes (`tds-protocol`) | No | No | No |
| Unsafe discipline | `deny(unsafe_code)` | 4 blocks | 372 blocks (FFI) | 0 in MSSQL code |
| Error model | `thiserror`, `#[non_exhaustive]` | `thiserror` | `thiserror` + ODBC diags | SQLx errors |
| Type-state connections | Yes (compile-time) | No | No | No |
| TODO/FIXME in source | Zero (CI-enforced) | Several | 1 | Several (inherited) |
| CI pipeline | 10+ jobs, Miri, fuzz, semver-checks | Clippy + 3 SQL versions | Clippy + 5 databases | Clippy + tests |

## Deployment

`mssql-driver` is pure Rust with no C/FFI dependencies (outside optional Windows SSPI). This solves real deployment problems reported by odbc-api users:

| Problem | odbc-api Issue | mssql-driver |
|---|---|---|
| Can't compile for musl/Alpine | [#526] | Pure Rust compiles everywhere |
| Static linking blocked by LGPL (unixODBC) | [#781] | No LGPL dependencies |
| iODBC vs unixODBC conflicts | [#503], [#148], [#502] | No driver manager needed |
| Connection not `Send`/`Sync` | [#354], [#115], [#276] | `Send + Sync` by design |
| `Connection::drop()` panics through FFI | [#574] | Pure Rust error handling |
| No true async | [#578], [#253], [#255] | Tokio-native from the ground up |
| Buffer pre-allocation OOM | [#41], [#192], [#212] | Streams actual data |

## Known Limitations

- **Tokio only** — no async-std or smol support (intentional design choice)
- **MARS not supported** — use connection pooling as a workaround
- **Pre-1.0** — API may change between minor versions
- **Smaller ecosystem** — no ORM integrations yet

See [LIMITATIONS.md](../LIMITATIONS.md) for the complete list.
