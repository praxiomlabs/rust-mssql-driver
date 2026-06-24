# ============================================================================
# rust-mssql-driver Development Justfile
# ============================================================================
#
# Modern command runner for the rust-mssql-driver SQL Server client.
#
# RECIPE NAMING CONVENTION:
#   - Base recipes (e.g., `build`, `test`, `clippy`) use DEFAULT features
#     and work on all platforms without additional dependencies.
#   - `-all` variants (e.g., `build-all`, `test-all`, `clippy-all`) use
#     --all-features and require libkrb5-dev on Linux for Kerberos support.
#
# QUICK START (new developers):
#   just bootstrap    - Full setup: system packages + tools + hooks (recommended)
#   just ci-all       - Verify everything works with all features
#
# ALTERNATIVE (no sudo, default features only):
#   just setup-all    - Install cargo tools + git hooks
#   just ci           - Run CI pipeline with default features
#
# DAILY WORKFLOW:
#   just quick        - Fast feedback: fmt + clippy + check (no tests)
#   just dev          - Full local cycle: build + test + lint
#
# REQUIREMENTS:
#   - Just >= 1.23.0 (for [group], [confirm], [doc] attributes)
#   - Rust toolchain (rustup recommended)
#   - For --all-features on Linux: libkrb5-dev + libclang-dev (installed by bootstrap)
#
# ============================================================================

# ----------------------------------------------------------------------------
# Project Configuration
# ----------------------------------------------------------------------------

project_name := "rust-mssql-driver"
# Version is read dynamically from Cargo.toml to avoid drift
version := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "mssql-client") | .version'`
msrv := "1.88"
edition := "2024"

# ----------------------------------------------------------------------------
# Tool Configuration (can be overridden via environment)
# ----------------------------------------------------------------------------

cargo := env_var_or_default("CARGO", "cargo")
docker := env_var_or_default("DOCKER", "docker")

# Parallel jobs: auto-detect CPU count
jobs := env_var_or_default("JOBS", num_cpus())

# Runtime configuration
rust_log := env_var_or_default("RUST_LOG", "info")
rust_backtrace := env_var_or_default("RUST_BACKTRACE", "1")

# SQL Server test configuration
mssql_host := env_var_or_default("MSSQL_HOST", "localhost")
mssql_port := env_var_or_default("MSSQL_PORT", "1433")
mssql_user := env_var_or_default("MSSQL_USER", "sa")
mssql_password := env_var_or_default("MSSQL_PASSWORD", "YourStrong@Passw0rd")

# Fuzz configuration
fuzz_time := env_var_or_default("FUZZ_TIME", "60")
fuzz_target := env_var_or_default("FUZZ_TARGET", "parse_packet")

# Paths
fuzz_dir := "fuzz"
target_dir := "target"

# ----------------------------------------------------------------------------
# Platform Detection
# ----------------------------------------------------------------------------

platform := if os() == "linux" { "linux" } else if os() == "macos" { "macos" } else { "windows" }
open_cmd := if os() == "linux" { "xdg-open" } else if os() == "macos" { "open" } else { "start" }

# ----------------------------------------------------------------------------
# ANSI Color Codes
# ----------------------------------------------------------------------------

reset := '\033[0m'
bold := '\033[1m'
green := '\033[0;32m'
yellow := '\033[0;33m'
red := '\033[0;31m'
cyan := '\033[0;36m'
blue := '\033[0;34m'
magenta := '\033[0;35m'

# ----------------------------------------------------------------------------
# Default Recipe & Settings
# ----------------------------------------------------------------------------

# Show help by default
default:
    @just --list --unsorted

# Load .env file if present
set dotenv-load

# Use bash with strict error handling
# -e: Exit on error
# -u: Error on undefined variables
# -o pipefail: Pipe failures propagate
set shell := ["bash", "-euo", "pipefail", "-c"]

# Export all variables to child processes
set export

# ============================================================================
# SETUP & PREREQUISITES
# ============================================================================

[group('setup')]
[doc("Check development environment and show missing dependencies")]
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Development Environment Check ══════{{reset}}\n\n'

    MISSING=0

    # Check required tools
    printf '{{cyan}}Required Tools:{{reset}}\n'
    for tool in rustc cargo just jq; do
        if command -v "$tool" &> /dev/null; then
            printf '  {{green}}✓{{reset}} %s (%s)\n' "$tool" "$($tool --version 2>/dev/null | head -1)"
        else
            printf '  {{red}}✗{{reset}} %s (not found)\n' "$tool"
            MISSING=1
        fi
    done

    # Check Rust components
    printf '\n{{cyan}}Rust Components:{{reset}}\n'
    for component in rustfmt clippy; do
        if rustup component list --installed 2>/dev/null | grep -q "$component"; then
            printf '  {{green}}✓{{reset}} %s\n' "$component"
        else
            printf '  {{red}}✗{{reset}} %s (run: rustup component add %s)\n' "$component" "$component"
            MISSING=1
        fi
    done

    # Check optional cargo extensions
    printf '\n{{cyan}}Optional Cargo Extensions:{{reset}}\n'
    for tool in nextest llvm-cov audit deny machete semver-checks; do
        if cargo $tool --version &> /dev/null 2>&1; then
            printf '  {{green}}✓{{reset}} cargo-%s\n' "$tool"
        else
            printf '  {{yellow}}○{{reset}} cargo-%s (optional)\n' "$tool"
        fi
    done

    # Check platform-specific dependencies
    printf '\n{{cyan}}Platform Dependencies ({{platform}}):{{reset}}\n'
    if [[ "{{platform}}" == "linux" ]]; then
        LINUX_MISSING=0
        if pkg-config --exists krb5-gssapi 2>/dev/null; then
            printf '  {{green}}✓{{reset}} libkrb5-dev (Kerberos support)\n'
        else
            printf '  {{yellow}}○{{reset}} libkrb5-dev (needed for --all-features)\n'
            LINUX_MISSING=1
        fi
        if command -v llvm-config &> /dev/null || [ -f /usr/lib/llvm-*/lib/libclang.so ]; then
            printf '  {{green}}✓{{reset}} libclang-dev (bindgen support)\n'
        else
            printf '  {{yellow}}○{{reset}} libclang-dev (needed for --all-features)\n'
            LINUX_MISSING=1
        fi
        if [[ $LINUX_MISSING -eq 1 ]]; then
            printf '       Install: sudo apt-get install libkrb5-dev libclang-dev\n'
        fi
    else
        printf '  {{green}}✓{{reset}} No additional dependencies needed\n'
    fi

    printf '\n'
    if [[ $MISSING -eq 1 ]]; then
        printf '{{yellow}}[WARN]{{reset}} Some required dependencies are missing\n'
        exit 1
    else
        printf '{{green}}[OK]{{reset}}   Development environment ready\n'
    fi

