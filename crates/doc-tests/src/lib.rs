//! Compile-tests for the Markdown documentation in this repository.
//!
//! This crate is **not published**. It exists so that
//! `cargo test --doc -p doc-tests` compiles the runnable Rust examples in the
//! project's Markdown guides, preventing the doc-rot that previously left broken
//! examples undetected.
//!
//! ## Convention
//!
//! - A ` ```rust ` block **must compile**. Use ` ```rust,no_run ` for examples
//!   that need a live SQL Server (they compile but are not executed).
//! - Genuinely illustrative fragments (partial snippets, undefined variables) and
//!   non-Rust snippets must use ` ```text `.
//!
//! ## Scope
//!
//! The guides below contain complete, runnable usage examples and are
//! compile-tested. Pure-reference docs that consist of short illustrative
//! fragments (e.g. `CONNECTION_STRINGS.md` connection-string formats,
//! `VERSION_REFS.md`, `DEPENDENCY_POLICY.md`, `BENCHMARKS.md`, `COMPARISON.md`,
//! `FEATURES.md`, `SQL_SERVER_COMPATIBILITY.md`, `FILESTREAM.md` Windows APIs)
//! keep their Rust syntax highlighting but are intentionally not included here.

#![doc = include_str!("../../../README.md")]

#[doc = include_str!("../../../docs/ALWAYS_ENCRYPTED.md")]
mod always_encrypted {}
#[doc = include_str!("../../../docs/CANCEL_SAFETY.md")]
mod cancel_safety {}
#[doc = include_str!("../../../docs/CONNECTION_RECOVERY.md")]
mod connection_recovery {}
#[doc = include_str!("../../../docs/DDL.md")]
mod ddl {}
#[doc = include_str!("../../../docs/DEPLOYMENT.md")]
mod deployment {}
#[doc = include_str!("../../../docs/DERIVE_MACROS.md")]
mod derive_macros {}
#[doc = include_str!("../../../docs/ERRORS.md")]
mod errors {}
#[doc = include_str!("../../../docs/LOB.md")]
mod lob {}
#[doc = include_str!("../../../docs/MEMORY.md")]
mod memory {}
#[doc = include_str!("../../../docs/OPENTELEMETRY.md")]
mod opentelemetry {}
#[doc = include_str!("../../../docs/OPERATIONS.md")]
mod operations {}
#[doc = include_str!("../../../docs/POOL_METRICS.md")]
mod pool_metrics {}
#[doc = include_str!("../../../docs/RETRY_STRATEGY.md")]
mod retry_strategy {}
#[doc = include_str!("../../../docs/STORED_PROCEDURES.md")]
mod stored_procedures {}
#[doc = include_str!("../../../docs/TIMEOUTS.md")]
mod timeouts {}
#[doc = include_str!("../../../docs/TLS.md")]
mod tls {}
#[doc = include_str!("../../../docs/TYPE_STATE.md")]
mod type_state {}
#[doc = include_str!("../../../docs/examples/production-configs.md")]
mod production_configs {}
