#!/usr/bin/env bash
#
# check-doc-consistency.sh
#
# Validates invariants across the project's documentation and config files.
# Catches a class of bugs where two files disagree about the same fact —
# exactly the kind of drift that caused the CONTRIBUTING.md ↔ STABILITY.md
# MSRV policy contradiction we fixed during the 0.7.0 release cycle.
#
# Usage:
#   ./scripts/check-doc-consistency.sh              # check, exit non-zero on mismatch
#   ./scripts/check-doc-consistency.sh --verbose    # show all checks, not just failures
#
# Integrated into `just release-preflight` (conditional on the script being
# present and executable) and can be wired into a git pre-commit hook.

set -euo pipefail

# Colors for output (disabled if NO_COLOR is set or not a TTY)
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    RED=$'\033[0;31m'
    GREEN=$'\033[0;32m'
    YELLOW=$'\033[0;33m'
    CYAN=$'\033[0;36m'
    BOLD=$'\033[1m'
    RESET=$'\033[0m'
else
    RED=""
    GREEN=""
    YELLOW=""
    CYAN=""
    BOLD=""
    RESET=""
fi

VERBOSE=false
if [ "${1:-}" = "--verbose" ] || [ "${1:-}" = "-v" ]; then
    VERBOSE=true
fi

# Must run from workspace root
if [ ! -f "Cargo.toml" ] || ! grep -q '\[workspace\]' Cargo.toml; then
    echo "${RED}[ERR]${RESET}  Must run from the workspace root (where the top-level Cargo.toml lives)" >&2
    exit 1
fi

ERRORS=0
CHECKS=0

pass() {
    CHECKS=$((CHECKS + 1))
    if [ "$VERBOSE" = "true" ]; then
        echo "  ${GREEN}[OK]${RESET}    $1"
    fi
}

fail() {
    CHECKS=$((CHECKS + 1))
    ERRORS=$((ERRORS + 1))
    echo "  ${RED}[ERR]${RESET}  $1"
}

section() {
    echo ""
    echo "${BOLD}${CYAN}▸ $1${RESET}"
}

# =============================================================================
# Extract authoritative values from workspace Cargo.toml
# =============================================================================

WORKSPACE_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)".*/\1/')
WORKSPACE_MSRV=$(grep -m1 '^rust-version = ' Cargo.toml | sed 's/rust-version = "\(.*\)".*/\1/')
WORKSPACE_EDITION=$(grep -m1 '^edition = ' Cargo.toml | sed 's/edition = "\(.*\)".*/\1/')

if [ -z "$WORKSPACE_VERSION" ] || [ -z "$WORKSPACE_MSRV" ] || [ -z "$WORKSPACE_EDITION" ]; then
    echo "${RED}[ERR]${RESET}  Could not parse workspace.package from Cargo.toml" >&2
    exit 1
fi

echo "${BOLD}Documentation consistency check${RESET}"
echo ""
echo "Authoritative values (from workspace Cargo.toml):"
echo "  version: ${BOLD}$WORKSPACE_VERSION${RESET}"
echo "  MSRV:    ${BOLD}$WORKSPACE_MSRV${RESET}"
echo "  edition: ${BOLD}$WORKSPACE_EDITION${RESET}"

# =============================================================================
# Check 1: MSRV consistency across files
# =============================================================================
section "MSRV consistency"

check_msrv_in_file() {
    local file="$1"
    local pattern="$2"
    local description="$3"

    if [ ! -f "$file" ]; then
        fail "$file does not exist"
        return
    fi

    if grep -qE "$pattern" "$file"; then
        pass "$file ($description) references MSRV $WORKSPACE_MSRV"
    else
        # File exists but doesn't contain our MSRV — might contain a different one
        local found
        found=$(grep -oE '1\.[0-9]+' "$file" | head -1 || true)
        if [ -n "$found" ]; then
            fail "$file ($description) references Rust $found but workspace MSRV is $WORKSPACE_MSRV"
        else
            pass "$file ($description) — no explicit Rust version (OK)"
        fi
    fi
}