[group('setup')]
[doc("Install Linux dependencies for --all-features (Kerberos)")]
setup-linux:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "{{platform}}" != "linux" ]]; then
        printf '{{yellow}}[WARN]{{reset}} This command is for Linux only\n'
        exit 0
    fi
    printf '{{cyan}}[INFO]{{reset}} Installing dependencies for Kerberos support...\n'
    printf '{{cyan}}[INFO]{{reset}} - libkrb5-dev: Kerberos/GSSAPI headers\n'
    printf '{{cyan}}[INFO]{{reset}} - libclang-dev: Required for bindgen (FFI generation)\n'
    sudo apt-get update && sudo apt-get install -y libkrb5-dev libclang-dev
    printf '{{green}}[OK]{{reset}}   Linux dependencies installed\n'

[group('setup')]
[doc("Install recommended cargo extensions (version-pinned for Rust 1.88)")]
setup-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Installing recommended cargo extensions...\n'
    printf '{{cyan}}[INFO]{{reset}} Note: Using version pins compatible with Rust 1.88\n'

    # Core testing and coverage
    cargo install cargo-nextest@0.9.100 --locked
    cargo install cargo-llvm-cov --locked

    # Security and dependency auditing
    cargo install cargo-audit --locked
    cargo install cargo-deny@0.18.3 --locked

    # Code quality
    cargo install cargo-machete@0.7.0 --locked
    cargo install cargo-semver-checks@0.42.0 --locked

    # Feature flag matrix validation (used by `cargo xtask check-features` and CI)
    cargo install cargo-hack --locked

    # Development workflow
    cargo install cargo-watch --locked

    printf '{{green}}[OK]{{reset}}   Tools installed\n'

[group('setup')]
[doc("Install git pre-commit hooks for code quality")]
setup-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Installing git pre-commit hooks...\n'

    # Create hooks directory if it doesn't exist
    mkdir -p .git/hooks

    # Create pre-commit hook using printf to avoid heredoc parsing issues
    printf '%s\n' '#!/usr/bin/env bash' > .git/hooks/pre-commit
    printf '%s\n' 'set -euo pipefail' >> .git/hooks/pre-commit
    printf '%s\n' '' >> .git/hooks/pre-commit
    printf '%s\n' 'echo "Running pre-commit checks..."' >> .git/hooks/pre-commit
    printf '%s\n' '' >> .git/hooks/pre-commit
    printf '%s\n' '# Check formatting' >> .git/hooks/pre-commit
    printf '%s\n' 'if ! cargo fmt --all -- --check 2>/dev/null; then' >> .git/hooks/pre-commit
    printf '%s\n' '    echo "❌ Formatting check failed. Run '\''cargo fmt --all'\'' to fix."' >> .git/hooks/pre-commit
    printf '%s\n' '    exit 1' >> .git/hooks/pre-commit
    printf '%s\n' 'fi' >> .git/hooks/pre-commit
    printf '%s\n' 'echo "✓ Format check passed"' >> .git/hooks/pre-commit
    printf '%s\n' '' >> .git/hooks/pre-commit
    printf '%s\n' '# Run clippy (default features for speed)' >> .git/hooks/pre-commit
    printf '%s\n' 'if ! cargo clippy --workspace --all-targets -- -D warnings 2>/dev/null; then' >> .git/hooks/pre-commit
    printf '%s\n' '    echo "❌ Clippy check failed. Fix the warnings above."' >> .git/hooks/pre-commit
    printf '%s\n' '    exit 1' >> .git/hooks/pre-commit
    printf '%s\n' 'fi' >> .git/hooks/pre-commit
    printf '%s\n' 'echo "✓ Clippy check passed"' >> .git/hooks/pre-commit
    printf '%s\n' '' >> .git/hooks/pre-commit
    printf '%s\n' '# Quick type check' >> .git/hooks/pre-commit
    printf '%s\n' 'if ! cargo check --workspace 2>/dev/null; then' >> .git/hooks/pre-commit
    printf '%s\n' '    echo "❌ Type check failed."' >> .git/hooks/pre-commit
    printf '%s\n' '    exit 1' >> .git/hooks/pre-commit
    printf '%s\n' 'fi' >> .git/hooks/pre-commit
    printf '%s\n' 'echo "✓ Type check passed"' >> .git/hooks/pre-commit
    printf '%s\n' '' >> .git/hooks/pre-commit
    printf '%s\n' 'echo "✅ All pre-commit checks passed!"' >> .git/hooks/pre-commit

    chmod +x .git/hooks/pre-commit
    printf '{{green}}[OK]{{reset}}   Pre-commit hook installed\n'
    printf '{{cyan}}[INFO]{{reset}} Hook will run: fmt-check, clippy, check\n'

[group('setup')]
[doc("Complete development environment setup")]
setup-all: setup setup-tools setup-hooks
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{green}}══════ Development Environment Complete ══════{{reset}}\n\n'
    printf 'All tools installed and hooks configured.\n'
    printf 'Run {{cyan}}just ci{{reset}} to verify everything works.\n\n'
    if [[ "{{platform}}" == "linux" ]]; then
        if ! pkg-config --exists krb5-gssapi 2>/dev/null || ! (command -v llvm-config &> /dev/null || [ -f /usr/lib/llvm-*/lib/libclang.so ]); then
            printf '{{yellow}}[NOTE]{{reset}} For --all-features support (Kerberos), run:\n'
            printf '       {{cyan}}just bootstrap{{reset}}  (includes sudo for system packages)\n'
            printf '       {{cyan}}— or —{{reset}}\n'
            printf '       sudo apt-get install libkrb5-dev libclang-dev\n\n'
        fi
    fi

