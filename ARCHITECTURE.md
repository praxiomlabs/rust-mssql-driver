# Architectural Reference: High-Performance Rust MS SQL Driver

**Version:** 1.5.0
**Status:** Design Complete (v0.5.0 Released)
**Target Protocol:** MS-TDS 7.3 – 8.0 (SQL Server 2008 – 2025)
**Toolchain Standard:** Rust 2024 Edition (v1.85+, released February 20, 2025)
**MSRV Policy:** Rust 1.85.0 (6-month rolling window)

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

1. **Tokio-Native:** Unapologetically built on Tokio 1.48+ to leverage the standard Rust async runtime ecosystem without compatibility shims or abstraction overhead.

2. **TDS 8.0 First-Class:** Designed for the strict TLS 1.3 architecture of SQL Server 2022+ where TLS handshake precedes all TDS messages, while maintaining support for legacy TDS 7.4 encryption negotiation.

3. **Minimal Allocation Churn:** Reduces memory allocation pressure through strategic use of reference counting (`Arc<Bytes>`) and buffer slicing. Note: This is *reduced*-copy rather than true zero-copy, which would require arena allocation or self-referential structures.

4. **Safety by Design:** Utilizes Rust's type system to render invalid protocol states uncompilable through genuine compile-time type-state patterns.

5. **Modern Workspace Standards:** Adopts a flat workspace layout, centralized dependency management, and eliminates legacy module patterns (`mod.rs`).

### 1.2 Competitive Positioning

| Driver | Weakness This Project Addresses |
|--------|--------------------------------|
| `tiberius` | Runtime-agnostic design compromises, TDS 8.0 as afterthought, `tokio_util::compat` overhead |
| `odbc` crate | FFI overhead, deployment complexity, platform-specific installation |
| Native `FreeTDS` | No async support, C memory safety concerns, manual resource management |

### 1.3 Non-Goals (Explicit Exclusions)

The following are explicitly out of scope for v1.0:

- **Runtime Agnosticism:** No `async-std`, `smol`, or generic executor support
- **MARS (Multiple Active Result Sets):** Deferred to v2.0 if demand warrants
- **Named Pipe Transport:** Windows-only, limited use case
- **Shared Memory Protocol:** Undocumented, no existing Rust implementations
- **SQL Server 2005 and Earlier:** TDS 7.2 and earlier protocols are not supported

### 1.4 TDS Protocol Version Support

| TDS Version | SQL Server Version | Status |
|-------------|-------------------|--------|
| TDS 7.3A | SQL Server 2008 | Supported via `TdsVersion::V7_3A` |
| TDS 7.3B | SQL Server 2008 R2 | Supported via `TdsVersion::V7_3B` |
| TDS 7.4 | SQL Server 2012+ | Default, full support |
| TDS 8.0 | SQL Server 2022+ | Full support (strict TLS mode) |

The driver defaults to TDS 7.4 for maximum compatibility with modern SQL Server while supporting legacy TDS 7.3 connections for enterprise environments with SQL Server 2008/2008 R2 instances.

---

## 2. System Architecture

The project follows a **Flat Workspace** layout. The root `Cargo.toml` is a virtual manifest; all code resides in `crates/`. This structure enforces strict boundaries and reduces compilation times.

### 2.1 Crate Topology

