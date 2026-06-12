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

Refer to `ARCHITECTURE.md` (v1.8.0) for complete details. Critical decisions:

| Decision | Choice | Rationale |
|----------|--------|-----------|
| TLS | rustls | Pure Rust, auditable, no OpenSSL dependency |
| Async Runtime | Tokio 1.48+ | Dominant ecosystem, hard dependency simplifies design |
| Error Handling | thiserror 2.0 | Derive macros, stable API |
| Observability | OpenTelemetry 0.32 | Industry standard, version-aligned crates |
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
3. **`WindowsCertStoreProvider`** - For Windows Certificate Store (`windows-certstore` feature, Windows only)
4. Implement the `KeyStoreProvider` trait for custom key storage
5. **Do NOT use ENCRYPTBYKEY** - it does not provide the same security guarantees

#### Decryption Wiring

When `Column Encryption Setting=Enabled` is in the connection string (or `Config::column_encryption` is set programmatically), the client:

1. Negotiates Always Encrypted support during login (`FeatureExt`)
2. Parses `CryptoMetadata` and `CekTable` from `ColMetaData` tokens (in `tds-protocol::crypto`)
3. Pre-resolves Column Encryption Keys asynchronously via `ColumnDecryptor::from_metadata()` (in `mssql-client::column_decryptor`) — this is where key store providers are called
4. Decrypts each encrypted column value synchronously during row parsing via `AeadEncryptor::decrypt()` (AEAD_AES_256_CBC_HMAC_SHA256)

Decryption is supported in all three response readers: `read_query_response()`, `read_procedure_result()`, and `read_multi_result_response()`. The pattern is symmetric across all three.

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

### Connection String Parser (ADO.NET Conformance)

The `Config::from_connection_string()` parser conforms to the Microsoft ADO.NET `SqlConnection.ConnectionString` specification. Key behaviors:

- **Quoted values**: `Password="my;pass"` and `Password='it''s complex'` supported per spec. Doubled quotes are escapes.
- **Protocol prefixes**: `tcp:` stripped automatically (Azure Portal format). `np:` and `lpc:` rejected with clear errors.
- **Boolean validation**: Invalid boolean values return errors (not silent defaults). Accepts `true/false/yes/no/1/0`.
- **Server aliases**: `Server`, `Data Source`, `Addr`, `Address`, `Network Address`, `Host`
- **`ApplicationIntent`**: `ReadOnly`/`ReadWrite` — wired to LOGIN7 `READONLY_INTENT` bit for AlwaysOn AG routing
- **`Workstation ID`** / `WSID`: Sent in LOGIN7 HostName field. Defaults to machine hostname via env var.
- **`Current Language`** / `Language`: Sent in LOGIN7 Language field.
- **`ConnectRetryCount`** / `ConnectRetryInterval`**: Wired to `RetryPolicy`.
- **`MultiSubnetFailover`**: Parallel TCP connect to all resolved IPs for AG listener failover.
- **`SendStringParametersAsUnicode`**: When `false`, sends string params as VARCHAR (Windows-1252) instead of NVARCHAR (UTF-16) for index seek compatibility.
- **`Encrypt`**: Supports `strict`, `mandatory`, `optional`, `no_tls`, plus standard booleans.
- **Pool keywords** (`Max Pool Size`, etc.): Recognized with info-level log directing to `PoolConfig`.
- **Known-but-unsupported keywords** (30+): Recognized at info level instead of silently ignored.

