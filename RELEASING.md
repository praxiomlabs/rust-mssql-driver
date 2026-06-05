# Releasing rust-mssql-driver

**MSRV:** 1.88 | **Edition:** 2024 | **Workspace:** 9 crates (8 published)

Releases are automated by [release-plz](https://release-plz.dev) in its
constrained Release-PR form. The history of the manual process this replaced —
including the tier-ordered publish workflow and the incident write-ups that
motivated the automation — is preserved in git history (`git log -- RELEASING.md`
and `git show 9e75f21^:.github/workflows/release.yml`).

## How a release happens

1. Work merges to `main` via PRs with [conventional commits](CONTRIBUTING.md#commit-messages).
   Commit types drive the version bump (pre-1.0: breaking → minor, else patch),
   cross-checked by `cargo-semver-checks`.
2. release-plz keeps a **Release PR** open/updated with the version bump,
   `CHANGELOG.md` entry, and `Cargo.lock` change. Nothing publishes while it sits.
3. Review the Release PR: the proposed version, the changelog, and the required
   CI checks (which run on it like any PR).
4. **Merging the Release PR is the release.** release-plz then publishes all
   8 crates to crates.io in dependency order, creates the `vX.Y.Z` tag, and
   creates the GitHub Release.

There is no manual `cargo publish`, no manual tag, no manual CHANGELOG edit.

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
