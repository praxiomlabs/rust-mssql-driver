#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tds_protocol::Collation;

/// Arbitrary collation data for fuzzing.
#[derive(Debug, Arbitrary)]
struct FuzzCollationInput {
    /// LCID value
    lcid: u32,
    /// Sort ID value
    sort_id: u8,
}

fuzz_target!(|input: FuzzCollationInput| {
    let collation = Collation {
        lcid: input.lcid,
        sort_id: input.sort_id,
    };

    // Test encoding lookup — should never panic
    let _encoding = collation.encoding();
    let _is_utf8 = collation.is_utf8();
    let _code_page = collation.code_page();
    let _name = collation.encoding_name();
});
