# Release Readiness Checklist

A comprehensive checklist for validating release readiness of rust-mssql-driver. This extracts specific validation steps, consistency checks, and "blast radius" considerations required to bring the repository to release-ready standard.

---

## 0. Pre-flight Checks

Quick verification before detailed review:

```bash
# Option A: Use just recipe (recommended)
just ci

# Option B: Manual commands
git status  # Should show no uncommitted changes (or only expected ones)
cargo check --workspace --all-features --all-targets
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets -- -D warnings
```

- [ ] Git working directory is clean (or changes are intentional)
- [ ] CI is passing on the target branch
- [ ] Local build/test/lint all pass (`just ci`)

---

## 1. Codebase Hygiene & Safety

### Work-in-Progress Markers

```bash
# Use just recipe (recommended)
just wip-check

# Manual commands
grep -rn "TODO\|FIXME\|XXX\|HACK" --include="*.rs" crates/
grep -rn "todo!\|unimplemented!" --include="*.rs" crates/
```
- [ ] Run `just wip-check` or grep for `TODO`, `FIXME`, `XXX`, `HACK` comments
- [ ] Verify no `todo!()`, `unimplemented!()` macros in production code
- [ ] Ensure no incomplete logic ships to production

### Panic Path Audit

```bash
# Use just recipe (recommended)
just panic-audit

# Manual commands
grep -rn "\.unwrap()" crates/*/src/ --include="*.rs"
grep -rn "\.expect(" crates/*/src/ --include="*.rs"
```

- [ ] Run `just panic-audit` to audit `.unwrap()` and `.expect()` calls
- [ ] **Note:** High line numbers often indicate test modules - verify context
- [ ] Verify all production panic paths have documented justification

### Dead Code Analysis
- [ ] Review `#[allow(dead_code)]` suppressions
- [ ] Ensure suppressions are either documented (reserved for future use) or removed
- [ ] Check for unused imports and dependencies

### Strict Linting

```bash
just clippy          # Standard linting
just clippy-strict   # Pedantic linting
```

- [ ] Run `just clippy` (warnings-as-errors)
- [ ] Address non-idiomatic patterns
- [ ] Verify all feature flag combinations pass linting

---

## 2. Version Consistency (The "Blast Radius")

```bash
just version-sync    # Verify README + docs match Cargo.toml
```

### Core Manifests
- [ ] Bump version in root `Cargo.toml` (workspace.package.version)
- [ ] Verify all workspace members inherit version correctly
- [ ] Update `CHANGELOG.md` with new version section

