# Release Process

This document describes the process for releasing new versions of rust-mssql-driver.

## Version Numbering

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR.MINOR.PATCH** (e.g., 1.2.3)
- Pre-1.0: Minor bumps may contain breaking changes
- Post-1.0: Strictly follow semver

## Crate Publication Order

Due to dependencies between crates, they must be published in a specific order:

```
1. tds-protocol        (no internal deps)
2. mssql-tls           (no internal deps)
3. mssql-codec         (depends on tds-protocol)
4. mssql-types         (depends on tds-protocol)
5. mssql-auth          (no internal deps)
6. mssql-derive        (no internal deps)
7. mssql-client        (depends on all above)
8. mssql-driver-pool   (depends on mssql-client)
9. mssql-testing       (depends on mssql-client)
```

## Pre-Release Checklist

Before starting the release process:

### Code Quality
- [ ] All tests pass: `cargo test --workspace --all-features`
- [ ] No clippy warnings: `cargo clippy --workspace --all-features`
- [ ] Code is formatted: `cargo fmt --all --check`
- [ ] No dependency issues: `cargo deny check`

### Documentation
- [ ] CHANGELOG.md is updated with all changes
- [ ] README.md examples are tested and working
- [ ] API documentation is complete: `cargo doc --workspace --no-deps`
- [ ] Breaking changes have migration guides

### Version Consistency
- [ ] `workspace.package.version` in root `Cargo.toml` is correct
- [ ] All crate versions match (they inherit from workspace)
- [ ] CHANGELOG.md date is set to release date
- [ ] Git tag matches the version number

### Final Verification
- [ ] CI pipeline passes on main branch
- [ ] Integration tests pass against SQL Server 2019 and 2022
- [ ] Examples run successfully

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

[Unreleased]: https://github.com/rust-mssql-driver/rust-mssql-driver/compare/vX.Y.Z...HEAD
[X.Y.Z]: https://github.com/rust-mssql-driver/rust-mssql-driver/compare/vX.Y-1.Z...vX.Y.Z
```

### 3. Create Release Commit

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: release version X.Y.Z"
```

### 4. Create Git Tag

```bash
git tag -a vX.Y.Z -m "Release version X.Y.Z"
```

### 5. Push to Remote

```bash
git push origin main
git push origin vX.Y.Z
```

### 6. Publish to crates.io

Publish crates in dependency order:

```bash
# Verify dry-run first
cargo publish -p tds-protocol --dry-run

# Publish each crate (wait for previous to propagate)
cargo publish -p tds-protocol
sleep 30
cargo publish -p mssql-tls
sleep 30
cargo publish -p mssql-codec
sleep 30
cargo publish -p mssql-types
sleep 30
cargo publish -p mssql-auth
sleep 30
cargo publish -p mssql-derive
sleep 30
cargo publish -p mssql-client
sleep 30
cargo publish -p mssql-driver-pool
# Note: mssql-testing is typically not published (test infrastructure only)
```

### 7. Create GitHub Release

1. Go to GitHub Releases
2. Create new release from tag `vX.Y.Z`
3. Use CHANGELOG content for release notes
4. Attach any relevant assets

### 8. Announce Release

- Update documentation site (if applicable)
- Post to relevant forums/channels
- Update project status if needed

## Automated Release (Future)

We plan to automate releases using:

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Publish crates
        run: cargo xtask publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

## Hotfix Process

For critical bug fixes that need immediate release:

### 1. Create Hotfix Branch

```bash
git checkout -b hotfix/X.Y.Z+1 vX.Y.Z
```

### 2. Apply Fix

- Make minimal changes to fix the issue
- Add regression test
- Update CHANGELOG

### 3. Bump Patch Version

```toml
version = "X.Y.Z+1"
```

### 4. Release

Follow normal release steps from step 3 onwards.

### 5. Merge Back

```bash
git checkout main
git merge hotfix/X.Y.Z+1
git push origin main
```

## Yanking a Release

If a release contains a critical bug:

```bash
# Yank the problematic version
cargo yank --version X.Y.Z mssql-client

# Publish fix as new patch version
# Follow hotfix process
```

**Note:** Yanking prevents new dependencies but doesn't remove existing ones.

## Release Cadence

- **Patch releases**: As needed for bug fixes
- **Minor releases**: Every 4-8 weeks during active development
- **Major releases**: When significant breaking changes accumulate

## Checklist Template

Copy this template for each release:

```markdown
## Release X.Y.Z Checklist

### Pre-Release
- [ ] All CI checks pass
- [ ] `cargo test --workspace --all-features`
- [ ] `cargo clippy --workspace --all-features`
- [ ] `cargo deny check`
- [ ] CHANGELOG.md updated
- [ ] Documentation reviewed

### Release
- [ ] Version bumped in Cargo.toml
- [ ] Release commit created
- [ ] Tag created and pushed
- [ ] Crates published in order
- [ ] GitHub release created

### Post-Release
- [ ] Verify crates.io pages
- [ ] Test `cargo add mssql-client`
- [ ] Announce release
- [ ] Update downstream projects
```

## Troubleshooting

### Publish Failed Mid-Way

If publishing fails partway through:

1. Note which crates were published
2. Fix the issue
3. Bump patch version
4. Continue publishing remaining crates

### Version Mismatch

If crates.io shows inconsistent versions:

1. Check `Cargo.lock` for version conflicts
2. Ensure all crates use `version.workspace = true`
3. Yank incorrect versions if necessary

### Authentication Issues

```bash
# Login to crates.io
cargo login

# Verify token
cargo owner --list mssql-client
```
