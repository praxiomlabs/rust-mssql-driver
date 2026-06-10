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

3. **Releases** — Releases are automated by release-plz: merging the
   Release PR it maintains *is* the release (publish, tag, GitHub Release).
   Maintainers must ensure all Cardinal Rules in
   [RELEASING.md](RELEASING.md) are satisfied before merging a Release PR,
   and never publish or tag by hand.

4. **Stewardship** — Keeping the project's documented policies (STABILITY.md,
   SECURITY.md, CODE_OF_CONDUCT.md, this file) honest and up to date. When a
   policy contradiction is discovered (as we found with MSRV breaking-change
   policy during the 0.7.0 release), resolve it promptly rather than leaving
   contradictory guidance in place.

5. **Continuity** — When a maintainer will be away for an extended period, note
   it in a pinned issue so contributors know what to expect.

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

## Decision Making

Decisions are made by the primary maintainer with input from the community via
issues and PRs. Architectural decisions are captured as ADRs in
[ARCHITECTURE.md](ARCHITECTURE.md) following the process in
[CONTRIBUTING.md § Architecture Decision Records](CONTRIBUTING.md#architecture-decision-records-adrs).

Interested in helping maintain the project? Open a
[Discussion](https://github.com/praxiomlabs/rust-mssql-driver/discussions) —
sustained, high-quality contributions in an area (TDS protocol, TLS, the pool,
auth) are the path in.
