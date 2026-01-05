# Version References

This document lists all files containing version numbers that must be updated during releases.

Use this as a checklist when preparing a release to ensure consistency.

---

## Primary Version Files

These files contain the authoritative version number:

| File | Location | Update Method |
|------|----------|---------------|
| `Cargo.toml` (root) | `workspace.package.version` | Manual edit |
| `CHANGELOG.md` | Release header | Change `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD` |

---

## Secondary Version References

These files reference the version and should be verified:

| File | What to Check |
|------|---------------|
| `RELEASING.md` | Header version in `**Version:** X.Y.Z` |
| `ARCHITECTURE.md` | Version in header and any version-specific notes |
| `README.md` | Version in badges, installation instructions |
| `docs/BENCHMARKS.md` | Baseline save commands (`--save-baseline vX.Y.Z`) |

---

## Files That May Reference Versions

These files occasionally mention versions:

| File | When to Check |
|------|---------------|
| `PRODUCTION_READINESS.md` | Milestone table, version references |
| `STABILITY.md` | API stability guarantees per version |
| `docs/MIGRATION_FROM_TIBERIUS.md` | If migration instructions are version-specific |

---

## Crate-Specific Versions

All workspace crates inherit from `workspace.package.version`. No individual updates needed.

Crates (all inherit workspace version):
- `crates/tds-protocol/Cargo.toml`
- `crates/mssql-tls/Cargo.toml`
- `crates/mssql-codec/Cargo.toml`
- `crates/mssql-types/Cargo.toml`
- `crates/mssql-auth/Cargo.toml`
- `crates/mssql-pool/Cargo.toml`
- `crates/mssql-client/Cargo.toml`
- `crates/mssql-derive/Cargo.toml`
- `crates/mssql-testing/Cargo.toml`

---

## Release Checklist

```bash
# 1. Update version in Cargo.toml
# Edit workspace.package.version = "X.Y.Z"

# 2. Update CHANGELOG.md
# Change [Unreleased] to [X.Y.Z] - YYYY-MM-DD
# Add comparison link at bottom

# 3. Update RELEASING.md header
# **Version:** X.Y.Z

# 4. Verify other references
grep -r "0\.5\." --include="*.md" .  # Replace with old version

# 5. Run release check
just release-check

# 6. Commit
git add -A
git commit -m "chore: release vX.Y.Z"
```

---

## Automation

The `just release-check` command verifies:
- Cargo.toml version consistency
- CHANGELOG.md has dated release
- No `[Unreleased]` section without content
- All crates have matching versions

---

## Version Format

This project uses [Semantic Versioning](https://semver.org/):

- **MAJOR** (X.0.0): Breaking API changes
- **MINOR** (0.X.0): New features, backwards compatible
- **PATCH** (0.0.X): Bug fixes, backwards compatible

Pre-1.0 releases may have breaking changes in minor versions.

---

*Last updated: January 2026*
