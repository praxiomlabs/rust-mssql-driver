//! Build automation tasks for the rust-mssql-driver workspace.
//!
//! Run with `cargo xtask <command>`.
//!
//! ## Available Commands
//!
//! - `ci`: Run all CI checks (format, lint, test, deny)
//! - `fmt`: Check/apply code formatting
//! - `clippy`: Run clippy lints
//! - `test`: Run all tests
//! - `deny`: Run cargo-deny checks
//! - `doc`: Generate documentation
//! - `bench`: Run benchmarks
//! - `clean`: Clean build artifacts
//! - `fuzz`: Run fuzz tests (requires cargo-fuzz + nightly)
//! - `codegen`: Generate protocol constants from TDS spec
//! - `release`: Prepare a release (bump versions, update changelog)
//! - `check-features`: Validate all feature flag combinations compile
//! - `ci-local`: Run full CI pipeline locally (mirrors GitHub Actions)
//! - `dist`: Build release artifacts for distribution

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use xshell::{Shell, cmd};

#[derive(Parser)]
#[command(name = "xtask", about = "Build automation for rust-mssql-driver")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run all checks (format, lint, test, deny)
    Ci,
    /// Run cargo fmt (--check by default, --fix to apply)
    Fmt {
        /// Apply formatting fixes
        #[arg(long)]
        fix: bool,
    },
    /// Run clippy with all features
    Clippy {
        /// Apply clippy suggestions
        #[arg(long)]
        fix: bool,
    },
    /// Run all tests
    Test {
        /// Test a specific package
        #[arg(short, long)]
        package: Option<String>,
        /// Run integration tests
        #[arg(long)]
        integration: bool,
    },
    /// Run cargo-deny checks
    Deny,
    /// Generate documentation
    Doc {
        /// Open documentation in browser
        #[arg(long)]
        open: bool,
    },
    /// Run benchmarks
    Bench {
        /// Benchmark filter pattern
        filter: Option<String>,
    },
    /// Clean build artifacts
    Clean,
    /// Run fuzz tests (requires cargo-fuzz + nightly)
    Fuzz {
        /// Fuzz target to run
        #[arg(default_value = "parse_packet")]
        target: String,
        /// Maximum runtime in seconds
        #[arg(long, default_value = "60")]
        max_time: u64,
        /// List available fuzz targets
        #[arg(long)]
        list: bool,
    },
    /// Generate protocol constants from TDS specification
    Codegen {
        /// Verify generated code matches without updating
        #[arg(long)]
        check: bool,
    },
    /// Build release artifacts for distribution
    Dist {
        /// Target triple (e.g., x86_64-unknown-linux-gnu)
        #[arg(long)]
        target: Option<String>,
        /// Skip running tests before building
        #[arg(long)]
        no_test: bool,
    },
    /// Initialize fuzz testing infrastructure
    FuzzInit,
    /// Run code coverage
    Coverage {
        /// Output format (html, lcov, json)
        #[arg(long, default_value = "html")]
        format: String,
    },
    /// Check for semver violations (requires cargo-semver-checks)
    Semver,
    /// Prepare a release: bump versions and update changelog
    Release {
        /// New version (e.g., 0.7.0)
        version: String,
        /// Skip changelog update
        #[arg(long)]
        no_changelog: bool,
    },
    /// Validate all feature flag combinations compile (requires cargo-hack)
    CheckFeatures,
    /// Run the full CI pipeline locally (mirrors GitHub Actions)
    CiLocal,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    // Change to workspace root
    let workspace_root = workspace_root()?;
    sh.change_dir(&workspace_root);

    match cli.command {
        Command::Ci => {
            println!("Running CI checks...");
            fmt(&sh, false)?;
            clippy(&sh, false)?;
            test(&sh, None, false)?;
            deny(&sh)?;
            println!("\n✅ All CI checks passed!");
        }
        Command::Fmt { fix } => fmt(&sh, fix)?,
        Command::Clippy { fix } => clippy(&sh, fix)?,
        Command::Test {
            package,
            integration,
        } => test(&sh, package.as_deref(), integration)?,
        Command::Deny => deny(&sh)?,
        Command::Doc { open } => doc(&sh, open)?,
        Command::Bench { filter } => bench(&sh, filter.as_deref())?,
        Command::Clean => clean(&sh)?,
        Command::Fuzz {
            target,
            max_time,
            list,
        } => fuzz(&sh, &target, max_time, list)?,
        Command::Codegen { check } => codegen(&sh, check)?,
        Command::Dist { target, no_test } => dist(&sh, target.as_deref(), no_test)?,
        Command::FuzzInit => fuzz_init(&sh)?,
        Command::Coverage { format } => coverage(&sh, &format)?,
        Command::Semver => semver(&sh)?,
        Command::Release {
            version,
            no_changelog,
        } => release(&sh, &version, no_changelog)?,
        Command::CheckFeatures => check_features(&sh)?,
        Command::CiLocal => ci_local(&sh)?,
    }

    Ok(())
}