[group('setup')]
[doc("Full bootstrap: system packages + tools + hooks (Linux: prompts for sudo)")]
bootstrap:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Full Development Bootstrap ══════{{reset}}\n\n'

    # Step 1: System packages (Linux only, requires sudo)
    if [[ "{{platform}}" == "linux" ]]; then
        NEED_SYSTEM=0
        if ! pkg-config --exists krb5-gssapi 2>/dev/null; then
            NEED_SYSTEM=1
        fi
        if ! (command -v llvm-config &> /dev/null || [ -f /usr/lib/llvm-*/lib/libclang.so ]); then
            NEED_SYSTEM=1
        fi

        if [[ $NEED_SYSTEM -eq 1 ]]; then
            printf '{{cyan}}[1/4]{{reset}} Installing system packages (requires sudo)...\n'
            printf '{{cyan}}[INFO]{{reset}} - libkrb5-dev: Kerberos/GSSAPI headers\n'
            printf '{{cyan}}[INFO]{{reset}} - libclang-dev: Required for bindgen (FFI generation)\n'
            sudo apt-get update && sudo apt-get install -y libkrb5-dev libclang-dev
            printf '{{green}}[OK]{{reset}}   System packages installed\n\n'
        else
            printf '{{cyan}}[1/4]{{reset}} System packages already installed\n'
            printf '{{green}}[OK]{{reset}}   Skipping\n\n'
        fi
    else
        printf '{{cyan}}[1/4]{{reset}} System packages (Linux-only)\n'
        printf '{{green}}[OK]{{reset}}   Skipping (not Linux)\n\n'
    fi

    # Step 2: Check environment
    printf '{{cyan}}[2/4]{{reset}} Checking development environment...\n'
    just setup

    # Step 3: Install cargo tools
    printf '\n{{cyan}}[3/4]{{reset}} Installing cargo extensions...\n'
    just setup-tools

    # Step 4: Install git hooks
    printf '\n{{cyan}}[4/4]{{reset}} Installing git hooks...\n'
    just setup-hooks

    printf '\n{{bold}}{{green}}══════ Bootstrap Complete ══════{{reset}}\n\n'
    printf 'Your development environment is fully configured.\n'
    printf 'Run {{cyan}}just ci-all{{reset}} to verify everything works with all features.\n\n'

# ============================================================================
# CORE BUILD RECIPES
# ============================================================================

[group('build')]
[doc("Build workspace (default features, works everywhere)")]
build:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Building (debug) ══════{{reset}}\n\n'
    {{cargo}} build --workspace -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Build complete\n'

[group('build')]
[doc("Build workspace with ALL features (requires libkrb5-dev on Linux)")]
build-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Building (debug, all features) ══════{{reset}}\n\n'
    {{cargo}} build --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Build complete\n'

[group('build')]
[doc("Build workspace in release mode")]
release:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Building (release) ══════{{reset}}\n\n'
    {{cargo}} build --workspace --release -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Release build complete\n'

[group('build')]
[doc("Build workspace in release mode with ALL features")]
release-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Building (release, all features) ══════{{reset}}\n\n'
    {{cargo}} build --workspace --all-features --release -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Release build complete\n'

[group('build')]
[doc("Fast type check (default features)")]
check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Type checking...\n'
    {{cargo}} check --workspace -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Type check passed\n'

[group('build')]
[doc("Fast type check with ALL features")]
check-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Type checking (all features)...\n'
    {{cargo}} check --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Type check passed\n'

[group('build')]
[doc("Analyze build times")]
build-timing:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building with timing analysis...\n'
    {{cargo}} build --workspace --timings
    printf '{{green}}[OK]{{reset}}   Build timing report generated (see target/cargo-timings/)\n'

[group('build')]
[confirm("This will delete all build artifacts. Continue?")]
[doc("Clean all build artifacts")]
clean:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Cleaning build artifacts...\n'
    {{cargo}} clean
    rm -rf coverage/ lcov.info *.profraw *.profdata
    printf '{{green}}[OK]{{reset}}   Clean complete\n'

[group('build')]
[doc("Clean and rebuild from scratch")]
rebuild: clean build

# ============================================================================
# TESTING RECIPES
# ============================================================================

[group('test')]
[doc("Run all unit tests (default features)")]
test:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests ══════{{reset}}\n\n'
    {{cargo}} test --workspace -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run all unit tests with ALL features")]
test-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (all features) ══════{{reset}}\n\n'
    {{cargo}} test --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run tests with locked dependencies (default features)")]
test-locked:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (locked) ══════{{reset}}\n\n'
    {{cargo}} test --workspace --locked -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed (locked)\n'

[group('test')]
[doc("Run tests with locked dependencies and ALL features")]
test-locked-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (locked, all features) ══════{{reset}}\n\n'
    {{cargo}} test --workspace --all-features --locked -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed (locked)\n'

[group('test')]
[doc("Run tests with output visible")]
test-verbose:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (verbose) ══════{{reset}}\n\n'
    {{cargo}} test --workspace -j {{jobs}} -- --nocapture
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Test specific crate")]
test-crate crate:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Testing crate: {{crate}}\n'
    {{cargo}} test -p {{crate}} -- --nocapture
    printf '{{green}}[OK]{{reset}}   Crate tests passed\n'

[group('test')]
[doc("Run documentation tests only")]
test-doc:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running doc tests...\n'
    {{cargo}} test --workspace --doc
    printf '{{green}}[OK]{{reset}}   Doc tests passed\n'

[group('test')]
[doc("Run SQL Server integration tests")]
test-integration:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Integration Tests ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Using SQL Server at {{mssql_host}}:{{mssql_port}}\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test integration -- --ignored --test-threads=1
    printf '{{green}}[OK]{{reset}}   Integration tests passed\n'

[group('test')]
[doc("Run protocol conformance tests")]
test-conformance:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Protocol Conformance Tests ══════{{reset}}\n\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test protocol_conformance -- --ignored
    printf '{{green}}[OK]{{reset}}   Protocol conformance tests passed\n'

[group('test')]
[doc("Run resilience tests")]
test-resilience:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Resilience Tests ══════{{reset}}\n\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test resilience -- --ignored
    printf '{{green}}[OK]{{reset}}   Resilience tests passed\n'

[group('test')]
[doc("Run stress tests (long-running)")]
test-stress:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Stress Tests ══════{{reset}}\n\n'
    printf '{{yellow}}[WARN]{{reset}} Stress tests may take several minutes\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test stress -- --ignored --test-threads=1
    printf '{{green}}[OK]{{reset}}   Stress tests passed\n'

