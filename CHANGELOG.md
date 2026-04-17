# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] - 2026-04-17

> **⚠️ v0.9.0 was yanked from crates.io.** v0.9.0 contained two critical bugs
> (LOGIN7 feature extension pointer indirection and `EncryptionContext`
> provider loss under `Config` clone) that prevented Always Encrypted from
> functioning at all. Both are fixed in this release. If you are evaluating
> or using Always Encrypted, upgrade directly from v0.8.x or earlier to
> v0.10.0 — do not use v0.9.0. Non-AE features in v0.9.0 functioned
> correctly, but upgrading is recommended regardless for the connection
> string, performance, and bulk insert improvements in this release.

This is a **correctness release**. Integration testing against a live SQL
Server uncovered a large number of shipped-in-release wire-format bugs across
bulk insert, stored procedures, RPC parameters, Always Encrypted, and the
query streams. Every such bug found is fixed and pinned by a regression test.
In aggregate, 51 commits land between v0.9.0 and v0.10.0, spanning roughly
100 new integration tests, 10+ critical protocol fixes, and several new
user-facing features. See the breakdown below.

### Added

- **`Client::bulk_insert()` end-to-end** — the `BulkInsert` packet generator
  shipped in v0.2.0 is now connected to an actual transport. Previously the
  example code ended at `bulk.take_packets()` with a comment to the effect of
  "in a real implementation these would be sent to the server." Bulk insert
  now streams BulkLoad (0x07) packets via `BulkWriter` and reads the server
  response. Works in both `Ready` and `InTransaction` states.
- **`Client::query_named()` / `Client::execute_named()`** — accept
  `&[NamedParam]` (as produced by `#[derive(ToParams)]`), closing the bridge
  between the derive macro and the client API. Previously `to_params()`
  returned a type the client could not consume.
- **`SendStringParametersAsUnicode` connection string option** — when set to
  `false`, `SqlValue::String` parameters are encoded as VARCHAR with the
  server's collation code page instead of NVARCHAR/UTF-16. Enables index
  seeks on VARCHAR-indexed columns. Default is `true` (NVARCHAR, unchanged).
- **`MultiSubnetFailover` connection string option** — when `true`, resolves
  the hostname to all addresses and races parallel TCP connects. Required
  for Always On Availability Group listener failover.
- **Connection retry loop** — `ConnectRetryCount` / `ConnectRetryInterval`
  connection string keywords (parsed since v0.5.x) now actually drive a
  retry loop in `Client::connect()` with exponential backoff for transient
  errors.
- **`in_params()` helper** — `mssql_client::in_params(start, count)` generates
  `(@pN, @pN+1, …)` SQL fragments for IN-clause composition without string
  building. Handles the eternal ergonomics gap between Rust slices and SQL
  IN lists.
- **`test_while_idle` pool config** — when enabled (default `false`), the
  pool reaper proactively pings idle connections with the configured health
  check query. Catches firewall timeouts / Azure idle disconnects before
  checkout rather than at first-request latency.
- **OTel pool metrics** — the `DatabaseMetrics` bridge now fires on every
  pool lifecycle event: create, close, checkout, expiration, discard,
  in-flight drop. Adds `PoolBuilder::pool_name()` for the
  `db.client.pool.name` label. Enabled via the `otel` feature on both
  `mssql-client` and `mssql-driver-pool`.
- **`Money`, `SmallMoney`, `SmallDateTime` wrapper types** — newtype wrappers
  around `Decimal` and `NaiveDateTime` that route to native TDS wire
  encoding (MONEY scaled-integer, SMALLDATETIME days+minutes) instead of
  falling back to DECIMAL / DATETIME2. `SqlValue::Money`, `SqlValue::SmallMoney`,
  `SqlValue::SmallDateTime` variants added.
- **`Row::from_values()` now public** — allows constructing `Row` objects for
  unit testing without going through a live server. Requested by Tiberius
  #383 (six years open).
- **Native binary encoding for all temporal and numeric RPC parameters** —
  `SqlValue::Date`, `::Time`, `::DateTime`, `::DateTimeOffset`, and
  `::Decimal` now use native TDS wire encoding (type IDs 0x28/0x29/0x2A/0x2B,
  and 0x6A DECIMALN) instead of serializing through NVARCHAR strings. Preserves
  sub-millisecond precision, enables index seeks, removes culturally-
  sensitive formatting dependencies.
