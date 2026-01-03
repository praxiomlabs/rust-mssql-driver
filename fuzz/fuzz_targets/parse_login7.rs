#![no_main]

use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use tds_protocol::login7::Login7Response;

fuzz_target!(|data: &[u8]| {
    // Fuzz Login7 response parsing
    let mut bytes = Bytes::copy_from_slice(data);
    let _ = Login7Response::decode(&mut bytes);
});