[group('test')]
[doc("Run the full ignored suite against SQL Server 2017/2019/2022; fails loudly if a version is unreachable")]
test-all-versions:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Testing SQL Server Version Compatibility ══════{{reset}}\n\n'

    # Mirrors CI's integration matrix: the full ignored suite against each
    # version. Fails loudly when a version is unreachable — no silent skips.
    # Start the containers first with: just sql-server-all
    failed=0
    for entry in "2022:1433" "2019:1434" "2017:1435"; do
        version="${entry%%:*}"
        port="${entry##*:}"
        # SQL Server 2017 predates UTF-8 collations (2019+); exclude that test
        # on the 2017 leg only (it runs with full assertions on 2019/2022).
        extra=""
        if [ "$version" = "2017" ]; then
            extra=" and not test(test_utf8_varchar_decoding)"
        fi
        printf '{{cyan}}[INFO]{{reset}} Testing SQL Server %s (port %s)...\n' "$version" "$port"
        if MSSQL_HOST={{mssql_host}} MSSQL_PORT="$port" MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
            {{cargo}} nextest run --all-features --run-ignored ignored-only --no-fail-fast \
            -E "not (binary(azure_sql) or test(azure_identity_auth) or test(cert_auth) or binary(kerberos_live))${extra}"; then
            printf '{{green}}[OK]{{reset}}   SQL Server %s passed\n' "$version"
        else
            printf '{{red}}[FAIL]{{reset}} SQL Server %s (port %s) unreachable or tests failed\n' "$version" "$port"
            failed=1
        fi
    done

    if [ "$failed" -ne 0 ]; then
        printf '\n{{red}}[FAIL]{{reset}} one or more SQL Server versions failed; start them with: just sql-server-all\n'
        exit 1
    fi
    printf '\n{{green}}[OK]{{reset}}   All SQL Server versions passed\n'

[group('test')]
[doc("Run pool integration tests")]
test-pool:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Pool Integration Tests ══════{{reset}}\n\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-pool --test integration -- --ignored --test-threads=1
    printf '{{green}}[OK]{{reset}}   Pool integration tests passed\n'

[group('test')]
[doc("Run all SQL Server tests (integration + conformance + resilience)")]
test-sql-server: test-integration test-conformance test-resilience test-pool
    @printf '{{green}}[OK]{{reset}}   All SQL Server tests passed\n'

[group('test')]
[doc("Run tests with various feature combinations")]
test-features:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Testing Feature Matrix ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Testing with no features...\n'
    {{cargo}} test --workspace --no-default-features -j {{jobs}}
    printf '{{cyan}}[INFO]{{reset}} Testing with default features...\n'
    {{cargo}} test --workspace -j {{jobs}}
    printf '{{cyan}}[INFO]{{reset}} Testing with all features...\n'
    {{cargo}} test --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Feature matrix tests passed\n'

[group('test')]
[doc("Test individual feature flags in isolation (mirrors CI feature-flags job)")]
check-feature-flags:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Feature Flag Isolation Check ══════{{reset}}\n\n'
    # Delegate to the exact xtask CI's "Feature Flag Validation" job runs
    # (cargo-hack --each-feature), under -D warnings. Calling the xtask rather
    # than a hand-listed cargo-check loop keeps this from drifting out of sync
    # with CI, and -D warnings is what catches the unused-import / dead-code
    # class of failures that only surfaces in --no-default-features or
    # single-feature builds. Requires cargo-hack (`just setup-tools`).
    export RUSTFLAGS="-D warnings"
    {{cargo}} xtask check-features

[group('test')]
[doc("Test zeroize feature (security-critical memory wiping)")]
test-zeroize:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Testing zeroize feature...\n'
    {{cargo}} test -p mssql-client --features zeroize -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Zeroize feature tests passed\n'

[group('test')]
[doc("Test integrated-auth feature (Linux only, requires libkrb5-dev)")]
test-integrated-auth:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "{{platform}}" != "linux" ]]; then
        printf '{{yellow}}[SKIP]{{reset}} integrated-auth is Linux-only\n'
        exit 0
    fi
    printf '{{cyan}}[INFO]{{reset}} Testing integrated-auth feature...\n'
    {{cargo}} test -p mssql-client --features integrated-auth -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Integrated-auth feature tests passed\n'

[group('test')]
[doc("Verify tds-protocol no_std compatibility")]
test-no-std:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking tds-protocol no_std compatibility...\n'
    # Check if thumbv7em target is installed
    if ! rustup target list --installed | grep -q thumbv7em-none-eabihf; then
        printf '{{cyan}}[INFO]{{reset}} Installing thumbv7em-none-eabihf target...\n'
        rustup target add thumbv7em-none-eabihf
    fi
    {{cargo}} check -p tds-protocol --no-default-features --target thumbv7em-none-eabihf
    printf '{{green}}[OK]{{reset}}   no_std compatibility verified\n'

[group('test')]
[doc("Run tests with cargo-nextest (default features)")]
nextest:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (nextest) ══════{{reset}}\n\n'
    {{cargo}} nextest run --workspace -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run tests with cargo-nextest and ALL features")]
nextest-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (nextest, all features) ══════{{reset}}\n\n'
    {{cargo}} nextest run --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run tests with cargo-nextest and locked dependencies (matches CI)")]
nextest-locked:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (nextest, locked) ══════{{reset}}\n\n'
    {{cargo}} nextest run --workspace --locked -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run tests with cargo-nextest, ALL features, and locked dependencies (matches CI)")]
