# ============================================================================
# rust-mssql-driver Development Justfile
# ============================================================================
#
# Modern command runner for the rust-mssql-driver SQL Server client.
# Replaces traditional Makefile with improved UX, safety, and features.
#
# Usage:
#   just              - Show all available commands
#   just build        - Build debug
#   just ci           - Run full CI pipeline
#   just <recipe>     - Run any recipe
#
# Requirements:
#   - Just >= 1.23.0 (for [group], [confirm], [doc] attributes)
#   - Rust toolchain (rustup recommended)
#
# Install Just:
#   cargo install just
#   # or: brew install just / apt install just / pacman -S just
#
# ============================================================================

# ----------------------------------------------------------------------------
# Project Configuration
# ----------------------------------------------------------------------------

project_name := "rust-mssql-driver"
# Version is read dynamically from Cargo.toml to avoid drift
version := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "mssql-client") | .version'`
msrv := "1.85"
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

# Use bash for shell commands
set shell := ["bash", "-cu"]

# Export all variables to child processes
set export

# ============================================================================
# CORE BUILD RECIPES
# ============================================================================

[group('build')]
[doc("Build workspace in debug mode")]
build:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Building (debug) ══════{{reset}}\n\n'
    {{cargo}} build --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Build complete\n'

[group('build')]
[doc("Build workspace in release mode with optimizations")]
release:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Building (release) ══════{{reset}}\n\n'
    {{cargo}} build --workspace --all-features --release -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Release build complete\n'

[group('build')]
[doc("Fast type check without code generation")]
check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Type checking...\n'
    {{cargo}} check --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Type check passed\n'

[group('build')]
[doc("Analyze build times")]
build-timing:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Building with timing analysis...\n'
    {{cargo}} build --workspace --all-features --timings
    printf '{{green}}[OK]{{reset}}   Build timing report generated (see target/cargo-timings/)\n'

[group('build')]
[confirm("This will delete all build artifacts. Continue?")]
[doc("Clean all build artifacts")]
clean:
    #!/usr/bin/env bash
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
[doc("Run all unit tests")]
test:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Tests ══════{{reset}}\n\n'
    {{cargo}} test --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Run tests with locked dependencies (reproducible)")]
test-locked:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Tests (locked) ══════{{reset}}\n\n'
    {{cargo}} test --workspace --all-features --locked -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed (locked)\n'

[group('test')]
[doc("Run tests with output visible")]
test-verbose:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Tests (verbose) ══════{{reset}}\n\n'
    {{cargo}} test --workspace --all-features -j {{jobs}} -- --nocapture
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

[group('test')]
[doc("Test specific crate")]
test-crate crate:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Testing crate: {{crate}}\n'
    {{cargo}} test -p {{crate}} --all-features -- --nocapture
    printf '{{green}}[OK]{{reset}}   Crate tests passed\n'

[group('test')]
[doc("Run documentation tests only")]
test-doc:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running doc tests...\n'
    {{cargo}} test --workspace --all-features --doc
    printf '{{green}}[OK]{{reset}}   Doc tests passed\n'

[group('test')]
[doc("Run SQL Server integration tests")]
test-integration:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Integration Tests ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Using SQL Server at {{mssql_host}}:{{mssql_port}}\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test integration -- --ignored --test-threads=1
    printf '{{green}}[OK]{{reset}}   Integration tests passed\n'

[group('test')]
[doc("Run protocol conformance tests")]
test-conformance:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Protocol Conformance Tests ══════{{reset}}\n\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test protocol_conformance -- --ignored
    printf '{{green}}[OK]{{reset}}   Protocol conformance tests passed\n'

[group('test')]
[doc("Run resilience tests")]
test-resilience:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Resilience Tests ══════{{reset}}\n\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test resilience -- --ignored
    printf '{{green}}[OK]{{reset}}   Resilience tests passed\n'

