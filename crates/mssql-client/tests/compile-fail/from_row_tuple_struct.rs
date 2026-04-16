use mssql_derive::FromRow;

/// FromRow requires named fields, not tuple structs.
#[derive(FromRow)]
struct TupleStruct(i32, String);

fn main() {}