nextest-locked-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Tests (nextest, locked, all features) ══════{{reset}}\n\n'
    {{cargo}} nextest run --workspace --all-features --locked -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run doctests, ALL features (nextest does NOT run them; matches CI's `cargo test --doc` step)")]
doctest-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Doctests (all features) ══════{{reset}}\n\n'
    {{cargo}} test --doc --workspace --all-features --locked
    printf '{{green}}[OK]{{reset}}   All doctests passed\n'

[group('test')]
[doc("Run Miri tests for unsafe code detection (requires nightly)")]
miri:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Miri Tests ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Setting up Miri...\n'
    cargo +nightly miri setup
    printf '{{cyan}}[INFO]{{reset}} Running Miri on tds-protocol...\n'
    cargo +nightly miri test -p tds-protocol
    printf '{{green}}[OK]{{reset}}   Miri tests passed\n'

# ============================================================================
# CODE QUALITY RECIPES
# ============================================================================

[group('lint')]
[doc("Format all code")]
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Formatting code...\n'
    {{cargo}} fmt --all
    printf '{{green}}[OK]{{reset}}   Formatting complete\n'

[group('lint')]
[doc("Check code formatting")]
fmt-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking format...\n'
    {{cargo}} fmt --all -- --check
    printf '{{green}}[OK]{{reset}}   Format check passed\n'

[group('lint')]
[doc("Run clippy lints (default features)")]
clippy:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running clippy...\n'
    {{cargo}} clippy --workspace --all-targets -- -D warnings
    printf '{{green}}[OK]{{reset}}   Clippy passed\n'

[group('lint')]
[doc("Run clippy lints with ALL features (matches CI)")]
clippy-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running clippy (all features)...\n'
    {{cargo}} clippy --workspace --all-features --all-targets -- -D warnings
    printf '{{green}}[OK]{{reset}}   Clippy passed\n'

[group('lint')]
[doc("Run clippy with strict pedantic lints")]
clippy-strict:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running clippy (strict)...\n'
    {{cargo}} clippy --workspace --all-targets -- \
        -D warnings \
        -D clippy::all \
        -D clippy::pedantic \
        -D clippy::nursery \
        -A clippy::module_name_repetitions \
        -A clippy::too_many_lines
    printf '{{green}}[OK]{{reset}}   Clippy (strict) passed\n'

[group('lint')]
[doc("Run clippy with strict pedantic lints and ALL features")]
clippy-strict-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running clippy (strict, all features)...\n'
    {{cargo}} clippy --workspace --all-features --all-targets -- \
        -D warnings \
        -D clippy::all \
        -D clippy::pedantic \
        -D clippy::nursery \
        -A clippy::module_name_repetitions \
        -A clippy::too_many_lines
    printf '{{green}}[OK]{{reset}}   Clippy (strict) passed\n'

[group('lint')]
[doc("Auto-fix clippy warnings")]
clippy-fix:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Auto-fixing clippy warnings...\n'
    {{cargo}} clippy --workspace --all-targets --fix --allow-dirty --allow-staged
    printf '{{green}}[OK]{{reset}}   Clippy fixes applied\n'

[group('security')]
[doc("Security vulnerability audit via cargo-audit")]
audit:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running security audit...\n'
    {{cargo}} audit
    printf '{{green}}[OK]{{reset}}   Security audit passed\n'

[group('security')]
[doc("Run cargo-deny checks (licenses, bans, advisories)")]
deny:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running cargo-deny...\n'
    {{cargo}} deny check
    printf '{{green}}[OK]{{reset}}   Deny checks passed\n'

[group('lint')]
[doc("Find unused dependencies via cargo-machete")]
machete:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Finding unused dependencies...\n'
    {{cargo}} machete
    printf '{{green}}[OK]{{reset}}   Machete check complete\n'

[group('lint')]
[doc("Verify MSRV compliance (default features)")]
msrv-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking MSRV {{msrv}}...\n'
    {{cargo}} +{{msrv}} check --workspace
    printf '{{green}}[OK]{{reset}}   MSRV {{msrv}} check passed\n'

[group('lint')]
[doc("Verify MSRV compliance with ALL features")]
msrv-check-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking MSRV {{msrv}} (all features)...\n'
    {{cargo}} +{{msrv}} check --workspace --all-features
    printf '{{green}}[OK]{{reset}}   MSRV {{msrv}} check passed\n'

[group('lint')]
[doc("Check for semver violations (advisory for pre-1.0)")]
semver:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking semver compliance...\n'
    # Exclude mssql-testing: testing utilities have relaxed API stability requirements
    # and are not published to crates.io (publish = false).
    # NOTE: cargo-semver-checks requires a newer Rust than our MSRV, so we explicitly
    # use +stable (MSRV is 1.88, set in rust-toolchain.toml).
    #
    # PRE-1.0 POLICY: Semver violations are ADVISORY, not blocking.
    # Per semver spec, breaking changes are allowed in 0.x.y minor bumps.
    # We run the check to surface breaking changes for documentation, but
    # don't fail the build. Post-1.0, this will become a hard failure.
    if {{cargo}} +stable semver-checks check-release --exclude mssql-testing; then
        printf '{{green}}[OK]{{reset}}   Semver check passed\n'
    else
        printf '{{yellow}}[WARN]{{reset}} Semver violations detected (advisory for pre-1.0)\n'
        printf '{{cyan}}[INFO]{{reset}} Breaking changes are allowed in 0.x.y minor versions.\n'
        printf '{{cyan}}[INFO]{{reset}} Ensure breaking changes are documented in CHANGELOG.md\n'
    fi

[group('lint')]
[doc("Run all lints (fmt + clippy, default features)")]
lint: fmt-check clippy
    @printf '{{green}}[OK]{{reset}}   All lints passed\n'

[group('lint')]
[doc("Run all lints with ALL features")]
lint-all: fmt-check clippy-all
    @printf '{{green}}[OK]{{reset}}   All lints passed\n'

[group('lint')]
[doc("Run comprehensive lint suite")]
lint-full: fmt-check clippy-strict audit deny machete
    @printf '{{green}}[OK]{{reset}}   Full lint suite passed\n'

# ============================================================================
# DOCUMENTATION RECIPES
# ============================================================================

[group('docs')]
[doc("Generate documentation (default features)")]
doc:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating documentation...\n'
    {{cargo}} doc --workspace --no-deps
    printf '{{green}}[OK]{{reset}}   Documentation generated\n'

[group('docs')]
[doc("Generate documentation with ALL features")]
doc-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating documentation (all features)...\n'
    {{cargo}} doc --workspace --all-features --no-deps
    printf '{{green}}[OK]{{reset}}   Documentation generated\n'

[group('docs')]
[doc("Generate and open documentation")]
doc-open:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating documentation...\n'
    {{cargo}} doc --workspace --no-deps --open
    printf '{{green}}[OK]{{reset}}   Documentation opened\n'

[group('docs')]
[doc("Generate docs including private items")]
doc-private:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating documentation (with private items)...\n'
    {{cargo}} doc --workspace --no-deps --document-private-items --open
    printf '{{green}}[OK]{{reset}}   Documentation opened\n'

[group('docs')]
[doc("Check documentation for warnings (default features)")]
doc-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking documentation...\n'
    RUSTDOCFLAGS="-D warnings" {{cargo}} doc --workspace --no-deps
    printf '{{green}}[OK]{{reset}}   Documentation check passed\n'

[group('docs')]
[doc("Check documentation for warnings with ALL features")]
doc-check-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking documentation (all features)...\n'
    RUSTDOCFLAGS="-D warnings" {{cargo}} doc --workspace --all-features --no-deps
    printf '{{green}}[OK]{{reset}}   Documentation check passed\n'

[group('docs')]
[doc("Check markdown links (requires lychee)")]
link-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking markdown links...\n'
    if ! command -v lychee &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} lychee not installed (cargo install lychee)\n'
        printf '{{yellow}}[WARN]{{reset}} Skipping link check\n'
        exit 0
    fi
    lychee --verbose --no-progress --accept 200,204,206 \
        --exclude '^https://crates.io' \
        --exclude '^https://docs.rs' \
        --exclude '^https://www.reddit.com' \
        './README.md' './CONTRIBUTING.md' './ARCHITECTURE.md' './MIGRATION.md'
    printf '{{green}}[OK]{{reset}}   Link check passed\n'

# ============================================================================
# COVERAGE RECIPES
# ============================================================================

[group('coverage')]
[doc("Generate HTML coverage report (default features)")]
coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Generating Coverage Report ══════{{reset}}\n\n'
    {{cargo}} llvm-cov --workspace --html --open
    printf '{{green}}[OK]{{reset}}   Coverage report opened\n'