[group('test')]
[doc("Run stress tests (long-running)")]
test-stress:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Stress Tests ══════{{reset}}\n\n'
    printf '{{yellow}}[WARN]{{reset}} Stress tests may take several minutes\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test stress -- --ignored --test-threads=1
    printf '{{green}}[OK]{{reset}}   Stress tests passed\n'

[group('test')]
[doc("Run version compatibility tests against SQL Server 2017/2019/2022")]
test-all-versions:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Testing SQL Server Version Compatibility ══════{{reset}}\n\n'

    # SQL Server 2022 (default port 1433)
    printf '{{cyan}}[INFO]{{reset}} Testing SQL Server 2022 (port 1433)...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT=1433 MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test version_compatibility -- --ignored 2>/dev/null && \
        printf '{{green}}[OK]{{reset}}   SQL Server 2022 passed\n' || \
        printf '{{yellow}}[SKIP]{{reset}} SQL Server 2022 not available\n'

    # SQL Server 2019 (port 1434)
    printf '{{cyan}}[INFO]{{reset}} Testing SQL Server 2019 (port 1434)...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT=1434 MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test version_compatibility -- --ignored 2>/dev/null && \
        printf '{{green}}[OK]{{reset}}   SQL Server 2019 passed\n' || \
        printf '{{yellow}}[SKIP]{{reset}} SQL Server 2019 not available\n'

    # SQL Server 2017 (port 1435)
    printf '{{cyan}}[INFO]{{reset}} Testing SQL Server 2017 (port 1435)...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT=1435 MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        {{cargo}} test -p mssql-client --test version_compatibility -- --ignored 2>/dev/null && \
        printf '{{green}}[OK]{{reset}}   SQL Server 2017 passed\n' || \
        printf '{{yellow}}[SKIP]{{reset}} SQL Server 2017 not available\n'

    printf '{{green}}[OK]{{reset}}   Version compatibility testing complete\n'

[group('test')]
[doc("Run pool integration tests")]
test-pool:
    #!/usr/bin/env bash
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
    printf '\n{{bold}}{{blue}}══════ Testing Feature Matrix ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Testing with no features...\n'
    {{cargo}} test --workspace --no-default-features -j {{jobs}}
    printf '{{cyan}}[INFO]{{reset}} Testing with default features...\n'
    {{cargo}} test --workspace -j {{jobs}}
    printf '{{cyan}}[INFO]{{reset}} Testing with all features...\n'
    {{cargo}} test --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Feature matrix tests passed\n'

[group('test')]
[doc("Run tests with cargo-nextest (faster, parallel)")]
nextest:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Tests (nextest) ══════{{reset}}\n\n'
    {{cargo}} nextest run --workspace --all-features -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   All tests passed\n'

# ============================================================================
# CODE QUALITY RECIPES
# ============================================================================

[group('lint')]
[doc("Format all code")]
fmt:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Formatting code...\n'
    {{cargo}} fmt --all
    printf '{{green}}[OK]{{reset}}   Formatting complete\n'

[group('lint')]
[doc("Check code formatting")]
fmt-check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking format...\n'
    {{cargo}} fmt --all -- --check
    printf '{{green}}[OK]{{reset}}   Format check passed\n'

[group('lint')]
[doc("Run clippy lints (matches CI configuration)")]
clippy:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running clippy...\n'
    {{cargo}} clippy --workspace --all-features --all-targets -- -D warnings
    printf '{{green}}[OK]{{reset}}   Clippy passed\n'

[group('lint')]
[doc("Run clippy with strict deny on warnings")]
clippy-strict:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running clippy (strict)...\n'
    {{cargo}} clippy --workspace --all-targets --all-features -- \
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
    printf '{{cyan}}[INFO]{{reset}} Auto-fixing clippy warnings...\n'
    {{cargo}} clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged
    printf '{{green}}[OK]{{reset}}   Clippy fixes applied\n'

