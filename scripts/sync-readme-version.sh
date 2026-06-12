#!/usr/bin/env bash
#
# Sync first-party crate dependency snippets in the docs to the workspace
# version (major.minor). release-plz bumps Cargo.toml and the CHANGELOG when it
# opens a Release PR, but it does NOT touch install snippets like
# `mssql-client = "0.12"` in README.md — so the doc-consistency gate
# (scripts/check-doc-consistency.sh, "Doc dependency snippets" check) fails on
# every Release PR until the snippet is hand-bumped.
#
# This script performs that bump deterministically. It is run by the
# release-plz workflow against the open Release PR branch, and can also be run
# locally. It mirrors the file list and crate list of the doc-consistency
# check so the two never disagree.
set -euo pipefail

cd "$(dirname "$0")/.."

WORKSPACE_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)".*/\1/')
EXPECTED_MINOR=${WORKSPACE_VERSION%.*}

# Keep in sync with scripts/check-doc-consistency.sh.
SNIPPET_CRATES='tds-protocol|mssql-client|mssql-auth|mssql-tls|mssql-codec|mssql-types|mssql-derive|mssql-driver-pool'

shopt -s nullglob
files=(README.md docs/*.md crates/*/README.md)

changed=0
for f in "${files[@]}"; do
    [ -f "$f" ] || continue
    before=$(cat "$f")
    # Rewrite the quoted version of any first-party dep line to major.minor.
    # Group 1 captures `<indent><crate> = "`; the version and closing quote are
    # replaced. Both `"X.Y"` and `"X.Y.Z"` forms are handled.
    sed -i -E \
        "s/^([[:space:]]*(${SNIPPET_CRATES}) = \")[0-9]+\.[0-9]+(\.[0-9]+)?\"/\1${EXPECTED_MINOR}\"/" \
        "$f"
    if [ "$before" != "$(cat "$f")" ]; then
        echo "synced $f → ${EXPECTED_MINOR}"
        changed=1
    fi
done

if [ "$changed" = "0" ]; then
    echo "all first-party dep snippets already at ${EXPECTED_MINOR}"
fi
