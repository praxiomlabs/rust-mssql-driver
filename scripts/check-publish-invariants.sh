#!/usr/bin/env bash
#
# Guards the v0.11.0 partial-publish incident class: a *versioned* first-party
# dev- or build-dependency.
#
# cargo strips path-only dev/build-dependencies when packaging a crate, but keeps
# versioned ones. So a published crate that dev-depends on a first-party crate
# *with a version* demands that exact version on the crates.io index at publish
# time. release-plz publishes crate-by-crate in runtime-dependency order and
# ignores dev-deps for ordering, so that demand cannot be satisfied — the release
# breaks mid-publish (exactly what happened to mssql-client -> mssql-derive in
# v0.11.0). First-party dev/build-dependencies must therefore be path-only.
#
# Unlike `cargo publish --dry-run`, this invariant is registry-independent: it
# holds identically on release PRs, where the workspace version is bumped ahead
# of the crates.io index.
set -euo pipefail

violations=$(cargo metadata --no-deps --format-version 1 | jq -r '
  .packages as $p
  | ($p | map(.name)) as $members
  | $p[]
  | select(.publish != [])            # skip publish = false (never reaches the registry)
  | .name as $pkg
  | .dependencies[]
  | select(.kind == "dev" or .kind == "build")
  | select([.name] | inside($members))
  | select(.req != "*")
  | "  - \($pkg): \(.kind)-dependency on first-party `\(.name)` carries version `\(.req)` (must be path-only)"
')

if [ -n "$violations" ]; then
  echo "error: versioned first-party dev/build-dependency found (v0.11.0 publish-incident class):"
  echo "$violations"
  echo
  echo "Fix: declare these as path-only in [workspace.dependencies] (drop the version field)."
  exit 1
fi

echo "OK: no versioned first-party dev/build-dependencies in publishable crates."