### Documentation Version References
Run `just version-sync` then grep for old version strings:
- [ ] README.md installation instructions
- [ ] docs/*.md files
- [ ] Migration guides
- [ ] Getting started examples

### Crate/Package Name Consistency
- [ ] Verify all documentation references correct package names
- [ ] Check ASCII diagrams and architecture docs for accuracy
- [ ] Update code examples in documentation

### Example Projects
- [ ] Ensure `examples/` compile and run correctly
- [ ] Verify examples use current API patterns

---

## 3. Environment & Infrastructure Alignment

### Minimum Supported Version (MSRV) Sync

```bash
just msrv-check    # Verify code compiles with declared MSRV
```

Ensure MSRV is consistent across **all** locations:
- [ ] CI configuration (workflow files)
- [ ] `CONTRIBUTING.md` and prerequisites docs
- [ ] `Cargo.toml` rust-version field (1.85)
- [ ] README.md prerequisites section

### CI Configuration Validity
- [ ] Verify CI tool paths match current project structure
- [ ] Check `codecov.yml` / coverage config points to valid directories
- [ ] Ensure all crates are included in coverage reporting
- [ ] Verify SQL Server Docker container setup in CI

### SQL Server Test Infrastructure
- [ ] SQL Server 2022 container configuration verified
- [ ] SQL Server 2019 compatibility tested
- [ ] SQL Server 2017 compatibility tested
- [ ] Environment variables documented (MSSQL_HOST, MSSQL_USER, etc.)

---

## 4. Dependency & Security Compliance

### Vulnerability Scan

```bash
just deny     # Run cargo-deny (licenses, bans, advisories)
just audit    # Run cargo-audit (security vulnerabilities)
```

- [ ] Run `just deny` (comprehensive: licenses, bans, advisories)
- [ ] Review and address all advisories
- [ ] **Note:** `duplicate` warnings are informational (common in large dependency trees)

### Advisory Documentation
- [ ] If ignoring an advisory, document rationale in `deny.toml`
- [ ] Include advisory ID, explanation, and user guidance

### License Compliance
- [ ] Verify no new dependencies violate licensing policy (MIT OR Apache-2.0)
- [ ] Check transitive dependencies

---

## 5. Documentation Integrity

### Link Validation

```bash
just link-check    # Uses lychee if installed
just doc-check     # Check documentation builds without warnings
```

- [ ] Run `just link-check` (or verify CI passed)
- [ ] Run `just doc-check` to verify docs build without warnings
- [ ] Verify internal relative links resolve
- [ ] Check external links haven't gone stale

### Structural Documentation
- [ ] Update ASCII art/diagrams when architecture changes
- [ ] Verify ARCHITECTURE.md reflects current structure
- [ ] Check dependency graphs are accurate

### Changelog Maintenance
- [ ] Move "Unreleased" changes to versioned header
- [ ] Add release date
- [ ] Ensure semantic versioning adherence
- [ ] Include all breaking changes prominently

### API Documentation
- [ ] All public items have doc comments
- [ ] Examples in doc comments compile (`cargo test --doc`)
- [ ] Security-sensitive APIs have warnings documented

---

## 6. Final Build Verification

```bash
just ci-release    # Full release CI pipeline
```

### Clean Build
```bash
just check    # Fast type check
just build    # Full debug build
```
- [ ] Verify clean compilation with all feature combinations

### Test Suite
```bash
just test              # Standard test run
just test-integration  # SQL Server integration tests
just test-all-versions # Test against SQL Server 2017/2019/2022
```
- [ ] All unit tests pass (`just test`)
- [ ] All integration tests pass (`just test-integration`)
- [ ] Multi-version compatibility verified (`just test-all-versions`)
- [ ] No flaky tests

### Linting (Final Pass)
```bash
just clippy    # Standard: warnings-as-errors
```
- [ ] Zero warnings
- [ ] All feature flag combinations pass

### Example Compilation
```bash
just examples    # Build all examples
```
- [ ] Run `just examples` to build all examples
- [ ] Examples execute without errors

---

## 7. API Compatibility & Semver

### Breaking Change Detection

```bash
just semver    # Run cargo-semver-checks
```

- [ ] Run `just semver` or verify CI passed
- [ ] Review any flagged breaking changes
- [ ] Ensure breaking changes warrant version bump (pre-1.0: minor for breaking)

### Public API Surface
- [ ] Audit public exports for unintended exposure
- [ ] Verify `#[doc(hidden)]` items are intentional
- [ ] Check that internal modules aren't accidentally public

### Deprecations
- [ ] Add `#[deprecated]` attributes with migration guidance
- [ ] Document deprecations in CHANGELOG
- [ ] Provide minimum one release cycle warning before removal

---

## 8. Publishing Preparation

### Pre-publish Verification

```bash
just publish-dry    # Dry-run publish all crates in dependency order
```

- [ ] Run `just publish-dry` - all crates succeed
- [ ] No unexpected files included in package
- [ ] Package size is reasonable

### Cargo.toml Metadata

```bash
just metadata-check    # Verify required metadata for crates.io
```

**Required fields:**
- [ ] `description` - concise crate description
- [ ] `license` - "MIT OR Apache-2.0"
- [ ] `repository` - GitHub URL

**Recommended fields:**
- [ ] `keywords` - up to 5 searchable keywords
- [ ] `categories` - crates.io categories
- [ ] `documentation` - docs.rs URL (auto-generated)

### Publishing Order
Crates must be published in dependency order:
1. `tds-protocol` (no internal deps)
2. `mssql-types` (no internal deps)
3. `mssql-tls` (no internal deps)
4. `mssql-codec` (depends on tds-protocol)
5. `mssql-auth` (depends on mssql-types)
6. `mssql-derive` (proc-macro, no internal deps)
7. `mssql-pool` (depends on above)
8. `mssql-client` (depends on all above)
9. `mssql-testing` (depends on mssql-client)

Allow ~30s between publishes for index propagation.

---

## 9. Git & Release Protocol

### Release Workflow (Follow This Order)

**Publishing to crates.io is IRREVERSIBLE.** Follow this exact sequence:

```
+-------------------------------------------------------------+
|  1. PREPARE: Version bump + CHANGELOG + commit              |
|                         |                                   |
|  2. PUSH: git push origin main                              |
|                         |                                   |
|  3. WAIT: CI must pass on main (watch with `gh run watch`)  |
|                         |                                   |
|  4. TAG: just tag (creates v<version>)                      |
|                         |                                   |
|  5. RELEASE: git push origin v<version>                     |
|                         |                                   |
|  6. AUTOMATED: CI publishes to crates.io + GitHub Release   |
+-------------------------------------------------------------+
```

**Step-by-step commands:**

```bash
# Step 1: Prepare (already done if following this checklist)
# - Bump version in Cargo.toml
# - Update CHANGELOG.md with new version section
# - Commit: git commit -m "chore: release v0.1.0"

# Step 2: Push to main
git push origin main

# Step 3: Wait for CI to pass
gh run watch                    # Interactive watch
# OR
gh run list --limit 1           # Check status

# Step 4: Create tag (ONLY after CI passes!)
just tag                        # Creates annotated tag v<version>

# Step 5: Push tag to trigger release
git push origin v<version>      # Triggers release.yml workflow

# Step 6: Monitor release
gh run watch                    # Watch publish workflow
```

### Pre-Tag Checklist

- [ ] Version bumped in Cargo.toml
- [ ] CHANGELOG.md updated with version and date
- [ ] Version bump committed and pushed to main
- [ ] **CI passing on main** (critical - verify before tagging!)

### Tagging

```bash
just tag    # Create annotated tag from Cargo.toml version
```

- [ ] Run `just tag` to create `v<version>` tag
- [ ] Tag matches version in Cargo.toml exactly
- [ ] Tags are annotated (not lightweight)
- [ ] Tag pushed: `git push origin v<version>`

---

## 10. Post-Release Verification

### Publication Verification
- [ ] Crates appear on crates.io
- [ ] Documentation builds on docs.rs
- [ ] Version numbers correct on registry

### Installation Test
```bash
cargo new test-install && cd test-install
cargo add mssql-driver@<new-version>
cargo build
```
- [ ] Fresh installation from registry succeeds
- [ ] Basic functionality works

### Repository Cleanup
- [ ] Update `[Unreleased]` section in CHANGELOG for next cycle
- [ ] Close related milestones/issues

### Announcement
- [ ] Post to relevant channels
- [ ] Update project documentation if applicable

---

## Summary of Workspace Crates

| Crate | Description | Internal Dependencies |
|-------|-------------|----------------------|
| tds-protocol | Pure TDS protocol (no_std) | None |
| mssql-types | SQL <-> Rust type mapping | None |
| mssql-tls | TLS negotiation (rustls) | None |
| mssql-codec | Async framing layer | tds-protocol |
| mssql-auth | Authentication strategies | mssql-types |
| mssql-derive | Proc macros for row mapping | None |
| mssql-pool | Connection pooling | Multiple |
| mssql-client | Public API surface | All above |
| mssql-testing | Test infrastructure | mssql-client |

---

## CI Automation Coverage

The following checks are **automated in CI**:

| Check | CI Job | Manual Needed |
|-------|--------|---------------|
| Format | `fmt` | No |
| Linting | `clippy` | No |
| Tests | `test` | No |
| MSRV | `msrv` | No |
| Cross-platform | `test-matrix` | No |
| Security/license | `deny` | No |
| Semver compliance | `semver` | No |
| Doc build | `docs` | No |
| Code coverage | `coverage` | No |
| Integration tests | `integration` | No |
| Publish to crates.io | `release.yml` | Tag triggers |
| GitHub Release | `release.yml` | Tag triggers |

**Still requires manual verification:**
- Grep for old versions in all docs
- Post-release installation test
- Announcement/communication

---

## Justfile Recipe Mapping

| Checklist Section | Just Recipe(s) | What It Covers |
|-------------------|----------------|----------------|
| **0. Pre-flight** | `just ci` | fmt, clippy, test, doc-check |
| **1. Code Hygiene** | `just wip-check` | TODO/FIXME/XXX/HACK, todo!/unimplemented! |
| **1. Code Hygiene** | `just panic-audit` | .unwrap()/.expect() in production code |
| **2. Version Consistency** | `just version-sync` | README version check |
| **3. Environment** | `just msrv-check` | MSRV compilation verification |
| **4. Security** | `just deny` | Licenses, bans, advisories |
| **5. Documentation** | `just link-check` | Markdown link validation |
| **5. Documentation** | `just doc-check` | Documentation builds without warnings |
| **6. Build Verification** | `just ci-release` | Full CI + coverage + security + semver |
| **6. Build Verification** | `just test-integration` | SQL Server integration tests |
| **7. Semver** | `just semver` | Breaking change detection |
| **8. Publishing** | `just publish-dry` | Dry-run publish all crates |
| **8. Publishing** | `just metadata-check` | Cargo.toml metadata verification |
| **9. Git Protocol** | `just tag` | Create annotated version tag |
| **9. Git Protocol** | `just release-check` | Full release validation + git state |

**Comprehensive Release Command:**
```bash
just release-check    # Runs: ci-release + wip-check + panic-audit + metadata-check + git checks
```
