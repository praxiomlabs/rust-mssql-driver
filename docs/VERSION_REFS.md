# Version References

This document is a **comprehensive checklist** of every file in the repository
that contains version numbers and must be reviewed during a release. Use it to
prevent the kind of version-drift incidents described in RELEASING.md § Lessons
Learned.

**Automation:** `just version-refs-check` scans for the most common patterns.
This document complements that automation by providing human context for
ambiguous cases (e.g., historical references that should NOT be updated).

---

## Categories

Version references fall into three categories. Handle each differently:

### 1. **Authoritative** — Single source of truth, must be updated every release

| File | Location | Update Rule |
|------|----------|-------------|
| `Cargo.toml` (root) | `workspace.package.version` | Must match the new release |
| `CHANGELOG.md` | `[Unreleased]` → `[X.Y.Z] - YYYY-MM-DD` | Add dated release section |

### 2. **Secondary** — Display/documentation, must be updated every release

These files reference the current version in text, badges, or examples. Stale
values here cause confusion and were the root cause of the v0.5.1 incident.

| File | What to Check | Update Rule |
|------|---------------|-------------|
| `RELEASING.md` | Header `**Version:** X.Y.Z` | Update to new version |
| `ARCHITECTURE.md` | Header `**Status:** Design Complete (vX.Y.Z Released)` | Update to new version |
| `ARCHITECTURE.md` | Example `Cargo.toml` snippets with `version = "X.Y.Z"` | Update example versions |
| `README.md` | Feature Status header `(vX.Y.x)` | Update to new minor version |
| `README.md` | MSRV badge (if MSRV bumped) | Update badge URL and label |
| `STABILITY.md` | No current version refs; updates only when MSRV changes | MSRV section only |
| `SECURITY.md` | § Supported Versions table | Add new row, mark old as EOL per policy |
| `SECURITY.md` | § Audit History | Add any new audit entries |
| `PRODUCTION_READINESS.md` | § Release Timeline | Add new row |
| `docs/BENCHMARKS.md` | `--save-baseline vX.Y.Z` / `--baseline vX.Y.Z` | Update to new version |

### 3. **Historical** — Must NOT be updated

These references document past events, incidents, or feature introductions.
Updating them would rewrite history and destroy context.

| File | What It Contains |
|------|------------------|
| `CHANGELOG.md` | All prior release entries — NEVER edit |
| `RELEASING.md` § Lessons Learned | References to past incidents (v0.2.1, v0.5.1) — keep |
| `RELEASING.md` § Git Hygiene examples | Example commit/branch names use historical versions — keep |
| `ARCHITECTURE.md` § Document History | Version history log — only append |
| `STABILITY.md` § Example Deprecation | Illustrative `since` / removal versions — keep |
| `docs/CONNECTION_RECOVERY.md` | "(v0.5.1)" / "(v0.5.0)" marking when features were added — keep |
| `PRODUCTION_READINESS.md` | "v0.5.0+" marking when features were added — keep |
| `docs/SQL_SERVER_COMPATIBILITY.md` | Support matrix entries for historical versions — keep |
| `docs/TEST_FAILURE_AUDIT.md` | Test audit history — keep |

---

## Crate-Specific Versions

All workspace crates inherit from `workspace.package.version`. **No individual
crate Cargo.toml updates are needed.**

Crates (all inherit workspace version):
- `crates/tds-protocol/Cargo.toml`
- `crates/mssql-tls/Cargo.toml`
- `crates/mssql-codec/Cargo.toml`
- `crates/mssql-types/Cargo.toml`
- `crates/mssql-auth/Cargo.toml`
- `crates/mssql-pool/Cargo.toml`
- `crates/mssql-client/Cargo.toml`
- `crates/mssql-derive/Cargo.toml`
- `crates/mssql-testing/Cargo.toml` (publish = false)

The `xtask` crate has its own version (`xtask/Cargo.toml`) and is `publish = false`.

---

## MSRV References

MSRV (Minimum Supported Rust Version) is referenced in many places. When the
MSRV is bumped (per the STABILITY.md § MSRV Increase Policy), update ALL of
these files consistently.

