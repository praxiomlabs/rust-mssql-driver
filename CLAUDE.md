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
6. **Modern Rust** - 2024 Edition, MSRV 1.88

## Key Architecture Decisions

Refer to `ARCHITECTURE.md` (v1.2.0) for complete details. Critical decisions:

| Decision | Choice | Rationale |
|----------|--------|-----------|
| TLS | rustls | Pure Rust, auditable, no OpenSSL dependency |
| Async Runtime | Tokio 1.48+ | Dominant ecosystem, hard dependency simplifies design |
| Error Handling | thiserror 2.0 | Derive macros, stable API |
| Observability | OpenTelemetry 0.31 | Industry standard, version-aligned crates |
| Edition | Rust 2024 | Latest language features, MSRV 1.88 |

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

- Rust 1.88+ (2024 Edition) — pinned via `rust-toolchain.toml`
- `just` — task runner used for all development workflows (`just ci-all`, `just release-status`, etc.)
- `gh` CLI — required for `just release-status` and `just tag` (workflow status checks)
- `cargo-deny` — dependency auditing
- `cargo-hack` — feature flag matrix validation
- `cargo-nextest` — fast test runner (CI uses this)
- `cargo-audit` — security advisory scanning
- `cargo-machete` — unused dependency detection
- `cargo-semver-checks` — semver compliance detection

Run `just setup-tools` to install all of the above with pinned versions compatible with our MSRV.

### Version Constraint Policy

Use minimum versions, not exact pins:

```toml
# Correct
tokio = "1.48"           # >=1.48.0, <2.0.0

# Avoid
tokio = "=1.48.0"        # Exact pin - blocks security updates
```

See [`docs/DEPENDENCY_POLICY.md`](docs/DEPENDENCY_POLICY.md) for the full dependency management policy: when to add a new dep, when to take a bump, when to bump MSRV for a security fix, and how advisory ignores are managed.

## Process and Governance

This is an actively maintained project with documented processes for contributions, releases, and incident response. When working in this repository as an AI assistant, these documents are the source of truth:

### Contributor-facing documents