[group('coverage')]
[doc("Generate HTML coverage report with ALL features")]
coverage-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Generating Coverage Report (all features) ══════{{reset}}\n\n'
    {{cargo}} llvm-cov --workspace --all-features --html --open
    printf '{{green}}[OK]{{reset}}   Coverage report opened\n'

[group('coverage')]
[doc("Generate LCOV coverage for CI (default features)")]
coverage-lcov output="lcov.info":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating LCOV coverage...\n'
    {{cargo}} llvm-cov --workspace --lcov --output-path {{output}}
    printf '{{green}}[OK]{{reset}}   Coverage saved to {{output}}\n'

[group('coverage')]
[doc("Generate LCOV coverage for CI with ALL features")]
coverage-lcov-all output="lcov.info":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating LCOV coverage (all features)...\n'
    {{cargo}} llvm-cov --workspace --all-features --lcov --output-path {{output}}
    printf '{{green}}[OK]{{reset}}   Coverage saved to {{output}}\n'

[group('coverage')]
[doc("Show coverage summary in terminal")]
coverage-summary:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Coverage summary:\n'
    {{cargo}} llvm-cov --workspace --text

[group('coverage')]
[doc("Generate Codecov-compatible coverage")]
coverage-codecov output="codecov.json":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating Codecov coverage...\n'
    {{cargo}} llvm-cov --workspace --all-features --codecov --output-path {{output}}
    printf '{{green}}[OK]{{reset}}   Coverage saved to {{output}}\n'

# ============================================================================
# FUZZING RECIPES
# ============================================================================

[group('fuzz')]
[doc("Run default fuzz target")]
fuzz target=fuzz_target time=fuzz_time:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Fuzzing: {{target}} ══════{{reset}}\n\n'
    cd {{fuzz_dir}} && {{cargo}} +nightly fuzz run {{target}} -- -max_total_time={{time}}
    printf '{{green}}[OK]{{reset}}   Fuzzing complete\n'

[group('fuzz')]
[doc("List available fuzz targets")]
fuzz-list:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Available fuzz targets:\n'
    cd {{fuzz_dir}} && {{cargo}} +nightly fuzz list

[group('fuzz')]
[doc("Run all fuzz targets briefly (smoke test)")]
fuzz-all time="30":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Fuzzing All Targets ══════{{reset}}\n\n'
    for target in parse_packet parse_token connection_string parse_prelogin decode_value; do
        printf '{{cyan}}[INFO]{{reset}} Fuzzing %s...\n' "$target"
        cd {{fuzz_dir}} && {{cargo}} +nightly fuzz run "$target" -- -max_total_time={{time}} || true
    done
    printf '{{green}}[OK]{{reset}}   All fuzz targets complete\n'

# ============================================================================
# EXAMPLE RECIPES
# ============================================================================

[group('examples')]
[doc("Build all examples (default features)")]
examples:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building all examples...\n'
    {{cargo}} build --examples
    printf '{{green}}[OK]{{reset}}   Examples built\n'

[group('examples')]
[doc("Build all examples with ALL features (matches CI)")]
examples-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building all examples (all features)...\n'
    {{cargo}} build --examples --all-features
    printf '{{green}}[OK]{{reset}}   Examples built\n'

[group('examples')]
[doc("Run basic query example")]
example-basic:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running basic_query example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example basic_query

[group('examples')]
[doc("Run transactions example")]
example-transactions:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running transactions example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example transactions

[group('examples')]
[doc("Run error handling example")]
example-errors:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running error_handling example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example error_handling

[group('examples')]
[doc("Run connection pool example")]
example-pool:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running connection_pool example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example connection_pool

# ============================================================================
# BENCHMARK RECIPES
# ============================================================================

[group('bench')]
[doc("Run benchmarks (optionally filtered to a name pattern)")]
bench filter="":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Running Benchmarks ══════{{reset}}\n\n'
    {{cargo}} bench --workspace -- {{filter}}
    printf '{{green}}[OK]{{reset}}   Benchmarks complete\n'

[group('bench')]
[doc("Run benchmarks and save baseline")]
bench-save name="baseline":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running benchmarks (saving baseline: {{name}})...\n'
    {{cargo}} bench --workspace -- --save-baseline {{name}}
    printf '{{green}}[OK]{{reset}}   Baseline saved: {{name}}\n'

[group('bench')]
[doc("Run benchmarks and compare to baseline")]
bench-compare name="baseline":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Comparing to baseline: {{name}}...\n'
    {{cargo}} bench --workspace -- --baseline {{name}}
    printf '{{green}}[OK]{{reset}}   Comparison complete\n'

# ============================================================================
# SQL SERVER DOCKER RECIPES
# ============================================================================

[group('docker')]
[doc("Start SQL Server 2022 container")]
sql-server-start:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Starting SQL Server 2022...\n'
    {{docker}} run -d --name sql_server \
        -e 'ACCEPT_EULA=Y' \
        -e 'SA_PASSWORD={{mssql_password}}' \
        -p 1433:1433 \
        mcr.microsoft.com/mssql/server:2022-latest
    printf '{{green}}[OK]{{reset}}   SQL Server 2022 started on port 1433\n'
    printf '{{cyan}}[INFO]{{reset}} Waiting for SQL Server to be ready...\n'
    sleep 15
    printf '{{green}}[OK]{{reset}}   SQL Server should be ready\n'

[group('docker')]
[doc("Start all SQL Server versions (2017, 2019, 2022)")]
sql-server-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Starting all SQL Server versions...\n'

    # SQL Server 2022
    {{docker}} run -d --name sql_server \
        -e 'ACCEPT_EULA=Y' \
        -e 'SA_PASSWORD={{mssql_password}}' \
        -p 1433:1433 \
        mcr.microsoft.com/mssql/server:2022-latest 2>/dev/null || true

    # SQL Server 2019
    {{docker}} run -d --name sql_server_2019 \
        -e 'ACCEPT_EULA=Y' \
        -e 'SA_PASSWORD={{mssql_password}}' \
        -p 1434:1433 \
        mcr.microsoft.com/mssql/server:2019-latest 2>/dev/null || true

    # SQL Server 2017
    {{docker}} run -d --name sql_server_2017 \
        -e 'ACCEPT_EULA=Y' \
        -e 'SA_PASSWORD={{mssql_password}}' \
        -p 1435:1433 \
        mcr.microsoft.com/mssql/server:2017-latest 2>/dev/null || true

    printf '{{green}}[OK]{{reset}}   SQL Server containers started\n'
    printf '  2022: localhost:1433\n'
    printf '  2019: localhost:1434\n'
    printf '  2017: localhost:1435\n'
    printf '{{cyan}}[INFO]{{reset}} Waiting for containers to be ready...\n'
    sleep 20
    printf '{{green}}[OK]{{reset}}   SQL Server instances should be ready\n'