- **Lazy `QueryStream` / `ResultSet` decoding** — typed `Row` objects are now
  constructed on demand when the caller pulls from the stream rather than
  eagerly during response parsing. Eliminates the
  `payload + Vec<Row>` double allocation for large result sets. Applies to
  all three response readers: `query()`, `query_multiple()`, and
  `call_procedure()`.
- **TEXT / NTEXT / IMAGE bulk insert rejected with redirect error** — rather
  than silently corrupting data on these 21-year-deprecated types, bulk
  insert now returns `TypeError::UnsupportedType` with a message naming
  the correct replacement (`VARCHAR(MAX)` / `NVARCHAR(MAX)` / `VARBINARY(MAX)`).
  Reading these columns in ordinary queries is still supported; only the
  bulk-insert write path is blocked.

### Fixed

- **Always Encrypted: LOGIN7 `ibExtension` pointer indirection (CRITICAL)** —
  per MS-TDS §2.2.6.4, `ibExtension` is the absolute offset of a 4-byte u32
  whose value is the FeatureExt data offset. The v0.9.x encoder set
  `ibExtension = base` (skipping the pointer indirection) AND computed
  `base` without accounting for the 4-byte pointer slot. SQL Server read
  the first four bytes of the FeatureExt blob as a u32 offset, landed deep
  inside the hostname string, and dropped the connection with no
  diagnostic. Every Always Encrypted connection attempt in v0.5.x through
  v0.9.x died at LOGIN7 as *"peer closed connection without sending TLS
  close_notify."* Pinned by `test_login7_feature_extension_pointer_indirection`.
- **Always Encrypted: `EncryptionContext::from_arc` dropped providers after
  `Config` clone (CRITICAL)** — `Client::connect` clones `Config` for
  retry/redirect handling, raising the inner `Arc<EncryptionConfig>`
  refcount above 1. `from_arc` called `Arc::try_unwrap`, which fails when
  refcount > 1, and fell back to an empty providers map with only a
  tracing warning. Every user who registered `InMemoryKeyStore`,
  `AzureKeyVaultProvider`, or `WindowsCertStoreProvider` silently got a
  context with no providers — every CEK lookup failed as
  `KeyStoreNotFound`. `EncryptionContext` now holds the Arc directly and
  delegates provider lookup to it.
- **`RETURNVALUE` token decode consumed phantom 2-byte length prefix
  (CRITICAL)** — per MS-TDS §2.2.7.18, the RETURNVALUE token has no outer
  length prefix; the decoder read two bytes that don't exist, shifted every
  subsequent field by 2 bytes, and the stream parser then read value bytes
  as the next token type. Every OUTPUT parameter value from stored
  procedures in v0.8.0 and v0.9.0 was garbage (when decode even completed
  at all). Test helpers authored against the buggy decoder were corrected.
- **`call_procedure` sent `@p1`/`@p2` positional names to named procedures
  (CRITICAL)** — the positional `call_procedure` path passed values through
  the same name-generation helper used for `sp_executesql`, producing names
  like `@p1`, `@p2`. SQL Server binds RPC parameters by name when names
  are non-empty; every procedure with real parameter names (virtually all
  of them) failed with server error 201: *"Procedure or function expects
  parameter '@a', which was not supplied."* Positional `call_procedure`
  now sends empty names, triggering by-position binding.
- **NVARCHAR RPC parameters miscounted supplementary Unicode characters
  (HIGH)** — the NVARCHAR length metadata used `value.chars().count()`
  (Rust chars, UTF-16 scalar values). Supplementary-plane characters
  (emoji, CJK Extension B) encode as surrogate pairs in UTF-16 and count
  as one char but two code units. Inputs containing such characters
  produced a length-mismatched RPC rejected by SQL Server with *"Data
  type 0xE7 has an invalid data length or metadata length."* Now counts
  UTF-16 code units via `encode_utf16().count()`.
- **UUID mixed-endian byte-order in `UNIQUEIDENTIFIER` decoding** — SQL
  Server stores `uniqueidentifier` with the first three groups byte-
  swapped vs. RFC 4122. The v0.9.x decoder returned the raw storage bytes
  without swapping, so reading a GUID from the database produced a UUID
  with different bytes than what was written. Fixed in both the standalone
  GUID column parser (`TypeId::Guid`) and the SQL_VARIANT `0x24` base-type
  path.