[group('security')]
[doc("Security vulnerability audit via cargo-audit")]
audit:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running security audit...\n'
    {{cargo}} audit
    printf '{{green}}[OK]{{reset}}   Security audit passed\n'

[group('security')]
[doc("Run cargo-deny checks (licenses, bans, advisories)")]
deny:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running cargo-deny...\n'
    {{cargo}} deny check
    printf '{{green}}[OK]{{reset}}   Deny checks passed\n'

[group('lint')]
[doc("Find unused dependencies via cargo-machete (fast, heuristic)")]
machete:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Finding unused dependencies (fast)...\n'
    {{cargo}} machete
    printf '{{green}}[OK]{{reset}}   Machete check complete\n'

[group('lint')]
[doc("Verify MSRV compliance")]
msrv-check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking MSRV {{msrv}}...\n'
    {{cargo}} +{{msrv}} check --workspace --all-features
    printf '{{green}}[OK]{{reset}}   MSRV {{msrv}} check passed\n'

[group('lint')]
[doc("Check for semver violations (for library crates)")]
semver:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking semver compliance...\n'
    {{cargo}} semver-checks check-release
    printf '{{green}}[OK]{{reset}}   Semver check passed\n'

[group('lint')]
[doc("Run all lints (fmt + clippy)")]
lint: fmt-check clippy
    @printf '{{green}}[OK]{{reset}}   All lints passed\n'

[group('lint')]
[doc("Run comprehensive lint suite")]
lint-full: fmt-check clippy-strict audit deny machete
    @printf '{{green}}[OK]{{reset}}   Full lint suite passed\n'

# ============================================================================
# DOCUMENTATION RECIPES
# ============================================================================

[group('docs')]
[doc("Generate documentation")]
doc:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Generating documentation...\n'
    {{cargo}} doc --workspace --all-features --no-deps
    printf '{{green}}[OK]{{reset}}   Documentation generated\n'

[group('docs')]
[doc("Generate and open documentation")]
doc-open:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Generating documentation...\n'
    {{cargo}} doc --workspace --all-features --no-deps --open
    printf '{{green}}[OK]{{reset}}   Documentation opened\n'

[group('docs')]
[doc("Generate docs including private items")]
doc-private:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Generating documentation (with private items)...\n'
    {{cargo}} doc --workspace --all-features --no-deps --document-private-items --open
    printf '{{green}}[OK]{{reset}}   Documentation opened\n'

[group('docs')]
[doc("Check documentation for warnings")]
doc-check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking documentation...\n'
    RUSTDOCFLAGS="-D warnings" {{cargo}} doc --workspace --all-features --no-deps
    printf '{{green}}[OK]{{reset}}   Documentation check passed\n'

[group('docs')]
[doc("Check markdown links (requires lychee)")]
link-check:
    #!/usr/bin/env bash
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
        './docs/**/*.md' './README.md' './CONTRIBUTING.md' './ARCHITECTURE.md'
    printf '{{green}}[OK]{{reset}}   Link check passed\n'

# ============================================================================
# COVERAGE RECIPES
# ============================================================================

[group('coverage')]
[doc("Generate HTML coverage report and open in browser")]
coverage:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Generating Coverage Report ══════{{reset}}\n\n'
    {{cargo}} llvm-cov --workspace --all-features --html --open
    printf '{{green}}[OK]{{reset}}   Coverage report opened\n'

[group('coverage')]
[doc("Generate LCOV coverage for CI integration")]
coverage-lcov output="lcov.info":
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Generating LCOV coverage...\n'
    {{cargo}} llvm-cov --workspace --all-features --lcov --output-path {{output}}
    printf '{{green}}[OK]{{reset}}   Coverage saved to {{output}}\n'

[group('coverage')]
[doc("Show coverage summary in terminal")]
coverage-summary:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Coverage summary:\n'
    {{cargo}} llvm-cov --workspace --all-features --text

