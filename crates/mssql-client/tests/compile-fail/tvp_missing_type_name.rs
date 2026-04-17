use mssql_derive::Tvp;

/// Tvp derive requires `#[mssql(type_name = "...")]` on the struct.
/// Omitting it should produce a compile error.
#[derive(Tvp)]
struct MissingTypeName {
    id: i32,
}

fn main() {}
