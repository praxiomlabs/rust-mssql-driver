# Contributing to rust-mssql-driver

Thank you for your interest in contributing! This document provides guidelines and processes for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Breaking Changes](#breaking-changes)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and constructive in all interactions.

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

### Prerequisites

| Tool | Version | Required | Notes |
|------|---------|----------|-------|
| Rust | 1.85+ | Yes | 2024 Edition |
| Just | 1.23+ | Yes | Command runner |
| jq | any | Yes | JSON parsing |
| Docker | any | No | For integration tests |

### Step-by-Step Setup

#### 1. Check Your Environment

```bash
just setup
```

This shows what's installed and what's missing.

#### 2. Install Cargo Extensions

```bash
just setup-tools
```

Installs version-pinned tools compatible with Rust 1.85:
- `cargo-nextest` - Fast test runner
- `cargo-llvm-cov` - Code coverage
- `cargo-audit` - Security auditing
- `cargo-deny` - License/dependency checking
- `cargo-machete` - Unused dependency detection
- `cargo-semver-checks` - API compatibility
- `cargo-watch` - File watching

#### 3. Install Git Hooks

```bash
just setup-hooks
```

Installs a pre-commit hook that runs:
- Format check (`cargo fmt --check`)
- Clippy lints
- Type check (`cargo check`)

#### 4. Platform-Specific: Linux Kerberos Support

For `--all-features` (integrated authentication):

```bash
# Debian/Ubuntu
sudo apt-get install libkrb5-dev libclang-dev

# RHEL/Fedora
sudo dnf install krb5-devel clang-devel

# Or use the just recipe
just setup-linux
```

The `integrated-auth` feature requires:
- `libkrb5-dev`: Kerberos/GSSAPI headers
- `libclang-dev`: Required by bindgen to generate FFI bindings

This is **Linux-only**.

### Justfile Recipe Naming Convention

This project uses a dual-recipe pattern to handle platform differences:

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

**Use base recipes for day-to-day development.** Use `-all` variants when you need to test Kerberos integration or match CI exactly.

**CI Alignment:** The `just ci-all` recipe mirrors GitHub Actions exactly:
- Uses `cargo-nextest` for tests (same as CI)
- Includes `--locked` flag (ensures Cargo.lock is respected)
- Builds examples with `--all-features`
- Runs documentation checks with `-D warnings`

### Building

```bash
# Build (default features - works everywhere)
just build

# Build with all features (requires libkrb5-dev on Linux)
just build-all

# Build in release mode
just release
```

### Running Tests

```bash
# Unit tests with cargo test (default features)
just test

# Unit tests with cargo test (all features)
just test-all

# Fast parallel tests with cargo-nextest (recommended)
just nextest

# Fast parallel tests with all features
just nextest-all

# Tests with locked dependencies (matches CI)
just nextest-locked
just nextest-locked-all

# Run specific crate tests
just test-crate mssql-client

# Run Miri tests for unsafe code detection (requires nightly)
just miri
```

### Code Quality

```bash
# Format code
just fmt

# Check formatting
just fmt-check

# Run clippy
just clippy

# Full CI pipeline (matches what runs on PRs)
just ci
```

### Setting Up SQL Server for Testing

```bash
# Start SQL Server in Docker (recommended)
just sql-server-start

# Or start all versions (2017, 2019, 2022) for compatibility testing
just sql-server-all

# Check container status
just sql-server-status

# Stop containers when done
just sql-server-stop
```

Environment variables (set automatically by just recipes):
```bash
export MSSQL_HOST=localhost
export MSSQL_PORT=1433
export MSSQL_USER=sa
export MSSQL_PASSWORD=YourStrong@Passw0rd
```

### Watch Mode (Auto-Rebuild on Save)

```bash
# Re-run tests on file changes
just watch

# Re-run type check on file changes
just watch-check

# Re-run clippy on file changes
just watch-clippy
```

## Making Changes

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style (formatting, semicolons, etc.)
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `test`: Adding or correcting tests
- `chore`: Build process or auxiliary tool changes

**Scopes:**
- `client`: mssql-client crate
- `pool`: mssql-pool crate
- `protocol`: tds-protocol crate
- `types`: mssql-types crate
- `derive`: mssql-derive crate
- `auth`: mssql-auth crate
- `tls`: mssql-tls crate
- `codec`: mssql-codec crate

**Examples:**
```
feat(client): add streaming query support
fix(pool): prevent connection leak on timeout
docs(readme): add transaction examples
refactor(protocol): simplify token parsing
```

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
- Increasing MSRV

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

### Breaking Change Policy

#### During 0.x Development

1. Breaking changes are allowed in minor version bumps (0.x.0)
2. All breaking changes must be:
   - Documented in CHANGELOG.md under "Breaking Changes"
   - Accompanied by a migration guide if the change is significant
   - Discussed in a GitHub issue before implementation

#### Post-1.0 Release

1. Breaking changes require a major version bump
2. Deprecated APIs must remain for at least one minor release
3. Breaking changes require:
   - RFC-style discussion for significant changes
   - Approval from maintainers
   - Comprehensive migration documentation

### Proposing a Breaking Change

1. Open an issue with the `breaking-change` label
2. Describe:
   - Current behavior
   - Proposed new behavior
   - Rationale for the change
   - Migration path for existing users
   - Alternatives considered
3. Wait for maintainer feedback before implementing

### Documenting Breaking Changes

In your PR that introduces a breaking change:

1. Add to CHANGELOG.md:
```markdown
### Breaking Changes

- **client**: `Config::new()` removed, use `Config::builder()` instead
  - Migration: Replace `Config::new().host("...")` with `Config::builder().host("...").build()`
```

2. Update any affected examples
3. Update any affected documentation

## Pull Request Process

1. **Before submitting:**
   - Ensure all tests pass: `cargo test --workspace`
   - Run lints: `cargo clippy --workspace --all-targets`
   - Format code: `cargo fmt --all`
   - Update documentation if needed

2. **PR Description should include:**
   - Summary of changes
   - Related issue number (if any)
   - Breaking changes (if any)
   - Testing performed

3. **Review process:**
   - At least one maintainer approval required
   - All CI checks must pass
   - Breaking changes require additional review

4. **After approval:**
   - Squash commits if requested
   - Maintainer will merge

## Coding Standards

### General Guidelines

- Follow Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- Use `#[must_use]` for functions with important return values
- Prefer `impl Trait` in return position for complex types
- Document all public APIs
- Add `#[non_exhaustive]` to public enums and structs that may grow

### Error Handling

- Use `thiserror` for error types
- Provide context in error messages
- Never panic in library code (except for unrecoverable bugs)
- Use `Result<T, Error>` for fallible operations

### Unsafe Code

- `unsafe` code is denied by default
- Any necessary unsafe must be:
  - Thoroughly documented
  - Reviewed by maintainers
  - Covered by miri tests if possible

### Performance

- Avoid unnecessary allocations
- Use `Arc<Bytes>` for shared data
- Profile before optimizing
- Document performance-critical code

## Testing

### Test Organization

- Unit tests: `src/*.rs` (in the same file)
- Integration tests: `tests/`
- Property tests: Use `proptest`
- Fuzz tests: `fuzz/` directory

### Test Naming

```rust
#[test]
fn should_[expected_behavior]_when_[condition]() {
    // Arrange
    // Act
    // Assert
}
```

### Test Coverage

- Aim for high coverage of public APIs
- All bug fixes should include a regression test
- Property tests for encoding/decoding

### Test Patterns for Database Drivers

This project uses specific patterns for tests and documentation examples:

#### Integration Tests with `#[ignore]`

Tests that require a live SQL Server are marked with `#[ignore = "Requires SQL Server"]`:

```rust
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_query_execution() {
    // This test requires SQL Server to be running
}
```

These tests:
- Run automatically in CI (with Docker SQL Server)
- Are skipped locally unless you run `cargo test -- --ignored`
- Can be run locally with `just sql-server-start` first

#### Doc Examples with `rust,ignore`

Documentation examples that require async runtime or SQL Server use `rust,ignore`:

```rust
/// Execute a query against the database.
///
/// # Examples
///
/// ```rust,ignore
/// let mut client = Client::connect(config).await?;
/// let rows = client.query("SELECT 1", &[]).await?;
/// ```
pub async fn query(&mut self, sql: &str) -> Result<QueryResult, Error> {
    // ...
}
```

Why `ignore`?
- **Async context**: Examples need `#[tokio::main]` or async runtime
- **SQL Server connection**: Examples would fail without a database
- **Illustrative**: The code shows usage patterns, not runnable tests

This is standard practice for database driver documentation. The examples are
syntactically checked by `cargo doc` but not executed as tests.

## Documentation

### Doc Comments

```rust
/// Brief description of the item.
///
/// Longer description with details about behavior,
/// panics, errors, and examples.
///
/// # Arguments
///
/// * `param` - Description of parameter
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// * `Error::Kind` - When this error occurs
///
/// # Examples
///
/// ```rust
/// let result = function(arg);
/// assert!(result.is_ok());
/// ```
pub fn function(param: Type) -> Result<Output, Error> {
    // ...
}
```

### README Updates

- Keep examples up to date
- Update feature tables when adding features
- Ensure compatibility tables are current

## Architecture Decision Records (ADRs)

ARCHITECTURE.md contains architectural decisions that guide the project's design. When making significant architectural changes, you should document them as ADRs.

### When to Create an ADR

Create a new ADR when:
- Adding a new major dependency
- Changing how a core component works
- Making a significant performance trade-off
- Changing security boundaries or trust model
- Selecting between multiple reasonable approaches

### ADR Format

ADRs follow this format in ARCHITECTURE.md:

```markdown
### ADR-NNN: Title

**Status**: Proposed | Accepted | Deprecated | Superseded by ADR-XXX
**Date**: YYYY-MM-DD

**Context**: What is the issue we're addressing?

**Decision**: What have we decided to do?

**Consequences**: What are the trade-offs?

**Alternatives Considered**: What else was considered?
```

### ADR Process

1. **Propose**: Open a PR with the new ADR in ARCHITECTURE.md
2. **Discuss**: Get feedback from maintainers and community
3. **Refine**: Update based on feedback
4. **Accept**: Merge PR when approved
5. **Implement**: Reference the ADR in relevant code changes

### Existing ADRs

| ADR | Topic |
|-----|-------|
| ADR-001 | Tokio as sole runtime |
| ADR-002 | TDS 8.0 first-class support |
| ADR-003 | Built-in connection pooling |
| ADR-004 | Arc<Bytes> for row data |
| ADR-005 | IO splitting for cancellation |
| ADR-006 | Authentication strategy pattern |
| ADR-007 | Type-state pattern for connections |
| ADR-008 | OpenTelemetry version alignment |
| ADR-009 | rustls for TLS |
| ADR-010 | thiserror for error handling |
| ADR-011 | Minimum version constraints |
| ADR-012 | Retry policy design |
| ADR-013 | Always Encrypted roadmap |

See ARCHITECTURE.md for full details on each decision.

## Questions?

- Open a GitHub Discussion for questions
- Join the community chat (if available)
- Check existing issues and PRs

Thank you for contributing!
