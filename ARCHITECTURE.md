# Architectural Reference: Rust MS SQL Driver

**Version:** 1.9.0
**Status:** Implemented and maintained (tracks the released `0.x` crates)
**Target Protocol:** MS-TDS 7.3 – 8.0 (SQL Server 2008 – 2025)
**Toolchain Standard:** Rust 2024 Edition (v1.85+, released February 20, 2025)
**MSRV Policy:** Rust 1.88.0 (6-month rolling window aligned with Tokio's policy)

---

## Table of Contents

1. [Project Manifesto](#1-project-manifesto)
2. [System Architecture](#2-system-architecture)
3. [Architectural Decision Records](#3-architectural-decision-records)
4. [Technical Implementation Specification](#4-technical-implementation-specification)
5. [Security Architecture](#5-security-architecture)
6. [Development Standards & Tooling](#6-development-standards--tooling)
7. [Implementation Roadmap](#7-implementation-roadmap)
8. [Appendices](#8-appendices)

---

## 1. Project Manifesto

This project aims to build the definitive Microsoft SQL Server driver for the Rust ecosystem. It prioritizes correctness, performance, and modern protocol support over backward compatibility or runtime agnosticism.

### 1.1 Core Tenets

1. **Tokio-Native:** Unapologetically built on Tokio 1.48+ to use the standard Rust async runtime ecosystem without compatibility shims or abstraction overhead.

2. **TDS 8.0 Strict Mode:** Designed for the strict TLS 1.3 architecture of SQL Server 2022+ where TLS handshake precedes all TDS messages, while maintaining support for legacy TDS 7.4 encryption negotiation.

3. **Minimal Allocation Churn:** Reduces memory allocation pressure through strategic use of reference counting (`Arc<Bytes>`) and buffer slicing. Note: This is *reduced*-copy rather than true zero-copy, which would require arena allocation or self-referential structures.

4. **Safety by Design:** Utilizes Rust's type system to render invalid protocol states uncompilable through genuine compile-time type-state patterns.

5. **Modern Workspace Standards:** Adopts a flat workspace layout, centralized dependency management, and eliminates legacy module patterns (`mod.rs`).

### 1.2 Competitive Positioning

See the [README](README.md) and [MIGRATION.md](MIGRATION.md) for how this driver
compares to `tiberius` and other options. In brief: Tokio-native (no
runtime-agnostic compatibility layer), TDS 8.0 strict support, and built-in
pooling and transactions.

### 1.3 Non-Goals (Explicit Exclusions)

The following are explicitly out of scope for v1.0:

- **Runtime Agnosticism:** No `async-std`, `smol`, or generic executor support
- **MARS (Multiple Active Result Sets):** Deferred to v2.0 if demand warrants
- **Named Pipe Transport:** Windows-only, limited use case
- **Shared Memory Protocol:** Undocumented, no existing Rust implementations
- **SQL Server 2005 and Earlier:** TDS 7.2 and earlier protocols are not supported

### 1.4 TDS Protocol Version Support

The driver supports TDS 7.3A (SQL Server 2008) through TDS 8.0 (SQL Server
2022+ strict mode) and defaults to TDS 7.4 for modern deployments; the
negotiated version is selectable via `TdsVersion`. See §8.1 (protocol-version
features) and §8.2 (the full SQL Server compatibility matrix) for details.

---

## 2. System Architecture

The project follows a **Flat Workspace** layout. The root `Cargo.toml` is a virtual manifest; all code resides in `crates/`. This structure enforces strict boundaries and reduces compilation times.

### 2.1 Crate Topology

```
mssql-driver/
├── Cargo.toml                    # Virtual manifest
├── rust-toolchain.toml           # Pin to 1.88+
├── deny.toml                     # cargo-deny configuration
├── hakari.toml                   # cargo-hakari configuration
├── crates/
│   ├── tds-protocol/             # Pure protocol logic (no_std)
│   ├── mssql-tls/                # TLS negotiation isolation
│   ├── mssql-codec/              # Async framing layer
│   ├── mssql-types/              # SQL ↔ Rust type mapping
│   ├── mssql-auth/               # Authentication strategies
│   ├── mssql-pool/               # Connection pooling (publishes as mssql-driver-pool)
│   ├── mssql-client/             # Public API surface
│   ├── mssql-derive/             # Procedural macros
│   └── mssql-testing/            # Test infrastructure
├── xtask/                        # Build automation
└── examples/                     # Usage examples
```

### 2.2 Crate Dependency Graph

```
mssql-pool                        built-in connection pooling
└── mssql-client                  ← public API surface
    ├── mssql-codec  ──►  tds-protocol
    ├── mssql-auth                authentication strategies
    ├── mssql-tls                 TLS negotiation
    ├── mssql-types               SQL ↔ Rust type mapping
    ├── mssql-derive              row-mapping proc macros
    └── tds-protocol              no_std core TDS protocol

Leaf crates (no workspace-internal dependencies):
  tds-protocol, mssql-types, mssql-tls, mssql-auth, mssql-derive.
mssql-codec depends only on tds-protocol; mssql-pool depends on mssql-client.
```

### 2.3 Crate Specifications

#### `crates/tds-protocol`

**Responsibility:** Pure implementation of MS-TDS packet structure, token parsing, and serialization.

**Constraints:**
- `no_std` compatible (`alloc` required)
- Must remain IO-agnostic—contains no network logic
- No dependencies on `tokio` or any async runtime

**Dependencies:** `bytes`, `thiserror`, `bitflags`

**Key Types:**
```rust
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub status: PacketStatus,
    pub length: u16,
    pub spid: u16,
    pub packet_id: u8,
    pub window: u8,
}

pub enum Token {
    ColMetaData(ColMetaData),
    Row(RawRow),
    NbcRow(NbcRow),
    Done(Done),
    DoneProc(DoneProc),
    DoneInProc(DoneInProc),
    ReturnStatus(i32),
    ReturnValue(ReturnValue),
    Error(ServerError),
    Info(ServerInfo),
    LoginAck(LoginAck),
    EnvChange(EnvChange),
    Order(Order),
    FeatureExtAck(FeatureExtAck),
}
```

#### `crates/mssql-tls`

**Responsibility:** Isolates TLS negotiation complexity including:
- TDS 7.x pre-login encryption negotiation
- TDS 8.0 strict mode (TLS-first handshake)
- Certificate validation and hostname verification
- TLS 1.2/1.3 protocol selection

**Dependencies:** `rustls`, `webpki-roots`, `tokio-rustls`

**Rationale:** TDS TLS negotiation is sufficiently complex to warrant isolation. TDS 8.0 fundamentally changes the handshake order (TCP → TLS → TDS vs. TCP → TDS prelogin → TLS → TDS), and this crate encapsulates that complexity.

#### `crates/mssql-codec`

**Responsibility:** Async framing layer. Transforms `AsyncRead`/`AsyncWrite` byte streams into high-level `Packet` structures.

**Key Features:**
- Packet reassembly across TCP segment boundaries
- Packet continuation handling (large packets split across multiple TDS packets)
- IO splitting for cancellation safety (see ADR-004)

**Dependencies:** `tds-protocol`, `tokio-util` (Codec), `bytes`, `tokio`

#### `crates/mssql-types`

**Responsibility:** Bidirectional mapping between SQL Server data types and Rust types.

**Coverage:**

| SQL Server Type | Rust Type | Notes |
|-----------------|-----------|-------|
| `BIT` | `bool` | |
| `TINYINT` | `u8` | |
| `SMALLINT` | `i16` | |
| `INT` | `i32` | |
| `BIGINT` | `i64` | |
| `REAL` | `f32` | |
| `FLOAT` | `f64` | |
| `DECIMAL`/`NUMERIC` | `rust_decimal::Decimal` | |
| `MONEY`/`SMALLMONEY` | `rust_decimal::Decimal` | |
| `CHAR`/`VARCHAR` | `String`, `&str` | |
| `NCHAR`/`NVARCHAR` | `String`, `&str` | UTF-16 → UTF-8 conversion |
| `VARCHAR(MAX)` | `String`, streaming | PLP encoding |
| `VARBINARY(MAX)` | `Vec<u8>`, streaming | PLP encoding |
| `DATE` | `chrono::NaiveDate` | TDS 7.3+ |
| `TIME` | `chrono::NaiveTime` | TDS 7.3+ |
| `DATETIME2` | `chrono::NaiveDateTime` | TDS 7.3+ |
| `DATETIMEOFFSET` | `chrono::DateTime<FixedOffset>` | TDS 7.3+ |
| `DATETIME` | `chrono::NaiveDateTime` | Legacy |
| `SMALLDATETIME` | `chrono::NaiveDateTime` | Legacy |
| `UNIQUEIDENTIFIER` | `uuid::Uuid` | |
| `XML` | `String` | |
| `JSON` | `serde_json::Value` | SQL Server 2016+ |

**Dependencies:** `bytes`, `chrono`, `uuid`, `rust_decimal`, `serde_json`

#### `crates/mssql-auth`

**Responsibility:** Authentication strategy implementations, isolated from connection logic.

**Supported Methods:**

| Method | Implementation | Feature Flag |
|--------|---------------|--------------|
| SQL Authentication | Pure Rust | Default |
| Azure AD / Entra ID Token | Pure Rust | Default |
| Managed Identity (Azure VM/Container) | Pure Rust + HTTP | `azure-identity` |
| Service Principal | Pure Rust + HTTP | `azure-identity` |
| Certificate-based (Entra) | Pure Rust + HTTP | `cert-auth` |
| Default credential chain | Pure Rust + HTTP | `azure-identity` |
| Integrated Auth (Kerberos) | `gssapi` bindings | `integrated-auth` |
| Integrated Auth (NTLM) | `sspi` bindings (Windows) | `integrated-auth` |

**Dependencies:** `tds-protocol`, `tokio`, optional: `gssapi`, `sspi-rs`, `azure_identity`

#### `crates/mssql-pool` (publishes as `mssql-driver-pool`)

**Responsibility:** Purpose-built connection pool with SQL Server-specific lifecycle management.

**Note:** Directory is `mssql-pool` but publishes to crates.io as `mssql-driver-pool` (the name `mssql-pool` is taken by an unrelated Rocket-specific crate).

**Key Features:**
- `sp_reset_connection` execution on connection return
- Health checks via `SELECT 1`
- Configurable min/max pool sizes
- Connection timeout and idle timeout
- Automatic reconnection on transient failures
- Per-connection state tracking

**Dependencies:** `mssql-client`, `tokio`

#### `crates/mssql-client`

**Responsibility:** Primary public API surface. Users interact exclusively through this crate.

**Key Types:**
- `Client<S>` — Type-state connection (see §4.1)
- `Transaction<'a>` — Transaction scope with lifetime tied to connection
- `Query` — Prepared statement builder
- `Row` — Result row accessor
- `Column` — Column metadata
- `Error` — Unified error type

**Dependencies:** All internal crates, `rustls`, `tokio`

#### `crates/mssql-derive`

**Responsibility:** Procedural macros for ergonomic type mapping.

**Provided Macros:**
- `#[derive(FromRow)]` — Map `Row` to struct
- `#[derive(ToSql)]` — Enable struct as query parameter
- `#[derive(Tvp)]` — Table-Valued Parameter mapping

**Constraint:** Strictly separated due to `proc-macro = true` requirements.

#### `crates/mssql-testing`

**Responsibility:** Test infrastructure for integration testing.

**Capabilities:**
- Mock TDS server for unit tests
- Recorded packet replay for regression tests
- Docker-based SQL Server test containers
- Fixture generation utilities
- Connection string builders for test scenarios

**Dependencies:** `tokio`, `testcontainers`, `tds-protocol`

#### `xtask`

**Responsibility:** Rust-native build helpers that the `Justfile` (the
canonical task runner) should not own.

**Commands:**
```bash
cargo xtask check-features   # Validate all feature flag combinations compile (CI gate)
```

---

## 3. Architectural Decision Records (ADR)

### ADR-001: Tokio Runtime Hard Dependency

**Status:** Accepted

**Decision:** The driver will depend directly on `tokio` 1.48+ for networking, concurrency, and time.

**Context:** Supporting generic runtimes (`async-std`, `smol`) requires compatibility layers that introduce:
- CPU overhead from trait object dispatch
- Inability to use runtime-specific optimizations (`tokio::select!`, `spawn_blocking`)
- Complexity in timeout and cancellation handling

**Consequences:**
- Users of other runtimes cannot use this driver natively
- Enables use of `tokio::io::split()` for cancellation safety

**Alternatives Considered:**
- Generic `AsyncRead`/`AsyncWrite` bounds: Rejected due to performance overhead
- Runtime detection: Rejected due to complexity and maintenance burden

---

### ADR-002: Connection String Format

**Status:** Accepted

**Decision:** Support ADO.NET-compatible connection strings as the primary configuration method, with a builder API as an alternative.

**Format:**
```
Server=tcp:hostname,port;Database=dbname;User Id=user;Password=pass;Encrypt=strict;
```

**Supported Keywords:**

| Keyword | Aliases | Description |
|---------|---------|-------------|
| `Server` | `Data Source`, `Address` | Host and optional port |
| `Database` | `Initial Catalog` | Database name |
| `User Id` | `UID`, `User` | SQL authentication username |
| `Password` | `PWD` | SQL authentication password |
| `Encrypt` | | `true`, `false`, `strict`, `no_tls` |
| `TrustServerCertificate` | | Skip certificate validation |
| `Authentication` | | `SqlPassword`, `ActiveDirectoryServicePrincipal` (`User Id=<client-id>@<tenant-id>`), `ActiveDirectoryManagedIdentity` / `ActiveDirectoryMSI`, `ActiveDirectoryDefault` (managed identity → `az`/`azd` CLI chain) — Azure AD values use the FEDAUTH SecurityToken workflow and require the `azure-identity` feature. Certificate credentials are programmatic-only (`Credentials::certificate`, `cert-auth` feature). The interactive flows (`ActiveDirectoryPassword` / `Interactive` / `DeviceCodeFlow`) are not built in — `azure_identity` ships no such credentials; pass a pre-acquired token via `Credentials::azure_token` |
| `Application Name` | | Application identifier |
| `Connect Timeout` | | Connection timeout in seconds |
| `Command Timeout` | | Default command timeout |
| `Pooling` | | Enable connection pooling |
| `Min Pool Size` | | Minimum pool connections |
| `Max Pool Size` | | Maximum pool connections |

**Builder API:**
```rust
let config = Config::builder()
    .host("sql.example.com")
    .port(1433)
    .database("mydb")
    .authentication(AuthMethod::sql_server("user", "pass"))
    .encrypt(EncryptMode::Strict)
    .build()?;
```

---

### ADR-003: Authentication Strategy (Tiered Support)

**Status:** Accepted

**Decision:** Support for authentication will be tiered to prioritize cloud-native and modern workloads while isolating legacy complexity.

**Tier 1 (Core — Pure Rust, Default Features):**

| Method | Description | Token Source |
|--------|-------------|--------------|
| SQL Authentication | Username/password via Login7 | N/A |
| Azure AD Token | Pre-acquired access token | User-provided |

**Tier 2 (Azure Native — `azure-identity` Feature):**

| Method | Description | Token Source |
|--------|-------------|--------------|
| Managed Identity | Azure VM/Container/App Service | Azure IMDS endpoint |
| Service Principal | Client ID + Secret | Entra ID token endpoint |
| Service Principal (Cert) | Client ID + Certificate | Entra ID token endpoint |
| Azure CLI | Development scenarios | `az account get-access-token` |
| DefaultAzureCredential | Chained credential lookup | Multiple sources |

Token acquisition uses `azure_identity` crate for HTTP calls to identity endpoints.

> **Known trade-off:** The `azure-identity` feature pulls in `openssl` as a transitive
> dependency via the Azure SDK. This contradicts the project's rustls-only philosophy
> but is unavoidable — the Azure SDK for Rust does not provide rustls-only builds.
> This only affects users who enable the `azure-identity` feature; the default build
> remains pure-Rust with rustls.

**Tier 3 (Enterprise/Legacy — `integrated-auth` Feature):**

| Method | Platform | Implementation |
|--------|----------|----------------|
| Kerberos | Linux/macOS | `gssapi` crate bindings |
| NTLM/Kerberos | Windows | `sspi-rs` crate bindings |

**Rationale:** Separating authentication tiers prevents heavyweight platform dependencies in default builds while supporting enterprise scenarios when explicitly enabled.

---

### ADR-004: Memory Model (The `Arc<Bytes>` Pattern)

**Status:** Accepted

**Decision:** Row data will be stored using `bytes::Bytes` rather than strictly owned types (`String`, `Vec<u8>`) or borrowed references (`&'a [u8]`).

**Context:** High-throughput database access generates massive allocation pressure. True zero-copy parsing makes resulting `Row` structs difficult to use due to lifetime constraints ("Lifetime Hell").

**Mechanism:**
```rust
pub struct Row {
    /// Shared reference to raw packet body
    buffer: Arc<Bytes>,
    /// Column offsets into buffer
    columns: Arc<[ColumnSlice]>,
    /// Column metadata
    metadata: Arc<ColMetaData>,
}

struct ColumnSlice {
    offset: u32,
    length: u32,
    is_null: bool,
}
```

**Access Patterns:**

```rust
impl Row {
    /// Returns borrowed slice into buffer (zero additional allocation)
    pub fn get_bytes(&self, index: usize) -> Option<&[u8]>;
    
    /// Returns Cow - borrowed if valid UTF-8, owned if conversion needed
    pub fn get_str(&self, index: usize) -> Option<Cow<'_, str>>;
    
    /// Allocates new String (explicit allocation)
    pub fn get_string(&self, index: usize) -> Option<String>;
    
    /// Type-converting accessor with allocation only if needed
    pub fn get<T: FromSql>(&self, index: usize) -> Result<T, Error>;
}
```

**Result:** The `Row` struct is `'static`, `Send`, and `Sync` (easy to use across threads/functions) while deferring allocation until explicitly requested.

---

### ADR-005: IO Splitting for Cancellation Safety

**Status:** Accepted

**Decision:** The TCP stream will be split into `OwnedReadHalf` and `OwnedWriteHalf` immediately upon connection establishment.

**Context:** SQL Server uses out-of-band "Attention" packets to cancel running queries. If the driver is blocked awaiting a read (processing a large result set), it must still be able to write an Attention packet.

**Implementation:**
```rust
pub struct Connection {
    read_half: OwnedReadHalf,
    write_half: Arc<Mutex<OwnedWriteHalf>>,
    cancel_token: CancellationToken,
}

impl Connection {
    pub async fn cancel(&self) -> Result<(), Error> {
        let mut writer = self.write_half.lock().await;
        writer.write_all(&ATTENTION_PACKET).await?;
        Ok(())
    }
}
```

**Alternative Considered:** Dedicated cancellation channel with background writer task. Deferred to v2.0 if mutex contention proves problematic under high load.

**Partial Token Stream Handling:** When cancellation occurs mid-result, the state machine transitions to `Draining` state, consuming tokens until `Done` with `ATTENTION` flag is received.

---

### ADR-006: Concurrency Model (No MARS in v1.0)

**Status:** Accepted

**Decision:** Multiple Active Result Sets (MARS) will not be supported in v1.0.

**Context:** MARS adds significant complexity:
- Session multiplexing logic in protocol layer
- Request/response correlation tracking
- Potential for server-side locking under certain workloads

**Handling Multiple Result Sets:** Stored procedures returning multiple result sets (e.g., `sp_help`) are supported via sequential result set consumption:

```rust
let mut results = client.query("EXEC sp_help 'Users'", &[]).await?;

// First result set (QueryStream is a synchronous iterator of Result<Row>)
while let Some(row) = results.next() {
    let _row = row?;
    // Process row
}

// Advance to the next result set
if results.next_result().await? {
    while let Some(row) = results.next() {
        let _row = row?;
        // Process next result set
    }
}
```

**Resolution:** Concurrent queries must use separate connections via `mssql-pool`. MARS may be considered for v2.0 based on user demand.

---

### ADR-007: Streaming vs. Buffered Results

**Status:** Accepted

**Decision:** Two query paths are offered — a buffered convenience path
(`query`) and a true incremental streaming path (`query_stream` /
`query_stream_blob`) — mirroring `tokio-postgres` (`query`/`query_raw`) and
`sqlx` (`fetch_all`/`fetch`).

> **Implementation status (2026-06):** true incremental network streaming is
> implemented. `query_stream` reads TDS packets on demand and yields rows
> without buffering the whole response (peak memory ≈ one packet + one row);
> `query_stream_blob` sub-streams a row's trailing MAX column from the socket
> (peak ≈ one chunk). The buffered `query` remains for the common small-result
> case — it reassembles the full payload then decodes rows lazily (peak ≈ raw
> payload size). Validated by the counting-allocator tests in
> `tests/streaming_memory.rs`: a ≈10 MB result set and a 30 MB BLOB each keep
> peak heap delta under the test's conservative 2 MB assertion (observed peaks
> are far lower — roughly ~20 KB and ~9 KB respectively — but the asserted
> bound is loose to stay robust against allocator noise). The buffered path
> would peak near the full payload (~40 MB for the same result set).

**Buffered (default, convenient — `QueryStream`):**
```rust
// Synchronously iterable; whole response buffered, rows decode lazily.
let rows = client.query("SELECT * FROM small_table", &[]).await?;
for row in rows {
    let _row = row?;
}
// or collect:
let rows: Vec<Row> = client.query(sql, &[]).await?.collect_all().await?;
```

**Incremental streaming (large result sets — `RowStream`):**
```rust
// Reads packets on demand; peak memory ~ one row, not the whole result set.
let mut stream = client.query_stream("SELECT * FROM huge_table", &[]).await?;
while let Some(row) = stream.try_next().await? {
    let _row = row?; // process and drop; nothing else is buffered
}
```

**Large Object (BLOB) sub-streaming (`BlobStream`):**
```rust
// The trailing MAX column streams from the socket, never fully materialized.
let mut stream = client
    .query_stream_blob("SELECT id, blob_column FROM documents", &[])
    .await?;
while let Some(row) = stream.next().await? {
    let _id: i32 = row.get_by_name("id")?;
    stream.copy_blob_to(&mut file).await?;
}
```

Cancellation/early-drop: `RowStream::cancel` (and `BlobStream` drop) send an
Attention and drain so the connection stays reusable; the split-I/O design
(ADR-005) carries the Attention path.

---

### ADR-008: Transaction Isolation and Savepoints

**Status:** Accepted

**Decision:** Full support for transaction isolation levels and savepoints.

**Isolation Levels:**
```rust
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,      // Default
    RepeatableRead,
    Serializable,
    Snapshot,           // Requires database configuration
}
```

**Transaction API:**
```rust
// Basic transaction
let tx = client.begin_transaction().await?;
tx.execute("INSERT INTO ...").await?;
tx.commit().await?;

// With isolation level
let tx = client
    .begin_transaction_with_isolation(IsolationLevel::Serializable)
    .await?;

// Savepoints
let tx = client.begin_transaction().await?;
tx.execute("INSERT INTO orders ...").await?;
let sp = tx.save_point("before_items").await?;
tx.execute("INSERT INTO order_items ...").await?;
// Oops, rollback just the items
tx.rollback_to(&sp).await?;
tx.commit().await?;
```

---

### ADR-009: Error Classification and Retry Policy

**Status:** Accepted

**Decision:** Errors will be classified into categories that inform retry decisions.

**Error Taxonomy:**

```rust
pub enum ErrorKind {
    /// Network-level failures - generally retryable
    Io(IoErrorKind),
    
    /// Protocol violations - indicates driver bug
    Protocol(ProtocolError),
    
    /// Server-reported errors - classified by severity
    Server(ServerError),
    
    /// Authentication failures - not retryable
    Auth(AuthError),
    
    /// Configuration errors - not retryable
    Config(ConfigError),
    
    /// Timeout errors - may be retryable
    Timeout(TimeoutKind),
}

impl ServerError {
    pub fn is_transient(&self) -> bool {
        matches!(self.number,
            1205 |      // Deadlock victim
            -2 |        // Timeout
            10928 |     // Resource limit (Azure)
            10929 |     // Resource limit (Azure)
            40197 |     // Service error (Azure)
            40501 |     // Service busy (Azure)
            40613 |     // Database unavailable (Azure)
            49918 |     // Cannot process request (Azure)
            49919 |     // Cannot process create/update (Azure)
            49920 |     // Cannot process request (Azure)
            4060 |      // Cannot open database
            18456       // Login failed (may be transient in Azure)
        )
    }
    
    pub fn is_terminal(&self) -> bool {
        matches!(self.number,
            102 |       // Syntax error
            207 |       // Invalid column
            208 |       // Invalid object
            547 |       // Constraint violation
            2627 |      // Unique constraint violation
            2601        // Duplicate key
        )
    }
}
```

**Retry Configuration:**
```rust
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}
```

---

### ADR-010: Workspace Configuration Strategy

**Status:** Accepted

**Decision:** Use centralized configuration in the root `Cargo.toml`.

**Implementation:** the root `Cargo.toml` centralizes `[workspace.package]`
(version, edition, license, MSRV) and `[workspace.dependencies]` so each
dependency version is declared once. See the live root `Cargo.toml` for the
current values — they are release-managed, so an inline copy here would only
drift. Workspace-wide lints are centralized too:

```toml
[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
todo = "warn"
dbg_macro = "warn"
```

**Tooling:**
- `cargo-hakari` prevents feature unification recompilation bloat
- `cargo-deny` enforces license compliance and bans duplicate dependencies

#### Version Constraint Policy

Dependencies use **minimum version constraints** (caret requirements) rather than exact pins to reduce maintenance burden and allow compatible updates:

```toml
# PREFERRED: Allows compatible updates (default Cargo behavior)
tokio = "1.48"           # Equivalent to "^1.48", allows 1.48.x and 1.49.x etc.
rustls = "0.23"          # Allows 0.23.x updates
opentelemetry = "0.32"   # Allows 0.32.x updates

# AVOID: Creates immediate tech debt, blocks security patches
tokio = "=1.48.0"        # Exact pin - requires manual update for every patch
```

**When exact pins are appropriate:**
- Known breaking changes in patch releases (document reason in comment)
- Security-sensitive crates requiring audit of each update
- Temporary workaround for upstream bugs (with tracking issue)

**Example with justification:**
```toml
# Pinned due to breaking change in 0.24.1 - see issue #123
some-crate = "=0.24.0"
```

---

### ADR-011: Module File Structure

**Status:** Accepted

**Decision:** Strictly ban `mod.rs` files.

**Pattern:** Use Rust 2018+ "file-alongside-directory" pattern:
```
src/
├── client.rs           # mod client
├── client/
│   ├── connection.rs   # mod client::connection
│   ├── query.rs        # mod client::query
│   └── transaction.rs  # mod client::transaction
├── error.rs
└── lib.rs
```

**Rationale:** Reduces filesystem noise, eliminates ambiguity between `foo/mod.rs` and `foo.rs`, improves IDE navigation.

---

### ADR-012: Bulk Copy (BCP) Support

**Status:** Accepted

**Decision:** Native support for bulk insert operations using the TDS Bulk Load protocol.

**API:**
```rust
let builder = BulkInsertBuilder::new("dbo.Users")
    .with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)?,
        BulkColumn::new("name", "NVARCHAR(100)", 1)?,
        BulkColumn::new("email", "NVARCHAR(200)", 2)?,
    ])
    .with_options(BulkOptions {
        batch_size: 1000, // sent as a ROWS_PER_BATCH hint
        check_constraints: true,
        fire_triggers: false,
        keep_nulls: true,
        table_lock: true,
        order_hint: None,
    });

let mut writer = client.bulk_insert(&builder).await?;
for user in users {
    writer.send_row_values(&[
        SqlValue::Int(user.id),
        SqlValue::String(user.name),
        SqlValue::String(user.email),
    ])?;
}

let result = writer.finish().await?;
println!("Inserted {} rows", result.rows_affected);
```

**Current limits:** rows are buffered in memory and sent as a single batch on
`finish()` — `BulkOptions::batch_size` is only a `ROWS_PER_BATCH` server hint.
Incremental client-side flushing and a streaming input API are planned
alongside the response-streaming work.

---

### ADR-013: Always Encrypted Support

**Status:** Implemented

**Decision:** Always Encrypted client-side decryption (the read path) is fully implemented with production-ready key providers. The encrypt-before-send write path is implemented for the full scalar, temporal, and fixed-width type set (#234) — parameters are described via `sp_describe_parameter_encryption` and encrypted client-side. See LIMITATIONS.md for the exact type list and constraints (e.g. encrypted `decimal` is bounded to scale ≤ 28).

**Implemented (v0.2.0):**
- AEAD_AES_256_CBC_HMAC_SHA256 encryption/decryption
- RSA-OAEP key unwrapping for CEK decryption
- CEK caching with TTL expiration
- InMemoryKeyStore for testing/development
- `KeyStoreProvider` trait for custom implementations

**Implemented (v0.3.0):**
- `AzureKeyVaultProvider` for Azure Key Vault integration (`azure-keyvault` feature)
- `WindowsCertStoreProvider` for Windows Certificate Store (`windows-certstore` feature, Windows only)

**Security Guidance:**

Always Encrypted provides **client-side encryption** where keys never reach the database server. This protects against:
- Compromised database administrators
- Server-side breaches and malware
- Cloud operator access to sensitive data

| Threat | Always Encrypted | T-SQL Encryption (`ENCRYPTBYKEY`) |
|--------|------------------|-----------------------------------|
| Malicious DBA | Protected | Vulnerable |
| Server Compromise | Protected | Vulnerable |
| Cloud Operator Access | Protected | Vulnerable |
| Keys Stored On Server | Never | Yes |

**Important:** T-SQL encryption functions (`ENCRYPTBYKEY`/`DECRYPTBYKEY`) provide **server-side encryption** where keys exist within SQL Server. They do **NOT** provide the same security guarantees as Always Encrypted and should not be considered a substitute.

**Available Key Providers:**
- **For development/testing:** Use the `InMemoryKeyStore` with the `always-encrypted` feature
- **For Azure Key Vault:** Use `AzureKeyVaultProvider` with the `azure-keyvault` feature
- **For Windows Certificate Store:** Use `WindowsCertStoreProvider` with the `windows-certstore` feature
- **For custom key storage:** Implement the `KeyStoreProvider` trait for your key management solution

**Do NOT use `ENCRYPTBYKEY`** as a workaround - it does not provide the same security guarantees

**References:**
- [Always Encrypted Overview](https://learn.microsoft.com/en-us/sql/relational-databases/security/encryption/always-encrypted-database-engine)
- [Key Management for Always Encrypted](https://learn.microsoft.com/en-us/sql/relational-databases/security/encryption/overview-of-key-management-for-always-encrypted)

---

### ADR-014: OpenTelemetry Instrumentation

**Status:** Accepted

**Decision:** OpenTelemetry tracing support via an optional feature flag.

**Feature:** `otel` (optional, not default)

**Instrumentation Points:**
- Connection establishment (span)
- Query execution (span with SQL as attribute, sanitized)
- Transaction boundaries (span)
- Connection pool metrics (gauge)
- Error events

**Example:**
```rust
// Automatic instrumentation when feature enabled
// With the `otel` feature enabled, instrumentation is automatic.
let client = Client::connect(config).await?;

// Spans are created automatically for each query
client.query("SELECT * FROM users WHERE id = @p1", &[&user_id]).await?;
```

---

## 4. Technical Implementation Specification

### 4.1 Connection State Machine (Compile-Time Type-State)

The client implements a **genuine compile-time type-state pattern** where invalid protocol states are **uncompilable**, not merely represented as runtime errors.

```rust
//! Type-state markers - these are zero-sized types
pub struct Disconnected;
pub struct Connected;
pub struct Ready;
pub struct InTransaction;
pub struct Streaming;

/// Client with compile-time state tracking
pub struct Client<S> {
    inner: Arc<ClientInner>,
    _state: PhantomData<S>,
}

impl Client<Disconnected> {
    /// Create a new disconnected client
    pub fn new(config: Config) -> Self {
        Client {
            inner: Arc::new(ClientInner::new(config)),
            _state: PhantomData,
        }
    }
    
    /// Connect to the server - only valid from Disconnected state
    pub async fn connect(self) -> Result<Client<Ready>, Error> {
        self.inner.establish_connection().await?;
        Ok(Client {
            inner: self.inner,
            _state: PhantomData,
        })
    }
}

impl Client<Ready> {
    /// Execute a query - only valid from Ready state
    pub async fn query(&mut self, sql: &str) -> Result<QueryStream<'_>, Error> {
        // ...
    }
    
    /// Begin a transaction - consumes Ready, returns InTransaction
    pub async fn begin_transaction(self) -> Result<Transaction<'_>, Error> {
        self.inner.execute("BEGIN TRANSACTION").await?;
        Ok(Transaction {
            client: self,
        })
    }
    
    /// Execute without results
    pub async fn execute(&mut self, sql: &str) -> Result<u64, Error> {
        // ...
    }
}

/// Transaction with lifetime tied to client
pub struct Transaction<'a> {
    client: Client<InTransaction>,
    _lifetime: PhantomData<&'a ()>,
}

impl Transaction<'_> {
    pub async fn execute(&mut self, sql: &str) -> Result<u64, Error> {
        self.client.inner.execute(sql).await
    }
    
    pub async fn query(&mut self, sql: &str) -> Result<QueryStream<'_>, Error> {
        // ...
    }
    
    /// Commit - consumes Transaction, returns Ready client
    pub async fn commit(self) -> Result<Client<Ready>, Error> {
        self.client.inner.execute("COMMIT").await?;
        Ok(Client {
            inner: self.client.inner,
            _state: PhantomData,
        })
    }
    
    /// Rollback - consumes Transaction, returns Ready client
    pub async fn rollback(self) -> Result<Client<Ready>, Error> {
        self.client.inner.execute("ROLLBACK").await?;
        Ok(Client {
            inner: self.client.inner,
            _state: PhantomData,
        })
    }
    
    /// Create a savepoint within the current transaction.
    ///
    /// # Savepoint Name Requirements
    ///
    /// Savepoint names must be valid SQL Server identifiers:
    /// - Start with a letter (a-z, A-Z) or underscore (_)
    /// - Subsequent characters: letters, digits (0-9), underscore, @, #, $
    /// - Maximum length: 128 characters
    /// - Cannot be a reserved keyword
    ///
    /// The driver validates names against pattern `^[a-zA-Z_][a-zA-Z0-9_@#$]{0,127}$`
    /// and rejects invalid names with `Error::Config`.
    ///
    /// # Security Note
    ///
    /// Although savepoint names are typically developer-controlled constants,
    /// validation prevents bugs from becoming injection vulnerabilities if
    /// names are inadvertently derived from external input.
    ///
    /// # Example
    ///
    /// ```rust
    /// let tx = client.begin_transaction().await?;
    /// let sp = tx.save_point("before_items").await?;  // Valid
    /// // tx.save_point("invalid;name").await?;        // Error::Config
    /// ```
    pub async fn save_point(&mut self, name: &str) -> Result<SavePoint<'_>, Error> {
        validate_identifier(name)?;
        self.client.inner.execute(&format!("SAVE TRANSACTION {}", name)).await?;
        Ok(SavePoint { name: name.to_owned(), _tx: PhantomData })
    }

    /// Rollback to a previously created savepoint.
    ///
    /// Uses the validated name stored in the `SavePoint` struct.
    pub async fn rollback_to(&mut self, sp: &SavePoint<'_>) -> Result<(), Error> {
        // Safety: sp.name was validated when SavePoint was created
        self.client.inner.execute(&format!("ROLLBACK TRANSACTION {}", sp.name)).await
    }
}

/// Validates SQL Server identifier names (savepoints, temp tables, etc.)
///
/// Returns `Err(Error::Config)` if name is invalid per SQL Server identifier rules.
fn validate_identifier(name: &str) -> Result<(), Error> {
    use once_cell::sync::Lazy;
    use regex::Regex;

    static IDENTIFIER_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_@#$]{0,127}$").unwrap());

    if name.is_empty() {
        return Err(Error::Config("Identifier cannot be empty".into()));
    }

    if !IDENTIFIER_RE.is_match(name) {
        return Err(Error::Config(format!(
            "Invalid identifier '{}': must start with letter/underscore, \
             contain only alphanumerics/_/@/#/$, and be 1-128 characters",
            name
        ).into()));
    }

    Ok(())
}

// This is a compile error - cannot call begin_transaction on InTransaction state
// let tx = tx.begin_transaction(); // ERROR: method not found

// This is a compile error - cannot commit Ready client
// client.commit(); // ERROR: method not found
```

**Runtime State (Internal):**
```rust
/// Internal state for protocol handling
enum ProtocolState {
    /// Awaiting server response
    AwaitingResponse,
    /// Processing token stream
    ProcessingTokens,
    /// Draining after cancellation
    Draining,
    /// Connection is poisoned (protocol error occurred)
    Poisoned(Error),
}
```

#### Async Trait Implementation Note

With MSRV 1.88 (Rust 2024 Edition), native `async fn` in traits is stable. The driver uses native async traits where possible, reducing reliance on the `#[async_trait]` proc macro and its associated overhead:

```rust
// Native async trait (preferred - no macro overhead, better compiler errors)
pub trait ConnectionLifecycle {
    /// Check if the connection is healthy
    async fn health_check(&self) -> Result<(), Error>;
    /// Reset connection state for pool return
    async fn reset(&mut self) -> Result<(), Error>;
}

// #[async_trait] still required for dyn-compatibility (trait objects)
#[async_trait]
pub trait DynAuthProvider: Send + Sync {
    /// Acquire authentication token (may be async for OAuth flows)
    async fn acquire_token(&self) -> Result<Token, Error>;
}
```

**When to use each approach:**

| Scenario | Implementation | Rationale |
|----------|---------------|-----------|
| Concrete types only | Native `async fn` | Zero overhead, better errors |
| Trait objects (`dyn Trait`) | `#[async_trait]` | Object safety requires boxing |
| Public extension points | `#[async_trait]` | Users may need trait objects |
| Internal abstractions | Native `async fn` | Performance, simplicity |

**Note:** Native async traits in Rust 2024 have some limitations around `dyn` compatibility. Use `#[async_trait]` when the trait will be used with `Box<dyn Trait>` or `&dyn Trait`.

### 4.2 Error Handling Architecture

```rust
/// Top-level error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),
    
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    
    #[error("Server error {number}: {message}")]
    Server(#[from] ServerError),
    
    #[error("Authentication failed: {0}")]
    Auth(#[from] AuthError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("TCP connect timeout to {host}:{port}")]
    ConnectTimeout { host: String, port: u16 },

    #[error("TLS handshake timeout to {host}:{port}")]
    TlsTimeout { host: String, port: u16 },

    #[error("Login timeout to {host}:{port}")]
    LoginTimeout { host: String, port: u16 },

    #[error("Command timeout")]
    CommandTimeout,
    
    #[error("Pool exhausted")]
    PoolExhausted,
}

#[derive(Debug, thiserror::Error)]
#[error("Protocol error: {kind}")]
pub struct ProtocolError {
    pub kind: ProtocolErrorKind,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug)]
pub enum ProtocolErrorKind {
    UnexpectedToken { expected: &'static str, got: &'static str },
    InvalidPacketHeader,
    InvalidTokenType(u8),
    DeserializationFailed,
    UnexpectedEof,
}

#[derive(Debug)]
pub struct ServerError {
    pub number: i32,
    pub state: u8,
    pub class: u8,
    pub message: String,
    pub server: Option<String>,
    pub procedure: Option<String>,
    pub line: i32,
}

impl Error {
    /// Check if this error is transient and may succeed on retry
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset |
                std::io::ErrorKind::ConnectionAborted |
                std::io::ErrorKind::TimedOut
            ),
            Error::Server(e) => e.is_transient(),
            Error::ConnectTimeout { .. } => true,
            Error::TlsTimeout { .. } => true,
            Error::LoginTimeout { .. } => true,
            Error::CommandTimeout => true,
            Error::PoolExhausted => true,
            _ => false,
        }
    }
    
    /// Check if this error indicates a driver bug
    pub fn is_protocol_error(&self) -> bool {
        matches!(self, Error::Protocol(_))
    }
}
```

### 4.3 Data Flow Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            Data Flow Pipeline                            │
└─────────────────────────────────────────────────────────────────────────┘

1. INGRESS (Network Layer)
   ┌─────────────┐
   │  TcpStream  │ ─── Raw bytes from network
   └──────┬──────┘
          │
          ▼
2. TLS LAYER (mssql-tls)
   ┌─────────────────────────────────────────────────────────────────────┐
   │  TDS 7.x: TCP → Prelogin → TLS Handshake → Login7                   │
   │  TDS 8.0: TCP → TLS Handshake → Prelogin → Login7 (all encrypted)   │
   └──────┬──────────────────────────────────────────────────────────────┘
          │
          ▼
3. FRAMING (mssql-codec)
   ┌─────────────────────────────────────────────────────────────────────┐
   │  • Read TDS packet headers (8 bytes)                                 │
   │  • Handle packet reassembly (split across TCP segments)             │
   │  • Handle packet continuation (large packets → multiple TDS packets)│
   │  • Yield complete PacketData                                         │
   └──────┬──────────────────────────────────────────────────────────────┘
          │
          ▼
4. TOKENIZATION (tds-protocol)
   ┌─────────────────────────────────────────────────────────────────────┐
   │  • Parse packet body into Token stream                              │
   │  • Token types: ColMetaData, Row, Done, Error, Info, EnvChange...  │
   │  • Lazy parsing - tokens yielded as iterator                        │
   └──────┬──────────────────────────────────────────────────────────────┘
          │
          ▼
5. BINDING (mssql-client)
   ┌─────────────────────────────────────────────────────────────────────┐
   │  • Consume tokens, update client state (database, transaction)      │
   │  • Map Row tokens to Row structs with Arc<Bytes> buffer             │
   │  • Yield user-facing Row objects                                    │
   └──────┬──────────────────────────────────────────────────────────────┘
          │
          ▼
6. USER CODE
   ┌─────────────┐
   │  while let  │
   │  Some(row)  │ ─── User processes rows
   │  = ...      │
   └─────────────┘
```

### 4.4 Timeout Configuration

```rust
pub struct TimeoutConfig {
    /// Time to establish TCP connection
    pub connect_timeout: Duration,
    
    /// Time to complete TLS handshake
    pub tls_timeout: Duration,
    
    /// Time to complete login sequence
    pub login_timeout: Duration,
    
    /// Default timeout for command execution
    pub command_timeout: Duration,
    
    /// Time before idle connection is closed
    pub idle_timeout: Duration,
    
    /// Interval for connection keep-alive
    pub keepalive_interval: Option<Duration>,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(15),
            tls_timeout: Duration::from_secs(10),
            login_timeout: Duration::from_secs(30),
            command_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            keepalive_interval: Some(Duration::from_secs(30)),
        }
    }
}
```

### 4.5 Prepared Statement Lifecycle

> **Implementation status:** the client-side `StatementCache` is wired into the
> buffered `query` path but is **opt-in, off by default** — enabled via
> `Statement Cache=true` / `Config::with_statement_cache(true)`. When disabled
> (the default), parameterized queries go through `sp_executesql` (SQL Server's
> server-side plan cache still provides plan reuse). See LIMITATIONS.md §
> Prepared Statement Cache for the wired scope and what remains on
> `sp_executesql`.

SQL Server supports server-side prepared statements via RPC. When the opt-in
statement cache is enabled (see the status note above), the driver manages
handles transparently:

- **Cold miss:** `sp_prepexec` prepares and executes in a **single** round-trip
  and returns the handle (the one-round-trip path from #337 — not a separate
  `sp_prepare` then `sp_execute`).
- **Hit:** `sp_execute` reuses the cached handle, skipping the re-parse.
- **Eviction:** LRU eviction best-effort `sp_unprepare`s the evicted handle.
- **Connection reset (`sp_reset_connection`):** the server invalidates every
  handle for the session, so the client clears its cache.

The concrete cached types, the config flag, and what still runs on
`sp_executesql` are documented in
[LIMITATIONS.md § Prepared Statement Cache](LIMITATIONS.md); the implementation
is `Client::send_query_request` and `StatementCache` in
`crates/mssql-client/src/`.

**Design Note:** cross-connection statement sharing is explicitly **not
supported** — handles are session-scoped, and sharing them across connections
would create invalidation races and session-affinity problems.

---

### 4.6 Azure SQL Routing and Failover

Azure SQL Database and certain on-premises configurations may redirect connections during the login phase via the `ENVCHANGE` token with routing information.

#### Redirect Scenarios

| Scenario | Trigger | Behavior |
|----------|---------|----------|
| Azure SQL Gateway | Initial connection to `*.database.windows.net` | Redirect to actual compute node |
| Geo-Replication Failover | Primary region failure | Redirect to geo-secondary |
| Load Balancing | Connection distribution | Redirect to available node |
| Planned Maintenance | Failover event | Redirect to partner replica |

#### Protocol Handling

The `ENVCHANGE` token type `Routing` (0x14) contains redirect information:

```rust
/// Routing information received from server during login
pub struct RoutingData {
    /// Target server hostname
    pub host: String,
    /// Target port (typically 1433, or 11000-11999 for Azure internal nodes)
    pub port: u16,
}

/// Error variant for routing redirects
#[derive(Debug)]
pub enum Error {
    // ... other variants ...

    /// Server requested connection redirect (Azure SQL gateway behavior)
    #[error("Routing redirect to {host}:{port}")]
    Routing { host: String, port: u16 },
}
```

#### Connection Flow with Redirect

```
Client                     Gateway                    Actual Node
  │                          │                            │
  ├─── TCP Connect ─────────►│                            │
  ├─── Prelogin ────────────►│                            │
  │◄── Prelogin Response ────┤                            │
  ├─── Login7 ──────────────►│                            │
  │◄── ENVCHANGE(Routing) ───┤                            │
  │    {host, port}          │                            │
  ├─── TCP Close ───────────►│                            │
  │                                                       │
  ├─── TCP Connect ──────────────────────────────────────►│
  ├─── Prelogin ─────────────────────────────────────────►│
  │◄── Prelogin Response ─────────────────────────────────┤
  ├─── Login7 ───────────────────────────────────────────►│
  │◄── Login Response (success) ──────────────────────────┤
```

#### Implementation

```rust
impl Client<Disconnected> {
    /// Connect to SQL Server, automatically following redirects.
    ///
    /// Azure SQL Database connections through the gateway will typically
    /// receive a redirect to an internal node. This is handled transparently.
    pub async fn connect(config: Config) -> Result<Client<Ready>, Error> {
        const MAX_REDIRECT_ATTEMPTS: u8 = 2;

        let mut attempts = 0;
        let mut current_config = config;

        loop {
            attempts += 1;
            if attempts > MAX_REDIRECT_ATTEMPTS {
                return Err(Error::TooManyRedirects {
                    max: MAX_REDIRECT_ATTEMPTS,
                });
            }

            match Self::try_connect(&current_config).await {
                Ok(client) => return Ok(client),
                Err(Error::Routing { host, port }) => {
                    tracing::info!(
                        target: "mssql::connection",
                        "Server requested redirect to {}:{} (attempt {}/{})",
                        host, port, attempts, MAX_REDIRECT_ATTEMPTS
                    );
                    current_config = current_config.with_host(&host).with_port(port);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

#### Configuration

```rust
pub struct RedirectConfig {
    /// Maximum redirect attempts before failing (default: 2)
    pub max_redirects: u8,
    /// Follow redirects to different hosts (default: true)
    ///
    /// Set to `false` to fail on redirect instead of following.
    /// Useful for security-sensitive environments that require
    /// explicit connection targets.
    pub follow_redirects: bool,
}

impl Default for RedirectConfig {
    fn default() -> Self {
        Self {
            max_redirects: 2,
            follow_redirects: true,
        }
    }
}
```

#### Security Considerations

- **Redirect Timing:** Redirects are only honored during the initial login phase, never mid-session
- **TLS Re-negotiation:** A new TLS handshake is performed with the redirect target
- **Certificate Validation:** The redirect target's certificate must pass validation against the configured trust store
- **Hostname Verification:** If `TrustServerCertificate=false`, the redirect target's hostname must match its certificate
- **Trust Settings:** `TrustServerCertificate` applies to both the original gateway and redirect targets

**Azure-Specific Note:** Connections to Azure SQL Database through the gateway (`*.database.windows.net`) will almost always receive a redirect to an internal compute node (`*.database.windows.net` on port 11000-11999). This is expected behavior and does not indicate a security issue.

### 4.7 Stored Procedure Execution

Stored procedures are called via TDS RPC (Remote Procedure Call) requests using `RpcRequest::named()`. Unlike `sp_executesql` (used for parameterized queries), named RPC calls directly invoke the stored procedure on the server without SQL text parsing.

#### Two-Tier API

| Method | Use Case | Parameters |
|--------|----------|------------|
| `call_procedure(name, &[params])` | Simple input-only calls | Positional, auto-named `@p1`, `@p2`, ... |
| `procedure(name)?.input().output_*().execute()` | Named params, OUTPUT params | Named by caller, output types declared |

Both methods are available on `impl<S: ConnectionState> Client<S>`, meaning they work identically in `Ready` and `InTransaction` states with zero code duplication.

#### TDS Response Token Flow

The server responds to an RPC procedure call with the following token sequence:

```
[COLMETADATA → ROW(s) → DONEINPROC]    ← per SELECT statement in the proc
RETURNVALUE(s)                          ← one per OUTPUT parameter
RETURNSTATUS                            ← procedure RETURN value (i32)
DONEPROC                                ← final token
```

The `read_procedure_result()` parser is order-tolerant and accumulative — it handles these tokens in any order and collects them into a `ProcedureResult`:

| Token | Action |
|-------|--------|
| `ColMetaData` | Start new result set (save previous if non-empty) |
| `Row` / `NbcRow` | Parse via `convert_raw_row()`/`convert_nbc_row()`, add to current result set |
| `DoneInProc` | Save current result set, accumulate `rows_affected` |
| `ReturnValue` | Decode via `parse_column_value()` using `ColumnData` bridge, push as `OutputParam` |
| `ReturnStatus` | Store as `return_value: i32` |
| `DoneProc` | Save remaining result set, break if `!more` |

#### ReturnValue Decoding

`ReturnValue` tokens carry the output parameter value in TYPE_VARBYTE format (the same format used for row column values). To decode, we construct a temporary `ColumnData` from the `ReturnValue`'s `col_type`, `flags`, `user_type`, and `type_info` fields, then call the existing `parse_column_value()` function. This reuses the full type decoding machinery without duplication.

#### Security

All procedure names are validated via `validate_qualified_identifier()` before being sent to the server. This prevents SQL injection through procedure name manipulation. Parameter values are sent as typed RPC parameters (never interpolated into SQL text).

---

## 5. Security Architecture

### 5.1 TLS Configuration

**Default Configuration (Secure):**
```rust
pub fn default_tls_config() -> ClientConfig {
    ClientConfig::builder()
        .with_root_certificates(webpki_roots::TLS_SERVER_ROOTS.clone())
        .with_no_client_auth()
}
```

**Supported TLS Versions:**
- TLS 1.2 (TDS 7.x with `Encrypt=true`)
- TLS 1.3 (TDS 8.0 with `Encrypt=strict`)

**Certificate Validation:**
- Hostname verification enabled by default
- `TrustServerCertificate=true` disables validation (development only, logs warning)
- Custom CA certificates supported via configuration

### 5.2 Credential Handling

See [SECURITY.md § Credential Handling](SECURITY.md): credentials are never
logged (even at trace level), passwords are zeroized via `zeroize`, access
tokens live in a redacting `SecretString`, and connection strings are redacted
in error messages.

### 5.3 SQL Injection Prevention

See [SECURITY.md](SECURITY.md): user values are always bound as parameters and
sent via the RPC protocol; there is no API for raw SQL string interpolation.

---

## 6. Development Standards & Tooling

### 6.1 Required Tooling

| Tool | Purpose | Required For |
|------|---------|--------------|
| `cargo-nextest` | Parallel test execution | CI, local testing |
| `cargo-deny` | License/duplicate dep checks | CI |
| `cargo-hakari` | Workspace-hack for feature unification | Development |
| `cargo-fuzz` | Fuzz testing | Security testing |
| `cargo-miri` | Undefined behavior detection | `unsafe` validation |
| `cargo-semver-checks` | API stability verification | Releases |
| `criterion` | Benchmarking | Performance validation |
| `cargo-llvm-cov` | Code coverage | CI |

#### cargo-deny and cargo-hakari Interaction

The workspace-hack crate generated by `cargo-hakari` may trigger duplicate dependency warnings in `cargo-deny`. Configure `deny.toml` to skip the workspace-hack crate:

```toml
# deny.toml

[bans]
multiple-versions = "warn"

# Skip workspace-hack crate for duplicate checks (auto-generated by cargo-hakari)
# The workspace-hack intentionally unifies features across the workspace, which
# may result in apparent "duplicates" that are actually intentional.
skip = [
    { crate = "workspace-hack@*", reason = "auto-generated by cargo-hakari" },
]
```

**Reference:** [cargo-deny issue #704](https://github.com/EmbarkStudios/cargo-deny/issues/704)

### 6.2 CI Pipeline

The CI pipeline is defined in [`.github/workflows/`](.github/workflows/) and
mirrored locally by `just ci-all`. The full gate list (cross-platform matrix,
live integration across SQL Server 2017/2019/2022, Miri, fuzzing,
public-API/semver, supply-chain, and the commit-hygiene gates) is documented in
CLAUDE.md. It is not reproduced here to avoid drift.

### 6.3 Fuzz Testing

Fuzz targets live in [`fuzz/fuzz_targets/`](fuzz/) (wire parsers, token stream,
connection-string parser). They run per-PR as a smoke job and on a nightly
long-budget schedule (`fuzz-nightly.yml`).

### 6.4 Benchmarking

Criterion benches live in `benches/`; performance-regression detection runs via
`benchmarks.yml`.

### 6.5 Documentation Standards

See [CONTRIBUTING.md § Documentation](CONTRIBUTING.md). All public items carry
doc comments with `# Errors` / `# Panics` sections where applicable, enforced by
`cargo doc -D warnings` in CI.

### 6.6 MSRV Policy

See [STABILITY.md § MSRV Increase Policy](STABILITY.md), which is authoritative
(MSRV bumps are NOT breaking changes). The current MSRV is pinned via
`rust-version` in `Cargo.toml` and verified by the `msrv` CI job.

### 6.7 Deprecation Strategy

See [STABILITY.md § Deprecation Policy](STABILITY.md), which is authoritative:
items are marked with `#[deprecated]`, kept for at least one minor release, and
removed in a subsequent breaking release with the migration path recorded in the
CHANGELOG.

---

## 7. Implementation Roadmap

Completed milestones are recorded in [CHANGELOG.md](CHANGELOG.md). Known
gaps and planned work toward 1.0 are tracked in [LIMITATIONS.md](LIMITATIONS.md).

---

## 8. Appendices

### 8.1 TDS Protocol Reference

**Official Documentation:**
- [MS-TDS: Tabular Data Stream Protocol](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/)
- [TDS 8.0 Overview](https://learn.microsoft.com/en-us/sql/relational-databases/security/networking/tds-8)

**Key Protocol Versions:**

| TDS Version | SQL Server Version | Key Features |
|-------------|-------------------|--------------|
| 7.4 | 2012+ | Default for modern deployments |
| 7.4 Rev 1 | 2016+ | UTF-8 support |
| 8.0 | 2022+ | TLS-first, strict encryption |

### 8.2 SQL Server Version Compatibility

| SQL Server Version | Supported | Notes |
|-------------------|-----------|-------|
| 2008 | Yes | TDS 7.3A, requires `Encrypt=no_tls` |
| 2008 R2 | Yes | TDS 7.3B, requires `Encrypt=no_tls` |
| 2012 | Yes | TDS 7.4, requires `Encrypt=no_tls` |
| 2014 | Yes | TDS 7.4, requires `Encrypt=no_tls` |
| 2016 | Yes | TDS 7.4, requires `Encrypt=no_tls` |
| 2017 | Yes | TDS 7.4 |
| 2019 | Yes | TDS 7.4 |
| 2022 | Yes | TDS 7.4/8.0 |
| 2025 | Yes | TDS 7.4/8.0, Managed Identity enhancements |
| Azure SQL Database | Yes | All authentication methods |
| Azure SQL Managed Instance | Yes | All authentication methods |

**How each version is validated:** the **Supported** column reflects protocol
capability, not test cadence. **SQL Server 2017, 2019, and 2022 are CI-verified**
— the integration suite runs the full ignored test suite against all three on
every change. SQL Server 2008–2016 and Azure SQL are validated manually against
real servers, not in CI. SQL Server 2025 is forward-looking — protocol-compatible
but not yet validated.

### 8.3 Feature Flag Matrix

Features are defined on `mssql-client` and forwarded to internal crates as needed.

| Feature | Default | Crate(s) Activated | Platform | Dependencies Added |
|---------|---------|-------------------|----------|-------------------|
| `chrono` | Yes | mssql-types | All | `chrono` |
| `uuid` | Yes | mssql-types | All | `uuid` |
| `decimal` | Yes | mssql-types | All | `rust_decimal` |
| `encoding` | Yes | tds-protocol, mssql-types | All | `encoding_rs` |
| `tls` | Yes | mssql-tls | All | `rustls`, `tokio-rustls` |
| `json` | No | mssql-types | All | `serde_json` |
| `otel` | No | (local to mssql-client) | All | `opentelemetry`, `tracing-opentelemetry` |
| `zeroize` | No | mssql-auth | All | `zeroize` |
| `always-encrypted` | No | mssql-auth | All | `aes`, `cbc`, `hmac`, `sha2`, `rsa`, `rand` |
| `azure-identity` | No | mssql-auth | All | `azure_identity` (pulls OpenSSL transitively) |
| `integrated-auth` | No | mssql-auth | Linux/macOS | `gssapi`, `libkrb5-dev` |
| `sspi-auth` | No | mssql-auth | Windows | `sspi-rs` |
| `cert-auth` | No | mssql-auth | All | `azure_identity/client_certificate` (pulls OpenSSL transitively) |

Features on `mssql-auth` only (not exposed via `mssql-client`):

| Feature | Dependencies | Notes |
|---------|-------------|-------|
| `azure-keyvault` | `azure_security_keyvault_keys` | Requires `always-encrypted` + `azure-identity` |
| `windows-certstore` | Windows Crypto API (FFI) | Requires `always-encrypted`, Windows only |

### 8.4 Migration Guide from Tiberius

See [MIGRATION.md](MIGRATION.md) for the
full migration guide — connection setup, query execution, transactions,
connection pooling, Azure SQL routing, error handling, type mappings, and a
quick-reference cheat sheet.

### 8.5 Glossary

| Term | Definition |
|------|------------|
| **TDS** | Tabular Data Stream - Microsoft's protocol for SQL Server communication |
| **MARS** | Multiple Active Result Sets - ability to have multiple pending requests on a single connection |
| **PLP** | Partially Length Prefixed - encoding for large objects allowing streaming |
| **Prelogin** | Initial TDS packet exchanged before authentication |
| **Login7** | Authentication packet format (version 7.0+) |
| **Attention** | Cancel signal sent out-of-band to abort running query |
| **sp_reset_connection** | Stored procedure to reset connection state in pools |
| **TVP** | Table-Valued Parameter - passing table data as a parameter |
| **BCP** | Bulk Copy Protocol - high-speed data loading |

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-11 | Initial specification |
| 1.1.0 | 2025-12-11 | Security guidance corrections (ADR-013 Always Encrypted), savepoint validation, prepared statement lifecycle (§4.5), Azure SQL routing (§4.6), OpenTelemetry 0.31, version constraint policy, cargo-deny/hakari integration, native async trait guidance, migration guide updates |
| 1.2.0 | 2025-12-24 | Updated for v0.2.0 release: Phase 5 auth complete, ADR-013 status updated (cryptography implemented), feature flag matrix expanded, v0.2.0 delivered features documented, v0.3.0 roadmap updated |
| 1.3.0 | 2025-12-25 | Updated for v0.3.0 release: Always Encrypted key providers (InMemoryKeyStore, KeyStoreProvider trait), true LOB streaming (LobStream), Change Tracking integration, all 12 data type parsing fixes complete |
| 1.4.0 | 2025-12-31 | Updated for v0.4.0 release: TDS 7.3 protocol support (SQL Server 2008/2008 R2), TdsVersion configuration, version negotiation |
| 1.5.0 | 2026-01-01 | Updated for v0.5.0 release: Collation-aware VARCHAR decoding, encoding feature, Column marked non_exhaustive |
| 1.6.0 | 2026-04-07 | Updated for v0.7.0 release: MSRV bumped to 1.88, SSPI integrated auth wired into client login, RUSTSEC advisories resolved, 33 public enums marked non_exhaustive for semver safety, deprecated APIs removed before 1.0 |
| 1.7.0 | 2026-04-13 | Updated for v0.8.0 release: Stored procedure support (§4.7), SQL Browser instance resolution, pool test_on_checkin, Azure SDK 0.34, mock TLS cross-platform fix |
| 1.8.0 | 2026-06-11 | Doc-accuracy corrections (#166): header version aligned with history; ADR-002 marks `Authentication` keyword as recognized-but-not-yet-supported (FEDAUTH pending, #155); ADR-013 clarifies Always Encrypted is read-path only (write path NULL-only); §8.3 corrects `cert-auth` dependency (pulls OpenSSL via azure_identity, not pure-rustls) |
| 1.9.0 | 2026-06-12 | #155 Phase 1: Azure AD / Entra logins via the FEDAUTH SecurityToken workflow (pre-acquired token, managed identity, service principal); ADR-002 `Authentication` keyword supported for SqlPassword / ActiveDirectoryServicePrincipal / ActiveDirectoryManagedIdentity; `azure-identity` feature added to mssql-client |

---

*This document is the authoritative architectural reference for the mssql-driver project. All implementation decisions should align with the principles and specifications herein.*

