#!/usr/bin/env bash
# Verify (or update) the committed public-API snapshots for every published
# crate, using cargo-public-api. The snapshots make every change to the public
# API surface a reviewable diff — catching the additive/return-type/generic
# changes that cargo-semver-checks admits it can miss.
#
#   scripts/check-public-api.sh          # check (CI gate): fail on any undiffed change
#   scripts/check-public-api.sh update   # regenerate the committed snapshots
#
# Reproducibility: cargo-public-api emits rustdoc JSON, whose format varies
# across nightly toolchains, so the nightly is PINNED. Bump PUBLIC_API_NIGHTLY
# deliberately and regenerate (update) in the same change. Snapshots are
# generated on Linux with --all-features, so platform-gated items (e.g. Windows
# certificate store, FILESTREAM) are intentionally absent from the baseline.
set -euo pipefail

PUBLIC_API_NIGHTLY="${PUBLIC_API_NIGHTLY:-nightly-2025-12-09}"
PACKAGES=(
  tds-protocol
  mssql-types
  mssql-tls
  mssql-codec
  mssql-auth
  mssql-driver-pool
  mssql-client
  mssql-derive
)
MODE="${1:-check}"
DIR="public-api"

if ! cargo "+${PUBLIC_API_NIGHTLY}" --version >/dev/null 2>&1; then
  echo "error: toolchain ${PUBLIC_API_NIGHTLY} is not installed."
  echo "       rustup toolchain install ${PUBLIC_API_NIGHTLY} --profile minimal --component rust-docs"
  exit 2
fi
if ! cargo public-api --version >/dev/null 2>&1; then
  echo "error: cargo-public-api is not installed (cargo install cargo-public-api --locked)."
  exit 2
fi

mkdir -p "$DIR"
rc=0
for pkg in "${PACKAGES[@]}"; do
  snap="$DIR/${pkg}.txt"
  current="$(cargo "+${PUBLIC_API_NIGHTLY}" public-api -p "$pkg" --all-features 2>/dev/null)"
  if [ "$MODE" = "update" ]; then
    printf '%s\n' "$current" >"$snap"
    echo "updated $snap"
  elif [ ! -f "$snap" ]; then
    echo "::error::missing public-API snapshot $snap — run: scripts/check-public-api.sh update"
    rc=1
  elif ! delta="$(diff -u "$snap" <(printf '%s\n' "$current"))"; then
    echo "::error::public API changed for ${pkg}:"
    printf '%s\n' "$delta"
    echo "If this change is intended, run: scripts/check-public-api.sh update — and justify the diff in your PR."
    rc=1
  else
    echo "ok: ${pkg}"
  fi
done
exit "$rc"