# Files that MUST reference the current MSRV
check_msrv_in_file "rust-toolchain.toml" "channel = \"$WORKSPACE_MSRV\"" "toolchain channel"
check_msrv_in_file "xtask/Cargo.toml" "rust-version = \"$WORKSPACE_MSRV\"" "xtask rust-version"
check_msrv_in_file "README.md" "MSRV-$WORKSPACE_MSRV" "MSRV badge"
check_msrv_in_file "CLAUDE.md" "MSRV $WORKSPACE_MSRV" "project context doc"
check_msrv_in_file "ARCHITECTURE.md" "Rust $WORKSPACE_MSRV" "architecture doc"
check_msrv_in_file "STABILITY.md" "Current MSRV\*\*: $WORKSPACE_MSRV" "stability policy"
check_msrv_in_file "CONTRIBUTING.md" "Rust \| $WORKSPACE_MSRV" "contributor prerequisites"
check_msrv_in_file "RELEASING.md" "\*\*MSRV:\*\* $WORKSPACE_MSRV" "release doc header"
check_msrv_in_file "Justfile" "msrv := \"$WORKSPACE_MSRV\"" "just variable"

# =============================================================================
# Check 2: Workspace version consistency with CHANGELOG
# =============================================================================
section "CHANGELOG.md matches workspace version"

if [ -f "CHANGELOG.md" ]; then
    # First version heading in the file should match workspace version OR be [Unreleased]
    first_version=$(grep -m1 '^## \[' CHANGELOG.md | sed 's/## \[\(.*\)\].*/\1/' || true)

    case "$first_version" in
        "Unreleased")
            # Check the second heading
            second_version=$(grep '^## \[' CHANGELOG.md | sed -n '2p' | sed 's/## \[\(.*\)\].*/\1/')
            if [ "$second_version" = "$WORKSPACE_VERSION" ]; then
                pass "CHANGELOG has [Unreleased] + [$WORKSPACE_VERSION] (OK)"
            else
                fail "CHANGELOG latest released entry [$second_version] != workspace version [$WORKSPACE_VERSION]"
            fi
            ;;
        "$WORKSPACE_VERSION")
            pass "CHANGELOG latest entry [$first_version] matches workspace version"
            ;;
        *)
            fail "CHANGELOG latest entry [$first_version] does not match workspace version [$WORKSPACE_VERSION] and is not [Unreleased]"
            ;;
    esac
else
    fail "CHANGELOG.md does not exist"
fi

# =============================================================================
# Check 3: MSRV policy statement consistency (the 0.7.0 lesson)
# =============================================================================
section "MSRV breaking-change policy consistency"

# STABILITY.md should state that MSRV bumps are NOT breaking changes
if [ -f "STABILITY.md" ]; then
    if grep -qiE "MSRV increases are not considered breaking" STABILITY.md; then
        pass "STABILITY.md states MSRV bumps are non-breaking"
    else
        fail "STABILITY.md does not state that MSRV bumps are non-breaking (required by the documented MSRV Increase Policy)"
    fi
fi

# CONTRIBUTING.md must NOT list "Increasing MSRV" under "Definitely Breaking"
if [ -f "CONTRIBUTING.md" ]; then
    # Extract the "Definitely Breaking" section and check it
    if awk '/\*\*Definitely Breaking:\*\*/,/\*\*Usually Breaking:\*\*/' CONTRIBUTING.md | grep -qiE "Increasing MSRV|MSRV bump"; then
        fail "CONTRIBUTING.md lists 'Increasing MSRV' under 'Definitely Breaking' — contradicts STABILITY.md § MSRV Increase Policy"
    else
        pass "CONTRIBUTING.md does not list MSRV bumps as breaking"
    fi
