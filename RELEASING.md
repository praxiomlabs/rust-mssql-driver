# Releasing rust-mssql-driver

Comprehensive guide for releasing new versions of rust-mssql-driver to crates.io.

**Version:** 0.4.0 | **MSRV:** 1.85 | **Edition:** 2024 | **Workspace:** 9 crates

---

## ⚠️ CRITICAL: Read Before Any Release

### The Cardinal Rules

1. **NEVER manually run `cargo publish`** — Always use the automated GitHub Actions workflow triggered by pushing a version tag. The `just publish` recipe exists only for disaster recovery.

2. **NEVER push a tag until CI passes on main** — Always run `gh run watch` or `just ci-status` to verify CI passed before creating a tag.

3. **ALWAYS use `--all-features` for pre-release checks** — Feature-gated code (like `zeroize`) must be validated before release. Run `just ci-all` not just `just ci`.

4. **Publishing to crates.io is IRREVERSIBLE** — You can yank a version, but you cannot delete or re-upload it. A yanked version still counts as "used" forever.

### The v0.2.1 Incident (Cautionary Tale)

During the v0.2.1 release, we:
- Ran `cargo clippy` without `--all-features`, missing a compilation error in the `zeroize` feature
- Manually ran `cargo publish` instead of using the GitHub Actions workflow
- Published all 9 crates before realizing v0.2.1 was broken

**Result:** We had to yank all 9 crates at v0.2.1, release v0.2.2 as a hotfix, and lost a version number forever. **Don't repeat this mistake.**

### Pre-Release Verification Checklist

Before creating a tag, **always** verify:

