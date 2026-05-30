# Contributing to rust-mssql-driver

Thank you for your interest in contributing! This document provides guidelines and processes for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [First Contribution (Quick Path)](#first-contribution-quick-path)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Breaking Changes](#breaking-changes)
- [Pull Request Process](#pull-request-process)
- [When Your PR Needs Review](#when-your-pr-needs-review)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Architecture Decision Records (ADRs)](#architecture-decision-records-adrs)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct), adopted in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Please be respectful and constructive in all interactions.

Report code-of-conduct violations privately via the [GitHub Security Advisory](https://github.com/praxiomlabs/rust-mssql-driver/security/advisories/new) channel or by contacting the maintainers listed in [MAINTAINERS.md](MAINTAINERS.md).

## First Contribution (Quick Path)

If this is your first time contributing to the project, here's the shortest path from clone to green CI:

1. **Pick an issue.** Browse [open issues](https://github.com/praxiomlabs/rust-mssql-driver/issues), or ask on an issue for guidance before starting.
2. **Fork and clone.** See [Getting Started](#getting-started) below.
3. **Bootstrap your environment.** Run `just bootstrap` (or `just setup-all` if you don't want sudo for Kerberos deps).
4. **Make your change.** Keep commits small and focused. Use [conventional commit format](#commit-messages).
5. **Verify locally.** Run `just ci-all` before pushing — this mirrors what CI will run and is your fastest feedback loop.
6. **Open a PR.** The [PR template](.github/pull_request_template.md) will walk you through what reviewers need to know. File a draft PR if you want early feedback.
7. **Respond to review.** CODEOWNERS automatically requests review from the right maintainers. See [When Your PR Needs Review](#when-your-pr-needs-review) below.

Don't worry if your first PR needs several rounds of revision — that's normal and expected. We try to keep review feedback kind, specific, and actionable.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/rust-mssql-driver.git`
3. Add upstream remote: `git remote add upstream https://github.com/praxiomlabs/rust-mssql-driver.git`
4. Create a feature branch: `git checkout -b feature/your-feature-name`

## Development Setup

### Quick Start (Recommended)

The fastest way to get started:

```bash
# Clone and enter the repository
git clone https://github.com/praxiomlabs/rust-mssql-driver.git
cd rust-mssql-driver

# Full bootstrap: system packages + cargo tools + git hooks
# (Linux: will prompt for sudo to install libkrb5-dev, libclang-dev)
just bootstrap

# Verify everything works with all features
just ci-all
```

**Alternative (no sudo):** If you only need default features (no Kerberos):

```bash
just setup-all   # Cargo tools + git hooks only
just ci          # CI with default features
```

`just setup` reports what's installed and what's missing. `just setup-tools` installs the version-pinned cargo extensions (`cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-semver-checks`, `cargo-watch`). `just setup-hooks` installs a pre-commit hook (format check, clippy, `cargo check`).

### Prerequisites

| Tool | Version | Required | Notes |
|------|---------|----------|-------|
| Rust | 1.88+ | Yes | 2024 Edition |
| Just | 1.23+ | Yes | Command runner |
| jq | any | Yes | JSON parsing |
| Docker | any | No | For integration tests |
| MSVC C++ Build Tools | any | Windows only | Required for TLS (ring/aws-lc-sys). Free via [VS Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) |
| libkrb5-dev | any | Linux only | Required for `integrated-auth` feature |

#### 4. Platform-Specific: Windows C++ Build Tools

The default TLS feature uses `rustls` with `ring`, which requires a C compiler to build
its cryptographic primitives. On Windows, install one of:

- **[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)** (free, ~2 GB) — select the "Desktop development with C++" workload
- **Visual Studio Community/Professional/Enterprise** — with the "Desktop development with C++" workload

This provides the MSVC compiler (`cl.exe`) and Windows SDK headers needed by `ring` and `aws-lc-sys`.

If you only need `sspi-auth` or `filestream` (no TLS), you can skip the C++ tools:
```bash
cargo build --no-default-features --features sspi-auth,filestream
```

#### 5. Platform-Specific: Linux Kerberos Support

For `--all-features` (integrated authentication):

```bash
# Debian/Ubuntu
sudo apt-get install libkrb5-dev libclang-dev

# RHEL/Fedora
sudo dnf install krb5-devel clang-devel

# Or use the just recipe
just setup-linux
```

The `integrated-auth` feature requires `libkrb5-dev` (Kerberos/GSSAPI headers) and
`libclang-dev` (bindgen FFI generation). This is **Linux-only**.

### Justfile Recipe Naming Convention

This project uses a dual-recipe pattern to handle platform differences. Base recipes (`build`, `test`, `nextest`, `ci`) use default features and work everywhere; the `-all` variants (`build-all`, `test-all`, `nextest-all`, `ci-all`) use `--all-features` and need `libkrb5-dev` on Linux.

| Recipe | Features | Platform | Notes |
|--------|----------|----------|-------|
| `just build` | Default | Works everywhere | Day-to-day development |
| `just build-all` | All features | Needs libkrb5-dev on Linux | Full build |
| `just test` | Default | Works everywhere | Uses cargo test |
| `just test-all` | All features | Needs libkrb5-dev on Linux | Uses cargo test |
| `just nextest` | Default | Works everywhere | Uses cargo-nextest (faster) |
| `just nextest-all` | All features | Needs libkrb5-dev on Linux | Uses cargo-nextest |
| `just ci` | Default | Works everywhere | fmt + clippy + nextest + docs + examples |
| `just ci-all` | All features | Matches GitHub Actions | Full CI pipeline |

Use base recipes for day-to-day development; use `-all` when you need Kerberos integration or to match CI exactly. `just ci-all` mirrors GitHub Actions (nextest, `--locked`, examples with `--all-features`, docs with `-D warnings`). Run `just --list` for the full recipe set, including `release`, `miri`, and the `nextest-locked*` variants.

### Setting Up SQL Server for Testing

```bash
# Start SQL Server in Docker (recommended)
just sql-server-start

# Or start all versions (2017, 2019, 2022) for compatibility testing
just sql-server-all

# Check container status / stop containers
just sql-server-status
just sql-server-stop
```

Environment variables (set automatically by the just recipes):
```bash
export MSSQL_HOST=localhost
export MSSQL_PORT=1433
export MSSQL_USER=sa
export MSSQL_PASSWORD=YourStrong@Passw0rd
```

### Build Automation (`cargo xtask`)

The project includes custom build commands via `cargo xtask`:

| Command | Purpose |
|---------|---------|
| `cargo xtask ci` | Run format, lint, test, and deny checks |
| `cargo xtask ci-local` | Full CI pipeline locally (mirrors GitHub Actions) |
| `cargo xtask release <version>` | Bump version across all crates + update CHANGELOG |
| `cargo xtask release-notes` | Generate a CHANGELOG draft from conventional commits since last tag |
| `cargo xtask check-features` | Validate all feature flag combinations compile (uses cargo-hack) |
| `cargo xtask fuzz` | Run fuzz tests on protocol parser |
| `cargo xtask coverage` | Generate code coverage report |
| `cargo xtask semver` | Check for semver-breaking API changes |

Maintainers preparing a release should follow [RELEASING.md](RELEASING.md), which documents the release-validation recipes (`just release-status`, `release-preflight`, `release-check`, `doc-consistency`, `tag`) and the Cardinal Rules that govern them.

## Making Changes

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`.

**Scopes** map to the crates: `client`, `pool`, `protocol`, `types`, `derive`, `auth`, `tls`, `codec`.

**Examples:**
```
feat(client): add streaming query support
fix(pool): prevent connection leak on timeout
```

Do not include AI-tool branding (e.g. `Co-Authored-By` trailers naming an AI assistant) in commit messages — CI rejects them. See [AI-Assisted Contributions](#ai-assisted-contributions).

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation changes
- `refactor/description` - Code refactoring

## Breaking Changes

Breaking changes are changes that may require users to modify their code when upgrading.

### What Constitutes a Breaking Change

**Definitely Breaking:**
- Removing a public API (function, struct, enum, trait)
- Changing a function signature (parameters, return type)
- Changing struct field types or visibility
- Removing enum variants
- Changing trait definitions
- Removing or renaming feature flags

**Usually Breaking:**
- Adding required fields to public structs (without `#[non_exhaustive]`)
- Changing error types or variants
- Changing default behavior

**Not Breaking:**
- Adding new public APIs
- Adding optional parameters with defaults
- Adding new enum variants (if `#[non_exhaustive]`)
- Bug fixes (even if code depended on buggy behavior)
- Performance improvements
- Documentation changes
- **MSRV increases** — see [STABILITY.md § MSRV Increase Policy](STABILITY.md#minimum-supported-rust-version-msrv). MSRV bumps are allowed in minor releases when necessary for security fixes, critical bug fixes, or features requiring new language/stdlib capabilities. This aligns with the broader Rust ecosystem (Tokio, serde, etc.) and is consistent with our rolling 6-month MSRV window.

### Policy

[STABILITY.md](STABILITY.md) is authoritative for the versioning and breaking-change policy. In short: during 0.x, breaking changes are allowed in minor bumps (0.x.0) and must be documented in [CHANGELOG.md](CHANGELOG.md) under "Breaking Changes" (with a migration note when the change is significant). Discuss a non-trivial breaking change in an issue before implementing it. The PR template has a breaking-change checklist to fill in.

## AI-Assisted Contributions

AI coding tools are welcome here — this project is itself built with them. What
we ask is **disclosure and accountability**, not abstinence:

- **Disclose** meaningful AI assistance in your PR description (one sentence is
  enough). You don't need to flag routine autocomplete.
- **Stand behind your code.** You should understand every line you submit and be
  able to explain *why* it's correct when a reviewer asks. "The AI wrote it" is
  not an answer a reviewer can act on.
- **Do the review pass yourself first.** Run the tests, read the diff, and
  confirm it actually does what the PR claims before opening it.

Contributions that are clearly unreviewed generated output — plausible-looking
code the author can't explain, or PRs that don't compile or pass tests — will be
closed. This isn't about where code comes from; it's about whether a human is
accountable for it. See [Coding Standards](#coding-standards) for the quality bar.

## Pull Request Process

1. **Before submitting:** run `just ci-all`. It runs the full gate (tests, clippy `-D warnings`, `cargo fmt --check`, doc build, doc-consistency) in one command — the same checks CI runs.
2. **Describe the PR using the [template](.github/pull_request_template.md).** It covers the summary, linked issues, type of change, test plan, documentation updates, breaking-change details (pointing to STABILITY.md), and security considerations.
3. **Review:** at least one approval plus green CI on all three platforms (Linux, macOS, Windows). CODEOWNERS automatically requests review from the right maintainers based on the files you touched.

## When Your PR Needs Review

CODEOWNERS automatically requests review based on the files you touched — you don't need to tag anyone. For substantial or architectural changes (roughly, PRs over ~500 lines), open a **draft PR early** for direction before polishing the implementation; large feature PRs naturally take longer to review than small ones.

## Coding Standards

- **Safety:** `unsafe` is denied by default (`unsafe_code = "deny"`); any necessary unsafe must be documented with a `// SAFETY:` rationale and covered by Miri tests where possible.
- **Panics:** never panic in library code — return `Result<T, Error>`. Error types use `thiserror`.
- **API evolution:** add `#[non_exhaustive]` to public enums and structs that may grow; `#[must_use]` where the return value matters.
- **Docs:** all public APIs must be documented (`missing_docs` is enforced in CI).
- **Performance:** use `Arc<Bytes>` for shared row data; profile before optimizing.

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) for anything not covered here.

## Testing

### Test Organization

- Unit tests: in the same `src/*.rs` file
- Integration tests: `tests/`
- Property tests: `proptest`
- Fuzz tests: `fuzz/`

#### Integration Test Taxonomy (`crates/mssql-client/tests/`)

Tests are organized by functional domain:

| File | Domain |
|------|--------|
| `integration.rs` | End-to-end workflows (queries, transactions, streaming) |
| `config.rs` | Configuration validation and connection string parsing |
| `error_handling.rs` | Error scenarios and recovery paths |
| `always_encrypted.rs` | Always Encrypted column encryption |
| `azure_sql.rs` | Azure SQL Gateway redirect/failover |
| `resilience.rs` | Connection resilience (retry, timeout, backoff) |
| `stress.rs` | Load testing and concurrent operations |
| `version_compatibility.rs` | TDS 7.3 through 8.0 support |
| `protocol_conformance.rs` | MS-TDS specification compliance |
| `collation_test.rs` | Character encoding and collation handling |
| `edge_cases.rs` | Boundary conditions (empty results, large values, etc.) |

### Test Patterns for Database Drivers

Tests that require a live SQL Server are marked `#[ignore = "Requires SQL Server"]`. They run automatically in CI (with Docker SQL Server) and are skipped locally unless you run `cargo test -- --ignored` (start a server first with `just sql-server-start`).

```rust
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_execution() {
    // This test requires SQL Server to be running
}
```

Documentation examples that need an async runtime or a database connection use ` ```rust,ignore ` — they are syntax-checked by `cargo doc` but not executed. This is standard practice for database-driver docs.

All bug fixes should include a regression test; encoding/decoding paths should have property tests.

## Documentation

- Public items need doc comments. Document behavior, `# Errors`, and `# Panics` where relevant, with an example for non-trivial APIs.
- Keep README examples, feature tables, and compatibility tables current when you change the relevant behavior.

## Architecture Decision Records (ADRs)

[ARCHITECTURE.md](ARCHITECTURE.md) holds the architectural decisions that guide the project's design, in ADR form — it is the canonical home for both the ADR format and the full list of accepted ADRs.

### When to Create an ADR

Create a new ADR when:
- Adding a new major dependency
- Changing how a core component works
- Making a significant performance trade-off
- Changing security boundaries or trust model
- Selecting between multiple reasonable approaches

### ADR Process

1. **Propose**: open a PR adding the new ADR to ARCHITECTURE.md (follow the ADR format used there).
2. **Discuss**: get feedback from maintainers.
3. **Refine**: update based on feedback.
4. **Accept**: merge when approved.
5. **Implement**: reference the ADR in relevant code changes.

## Questions?

- Open a [GitHub Discussion](https://github.com/praxiomlabs/rust-mssql-driver/discussions) for questions
- Check existing issues and PRs

Thank you for contributing!