| File | What to Update |
|------|----------------|
| `Cargo.toml` (root) | `workspace.package.rust-version` |
| `xtask/Cargo.toml` | `rust-version` |
| `rust-toolchain.toml` | `channel` |
| `Justfile` | `msrv := "X.Y"` and version-pinned-tools comments |
| `README.md` | MSRV badge (label + URL to Rust release blog post) |
| `README.md` | "MSRV X.Y" in feature bullet list |
| `CLAUDE.md` | "MSRV X.Y" references (3 occurrences) |
| `ARCHITECTURE.md` | § 1 header "**MSRV Policy:**" line |
| `ARCHITECTURE.md` | § 6.6 "Current MSRV" and toolchain examples |
| `ARCHITECTURE.md` | Any `rust-version = "X.Y"` examples |
| `ARCHITECTURE.md` | Tree diagram "Pin to X.Y+" comment |
| `STABILITY.md` | § "Current MSRV" line |
| `CONTRIBUTING.md` | Prerequisites table |
| `CONTRIBUTING.md` | "version-pinned tools compatible with Rust X.Y" |
| `RELEASING.md` | Header `**MSRV:**` |
| `RELEASING.md` | CI Parity table `cargo +X.Y check` example |
| `PRODUCTION_READINESS.md` | "MSRV verification (X.Y)" and "MSRV X.Y documented" |
| `PRODUCTION_READINESS.md` | `cargo +X.Y check` example |
| `docs/BENCHMARKS.md` | Prerequisites "Rust X.Y+" |
| `docs/BENCHMARKS.md` | "Benchmarks run on Linux with Rust X.Y" footer |
| `docs/BENCHMARKS.md` | Environment spec "Rust: X.Y.0" |
| `xtask/src/main.rs` | `rustup run X.Y` example string in ci() output |
| `.github/workflows/ci.yml` | MSRV job reads from Cargo.toml automatically — no change needed |
| `.github/workflows/ci.yml` | Any MSRV reference in comments |
| `.github/workflows/release.yml` | Any MSRV reference in comments |

---

## Release Checklist

```bash
# 1. Update authoritative files (Category 1)
# Edit Cargo.toml: workspace.package.version = "X.Y.Z"
# Edit CHANGELOG.md: change [Unreleased] to [X.Y.Z] - YYYY-MM-DD

# 2. Update secondary files (Category 2)
# Update files listed in the "Secondary" table above

# 3. If bumping MSRV, update MSRV references (MSRV section above)

# 4. Run automated check
just version-refs-check

# 5. Verify with grep — should find only historical references
grep -rn "vPREV\|0\.PREV" --include="*.md" --include="*.toml" .

# 6. Run full release check
just release-check

# 7. Commit everything as ONE commit on a release branch
git checkout -b release/vX.Y.Z
git add -A
git commit -m "chore: release vX.Y.Z"
```

---

## Automation Coverage

The `just version-refs-check` command verifies:
- `Cargo.toml` `workspace.package.version` matches `CHANGELOG.md` latest entry
- All workspace crates inherit correctly
- No `[Unreleased]` section without content
- Basic grep for stale version patterns

**Not automated (manual verification required):**
- Historical vs. current context (requires human judgement)
- MSRV bump ripple (too many files across different formats)
- External references (docs.rs badges, crates.io shields)
- Example snippets inside documentation

Treat `just version-refs-check` as a floor, not a ceiling. Cross-reference this
document for everything it can't catch.

---

## Version Format

This project uses [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** (X.0.0): Breaking API changes (post-1.0 only)
- **MINOR** (0.X.0): New features, backwards compatible (pre-1.0: may include breaking changes)
- **PATCH** (0.0.X): Bug fixes, backwards compatible

Pre-1.0 releases may contain breaking changes in minor versions per
[Semver § 4](https://semver.org/#spec-item-4). See [STABILITY.md](../STABILITY.md)
for the full API stability policy.

---

*This document was last comprehensively audited for the v0.7.0 release.*
