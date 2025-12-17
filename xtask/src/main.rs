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
//! - `hakari`: Update workspace-hack crate
//! - `fuzz`: Run fuzz tests (requires cargo-fuzz + nightly)
//! - `codegen`: Generate protocol constants from TDS spec
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
    /// Update workspace-hack crate (requires cargo-hakari)
    Hakari,
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
        Command::Hakari => hakari(&sh)?,
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
    }

    Ok(())
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

fn hakari(sh: &Shell) -> Result<()> {
    println!("Updating workspace-hack...");
    cmd!(sh, "cargo hakari generate").run()?;
    cmd!(sh, "cargo hakari manage-deps").run()?;
    println!("✅ Workspace-hack updated.");
    Ok(())
}

fn fuzz(sh: &Shell, target: &str, max_time: u64, list: bool) -> Result<()> {
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
    println!("Running code coverage...");

    // Requires cargo-llvm-cov
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
            bail!(
                "Unknown coverage format: {}. Use html, lcov, or json.",
                format
            );
        }
    }

    Ok(())
}

fn semver(sh: &Shell) -> Result<()> {
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