```
mssql-driver/
├── Cargo.toml                    # Virtual manifest
├── rust-toolchain.toml           # Pin to 1.85+
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
                    ┌─────────────────┐
                    │  mssql-client   │ ← Public API
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│  mssql-pool   │   │  mssql-auth   │   │ mssql-derive  │
└───────┬───────┘   └───────┬───────┘   └───────────────┘
        │                   │
        └─────────┬─────────┘
                  │
                  ▼
          ┌───────────────┐
          │  mssql-codec  │
          └───────┬───────┘
                  │
        ┌─────────┴─────────┐
        │                   │
        ▼                   ▼
┌───────────────┐   ┌───────────────┐
│   mssql-tls   │   │  mssql-types  │
└───────┬───────┘   └───────────────┘
        │
        ▼
┌───────────────┐
│ tds-protocol  │ ← no_std compatible
└───────────────┘
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

**Dependencies:** `rustls`, `webpki-roots`, `tokio-rustls`, `tds-protocol`

**Rationale:** TDS TLS negotiation is sufficiently complex to warrant isolation. TDS 8.0 fundamentally changes the handshake order (TCP → TLS → TDS vs. TCP → TDS prelogin → TLS → TDS), and this crate encapsulates that complexity.

#### `crates/mssql-codec`

**Responsibility:** Async framing layer. Transforms `AsyncRead`/`AsyncWrite` byte streams into high-level `Packet` structures.

**Key Features:**
- Packet reassembly across TCP segment boundaries
- Packet continuation handling (large packets split across multiple TDS packets)
- IO splitting for cancellation safety (see ADR-004)

**Dependencies:** `tds-protocol`, `mssql-tls`, `tokio-util` (Codec), `bytes`, `tokio`

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
| Certificate-based | Pure Rust | `cert-auth` |
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

**Responsibility:** Build automation replacing `Makefile`/`bash` scripts.

**Commands:**
```bash
cargo xtask dist       # Build release artifacts
cargo xtask test       # Run nextest with filtering
cargo xtask fuzz       # Run fuzz tests
cargo xtask bench      # Run benchmarks
cargo xtask codegen    # Generate protocol constants from spec
cargo xtask ci         # Full CI pipeline
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
| `Authentication` | | `SqlPassword`, `ActiveDirectoryPassword`, `ActiveDirectoryManagedIdentity`, `ActiveDirectoryServicePrincipal` |
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
let mut results = client.query("EXEC sp_help 'Users'").await?;

// First result set
while let Some(row) = results.next().await? {
    // Process row
}

// Advance to next result set
if results.next_result_set().await? {
    while let Some(row) = results.next().await? {
        // Process next result set
    }
}
```

**Resolution:** Concurrent queries must use separate connections via `mssql-pool`. MARS may be considered for v2.0 based on user demand.

---

### ADR-007: Streaming vs. Buffered Results

**Status:** Accepted

**Decision:** Results are streamed by default with optional buffering.

**Streaming (Default):**
```rust
let mut rows = client.query("SELECT * FROM large_table").await?;
while let Some(row) = rows.next().await? {
    // Process row - memory usage stays constant
}
```

**Buffered (Explicit):**
```rust
let rows: Vec<Row> = client
    .query("SELECT * FROM small_table")
    .await?
    .collect()
    .await?;
```

**Large Object Streaming:**
```rust
let mut stream = client
    .query("SELECT blob_column FROM documents WHERE id = @p1")
    .bind(doc_id)
    .await?;

if let Some(row) = stream.next().await? {
    let mut blob_reader = row.get_stream::<BlobReader>(0)?;
    tokio::io::copy(&mut blob_reader, &mut file).await?;
}
```

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
    .begin_transaction()
    .isolation_level(IsolationLevel::Serializable)
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

**Implementation:**

```toml
# Root Cargo.toml
[workspace]
resolver = "2"
members = ["crates/*", "xtask"]

[workspace.package]
version = "0.5.1"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/praxiomlabs/rust-mssql-driver"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.48", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }
tokio-rustls = "0.26"

# Data handling
bytes = "1.9"
chrono = { version = "0.4", default-features = false, features = ["std"] }
uuid = { version = "1.11", features = ["v4"] }
rust_decimal = "1.36"
serde_json = "1.0"

# TLS
rustls = { version = "0.23", default-features = false, features = ["std", "tls12", "ring"] }
webpki-roots = "1.0"

# Error handling
thiserror = "2.0"

# Observability (optional `otel` feature)
opentelemetry = { version = "0.31", optional = true }
opentelemetry_sdk = { version = "0.31", optional = true }
opentelemetry-otlp = { version = "0.31", optional = true }
tracing-opentelemetry = { version = "0.32", optional = true }

# Testing
criterion = { version = "0.7", features = ["async_tokio"] }
proptest = "1.5"
testcontainers = "0.25"

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
opentelemetry = "0.31"   # Allows 0.31.x updates

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

**Decision:** First-class support for bulk insert operations using the TDS Bulk Load protocol.

