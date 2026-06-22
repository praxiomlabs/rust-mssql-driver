#![no_main]

//! Fuzz the REAL client row-parse path (`mssql_client::column_parser`), the
//! code that decodes wire bytes from the server during normal queries. This
//! is distinct from `decode_value`, which fuzzes the parallel decoder in
//! `mssql-types`. A panic here means a malicious or buggy server can crash
//! the client (the bug class fixed alongside this target's introduction).

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tds_protocol::token::{ColumnData, TypeInfo};
use tds_protocol::types::TypeId;

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    type_byte: u8,
    max_length: Option<u32>,
    precision: Option<u8>,
    scale: Option<u8>,
    data: Vec<u8>,
}

fuzz_target!(|input: FuzzInput| {
    let Some(type_id) = TypeId::from_u8(input.type_byte) else {
        return;
    };
    let col = ColumnData {
        name: String::new(),
        type_id,
        col_type: input.type_byte,
        flags: 0,
        user_type: 0,
        type_info: TypeInfo {
            max_length: input.max_length,
            precision: input.precision,
            scale: input.scale,
            collation: None,
        },
        crypto_metadata: None,
    };
    let mut buf: &[u8] = &input.data;
    // Any Ok/Err is fine; panics are findings. `None` = copy path: the fuzz
    // input is a bare slice, not a tail of a backing `Bytes`.
    let _ = mssql_client::__fuzzing::parse_column_value(&mut buf, &col, None);
});
