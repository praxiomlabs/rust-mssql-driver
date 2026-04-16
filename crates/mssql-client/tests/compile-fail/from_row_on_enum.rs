use mssql_derive::FromRow;

/// FromRow can only be derived for structs, not enums.
#[derive(FromRow)]
enum NotAStruct {
    A,
    B,
}

fn main() {}
