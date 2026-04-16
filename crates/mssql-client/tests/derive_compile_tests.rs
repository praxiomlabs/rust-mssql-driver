//! Compile-fail tests for derive macros (FromRow, ToParams, Tvp).
//!
//! These tests verify that invalid derive macro usage produces
//! compile errors rather than silently generating broken code.

#[test]
fn derive_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}
