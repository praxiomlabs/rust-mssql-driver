#!/usr/bin/env bash
# Changelog-completeness guard (issue #184).
#
# release-plz has silently dropped non-merge commits from a generated CHANGELOG
# section while bumping the version correctly (v0.13.0 / PR #170: two `fix`
# commits omitted). git-cliff rendered them fine; the drop happened in
# release-plz's own layer and recurs unpredictably. This guard fails when a
# non-merge commit in the release range is missing from the pending CHANGELOG
# section, so the gap is caught on the Release PR instead of after publish.
#
# Mechanism: release-plz's commit_parsers (release-plz.toml) skip only `^Merge`;
# every other non-merge commit lands in some group. AND release-plz attributes
# commits to packages by changed file paths, so a commit that touches only root
# files (release-plz.toml, RELEASING.md, .github/, the release commit itself,
# xtask, the publish=false crates) is legitimately absent from the changelog.
# So: every non-merge commit in <latest-tag>..<compare-ref> that touches a
# PUBLISHED crate directory must appear in the pending `## [X.Y.Z]` section.
#
# Usage: check-changelog-completeness.sh [<compare-ref>]
#   <compare-ref> defaults to origin/main — the commits the release will cover.
#   On the Release PR, pass the PR base sha (the main tip the CHANGELOG was
#   generated against).
#
# Self-gating: if the top versioned CHANGELOG section equals the latest release
# tag (no pending release), the check is a no-op — safe to run on any branch.
set -euo pipefail

CHANGELOG="CHANGELOG.md"
compare_ref="${1:-origin/main}"

# Resolve the compare ref, falling back for local runs without an origin remote.
if ! git rev-parse --verify --quiet "$compare_ref^{commit}" >/dev/null; then
  for alt in main HEAD; do
    if git rev-parse --verify --quiet "$alt^{commit}" >/dev/null; then
      compare_ref="$alt"
      break
    fi
  done
fi

# Latest release tag and its bare version.
last_tag="$(git tag --list 'v*' --sort=-v:refname | head -1)"
if [ -z "$last_tag" ]; then
  echo "No v* tags found; cannot determine the release range. Skipping."
  exit 0
fi
last_version="${last_tag#v}"

# The topmost versioned section header, e.g. "## [0.19.2](...) - 2026-06-18".
# This deliberately skips a leading "## [Unreleased]" (no version digits).
top_header="$(grep -m1 -E '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' "$CHANGELOG" || true)"
if [ -z "$top_header" ]; then
  echo "No versioned section in $CHANGELOG; nothing to check."
  exit 0
fi
top_version="$(sed -E 's/^## \[([0-9]+\.[0-9]+\.[0-9]+)\].*/\1/' <<<"$top_header")"

if [ "$top_version" = "$last_version" ]; then
  echo "Top CHANGELOG section [$top_version] is the released version; no pending"
  echo "release to check."
  exit 0
fi

# Body of the pending section: from its header line to the next "## [".
section="$(awk -v hdr="$top_header" '
  index($0, hdr) == 1 { grab = 1; next }
  grab && /^## \[/ { exit }
  grab { print }
' "$CHANGELOG")"

# Normalize text for tolerant substring matching: drop markdown link targets,
# issue/PR numbers (release-plz rewrites "(#42)" into a link), and markdown
# punctuation; lowercase; collapse whitespace.
normalize() {
  sed -E '
    s/\]\([^)]*\)//g
    s/#[0-9]+//g
    s/[][()*`_]//g
    s/[[:space:]]+/ /g
  ' | tr '[:upper:]' '[:lower:]'
}

norm_section="$(printf '%s\n' "$section" | normalize)"

missing=0
checked=0
while IFS=$'\t' read -r sha subject; do
  [ -z "$sha" ] && continue
  # release-plz skips only merge-like subjects.
  case "$subject" in
  Merge\ *) continue ;;
  esac
  checked=$((checked + 1))
  # Description = subject minus the conventional "type(scope)!: " prefix; if
  # there is no such prefix, release-plz renders the whole subject, so use it.
  desc="$(sed -E 's/^[a-zA-Z]+(\([^)]*\))?!?:[[:space:]]*//' <<<"$subject")"
  norm_desc="$(printf '%s' "$desc" | normalize | sed -E 's/^ //; s/ $//')"
  [ -z "$norm_desc" ] && continue
  if ! printf '%s' "$norm_section" | grep -qF -- "$norm_desc"; then
    echo "::error::CHANGELOG [$top_version] is missing commit $sha: '$subject'"
    missing=1
  fi
# Only commits touching a published crate dir are attributed to the changelog
# by release-plz. Include crates/ and exclude the publish=false crates, so a
# newly added published crate is covered automatically (a false positive is a
# loud, easy fix; silently skipping a real crate would defeat the guard).
done < <(git log --no-merges --pretty=$'%h\t%s' "$last_tag..$compare_ref" -- \
  'crates/' \
  ':(exclude)crates/api-smoke/' \
  ':(exclude)crates/doc-tests/' \
  ':(exclude)crates/mssql-testing/')

if [ "$missing" -ne 0 ]; then
  echo
  echo "The pending CHANGELOG [$top_version] section is incomplete (issue #184:"
  echo "release-plz can silently drop commits). Restore the missing entries on"
  echo "the Release PR branch — matching git-cliff's rendering — before merging."
  exit 1
fi

echo "CHANGELOG [$top_version] covers all $checked non-merge commits in" \
  "$last_tag..$compare_ref."
