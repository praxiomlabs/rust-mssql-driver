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
3. Add upstream remote: `git remote add upstream https://github.com/rust-mssql-driver/rust-mssql-driver.git`
4. Create a feature branch: `git checkout -b feature/your-feature-name`

## Development Setup

### Prerequisites

- Rust 1.85+ (2024 Edition)
- Docker (for integration tests)
- SQL Server instance (or use Docker)

### Building

```bash
# Build all crates
cargo build --workspace

# Build with all features
cargo build --workspace --all-features

# Run the build automation
cargo xtask build
```

### Running Tests

```bash
# Unit tests only
cargo test --workspace

# Integration tests (requires SQL Server)
cargo xtask test

# With coverage
cargo xtask coverage
```

### Setting Up SQL Server for Testing

```bash
# Start SQL Server in Docker
docker run -e "ACCEPT_EULA=Y" -e "SA_PASSWORD=YourStrong@Passw0rd" \
    -p 1433:1433 --name mssql-test \
    -d mcr.microsoft.com/mssql/server:2022-latest

# Set environment variables
export MSSQL_HOST=localhost
export MSSQL_USER=sa
export MSSQL_PASSWORD=YourStrong@Passw0rd
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
