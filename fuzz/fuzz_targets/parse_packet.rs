#![no_main]

use libfuzzer_sys::fuzz_target;
use tds_protocol::PacketHeader;
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    // Fuzz packet header parsing
    if data.len() >= 8 {
        let mut cursor = data;
        let _ = PacketHeader::decode(&mut cursor);
    }
});
