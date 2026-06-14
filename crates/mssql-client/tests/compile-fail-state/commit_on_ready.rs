//! `commit` is only available on `Client<InTransaction>`: a ready (not
//! in-transaction) client cannot commit.
use mssql_client::{Client, Ready};

fn use_commit(client: Client<Ready>) {
    let _ = client.commit();
}

fn main() {}
