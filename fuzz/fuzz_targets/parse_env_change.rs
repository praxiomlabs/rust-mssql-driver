#![no_main]

use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use tds_protocol::token::EnvChange;

fuzz_target!(|data: &[u8]| {
    // Fuzz EnvChange token parsing
    // EnvChange tokens contain database, language, collation, and routing info
    let mut bytes = Bytes::copy_from_slice(data);
    let _ = EnvChange::decode(&mut bytes);
});