**API:**
```rust
let bulk = client
    .bulk_insert("dbo.Users")
    .with_columns(&["id", "name", "email"])
    .with_options(BulkOptions {
        batch_size: 1000,
        check_constraints: true,
        fire_triggers: false,
        keep_nulls: true,
        table_lock: true,
    })
    .build()
    .await?;

// Stream rows
for user in users {
    bulk.send_row(&[&user.id, &user.name, &user.email]).await?;
}

let result = bulk.finish().await?;
println!("Inserted {} rows", result.rows_affected);
```

**Streaming from CSV:**
```rust
let file = File::open("users.csv").await?;
let reader = csv_async::AsyncReader::from_reader(file);

let bulk = client.bulk_insert("dbo.Users").build().await?;
bulk.send_stream(reader).await?;
let result = bulk.finish().await?;
```

---

### ADR-013: Always Encrypted Support

**Status:** Implemented ✅

**Decision:** Always Encrypted client-side encryption is fully implemented with production-ready key providers.

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
| Malicious DBA | ✅ Protected | ❌ Vulnerable |
| Server Compromise | ✅ Protected | ❌ Vulnerable |
| Cloud Operator Access | ✅ Protected | ❌ Vulnerable |
| Keys Stored On Server | ❌ Never | ✅ Yes |

**⚠️ Important:** T-SQL encryption functions (`ENCRYPTBYKEY`/`DECRYPTBYKEY`) provide **server-side encryption** where keys exist within SQL Server. They do **NOT** provide the same security guarantees as Always Encrypted and should not be considered a substitute.

**Available Key Providers:**
- **For development/testing:** Use the `InMemoryKeyStore` with the `always-encrypted` feature
- **For Azure Key Vault:** Use `AzureKeyVaultProvider` with the `azure-keyvault` feature
- **For Windows Certificate Store:** Use `WindowsCertStoreProvider` with the `windows-certstore` feature
- **For custom key storage:** Implement the `KeyStoreProvider` trait for your key management solution

**⚠️ Do NOT use `ENCRYPTBYKEY`** as a workaround - it does not provide the same security guarantees