/// Check that a cargo subcommand is installed, providing install instructions if not.
fn require_tool(tool: &str, install_cmd: &str) -> Result<()> {
    let status = std::process::Command::new("cargo")
        .args([tool, "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => bail!(
            "`cargo {tool}` is not installed.\n\n\
             Install it with:\n\n    \
             {install_cmd}\n"
        ),
    }
}

fn workspace_root() -> Result<PathBuf> {
    let output = std::process::Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .context("failed to run cargo locate-project")?;

    let path = String::from_utf8(output.stdout)
        .context("invalid UTF-8 in cargo output")?
        .trim()
        .to_string();

    Ok(PathBuf::from(path)
        .parent()
        .context("failed to get workspace root")?
        .to_path_buf())
}

fn fmt(sh: &Shell, fix: bool) -> Result<()> {
    if fix {
        println!("Applying formatting...");
        cmd!(sh, "cargo fmt --all").run()?;
        println!("✅ Formatting applied.");
    } else {
        println!("Checking formatting...");
        cmd!(sh, "cargo fmt --all -- --check").run()?;
        println!("✅ Formatting check passed.");
    }
    Ok(())
}

fn clippy(sh: &Shell, fix: bool) -> Result<()> {
    if fix {
        println!("Applying clippy suggestions...");
        cmd!(
            sh,
            "cargo clippy --all-features --all-targets --fix --allow-dirty"
        )
        .run()?;
        println!("✅ Clippy suggestions applied.");
    } else {
        println!("Running clippy...");
        cmd!(
            sh,
            "cargo clippy --all-features --all-targets -- -D warnings"
        )
        .run()?;
        println!("✅ Clippy check passed.");
    }
    Ok(())
}

fn test(sh: &Shell, package: Option<&str>, integration: bool) -> Result<()> {
    println!("Running tests...");

    let mut args = vec!["test"];

    if let Some(pkg) = package {
        args.push("-p");
        args.push(pkg);
    }

    args.push("--all-features");

    if integration {
        args.push("--features");
        args.push("integration-tests");
    }

    let args_str = args.join(" ");
    cmd!(sh, "cargo {args_str}").run()?;
    println!("✅ All tests passed.");
    Ok(())
}

fn deny(sh: &Shell) -> Result<()> {
    require_tool("deny", "cargo install cargo-deny")?;
    println!("Running cargo-deny...");
    cmd!(sh, "cargo deny check").run()?;
    println!("✅ Cargo-deny check passed.");
    Ok(())
}

fn doc(sh: &Shell, open: bool) -> Result<()> {
    println!("Generating documentation...");
    if open {
        cmd!(sh, "cargo doc --all-features --no-deps --open").run()?;
    } else {
        cmd!(sh, "cargo doc --all-features --no-deps").run()?;
    }
    println!("✅ Documentation generated.");
    Ok(())
}

fn bench(sh: &Shell, filter: Option<&str>) -> Result<()> {
    println!("Running benchmarks...");
    if let Some(f) = filter {
        cmd!(sh, "cargo bench -- {f}").run()?;
    } else {
        cmd!(sh, "cargo bench").run()?;
    }
    Ok(())
}

fn clean(sh: &Shell) -> Result<()> {
    println!("Cleaning build artifacts...");
    cmd!(sh, "cargo clean").run()?;
    println!("✅ Clean complete.");
    Ok(())
}