- **VARCHAR/CHAR bulk insert corrupted extended characters** — the bulk
  insert row encoder unconditionally encoded strings as UTF-16 regardless
  of the column's `type_id`. For VARCHAR (0xA7) / CHAR (0x2F) columns,
  SQL Server interpreted each UTF-16 code unit's low byte as one char and
  the padding high byte as another, so `"abc"` was stored as
  `"a\0b\0c\0"`. Now routes through the column collation's code page.
- **VARCHAR RPC params hardcoded Latin1_General_CI_AS / Windows-1252** —
  captured the server collation from the login ENVCHANGE and threaded it
  through `sql_value_to_rpc_param` so VARCHAR parameters use the server's
  actual code page. Fixes silent corruption of extended characters on
  Chinese/Cyrillic/Arabic-collation servers when `SendStringParametersAsUnicode=false`.
- **PLP length marker for NVARCHAR(MAX) / VARBINARY(MAX) bulk insert** —
  the 8-byte ULONGLONGLEN at the head of the PLP wire format was set to
  the actual byte count. SQL Server's BulkLoad (0x07) parser only accepts
  `PLP_UNKNOWN_LEN` (0xFFFFFFFFFFFFFFFE) here. Now emits the sentinel per
  MS-TDS §2.2.5.2.3 and the BCP-stricter parser requirement.
- **MONEY / SMALLMONEY / DATETIME / SMALLDATETIME bulk insert wire format** —
  these were being encoded as DECIMAL (length-prefixed mantissa) or
  DATETIME2 (time-then-date with scale) respectively, which SQL Server
  silently implicit-converted. Now emits the native formats: MONEY as
  `i64 * 10_000` (high u32 + low u32 LE), SMALLMONEY as `i32 * 10_000` LE,
  DATETIME as `days + 1/300s-ticks` (8 bytes), SMALLDATETIME as
  `days + minutes` (4 bytes) per MS-TDS §2.2.5.5.1.2.
- **SMALLDATETIME type ID wrong in bulk insert** — `parse_sql_type("SMALLDATETIME")`
  returned type ID `0x3F`, which is `TypeId::Numeric`. Correct is `0x3A`
  (DateTime4) or `0x6F` (DateTimeN). Fixed in both `parse_sql_type` and
  the `write_colmetadata` encoder.
- **COLMETADATA emitted variable-width type IDs for NOT NULL columns** —
  the hand-crafted COLMETADATA path (used when the server hasn't been
  queried for schema) emitted nullable type IDs (0x6E, 0x6F, etc.) even
  for columns declared NOT NULL. SQL Server rejected the resulting
  COLMETADATA as invalid. Now emits fixed-width variants (0x38, 0x3A,
  0x3C, 0x3D, 0x32, etc.) when the column is NOT NULL.
- **Hand-crafted COLMETADATA hardcoded Latin1 collation** — the fallback
  COLMETADATA path ignored `BulkColumn::with_collation()` and wrote
  Latin1_General_CI_AS regardless. Now honors the caller-supplied collation.
- **`(local)` host alias with named instance** — `Server=(local)\SQLEXPRESS`
  now resolves to 127.0.0.1 matching ADO.NET behavior.
- **PoolConfig `test_while_idle` rename semantics** — the existing
  `health_check_interval` continues to control the reaper tick, while
  actual idle pinging is behind the new `test_while_idle` flag (default
  `false`).
- **Option<T> in `#[derive(Tvp)]` now infers from inner T** — previously
  `Option<i32>` always mapped to NVARCHAR(MAX). Now recursively unwraps
  and infers from `T`, falling back to NVARCHAR(MAX) only when generic
  argument parsing fails.
- **SQL_VARIANT missing TIME / DATETIME2 / DATETIMEOFFSET base-type arms** —
  columns embedding TIME (0x29), DATETIME2 (0x2A), or DATETIMEOFFSET (0x2B)
  inside SQL_VARIANT fell through to the raw-bytes default instead of
  being parsed as typed values.
- **VARBINARY RPC param rejected empty and > 8000-byte buffers** —
  `varbinary(0)` is invalid type metadata; oversized fixed VARBINARY
  also fails server-side. Empty buffers now pad to `.max(1)` and buffers
  > 8000 bytes route through VARBINARY(MAX) / PLP.
