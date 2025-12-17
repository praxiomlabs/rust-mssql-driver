# mssql-derive

Procedural macros for SQL Server row mapping and parameter handling.

## Overview

This crate provides derive macros for automatically implementing row-to-struct mapping and struct-to-parameter conversion, reducing boilerplate when working with SQL Server data.

## Available Macros

| Macro | Description |
|-------|-------------|
| `#[derive(FromRow)]` | Convert database rows to structs |
| `#[derive(ToParams)]` | Convert structs to query parameters |
| `#[derive(Tvp)]` | Table-valued parameter support |

## FromRow

Automatically implement row-to-struct conversion:

```rust
use mssql_derive::FromRow;

#[derive(FromRow)]
struct User {
    id: i32,
    #[mssql(rename = "user_name")]
    name: String,
    email: Option<String>,
}

// Usage
let user: User = row.into()?;
```

### Field Attributes

| Attribute | Description |
|-----------|-------------|
| `#[mssql(rename = "column")]` | Map field to different column name |
| `#[mssql(skip)]` | Skip field (must implement Default) |
| `#[mssql(default)]` | Use Default if NULL or missing |
| `#[mssql(flatten)]` | Flatten nested FromRow struct |

### Struct Attributes

| Attribute | Description |
|-----------|-------------|
| `#[mssql(rename_all = "case")]` | Apply naming convention to all fields |

Supported cases: `snake_case`, `camelCase`, `PascalCase`, `SCREAMING_SNAKE_CASE`

## ToParams

Automatically convert structs to query parameters:

```rust
use mssql_derive::ToParams;

#[derive(ToParams)]
struct NewUser {
    name: String,
    #[mssql(rename = "email_address")]
    email: String,
    #[mssql(skip)]
    internal_id: u64,
}

let user = NewUser {
    name: "Alice".into(),
    email: "alice@example.com".into(),
    internal_id: 0,
};

client.execute(
    "INSERT INTO users (name, email_address) VALUES (@name, @email_address)",
    &user.to_params()?,
).await?;
```

## Tvp (Table-Valued Parameters)

Create table-valued parameters for passing collections to stored procedures:

```rust
use mssql_derive::Tvp;

// First, create the table type in SQL Server:
// CREATE TYPE dbo.UserIdList AS TABLE (UserId INT NOT NULL);

#[derive(Tvp)]
#[mssql(type_name = "dbo.UserIdList")]
struct UserId {
    #[mssql(rename = "UserId")]
    user_id: i32,
}

let ids = vec![UserId { user_id: 1 }, UserId { user_id: 2 }];
let tvp = TvpValue::new(&ids)?;

client.execute(
    "SELECT * FROM users WHERE id IN (SELECT UserId FROM @ids)",
    &[&tvp],
).await?;
```

## Complete Example

```rust
use mssql_derive::{FromRow, ToParams};

#[derive(FromRow)]
#[mssql(rename_all = "PascalCase")]
struct User {
    id: i32,
    #[mssql(rename = "UserName")]
    name: String,
    #[mssql(default)]
    email: Option<String>,
    #[mssql(skip)]
    computed: String,
}

#[derive(ToParams)]
struct UpdateUser {
    #[mssql(rename = "UserId")]
    id: i32,
    name: String,
}

// Read users
let mut stream = client.query("SELECT * FROM Users", &[]).await?;
while let Some(row) = stream.next().await {
    let user: User = row?.into()?;
    println!("{}: {}", user.id, user.name);
}

// Update user
let update = UpdateUser { id: 1, name: "Bob".into() };
client.execute(
    "UPDATE Users SET UserName = @name WHERE Id = @UserId",
    &update.to_params()?,
).await?;
```

## Type Inference for TVPs

The macro automatically infers SQL types from Rust types:

| Rust Type | SQL Type |
|-----------|----------|
| `i8`, `u8` | `TINYINT` |
| `i16` | `SMALLINT` |
| `i32` | `INT` |
| `i64` | `BIGINT` |
| `f32` | `REAL` |
| `f64` | `FLOAT` |
| `bool` | `BIT` |
| `String` | `NVARCHAR(MAX)` |
| `Uuid` | `UNIQUEIDENTIFIER` |
| `NaiveDate` | `DATE` |
| `NaiveTime` | `TIME` |
| `NaiveDateTime` | `DATETIME2` |
| `DateTime` | `DATETIMEOFFSET` |
| `Decimal` | `DECIMAL(38,10)` |

## License

MIT OR Apache-2.0
