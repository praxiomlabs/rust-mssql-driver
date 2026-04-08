# Maintainers

This document lists the current maintainers of rust-mssql-driver, their areas
of focus, and how to contact them.

## Current Maintainers

| Name | GitHub | Areas |
|------|--------|-------|
| Justin Kindrix | [@jkindrix](https://github.com/jkindrix) | All areas (primary maintainer) |

## Responsibilities

Maintainers are responsible for:

1. **Review** — Reviewing pull requests in their areas of ownership (see
   [`.github/CODEOWNERS`](.github/CODEOWNERS)). Aim for a first response
   within one week of submission, even if it's just acknowledging that the
   PR is in the queue.

2. **Triage** — Labeling and prioritizing incoming issues. Closing obvious
   duplicates. Routing security reports through the private
   [Security Advisory](https://github.com/praxiomlabs/rust-mssql-driver/security/advisories/new)
   channel rather than discussing them publicly.

3. **Releases** — Following [RELEASING.md](RELEASING.md) to cut new versions
   when the dev branch has accumulated meaningful changes. Maintainers must
   ensure all Cardinal Rules in RELEASING.md are satisfied before pushing a
   release tag.

4. **Stewardship** — Keeping the project's documented policies (STABILITY.md,
   SECURITY.md, CODE_OF_CONDUCT.md, this file) honest and up to date. When a
   policy contradiction is discovered (as we found with MSRV breaking-change
   policy during the 0.7.0 release), resolve it promptly rather than leaving
   contradictory guidance in place.

5. **Continuity** — When a maintainer knows they'll be away for an extended
   period, note it here or in a pinned issue so contributors know what to
   expect. When a maintainer is no longer active, move them to the
   **Emeritus Maintainers** section below.

## Contact

- **Bug reports, feature requests, questions:** Use the
  [issue tracker](https://github.com/praxiomlabs/rust-mssql-driver/issues/new/choose)
  with the appropriate template, or start a
  [Discussion](https://github.com/praxiomlabs/rust-mssql-driver/discussions)
  for conversational topics.
- **Security vulnerabilities:** Report privately via
  [GitHub Security Advisories](https://github.com/praxiomlabs/rust-mssql-driver/security/advisories/new).
  See [SECURITY.md](SECURITY.md) for the full policy.
- **Code of conduct violations:** Contact the maintainers privately via the
  same Security Advisory channel. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Becoming a Maintainer

We welcome additional maintainers, especially contributors who have
demonstrated:

- **Sustained, high-quality contributions** — multiple merged PRs over at
  least a few months covering meaningful work (not just trivial fixes).
- **Deep knowledge in a specific area** — e.g., TDS protocol internals, TLS,
  the connection pool, or authentication strategies. A maintainer doesn't
  need to know everything; CODEOWNERS lets us scope ownership to specific
  crates.
- **Good reviewing judgment** — substantive, kind, actionable feedback on
  other people's PRs.
- **Alignment with project values** — the quality-over-speed and
  correctness-first philosophies described in
  [CONTRIBUTING.md](CONTRIBUTING.md).

If you think you'd be a good fit, open a Discussion (or email the current
maintainers via the Security Advisory channel) describing your interest and
the area you'd like to take ownership of. We'll work with you on a
trial period, then formalize the role with an update to this file and to
`.github/CODEOWNERS`.

## Decision Making

Today, decisions are made by the primary maintainer with input from the
community via issues and PRs. As the project grows, we'll move to a more
formal governance model — likely a simple "lazy consensus" approach where
significant decisions are proposed via issues and go through if nobody
objects within a reasonable window (e.g., one week).

Architectural decisions should be captured as ADRs in
[ARCHITECTURE.md](ARCHITECTURE.md) following the process described in
[CONTRIBUTING.md § Architecture Decision Records](CONTRIBUTING.md#architecture-decision-records-adrs).

## Emeritus Maintainers

*None yet.*