fn fuzz(sh: &Shell, target: &str, max_time: u64, list: bool) -> Result<()> {
    require_tool("fuzz", "cargo install cargo-fuzz && rustup install nightly")?;
    let fuzz_dir = sh.current_dir().join("fuzz");

    if list {
        println!("Available fuzz targets:");
        let targets_dir = fuzz_dir.join("fuzz_targets");
        if targets_dir.exists() {
            for entry in fs::read_dir(&targets_dir)? {
                let entry = entry?;
                if let Some(name) = entry.path().file_stem() {
                    println!("  - {}", name.to_string_lossy());
                }
            }
        } else {
            println!("  No fuzz targets found. Run `cargo xtask fuzz-init` to set up fuzzing.");
        }
        return Ok(());
    }

    if !fuzz_dir.exists() {
        bail!(
            "Fuzz directory not found. Run `cargo xtask fuzz-init` to set up fuzzing infrastructure."
        );
    }

    println!("Running fuzz target: {target}");
    println!("Max time: {max_time} seconds");

    // cargo-fuzz requires nightly
    let max_time_str = max_time.to_string();
    cmd!(
        sh,
        "cargo +nightly fuzz run {target} -- -max_total_time={max_time_str}"
    )
    .run()?;

    Ok(())
}

fn fuzz_init(sh: &Shell) -> Result<()> {
    let fuzz_dir = sh.current_dir().join("fuzz");

    if fuzz_dir.exists() {
        println!("Fuzz directory already exists.");
        return Ok(());
    }

    println!("Initializing fuzz testing infrastructure...");

    // Create fuzz directory structure
    fs::create_dir_all(fuzz_dir.join("fuzz_targets"))?;

    // Create fuzz Cargo.toml
    let cargo_toml = r#"[package]
name = "mssql-fuzz"
version = "0.0.0"
publish = false
edition = "2024"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
arbitrary = { version = "1.3", features = ["derive"] }

[dependencies.tds-protocol]
path = "../crates/tds-protocol"

[dependencies.mssql-client]
path = "../crates/mssql-client"

[[bin]]
name = "parse_packet"
path = "fuzz_targets/parse_packet.rs"
test = false
doc = false
bench = false

[[bin]]
name = "parse_token"
path = "fuzz_targets/parse_token.rs"
test = false
doc = false
bench = false

[[bin]]
name = "connection_string"
path = "fuzz_targets/connection_string.rs"
test = false
doc = false
bench = false
"#;
    fs::write(fuzz_dir.join("Cargo.toml"), cargo_toml)?;

    // Create parse_packet fuzz target
    let parse_packet = r#"#![no_main]

use libfuzzer_sys::fuzz_target;
use tds_protocol::PacketHeader;
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    // Fuzz packet header parsing
    if data.len() >= 8 {
        let mut cursor = data;
        let _ = PacketHeader::decode(&mut cursor);
    }
});
"#;
    fs::write(fuzz_dir.join("fuzz_targets/parse_packet.rs"), parse_packet)?;

    // Create parse_token fuzz target
    let parse_token = r#"#![no_main]

use libfuzzer_sys::fuzz_target;
use tds_protocol::TokenParser;
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    // Fuzz token parsing
    let bytes = Bytes::copy_from_slice(data);
    let mut parser = TokenParser::new(bytes);

    // Try to parse tokens until exhausted or error
    while let Ok(Some(_)) = parser.next_token() {
        // Continue parsing
    }
});
"#;
    fs::write(fuzz_dir.join("fuzz_targets/parse_token.rs"), parse_token)?;

    // Create connection_string fuzz target
    let connection_string = r#"#![no_main]

use libfuzzer_sys::fuzz_target;
use mssql_client::Config;

fuzz_target!(|data: &[u8]| {
    // Fuzz connection string parsing
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Config::from_connection_string(s);
    }
});
"#;
    fs::write(
        fuzz_dir.join("fuzz_targets/connection_string.rs"),
        connection_string,
    )?;

    println!("✅ Fuzz infrastructure initialized.");
    println!("\nAvailable fuzz targets:");
    println!("  - parse_packet   : Fuzz TDS packet header parsing");
    println!("  - parse_token    : Fuzz TDS token stream parsing");
    println!("  - connection_string : Fuzz connection string parsing");
    println!("\nTo run fuzzing:");
    println!("  cargo xtask fuzz parse_packet --max-time 300");
    println!("\nNote: Fuzzing requires nightly Rust and cargo-fuzz:");
    println!("  rustup install nightly");
    println!("  cargo install cargo-fuzz");

    Ok(())
}