[group('coverage')]
[doc("Generate Codecov-compatible coverage")]
coverage-codecov output="codecov.json":
    #!/usr/bin/env bash
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
    printf '\n{{bold}}{{blue}}══════ Fuzzing: {{target}} ══════{{reset}}\n\n'
    cd {{fuzz_dir}} && {{cargo}} +nightly fuzz run {{target}} -- -max_total_time={{time}}
    printf '{{green}}[OK]{{reset}}   Fuzzing complete\n'

[group('fuzz')]
[doc("List available fuzz targets")]
fuzz-list:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Available fuzz targets:\n'
    cd {{fuzz_dir}} && {{cargo}} +nightly fuzz list

[group('fuzz')]
[doc("Run all fuzz targets briefly (smoke test)")]
fuzz-all time="30":
    #!/usr/bin/env bash
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
[doc("Build all examples")]
examples:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Building all examples...\n'
    {{cargo}} build -p mssql-client --examples --all-features
    printf '{{green}}[OK]{{reset}}   Examples built\n'

[group('examples')]
[doc("Run basic query example")]
example-basic:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running basic_query example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example basic_query

[group('examples')]
[doc("Run transactions example")]
example-transactions:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running transactions example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example transactions

[group('examples')]
[doc("Run error handling example")]
example-errors:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running error_handling example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example error_handling

[group('examples')]
[doc("Run connection pool example")]
example-pool:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running connection_pool example...\n'
    MSSQL_HOST={{mssql_host}} MSSQL_PORT={{mssql_port}} MSSQL_USER={{mssql_user}} MSSQL_PASSWORD='{{mssql_password}}' \
        RUST_LOG={{rust_log}} {{cargo}} run -p mssql-client --example connection_pool

# ============================================================================
# BENCHMARK RECIPES
# ============================================================================

[group('bench')]
[doc("Run benchmarks")]
bench:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Running Benchmarks ══════{{reset}}\n\n'
    {{cargo}} bench --workspace
    printf '{{green}}[OK]{{reset}}   Benchmarks complete\n'

[group('bench')]
[doc("Run benchmarks and save baseline")]
bench-save name="baseline":
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Running benchmarks (saving baseline: {{name}})...\n'
    {{cargo}} bench --workspace -- --save-baseline {{name}}
    printf '{{green}}[OK]{{reset}}   Baseline saved: {{name}}\n'

[group('bench')]
[doc("Run benchmarks and compare to baseline")]
bench-compare name="baseline":
    #!/usr/bin/env bash
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
    printf '{{cyan}}[INFO]{{reset}} Stopping SQL Server containers...\n'
    {{docker}} stop sql_server sql_server_2019 sql_server_2017 2>/dev/null || true
    {{docker}} rm sql_server sql_server_2019 sql_server_2017 2>/dev/null || true
    printf '{{green}}[OK]{{reset}}   SQL Server containers stopped\n'

[group('docker')]
[doc("Show SQL Server container status")]
sql-server-status:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} SQL Server container status:\n'
    {{docker}} ps --filter "name=sql_server" --format "table {{{{.Names}}}}\t{{{{.Status}}}}\t{{{{.Ports}}}}"

# ============================================================================
# DEVELOPMENT WORKFLOW RECIPES
# ============================================================================

[group('dev')]
[doc("Full development setup")]
dev: build test lint
    @printf '{{green}}[OK]{{reset}}   Development environment ready\n'

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run tests on file changes")]
watch:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (tests)...\n'
    {{cargo}} watch -x "test --workspace --all-features"

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run check on file changes")]
watch-check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (check)...\n'
    {{cargo}} watch -x "check --workspace --all-features"

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run clippy on file changes")]
watch-clippy:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (clippy)...\n'
    {{cargo}} watch -x "clippy --workspace --all-targets --all-features"

# ============================================================================
# CI/CD RECIPES
# ============================================================================

