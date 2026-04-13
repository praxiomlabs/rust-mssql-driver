# Dependency Policy

This document captures the project's dependency-management philosophy. It's
written for maintainers making routine decisions (taking a dep bump, accepting
a new dep, ignoring an advisory) and for contributors proposing PRs that touch
`Cargo.toml`, `deny.toml`, or `.cargo/audit.toml`.

The policy distills decisions that were previously tribal knowledge into
written guidance. Where possible, it references the concrete precedents that
informed the decision — in particular the v0.7.0 release cycle, which forced
several judgment calls that are now documented as precedent here.

## Philosophy

1. **Minimize surface area.** Every dependency is code we are responsible for.
   A dep that's "fine today" can become unmaintained, CVE-ridden, or licensed
   incompatibly at any time. Prefer the standard library where it's adequate,
   well-maintained crates where it isn't.

2. **Correctness over convenience.** If a dep offers a nicer API but has
   questionable maintenance, unclear licensing, or a history of security
   incidents, don't adopt it just because it's ergonomic.

3. **Modern Rust, not cutting-edge Rust.** The project targets a rolling
   6-month MSRV window aligned with Tokio's policy (see
   [ARCHITECTURE.md § 6.6](../ARCHITECTURE.md) and [STABILITY.md § MSRV](../STABILITY.md#minimum-supported-rust-version-msrv)).
   Some deps bump their MSRV aggressively; we keep up but don't chase.

4. **Prefer pure Rust where it matters.** The TLS stack is [rustls](https://github.com/rustls/rustls),
   not OpenSSL. JSON is `serde_json`, not a C wrapper. This is partly about
   audit surface, partly about cross-compilation ergonomics, partly about
   avoiding the licensing tangles of mixed-license C code.

5. **Minimize feature flags on deps.** `tokio = { version = "1", features = ["rt", "net", "io-util"] }`
   is better than `tokio = { version = "1", features = ["full"] }`. We already
   did this cleanup in v0.7.0 — see commit `0d07504`.

## Adding a new dependency

Before adding a new dep, check:

| Criterion | How to check |
|-----------|--------------|
| **Necessary** | Is there a way to do this with the stdlib or existing deps? If the new dep only saves ~50 lines of code, it may not be worth the audit surface. |
| **Maintained** | [crates.io](https://crates.io/) lists recent releases, the GitHub repo has recent commits, issues are being triaged. "One maintainer, last commit 2 years ago" is a red flag. |
| **Popular** | Downloads and reverse-dep count on [crates.io](https://crates.io/). Popular isn't a guarantee of quality, but unpopular deps carry higher risk of being abandoned. |
| **License** | Must be in the `allow` list in [`deny.toml`](../deny.toml). MIT / Apache-2.0 / BSD-* / ISC are always fine. Unusual licenses require adding a new allow entry and documenting why. |
| **MSRV** | The dep's minimum Rust version must be ≤ our workspace MSRV, OR we must be willing to bump our MSRV (rare). |
| **Transitive deps** | Look at `cargo tree -p <your-crate>`. If the new dep pulls in 50 transitive crates, that's 50 new audit surfaces. Check if alternatives exist with leaner dep trees. |
| **Feature flags** | Only enable the features we actually use. Never use `features = ["full"]` blindly. |
| **Security history** | [RustSec advisory DB](https://rustsec.org/) — search for the crate name. A crate with a single old fixed advisory is fine; a crate with recurring advisories is not. |
| **no_std compatibility** | `tds-protocol` is `no_std + alloc` compatible. Deps in `tds-protocol` must not require `std`. Check `Cargo.toml` features for `std` / `alloc` flags. |

If you're unsure, open an issue asking before adding the dep. We'd rather have
the discussion up front than revert a PR after merge.

## Taking a dependency upgrade

Routine dep bumps arrive via Dependabot on Monday mornings. The workflow is:

1. **Patch bumps** (e.g., `0.18.6 → 0.18.7`): usually safe to merge if CI is
   green. Dependabot's PR shows the changelog diff.
2. **Minor bumps** (e.g., `0.18.x → 0.19.0` for 0.x crates, or `1.5.x → 1.6.0`
   for 1.x crates): read the changelog. 0.x crates can break API in minor
   bumps per SemVer. Check CI status carefully.
3. **Major bumps** (e.g., `1.x → 2.0`): treat as a mini-project. Read the
   upgrade guide. Expect code changes. May want to defer to a dedicated PR
   rather than merging dependabot's raw output.

### When to defer

Defer a dep bump when:

- It introduces breaking API changes that require adapter code in our
  codebase. Better to do this in a focused PR than a dependabot rebase loop.
- It bumps a transitive dep we don't want to bump yet (e.g., because our
  MSRV can't handle the transitive dep's MSRV).
- The changelog mentions behavior changes that need audit attention (e.g.,
  crypto library updates, parser rewrites).

### Bundled bumps

Dependabot is configured to group related deps in
[`.github/dependabot.yml`](../.github/dependabot.yml):

- `opentelemetry*` crates move together (version alignment is required — see
  [ARCHITECTURE.md § ADR-008](../ARCHITECTURE.md#adr-008-opentelemetry-version-alignment))
- `tokio` minor/patch bumps move together

Add new groupings when we discover another "moves together" set.

## Handling security advisories

The weekly [Security Audit workflow](../.github/workflows/security-audit.yml)
runs `cargo audit` and `cargo deny check`. When a new RUSTSEC advisory
lands in our dep tree, one of three things is true:

### Case A: An upgrade is available AND we can take it

This is the easy path. Run `cargo update` (or let Dependabot open a PR),
verify CI passes, merge. Close any tracking issue.

**Precedent**: v0.7.0 resolved 6 of 7 RUSTSEC advisories this way. The
`aws-lc-sys 0.35 → 0.39`, `rustls 0.23.36 → 0.23.37`, and `rustls-webpki
0.103.8 → 0.103.10` bumps all came from a single `cargo update` after
bumping MSRV.

### Case B: An upgrade is available BUT requires an MSRV bump

This is the tricky case. If the fix is in a new version that requires a
newer Rust, we must choose between bumping MSRV (to take the fix) or
ignoring the advisory (to hold MSRV).

**Policy**: bumping MSRV is permitted for security fixes per
[STABILITY.md § MSRV Increase Policy](../STABILITY.md#minimum-supported-rust-version-msrv).
MSRV bumps are _not_ considered breaking changes. Prefer the upgrade.

**Precedent**: v0.7.0 bumped MSRV from 1.85 to 1.88 specifically to take
`time 0.3.47` (RUSTSEC-2026-0009). The alternative would have been adding
`RUSTSEC-2026-0009` to the deny.toml ignore list with a "not exploited in
our use case" justification. We chose the MSRV bump because:

1. The project explicitly aims for "modern Rust" (see
   [CLAUDE.md](../CLAUDE.md) and [ARCHITECTURE.md](../ARCHITECTURE.md)).
2. Rust 1.88 had been stable for ~10 months, well within the 6-month rolling
   MSRV window aligned with Tokio's policy.
3. The deny.toml has historically preferred fixing over ignoring.
4. Pre-1.0 (0.7.0) is appropriate timing for MSRV bumps.

The MSRV bump unlocked other pinned deps as a side benefit (the `home`
crate pin was removed because MSRV 1.88 supports the newer version).

### Case C: No upgrade is available OR the affected code path is not reachable

Sometimes a crate has an unfixed advisory that's either not yet patched
upstream, or patched only in a pre-release. Sometimes the vulnerable
function isn't called from our code paths.

**Policy**: these may be added to the `[advisories] ignore` list in
`deny.toml`, but **only with**:

1. **A documented reason** — explain exactly why we're ignoring and
   what the exposure actually is in _our_ use.
2. **A tracking link** — usually an upstream issue, GitHub PR, or
   RustSec advisory link. If the issue has no tracking, create a local
   tracking issue first.
3. **A review trigger** — note a condition under which we'll reevaluate
   (e.g., "when `<upstream-crate>` 1.0 stabilizes", "when we upgrade to
   `<other-crate>` 2.0 which replaces this code path").
4. **Synced entries in `.cargo/audit.toml`** — `cargo audit` reads from
   `.cargo/audit.toml`, `cargo deny` reads from `deny.toml`. The
   doc-consistency linter catches when they drift (see
   `scripts/check-doc-consistency.sh`).

**Precedents in `deny.toml`** (as of v0.7.0):

- `RUSTSEC-2023-0071` (RSA Marvin timing attack): no upstream fix available,
  our RSA usage is client-side-local (CEK unwrapping, SSPI auth), not
  network-observable. Tracked at [RustCrypto/RSA#390](https://github.com/RustCrypto/RSA/issues/390).
- `RUSTSEC-2025-0134` (rustls-pemfile unmaintained): pulled indirectly via
  bollard (a dev-only testcontainers dep), migration path pending upstream.
- `RUSTSEC-2026-0066` (astral-tokio-tar PAX validation): only reachable via
  `testcontainers` → `mssql-testing` (publish=false). Low CVSS (1.7). Fix
  available only in a pre-release (0.6.1-rc1).

When in doubt, **upgrade rather than ignore**. The cost of a deferred MSRV
bump is usually lower than the long-term cost of documentation drift and
advisory re-review.

## deny.toml and .cargo/audit.toml sync

Both files ignore the same set of advisories; their formats differ but the
content must match. The [`scripts/check-doc-consistency.sh`](../scripts/check-doc-consistency.sh)
linter verifies this automatically. If you add an ignore, always add it to
both files and verify the linter passes.

## Removing a dependency

When removing a dep:

1. **Delete from the relevant Cargo.toml** (workspace or crate).
2. **Run `cargo machete`** to verify no other crate is still using it.
3. **Run `cargo update`** to clean up the lockfile.
4. **Review transitive deps** — did the removal free up other transitive
   deps that are now unused? If so, those should also exit the lockfile
   after `cargo update`.
5. **Check deny.toml** — if the removed dep (or a transitive dep that left
   the tree) was in a `skip-tree` or `skip` entry, clean that up too.

Don't leave dead deps sitting in `Cargo.toml` "just in case". Future you
can always re-add them.

## New license requirements

New license strings appear periodically. The authoritative list of allowed
licenses is in [`deny.toml`](../deny.toml) under `[licenses] allow`. When a
cargo-deny run fails with `license-not-allowed`:

1. **Check what the license actually is.** Run `cargo info <crate>` and
   look at the `license:` field. SPDX expressions like `MIT OR Apache-2.0`
   are common.
2. **Decide if it's acceptable.** The current policy allows all OSI-approved
   permissive licenses (MIT, Apache-2.0, BSD-2, BSD-3, ISC, Zlib, CC0,
   BSL-1.0, MIT-0, Unicode-3.0, CDLA-Permissive-2.0). Copyleft licenses
   (GPL, AGPL, LGPL) are _not_ acceptable for this project.
3. **If acceptable**, add the license string to the `allow` list with a
   comment explaining which crate pulls it in.
4. **If not acceptable**, find an alternative crate or file an upstream
   issue asking for license clarification.

**Precedent**: v0.7.0 added `MIT-0` to the allow list because the newer
`aws-lc-sys 0.39.1` introduced MIT-0-licensed source files. The comment in
`deny.toml` records this.

## Maintenance schedule

- **Weekly**: Dependabot opens minor/patch bump PRs (Monday 09:00 UTC).
- **Weekly**: Security Audit workflow runs (Monday 09:00 UTC), cross-checks
  `cargo audit` + `cargo deny` against RustSec database.
- **Weekly**: Token Health Check runs (Monday 09:15 UTC) to verify the
  crates.io token is still valid.
- **Per release**: Run `cargo update` once before the release cycle so
  transitive bumps land on `dev` with time for CI to catch regressions.
  Don't run `cargo update` inside the release prep commit itself — do it
  days earlier so you have time to respond if something breaks.
- **Quarterly-ish**: Review `cargo outdated` output for major-version
  bumps that Dependabot won't open automatically (outdated major versions).
  Decide case-by-case whether to tackle them.

## Questions?

If you're unsure about a dep change, open an issue or draft PR and tag the
CODEOWNERS for the affected area. It's always cheaper to discuss than to
revert.
