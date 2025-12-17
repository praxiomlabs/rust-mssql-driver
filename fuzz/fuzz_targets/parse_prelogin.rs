#![no_main]

use libfuzzer_sys::fuzz_target;
use tds_protocol::PreLogin;
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    // Fuzz PRELOGIN response parsing
    // This is security-critical as it parses server responses
    let bytes = Bytes::copy_from_slice(data);
    let _ = PreLogin::decode(bytes);
});