**References:**
- [Always Encrypted Overview](https://learn.microsoft.com/en-us/sql/relational-databases/security/encryption/always-encrypted-database-engine)
- [Key Management for Always Encrypted](https://learn.microsoft.com/en-us/sql/relational-databases/security/encryption/overview-of-key-management-for-always-encrypted)

---

### ADR-014: OpenTelemetry Instrumentation

**Status:** Accepted

**Decision:** First-class OpenTelemetry tracing support via optional feature flag.

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
let client = Client::connect_with_tracing(config, tracer).await?;

// Spans automatically created
client.query("SELECT * FROM users WHERE id = @p1")
    .bind(user_id)
    .await?;
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

With MSRV 1.85 (Rust 2024 Edition), native `async fn` in traits is stable. The driver uses native async traits where possible, reducing reliance on the `#[async_trait]` proc macro and its associated overhead:

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
    
    #[error("Connection timeout")]
    ConnectionTimeout,
    
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
            Error::ConnectionTimeout => true,
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

SQL Server supports server-side prepared statements via RPC calls. The driver manages statement handles transparently to optimize repeated query execution.

#### Protocol Flow

```
Client                              Server
  │                                   │
  ├──── sp_prepare(sql) ─────────────►│
  │◄─── handle (int32) ───────────────┤
  │                                   │
  ├──── sp_execute(handle, params) ──►│  (repeatable)
  │◄─── results ──────────────────────┤
  │                                   │
  ├──── sp_unprepare(handle) ────────►│
  │◄─── done ─────────────────────────┤
```

#### Handle Management

**Statement Cache:**
```rust
pub struct PreparedStatement {
    /// Server-assigned handle for this prepared statement
    handle: i32,
    /// Hash of the SQL text for cache lookup
    sql_hash: u64,
    /// Parameter metadata from sp_describe_parameter_encryption
    param_metadata: Arc<ParamMetaData>,
    /// Timestamp for optional TTL-based eviction
    created_at: Instant,
}

pub struct StatementCache {
    /// LRU cache of prepared statements keyed by SQL hash
    cache: LruCache<u64, PreparedStatement>,
    /// Maximum cached statements per connection
    max_size: usize,
}
```

**Lifecycle Rules:**

1. **Preparation:** First execution of a parameterized query calls `sp_prepare`, which returns a handle
2. **Caching:** Handle is cached by SQL hash; subsequent executions use `sp_execute` with the cached handle
3. **Eviction:** LRU eviction calls `sp_unprepare` for evicted handles to release server resources
4. **Connection Return:** Pool reset (`sp_reset_connection`) invalidates all server-side handles
5. **Connection Close:** Handles are implicitly released by the server

**Configuration:**
```rust
pub struct StatementCacheConfig {
    /// Enable statement caching (default: true)
    pub enabled: bool,
    /// Maximum statements per connection (default: 100)
    pub max_statements: usize,
    /// TTL before re-preparation (default: None - no expiry)
    pub ttl: Option<Duration>,
}

impl Default for StatementCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_statements: 100,
            ttl: None,
        }
    }
}
```

#### Pool Interaction

When a connection is returned to the pool:

1. `sp_reset_connection` is called (per §2.3 mssql-pool specification)
2. Server invalidates all prepared statement handles for that session
3. Client clears its local statement cache
4. Next use re-prepares statements on demand (cache miss)

**Design Note:** This design prioritizes correctness over maximum cache hit rate. Cross-connection statement sharing is explicitly **not supported** to avoid handle invalidation race conditions and session affinity issues.

#### Usage Example

```rust
// Automatic statement caching (default behavior)
for user_id in user_ids {
    // First iteration: sp_prepare + sp_execute
    // Subsequent iterations: sp_execute only (cache hit)
    let row = client.query("SELECT name FROM users WHERE id = @p1")
        .bind(user_id)
        .await?
        .next()
        .await?;
}

// Explicit prepared statement (for fine-grained control)
let stmt = client.prepare("SELECT name FROM users WHERE id = @p1").await?;
for user_id in user_ids {
    let row = stmt.query().bind(user_id).await?.next().await?;
}
// stmt dropped: sp_unprepare called automatically
```

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

**Principles:**
- Credentials never logged, even at trace level
- Passwords zeroized after use via `zeroize` crate
- Access tokens stored in `SecretString` wrapper
- Connection strings redacted in error messages

```rust
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    username: String,
    password: String,
}

pub struct SecretString(String);

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}
```

### 5.3 SQL Injection Prevention

**Parameterized Queries (Required):**
```rust
// GOOD - Parameterized
client.query("SELECT * FROM users WHERE id = @p1")
    .bind(user_id)
    .await?;

// BAD - String interpolation (no API support for this pattern)
// client.query(format!("SELECT * FROM users WHERE id = {}", user_id))
```

**Parameter Binding:**
- All user values must be bound via `.bind()` method
- No API for raw SQL string execution with interpolation
- Parameters sent via RPC protocol, never interpolated into SQL text

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

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@nextest
      - run: cargo nextest run --all-features
      
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --check
      - run: cargo clippy --all-features -- -D warnings
      
  deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v1
      
  miri:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri
      - run: cargo miri test -p tds-protocol
      
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - run: cargo fuzz run parse_packet -- -max_total_time=300
      
  semver:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v4
      - uses: obi1kenobi/cargo-semver-checks-action@v2
        
  integration:
    runs-on: ubuntu-latest
    services:
      mssql:
        image: mcr.microsoft.com/mssql/server:2022-latest
        env:
          ACCEPT_EULA: Y
          SA_PASSWORD: YourStrong@Passw0rd
        ports:
          - 1433:1433
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo nextest run --features integration-tests
```

### 6.3 Fuzz Testing Targets

```rust
// fuzz/fuzz_targets/parse_packet.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use tds_protocol::packet::PacketHeader;

fuzz_target!(|data: &[u8]| {
    let _ = PacketHeader::parse(data);
});

// fuzz/fuzz_targets/parse_token.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use tds_protocol::token::Token;

fuzz_target!(|data: &[u8]| {
    let _ = Token::parse(data);
});

// fuzz/fuzz_targets/connection_string.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use mssql_client::Config;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = s.parse::<Config>();
    }
});
```

### 6.4 Benchmarking

```rust
// benches/query_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use mssql_client::Client;
use tokio::runtime::Runtime;

