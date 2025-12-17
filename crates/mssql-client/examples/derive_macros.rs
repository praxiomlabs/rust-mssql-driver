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
//!
//! Note: This example requires the mssql-derive crate to be available.
//! The derive macros generate code at compile time.

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config, Error, ToParams, Tvp, TvpColumn};
use mssql_types::{SqlValue, ToSql, TypeError};

/// A user struct that can be populated from query results.
///
/// The `FromRow` derive macro generates code to extract values
/// from a Row by column name or index.
///
/// With the derive macro, you would write:
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct User { ... }
/// ```
#[derive(Debug)]
#[allow(dead_code)] // Fields are used in documentation and database_example
struct User {
    /// Maps to column "id" by default
    id: i32,

    /// Maps to column "name" by default
    name: String,

    /// Use `rename` to map to a different column name
    email: String,

    /// Optional fields handle NULL values gracefully
    phone: Option<String>,
}

/// A struct for creating new users.
///
/// The `ToParams` derive macro generates code to convert
/// the struct fields into named query parameters.
///
/// With the derive macro, you would write:
/// ```rust,ignore
/// #[derive(ToParams)]
/// struct NewUser { ... }
/// ```
#[derive(Debug)]
struct NewUser {
    /// Will be parameter @name
    name: String,

    /// Will be parameter @email_address (with rename attribute)
    email: String,

    /// Will be parameter @active
    active: bool,
}

// Manual implementation of ToParams to demonstrate the pattern
impl ToParams for NewUser {
    fn to_params(&self) -> Result<Vec<mssql_client::NamedParam>, TypeError> {
        Ok(vec![
            mssql_client::NamedParam::new("name", self.name.to_sql()?),
            mssql_client::NamedParam::new("email_address", self.email.to_sql()?),
            mssql_client::NamedParam::new("active", self.active.to_sql()?),
        ])
    }
}

/// A table-valued parameter for batch operations.
///
/// The `Tvp` derive macro generates code to create TVP rows
/// from struct instances.
///
/// With the derive macro, you would write:
/// ```rust,ignore
/// #[derive(Tvp)]
/// #[mssql(type_name = "dbo.UserIdList")]
/// struct UserIdParam { ... }
/// ```
#[derive(Debug)]
struct UserIdParam {
    /// Maps to column "UserId" in the TVP
    user_id: i32,
}

// Manual implementation of Tvp to demonstrate the pattern
impl Tvp for UserIdParam {
    fn type_name() -> &'static str {
        "dbo.UserIdList"
    }

    fn columns() -> Vec<TvpColumn> {
        vec![TvpColumn::new("UserId", "INT", 0)]
    }

    fn to_row(&self) -> Result<mssql_client::TvpRow, TypeError> {
        Ok(mssql_client::TvpRow::new(vec![self.user_id.to_sql()?]))
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    // Demonstrate the derive macros without a database connection
    // (The actual database operations would require a running SQL Server)

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
    // Create a mock row with sample data
    // In practice, this comes from query results
    let _values = [
        SqlValue::Int(42),
        SqlValue::String("Alice".into()),
        SqlValue::String("alice@example.com".into()),
        SqlValue::String("+1-555-1234".into()),
    ];

    println!("Mock row data:");
    println!("  id: 42");
    println!("  name: 'Alice'");
    println!("  email_address: 'alice@example.com'");
    println!("  phone: '+1-555-1234'");

    // In a real application with the derive macro, you would use:
    // let user: User = row.map_to()?;
    // or
    // let users: Vec<User> = rows.map_rows::<User>().collect();

    println!("\nWith FromRow derive, you can do:");
    println!("  let user: User = row.map_to()?;");
    println!("  // user.id = 42");
    println!("  // user.name = \"Alice\"");
    println!("  // user.email = \"alice@example.com\"");
    println!("  // user.phone = Some(\"+1-555-1234\")");
}

fn demonstrate_to_params() {
    // Create a struct to convert to parameters
    let new_user = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        active: true,
    };

    println!("NewUser struct:");
    println!("  name: '{}'", new_user.name);
    println!("  email: '{}'", new_user.email);
    println!("  active: {}", new_user.active);

    // Convert to parameters (this uses our manual ToParams implementation)
    match new_user.to_params() {
        Ok(params) => {
            println!("\nGenerated parameters:");
            for param in &params {
                println!("  @{} = {:?}", param.name, param.value);
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    println!("\nUsage in query:");
    println!("  client.execute(");
    println!(
        "      \"INSERT INTO users (name, email_address, active) VALUES (@name, @email_address, @active)\","
    );
    println!("      &new_user.to_params()?");
    println!("  ).await?;");
}

fn demonstrate_tvp() {
    // Create a list of user IDs for a batch operation
    let user_ids = vec![
        UserIdParam { user_id: 1 },
        UserIdParam { user_id: 2 },
        UserIdParam { user_id: 3 },
    ];

    println!("UserIdParam list:");
    for param in &user_ids {
        println!("  user_id: {}", param.user_id);
    }

    println!("\nTVP type name: {}", UserIdParam::type_name());
    println!("TVP columns:");
    for col in UserIdParam::columns() {
        println!(
            "  {} ({}) at ordinal {}",
            col.name, col.sql_type, col.ordinal
        );
    }

    println!("\nUsage with stored procedure:");
    println!("  // First, create the type in SQL Server:");
    println!("  // CREATE TYPE dbo.UserIdList AS TABLE (UserId INT NOT NULL);");
    println!();
    println!("  // Then use it in Rust:");
    println!("  let tvp = TvpValue::new(&user_ids)?;");
    println!("  client.execute(");
    println!("      \"EXEC GetUserDetails @UserIds\",");
    println!("      &[&tvp]");
    println!("  ).await?;");
}

/// Example with actual database connection
#[allow(dead_code)]
async fn database_example() -> Result<(), Error> {
    let conn_str = "Server=localhost;Database=master;User Id=sa;Password=Password123!;TrustServerCertificate=true";
    let config = Config::from_connection_string(conn_str)?;
    let mut client = Client::connect(config).await?;

    // Query and manually map rows to User struct
    let rows = client
        .query(
            "SELECT 1 as id, 'Alice' as name, 'alice@example.com' as email_address, NULL as phone",
            &[],
        )
        .await?;

    // Process rows
    for result in rows {
        let row = result?;
        let user = User {
            id: row.get(0)?,
            name: row.get(1)?,
            email: row.get(2)?,
            phone: row.try_get(3),
        };
        println!("User: {:?}", user);
    }

    // Using ToParams with insert
    let new_user = NewUser {
        name: "Charlie".into(),
        email: "charlie@example.com".into(),
        active: true,
    };

    // Get the parameters (returns Result, so handle it)
    let params = new_user
        .to_params()
        .map_err(|e| Error::Config(e.to_string()))?;
    println!("Generated {} parameters", params.len());

    // Note: The execute method expects &[&dyn ToSql], not Vec<NamedParam>
    // In a full implementation, you'd have a method that accepts named params

    client.close().await?;

    Ok(())
}