fn codegen(sh: &Shell, check: bool) -> Result<()> {
    println!("Generating protocol constants...");

    let codegen_dir = sh.current_dir().join("crates/tds-protocol/src/generated");

    if !codegen_dir.exists() {
        fs::create_dir_all(&codegen_dir)?;
    }

    // Generate TDS type IDs from MS-TDS specification
    let type_ids = generate_type_ids();
    let type_ids_path = codegen_dir.join("type_ids.rs");

    // Generate token type IDs
    let token_types = generate_token_types();
    let token_types_path = codegen_dir.join("token_types.rs");

    // Generate packet types
    let packet_types = generate_packet_types();
    let packet_types_path = codegen_dir.join("packet_types.rs");

    if check {
        // Verify generated code matches
        let current_type_ids = fs::read_to_string(&type_ids_path).unwrap_or_default();
        let current_token_types = fs::read_to_string(&token_types_path).unwrap_or_default();
        let current_packet_types = fs::read_to_string(&packet_types_path).unwrap_or_default();

        if current_type_ids != type_ids
            || current_token_types != token_types
            || current_packet_types != packet_types
        {
            bail!("Generated code is out of date. Run `cargo xtask codegen` to update.");
        }
        println!("✅ Generated code is up to date.");
    } else {
        fs::write(&type_ids_path, &type_ids)?;
        fs::write(&token_types_path, &token_types)?;
        fs::write(&packet_types_path, &packet_types)?;

        // Generate mod.rs to expose generated modules
        let mod_rs = r#"//! Generated protocol constants.
//!
//! This module is auto-generated by `cargo xtask codegen`.
//! Do not edit manually.

pub mod type_ids;
pub mod token_types;
pub mod packet_types;
"#;
        fs::write(codegen_dir.join("mod.rs"), mod_rs)?;

        println!("✅ Protocol constants generated.");
        println!("   - {}", type_ids_path.display());
        println!("   - {}", token_types_path.display());
        println!("   - {}", packet_types_path.display());
    }

    Ok(())
}

fn generate_type_ids() -> String {
    r#"//! TDS Type IDs from MS-TDS specification.
//!
//! Auto-generated by `cargo xtask codegen`.

/// TDS data type identifiers.
///
/// These values are defined in [MS-TDS] Section 2.2.5.4.
pub mod type_id {
    // Fixed-length types
    pub const NULL: u8 = 0x1F;
    pub const INT1: u8 = 0x30;        // TinyInt
    pub const BIT: u8 = 0x32;
    pub const INT2: u8 = 0x34;        // SmallInt
    pub const INT4: u8 = 0x38;        // Int
    pub const DATETIM4: u8 = 0x3A;    // SmallDateTime
    pub const FLT4: u8 = 0x3B;        // Real
    pub const MONEY: u8 = 0x3C;
    pub const DATETIME: u8 = 0x3D;
    pub const FLT8: u8 = 0x3E;        // Float
    pub const MONEY4: u8 = 0x7A;      // SmallMoney
    pub const INT8: u8 = 0x7F;        // BigInt

    // Variable-length types
    pub const GUID: u8 = 0x24;
    pub const INTN: u8 = 0x26;
    pub const DECIMAL: u8 = 0x37;
    pub const NUMERIC: u8 = 0x3F;
    pub const BITN: u8 = 0x68;
    pub const DECIMALN: u8 = 0x6A;
    pub const NUMERICN: u8 = 0x6C;
    pub const FLTN: u8 = 0x6D;
    pub const MONEYN: u8 = 0x6E;
    pub const DATETIMN: u8 = 0x6F;

    // Legacy byte-length strings
    pub const CHAR: u8 = 0x2F;
    pub const VARCHAR: u8 = 0x27;
    pub const BINARY: u8 = 0x2D;
    pub const VARBINARY: u8 = 0x25;

    // Big types (2-byte length prefix)
    pub const BIGVARCHAR: u8 = 0xA7;
    pub const BIGCHAR: u8 = 0xAF;
    pub const BIGVARBINARY: u8 = 0xA5;
    pub const BIGBINARY: u8 = 0xAD;

    // Unicode types
    pub const NVARCHAR: u8 = 0xE7;
    pub const NCHAR: u8 = 0xEF;

    // Date/Time types (SQL Server 2008+)
    pub const DATE: u8 = 0x28;
    pub const TIME: u8 = 0x29;
    pub const DATETIME2: u8 = 0x2A;
    pub const DATETIMEOFFSET: u8 = 0x2B;

    // LOB types
    pub const TEXT: u8 = 0x23;
    pub const NTEXT: u8 = 0x63;
    pub const IMAGE: u8 = 0x22;

    // Complex types
    pub const XML: u8 = 0xF1;
    pub const UDT: u8 = 0xF0;
    pub const TVP: u8 = 0xF3;
    pub const VARIANT: u8 = 0x62;
}
"#
    .to_string()
}

