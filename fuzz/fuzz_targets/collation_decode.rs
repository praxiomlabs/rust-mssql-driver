#![no_main]

use arbitrary::Arbitrary;
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use tds_protocol::collation::Collation;

/// Arbitrary collation data for fuzzing.
#[derive(Debug, Arbitrary)]
struct FuzzCollationInput {
    /// Raw collation bytes (5 bytes in TDS protocol)
    collation_bytes: [u8; 5],
    /// String data to decode with this collation
    string_data: Vec<u8>,
}

fuzz_target!(|input: FuzzCollationInput| {
    // Parse collation from bytes
    let mut bytes = Bytes::copy_from_slice(&input.collation_bytes);
    if let Ok(collation) = Collation::decode(&mut bytes) {
        // Test collation properties
        let _lcid = collation.lcid();
        let _sort_id = collation.sort_id();
        let _flags = collation.flags();

        // Test encoding lookup
        let _encoding = collation.encoding();

        // Test string decoding with this collation (if we have string data)
        if !input.string_data.is_empty() {
            let _ = collation.decode_varchar(&input.string_data);
        }
    }
});