See the [`mssql-client` `config` module docs](https://docs.rs/mssql-client/latest/mssql_client/config/) for the full keyword reference.

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

**Status: designed, not wired.** The LRU cache exists in
`mssql-client/src/statement_cache.rs` but no query path consults it — every
parameterized query uses `sp_executesql` (server-side plan cache still gives
plan reuse). The intended design when wiring lands:

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

### Stored Procedure Execution (v0.8.0)

Two-tier API for calling stored procedures via TDS RPC:

```rust
// Simple (input-only, positional params):
let result = client.call_procedure("dbo.MyProc", &[&1i32]).await?;

// Builder (named params, OUTPUT support):
let result = client.procedure("dbo.CalculateSum")?
    .input("@a", &10i32)
    .input("@b", &20i32)
    .output_int("@result")
    .execute().await?;
```

Returns `ProcedureResult` with `return_value`, `rows_affected`, `output_params`, and `result_sets`. All methods on `impl<S: ConnectionState>` — works in both `Ready` and `InTransaction` states.

### SQL Browser Instance Resolution (v0.8.0)

Named instances (e.g., `Server=localhost\SQLEXPRESS`) are automatically resolved via the SQL Server Browser service (UDP 1434). The `crate::browser` module implements the SSRP protocol (MC-SQLR spec). Resolution happens transparently in `Client::connect()` when `config.instance` is `Some`.

### FILESTREAM BLOB Access (Windows only, `filestream` feature)

Async read/write of SQL Server FILESTREAM data via `OpenSqlFilestream` from the OLE DB Driver DLL. The implementation uses runtime dynamic loading (`LoadLibraryW` + `GetProcAddress`) with a fallback chain: `msoledbsql19.dll` → `msoledbsql.dll` → `sqlncli11.dll`. The function pointer is cached via `OnceLock`. The Win32 `HANDLE` is wrapped in `tokio::fs::File` for `AsyncRead + AsyncWrite`. See the [`mssql-client` `filestream` module docs](https://docs.rs/mssql-client/latest/mssql_client/filestream/) for setup and usage.

## Development Tooling

### Required Tools

- Rust 1.88+ (2024 Edition) — pinned via `rust-toolchain.toml`
- `just` — task runner used for all development workflows (`just ci-all`, `just doc-consistency`, etc.)
- `gh` CLI — useful for inspecting workflow runs and PRs
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

See the [Dependency Policy section of CONTRIBUTING.md](CONTRIBUTING.md#dependency-policy) for the full dependency management policy: when to add a new dep, when to take a bump, when to bump MSRV for a security fix, and how advisory ignores are managed.

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

- [`RELEASING.md`](RELEASING.md) — how the release-plz pipeline works, the rules that survive automation (irreversibility, never cancel mid-publish), recovery procedures, and post-release verification
- [`STABILITY.md`](STABILITY.md) — API stability guarantees, MSRV Increase Policy (**authoritative: MSRV bumps are NOT breaking changes**), supported versions
- [`SECURITY.md`](SECURITY.md) — security policy, threat model, supported versions for security fixes
- [`CONTRIBUTING.md` § Dependency Policy](CONTRIBUTING.md#dependency-policy) — when and how to take dep bumps, handle advisories, bump MSRV

### How releases work (release-plz)

Releases are fully automated by [release-plz](https://release-plz.dev) (`release-plz.toml` + `.github/workflows/release-plz.yml`):

1. Conventional commits merge to the trunk; release-plz keeps a **Release PR** open with the version bump and CHANGELOG entry (versions derived from commit types + `cargo-semver-checks`; pre-1.0, breaking → minor).
2. **Merging the Release PR is the release** — release-plz publishes all 8 crates in dependency order, creates the `vX.Y.Z` tag, and creates the GitHub Release. Nothing publishes before that merge (`release_always = false`).
3. The publish job is idempotent — if it fails partway, re-run it; it skips already-published crates.

Rules for agents: **never run `cargo publish`, never create version tags, never hand-edit the workspace version** — those all belong to release-plz. Never cancel the publish job mid-run. Merging a Release PR requires explicit human approval. See RELEASING.md for recovery procedures.

**Verify the Release PR before it is merged** (both failure modes have happened):

- **Version**: check the proposed bump against the commits since the last
  release. Pre-1.0, any breaking change requires a minor bump; release-plz
  under-bumps when the breaking marker is missing from commits (see Commit
  Standards) — the v0.13.2-for-breaking-main incident was caught only by this
  check. To correct a wrong bump, do NOT hand-edit the version: land a marker
  commit with the proper `type!:` subject / `BREAKING CHANGE:` footer.
  release-plz attributes commits to crates **by changed file paths**, so an
  empty commit has no effect — the marker must touch the affected crate
  (PR #201 used a genuine doc improvement).
- **Changelog completeness**: release-plz has dropped commits from generated
  CHANGELOG entries before (#184, v0.13.0). Diff the entries against
  `git log` since the last tag.

`just doc-consistency` (also a CI gate) verifies MSRV references agree across files, CHANGELOG matches the workspace version, deny.toml/audit.toml ignore lists stay in sync, and first-party dependency snippets in README/docs match the workspace version.

### CI/CD workflows

- `.github/workflows/ci.yml` — runs on pushes to main and PRs to main. Cross-platform matrix (Linux / macOS / Windows) plus hygiene (typos, unused deps), ADR-011 (no mod.rs), doc-consistency, and AI-branding gates. Has `workflow_dispatch` for manual reruns.
- `.github/workflows/benchmarks.yml` — runs on pushes/PRs to main. Performance regression detection.
- `.github/workflows/fuzz-nightly.yml` — daily scheduled long-budget fuzzing (5 min per target, all 12 targets); crash artifacts uploaded for triage. The per-PR `fuzz-smoke` job in ci.yml stays at 15 s/target.
- `.github/workflows/security-audit.yml` — weekly schedule + dep-file changes on pushes/PRs to main.
- `.github/workflows/release-plz.yml` — runs on push to main. Maintains the Release PR and performs the publish when one merges (see above).

All workflows use `concurrency: cancel-in-progress` for non-main branches to save CI cycles, while keeping main runs to completion for the full audit trail.

## OpenTelemetry Dependencies

The three core otel crates are version-aligned at 0.32. `tracing-opentelemetry` tracks 0.33, which depends on otel 0.32 (see the note in `Cargo.toml`):

```toml
opentelemetry = "0.32"
opentelemetry_sdk = "0.32"
opentelemetry-otlp = "0.32"
tracing-opentelemetry = "0.33"
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
| `QueryResult` iterator | `RowStream` (lazy decode; response buffered) |
| Manual prepared | `sp_executesql` (client cache planned) |
| Manual Azure redirect | Automatic redirect handling |

## Commit Standards

- Use conventional commits (feat, fix, refactor, docs, test)
- **Breaking changes MUST carry the conventional marker**: `!` after the type
  (`fix(types)!:`) or a `BREAKING CHANGE:` footer. A `BREAKING:` line in the
  commit body is **ignored** by release-plz — and the `cargo-semver-checks`
  backstop runs with default features, so breaking changes in feature-gated
  API (chrono/uuid/decimal codecs, Always Encrypted, FILESTREAM) are invisible
  to it (#202). Version correctness rests on the commit message. Precedent:
  the v0.13.2 wrong bump, corrected by PR #201 before release.
- Run `cargo fmt --all` **before** committing, not after the CI mirror flags
  it — fmt-check is the first gate in `just ci-all`, and a failure there
  costs a full gauntlet cycle plus a commit amend.
- No AI branding in commit messages
- Logical, incremental commits

## Document References

Primary references (in the repository):

- `ARCHITECTURE.md` — Complete architecture specification (includes ADRs and MSRV policy §6.6)
- `STABILITY.md` — API stability guarantees and the authoritative MSRV Increase Policy
- `RELEASING.md` — how the release-plz pipeline works, rules that survive automation, recovery procedures (incl. the v0.11.0 partial-publish recovery), post-release verification
- `SECURITY.md` — Security policy and threat model
- `CONTRIBUTING.md` — Contribution guide, commit format, review process
- `MAINTAINERS.md` — Maintainer list and contact channels
- `CODE_OF_CONDUCT.md` — Rust Code of Conduct
- `CONTRIBUTING.md` (Dependency Policy section) — dependency management policy
- `MIGRATION.md` — Migration guide from Tiberius
- `mssql-client` `config` module rustdoc (docs.rs) — ADO.NET connection string keyword reference (full spec conformance)
- `mssql-client` `encryption` module rustdoc (docs.rs) — Always Encrypted (key providers, transparent decryption)
- `mssql-client` `procedure` module rustdoc (docs.rs) — stored procedure API

External references:

- MS-TDS Protocol Spec — Microsoft documentation
- Tiberius source — Reference only (not a dependency)

## Conventions for AI Assistants Working in This Repository

When making changes here, remember:

1. **MSRV bumps are NOT breaking changes.** This is stated authoritatively in STABILITY.md § MSRV Increase Policy. If CONTRIBUTING.md ever contradicts this, STABILITY.md wins — and fix CONTRIBUTING.md as part of your change. The doc consistency linter (`scripts/check-doc-consistency.sh`) catches this contradiction automatically.

2. **Prefer fixing over ignoring** security advisories. Bumping MSRV for a security fix is explicitly permitted. See the v0.7.0 precedent documented in the Dependency Policy section of `CONTRIBUTING.md`.

3. **Releases belong to release-plz.** Never run `cargo publish`, never create version tags, never hand-edit the workspace version or hand-write CHANGELOG release entries — the Release PR does all of that, and merging it (a human decision) is the release. `scripts/check-doc-consistency.sh` (a CI gate) catches version/MSRV drift.

4. **Respect the release rules** documented in RELEASING.md (irreversibility, never cancel a publish mid-run, recovery is re-run). They exist because of specific past incidents. Don't work around them — if you find them inconvenient, propose a process change via an issue.

5. **Trunk-based development.** Work happens on short-lived feature branches merged into `main` via PRs (no squash-merging — history stays linked). CI (`ci.yml`) triggers on pushes to `main` and on PRs whose base is `main` — not on feature-branch pushes that lack an open PR to `main`. Cross-platform issues surface at PR time, not at release time.

6. **Update CLAUDE.md when you add new infrastructure.** This document is the entry point for future AI assistants working on the repo. Adding a new tool, workflow, or policy without updating CLAUDE.md leaves future sessions flying blind. The Process and Governance section above should reference all the discoverable infrastructure.

7. **Use issue/PR templates.** They ask the right questions. If you're opening an issue or PR, fill out the template completely — it helps the human reviewer and it helps future AI sessions parse the intent.

8. **Run the full local CI mirror before every push; never trust a subset.** `cargo check` and unit tests do not catch the `-D warnings`-class failures CI rejects. This repo's gate is reproduced by `just ci-all` (fmt + clippy `--all-features --all-targets -D warnings` + nextest all-features + `cargo doc -D warnings` + examples). `ci-all` does NOT compose two gates CI enforces, so the real pre-push command is:

   ```bash
   just ci-all && just typos && \
     MSSQL_HOST=localhost MSSQL_PORT=1433 MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
       cargo nextest run --all-features --run-ignored ignored-only --no-fail-fast \
       -E 'not (binary(azure_sql) or test(azure_identity_auth) or test(cert_auth))'
   ```

   - `just typos` runs the same bare `typos` (whole-tree, `typos.toml`-aware) the CI Hygiene job runs. It needs the tool installed at an MSRV-1.88-compatible version: `cargo install typos-cli --version 1.42.3 --locked` (newer requires rustc 1.91). The recipe still prints a WARN and passes if typos is absent, so confirm it is actually installed — a silent skip is how a spell-check failure reaches CI.
   - The ignored-only suite needs a local SQL Server 2022 container (`just sql-server-start`). `ci-all` and unit tests will NOT catch live-only failures (e.g. bulk temporal, decimal high-scale).
   - **Never open a PR stacked on another feature branch.** CI only triggers on PRs based on `main` (see convention 5), so a stacked PR gets zero CI until it is retargeted to `main`.
   - **Branch protection requires up-to-date-with-`main`.** Merge each green PR before opening the next to minimize the `update-branch` + CI-re-run churn that comes from keeping many PRs in flight at once.
   - **The Semver Check CI job is advisory** (`continue-on-error: true`), but `gh pr checks --watch` still exits 1 when it fails — a red advisory check is not a blocked PR. Before concluding a PR is stuck, check the required checks / `mergeable` state (`gh api .../pulls/N -q .mergeable_state`). The job's known blind spots are tracked in #202.
   - The tests excluded from the pre-push command (`azure_sql`, `azure_identity_auth`, `cert_auth`) need a **live Azure SQL environment**, not the local container. One exists for #155 work (credentials live outside the repo, e.g. a local gitignored `.tmp/azure.env`); run that suite when touching authentication or Azure-path code.