- **`write_colmetadata` produced invalid wire format on some type
  families** — completed a per-type audit and fixed remaining issues
  alongside the collation/nullability fixes above. Five new live-server
  integration tests cover MONEY / SMALLMONEY / DATETIME / SMALLDATETIME,
  DATE / TIME(7) / DATETIME2 / DATETIMEOFFSET, UUID with asymmetric
  bytes, VARBINARY boundary sizes, and VARCHAR Latin-1 extended
  characters.
- **Broken fuzz targets (`parse_rpc.rs`, `collation_decode.rs`,
  `parse_login7.rs`)** — rewritten to match the current tds-protocol
  API after prior refactors broke them.
- **`test_pool_status_tracking` asserted `available == 0` without
  `.min_connections(0)`** — pool warm-up created one connection before
  the assertion ran. Test now configures `min_connections(0)` explicitly.

### Performance

- **Lazy-decode `QueryStream`** — `read_query_response` no longer eagerly
  decodes rows during response parsing. `PendingRow::Raw` / `PendingRow::Nbc`
  values stash cheap refcounted slices into the reassembled TDS payload;
  typed `Row`s are constructed on demand when callers pull via `Iterator`,
  `Stream`, or `collect_all`. Peak memory for large result sets drops from
  roughly 2× payload to 1× payload.
- **Lazy-decode `MultiResultStream` and `ProcedureResult`** — the same
  pattern applied to `read_multi_result_response` and
  `read_procedure_result`. `ResultSet` now stores `PendingRow` slices
  alongside `Option<ColMetaData>` and `Option<Arc<ColumnDecryptor>>` so
  Always Encrypted decryption still works in the lazy path.

### Changed

- **Behavior change (pre-1.0) — per-row decode errors now surface at
  iteration time** — `QueryStream::Iterator::next` and `Stream::poll_next`,
  `ResultSet::next_row`, and `MultiResultStream::next_row` now yield
  `Some(Err(_))` (or equivalent) for malformed row bytes rather than
  failing the outer `query().await?` / `call_procedure().await?` itself.
  Callers must check per-row results. `collect_all` short-circuits on
  first error so collector-based callers are unaffected.
- `ResultSet::next_row` now returns `Option<Result<Row, Error>>` (was
  `Option<Row>`).
- `ResultSet::collect_all` now returns `Result<Vec<Row>, Error>` (was
  `Vec<Row>`).
- `MultiResultStream::collect_current` now returns `Result<Vec<Row>, Error>`
  (was `Vec<Row>`).
- `BulkColumn::new` now returns `Result<Self, TypeError>` instead of
  `Self`, so it can reject deprecated TEXT/NTEXT/IMAGE types at
  construction time. Callers must `?`-propagate or `.unwrap()`.
- Extracted MONEY/DATETIME conversion helpers (`decimal_to_money_cents_i64`,
  `decimal_to_smallmoney_cents_i32`, `datetime_to_legacy_days_ticks`,
  `datetime_to_smalldatetime_days_minutes`) into `mssql_types::encode` so
  RPC and bulk/TVP paths share one implementation.
- Fixed a pre-existing carry-over bug in
  `datetime_to_smalldatetime_days_minutes`: 23:59:45 rounded up to 1440
  minutes, outside SQL Server's valid `[0, 1439]` range. Carry now
  propagates into the next day.

### Security

- **RUSTSEC-2026-0098 / RUSTSEC-2026-0099 (rustls-webpki)** — resolved
  via Cargo.lock update.

### Breaking Changes (pre-1.0)

Per STABILITY.md § Pre-1.0 Releases, breaking changes are permitted in
pre-1.0 minor bumps. All breaking changes are listed here with migration
notes.

#### 1. Per-row decode errors surface at stream iteration

- **What changed**: `QueryStream` iteration, `ResultSet::next_row`, and
  `MultiResultStream::next_row` now yield `Result` per row.
- **Why**: Lazy decoding eliminates the eager `Vec<Row>` double allocation,
  but a malformed row can no longer be raised from the outer `query()`
  future — the bytes haven't been parsed yet when `query()` returns.
- **Migration**: Check per-row results. For streams:
  ```rust
  // Before (v0.9.x)
  let rows = client.query("SELECT ...", &[]).await?;
  for row in rows {
      let name: String = row.get(0)?;
  }

  // After (v0.10.0)
  let rows = client.query("SELECT ...", &[]).await?;
  for row in rows {
      let row = row?;                       // new: handle per-row decode error
      let name: String = row.get(0)?;
  }
  ```
  `collect_all()` short-circuits on first error, so callers using it are
  unaffected.