fn generate_token_types() -> String {
    r#"//! TDS Token Types from MS-TDS specification.
//!
//! Auto-generated by `cargo xtask codegen`.

/// TDS token type identifiers.
///
/// These values are defined in [MS-TDS] Section 2.2.4.
pub mod token_type {
    pub const ALTMETADATA: u8 = 0x88;
    pub const ALTROW: u8 = 0xD3;
    pub const COLINFO: u8 = 0xA5;
    pub const COLMETADATA: u8 = 0x81;
    pub const DONE: u8 = 0xFD;
    pub const DONEINPROC: u8 = 0xFF;
    pub const DONEPROC: u8 = 0xFE;
    pub const ENVCHANGE: u8 = 0xE3;
    pub const ERROR: u8 = 0xAA;
    pub const FEATUREEXTACK: u8 = 0xAE;
    pub const FEDAUTHINFO: u8 = 0xEE;
    pub const INFO: u8 = 0xAB;
    pub const LOGINACK: u8 = 0xAD;
    pub const NBCROW: u8 = 0xD2;
    pub const OFFSET: u8 = 0x78;
    pub const ORDER: u8 = 0xA9;
    pub const RETURNSTATUS: u8 = 0x79;
    pub const RETURNVALUE: u8 = 0xAC;
    pub const ROW: u8 = 0xD1;
    pub const SESSIONSTATE: u8 = 0xE4;
    pub const SSPI: u8 = 0xED;
    pub const TABNAME: u8 = 0xA4;
}
"#
    .to_string()
}

fn generate_packet_types() -> String {
    r#"//! TDS Packet Types from MS-TDS specification.
//!
//! Auto-generated by `cargo xtask codegen`.

/// TDS packet type identifiers.
///
/// These values are defined in [MS-TDS] Section 2.2.3.1.1.
pub mod packet_type {
    pub const SQL_BATCH: u8 = 0x01;
    pub const PRE_TDS7_LOGIN: u8 = 0x02;
    pub const RPC: u8 = 0x03;
    pub const TABULAR_RESULT: u8 = 0x04;
    pub const ATTENTION: u8 = 0x06;
    pub const BULK_LOAD: u8 = 0x07;
    pub const FED_AUTH_TOKEN: u8 = 0x08;
    pub const TRANSACTION_MANAGER: u8 = 0x0E;
    pub const TDS7_LOGIN: u8 = 0x10;
    pub const SSPI: u8 = 0x11;
    pub const PRELOGIN: u8 = 0x12;
}

/// TDS packet status flags.
///
/// These values are defined in [MS-TDS] Section 2.2.3.1.1.
pub mod packet_status {
    pub const NORMAL: u8 = 0x00;
    pub const END_OF_MESSAGE: u8 = 0x01;
    pub const IGNORE_EVENT: u8 = 0x02;
    pub const RESET_CONNECTION: u8 = 0x08;
    pub const RESET_CONNECTION_KEEP_TRANSACTION: u8 = 0x10;
}
"#
    .to_string()
}

