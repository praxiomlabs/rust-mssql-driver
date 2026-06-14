//! `query` is only available on `Client<Ready>` / `Client<InTransaction>`,
//! never on a disconnected client.
use mssql_client::{Client, Disconnected};

fn use_query(client: &mut Client<Disconnected>) {
    let _ = client.query("SELECT 1", &[]);
}

fn main() {}
