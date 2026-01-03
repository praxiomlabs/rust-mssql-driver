//! Internal prelude for no_std compatibility.
//!
//! This module re-exports types that are needed throughout the crate,
//! sourcing them from either `std` or `alloc` depending on feature flags.

// When std is enabled, re-export from std
#[cfg(feature = "std")]
pub use std::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

// When alloc is enabled (but not std), re-export from alloc
#[cfg(all(feature = "alloc", not(feature = "std")))]
pub use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
