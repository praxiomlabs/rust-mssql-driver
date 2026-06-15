//! Build automation tasks for the rust-mssql-driver workspace.
//!
//! Run with `cargo xtask <command>`.
//!
//! `just` is the canonical task runner for this project (see `Justfile`);
//! xtask carries only the Rust-native helpers that `just` should not own:
//!
//! - `check-features`: Validate all feature flag combinations compile (used by CI)

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
    /// Validate all feature flag combinations compile (requires cargo-hack)
    CheckFeatures,
    /// Removed: use `just ci` instead
    #[command(hide = true)]
    Ci,
    /// Removed: use `just ci-all` instead
    #[command(hide = true)]
    CiLocal,
    /// Removed: use `just test` instead
    #[command(hide = true)]
    Test {
        /// Test a specific package
        #[arg(short, long)]
        package: Option<String>,
        /// Run integration tests
        #[arg(long)]
        integration: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    // Change to workspace root
    let workspace_root = workspace_root()?;
    sh.change_dir(&workspace_root);

    match cli.command {
        Command::CheckFeatures => check_features(&sh)?,
        Command::Ci => bail!(
            "`cargo xtask ci` has been removed; `just` is the canonical task runner.\n\
             Use `just ci` (or `just ci-all` for all features)."
        ),
        Command::CiLocal => bail!(
            "`cargo xtask ci-local` has been removed; `just` is the canonical task runner.\n\
             Use `just ci-all` (mirrors the GitHub Actions pipeline)."
        ),
        Command::Test { .. } => bail!(
            "`cargo xtask test` has been removed; `just` is the canonical task runner.\n\
             Use `just test` or `just test-all`; see `just --list` for the full test matrix."
        ),
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
    // Verify no_std for REAL, against a bare-metal target where std is genuinely
    // unavailable. A host `--no-default-features` build does NOT prove no_std —
    // std is still linkable — which is exactly how a broken no_std (bytes/thiserror
    // pulling std) once passed CI. Fall back to the host check with a loud warning
    // when the target isn't installed.
    let no_std_target = "thumbv7em-none-eabi";
    let installed = cmd!(sh, "rustup target list --installed")
        .read()
        .unwrap_or_default();
    if installed.lines().any(|l| l.trim() == no_std_target) {
        cmd!(
            sh,
            "cargo build -p tds-protocol --no-default-features --features alloc --target {no_std_target}"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p tds-protocol --no-default-features --features alloc,encoding --target {no_std_target}"
        )
        .run()?;
    } else if std::env::var_os("CI").is_some() {
        // In CI the target MUST be present — silently falling back to a host
        // check is how the no_std guarantee would rot unnoticed.
        bail!(
            "{no_std_target} is not installed for the active toolchain, so the no_std build \
             cannot be verified. The CI job must `rustup target add {no_std_target}` for the \
             pinned toolchain (rust-toolchain.toml) before running this."
        );
    } else {
        println!(
            "  WARN: {no_std_target} not installed — falling back to a host check that does NOT prove no_std."
        );
        println!("        Install it with: rustup target add {no_std_target}");
        cmd!(
            sh,
            "cargo check -p tds-protocol --no-default-features --features alloc"
        )
        .run()?;
        cmd!(
            sh,
            "cargo check -p tds-protocol --no-default-features --features alloc,encoding"
        )
        .run()?;
    }

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
