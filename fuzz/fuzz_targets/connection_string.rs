#![no_main]

use libfuzzer_sys::fuzz_target;
use mssql_client::Config;

fuzz_target!(|data: &[u8]| {
    // Fuzz connection string parsing
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Config::from_connection_string(s);
    }
});