[group('ci')]
[doc("Check documentation versions match Cargo.toml")]
version-sync:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking version sync...\n'
    VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "mssql-client") | .version')
    MAJOR_MINOR=$(echo "$VERSION" | cut -d. -f1,2)

    # Check README.md
    if ! grep -q "mssql-client = \"$MAJOR_MINOR\"" README.md 2>/dev/null; then
        printf '{{yellow}}[WARN]{{reset}} README.md may need version update (expected %s)\n' "$MAJOR_MINOR"
    fi

    printf '{{green}}[OK]{{reset}}   Version sync check complete (v%s)\n' "$VERSION"

[group('ci')]
[doc("Standard CI pipeline (matches GitHub Actions)")]
ci: fmt-check clippy test-locked doc-check
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ CI Pipeline Complete ══════{{reset}}\n\n'
    printf '{{green}}[OK]{{reset}}   All CI checks passed\n'

[group('ci')]
[doc("Fast CI checks (no tests)")]
ci-fast: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Fast CI checks passed\n'

[group('ci')]
[doc("Full CI with coverage and security audit")]
ci-full: ci coverage-lcov audit deny
    @printf '{{green}}[OK]{{reset}}   Full CI pipeline passed\n'

[group('ci')]
[doc("Complete CI with all checks (for releases)")]
ci-release: ci-full semver msrv-check test-features
    @printf '{{green}}[OK]{{reset}}   Release CI pipeline passed\n'

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
    printf '{{cyan}}[INFO]{{reset}} Checking for outdated dependencies...\n'
    {{cargo}} outdated -R

[group('deps')]
[doc("Update Cargo.lock to latest compatible versions")]
update:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Updating dependencies...\n'
    {{cargo}} update
    printf '{{green}}[OK]{{reset}}   Dependencies updated\n'

[group('deps')]
[doc("Show dependency tree")]
tree:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Dependency tree:\n'
    {{cargo}} tree --workspace

[group('deps')]
[doc("Show duplicate dependencies")]
tree-duplicates:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Duplicate dependencies:\n'
    {{cargo}} tree --workspace --duplicates

# ============================================================================
# RELEASE CHECKLIST RECIPES
# ============================================================================

