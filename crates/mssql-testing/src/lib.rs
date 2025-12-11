//! # mssql-testing
//!
//! Test infrastructure for SQL Server driver development.
//!
//! This crate provides utilities for integration testing against
//! SQL Server instances, including testcontainers support.
//!
//! ## Features
//!
//! - SQL Server container management via testcontainers
//! - Test fixture utilities
//! - Connection helpers for tests

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod container;
pub mod fixtures;

pub use container::SqlServerContainer;
