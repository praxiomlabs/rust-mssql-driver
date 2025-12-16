#![no_main]

use libfuzzer_sys::fuzz_target;
use tds_protocol::TokenParser;
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    // Fuzz token parsing
    let bytes = Bytes::copy_from_slice(data);
    let mut parser = TokenParser::new(bytes);

    // Try to parse tokens until exhausted or error
    while let Ok(Some(_)) = parser.next_token() {
        // Continue parsing
    }
});
