#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tds_protocol::login7::Login7;
use tds_protocol::version::TdsVersion;

/// Arbitrary Login7 inputs for fuzzing encoding.
#[derive(Debug, Arbitrary)]
struct FuzzLogin7Input {
    username: String,
    password: String,
    database: String,
    hostname: String,
    app_name: String,
    server_name: String,
    language: String,
    packet_size: u32,
    read_only: bool,
    tds_version: u8,
}

fuzz_target!(|input: FuzzLogin7Input| {
    let version = match input.tds_version % 4 {
        0 => TdsVersion::V7_3A,
        1 => TdsVersion::V7_3B,
        2 => TdsVersion::V7_4,
        3 => TdsVersion::V8_0,
        _ => unreachable!(),
    };

    let login = Login7::new()
        .with_tds_version(version)
        .with_sql_auth(&input.username, &input.password)
        .with_database(&input.database)
        .with_hostname(&input.hostname)
        .with_app_name(&input.app_name)
        .with_server_name(&input.server_name)
        .with_language(&input.language)
        .with_packet_size(input.packet_size)
        .with_read_only_intent(input.read_only);

    // Encode the Login7 packet — should never panic
    let _encoded = login.encode();
});