fn dist(sh: &Shell, target: Option<&str>, no_test: bool) -> Result<()> {
    println!("Building release artifacts...");

    if !no_test {
        println!("Running tests before build...");
        test(sh, None, false)?;
    }

    let dist_dir = sh.current_dir().join("target/dist");
    fs::create_dir_all(&dist_dir)?;

    // Build in release mode
    println!("Building release binaries...");
    if let Some(t) = target {
        cmd!(sh, "cargo build --release --target {t}").run()?;
    } else {
        cmd!(sh, "cargo build --release").run()?;
    }

    // Package each crate
    println!("Packaging crates...");
    let crates = [
        "tds-protocol",
        "mssql-types",
        "mssql-tls",
        "mssql-codec",
        "mssql-auth",
        "mssql-client",
        "mssql-pool",
        "mssql-derive",
    ];

    for crate_name in &crates {
        cmd!(sh, "cargo package -p {crate_name} --allow-dirty").run()?;
    }

    println!("✅ Distribution artifacts built.");
    println!("   Release binaries: target/release/");
    println!("   Packages: target/package/");

    Ok(())
}

fn coverage(sh: &Shell, format: &str) -> Result<()> {
    require_tool("llvm-cov", "cargo install cargo-llvm-cov")?;
    println!("Running code coverage...");
    match format {
        "html" => {
            cmd!(sh, "cargo llvm-cov --all-features --html").run()?;
            println!("✅ Coverage report: target/llvm-cov/html/index.html");
        }
        "lcov" => {
            cmd!(
                sh,
                "cargo llvm-cov --all-features --lcov --output-path target/lcov.info"
            )
            .run()?;
            println!("✅ Coverage report: target/lcov.info");
        }
        "json" => {
            cmd!(
                sh,
                "cargo llvm-cov --all-features --json --output-path target/coverage.json"
            )
            .run()?;
            println!("✅ Coverage report: target/coverage.json");
        }
        _ => {
            bail!("Unknown coverage format: {format}. Use html, lcov, or json.");
        }
    }

    Ok(())
}

fn semver(sh: &Shell) -> Result<()> {
    require_tool("semver-checks", "cargo install cargo-semver-checks")?;
    println!("Checking for semver violations...");

    let crates = [
        "tds-protocol",
        "mssql-types",
        "mssql-client",
        "mssql-driver-pool",
    ];

    for crate_name in &crates {
        println!("  Checking {crate_name}...");
        cmd!(sh, "cargo semver-checks check-release -p {crate_name}").run()?;
    }

    println!("✅ No semver violations detected.");
    Ok(())
}

fn check_features(sh: &Shell) -> Result<()> {
    require_tool("hack", "cargo install cargo-hack")?;
    println!("Checking feature flag combinations...");

    // tds-protocol: requires std or alloc; encoding also needs one of them.
    // Exclude both bare --no-default-features and encoding from the sweep,
    // then test encoding + alloc (no_std) separately.
    println!("\n  tds-protocol...");
    cmd!(
        sh,
        "cargo hack check -p tds-protocol --each-feature --no-dev-deps --exclude-no-default-features --exclude-features encoding"
    )
    .run()?;
    // Verify no_std + alloc works
    cmd!(
        sh,
        "cargo check -p tds-protocol --no-default-features --features alloc"
    )
    .run()?;
    // Verify encoding works in no_std context
    cmd!(
        sh,
        "cargo check -p tds-protocol --no-default-features --features alloc,encoding"
    )
    .run()?;

    // mssql-types: all features are independent
    println!("\n  mssql-types...");
    cmd!(
        sh,
        "cargo hack check -p mssql-types --each-feature --no-dev-deps"
    )
    .run()?;

    // mssql-client: all features are independent
    println!("\n  mssql-client...");
    cmd!(
        sh,
        "cargo hack check -p mssql-client --each-feature --no-dev-deps"
    )
    .run()?;

    // mssql-auth: platform-specific features need exclusion
    println!("\n  mssql-auth...");
    let excluded = platform_excluded_auth_features();
    if excluded.is_empty() {
        cmd!(
            sh,
            "cargo hack check -p mssql-auth --each-feature --no-dev-deps"
        )
        .run()?;
    } else {
        let excluded_str = excluded.join(",");
        cmd!(
            sh,
            "cargo hack check -p mssql-auth --each-feature --no-dev-deps --exclude-features {excluded_str}"
        )
        .run()?;
    }

    // mssql-driver-pool: no features, but verify it compiles
    println!("\n  mssql-driver-pool...");
    cmd!(sh, "cargo hack check -p mssql-driver-pool --no-dev-deps").run()?;

    println!("\n✅ All feature flag combinations compile.");
    Ok(())
}

