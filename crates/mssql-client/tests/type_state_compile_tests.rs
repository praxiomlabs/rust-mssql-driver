//! Compile-fail tests for the type-state Client API.
//!
//! The driver encodes the connection lifecycle in the type system
//! (`Client<Disconnected>` / `Client<Ready>` / `Client<InTransaction>`), so a
//! state-specific method called in the wrong state must fail to compile. These
//! trybuild cases pin that guarantee — the marquee compile-time-safety claim
//! was previously never tested.

#[test]
fn type_state_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail-state/*.rs");
}
