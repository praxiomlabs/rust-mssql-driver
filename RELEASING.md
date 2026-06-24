# Releasing rust-mssql-driver

**MSRV:** 1.88 | **Edition:** 2024 | **Workspace:** 9 crates (8 published)

Releases are automated by [release-plz](https://release-plz.dev) in its
constrained Release-PR form. The history of the manual process this replaced —
including the tier-ordered publish workflow and the incident write-ups that
motivated the automation — is preserved in git history (`git log -- RELEASING.md`
and `git show 91074b6^:.github/workflows/release.yml`).

## How a release happens

1. Work merges to `main` via PRs with [conventional commits](CONTRIBUTING.md#commit-messages).
   Commit types drive the version bump (pre-1.0: breaking → minor, else patch),
   cross-checked by `cargo-semver-checks`.
2. release-plz keeps a **Release PR** open/updated with the version bump,
   `CHANGELOG.md` entry, and `Cargo.lock` change. Nothing publishes while it sits.
3. Review the Release PR: the proposed version, the changelog, and the required
   CI checks (which run on it like any PR). Also check
   `git log <last-tag>..main -- Cargo.toml` for workspace-level dependency
   bumps — release-plz attributes commits to packages by file path, so a bump
   that only touches the root manifest is invisible to the generated changelog
   (this bit v0.11.0: the lru 0.16→0.17 requirement change was added by hand).
   Then complete the [pre-release live-server validation](#pre-release-live-server-validation)
   for the paths the release touches.
4. **Merging the Release PR is the release.** release-plz publishes the
   8 crates to crates.io in dependency order, and — only after every crate is
   up — creates the `vX.Y.Z` tag and the GitHub Release. (Publish first, tag
   last: a failed publish leaves no tag pointing at a half-released state.)

There is no manual `cargo publish`, no manual tag, no manual CHANGELOG edit.

## Pre-release live-server validation

CI cannot exercise every path: some features need live SQL Server, domain, or
Azure configuration that Docker CI can't provide, so their integration tests are
`#[ignore]`d and only the maintainer runs them locally. **A release must not be
tagged without running the live paths it could affect** — v0.9.0 shipped an
Always Encrypted bug despite local tests existing, precisely because there was
no checklist forcing the run.

Before merging the Release PR, diff `git log <last-tag>..main`, run the relevant
rows below, and **attest in the Release PR review which paths were exercised**
(when a release touches none — e.g. docs-only or pure-internal — say so
explicitly). Enforcement is by maintainer discipline, not an automated gate.

The container rows assume the `MSSQL_HOST/PORT/USER/PASSWORD` env and the full
ignored-suite command from [CLAUDE.md convention 8](CLAUDE.md) (the canonical,
drift-free source for the exact invocation); the rows below give the
distinguishing feature/binary filter to scope a single path.

| Path | Scope it with | Needs |
|------|---------------|-------|
| **Container suite** — TVP, bulk insert, temporal, collation, decimal, streaming | the full ignored-suite command (CLAUDE.md convention 8) | local SQL Server 2022 (`just sql-server-start`) |
| **Always Encrypted** (read + write) | `--all-features … -E 'binary(always_encrypted)'` | container provisioned with CMK/CEK (tracked in #86) |
| **Azure AD / FEDAUTH** (service principal, managed identity, certificate, default chain) | `cargo nextest run -p mssql-client --features azure-identity,cert-auth --run-ignored ignored-only -E 'binary(azure_sql)'` | live Azure SQL + `AZURE_SQL_TENANT_ID/CLIENT_ID/CLIENT_SECRET` (+ `AZURE_SQL_CLIENT_CERTIFICATE_PATH` for cert; managed-identity test needs Azure compute) |
| **Kerberos / integrated auth** | `MSSQL_KERBEROS_HOST=… KRB5CCNAME=… cargo nextest run -p mssql-auth --features integrated-auth --run-ignored ignored-only -E 'binary(kerberos_live)'` | live KDC with `MSSQLSvc/<host>` SPN + a `kinit`'d ticket |
| **FILESTREAM** (Windows only) | `cargo nextest run -p mssql-client --features filestream --run-ignored ignored-only -E 'binary(windows_filestream)'` | Windows + a FILESTREAM-enabled instance |

The standard pre-push gate (CLAUDE.md convention 8) deliberately **excludes** the
Azure, Kerberos, and client-cert suites because they need infrastructure neither
CI nor the local container provides — those are exactly the rows to run by hand
at release time.

## Rules that survive automation

- **Publishing to crates.io is irreversible.** You can yank, but never delete or
  re-upload a version. Review the Release PR like the publish button it is.
- **Never cancel the publish job mid-run.** If it fails or is interrupted,
  re-run it: release-plz is idempotent and skips already-published crates.
- **Don't hand-edit the workspace version.** Version bumps belong to the
  Release PR. (With `release_always = false`, a stray bump won't publish — but
  it will confuse the next Release PR.)

## If a release goes wrong

- **Publish job failed partway:** re-run the job (idempotent, resumes where it
  stopped). Verify afterward that all 8 crates show the new version on crates.io.
- **The recovery fix has to land as a direct push:** `release_always = false`
  refuses to publish from it ("current commit is not from a release PR").
  Temporarily set `release_always = true` in release-plz.toml, push, let the
  release job complete the partial publish, then revert in the next commit.
  This exact sequence recovered v0.11.0 (a dev-dependency cycle the old
  tier-ordered workflow had masked broke packaging for mssql-client; fixed
  permanently with a path-only mssql-derive workspace entry, guarded forward
  by the `Publish Dry-Run (packaging)` CI check).
- **A broken version shipped:** yank it and release a fix —
  `cargo yank --version X.Y.Z <crate>` for each affected crate, then land the
  fix and merge the next Release PR. Document the yank in the CHANGELOG entry.
- **crates.io auth failed:** rotate `CARGO_REGISTRY_TOKEN`
  (crates.io → Settings → Tokens, scope `publish-update`), update the GitHub
  secret, re-run the job. Planned follow-up: switch to crates.io Trusted
  Publishing (OIDC) and delete the token entirely.

## Post-release verification

- [ ] All 8 crates at the new version: `cargo search mssql-client` (spot-check others)
- [ ] Fresh-project install works: `cargo add mssql-client@X.Y.Z && cargo check`
- [ ] GitHub Release exists with changelog content
- [ ] docs.rs build is green (may take ~15–30 min)
