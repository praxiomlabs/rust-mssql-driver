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
//! compile-tested. Two further categories are deliberately *not* compile-tested
//! here, so their examples are verified by hand and can still drift:
//!
//! - Reference docs that are only short illustrative fragments, with no
//!   self-contained example to compile: `CONNECTION_STRINGS.md`,
//!   `DEPENDENCY_POLICY.md`, `COMPARISON.md`,
//!   `SQL_SERVER_COMPATIBILITY.md`, and `FILESTREAM.md` (Windows APIs).
//! - Docs whose examples cannot build as-is in this harness: `ARCHITECTURE.md`
//!   (design-illustrative fragments such as bare `impl` blocks) and
//!   `MIGRATION.md` (interleaves tiberius code that does not
//!   compile against this workspace).
//!
//! Crate READMEs: only `mssql-client` and `mssql-driver-pool` include their
//! README as a doctest (via `#![doc = include_str!("../README.md")]`). The
//! `mssql-tls`, `mssql-types`, `mssql-derive`, `mssql-auth`, `mssql-codec`, and
//! `tds-protocol` READMEs are not yet guarded.

#![doc = include_str!("../../../README.md")]

#[doc = include_str!("../../../docs/ALWAYS_ENCRYPTED.md")]
mod always_encrypted {}
#[doc = include_str!("../../../docs/CANCEL_SAFETY.md")]
mod cancel_safety {}
#[doc = include_str!("../../../docs/DDL.md")]
mod ddl {}
#[doc = include_str!("../../../docs/ERRORS.md")]
mod errors {}
#[doc = include_str!("../../../docs/LOB.md")]
mod lob {}
#[doc = include_str!("../../../docs/OPENTELEMETRY.md")]
mod opentelemetry {}
#[doc = include_str!("../../../docs/POOL_METRICS.md")]
mod pool_metrics {}
#[doc = include_str!("../../../docs/STORED_PROCEDURES.md")]
mod stored_procedures {}
#[doc = include_str!("../../../docs/TLS.md")]
mod tls {}