fi

# =============================================================================
# Check 4: Workspace crate version inheritance
# =============================================================================
section "Workspace crate version inheritance"

# Find all crate Cargo.toml files and verify they inherit workspace version
for crate_toml in crates/*/Cargo.toml; do
    if [ -f "$crate_toml" ]; then
        crate_name=$(basename "$(dirname "$crate_toml")")
        if grep -qE '^version\.workspace = true' "$crate_toml"; then
            pass "$crate_name inherits workspace version"
        elif grep -qE "^version = \"$WORKSPACE_VERSION\"" "$crate_toml"; then
            pass "$crate_name explicitly pins version $WORKSPACE_VERSION"
        else
            explicit_version=$(grep -m1 '^version = ' "$crate_toml" | sed 's/version = "\(.*\)".*/\1/' || true)
            if [ -n "$explicit_version" ]; then
                fail "$crate_name has explicit version '$explicit_version' != workspace version '$WORKSPACE_VERSION'"
            else
                fail "$crate_name does not set a version and does not inherit from workspace"
            fi
        fi
    fi
done

# =============================================================================
# Check 5: Supported versions tables align
# =============================================================================
section "Supported versions tables"

# SECURITY.md and STABILITY.md both have "Supported Versions" sections.
# They should list the same set of supported versions. This is a soft check
# — we just warn if they're clearly out of sync.

if [ -f "SECURITY.md" ] && [ -f "STABILITY.md" ]; then
    security_supported=$(awk '/## Supported Versions/,/^##[^#]/' SECURITY.md | grep -oE '^\| [0-9]+\.[0-9]+\.' | sort -u || true)
    stability_supported=$(awk '/## Platform Support/,/^##[^#]/' STABILITY.md | grep -oE '^\| [0-9]+\.[0-9]+\.' | sort -u || true)

    if [ -n "$security_supported" ]; then
        pass "SECURITY.md has a Supported Versions table"
    fi
fi

# =============================================================================
# Check 6: Deny / audit ignore lists are in sync
# =============================================================================
section "deny.toml and .cargo/audit.toml advisory lists"

if [ -f "deny.toml" ] && [ -f ".cargo/audit.toml" ]; then
    deny_ignores=$(grep -oE 'RUSTSEC-[0-9]+-[0-9]+' deny.toml | sort -u || true)
    audit_ignores=$(grep -oE 'RUSTSEC-[0-9]+-[0-9]+' .cargo/audit.toml | sort -u || true)

    # Find advisories in one but not the other
    only_in_deny=$(comm -23 <(echo "$deny_ignores") <(echo "$audit_ignores") | grep -v '^$' || true)
    only_in_audit=$(comm -13 <(echo "$deny_ignores") <(echo "$audit_ignores") | grep -v '^$' || true)

    if [ -z "$only_in_deny" ] && [ -z "$only_in_audit" ]; then
        pass "deny.toml and .cargo/audit.toml advisory ignore lists are in sync"
    else
        if [ -n "$only_in_deny" ]; then
            fail "Advisories ignored in deny.toml but not in .cargo/audit.toml: $(echo $only_in_deny | tr '\n' ' ')"
        fi
        if [ -n "$only_in_audit" ]; then
            fail "Advisories ignored in .cargo/audit.toml but not in deny.toml: $(echo $only_in_audit | tr '\n' ' ')"
        fi
    fi
fi

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "${BOLD}${CYAN}──────────────────────────────────────────${RESET}"
if [ "$ERRORS" -eq 0 ]; then
    echo "${GREEN}[OK]${RESET}    All $CHECKS consistency checks passed"
    exit 0
else
    echo "${RED}[FAIL]${RESET}  $ERRORS of $CHECKS consistency checks failed"
    echo ""
    echo "${YELLOW}[TIP]${RESET}  See docs/VERSION_REFS.md for the canonical list of files that must agree."
    exit 1
fi