/// Returns auth features that cannot compile on the current platform.
fn platform_excluded_auth_features() -> Vec<&'static str> {
    let mut excluded = Vec::new();
    if !cfg!(target_os = "windows") {
        excluded.push("sspi-auth");
        excluded.push("windows-certstore");
    }
    if cfg!(target_os = "windows") {
        // libgssapi requires GSSAPI/Kerberos libraries (Linux/macOS)
        excluded.push("integrated-auth");
    }
    excluded
}

fn ci_local(sh: &Shell) -> Result<()> {
    println!("Running full CI pipeline locally (mirrors GitHub Actions)...\n");

    // Step 1: Format check
    println!("── Step 1/7: Format check ──");
    fmt(sh, false)?;

    // Step 2: Clippy
    println!("\n── Step 2/7: Clippy ──");
    clippy(sh, false)?;

    // Step 3: Tests
    println!("\n── Step 3/7: Tests ──");
    test(sh, None, false)?;

    // Step 4: Documentation
    println!("\n── Step 4/7: Documentation ──");
    doc(sh, false)?;

    // Step 5: Build examples
    println!("\n── Step 5/7: Examples ──");
    println!("Building examples...");
    cmd!(sh, "cargo build --examples --all-features").run()?;
    println!("✅ Examples build passed.");

    // Step 6: Feature flag validation
    println!("\n── Step 6/7: Feature flags ──");
    check_features(sh)?;

    // Step 7: cargo-deny (if available)
    println!("\n── Step 7/7: Dependency audit ──");
    match deny(sh) {
        Ok(()) => {}
        Err(e) => {
            println!("⚠ cargo-deny skipped: {e}");
            println!("  Install with: cargo install cargo-deny");
        }
    }

    println!("\n✅ Full CI pipeline passed!");
    println!("\nNote: MSRV check and Miri are not included in local CI.");
    println!("      Run these separately if needed:");
    println!("        MSRV:  rustup run 1.88 cargo check --all-features");
    println!("        Miri:  cargo +nightly miri test -p tds-protocol");

    Ok(())
}

fn release(sh: &Shell, version: &str, no_changelog: bool) -> Result<()> {
    let new_ver = parse_semver(version)
        .with_context(|| format!("Invalid version: {version} (expected X.Y.Z)"))?;

    let cargo_toml_path = sh.current_dir().join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;

    let current = extract_workspace_version(&cargo_toml)
        .context("Failed to find workspace.package version in Cargo.toml")?;
    let cur_ver =
        parse_semver(&current).context("Current version in Cargo.toml is not valid semver")?;

    if new_ver <= cur_ver {
        bail!("New version ({version}) must be greater than current version ({current})");
    }

    println!("Bumping version: {current} → {version}");

    // Update root Cargo.toml
    let updated_cargo = bump_cargo_versions(&cargo_toml, &current, version)?;
    fs::write(&cargo_toml_path, &updated_cargo)?;
    println!("  ✓ Cargo.toml workspace version");
    println!("  ✓ Cargo.toml internal dependency versions (9 crates)");

    // Update CHANGELOG.md
    if !no_changelog {
        let changelog_path = sh.current_dir().join("CHANGELOG.md");
        if changelog_path.exists() {
            let changelog = fs::read_to_string(&changelog_path)?;
            let today = cmd!(sh, "date +%Y-%m-%d").read()?;
            let updated = bump_changelog(&changelog, version, &current, today.trim())?;
            fs::write(&changelog_path, &updated)?;
            println!("  ✓ CHANGELOG.md");
        } else {
            println!("  ⚠ CHANGELOG.md not found, skipping");
        }
    } else {
        println!("  ⊘ CHANGELOG.md (skipped)");
    }

    // Validate consistency
    let final_toml = fs::read_to_string(&cargo_toml_path)?;
    validate_version_consistency(&final_toml, version)?;

    println!("\n✅ Version bumped to {version}");
    println!("\nNext steps:");
    println!("  1. Review changes:  git diff");
    println!("  2. Commit:          git commit -am \"chore: release v{version}\"");
    println!("  3. Push to main:    git push origin main");
    println!("  4. Wait for CI to pass");
    println!("  5. Tag:             git tag -a v{version} -m \"Release v{version}\"");
    println!("  6. Push tag:        git push origin v{version}");

    Ok(())
}

