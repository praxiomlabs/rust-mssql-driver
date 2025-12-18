# Release Process

This document describes the process for releasing new versions of rust-mssql-driver.

## Version Numbering

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR.MINOR.PATCH** (e.g., 1.2.3)
- Pre-1.0: Minor bumps may contain breaking changes
- Post-1.0: Strictly follow semver

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

### Circular Dev-Dependencies

**CRITICAL**: The following circular dev-dependencies exist and must be handled:

- `mssql-derive` ↔ `mssql-client` (both have each other as dev-deps)
- `mssql-client` → `mssql-driver-pool` (dev-dep, but pool depends on client at runtime)

For **initial releases** or when these crates haven't been published:
1. Temporarily comment out circular dev-dependencies
2. Publish in order
3. Restore dev-dependencies after all crates are published

---

## Pre-Release Checklist

### Phase 1: Code Quality Validation

```bash
# Run all checks (requires libkrb5-dev on Linux for --all-features)
cargo fmt --all --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

- [ ] All formatting passes
- [ ] No clippy warnings
- [ ] All tests pass
- [ ] Documentation builds without warnings

### Phase 2: Security & Dependency Audit

```bash
cargo deny check
cargo audit
```

- [ ] No license violations
- [ ] No banned dependencies
- [ ] No unaddressed security advisories (or documented exceptions in `.cargo/audit.toml`)

### Phase 3: Dependency Graph Validation

```bash
# Verify publish order by checking what each crate depends on
cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | "\(.name): \(.dependencies | map(.name) | join(", "))"'

# Check for circular dependencies in dev-dependencies
# Manual inspection required - see "Circular Dev-Dependencies" section above
```

- [ ] Dependency graph matches documented tiers
- [ ] No unexpected circular dependencies
- [ ] All internal dependencies use `workspace = true`

### Phase 4: Version & Metadata Verification

```bash
# Check version consistency
cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name | startswith("mssql") or . == "tds-protocol") | "\(.name): \(.version)"'

# Verify crates.io required metadata
cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "mssql-client") | {name, version, description, license, repository, keywords, categories}'
```

- [ ] All crate versions match workspace version
- [ ] Repository URL is correct (`praxiomlabs/rust-mssql-driver`)
- [ ] All crates have description, license, keywords, categories
- [ ] README version references match release version

### Phase 5: Documentation Review

- [ ] CHANGELOG.md has entry for this version with correct date
- [ ] CHANGELOG.md links at bottom are correct
- [ ] README.md examples are accurate
- [ ] API documentation is complete
- [ ] Breaking changes have migration notes

### Phase 6: CI Verification

- [ ] All CI jobs pass on main branch
- [ ] Release workflow is correctly configured
- [ ] `CARGO_REGISTRY_TOKEN` secret is set in GitHub

### Phase 7: Dry-Run Publish Test

```bash
# Test independent crates can be packaged
cargo publish -p tds-protocol --dry-run
cargo publish -p mssql-types --dry-run
cargo publish -p mssql-derive --dry-run

# Note: Dependent crates cannot be dry-run tested until dependencies are published
```

- [ ] Independent crates package successfully
- [ ] No packaging errors

---

## Release Steps

### 1. Update Version Numbers

Edit `Cargo.toml` in the workspace root:

```toml
[workspace.package]
version = "X.Y.Z"
```

All crates inherit this version automatically.

### 2. Update CHANGELOG

1. Move items from `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD`
2. Add new `[Unreleased]` section
3. Update comparison links at the bottom

```markdown
## [Unreleased]

## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

[Unreleased]: https://github.com/praxiomlabs/rust-mssql-driver/compare/vX.Y.Z...HEAD
[X.Y.Z]: https://github.com/praxiomlabs/rust-mssql-driver/compare/vPREV...vX.Y.Z
```

### 3. Create Release Commit

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: release version X.Y.Z"
git push origin main
```

### 4. Create and Push Tag

```bash
git tag v.X.Y.Z
git push origin vX.Y.Z
```

This triggers the automated release workflow.

### 5. Monitor Release Workflow

Watch the GitHub Actions release workflow:
- https://github.com/praxiomlabs/rust-mssql-driver/actions

If the workflow fails, you may need to publish manually (see Manual Publishing below).

---

## Manual Publishing

If automated publishing fails, publish manually in this exact order:

```bash
# Tier 0: Independent crates
cargo publish -p tds-protocol
cargo publish -p mssql-types
sleep 30  # Wait for crates.io index propagation

# Tier 1: Crates depending on tds-protocol
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
   # Or manually uncomment the lines
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
cd /tmp && mkdir test-release && cd test-release
cargo init
echo 'mssql-client = "X.Y.Z"' >> Cargo.toml
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

### Downstream Verification

- [ ] Example projects compile with new version
- [ ] No unexpected breaking changes reported

---

## Release Checklist Template

Copy this for each release:

```markdown
## Release X.Y.Z Checklist

### Pre-Release Validation
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-features` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] `cargo doc --workspace --all-features --no-deps` passes
- [ ] `cargo deny check` passes
- [ ] `cargo audit` passes (or exceptions documented)
- [ ] Dependency graph validated
- [ ] All versions consistent
- [ ] Repository URLs correct (praxiomlabs/rust-mssql-driver)
- [ ] CHANGELOG.md updated with date
- [ ] CI passing on main branch

### Release Execution
- [ ] Version bumped in Cargo.toml
- [ ] Release commit pushed
- [ ] Tag created and pushed
- [ ] Release workflow completed (or manual publish done)
- [ ] Circular dev-deps restored (if removed)

### Post-Release Verification
- [ ] `cargo search mssql-client` shows X.Y.Z
- [ ] `cargo add mssql-client` works in fresh project
- [ ] GitHub release exists
- [ ] docs.rs building/built
- [ ] Badges updated
```

---

## Troubleshooting

### "no matching package named X found"

**Cause**: Publishing a crate before its dependencies are on crates.io.

**Fix**: Follow the tier-based publish order. Wait 30 seconds between tiers for index propagation.

### "circular dependency" or dev-dependency resolution failure

**Cause**: Circular dev-dependencies between crates.

**Fix**: Temporarily remove the circular dev-deps, publish, then restore. See "Handling Circular Dev-Dependencies" above.

### Rate Limited (429 Too Many Requests)

**Cause**: crates.io limits new crate publications.

**Fix**: Wait for the time specified in the error message, then retry.

### docs.rs Build Failed

**Cause**: Documentation requires features or dependencies not available in docs.rs environment.

**Fix**:
1. Check docs.rs build logs
2. Add `[package.metadata.docs.rs]` configuration if needed
3. Ensure all doc examples compile

### GitHub Release Not Created

**Cause**: Release workflow failed or tag format incorrect.

**Fix**:
1. Verify tag format is `vX.Y.Z`
2. Check workflow logs
3. Manually create release if needed via `gh release create`

---

## Platform-Specific Notes

### Linux (with integrated-auth feature)

The `--all-features` flag requires `libkrb5-dev`:

```bash
# Debian/Ubuntu
sudo apt-get install libkrb5-dev

# RHEL/Fedora
sudo dnf install krb5-devel
```

### macOS/Windows

Use default features (omit `--all-features`) as Kerberos/GSSAPI is Linux-only.

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
