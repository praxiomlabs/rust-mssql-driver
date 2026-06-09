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

### Build Automation

`just` is the canonical task runner — run `just --list` for the full recipe
catalog. The common workflows:

| Command | Purpose |
|---------|---------|
| `just ci` | Format check, clippy, tests, doc check, examples |
| `just ci-all` | Same with all features (matches GitHub Actions) |
| `just test` / `just test-all` | Run the test suite |
| `just fuzz-all` | Run all fuzz targets briefly (smoke test) |
| `just coverage` | Generate code coverage report |
| `just semver` | Check for semver-breaking API changes |

The `xtask` crate carries only `cargo xtask check-features` (validates all
feature flag combinations compile via cargo-hack; used by CI).

Releases are automated by release-plz — see [RELEASING.md](RELEASING.md). Your commit messages drive version bumps and the CHANGELOG, which is why the [conventional commit format](#commit-messages) matters.

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

## Dependency Policy

This section captures the project's dependency-management philosophy. It's
written for maintainers making routine decisions (taking a dep bump, accepting
a new dep, ignoring an advisory) and for contributors proposing PRs that touch
`Cargo.toml`, `deny.toml`, or `.cargo/audit.toml`.

The policy distills decisions that were previously tribal knowledge into
written guidance. Where possible, it references the concrete precedents that
informed the decision — in particular the v0.7.0 release cycle, which forced
several judgment calls that are now documented as precedent here.

### Philosophy

1. **Minimize surface area.** Every dependency is code we are responsible for.
   A dep that's "fine today" can become unmaintained, CVE-ridden, or licensed
   incompatibly at any time. Prefer the standard library where it's adequate,
   well-maintained crates where it isn't.

2. **Correctness over convenience.** If a dep offers a nicer API but has
   questionable maintenance, unclear licensing, or a history of security
   incidents, don't adopt it just because it's ergonomic.

3. **Modern Rust, not cutting-edge Rust.** The project targets a rolling
   6-month MSRV window aligned with Tokio's policy (see
   [ARCHITECTURE.md § 6.6](ARCHITECTURE.md) and [STABILITY.md § MSRV](STABILITY.md#minimum-supported-rust-version-msrv)).
   Some deps bump their MSRV aggressively; we keep up but don't chase.

4. **Prefer pure Rust where it matters.** The TLS stack is [rustls](https://github.com/rustls/rustls),
   not OpenSSL. JSON is `serde_json`, not a C wrapper. This is partly about
   audit surface, partly about cross-compilation ergonomics, partly about
   avoiding the licensing tangles of mixed-license C code.

5. **Minimize feature flags on deps.** `tokio = { version = "1", features = ["rt", "net", "io-util"] }`
   is better than `tokio = { version = "1", features = ["full"] }`. We already
   did this cleanup in v0.7.0 — see commit `0d07504`.

### Adding a new dependency

Before adding a new dep, check:

| Criterion | How to check |
|-----------|--------------|
| **Necessary** | Is there a way to do this with the stdlib or existing deps? If the new dep only saves ~50 lines of code, it may not be worth the audit surface. |
| **Maintained** | [crates.io](https://crates.io/) lists recent releases, the GitHub repo has recent commits, issues are being triaged. "One maintainer, last commit 2 years ago" is a red flag. |
| **Popular** | Downloads and reverse-dep count on [crates.io](https://crates.io/). Popular isn't a guarantee of quality, but unpopular deps carry higher risk of being abandoned. |
| **License** | Must be in the `allow` list in [`deny.toml`](deny.toml). MIT / Apache-2.0 / BSD-* / ISC are always fine. Unusual licenses require adding a new allow entry and documenting why. |
| **MSRV** | The dep's minimum Rust version must be ≤ our workspace MSRV, OR we must be willing to bump our MSRV (rare). |
| **Transitive deps** | Look at `cargo tree -p <your-crate>`. If the new dep pulls in 50 transitive crates, that's 50 new audit surfaces. Check if alternatives exist with leaner dep trees. |
| **Feature flags** | Only enable the features we actually use. Never use `features = ["full"]` blindly. |
| **Security history** | [RustSec advisory DB](https://rustsec.org/) — search for the crate name. A crate with a single old fixed advisory is fine; a crate with recurring advisories is not. |
| **no_std compatibility** | `tds-protocol` is `no_std + alloc` compatible. Deps in `tds-protocol` must not require `std`. Check `Cargo.toml` features for `std` / `alloc` flags. |

If you're unsure, open an issue asking before adding the dep. We'd rather have
the discussion up front than revert a PR after merge.

### Taking a dependency upgrade

Routine dep bumps arrive via Dependabot on Monday mornings. The workflow is:

1. **Patch bumps** (e.g., `0.18.6 → 0.18.7`): usually safe to merge if CI is
   green. Dependabot's PR shows the changelog diff.
2. **Minor bumps** (e.g., `0.18.x → 0.19.0` for 0.x crates, or `1.5.x → 1.6.0`
   for 1.x crates): read the changelog. 0.x crates can break API in minor
   bumps per SemVer. Check CI status carefully.
3. **Major bumps** (e.g., `1.x → 2.0`): treat as a mini-project. Read the
   upgrade guide. Expect code changes. May want to defer to a dedicated PR
   rather than merging dependabot's raw output.

#### When to defer

Defer a dep bump when:

- It introduces breaking API changes that require adapter code in our
  codebase. Better to do this in a focused PR than a dependabot rebase loop.
- It bumps a transitive dep we don't want to bump yet (e.g., because our
  MSRV can't handle the transitive dep's MSRV).
- The changelog mentions behavior changes that need audit attention (e.g.,
  crypto library updates, parser rewrites).

#### Bundled bumps

Dependabot is configured to group related deps in
[`.github/dependabot.yml`](.github/dependabot.yml):

- `opentelemetry*` crates move together (version alignment is required — see
  [ARCHITECTURE.md § ADR-008](ARCHITECTURE.md#adr-008-opentelemetry-version-alignment))
- `tokio` minor/patch bumps move together

Add new groupings when we discover another "moves together" set.

### Handling security advisories

The weekly [Security Audit workflow](.github/workflows/security-audit.yml)
runs `cargo audit` and `cargo deny check`. When a new RUSTSEC advisory
lands in our dep tree, one of three things is true:

#### Case A: An upgrade is available AND we can take it

This is the easy path. Run `cargo update` (or let Dependabot open a PR),
verify CI passes, merge. Close any tracking issue.

**Precedent**: v0.7.0 resolved 6 of 7 RUSTSEC advisories this way. The
`aws-lc-sys 0.35 → 0.39`, `rustls 0.23.36 → 0.23.37`, and `rustls-webpki
0.103.8 → 0.103.10` bumps all came from a single `cargo update` after
bumping MSRV.

#### Case B: An upgrade is available BUT requires an MSRV bump

This is the tricky case. If the fix is in a new version that requires a
newer Rust, we must choose between bumping MSRV (to take the fix) or
ignoring the advisory (to hold MSRV).

**Policy**: bumping MSRV is permitted for security fixes per
[STABILITY.md § MSRV Increase Policy](STABILITY.md#minimum-supported-rust-version-msrv).
MSRV bumps are _not_ considered breaking changes. Prefer the upgrade.

**Precedent**: v0.7.0 bumped MSRV from 1.85 to 1.88 specifically to take
`time 0.3.47` (RUSTSEC-2026-0009). The alternative would have been adding
`RUSTSEC-2026-0009` to the deny.toml ignore list with a "not exploited in
our use case" justification. We chose the MSRV bump because:

1. The project explicitly aims for "modern Rust" (see
   [CLAUDE.md](CLAUDE.md) and [ARCHITECTURE.md](ARCHITECTURE.md)).
2. Rust 1.88 had been stable for ~10 months, well within the 6-month rolling
   MSRV window aligned with Tokio's policy.
3. The deny.toml has historically preferred fixing over ignoring.
4. Pre-1.0 (0.7.0) is appropriate timing for MSRV bumps.

The MSRV bump unlocked other pinned deps as a side benefit (the `home`
crate pin was removed because MSRV 1.88 supports the newer version).

#### Case C: No upgrade is available OR the affected code path is not reachable

Sometimes a crate has an unfixed advisory that's either not yet patched
upstream, or patched only in a pre-release. Sometimes the vulnerable
function isn't called from our code paths.

**Policy**: these may be added to the `[advisories] ignore` list in
`deny.toml`, but **only with**:

1. **A documented reason** — explain exactly why we're ignoring and
   what the exposure actually is in _our_ use.
2. **A tracking link** — usually an upstream issue, GitHub PR, or
   RustSec advisory link. If the issue has no tracking, create a local
   tracking issue first.
3. **A review trigger** — note a condition under which we'll reevaluate
   (e.g., "when `<upstream-crate>` 1.0 stabilizes", "when we upgrade to
   `<other-crate>` 2.0 which replaces this code path").
4. **Synced entries in `.cargo/audit.toml`** — `cargo audit` reads from
   `.cargo/audit.toml`, `cargo deny` reads from `deny.toml`. The
   doc-consistency linter catches when they drift (see
   `scripts/check-doc-consistency.sh`).

**Precedents in `deny.toml`** (as of v0.7.0):

- `RUSTSEC-2023-0071` (RSA Marvin timing attack): no upstream fix available,
  our RSA usage is client-side-local (CEK unwrapping, SSPI auth), not
  network-observable. Tracked at [RustCrypto/RSA#390](https://github.com/RustCrypto/RSA/issues/390).
- `RUSTSEC-2025-0134` (rustls-pemfile unmaintained): pulled indirectly via
  bollard (a dev-only testcontainers dep), migration path pending upstream.
- `RUSTSEC-2026-0066` (astral-tokio-tar PAX validation): only reachable via
  `testcontainers` → `mssql-testing` (publish=false). Low CVSS (1.7). Fix
  available only in a pre-release (0.6.1-rc1).

When in doubt, **upgrade rather than ignore**. The cost of a deferred MSRV
bump is usually lower than the long-term cost of documentation drift and
advisory re-review.

### deny.toml and .cargo/audit.toml sync

Both files ignore the same set of advisories; their formats differ but the
content must match. The [`scripts/check-doc-consistency.sh`](scripts/check-doc-consistency.sh)
linter verifies this automatically. If you add an ignore, always add it to
both files and verify the linter passes.

### Removing a dependency

When removing a dep:

1. **Delete from the relevant Cargo.toml** (workspace or crate).
2. **Run `cargo machete`** to verify no other crate is still using it.
3. **Run `cargo update`** to clean up the lockfile.
4. **Review transitive deps** — did the removal free up other transitive
   deps that are now unused? If so, those should also exit the lockfile
   after `cargo update`.
5. **Check deny.toml** — if the removed dep (or a transitive dep that left
   the tree) was in a `skip-tree` or `skip` entry, clean that up too.

Don't leave dead deps sitting in `Cargo.toml` "just in case". Future you
can always re-add them.

### New license requirements

New license strings appear periodically. The authoritative list of allowed
licenses is in [`deny.toml`](deny.toml) under `[licenses] allow`. When a
cargo-deny run fails with `license-not-allowed`:

1. **Check what the license actually is.** Run `cargo info <crate>` and
   look at the `license:` field. SPDX expressions like `MIT OR Apache-2.0`
   are common.
2. **Decide if it's acceptable.** The current policy allows all OSI-approved
   permissive licenses (MIT, Apache-2.0, BSD-2, BSD-3, ISC, Zlib, CC0,
   BSL-1.0, MIT-0, Unicode-3.0, CDLA-Permissive-2.0). Copyleft licenses
   (GPL, AGPL, LGPL) are _not_ acceptable for this project.
3. **If acceptable**, add the license string to the `allow` list with a
   comment explaining which crate pulls it in.
4. **If not acceptable**, find an alternative crate or file an upstream
   issue asking for license clarification.

**Precedent**: v0.7.0 added `MIT-0` to the allow list because the newer
`aws-lc-sys 0.39.1` introduced MIT-0-licensed source files. The comment in
`deny.toml` records this.

### Maintenance schedule

- **Weekly**: Dependabot opens minor/patch bump PRs (Monday 09:00 UTC).
- **Weekly**: Security Audit workflow runs (Monday 09:00 UTC), cross-checks
  `cargo audit` + `cargo deny` against RustSec database.
- **Per release**: Run `cargo update` once before the release cycle so
  transitive bumps land on `dev` with time for CI to catch regressions.
  Don't run `cargo update` inside the release prep commit itself — do it
  days earlier so you have time to respond if something breaks.
- **Quarterly-ish**: Review `cargo outdated` output for major-version
  bumps that Dependabot won't open automatically (outdated major versions).
  Decide case-by-case whether to tackle them.

## Questions?

- Open a [GitHub Discussion](https://github.com/praxiomlabs/rust-mssql-driver/discussions) for questions
- Check existing issues and PRs

Thank you for contributing!