fn query_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = rt.block_on(async {
        Client::connect("Server=localhost;...").await.unwrap()
    });
    
    let mut group = c.benchmark_group("query");
    
    for rows in [100, 1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::new("select_rows", rows),
            &rows,
            |b, &rows| {
                b.to_async(&rt).iter(|| async {
                    let sql = format!("SELECT TOP {} * FROM test_data", rows);
                    let mut stream = client.query(&sql).await.unwrap();
                    let mut count = 0;
                    while let Some(_) = stream.next().await.unwrap() {
                        count += 1;
                    }
                    count
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, query_benchmark);
criterion_main!(benches);
```

### 6.5 Documentation Standards

**Required Documentation:**
- All public items must have doc comments
- Examples required for public functions
- Module-level documentation explaining purpose
- `# Errors` section for fallible functions
- `# Panics` section if function can panic

**Example:**
```rust
/// Execute a SQL query and return a stream of rows.
///
/// # Arguments
///
/// * `sql` - The SQL query to execute. Use `@p1`, `@p2`, etc. for parameters.
///
/// # Returns
///
/// A stream of [`Row`] objects that can be iterated asynchronously.
///
/// # Errors
///
/// Returns an error if:
/// - The connection is not in a ready state
/// - The SQL syntax is invalid
/// - A network error occurs
/// - The command times out
///
/// # Examples
///
/// ```rust
/// # async fn example() -> Result<(), mssql_client::Error> {
/// let mut client = mssql_client::Client::connect("...").await?;
/// 
/// let mut rows = client.query("SELECT id, name FROM users").await?;
/// while let Some(row) = rows.next().await? {
///     let id: i32 = row.get(0)?;
///     let name: String = row.get(1)?;
///     println!("{}: {}", id, name);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn query(&mut self, sql: &str) -> Result<QueryStream<'_>, Error> {
    // ...
}
```

### 6.6 MSRV Policy

**Policy:** Rolling 6-month MSRV window aligned with Tokio's policy.

**Current MSRV:** Rust 1.85.0 (Rust 2024 Edition)

**Enforcement:**
```toml
# Cargo.toml
[package]
rust-version = "1.85"
```

**CI Verification:**
```yaml
msrv:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@1.85.0
    - run: cargo check --all-features
```

### 6.7 Deprecation Strategy

**Process:**
1. Mark item with `#[deprecated(since = "X.Y.Z", note = "Use `new_item` instead")]`
2. Maintain deprecated item for at least 2 minor versions
3. Remove in next major version
4. Document migration path in CHANGELOG

---

## 7. Implementation Roadmap

### Phase 1: Workspace Foundation (Week 1-2)

- [x] Initialize flat workspace with virtual manifest
- [x] Configure `[workspace.lints]` and `[workspace.dependencies]`
- [x] Initialize `xtask` crate with basic commands
- [x] Configure `cargo-deny` and `cargo-hakari`
- [x] Set up CI pipeline (GitHub Actions)
- [x] Create `mssql-testing` crate scaffolding

### Phase 2: Protocol Layer ✅ Complete

**`tds-protocol` crate:**
- [x] Implement `PacketHeader` and `PacketStatus` bitflags
- [x] Implement token parser framework
- [x] Implement core tokens: `ColMetaData`, `Row`, `Done`, `Error`, `Info`
- [x] Implement Login7 packet construction
- [x] Implement Prelogin packet construction
- [x] Verify strict `no_std` + `alloc` compatibility
- [x] Fuzz testing for packet/token parsing (11 fuzz targets)

### Phase 3: TLS Layer ✅ Complete

**`mssql-tls` crate:**
- [x] Implement TDS 7.x TLS negotiation (post-prelogin)
- [x] Implement TDS 8.0 strict mode (TLS-first)
- [x] Certificate validation and hostname verification
- [x] `TrustServerCertificate` support with warnings

### Phase 4: Codec Layer ✅ Complete

**`mssql-codec` crate:**
- [x] Implement `tokio_util::codec::Decoder` for TDS packets
- [x] Implement packet reassembly logic
- [x] Implement IO splitting for cancellation
- [x] Integration tests with mock server

### Phase 5: Authentication ✅ Complete

**`mssql-auth` crate:**
- [x] Implement SQL Authentication (Login7 flow)
- [x] Implement Azure AD token authentication
- [x] Add `azure-identity` feature for Managed Identity
- [x] Add `integrated-auth` feature (Kerberos/GSSAPI)
- [x] Add `sspi-auth` feature (cross-platform SSPI via sspi-rs)
- [x] Add `cert-auth` feature (client certificate authentication)

### Phase 6: Client API ✅ Complete

**`mssql-client` crate:**
- [x] Implement connection string parser
- [x] Implement `Client<S>` type-state machine
- [x] Implement basic query execution
- [x] Implement `Row` struct with `Arc<Bytes>` pattern
- [x] Implement parameterized queries
- [x] Implement transaction support with savepoints

### Phase 7: Type System ✅ Complete

**`mssql-types` crate:**
- [x] Implement basic type mappings (integers, strings)
- [x] Implement date/time type mappings
- [x] Implement `Decimal` mapping
- [x] Implement `Uuid` mapping
- [x] Implement PLP chunk decoding for large objects
- [x] Implement streaming blob reader

### Phase 8: Production Features ✅ Complete

- [x] Implement `mssql-driver-pool` with `sp_reset_connection`
- [x] Implement bulk copy (BCP) support
- [x] Add OpenTelemetry instrumentation feature
- [x] Implement retry policies
- [x] Performance optimization and benchmarking
- [x] Documentation and examples

### Phase 9: Derive Macros ✅ Complete

**`mssql-derive` crate:**
- [x] Implement `#[derive(FromRow)]`
- [x] Implement `#[derive(ToParams)]`
- [x] Implement `#[derive(Tvp)]`

### Phase 10: Release Preparation ✅ v0.2.0 Released

- [x] API review and stabilization
- [x] Security audit (cargo-deny, cargo-audit)
- [x] Documentation review
- [x] Migration guide from `tiberius`
- [x] Publish to crates.io

### v0.2.0 Delivered Features

- [x] Table-Valued Parameters (TVP) via `Tvp` type
- [x] Azure Managed Identity (`azure-identity` feature)
- [x] Integrated authentication (`integrated-auth` feature)
- [x] SSPI authentication (`sspi-auth` feature)
- [x] Client certificate authentication (`cert-auth` feature)
- [x] Query cancellation with ATTENTION packets
- [x] Per-query timeouts
- [x] OpenTelemetry metrics (`DatabaseMetrics`)
- [x] Always Encrypted cryptography (AEAD, RSA-OAEP, CEK caching)

### Future Releases

**v0.3.0 Delivered:**
- [x] Always Encrypted key providers (Azure KeyVault, Windows CertStore)
- [x] Streaming LOB API (`Row::get_stream()` → `BlobReader`)
- [x] Change Tracking integration (`ChangeTrackingQuery`, `ChangeOperation`)

**v0.4.0 Delivered:**
- [x] TDS 7.3 protocol support (SQL Server 2008/2008 R2)
- [x] `TdsVersion` configuration API
- [x] Version negotiation and detection

**v0.5.0 Delivered:**
- [x] Collation-aware VARCHAR decoding (`encoding` feature)
- [x] `Column` struct marked `#[non_exhaustive]`
- [x] Windows code page support (1252, 1251, 1250, etc.)

**v1.0.0+ Roadmap:**
- [x] `#[derive(Tvp)]` macro (procedural) - Completed in v0.5.0
- [ ] True network-level LOB streaming (currently buffered up to 100MB)
- [ ] Connection resiliency improvements (automatic reconnection)

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
| 2016 | ✅ | Minimum supported |
| 2017 | ✅ | |
| 2019 | ✅ | |
| 2022 | ✅ | TDS 8.0 support |
| 2025 | ✅ | Managed Identity enhancements |
| Azure SQL Database | ✅ | All authentication methods |
| Azure SQL Managed Instance | ✅ | All authentication methods |

### 8.3 Feature Flag Matrix

| Feature | Default | Dependencies Added | Status |
|---------|---------|-------------------|--------|
| `default` | ✅ | Core functionality | Stable |
| `chrono` | ✅ | `chrono` for date/time types | Stable |
| `uuid` | ✅ | `uuid` for UNIQUEIDENTIFIER | Stable |
| `decimal` | ✅ | `rust_decimal` for DECIMAL/NUMERIC | Stable |
| `json` | ❌ | `serde_json` for JSON parsing | Stable |
| `azure-identity` | ❌ | `azure_identity` | Stable |
| `integrated-auth` | ❌ | `gssapi` (Linux/macOS) | Stable |
| `sspi-auth` | ❌ | `sspi-rs` (cross-platform) | Stable |
| `cert-auth` | ❌ | None (uses rustls) | Stable |
| `otel` | ❌ | `opentelemetry`, `tracing-opentelemetry` | Stable |
| `zeroize` | ❌ | `zeroize` for credential cleanup | Stable |
| `always-encrypted` | ❌ | Cryptography dependencies | Stable |
| `encoding` | ❌ | `encoding_rs` | Stable |

### 8.4 Migration Guide from Tiberius

**Connection String:**
```rust
// Tiberius
let config = Config::from_ado_string(&connection_string)?;
let tcp = TcpStream::connect(config.get_addr()).await?;
let client = Client::connect(config, tcp.compat_write()).await?;

// This driver
let client = Client::connect(&connection_string).await?;
```

**Query Execution:**
```rust
// Tiberius
let stream = client.query("SELECT @P1", &[&1i32]).await?;
let row = stream.into_row().await?.unwrap();

// This driver
let mut stream = client.query("SELECT @p1").bind(1i32).await?;
let row = stream.next().await?.unwrap();
```

**Transactions:**
```rust
// Tiberius (manual)
client.execute("BEGIN TRANSACTION", &[]).await?;
client.execute("INSERT INTO ...", &[]).await?;
client.execute("COMMIT", &[]).await?;

// This driver (type-safe)
let tx = client.begin_transaction().await?;
tx.execute("INSERT INTO ...").await?;
tx.commit().await?; // Returns Client<Ready>
```

**Prepared Statements:**
```rust
// Tiberius (implicit, no caching control)
// Statements are prepared per-execution
let stream = client.query("SELECT @P1", &[&user_id]).await?;

// This driver (automatic caching with LRU eviction)
// First call: sp_prepare + sp_execute
// Subsequent calls: sp_execute only (cache hit)
for user_id in user_ids {
    let row = client.query("SELECT @p1")
        .bind(user_id)
        .await?
        .next()
        .await?;
}

// Or explicit prepared statement for fine-grained control
let stmt = client.prepare("SELECT @p1").await?;
for user_id in user_ids {
    let row = stmt.query().bind(user_id).await?.next().await?;
}
// sp_unprepare called automatically when stmt is dropped
```

**Azure SQL Redirects:**
```rust
// Tiberius (manual handling required)
loop {
    match Client::connect(config, tcp.compat_write()).await {
        Ok(client) => break client,
        Err(Error::Routing { host, port }) => {
            // Manual reconnection logic required
            config = config.with_server(format!("{}:{}", host, port));
            tcp = TcpStream::connect(&config).await?;
            continue;
        }
        Err(e) => return Err(e),
    }
}

// This driver (automatic, transparent handling)
let client = Client::connect(&connection_string).await?;
// Azure SQL gateway redirects handled automatically
// No manual intervention required
```

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
| 1.0.0 | 2025-12-11 | Initial comprehensive specification |
| 1.1.0 | 2025-12-11 | Security guidance corrections (ADR-013 Always Encrypted), savepoint validation, prepared statement lifecycle (§4.5), Azure SQL routing (§4.6), OpenTelemetry 0.31, version constraint policy, cargo-deny/hakari integration, native async trait guidance, migration guide updates |
| 1.2.0 | 2025-12-24 | Updated for v0.2.0 release: Phase 5 auth complete, ADR-013 status updated (cryptography implemented), feature flag matrix expanded, v0.2.0 delivered features documented, v0.3.0 roadmap updated |
| 1.3.0 | 2025-12-25 | Updated for v0.3.0 release: Always Encrypted key providers (InMemoryKeyStore, KeyStoreProvider trait), true LOB streaming (LobStream), Change Tracking integration, all 12 data type parsing fixes complete |
| 1.4.0 | 2025-12-31 | Updated for v0.4.0 release: TDS 7.3 protocol support (SQL Server 2008/2008 R2), TdsVersion configuration, version negotiation |
| 1.5.0 | 2026-01-01 | Updated for v0.5.0 release: Collation-aware VARCHAR decoding, encoding feature, Column marked non_exhaustive |

---

*This document is the authoritative architectural reference for the mssql-driver project. All implementation decisions should align with the principles and specifications herein.*

