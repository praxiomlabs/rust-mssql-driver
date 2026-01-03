#![no_main]

use arbitrary::Arbitrary;
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use tds_protocol::rpc::{ParamFlags, ProcId, RpcOptionFlags, RpcParam, RpcRequest, TypeInfo};

/// Arbitrary RPC parameters for fuzzing encoding.
#[derive(Debug, Arbitrary)]
struct FuzzRpcInput {
    /// Procedure ID type (0 = named, 1-65535 = built-in)
    proc_id_type: u16,
    /// Procedure name for named procs
    proc_name: String,
    /// Number of parameters
    num_params: u8,
    /// Parameter data
    param_data: Vec<u8>,
    /// Option flags
    option_flags: u16,
}

fuzz_target!(|input: FuzzRpcInput| {
    // Test RPC request construction with arbitrary inputs
    let _proc_id = if input.proc_id_type == 0 {
        ProcId::Name(input.proc_name.clone())
    } else {
        ProcId::Id(input.proc_id_type)
    };

    // Test option flags parsing
    let _flags = RpcOptionFlags::from_bits_truncate(input.option_flags);

    // Test param flags
    for byte in &input.param_data {
        let _param_flags = ParamFlags::from_bits_truncate(*byte);
    }
});
