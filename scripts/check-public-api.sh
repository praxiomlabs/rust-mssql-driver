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
# deliberately and regenerate (update) in the same change.
#
# Platform: the snapshot set is platform-specific where the public surface
# diverges (#283):
#   * Linux (default): all crates, --all-features -> public-api/<crate>.txt
#   * Windows (the CI windows-latest leg): only the crates whose public API
#     diverges by platform -> public-api/<crate>.windows.txt. These carry the
#     cfg(windows)/Windows-feature items (FILESTREAM, the Windows certificate
#     store CMK provider, SSPI auth) that the Linux --all-features baseline
#     physically cannot see. integrated-auth is excluded from the Windows
#     feature set: libgssapi is Linux-only and will not build on Windows.
# The remaining crates are platform-independent — their Linux baseline freezes
# the whole surface. Add a crate to WINDOWS_ENTRIES if you introduce
# cfg(windows) public API to it.
set -euo pipefail

PUBLIC_API_NIGHTLY="${PUBLIC_API_NIGHTLY:-nightly-2025-12-09}"
MODE="${1:-check}"
DIR="public-api"

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN* | *NT*) PLATFORM=windows ;;
  *) PLATFORM=linux ;;
esac

# Each entry: "package|snapshot-suffix|feature-args" (feature-args is word-split).
LINUX_ENTRIES=(
  "tds-protocol||--all-features"
  "mssql-types||--all-features"
  "mssql-tls||--all-features"
  "mssql-codec||--all-features"
  "mssql-auth||--all-features"
  "mssql-driver-pool||--all-features"
  "mssql-client||--all-features"
  "mssql-derive||--all-features"
)
# Only the Windows-only features are enabled here — they are what the Linux
# baseline cannot see. Cross-platform features (azure-*, json, otel, …) are
# already frozen by the Linux baseline; including them here would add nothing
# but double-maintenance, and cert-auth/azure pull openssl-sys, which does not
# build cleanly on windows-latest. windows-certstore implies always-encrypted.
WINDOWS_ENTRIES=(
  "mssql-auth|.windows|--features sspi-auth,windows-certstore"
  "mssql-client|.windows|--features filestream,sspi-auth"
)

if [ "$PLATFORM" = windows ]; then
  ENTRIES=("${WINDOWS_ENTRIES[@]}")
else
  ENTRIES=("${LINUX_ENTRIES[@]}")
fi

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
for entry in "${ENTRIES[@]}"; do
  IFS='|' read -r pkg suffix feats <<<"$entry"
  snap="$DIR/${pkg}${suffix}.txt"
  # shellcheck disable=SC2086 # $feats is intentionally word-split into args
  # tr -d '\r' keeps the snapshot LF-only regardless of host (no-op on Linux),
  # so Windows-generated baselines diff cleanly against committed LF files.
  current="$(cargo "+${PUBLIC_API_NIGHTLY}" public-api -p "$pkg" $feats 2>/dev/null | tr -d '\r')"
  if [ "$MODE" = "update" ]; then
    printf '%s\n' "$current" >"$snap"
    echo "updated $snap"
  elif [ ! -f "$snap" ]; then
    echo "::error::missing public-API snapshot $snap — run: scripts/check-public-api.sh update"
    rc=1
  elif ! delta="$(diff -u "$snap" <(printf '%s\n' "$current"))"; then
    echo "::error::public API changed for ${pkg} (${PLATFORM}):"
    printf '%s\n' "$delta"
    echo "If this change is intended, run: scripts/check-public-api.sh update — and justify the diff in your PR."
    rc=1
  else
    echo "ok: ${pkg} (${PLATFORM})"
  fi
done
exit "$rc"