- [`README.md`](README.md) — project overview and quick start
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contribution guidelines, commit format, review process
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) — community standards (Rust Code of Conduct)
- [`MAINTAINERS.md`](MAINTAINERS.md) — current maintainers and contact channels
- [`.github/CODEOWNERS`](.github/CODEOWNERS) — auto-review routing for PRs
- [`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/) — structured issue templates (bug / feature / question)
- [`.github/pull_request_template.md`](.github/pull_request_template.md) — PR checklist including **MSRV bumps are NOT breaking** note

### Release and policy documents

- [`RELEASING.md`](RELEASING.md) — the Cardinal Rules, the tier-based publish order, the Lessons Learned section (including the v0.5.1 and v0.7.0 incidents), and the Token Health section
- [`STABILITY.md`](STABILITY.md) — API stability guarantees, MSRV Increase Policy (**authoritative: MSRV bumps are NOT breaking changes**), supported versions
- [`SECURITY.md`](SECURITY.md) — security policy, threat model, supported versions for security fixes
- [`docs/VERSION_REFS.md`](docs/VERSION_REFS.md) — comprehensive checklist of every file that must agree on version/MSRV at release time
- [`docs/DEPENDENCY_POLICY.md`](docs/DEPENDENCY_POLICY.md) — when and how to take dep bumps, handle advisories, bump MSRV

### Release observability tooling

These just recipes and xtask commands were added post-v0.7.0 specifically to make releases reliable:

- `just release-status` — dashboard: dev↔main divergence, last tag, CI/Security/Benchmarks/Token Health status, open PRs (bot vs contributor), issue count, local working copy state
- `just release-preflight` — sequential gate check: working copy clean, version refs, audit, deny, wip-check, metadata, URLs, tier-0 publish dry-run
- `just release-check` — comprehensive release validation (includes `release-preflight` gates plus full CI + feature-flag check + panic audit + doc consistency + typos + machete)
- `just doc-consistency` — runs `scripts/check-doc-consistency.sh` to verify MSRV references agree across all files, CHANGELOG matches workspace version, deny.toml and audit.toml ignore lists are in sync, and the STABILITY.md ↔ CONTRIBUTING.md MSRV policy contradiction can never be reintroduced
- `just ci-status-all` — verify CI + Security Audit + Benchmarks all passed on main HEAD (required before tagging per Cardinal Rule #2)
- `just tag` — create an annotated release tag (reverifies workflows green)
- `cargo xtask release-notes [--since <tag>]` — generate a CHANGELOG draft from conventional commits since the last tag, grouped by type with breaking-change detection

When preparing a release, the canonical path is:

```bash
just release-status            # what's the state of the world?
just release-check             # do all the gates pass?
just ci-status-all             # are all workflows green on main HEAD?
just tag                       # create the tag (revalidates workflows)
git push origin vX.Y.Z         # trigger release.yml (publishes to crates.io)
```

Never manually run `cargo publish` — use the automated workflow. Never cancel the release workflow mid-publish. See RELEASING.md for the full Cardinal Rules.

### CI/CD workflows

- `.github/workflows/ci.yml` — runs on main, dev, PRs to main. Cross-platform matrix (Linux / macOS / Windows). Has `workflow_dispatch` for manual reruns.
- `.github/workflows/benchmarks.yml` — runs on main, dev, PRs to main. Performance regression detection.
- `.github/workflows/security-audit.yml` — weekly schedule + triggers on Cargo.toml/Cargo.lock/deny.toml/audit.toml changes on main or dev.
- `.github/workflows/token-health.yml` — weekly schedule + manual dispatch. Verifies `CARGO_REGISTRY_TOKEN` secret is still valid. Opens an issue on failure.
- `.github/workflows/release.yml` — triggered by `v*.*.*` tag push. Publishes all 8 crates to crates.io in tier order with exponential retry.

All workflows use `concurrency: cancel-in-progress` for non-main branches to save CI cycles, while keeping main runs to completion for the full audit trail.

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

Primary references (in the repository):

- `ARCHITECTURE.md` — Complete architecture specification (includes ADRs and MSRV policy §6.6)
- `STABILITY.md` — API stability guarantees and the authoritative MSRV Increase Policy
- `RELEASING.md` — Release process, Cardinal Rules, Lessons Learned (including v0.5.1 and v0.7.0 incidents), Token Health
- `SECURITY.md` — Security policy and threat model
- `CONTRIBUTING.md` — Contribution guide, commit format, review process
- `MAINTAINERS.md` — Maintainer list and contact channels
- `CODE_OF_CONDUCT.md` — Rust Code of Conduct
- `docs/DEPENDENCY_POLICY.md` — Dependency management policy
- `docs/VERSION_REFS.md` — Release-time version reference checklist
- `docs/MIGRATION_FROM_TIBERIUS.md` — Migration guide from Tiberius

External references:

- MS-TDS Protocol Spec — Microsoft documentation
- Tiberius source — Reference only (not a dependency)

## Conventions for AI Assistants Working in This Repository

When making changes here, remember:

1. **MSRV bumps are NOT breaking changes.** This is stated authoritatively in STABILITY.md § MSRV Increase Policy. If CONTRIBUTING.md ever contradicts this, STABILITY.md wins — and fix CONTRIBUTING.md as part of your change. The doc consistency linter (`scripts/check-doc-consistency.sh`) catches this contradiction automatically.

2. **Prefer fixing over ignoring** security advisories. Bumping MSRV for a security fix is explicitly permitted. See the v0.7.0 precedent documented in `docs/DEPENDENCY_POLICY.md`.

3. **Use the release recipes.** Don't manually run `cargo publish`, don't manually construct CHANGELOG entries from scratch, don't manually check each file for version drift. `just release-notes`, `just release-status`, `just release-preflight`, and `scripts/check-doc-consistency.sh` exist to prevent exactly the kinds of mistakes that caused past incidents.

4. **Respect the Cardinal Rules** documented in RELEASING.md. They exist because of specific past incidents. Don't work around them — if you find them inconvenient, propose a process change via an issue.

5. **`dev` branch has CI.** Since post-v0.7.0, both `ci.yml` and `benchmarks.yml` trigger on pushes to `dev`. Cross-platform issues will surface there, not just at release PR time. Push to dev confidently — CI will tell you if something broke.

6. **Update CLAUDE.md when you add new infrastructure.** This document is the entry point for future AI assistants working on the repo. Adding a new tool, workflow, or policy without updating CLAUDE.md leaves future sessions flying blind. The Process and Governance section above should reference all the discoverable infrastructure.

7. **Use issue/PR templates.** They ask the right questions. If you're opening an issue or PR, fill out the template completely — it helps the human reviewer and it helps future AI sessions parse the intent.