[group('docker')]
[doc("Stop and remove SQL Server containers")]
sql-server-stop:
    #!/usr/bin/env bash
    set -uo pipefail  # Note: no -e, intentional - containers may not exist
    printf '{{cyan}}[INFO]{{reset}} Stopping SQL Server containers...\n'
    {{docker}} stop sql_server sql_server_2019 sql_server_2017 2>/dev/null || true
    {{docker}} rm sql_server sql_server_2019 sql_server_2017 2>/dev/null || true
    printf '{{green}}[OK]{{reset}}   SQL Server containers stopped\n'

[group('docker')]
[doc("Show SQL Server container status")]
sql-server-status:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} SQL Server container status:\n'
    {{docker}} ps --filter "name=sql_server" --format "table {{{{.Names}}}}\t{{{{.Status}}}}\t{{{{.Ports}}}}"

# ============================================================================
# DEVELOPMENT WORKFLOW RECIPES
# ============================================================================

[group('dev')]
[doc("Full development setup (default features)")]
dev: build test lint
    @printf '{{green}}[OK]{{reset}}   Development environment ready\n'

[group('dev')]
[doc("Full development setup with ALL features")]
dev-all: build-all test-all lint-all
    @printf '{{green}}[OK]{{reset}}   Development environment ready\n'

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run tests on file changes")]
watch:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (tests)...\n'
    {{cargo}} watch -x "test --workspace"

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run check on file changes")]
watch-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (check)...\n'
    {{cargo}} watch -x "check --workspace"

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run clippy on file changes")]
watch-clippy:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (clippy)...\n'
    {{cargo}} watch -x "clippy --workspace --all-targets"

# ============================================================================
# CI/CD RECIPES
# ============================================================================

[group('ci')]
[doc("Standard CI pipeline (default features, fast local checks)")]
ci: fmt-check clippy nextest-locked doc-check examples
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ CI Pipeline Complete ══════{{reset}}\n\n'
    printf '{{green}}[OK]{{reset}}   All CI checks passed\n'

[group('ci')]
[doc("CI pipeline with ALL features (matches GitHub Actions on Linux)")]
ci-all: fmt-check clippy-all nextest-locked-all doctest-all doc-check-all examples-all
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ CI Pipeline Complete (all features) ══════{{reset}}\n\n'
    printf '{{green}}[OK]{{reset}}   All CI checks passed\n'

[group('ci')]
[doc("Quick verification: fmt + clippy + check (no tests, fastest feedback)")]
quick: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Quick checks passed\n'

[group('ci')]
[doc("Fast CI checks (no tests)")]
ci-fast: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Fast CI checks passed\n'

[group('ci')]
[doc("Full CI with coverage and security audit (matches GitHub Actions)")]
ci-full: ci coverage-lcov audit deny
    @printf '{{green}}[OK]{{reset}}   Full CI pipeline passed\n'

[group('ci')]
[doc("Full CI with ALL features and security audit (matches GitHub Actions)")]
ci-full-all: ci-all coverage-lcov-all audit deny
    @printf '{{green}}[OK]{{reset}}   Full CI pipeline passed\n'

[group('ci')]
[doc("Pre-commit hook checks")]
pre-commit: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Pre-commit checks passed\n'

[group('ci')]
[doc("Pre-push hook checks")]
pre-push: ci
    @printf '{{green}}[OK]{{reset}}   Pre-push checks passed\n'

# ============================================================================
# DEPENDENCY MANAGEMENT
# ============================================================================

[group('deps')]
[doc("Check for outdated dependencies")]
outdated:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking for outdated dependencies...\n'
    {{cargo}} outdated -R

[group('deps')]
[doc("Update Cargo.lock to latest compatible versions")]
update:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Updating dependencies...\n'
    {{cargo}} update
    printf '{{green}}[OK]{{reset}}   Dependencies updated\n'

[group('deps')]
[doc("Show dependency tree")]
tree:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Dependency tree:\n'
    {{cargo}} tree --workspace

[group('deps')]
[doc("Show duplicate dependencies")]
tree-duplicates:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Duplicate dependencies:\n'
    {{cargo}} tree --workspace --duplicates

# ============================================================================
# RELEASE CHECKLIST RECIPES
# ============================================================================

[group('release')]
[doc("Run typos spell checker")]
typos:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running typos spell checker...\n'
    if ! command -v typos &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} typos not installed (cargo install typos-cli)\n'
        exit 0
    fi
    # Scan the whole tree (respecting typos.toml), matching the bare `typos`
    # invocation in the CI Hygiene job. The previous hand-picked path list
    # included a `docs/` directory that no longer exists, so the recipe
    # failed (exit 64) and also diverged from what CI actually checks.
    typos
    printf '{{green}}[OK]{{reset}}   Typos check passed\n'

[group('lint')]
[doc("Check documentation consistency (MSRV across files, policy agreement, version inheritance)")]
doc-consistency:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -x ./scripts/check-doc-consistency.sh ]; then
        printf '{{yellow}}[WARN]{{reset}} scripts/check-doc-consistency.sh is missing or not executable\n'
        exit 0
    fi
    ./scripts/check-doc-consistency.sh

# Verify the committed public-API snapshots match the current surface
# (the same gate CI runs). Needs the pinned nightly + cargo-public-api.
public-api:
    ./scripts/check-public-api.sh

# Regenerate the committed public-API snapshots after an intended API change.
public-api-update:
    ./scripts/check-public-api.sh update

[group('release')]
[confirm("⚠️ This will YANK all crates at the current workspace version. This is for SECURITY INCIDENTS only. Continue?")]
[doc("Yank all crates at current version (SECURITY INCIDENTS ONLY)")]
yank-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{red}}════════════════════════════════════════════════════════════════{{reset}}\n'
    printf '{{bold}}{{red}}  ⚠️  YANKING ALL CRATES AT VERSION {{version}}                    {{reset}}\n'
    printf '{{bold}}{{red}}════════════════════════════════════════════════════════════════{{reset}}\n\n'
    printf '{{yellow}}This action is for security incidents only.{{reset}}\n'
    printf '{{yellow}}Yanked versions cannot be un-yanked without contacting crates.io support.{{reset}}\n\n'

    for crate in tds-protocol mssql-types mssql-tls mssql-codec mssql-auth mssql-derive mssql-client mssql-driver-pool mssql-testing; do
        printf '{{cyan}}[INFO]{{reset}} Yanking %s@{{version}}...\n' "$crate"
        {{cargo}} yank --version {{version}} "$crate" || printf '{{yellow}}[WARN]{{reset}} Failed to yank %s (may not exist at this version)\n' "$crate"
    done

    printf '\n{{green}}[OK]{{reset}}   Yank complete for version {{version}}\n'
    printf '{{cyan}}[NEXT]{{reset}} Prepare and publish a patched version\n'

