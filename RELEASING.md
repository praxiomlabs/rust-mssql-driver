# Releasing rust-mssql-driver

Comprehensive guide for releasing new versions of rust-mssql-driver to crates.io.

---

## Quick Start

For routine releases, use the automated workflow:

```bash
# 1. Validate everything is ready
just release-check

# 2. Bump version and update CHANGELOG (manual edit)
#    - Edit Cargo.toml: workspace.package.version = "X.Y.Z"
#    - Update CHANGELOG.md with release date

# 3. Commit, push, and wait for CI
git add Cargo.toml CHANGELOG.md
git commit -m "chore: release vX.Y.Z"
git push origin main
gh run watch  # Wait for CI to pass

# 4. Tag and release
just tag                    # Creates annotated tag vX.Y.Z
git push origin vX.Y.Z      # Triggers automated publish
```

---

## Table of Contents

1. [Version Numbering](#version-numbering)
2. [Crate Dependency Graph](#crate-dependency-graph)
3. [Pre-Release Checklist](#pre-release-checklist)
4. [Release Workflow](#release-workflow)
5. [Manual Publishing](#manual-publishing)
6. [Post-Release Verification](#post-release-verification)
7. [CI Automation Coverage](#ci-automation-coverage)
8. [Justfile Recipe Reference](#justfile-recipe-reference)
9. [Troubleshooting](#troubleshooting)
10. [Platform-Specific Notes](#platform-specific-notes)

---

## Version Numbering

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR.MINOR.PATCH** (e.g., 1.2.3)
- **Pre-1.0**: Minor bumps may contain breaking changes
- **Post-1.0**: Strictly follow semver

---

## Crate Dependency Graph

Understanding the dependency structure is critical for correct publish order.

```
Tier 0 (Independent - no internal deps):
├── tds-protocol
└── mssql-types

Tier 1 (Depend on Tier 0):
├── mssql-tls      → tds-protocol
├── mssql-codec    → tds-protocol
└── mssql-auth     → tds-protocol

Tier 2 (Proc-macro, no internal runtime deps):
└── mssql-derive   (dev-dep on mssql-client, but NOT runtime)

Tier 3 (Main client):
└── mssql-client   → tds-protocol, mssql-tls, mssql-codec, mssql-types, mssql-auth
                     (dev-dep on mssql-derive, mssql-driver-pool)

Tier 4 (Depend on mssql-client):
├── mssql-driver-pool → mssql-client
└── mssql-testing     → mssql-client
```

### Summary Table

| Crate | Description | Internal Dependencies |
|-------|-------------|----------------------|
| tds-protocol | Pure TDS protocol (no_std) | None |
| mssql-types | SQL ↔ Rust type mapping | None |
| mssql-tls | TLS negotiation (rustls) | tds-protocol |
| mssql-codec | Async framing layer | tds-protocol |
| mssql-auth | Authentication strategies | tds-protocol |
| mssql-derive | Proc macros for row mapping | None (dev-dep on mssql-client) |
| mssql-client | Public API surface | All Tier 0-2 |
| mssql-driver-pool | Connection pooling | mssql-client |
| mssql-testing | Test infrastructure | mssql-client |

### Circular Dev-Dependencies

**CRITICAL**: The following circular dev-dependencies exist and must be handled for first-time publishes:

- `mssql-derive` ↔ `mssql-client` (both have each other as dev-deps)
- `mssql-client` → `mssql-driver-pool` (dev-dep, but pool depends on client at runtime)

For **initial releases** or when these crates haven't been published:
1. Temporarily comment out circular dev-dependencies
2. Publish in tier order
3. Restore dev-dependencies after all crates are published

See [Handling Circular Dev-Dependencies](#handling-circular-dev-dependencies-first-time-publish) for detailed steps.

---

## Pre-Release Checklist

### 0. Pre-flight Checks

```bash
just release-check  # Comprehensive validation
```

- [ ] Git working directory is clean
- [ ] CI is passing on main branch
- [ ] `just release-check` completes successfully

### 1. Codebase Hygiene & Safety

```bash
just wip-check      # TODO/FIXME/XXX/HACK, todo!/unimplemented!
just panic-audit    # .unwrap()/.expect() audit
just clippy-all     # Warnings-as-errors
```

- [ ] No blocking `todo!()` or `unimplemented!()` in production code
- [ ] All `.unwrap()` and `.expect()` calls reviewed for safety
- [ ] No clippy warnings

### 2. Version Consistency

```bash
just version-sync   # Check README matches Cargo.toml
```

Verify version is consistent in:
- [ ] `Cargo.toml` (workspace.package.version)
- [ ] All workspace members inherit correctly
- [ ] README.md installation instructions
- [ ] CHANGELOG.md has entry with correct date

### 3. Security & Dependency Audit

```bash
just deny    # Licenses, bans, advisories
just audit   # Security vulnerabilities
```

- [ ] No license violations
- [ ] No banned dependencies
- [ ] No unaddressed security advisories (or documented in `deny.toml`)

### 4. Documentation Integrity

```bash
just doc-check      # Documentation builds without warnings
just link-check     # Markdown link validation (requires lychee)
```

- [ ] Documentation builds without warnings
- [ ] Internal links resolve correctly
- [ ] CHANGELOG.md updated with new version section
- [ ] Breaking changes have migration notes

### 5. API Compatibility

```bash
just semver    # Breaking change detection
```

- [ ] No unintended breaking changes (or version bump accounts for them)
- [ ] Public API surface reviewed
- [ ] Deprecations documented

### 6. Final Build Verification

```bash
just ci-release    # Full CI + semver + MSRV
```

- [ ] All tests pass
- [ ] All feature combinations compile
- [ ] Examples build successfully

### 7. Publishing Preparation

```bash
just publish-dry      # Dry-run all crates
just metadata-check   # Verify crates.io metadata
just url-check        # Verify repository URLs
```

- [ ] All 9 crates pass dry-run publish
- [ ] Required metadata present (description, license, repository)
- [ ] Repository URL is `praxiomlabs/rust-mssql-driver`

---

## Release Workflow

**Publishing to crates.io is IRREVERSIBLE.** Follow this exact sequence:

```
┌─────────────────────────────────────────────────────────────┐
│  1. PREPARE: Version bump + CHANGELOG + commit              │
│                         ↓                                   │
│  2. PUSH: git push origin main                              │
│                         ↓                                   │
│  3. WAIT: CI must pass on main (watch with `gh run watch`)  │
│                         ↓                                   │
│  4. TAG: just tag (creates vX.Y.Z)                          │
│                         ↓                                   │
│  5. RELEASE: git push origin vX.Y.Z                         │
│                         ↓                                   │
│  6. AUTOMATED: CI publishes to crates.io + GitHub Release   │
└─────────────────────────────────────────────────────────────┘
```

### Step-by-Step Commands

#### Step 1: Prepare Version

Edit `Cargo.toml` in the workspace root:

```toml
[workspace.package]
version = "X.Y.Z"
```

Update `CHANGELOG.md`:

```markdown
## [Unreleased]

## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

[Unreleased]: https://github.com/praxiomlabs/rust-mssql-driver/compare/vX.Y.Z...HEAD
[X.Y.Z]: https://github.com/praxiomlabs/rust-mssql-driver/compare/vPREV...vX.Y.Z
```

#### Step 2-3: Commit and Push

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: release vX.Y.Z"
git push origin main

# Wait for CI to pass
gh run watch                    # Interactive watch
# OR
gh run list --limit 1           # Check status
```

#### Step 4-5: Tag and Release

```bash
# ONLY after CI passes on main!
just tag                        # Creates annotated tag vX.Y.Z
git push origin vX.Y.Z          # Triggers release.yml workflow
```

#### Step 6: Monitor

```bash
gh run watch    # Watch publish workflow
```

---

## Manual Publishing

If automated publishing fails, publish manually in this exact order:

```bash
# Tier 0: Independent crates
cargo publish -p tds-protocol
cargo publish -p mssql-types
sleep 30  # Wait for crates.io index propagation

# Tier 1: Crates depending on Tier 0
cargo publish -p mssql-tls
cargo publish -p mssql-codec
cargo publish -p mssql-auth
sleep 30

# Tier 2: Proc-macro crate
cargo publish -p mssql-derive
sleep 30

# Tier 3: Main client
cargo publish -p mssql-client
sleep 30

# Tier 4: Crates depending on mssql-client
cargo publish -p mssql-driver-pool
cargo publish -p mssql-testing
```

### Handling Circular Dev-Dependencies (First-Time Publish)

If `mssql-derive` or `mssql-client` fail due to circular dev-dependencies:

1. **Temporarily remove circular dev-deps:**

   In `crates/mssql-derive/Cargo.toml`:
   ```toml
   [dev-dependencies]
   # mssql-client = { workspace = true }  # Comment out
   ```

   In `crates/mssql-client/Cargo.toml`:
   ```toml
   [dev-dependencies]
   # mssql-derive = { workspace = true }  # Comment out
   # mssql-driver-pool = { workspace = true }  # Comment out
   ```

2. **Publish in order** (Tier 2 → Tier 3 → Tier 4)

3. **Restore dev-dependencies:**
   ```bash
   git checkout crates/mssql-derive/Cargo.toml crates/mssql-client/Cargo.toml
   git add -A && git commit -m "chore: restore dev-dependencies after publish"
   git push origin main
   ```

---

## Post-Release Verification

### Immediate Checks (within 5 minutes)

```bash
# Verify crates are on crates.io
cargo search mssql-client

# Verify crate is usable
cd /tmp && cargo new test-release && cd test-release
cargo add mssql-client@X.Y.Z
cargo check

# Verify GitHub release was created
gh release view vX.Y.Z --repo praxiomlabs/rust-mssql-driver
```

- [ ] `cargo search mssql-client` shows correct version
- [ ] `cargo add mssql-client` works in fresh project
- [ ] GitHub release exists with changelog content

### Delayed Checks (15-30 minutes)

```bash
# Check docs.rs (takes time to build)
curl -I https://docs.rs/mssql-client/X.Y.Z

# Check badges
curl -I https://img.shields.io/crates/v/mssql-client.svg
```

- [ ] docs.rs documentation is built and accessible
- [ ] README badges show correct version

### Repository Cleanup

- [ ] Update `[Unreleased]` section in CHANGELOG for next cycle
- [ ] Close related milestones/issues

---

## CI Automation Coverage

The following checks are **automated in CI**:

| Check | CI Job | Local Recipe | Manual Needed |
|-------|--------|--------------|---------------|
| Format | `lint` | `just fmt-check` | No |
| Linting | `lint` | `just clippy-all` | No |
| Tests | `test` | `just nextest-all` | No |
| MSRV | `msrv` | `just msrv-check` | No |
| Cross-platform | `test` matrix | N/A | No |
| Security/license | `deny` | `just deny` | No |
| Semver compliance | `semver` | `just semver` | No |
| Doc build | `docs` | `just doc-check` | No |
| Code coverage | `coverage` | `just coverage` | No |
| Integration tests | `integration` | `just test-integration` | No |
| Miri (unsafe) | `miri` | `just miri` | No |
| Publish to crates.io | `release.yml` | `just publish-dry` | Tag triggers |
| GitHub Release | `release.yml` | N/A | Tag triggers |

**Still requires manual verification:**
- Version string updates in documentation
- Post-release installation test
- Announcement/communication

---

## Justfile Recipe Reference

| Checklist Section | Recipe | What It Does |
|-------------------|--------|--------------|
| Pre-flight | `just release-check` | Full validation + git state |
| Code hygiene | `just wip-check` | TODO/FIXME/todo!/unimplemented! |
| Code hygiene | `just panic-audit` | .unwrap()/.expect() audit |
| Version sync | `just version-sync` | README version matches Cargo.toml |
| Security | `just deny` | Licenses, bans, advisories |
| Security | `just audit` | Vulnerability scan |
| Documentation | `just doc-check` | Docs build without warnings |
| Documentation | `just link-check` | Markdown link validation |
| Semver | `just semver` | Breaking change detection |
| MSRV | `just msrv-check` | Compile with declared MSRV |
| Publishing | `just publish-dry` | Dry-run all 9 crates |
| Publishing | `just metadata-check` | crates.io metadata |
| Publishing | `just url-check` | Repository URLs |
| Publishing | `just dep-graph` | Dependency tier visualization |
| Git | `just tag` | Create annotated version tag |
| Full CI | `just ci-release` | ci-full + semver + msrv + features |

---

## Troubleshooting

### "no matching package named X found"

**Cause**: Publishing a crate before its dependencies are on crates.io.

**Fix**: Follow the tier-based publish order. Wait 30 seconds between tiers for index propagation.

### "circular dependency" or dev-dependency resolution failure

**Cause**: Circular dev-dependencies between crates.

**Fix**: Temporarily remove the circular dev-deps, publish, then restore. See [Handling Circular Dev-Dependencies](#handling-circular-dev-dependencies-first-time-publish).

### Rate Limited (429 Too Many Requests)

**Cause**: crates.io limits new crate publications.

**Fix**: Wait for the time specified in the error message, then retry.

### docs.rs Build Failed

**Cause**: Documentation requires features or dependencies not available in docs.rs environment.

**Fix**:
1. Check docs.rs build logs
2. Add `[package.metadata.docs.rs]` configuration if needed
3. Ensure all doc examples compile (`cargo test --doc`)

### GitHub Release Not Created

**Cause**: Release workflow failed or tag format incorrect.

**Fix**:
1. Verify tag format is `vX.Y.Z` (not `v.X.Y.Z` or other variants)
2. Check workflow logs in GitHub Actions
3. Manually create release if needed: `gh release create vX.Y.Z --generate-notes`

### Semver Check Fails on mssql-testing

**Cause**: The `testcontainers` crate has a transitive dependency (`home`) that requires a newer Rust version than our MSRV.

**Fix**: This is expected. The `mssql-testing` crate is excluded from semver-checks in CI. Testing utilities have relaxed API stability requirements.

---

## Platform-Specific Notes

### Linux (with integrated-auth feature)

The `--all-features` flag requires system libraries:

```bash
# Debian/Ubuntu
sudo apt-get install libkrb5-dev libclang-dev

# RHEL/Fedora
sudo dnf install krb5-devel clang-devel

# Or use the just recipe
just setup-linux
```

### macOS/Windows

Use default features (omit `--all-features`) as Kerberos/GSSAPI is Linux-only:

```bash
just ci      # Instead of just ci-all
just test    # Instead of just test-all
```

---

## Automated vs Manual Release

| Aspect | Automated (tag push) | Manual |
|--------|---------------------|--------|
| Trigger | Push `vX.Y.Z` tag | Run commands locally |
| CI checks | Run automatically | Must run manually first |
| Publish order | Handled by workflow | Must follow tier order |
| Rate limits | May hit limits | Can wait between publishes |
| Circular deps | Must be pre-resolved | Can resolve during process |

**Recommendation**: Use automated release for routine releases. Use manual process for first-time publishes or when troubleshooting.

---

## Release Checklist Template

Copy this for each release:

```markdown
## Release vX.Y.Z Checklist

### Pre-Release
- [ ] `just release-check` passes
- [ ] Version bumped in Cargo.toml
- [ ] CHANGELOG.md updated with date
- [ ] CI passing on main branch

### Release Execution
- [ ] Release commit pushed to main
- [ ] CI passed on main (verified via `gh run watch`)
- [ ] Tag created with `just tag`
- [ ] Tag pushed to trigger release workflow
- [ ] Release workflow completed successfully

### Post-Release
- [ ] `cargo search mssql-client` shows vX.Y.Z
- [ ] `cargo add mssql-client@X.Y.Z` works
- [ ] GitHub release exists
- [ ] docs.rs building/built
- [ ] CHANGELOG [Unreleased] section reset
```
