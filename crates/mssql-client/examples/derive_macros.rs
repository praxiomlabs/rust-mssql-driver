//! Derive macros example.
//!
//! This example demonstrates the use of derive macros for:
//! - `FromRow`: Automatically map query results to structs
//! - `ToParams`: Automatically convert structs to query parameters
//! - `Tvp`: Create table-valued parameters from structs
//!
//! # Running
//!
//! ```bash
//! cargo run --example derive_macros
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Column, Config, Error, FromRow, Row, ToParams, Tvp, TvpValue};
use mssql_derive::{FromRow, ToParams, Tvp};
use mssql_types::SqlValue;

/// A user struct that can be populated from query results.
///
/// The `FromRow` derive generates `FromRow::from_row(&Row)` which
/// extracts values by column name. `Option<T>` fields handle NULLs.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct User {
    id: i32,
    name: String,
    #[mssql(rename = "email_address")]
    email: String,
    phone: Option<String>,
}

/// A struct for creating new users.
///
/// The `ToParams` derive generates `to_params() -> Vec<NamedParam>`.
/// Use `#[mssql(rename = "...")]` to control the SQL parameter name.
#[derive(Debug, ToParams)]
struct NewUser {
    name: String,
    #[mssql(rename = "email_address")]
    email: String,
    active: bool,
}

/// A table-valued parameter for batch operations.
///
/// The `Tvp` derive generates `type_name()`, `columns()`, and `to_row()`
/// methods. The `#[mssql(type_name = "...")]` attribute is required and
/// must match the SQL Server user-defined table type.
#[derive(Debug, Tvp)]
#[mssql(type_name = "dbo.UserIdList")]
struct UserIdParam {
    #[mssql(rename = "UserId")]
    user_id: i32,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    println!("=== FromRow Example ===\n");
    demonstrate_from_row();

    println!("\n=== ToParams Example ===\n");
    demonstrate_to_params();

    println!("\n=== TVP Example ===\n");
    demonstrate_tvp();

    // If you have a database connection, uncomment this:
    // database_example().await?;

    Ok(())
}

fn demonstrate_from_row() {
    // Build a mock Row using from_values (public since v0.9.0)
    let columns = vec![
        Column::new("id", 0, "INT"),
        Column::new("name", 1, "NVARCHAR"),
        Column::new("email_address", 2, "NVARCHAR"),
        Column::new("phone", 3, "NVARCHAR"),
    ];
    let values = vec![
        SqlValue::Int(42),
        SqlValue::String("Alice".into()),
        SqlValue::String("alice@example.com".into()),
        SqlValue::String("+1-555-1234".into()),
    ];
    let row = Row::from_values(columns, values);

    // FromRow::from_row extracts typed fields by column name
    let user = User::from_row(&row).expect("from_row should succeed");
    println!("Mapped user: {user:?}");

    // Also works with NULL columns (phone is Option<String>)
    let columns_null = vec![
        Column::new("id", 0, "INT"),
        Column::new("name", 1, "NVARCHAR"),
        Column::new("email_address", 2, "NVARCHAR"),
        Column::new("phone", 3, "NVARCHAR"),
    ];
    let values_null = vec![
        SqlValue::Int(99),
        SqlValue::String("Bob".into()),
        SqlValue::String("bob@example.com".into()),
        SqlValue::Null,
    ];
    let row_null = Row::from_values(columns_null, values_null);
    let user_null = User::from_row(&row_null).expect("from_row with NULL should succeed");
    println!("User with NULL phone: {user_null:?}");
}

fn demonstrate_to_params() {
    let new_user = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        active: true,
    };

    println!("NewUser struct: {new_user:?}");

    // The derived to_params() returns Vec<NamedParam>
    match new_user.to_params() {
        Ok(params) => {
            println!("\nGenerated parameters:");
            for param in &params {
                println!("  @{} = {:?}", param.name, param.value);
            }
        }
        Err(e) => println!("Error: {e}"),
    }

    println!("\nUsage:");
    println!("  client.execute_named(");
    println!("      \"INSERT INTO users (name, email_address, active) VALUES (@name, @email_address, @active)\",");
    println!("      &new_user.to_params()?");
    println!("  ).await?;");
}

fn demonstrate_tvp() {
    let user_ids = vec![
        UserIdParam { user_id: 1 },
        UserIdParam { user_id: 2 },
        UserIdParam { user_id: 3 },
    ];

    println!("UserIdParam list:");
    for param in &user_ids {
        println!("  user_id: {}", param.user_id);
    }

    // Derived Tvp methods
    println!("\nTVP type name: {}", UserIdParam::type_name());
    println!("TVP columns:");
    for col in UserIdParam::columns() {
        println!(
            "  {} ({}) at ordinal {}",
            col.name, col.sql_type, col.ordinal
        );
    }

    // TvpValue wraps a slice of Tvp-implementing structs for use as a parameter
    match TvpValue::new(&user_ids) {
        Ok(tvp) => println!("\nTvpValue created with {} rows", tvp.len()),
        Err(e) => println!("\nTvpValue error: {e}"),
    }

    println!("\nUsage:");
    println!("  // SQL Server type: CREATE TYPE dbo.UserIdList AS TABLE (UserId INT NOT NULL);");
    println!("  let tvp = TvpValue::new(&user_ids)?;");
    println!("  client.execute(\"EXEC GetUserDetails @UserIds\", &[&tvp]).await?;");
}

/// Example with actual database connection.
#[allow(dead_code)]
async fn database_example() -> Result<(), Error> {
    let conn_str = "Server=localhost;Database=master;User Id=sa;Password=Password123!;TrustServerCertificate=true";
    let config = Config::from_connection_string(conn_str)?;
    let mut client = Client::connect(config).await?;

    // Query and map rows using FromRow derive
    let rows = client
        .query(
            "SELECT 1 as id, 'Alice' as name, 'alice@example.com' as email_address, NULL as phone",
            &[],
        )
        .await?;

    for result in rows {
        let row = result?;
        let user = User::from_row(&row)?;
        println!("User: {user:?}");
    }

    // Insert using ToParams derive
    let new_user = NewUser {
        name: "Charlie".into(),
        email: "charlie@example.com".into(),
        active: true,
    };

    let params = new_user
        .to_params()
        .map_err(|e| Error::Config(e.to_string()))?;
    client
        .execute_named(
            "INSERT INTO users (name, email_address, active) VALUES (@name, @email_address, @active)",
            &params,
        )
        .await?;

    client.close().await?;

    Ok(())
}
