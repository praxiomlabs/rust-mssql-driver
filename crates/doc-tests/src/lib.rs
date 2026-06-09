//! Compile-tests for the repository's root `README.md`.
//!
//! This crate is **not published**. It exists so that
//! `cargo test --doc -p doc-tests` compiles the runnable Rust examples in the
//! root README, preventing the doc-rot that previously left broken examples
//! undetected. The former per-guide Markdown files have been folded into the
//! relevant crates' rustdoc, where their examples are compile-tested by each
//! crate's own `cargo test --doc`.
//!
//! ## Convention
//!
//! - A ` ```rust ` block **must compile**. Use ` ```rust,no_run ` for examples
//!   that need a live SQL Server (they compile but are not executed).
//! - Genuinely illustrative fragments (partial snippets, undefined variables) and
//!   non-Rust snippets must use ` ```text `.
//!
//! Crate READMEs: only `mssql-client` and `mssql-driver-pool` include their
//! README as a doctest (via `#![doc = include_str!("../README.md")]`). The
//! `mssql-tls`, `mssql-types`, `mssql-derive`, `mssql-auth`, `mssql-codec`, and
//! `tds-protocol` READMEs are not yet guarded.

#![doc = include_str!("../../../README.md")]
