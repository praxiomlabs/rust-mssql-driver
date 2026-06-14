//! `begin_transaction` is only available on `Client<Ready>`: a disconnected
//! client has no session to start a transaction on.
use mssql_client::{Client, Disconnected};

fn use_begin(client: Client<Disconnected>) {
    let _ = client.begin_transaction();
}

fn main() {}