#### 2. `ResultSet` / `MultiResultStream` return types

- **What changed**:
  - `ResultSet::next_row` — `Option<Row>` → `Option<Result<Row, Error>>`
  - `ResultSet::collect_all` — `Vec<Row>` → `Result<Vec<Row>, Error>`
  - `MultiResultStream::collect_current` — `Vec<Row>` → `Result<Vec<Row>, Error>`
- **Migration**: Add `?` to `collect_all` / `collect_current` call sites
  and `.unwrap()` or `?` to the inner `Result` for `next_row`.

#### 3. `BulkColumn::new` signature

- **What changed**: `BulkColumn::new(name, sql_type, ordinal)` now returns
  `Result<Self, TypeError>` (was `Self`). Attempting to construct a
  `BulkColumn` for `TEXT`, `NTEXT`, or `IMAGE` returns
  `TypeError::UnsupportedType` with a redirect to `VARCHAR(MAX)` /
  `NVARCHAR(MAX)` / `VARBINARY(MAX)`.
- **Migration**: `?`-propagate or `.unwrap()`.

#### 4. Deprecated types rejected in bulk insert

- **What changed**: `Client::bulk_insert` against a table whose server
  COLMETADATA reports `TEXT`, `NTEXT`, or `IMAGE` columns returns
  `TypeError::UnsupportedType`. Reading these columns in ordinary
  queries is still supported; only the bulk-insert write path is
  blocked.
- **Migration**: Migrate affected columns to `VARCHAR(MAX)` /
  `NVARCHAR(MAX)` / `VARBINARY(MAX)`:
  ```sql
  ALTER TABLE MyTable ALTER COLUMN Body VARCHAR(MAX);    -- was TEXT
  ALTER TABLE MyTable ALTER COLUMN Body NVARCHAR(MAX);   -- was NTEXT
  ALTER TABLE MyTable ALTER COLUMN Blob VARBINARY(MAX);  -- was IMAGE
  ```
  See `LIMITATIONS.md § TEXT / NTEXT / IMAGE`.

### Testing

- **~100 new integration tests** against a live SQL Server 2022 container,
  covering the bulk insert type matrix, RPC parameter round-trip for every
  `SqlValue` variant, Always Encrypted metadata/NULL roundtrip, non-Latin
  VARCHAR collation (Chinese_PRC, GB18030), TVP MONEY/SMALLMONEY/DATETIME/
  SMALLDATETIME/UUID, trigger row count, hand-crafted COLMETADATA, and
  cancel-safety pool discard.
- **Fuzz target expansion** — `type_roundtrip` now covers Decimal, UUID,
  Date, Time, DateTime, DateTimeOffset, and Xml.
- **Property tests** — 4 proptest invocations added in
  `crates/mssql-types/src/decode.rs` for decimal encoding.
- **Trybuild compile-fail tests** for the derive macros (6 tests).
- **Env var standardization** — all integration tests now use
  `MSSQL_HOST/USER/PASSWORD` (previously `edge_cases.rs` silently skipped
  without `MSSQL_TEST_*`).

### Known Limitations

- **Always Encrypted parameter encryption (write path) is not yet
  implemented.** The read path — including transparent decryption of
  encrypted columns, CEK resolution via Azure Key Vault, Windows
  Certificate Store, and custom providers, and NULL-value writes into
  encrypted columns — is fully supported and live-server validated in
  this release. Sending non-NULL plaintext into an encrypted column will
  be rejected by SQL Server with an "Operand type clash" error. See
  `docs/ALWAYS_ENCRYPTED.md § Limitations` and upcoming issue tracker
  entry.

## [0.9.0] - 2026-04-15

### Added

