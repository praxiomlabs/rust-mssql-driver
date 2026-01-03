#![no_main]

use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use tds_protocol::crypto::{CekTableEntry, CryptoMetadata};

fuzz_target!(|data: &[u8]| {
    // Fuzz crypto metadata parsing (Always Encrypted)
    let mut bytes = Bytes::copy_from_slice(data);

    // Try parsing CryptoMetadata
    let _ = CryptoMetadata::decode(&mut bytes);

    // Also try parsing CEK table entries
    let mut bytes2 = Bytes::copy_from_slice(data);
    let _ = CekTableEntry::decode(&mut bytes2);
});
