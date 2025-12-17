#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use bytes::Bytes;

/// Arbitrary type info for fuzzing.
#[derive(Debug, Arbitrary)]
struct FuzzTypeInfo {
    type_id: u8,
    length: Option<u32>,
    scale: Option<u8>,
    precision: Option<u8>,
}

/// Fuzz input combining type info with raw bytes.
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    type_info: FuzzTypeInfo,
    data: Vec<u8>,
}

fuzz_target!(|input: FuzzInput| {
    // Convert to the real TypeInfo
    let type_info = mssql_types::decode::TypeInfo {
        type_id: input.type_info.type_id,
        length: input.type_info.length,
        scale: input.type_info.scale,
        precision: input.type_info.precision,
        collation: None,
    };

    // Try to decode the value
    let mut bytes = Bytes::from(input.data);
    let _ = mssql_types::decode::decode_value(&mut bytes, &type_info);
});