/// Parse a version string like "1.2.3" into a comparable tuple.
fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

/// Extract `version = "X.Y.Z"` from the `[workspace.package]` section.
fn extract_workspace_version(cargo_toml: &str) -> Option<String> {
    let mut in_workspace_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
        }
        if in_workspace_package && trimmed.starts_with("version") {
            let value = trimmed.split('=').nth(1)?.trim();
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// Replace version strings in root Cargo.toml:
/// 1. `workspace.package.version`
/// 2. All internal dependency entries (lines with both `version = "..."` and `path = "crates/..."`)
fn bump_cargo_versions(content: &str, old: &str, new: &str) -> Result<String> {
    let mut result = Vec::new();
    let mut in_workspace_package = false;
    let mut version_bumped = false;
    let mut dep_count = 0u32;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
        }

        if in_workspace_package
            && trimmed.starts_with("version")
            && trimmed.contains(&format!("\"{old}\""))
        {
            result.push(line.replace(old, new));
            version_bumped = true;
        } else if trimmed.contains(&format!("version = \"{old}\""))
            && trimmed.contains("path = \"crates/")
        {
            result.push(line.replace(old, new));
            dep_count += 1;
        } else {
            result.push(line.to_string());
        }
    }

    if !version_bumped {
        bail!("Could not find workspace.package version = \"{old}\" in Cargo.toml");
    }
    if dep_count == 0 {
        bail!("No internal dependency versions found matching \"{old}\"");
    }

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// Update CHANGELOG.md for a new release:
/// - If `## [Unreleased]` exists, rename it to `## [version] - date` and add new `## [Unreleased]`
/// - If no `## [Unreleased]`, insert both before the first version entry
/// - Update comparison links at the bottom
fn bump_changelog(
    content: &str,
    new_version: &str,
    old_version: &str,
    today: &str,
) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let repo_url = "https://github.com/praxiomlabs/rust-mssql-driver";

    let has_unreleased = lines.iter().any(|l| l.starts_with("## [Unreleased]"));

    for line in &lines {
        if has_unreleased && *line == "## [Unreleased]" {
            // Keep [Unreleased] as new empty section, add versioned section below
            result.push("## [Unreleased]".to_string());
            result.push(String::new());
            result.push(format!("## [{new_version}] - {today}"));
        } else if !has_unreleased
            && line.starts_with("## [")
            && !result.iter().any(|r| r.starts_with("## [Unreleased]"))
        {
            // First version entry found — insert new sections before it
            result.push("## [Unreleased]".to_string());
            result.push(String::new());
            result.push(format!("## [{new_version}] - {today}"));
            result.push(String::new());
            result.push(line.to_string());
        } else if line.starts_with("[Unreleased]:") {
            // Update unreleased comparison link and add new version link
            result.push(format!(
                "[Unreleased]: {repo_url}/compare/v{new_version}...HEAD"
            ));
            result.push(format!(
                "[{new_version}]: {repo_url}/compare/v{old_version}...v{new_version}"
            ));
        } else {
            result.push(line.to_string());
        }
    }

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// Verify that all versions in Cargo.toml are consistent after bumping.
fn validate_version_consistency(cargo_toml: &str, expected: &str) -> Result<()> {
    let actual = extract_workspace_version(cargo_toml)
        .context("Could not read workspace version after update")?;
    if actual != expected {
        bail!("Validation failed: workspace version is {actual}, expected {expected}");
    }

    let dep_count = cargo_toml
        .lines()
        .filter(|line| {
            line.contains("path = \"crates/") && line.contains(&format!("version = \"{expected}\""))
        })
        .count();

    if dep_count != 9 {
        bail!(
            "Validation failed: found {dep_count}/9 internal dependencies with version {expected}"
        );
    }

    Ok(())
}
