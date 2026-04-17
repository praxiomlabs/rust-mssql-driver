use mssql_derive::Tvp;

/// Tvp can only be derived for structs, not enums.
#[derive(Tvp)]
#[mssql(type_name = "dbo.SomeType")]
enum NotAStruct {
    A,
    B,
}

fn main() {}
