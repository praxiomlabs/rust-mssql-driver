//! Runtime tests for the `#[derive(FromRow)]` and `#[derive(ToParams)]` macros.
//!
//! The compile-fail suite proves the macros reject bad input, but nothing
//! exercised the *generated* code. These build a struct via each derive and run
//! the generated `from_row` / `to_params` against real values.
#![allow(clippy::unwrap_used)]

use mssql_client::{Column, FromRow, Row, SqlValue, ToParams};

#[derive(FromRow)]
struct User {
    id: i32,
    name: String,
}

#[test]
fn derived_from_row_maps_columns_by_name() {
    let columns = vec![
        Column::new("id", 0, "INT".to_string()),
        Column::new("name", 1, "NVARCHAR".to_string()),
    ];
    let row = Row::from_values(
        columns,
        vec![SqlValue::Int(7), SqlValue::String("Ada".to_string())],
    );

    let user = User::from_row(&row).unwrap();
    assert_eq!(user.id, 7);
    assert_eq!(user.name, "Ada");
}

#[derive(ToParams)]
struct Filter {
    min_id: i32,
    name: String,
}

#[test]
fn derived_to_params_emits_named_params() {
    let f = Filter {
        min_id: 10,
        name: "Ada".to_string(),
    };

    let params = f.to_params().unwrap();

    // One NamedParam per field, named after the field (no @ prefix), with the
    // field's value converted to a SqlValue.
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].name, "min_id");
    assert_eq!(params[0].value, SqlValue::Int(10));
    assert_eq!(params[1].name, "name");
    assert_eq!(params[1].value, SqlValue::String("Ada".to_string()));
}