- **Always Encrypted decryption integration** — wired CryptoMetadata parsing and AEAD_AES_256_CBC_HMAC_SHA256 decryption into query execution. Encrypted columns are transparently decrypted when `Column Encryption Setting=Enabled` is set in the connection string. Decryption is supported across all response readers: `query()`, `call_procedure()`, and `query_multiple()`. CEK resolution is performed asynchronously at ColMetaData time; per-row decryption is synchronous in the hot path.
- **Native Windows SSPI authentication** — integrated auth (`Integrated Security=true`) now uses the native Windows SSPI subsystem (`secur32.dll`) instead of sspi-rs on Windows, supporting all account types including Microsoft Accounts, domain accounts, and local accounts without explicit credentials (closes #65)
- **FILESTREAM BLOB access** (Windows only, `filestream` feature) — async read/write access to SQL Server FILESTREAM data via `OpenSqlFilestream`. `FileStream` implements `AsyncRead + AsyncWrite` for tokio compatibility. Accessed via `Client<InTransaction>::open_filestream()` or the low-level `FileStream::open()` API. Requires the Microsoft OLE DB Driver for SQL Server at runtime. (closes #67)
- **34 new unit tests** for tds-protocol token parsing (ReturnValue, ReturnStatus, DoneProc, DoneInProc, ServerError, multi-token streams) and mssql-auth error/provider classification
- **ADO.NET connection string conformance** — comprehensive parser rewrite:
  - **Quoted value support** — `Password="my;complex;pass"` and `Password='it''s complex'` now work per ADO.NET spec. Previously, passwords with semicolons were silently truncated.
  - **`tcp:` prefix stripping** — `Server=tcp:host.database.windows.net,1433` (Azure Portal format) now works. `np:` and `lpc:` prefixes return clear errors.
  - **New Server aliases** — `Addr`, `Address`, `Network Address` now accepted per Microsoft docs
  - **`Timeout` alias** for Connect Timeout per ADO.NET spec
  - **ApplicationIntent** — `ReadOnly`/`ReadWrite` for AlwaysOn AG read-only routing, wired to LOGIN7 TypeFlags READONLY_INTENT bit
  - **Workstation ID** / `WSID` — client machine name for audit trails via `sys.dm_exec_sessions.host_name`
  - **Current Language** / `Language` — session language, wired to LOGIN7 Language field
  - **ConnectRetryCount** / **ConnectRetryInterval** — wired to RetryPolicy
  - **Pool keywords** (`Max Pool Size`, `Min Pool Size`, `Pooling`, etc.) — recognized with info-level guidance to use PoolConfig
  - **30+ known ADO.NET keywords** recognized at info level instead of silently ignored at debug level
  - **Boolean validation** — invalid values like `TrustServerCertificate=banana` now return errors instead of silently defaulting to false
  - **`Encrypt=Mandatory`/`Optional`** — Microsoft.Data.SqlClient v5+ aliases for `true`/`false` now accepted
  - **Case-insensitive protocol prefixes** — `Tcp:`, `TCP:`, `tCp:` all stripped correctly (not just `tcp:` and `TCP:`)
  - **Empty values reset optional fields** — `Database=;` now results in `None` instead of `Some("")`, matching ADO.NET reset-to-default behavior

### Changed

- **unwrap() audit** — replaced ~20 production `unwrap()` calls with `expect()` containing descriptive context strings across library code
- **panic! audit** — audited all panic-family macros (`panic!`, `unreachable!`, `unimplemented!`, `todo!`) in library code; converted one unjustified `unreachable!` to proper error propagation
- Updated LIMITATIONS.md to reflect v0.8.0+ features (stored procedures, SQL Browser, pool health checks, Always Encrypted)

### Fixed

- **Always Encrypted in procedures and multi-result queries** — decryption was missing from `read_procedure_result()` and `read_multi_result_response()`, causing encrypted columns to return raw ciphertext instead of plaintext when accessed via `call_procedure()` or `query_multiple()`
- **windows-certstore compilation errors** — resolved 9 compilation errors in the Always Encrypted Windows Certificate Store provider caused by API changes in the `windows` 0.62 crate (#83)
- **Silent error swallowing** — replaced `filter_map(|r| r.ok())` in test code with explicit `unwrap()` so failures are visible; documented intentional best-effort parsing in bulk insert type resolution
- **`(local)` host alias** — `Server=(local)\SQLEXPRESS` now correctly resolves to `127.0.0.1`, matching ADO.NET behavior. Previously only `.` was normalized to localhost (#66)
- **LOGIN7 HostName field** — now sends the actual client machine hostname (or `Workstation ID` if configured) instead of the server hostname. Previously `sys.dm_exec_sessions.host_name` showed the server's own name. Per MS-TDS spec, the LOGIN7 HostName field is "the name of the client machine."

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

[Unreleased]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.10.0...HEAD
[0.10.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/praxiomlabs/rust-mssql-driver/compare/v0.8.0...v0.9.0
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
