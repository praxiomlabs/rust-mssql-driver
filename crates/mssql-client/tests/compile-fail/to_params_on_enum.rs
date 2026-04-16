use mssql_derive::ToParams;

/// ToParams can only be derived for structs, not enums.
#[derive(ToParams)]
enum NotAStruct {
    A,
    B,
}

fn main() {}