```bash
# 1. Run the FULL release check (validates ALL features)
just release-check

# 2. Verify CI passed on main (blocking check)
just ci-status  # Must show "completed" with green check

# 3. Only then create the tag
just tag
git push origin vX.Y.Z
```

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
4. [Feature-Specific Testing](#feature-specific-testing)
5. [Release Workflow](#release-workflow)
6. [Manual Publishing](#manual-publishing)
7. [Post-Release Verification](#post-release-verification)
8. [CI Automation Coverage](#ci-automation-coverage)
9. [CI Parity](#ci-parity)
10. [Justfile Recipe Reference](#justfile-recipe-reference)
11. [Troubleshooting](#troubleshooting)
12. [Platform-Specific Notes](#platform-specific-notes)
13. [Security Incident Response](#security-incident-response)
14. [SBOM Generation](#sbom-generation)
15. [Lessons Learned](#lessons-learned)
16. [Release Checklist Template](#release-checklist-template)
17. [Manual Recovery Procedures](#manual-recovery-procedures)
18. [Additional Resources](#additional-resources)

---

## Version Numbering

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR.MINOR.PATCH** (e.g., 1.2.3)
- **Pre-1.0**: Minor bumps may contain breaking changes
- **Post-1.0**: Strictly follow semver

### Version Bump Guidelines

| Change Type | Version Bump | Examples |
|-------------|-------------|----------|
| Bug fixes only | PATCH (0.1.x) | Fix query parsing, handle edge case |
| New features (backwards compatible) | MINOR (0.x.0) | Add new API method, new error type |
| Breaking API changes | MAJOR (x.0.0) | Remove method, change return type, rename public type |
| Internal refactoring | PATCH | Code cleanup, dependency updates (non-breaking) |
| Security fixes | PATCH | CVE fix, vulnerability patch |
| Deprecation without removal | MINOR | Mark methods as deprecated (still work) |
| Removal of deprecated API | MAJOR | Delete previously deprecated methods |

**Note:** While pre-1.0, MINOR bumps may contain breaking changes. Document all breaking changes in CHANGELOG.md regardless of version number.

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
just typos          # Spell checking
```

- [ ] No blocking `todo!()` or `unimplemented!()` in production code
- [ ] All `.unwrap()` and `.expect()` calls reviewed for safety
- [ ] No clippy warnings
- [ ] No typos in code or documentation

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

## Feature-Specific Testing

This workspace has features that require specific testing and sometimes system dependencies.

### Feature Matrix

| Crate | Feature | Description | Test Command | System Deps |
|-------|---------|-------------|--------------|-------------|
| mssql-client | `default` | Standard functionality | `cargo test -p mssql-client` | None |
| mssql-client | `zeroize` | Secure memory wiping | `cargo test -p mssql-client --features zeroize` | None |
| mssql-client | `integrated-auth` | Kerberos/GSSAPI auth | `cargo test -p mssql-client --features integrated-auth` | libkrb5-dev |
| tds-protocol | `no_std` | no_std compatible | `cargo check -p tds-protocol --no-default-features` | None |
| mssql-derive | `default` | Row mapping macros | `cargo test -p mssql-derive` | None |
| mssql-driver-pool | `default` | Connection pooling | `cargo test -p mssql-driver-pool` | None |

### Critical Feature Combinations

```bash
# Test each crate's default features
for crate in tds-protocol mssql-types mssql-tls mssql-codec mssql-auth mssql-derive mssql-client mssql-driver-pool mssql-testing; do
    cargo test -p "$crate"
done

# Test zeroize feature (security-critical)
cargo test -p mssql-client --features zeroize

# Test integrated-auth (Linux only, requires libkrb5-dev)
cargo test -p mssql-client --features integrated-auth

# Test all features combined (the v0.2.1 lesson)
cargo test --workspace --all-features

# Test no_std compatibility for tds-protocol
cargo check -p tds-protocol --no-default-features --target thumbv7em-none-eabihf
```

### The All-Features Rule

**CRITICAL**: Before any release, always run tests with `--all-features`:

```bash
just ci-all           # Full CI with all features
just test-all         # Tests with all features
just clippy-all       # Clippy with all features
```

This prevents the v0.2.1 incident where feature-gated code broke but wasn't caught.

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

> **⚠️ WARNING: Manual publishing should be a LAST RESORT only.**
>
> The automated GitHub Actions workflow is the **only** sanctioned way to publish.
> Manual publishing bypasses CI checks and has historically caused broken releases.
>
> **Only use manual publishing when:**
> - GitHub Actions is completely down/unavailable
> - The automated workflow failed mid-publish (some crates published, some didn't)
> - You have explicitly verified ALL checks pass locally with `just release-check`

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

## CI Parity

Ensure your local commands match CI behavior to avoid surprise failures.

### Local vs CI Commands

| Check | Local Command | CI Equivalent | Notes |
|-------|--------------|---------------|-------|
| **Quick check** | `just quick` | N/A | Fast local feedback only |
| **Format** | `just fmt-check` | `cargo fmt --all -- --check` | Exact match |
| **Clippy (default)** | `just clippy` | N/A | CI uses `--all-features` |
| **Clippy (all)** | `just clippy-all` | `cargo clippy --workspace --all-features --all-targets -- -D warnings` | **Use this before push** |
| **Tests (default)** | `just test` | N/A | CI uses `--all-features --locked` |
| **Tests (locked)** | `just nextest-locked-all` | `cargo nextest run --workspace --all-features --locked` | **Matches CI exactly** |
| **Docs** | `just doc-check-all` | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps` | Warnings as errors |
| **MSRV** | `just msrv-check-all` | `cargo +1.85 check --workspace --all-features` | Uses toolchain from rust-toolchain.toml |
| **Deny** | `just deny` | `cargo deny check` | License + advisory check |
| **Semver** | `just semver` | `cargo semver-checks check-release --exclude mssql-testing` | Uses stable toolchain |

### Feature-Specific Testing

Some features require additional system dependencies:

| Feature | Dependency | Install Command (Ubuntu) |
|---------|-----------|-------------------------|
| `integrated-auth` | libkrb5-dev | `sudo apt-get install libkrb5-dev` |
| (bindgen) | libclang-dev | `sudo apt-get install libclang-dev` |

**IMPORTANT:** Always run `just ci-all` (not `just ci`) before pushing to ensure feature-gated code compiles.

---

## Justfile Recipe Reference

| Checklist Section | Recipe | What It Does |
|-------------------|--------|--------------|
| **Pre-flight (REQUIRED)** | `just release-check` | Full validation with ALL features ⭐ |
| Pre-flight | `just ci-status` | Verify CI passed on main branch |
| Code hygiene | `just wip-check` | TODO/FIXME/todo!/unimplemented! |
| Code hygiene | `just panic-audit` | .unwrap()/.expect() audit |
| Code hygiene | `just typos` | Spell checking |
| Version sync | `just version-sync` | README version matches Cargo.toml |
| Security | `just deny` | Licenses, bans, advisories |
| Security | `just audit` | Vulnerability scan |
| Documentation | `just doc-check-all` | Docs build without warnings (all features) |
| Documentation | `just link-check` | Markdown link validation |
| Semver | `just semver` | Breaking change detection |
| MSRV | `just msrv-check-all` | Compile with declared MSRV (all features) |
| Publishing | `just publish-dry` | Dry-run all 9 crates |
| Publishing | `just publish` | Publish all crates (**LAST RESORT ONLY**) |
| Publishing | `just metadata-check` | crates.io metadata |
| Publishing | `just url-check` | Repository URLs |
| Publishing | `just dep-graph` | Dependency tier visualization |
| Git | `just tag` | Create annotated version tag |
| Full CI | `just ci-release-all` | Full release validation with ALL features ⭐ |

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

## Security Incident Response

This section documents procedures for handling security vulnerabilities in released versions.

### Severity Assessment

| Severity | CVSS Score | Response Time | Examples |
|----------|------------|---------------|----------|
| **Critical** | 9.0-10.0 | Immediate (same day) | SQL injection, auth bypass, TLS downgrade |
| **High** | 7.0-8.9 | 24-48 hours | Credential exposure, privilege escalation |
| **Medium** | 4.0-6.9 | 1 week | Information disclosure, DoS |
| **Low** | 0.1-3.9 | Next release | Minor information disclosure |

### Security Release Process

1. **Assess and Confirm**
   - Verify the vulnerability is real and reproducible
   - Determine affected versions and severity
   - Check if actively exploited

2. **Develop Fix**
   - Create fix on private branch
   - Ensure fix doesn't introduce new issues
   - Prepare minimal, targeted patch

3. **Coordinate Disclosure** (for Critical/High)
   - Notify affected downstream users privately if known
   - Coordinate with security researchers if externally reported
   - Prepare security advisory

4. **Release Security Patch**
   - Follow standard release process with expedited timeline
   - Use PATCH version bump (e.g., 0.3.0 → 0.3.1)
   - Document as security fix in CHANGELOG

5. **Post-Release**
   - Publish GitHub Security Advisory
   - Request CVE if applicable
   - Update RustSec advisory database

### Yanking All Crates

For severe security issues affecting the entire workspace:

```bash
VERSION="0.3.0"
for crate in tds-protocol mssql-types mssql-tls mssql-codec mssql-auth mssql-derive mssql-client mssql-driver-pool mssql-testing; do
    cargo yank --version "$VERSION" "$crate"
done
```

---

## SBOM Generation

Software Bill of Materials (SBOM) generation is supported for supply chain transparency.

### Generating SBOM

```bash
# Generate SBOM in CycloneDX format
cargo sbom --output-format cyclonedx-json > sbom.json

# Generate SBOM in SPDX format
cargo sbom --output-format spdx-json > sbom.spdx.json
```

### CI Integration

The release workflow can automatically generate and attach SBOM to GitHub releases:

```yaml
# In .github/workflows/release.yml
- name: Generate SBOM
  run: |
    cargo install cargo-sbom
    cargo sbom --output-format cyclonedx-json > sbom.cyclonedx.json

- name: Upload SBOM to Release
  run: |
    gh release upload ${{ github.ref_name }} sbom.cyclonedx.json
```

### SBOM Best Practices

1. **Generate for each release**: Attach SBOM to every GitHub release
2. **Include all dependencies**: Use `--all-features` to capture all possible dependencies
3. **Verify contents**: Review SBOM for unexpected dependencies before publishing
4. **Archive historical SBOMs**: Maintain SBOMs for older versions for audit purposes

---

## Lessons Learned

This section documents issues encountered in past releases and patterns to avoid.

### 1. The v0.2.1 Incident (Feature-Gated Code)

**Issue**: Released v0.2.1 with broken `zeroize` feature because we ran `cargo clippy` without `--all-features`.

**What went wrong**:
- Ran `cargo clippy` (default features only)
- Feature-gated code in `zeroize` feature had a compilation error
- Manually published all 9 crates before realizing the error
- Had to yank all 9 crates and release v0.2.2

**Solution**: Always use `just ci-all` instead of `just ci`. The `-all` suffix variants include `--all-features`.

### 2. Circular Dev-Dependencies

**Issue**: First-time publish failed due to circular dev-dependencies between mssql-derive and mssql-client.

**Solution**: Temporarily comment out circular dev-deps, publish in tier order, then restore. Document this in the release guide.

### 3. crates.io Index Propagation

**Issue**: Publishing mssql-client immediately after mssql-tls caused "package not found" errors.

**Solution**: Wait 30 seconds between tiers. The automated workflow handles this correctly.

### 4. MSRV Compliance

**Issue**: Used Rust 2024 edition features without updating MSRV documentation.

**Solution**: Run `just msrv-check-all` before every release. CI enforces this automatically.

### 5. Tag Format Consistency

**Issue**: Tags without `v` prefix don't trigger release workflow.

**Solution**: Always use `just tag` which enforces the `vX.Y.Z` format.

### 6. Manual Publishing Bypass

**Issue**: Manual `cargo publish` bypasses all CI checks and has caused broken releases.

**Solution**: Only use manual publishing as absolute last resort. Document the v0.2.1 incident as a cautionary tale.

### 7. Integration Test Dependencies

**Issue**: Integration tests require SQL Server container, which isn't always available in CI.

**Solution**: Integration tests are in a separate CI job that may be skipped. Document this in CI coverage.

### 8. Platform-Specific Features

**Issue**: `integrated-auth` feature requires libkrb5-dev on Linux, causing CI failures on macOS/Windows.

**Solution**: Use feature matrix in CI. Local devs on macOS/Windows should use `just ci` not `just ci-all`.

### 9. Semver Exclusions

**Issue**: mssql-testing crate has relaxed API stability, causing semver-checks failures.

**Solution**: Exclude mssql-testing from semver-checks. Test utilities don't need strict API stability.

### 10. Workspace Version Sync

**Issue**: Forgot to update all crate versions, causing dependency resolution failures.

**Solution**: Use workspace.package.version inheritance. All crates share the same version automatically.

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

---

## Manual Recovery Procedures

When automated processes fail, use these recovery procedures.

### Recovery: Partial Publish Failure

If the automated publish workflow fails mid-way (some crates published, others not):

1. **Identify what published:**
   ```bash
   for crate in tds-protocol mssql-types mssql-tls mssql-codec mssql-auth mssql-derive mssql-client mssql-driver-pool mssql-testing; do
       echo -n "$crate: "
       cargo search $crate 2>/dev/null | head -1 || echo "not found"
   done
   ```

2. **Find the last successful tier:**
   - Tier 0: tds-protocol, mssql-types
   - Tier 1: mssql-tls, mssql-codec, mssql-auth
   - Tier 2: mssql-derive
   - Tier 3: mssql-client
   - Tier 4: mssql-driver-pool, mssql-testing

3. **Resume from the failed crate:**
   ```bash
   # Example: mssql-client failed
   cargo publish -p mssql-client
   sleep 30
   cargo publish -p mssql-driver-pool
   cargo publish -p mssql-testing
   ```

### Recovery: Tag Created But Not Pushed

If you created a tag locally but haven't pushed it:

```bash
# Delete the local tag
git tag -d vX.Y.Z

# Fix any issues, then recreate
just release-check
just tag
git push origin vX.Y.Z
```

### Recovery: Tag Pushed But Workflow Failed

If you pushed a tag but the GitHub Actions workflow failed:

1. **Do NOT delete the tag** — tags should be immutable once pushed.

2. **Fix the issue** — If it's a code issue, create a new patch version:
   ```bash
   # Bump to X.Y.Z+1
   # Edit Cargo.toml and CHANGELOG.md
   git commit -am "fix: patch release vX.Y.(Z+1)"
   git push origin main
   gh run watch
   just tag  # Creates vX.Y.(Z+1)
   git push origin vX.Y.(Z+1)
   ```

3. **If workflow issue** — Re-run the workflow:
   ```bash
   gh run rerun --failed
   ```

### Recovery: Wrong Version Published

If you published the wrong version:

1. **Yank the broken version** (cannot delete, only yank):
   ```bash
   for crate in tds-protocol mssql-types mssql-tls mssql-codec mssql-auth mssql-derive mssql-client mssql-driver-pool mssql-testing; do
       cargo yank --vers X.Y.Z $crate
   done
   ```

2. **Publish a corrected version:**
   ```bash
   # Bump to next patch version
   # Edit Cargo.toml, CHANGELOG.md
   just release-check
   git commit -am "chore: release vX.Y.(Z+1) (fixes yanked vX.Y.Z)"
   git push origin main
   gh run watch
   just tag
   git push origin vX.Y.(Z+1)
   ```

3. **Document the yank** in CHANGELOG.md:
   ```markdown
   ## [X.Y.(Z+1)] - YYYY-MM-DD

   ### Fixed
   - (describe fix from yanked version)

   **Note:** v X.Y.Z was yanked due to (reason).
   ```

---

## Additional Resources

### Official Documentation

- [Semantic Versioning 2.0.0](https://semver.org/) — Version numbering standard
- [Cargo Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html) — Official cargo publish docs
- [crates.io Policies](https://crates.io/policies) — crates.io terms and policies

### Tools

- [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) — Detect breaking changes
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) — License and advisory checks
- [cargo-release](https://github.com/crate-ci/cargo-release) — Alternative release workflow tool
- [GitHub CLI (gh)](https://cli.github.com/) — Required for `just ci-status`

### Project-Specific Links

- [Repository](https://github.com/praxiomlabs/rust-mssql-driver) — Source code
- [crates.io: mssql-client](https://crates.io/crates/mssql-client) — Main crate
- [docs.rs: mssql-client](https://docs.rs/mssql-client) — Documentation
- [GitHub Actions Workflows](https://github.com/praxiomlabs/rust-mssql-driver/actions) — CI/CD status

### Related Reading

- [The Cargo Book: Package Layout](https://doc.rust-lang.org/cargo/guide/project-layout.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — API design best practices
- [Keep a Changelog](https://keepachangelog.com/) — Changelog format standard
