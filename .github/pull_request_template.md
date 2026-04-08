<!--
Thanks for contributing to rust-mssql-driver! Please fill out this template to
help reviewers understand your change quickly.

If this is your first PR, also see CONTRIBUTING.md for setup and style guidelines.
-->

## Summary

<!-- One or two sentences: what does this PR do and why? -->

## Linked issues

<!--
If this PR closes an issue, use "Closes #123". GitHub will auto-close the
issue when the PR merges. If it relates to an issue without closing it, use
"Refs #123".
-->

Closes #

## Type of change

<!-- Check all that apply. -->

- [ ] 🐛 Bug fix (non-breaking change that fixes an issue)
- [ ] ✨ New feature (non-breaking change that adds functionality)
- [ ] 💥 Breaking change (fix or feature that changes existing API — see breaking-change checklist below)
- [ ] 📝 Documentation only
- [ ] 🔧 Refactoring (no behavioral change)
- [ ] 🚀 Performance improvement
- [ ] ✅ Test-only change
- [ ] 🏗️ Build / CI / tooling

## Test plan

<!--
How did you verify this change works? List the specific commands you ran.
CI will re-run these on all platforms, but knowing what you tested locally
helps reviewers focus.
-->

- [ ] `cargo test --workspace --all-features` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo fmt --all --check` is clean
- [ ] For behavioral changes: new or updated tests cover the change
- [ ] For integration-affecting changes: ran against live SQL Server (`just sql-server-start` + ignored tests)

## Documentation updates

<!-- Documentation is part of the change, not an afterthought. Check what you've updated. -->

- [ ] Updated rustdoc comments on new/changed public items
- [ ] Updated CHANGELOG.md `[Unreleased]` section (under Added/Fixed/Changed/Deprecated/Removed/Security)
- [ ] Updated README.md if the change affects installation, features, or quick-start examples
- [ ] Updated relevant files in `docs/` if the change affects user-facing behavior
- [ ] Updated ARCHITECTURE.md if the change introduces an architectural decision (add or update an ADR)

## Breaking changes

<!--
If this PR introduces a breaking change, expand this section. Otherwise,
delete everything between the HTML comment markers below.
-->

<!-- BREAKING-CHANGE-BEGIN

### What breaks

<!-- Describe the specific API surface that changes, removals, or semantic shifts. -->

### Migration guide

<!-- Concrete before/after code for users. -->

```rust
// Before (0.x.y):

// After (0.x+1.0):
```

### Pre-1.0 policy check

- [ ] This change is allowed under [STABILITY.md § Pre-1.0 Releases](../blob/main/STABILITY.md#pre-10-releases-0xy)
      (minor bumps may contain breaking changes pre-1.0)
- [ ] Documented in CHANGELOG.md under a `### Breaking Changes` heading
- [ ] **MSRV bumps are NOT a breaking change** per
      [STABILITY.md § MSRV Increase Policy](../blob/main/STABILITY.md#minimum-supported-rust-version-msrv).
      If you're bumping MSRV, mark this as a non-breaking "Changed" entry, not a breaking change.

BREAKING-CHANGE-END -->

## Security considerations

<!--
For any PR that touches authentication, TLS, credential handling, SQL
generation, parsing untrusted input, or unsafe code blocks: please describe
the security reasoning. For all other PRs, delete this section.
-->

## Additional notes

<!-- Anything else reviewers should know: design alternatives considered, trade-offs, related PRs, follow-ups. -->