[group('release')]
[doc("Validate dependency graph for publishing")]
dep-graph:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Dependency graph for publishing:\n\n'
    printf '{{bold}}Tier 0 (Independent):{{reset}}\n'
    printf '  tds-protocol\n'
    printf '  mssql-types\n\n'
    printf '{{bold}}Tier 1 (Depend on Tier 0):{{reset}}\n'
    printf '  mssql-tls      → tds-protocol\n'
    printf '  mssql-codec    → tds-protocol\n'
    printf '  mssql-auth     → tds-protocol\n\n'
    printf '{{bold}}Tier 2 (Proc-macro):{{reset}}\n'
    printf '  mssql-derive   (dev-dep on mssql-client)\n\n'
    printf '{{bold}}Tier 3 (Main client):{{reset}}\n'
    printf '  mssql-client   → tds-protocol, mssql-tls, mssql-codec, mssql-types, mssql-auth\n\n'
    printf '{{bold}}Tier 4 (Depend on client):{{reset}}\n'
    printf '  mssql-driver-pool → mssql-client\n'
    printf '  mssql-testing     → mssql-client\n\n'
    printf '{{yellow}}[NOTE]{{reset}} Circular dev-deps: mssql-derive ↔ mssql-client\n'
    printf '{{yellow}}[NOTE]{{reset}} See RELEASING.md for handling first-time publishes\n'

# ============================================================================
# UTILITIES
# ============================================================================

[group('util')]
[doc("Open crate on crates.io")]
crates-io crate="mssql-client":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Opening {{crate}} on crates.io...\n'
    {{open_cmd}} "https://crates.io/crates/{{crate}}"

[group('util')]
[doc("Open crate documentation on docs.rs")]
docs-rs crate="mssql-client":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Opening {{crate}} on docs.rs...\n'
    {{open_cmd}} "https://docs.rs/{{crate}}"

[group('util')]
[doc("Count lines of code")]
loc:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Lines of code:\n'
    if command -v tokei &> /dev/null; then
        tokei . --exclude target --exclude node_modules
    else
        find crates -name '*.rs' | xargs wc -l | tail -1
    fi

[group('util')]
[doc("Analyze binary size bloat")]
bloat crate="mssql-client":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Binary size analysis for {{crate}}...\n'
    {{cargo}} bloat --release -p {{crate}} --crates

[group('security')]
[doc("Generate Software Bill of Materials (SBOM) in CycloneDX format")]
sbom output="sbom.cyclonedx.json":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating SBOM...\n'
    if ! command -v cargo-sbom &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} cargo-sbom not installed\n'
        printf '{{cyan}}[INFO]{{reset}} Install with: cargo install cargo-sbom\n'
        exit 1
    fi
    {{cargo}} sbom --output-format cyclonedx-json > {{output}}
    printf '{{green}}[OK]{{reset}}   SBOM generated: {{output}}\n'

[group('security')]
[doc("Check for unsafe code usage")]
geiger:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Scanning for unsafe code...\n'
    for crate in crates/*/; do
        name=$(basename "$crate")
        printf '{{cyan}}[INFO]{{reset}} Scanning %s...\n' "$name"
        {{cargo}} geiger -p "$name" --all-targets 2>/dev/null || true
    done
    printf '{{green}}[OK]{{reset}}   Unsafe code scan complete\n'

[group('util')]
[doc("Show expanded macros")]
expand crate:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Expanding macros in {{crate}}...\n'
    {{cargo}} expand -p {{crate}}

[group('util')]
[doc("Generate and display project statistics")]
stats: loc
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Project Statistics ══════{{reset}}\n\n'
    printf '{{cyan}}Crates:{{reset}}\n'
    find crates -maxdepth 1 -type d | tail -n +2 | while read dir; do
        name=$(basename "$dir")
        printf '  - %s\n' "$name"
    done
    printf '\n{{cyan}}Dependencies:{{reset}}\n'
    printf '  Direct: %s\n' "$({{cargo}} tree --workspace --depth 1 | grep -c '├\|└')"
    printf '  Total:  %s\n' "$({{cargo}} tree --workspace | wc -l)"
    printf '\n'

# ============================================================================
# HELP & DOCUMENTATION
# ============================================================================

[group('help')]
[doc("Show version and environment info")]
info:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{project_name}} v{{version}}{{reset}}\n'
    printf '═══════════════════════════════════════\n'
    printf '{{cyan}}MSRV:{{reset}}      {{msrv}}\n'
    printf '{{cyan}}Edition:{{reset}}   {{edition}}\n'
    printf '{{cyan}}Platform:{{reset}}  {{platform}}\n'
    printf '{{cyan}}Jobs:{{reset}}      {{jobs}}\n'
    printf '\n{{cyan}}Rust:{{reset}}      %s\n' "$(rustc --version)"
    printf '{{cyan}}Cargo:{{reset}}     %s\n' "$(cargo --version)"
    printf '{{cyan}}Just:{{reset}}      %s\n' "$(just --version)"
    printf '\n'

[group('help')]
[doc("Check which development tools are installed")]
check-tools: setup

[group('help')]
[doc("Show all available recipes grouped by category")]
help:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{project_name}} v{{version}}{{reset}} — SQL Server Driver Development\n'
    printf 'MSRV: {{msrv}} | Edition: {{edition}} | Platform: {{platform}}\n\n'
    printf '{{bold}}Usage:{{reset}} just [recipe] [arguments...]\n\n'
    printf '{{bold}}Recipe Naming Convention:{{reset}}\n'
    printf '  Base recipes use DEFAULT features (work everywhere)\n'
    printf '  -all variants use ALL features (need libkrb5-dev on Linux)\n\n'
    printf '{{bold}}Quick Start:{{reset}}\n'
    printf '  just bootstrap   Full setup (system pkgs + tools + hooks)\n'
    printf '  just setup       Check development environment\n'
    printf '  just quick       Fast feedback (fmt + clippy + check)\n'
    printf '  just ci          Run CI pipeline (default features)\n'
    printf '  just ci-all      Run CI pipeline (all features, matches GH Actions)\n\n'
    just --list --unsorted