[group('release')]
[doc("Check for WIP markers (TODO, FIXME, XXX, HACK, todo!, unimplemented!)")]
wip-check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking for WIP markers...\n'

    # Search for comment markers
    COMMENTS=$(grep -rn "TODO\|FIXME\|XXX\|HACK" --include="*.rs" crates/ 2>/dev/null || true)
    if [ -n "$COMMENTS" ]; then
        printf '{{yellow}}[WARN]{{reset}} Found WIP comments:\n'
        echo "$COMMENTS" | head -20
        COMMENT_COUNT=$(echo "$COMMENTS" | wc -l)
        if [ "$COMMENT_COUNT" -gt 20 ]; then
            printf '{{yellow}}[WARN]{{reset}} ... and %d more\n' "$((COMMENT_COUNT - 20))"
        fi
    fi

    # Search for incomplete macros (excluding tests)
    MACROS=$(grep -rn "todo!\|unimplemented!" --include="*.rs" crates/*/src/ 2>/dev/null || true)
    if [ -n "$MACROS" ]; then
        printf '{{red}}[ERR]{{reset}}  Found incomplete macros in production code:\n'
        echo "$MACROS"
        exit 1
    fi

    printf '{{green}}[OK]{{reset}}   WIP check passed (no blocking issues)\n'

[group('release')]
[doc("Audit panic paths (.unwrap(), .expect()) in production code")]
panic-audit:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Auditing panic paths in production code...\n'

    # Find .unwrap() in src/ directories (production code)
    UNWRAPS=$(grep -rn "\.unwrap()" crates/*/src/ --include="*.rs" 2>/dev/null || true)
    EXPECTS=$(grep -rn "\.expect(" crates/*/src/ --include="*.rs" 2>/dev/null || true)

    if [ -n "$UNWRAPS" ] || [ -n "$EXPECTS" ]; then
        printf '{{yellow}}[WARN]{{reset}} Found potential panic paths:\n'
        if [ -n "$UNWRAPS" ]; then
            echo "$UNWRAPS" | head -15
            UNWRAP_COUNT=$(echo "$UNWRAPS" | wc -l)
            printf '{{cyan}}[INFO]{{reset}} Total .unwrap() calls: %d\n' "$UNWRAP_COUNT"
        fi
        if [ -n "$EXPECTS" ]; then
            echo "$EXPECTS" | head -10
            EXPECT_COUNT=$(echo "$EXPECTS" | wc -l)
            printf '{{cyan}}[INFO]{{reset}} Total .expect() calls: %d\n' "$EXPECT_COUNT"
        fi
        printf '{{yellow}}[NOTE]{{reset}} Review each for production safety. High line numbers may be test modules.\n'
    else
        printf '{{green}}[OK]{{reset}}   No panic paths found in production code\n'
    fi

[group('release')]
[doc("Verify Cargo.toml metadata for crates.io publishing")]
metadata-check:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Checking Cargo.toml metadata...\n'

    METADATA=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "mssql-client")')

    # Required fields
    DESC=$(echo "$METADATA" | jq -r '.description // empty')
    LICENSE=$(echo "$METADATA" | jq -r '.license // empty')
    REPO=$(echo "$METADATA" | jq -r '.repository // empty')

    MISSING=""
    [ -z "$DESC" ] && MISSING="$MISSING description"
    [ -z "$LICENSE" ] && MISSING="$MISSING license"
    [ -z "$REPO" ] && MISSING="$MISSING repository"

    if [ -n "$MISSING" ]; then
        printf '{{red}}[ERR]{{reset}}  Missing required fields:%s\n' "$MISSING"
        exit 1
    fi

    # Recommended fields
    KEYWORDS=$(echo "$METADATA" | jq -r '.keywords // [] | length')
    CATEGORIES=$(echo "$METADATA" | jq -r '.categories // [] | length')

    [ "$KEYWORDS" -eq 0 ] && printf '{{yellow}}[WARN]{{reset}} No keywords defined (recommended for discoverability)\n'
    [ "$CATEGORIES" -eq 0 ] && printf '{{yellow}}[WARN]{{reset}} No categories defined (recommended for discoverability)\n'

    printf '{{cyan}}[INFO]{{reset}} Package metadata:\n'
    printf '  description: %s\n' "$DESC"
    printf '  license:     %s\n' "$LICENSE"
    printf '  repository:  %s\n' "$REPO"
    printf '  keywords:    %d defined\n' "$KEYWORDS"
    printf '  categories:  %d defined\n' "$CATEGORIES"

    printf '{{green}}[OK]{{reset}}   Metadata check passed\n'

[group('release')]
[doc("Prepare for release (full validation)")]
release-check: ci-release wip-check panic-audit metadata-check
    #!/usr/bin/env bash
    printf '\n{{bold}}{{blue}}══════ Release Validation ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Checking for uncommitted changes...\n'
    if ! git diff-index --quiet HEAD --; then
        printf '{{red}}[ERR]{{reset}}  Uncommitted changes detected\n'
        exit 1
    fi
    printf '{{cyan}}[INFO]{{reset}} Checking for unpushed commits...\n'
    if [ -n "$(git log @{u}.. 2>/dev/null)" ]; then
        printf '{{yellow}}[WARN]{{reset}} Unpushed commits detected\n'
    fi
    printf '{{green}}[OK]{{reset}}   Ready for release\n'

[group('release')]
[doc("Publish all crates to crates.io (dry run)")]
publish-dry:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Publishing (dry run) in dependency order...\n'
    # Publish in dependency order
    {{cargo}} publish --dry-run -p tds-protocol
    {{cargo}} publish --dry-run -p mssql-types
    {{cargo}} publish --dry-run -p mssql-tls
    {{cargo}} publish --dry-run -p mssql-codec
    {{cargo}} publish --dry-run -p mssql-auth
    {{cargo}} publish --dry-run -p mssql-derive
    {{cargo}} publish --dry-run -p mssql-driver-pool
    {{cargo}} publish --dry-run -p mssql-client
    {{cargo}} publish --dry-run -p mssql-testing
    printf '{{green}}[OK]{{reset}}   Dry run complete\n'

[group('release')]
[doc("Create git tag for release")]
tag:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Creating tag v{{version}}...\n'
    git tag -a "v{{version}}" -m "Release v{{version}}"
    printf '{{green}}[OK]{{reset}}   Tag created: v{{version}}\n'

# ============================================================================
# UTILITIES
# ============================================================================

[group('util')]
[doc("Count lines of code")]
loc:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Lines of code:\n'
    tokei . --exclude target --exclude node_modules 2>/dev/null || \
        find crates -name '*.rs' | xargs wc -l | tail -1

[group('util')]
[doc("Analyze binary size bloat")]
bloat crate="mssql-client":
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Binary size analysis for {{crate}}...\n'
    {{cargo}} bloat --release -p {{crate}} --crates

[group('security')]
[doc("Check for unsafe code usage")]
geiger:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Scanning for unsafe code...\n'
    for crate in crates/*/; do
        name=$(basename "$crate")
        printf '{{cyan}}[INFO]{{reset}} Scanning %s...\n' "$name"
        {{cargo}} geiger -p "$name" --all-features --all-targets 2>/dev/null || true
    done
    printf '{{green}}[OK]{{reset}}   Unsafe code scan complete\n'

[group('util')]
[doc("Show expanded macros")]
expand crate:
    #!/usr/bin/env bash
    printf '{{cyan}}[INFO]{{reset}} Expanding macros in {{crate}}...\n'
    {{cargo}} expand -p {{crate}}

[group('util')]
[doc("Generate and display project statistics")]
stats: loc
    #!/usr/bin/env bash
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
check-tools:
    #!/usr/bin/env bash
    printf '\n{{bold}}Development Tool Status{{reset}}\n'
    printf '═══════════════════════════════════════\n'

    check_tool() {
        if command -v "$1" &> /dev/null || {{cargo}} "$1" --version &> /dev/null 2>&1; then
            printf '{{green}}✓{{reset}} %s\n' "$1"
        else
            printf '{{red}}✗{{reset}} %s (not installed)\n' "$1"
        fi
    }

    # Core tools
    printf '\n{{cyan}}Core:{{reset}}\n'
    check_tool "rustfmt"
    check_tool "clippy"

    # Cargo extensions
    printf '\n{{cyan}}Cargo Extensions:{{reset}}\n'
    for tool in nextest llvm-cov audit deny outdated watch \
                semver-checks machete bloat geiger expand; do
        if {{cargo}} $tool --version &> /dev/null 2>&1; then
            printf '{{green}}✓{{reset}} cargo-%s\n' "$tool"
        else
            printf '{{red}}✗{{reset}} cargo-%s\n' "$tool"
        fi
    done

    # External tools
    printf '\n{{cyan}}External:{{reset}}\n'
    check_tool "tokei"
    check_tool "lychee"
    check_tool "docker"
    check_tool "jq"

    printf '\n'

[group('help')]
[doc("Show all available recipes grouped by category")]
help:
    #!/usr/bin/env bash
    printf '\n{{bold}}{{project_name}} v{{version}}{{reset}} — SQL Server Driver Development\n'
    printf 'MSRV: {{msrv}} | Edition: {{edition}} | Platform: {{platform}}\n\n'
    printf '{{bold}}Usage:{{reset}} just [recipe] [arguments...]\n\n'
    just --list --unsorted
