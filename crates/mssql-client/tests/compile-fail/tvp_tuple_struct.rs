use mssql_derive::Tvp;

/// Tvp requires named fields, not tuple structs.
#[derive(Tvp)]
#[mssql(type_name = "dbo.SomeType")]
struct TupleStruct(i32, String);

fn main() {}
