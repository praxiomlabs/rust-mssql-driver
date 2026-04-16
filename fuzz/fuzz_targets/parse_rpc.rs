#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tds_protocol::rpc::{RpcParam, RpcRequest};

/// Arbitrary RPC parameters for fuzzing encoding.
#[derive(Debug, Arbitrary)]
struct FuzzRpcInput {
    /// Whether to use a named procedure (true) or sp_executesql (false)
    use_named: bool,
    /// Procedure name for named procs
    proc_name: String,
    /// SQL for sp_executesql path
    sql: String,
    /// Parameter values to encode
    params: Vec<FuzzParam>,
}

#[derive(Debug, Arbitrary)]
struct FuzzParam {
    name: String,
    kind: u8,
    int_val: i32,
    bigint_val: i64,
    str_val: String,
}

fuzz_target!(|input: FuzzRpcInput| {
    // Build parameters from arbitrary input
    let mut rpc_params = Vec::new();
    for p in &input.params {
        let param = match p.kind % 4 {
            0 => RpcParam::int(&p.name, p.int_val),
            1 => RpcParam::bigint(&p.name, p.bigint_val),
            2 => RpcParam::nvarchar(&p.name, &p.str_val),
            3 => RpcParam::varchar(&p.name, &p.str_val),
            _ => unreachable!(),
        };
        rpc_params.push(param);
    }

    // Test encoding with either named procedure or sp_executesql
    let rpc = if input.use_named {
        let mut req = RpcRequest::named(&input.proc_name);
        for param in rpc_params {
            req = req.param(param);
        }
        req
    } else {
        RpcRequest::execute_sql(&input.sql, rpc_params)
    };

    // Encode the request — should never panic
    let _encoded = rpc.encode();
});
