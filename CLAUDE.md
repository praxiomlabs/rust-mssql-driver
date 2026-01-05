# CLAUDE.md - Project Context for rust-mssql-driver

## Project Overview

A high-performance MS SQL Server driver for Rust that aims to surpass `prisma/tiberius`. This is a greenfield implementation built from scratch using modern Rust practices.

**Reference Implementation:** `/tmp/tiberius/` (cloned for analysis, not as a base)

## Goals

1. **Broad TDS support** - TDS 7.3 (SQL Server 2008+) through TDS 8.0 (SQL Server 2022+ strict mode)
2. **Built-in connection pooling** - Unlike Tiberius which defers to bb8/deadpool
3. **Type-state pattern** - Compile-time connection state enforcement
4. **Tokio-native** - No runtime agnosticism; Tokio 1.48+ hard dependency
5. **Zero-copy where possible** - `Arc<Bytes>` pattern for row data
6. **Modern Rust** - 2024 Edition, MSRV 1.85

## Key Architecture Decisions

Refer to `ARCHITECTURE.md` (v1.2.0) for complete details. Critical decisions:

| Decision | Choice | Rationale |
|----------|--------|-----------|
| TLS | rustls | Pure Rust, auditable, no OpenSSL dependency |
| Async Runtime | Tokio 1.48+ | Dominant ecosystem, hard dependency simplifies design |
| Error Handling | thiserror 2.0 | Derive macros, stable API |
| Observability | OpenTelemetry 0.31 | Industry standard, version-aligned crates |
| Edition | Rust 2024 | Latest language features, MSRV 1.85 |

## Security-Critical Guidelines

### Always Encrypted vs T-SQL Encryption

**NEVER suggest ENCRYPTBYKEY as a workaround for Always Encrypted.**

| Feature | Always Encrypted | ENCRYPTBYKEY |
|---------|------------------|--------------|
| Key Location | Client only | SQL Server |
| DBA Access | Cannot see plaintext | Can see plaintext |
| Threat Model | Protects FROM server | Protects ON server |

Always Encrypted is fully implemented via the `always-encrypted` feature with production-ready key providers:
1. **`InMemoryKeyStore`** - For development/testing
2. **`AzureKeyVaultProvider`** - For Azure Key Vault (`azure-identity` feature)
3. **`WindowsCertStoreProvider`** - For Windows Certificate Store (`sspi-auth` feature, Windows only)
4. Implement the `KeyStoreProvider` trait for custom key storage
5. **Do NOT use ENCRYPTBYKEY** - it does not provide the same security guarantees

### Savepoint Name Validation

All savepoint names MUST be validated before use in SQL:

```rust
fn validate_identifier(name: &str) -> Result<(), Error> {
    static IDENTIFIER_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_@#$]{0,127}$").unwrap());

    if name.is_empty() || !IDENTIFIER_RE.is_match(name) {
        return Err(Error::Config("Invalid identifier".into()));
    }
    Ok(())
}
```

## Workspace Structure

```
rust-mssql-driver/
├── crates/
│   ├── tds-protocol/      # Pure TDS protocol (no_std)
│   ├── mssql-tls/         # TLS negotiation
│   ├── mssql-codec/       # Async framing layer
│   ├── mssql-types/       # SQL ↔ Rust type mapping
│   ├── mssql-auth/        # Authentication strategies
│   ├── mssql-pool/        # Connection pooling (publishes as mssql-driver-pool)
│   ├── mssql-client/      # Public API surface
│   ├── mssql-derive/      # Proc macros for row mapping
│   └── mssql-testing/     # Test infrastructure
├── xtask/                 # Build automation
├── ARCHITECTURE.md        # Comprehensive architecture document
├── CLAUDE.md              # This file
└── Cargo.toml             # Virtual workspace manifest
```

## Key Implementation Patterns

### Type-State Connection

```rust
pub struct Client<S: ConnectionState> { /* ... */ }

impl Client<Disconnected> {
    pub async fn connect(config: Config) -> Result<Client<Ready>, Error>;
}

impl Client<Ready> {
    pub async fn query(&mut self, sql: &str) -> Result<QueryResult, Error>;
    pub fn begin_transaction(self) -> Result<Client<InTransaction>, Error>;
}
```

### Prepared Statement Lifecycle

1. Hash SQL → check LRU cache
2. Cache miss → `sp_prepare` → store handle
3. Execute via `sp_execute` with handle
4. On eviction/close → `sp_unprepare`
5. Pool returns handle to pool-level cache

### Azure SQL Redirect Handling

Azure SQL Gateway may redirect connections. Handle `ENVCHANGE` routing tokens:

```rust
const MAX_REDIRECT_ATTEMPTS: u8 = 2;

loop {
    match Self::try_connect(&current_config).await {
        Ok(client) => return Ok(client),
        Err(Error::Routing { host, port }) => {
            current_config = current_config.with_host(&host).with_port(port);
            continue;
        }
        Err(e) => return Err(e),
    }
}
```

## Development Tooling

### Required Tools

- Rust 1.85+ (2024 Edition)
- cargo-hakari (workspace-hack management)
- cargo-deny (dependency auditing)

### cargo-deny + cargo-hakari Interaction

cargo-deny may flag workspace-hack as unused. Add to `deny.toml`:

```toml
[bans]
skip = [
    { name = "workspace-hack", reason = "cargo-hakari managed crate" }
]
```

### Version Constraint Policy

Use minimum versions, not exact pins:

```toml
# Correct
tokio = "1.48"           # >=1.48.0, <2.0.0

# Avoid
tokio = "=1.48.0"        # Exact pin - blocks security updates
```

## OpenTelemetry Dependencies

All otel crates must be version-aligned at 0.31:

```toml
opentelemetry = "0.31"
opentelemetry_sdk = "0.31"
opentelemetry-otlp = "0.31"
tracing-opentelemetry = "0.31"
```

## Testing Strategy

1. **Unit tests** - Protocol encoding/decoding, type conversions
2. **Integration tests** - Against SQL Server (Docker)
3. **Compatibility tests** - TDS 7.4, 8.0; SQL Server 2017-2022
4. **Fuzzing** - Protocol parser with cargo-fuzz

## Migration Guide (from Tiberius)

Key differences for migrators:

| Tiberius | This Driver |
|----------|-------------|
| `Client::connect()` | `Client::connect()` (type-state) |
| External pooling (bb8) | Built-in `Pool` |
| Runtime agnostic | Tokio-only |
| `QueryResult` iterator | Streaming `RowStream` |
| Manual prepared | Auto-cached prepared statements |
| Manual Azure redirect | Automatic redirect handling |

## Commit Standards

- Use conventional commits (feat, fix, refactor, docs, test)
- No AI branding in commit messages
- Logical, incremental commits

## Document References

- `ARCHITECTURE.md` - Complete architecture specification (v1.2.0)
- MS-TDS Protocol Spec - Microsoft documentation
- Tiberius source - `/tmp/tiberius/` (reference only)
