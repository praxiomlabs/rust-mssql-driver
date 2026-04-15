# Derive Macros

`mssql-derive` provides three derive macros for reducing boilerplate when working with SQL Server data.

## `#[derive(FromRow)]`

Maps a SQL Server result row to a Rust struct.

```rust
use mssql_client::FromRow;

#[derive(FromRow)]
struct User {
    id: i32,
    name: String,
    email: Option<String>,
}

let rows = client.query("SELECT id, name, email FROM users", &[]).await?;
for row in rows.iter() {
    let user = User::from_row(row)?;
}
```

### Struct-Level Attributes

| Attribute | Description |
|---|---|
| `#[mssql(rename_all = "...")]` | Apply a naming convention to all fields |

Supported `rename_all` values: `snake_case`, `camelCase`, `PascalCase`, `SCREAMING_SNAKE_CASE`.

```rust
#[derive(FromRow)]
#[mssql(rename_all = "PascalCase")]
struct User {
    user_id: i32,    // matches column "UserId"
    full_name: String, // matches column "FullName"
}
```

### Field-Level Attributes

| Attribute | Description |
|---|---|
| `#[mssql(rename = "column_name")]` | Map to a specific column name |
| `#[mssql(skip)]` | Skip this field (must implement `Default`) |
| `#[mssql(default)]` | Use `Default::default()` if the column is NULL or missing |
| `#[mssql(flatten)]` | Recursively apply `FromRow` on this field using the same row |

```rust
#[derive(FromRow)]
struct Order {
    #[mssql(rename = "OrderID")]
    id: i32,

    #[mssql(skip)]
    computed_total: f64,  // not from the database, defaults to 0.0

    #[mssql(default)]
    notes: String,  // empty string if NULL

    #[mssql(flatten)]
    audit: AuditInfo,  // reads created_at, updated_at from the same row
}

#[derive(FromRow)]
struct AuditInfo {
    created_at: chrono::NaiveDateTime,
    updated_at: Option<chrono::NaiveDateTime>,
}
```

### Option Handling

Fields typed as `Option<T>` automatically return `None` for NULL values. Non-`Option` fields return an error if the column is NULL (unless `#[mssql(default)]` is set).

---

## `#[derive(ToParams)]`

Converts a struct into named query parameters.

```rust
use mssql_client::ToParams;

#[derive(ToParams)]
struct CreateUser {
    name: String,
    email: String,
    age: i32,
}

let params = CreateUser {
    name: "Alice".into(),
    email: "alice@example.com".into(),
    age: 30,
};

let named = params.to_params()?;
// Produces: [("name", "Alice"), ("email", "alice@example.com"), ("age", 30)]
```

### Struct-Level Attributes

| Attribute | Description |
|---|---|
| `#[mssql(rename_all = "...")]` | Apply a naming convention to all parameter names |

### Field-Level Attributes

| Attribute | Description |
|---|---|
| `#[mssql(rename = "param_name")]` | Override the parameter name |
| `#[mssql(skip)]` | Exclude this field from parameters |

```rust
#[derive(ToParams)]
#[mssql(rename_all = "PascalCase")]
struct UpdateUser {
    #[mssql(rename = "@UserID")]
    id: i32,

    #[mssql(skip)]
    original_name: String,  // not sent as a parameter

    new_name: String,  // sent as "NewName"
}
```

> **Note:** `ToParams` generates `Vec<NamedParam>`. A future release will add `Client::execute_named()` to accept named parameters directly.

---

## `#[derive(Tvp)]`

Defines a struct as a Table-Valued Parameter row type.

```rust
use mssql_client::Tvp;

#[derive(Tvp)]
#[mssql(type_name = "dbo.UserTableType")]
struct UserRow {
    id: i32,
    name: String,
    active: bool,
}
```

### Required Struct-Level Attribute

| Attribute | Description |
|---|---|
| `#[mssql(type_name = "schema.TypeName")]` | **Required.** The SQL Server table type name |

The `type_name` must match a table type that exists on the server:

```sql
CREATE TYPE dbo.UserTableType AS TABLE (
    id INT,
    name NVARCHAR(MAX),
    active BIT
);
```

### Field-Level Attributes

| Attribute | Description |
|---|---|
| `#[mssql(rename = "column_name")]` | Override the TVP column name |
| `#[mssql(skip)]` | Exclude this field from the TVP |

### SQL Type Inference

The derive macro infers the SQL type from the Rust type:

| Rust Type | SQL Type |
|---|---|
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
| `Vec<u8>` | `VARBINARY(MAX)` |
| `Option<T>` | Same as `T` (nullable) |

`Option<T>` unwraps to the inner type's SQL mapping. For example, `Option<i32>` maps to `INT`, not `NVARCHAR(MAX)`.
