# Stored Procedures

This guide covers stored procedure support in `mssql-client`.

## Quick Start

### Simple Call (Input Parameters Only)

Use `call_procedure()` for the common case of calling a procedure with positional input parameters:

```rust,ignore
let result = client.call_procedure("dbo.GetUser", &[&1i32]).await?;

// Check the return value (from the proc's RETURN statement)
assert_eq!(result.return_value, 0);

// Access result sets
for mut rs in result.result_sets {
    while let Some(row) = rs.next_row() {
        let name: String = row.get_by_name("name")?;
        println!("User: {name}");
    }
}
```

### Builder (Named Parameters, OUTPUT Parameters)

Use `procedure()` when you need named parameters or output parameters:

```rust,ignore
let result = client.procedure("dbo.CalculateSum")?
    .input("@a", &10i32)
    .input("@b", &20i32)
    .output_int("@result")
    .execute().await?;

// Access output parameter (case-insensitive, @-prefix tolerant)
let output = result.get_output("@result").expect("output param");
// output.value is a SqlValue::Int(30)
```

## API Reference

### `call_procedure(proc_name, params)`

- **proc_name**: Schema-qualified procedure name (e.g., `"dbo.MyProc"`, `"MyProc"`)
- **params**: Positional input parameters as `&[&(dyn ToSql + Sync)]`
- **Returns**: `Result<ProcedureResult>`

Parameters are auto-named `@p1`, `@p2`, etc. (matching SQL Server's positional RPC parameter convention).

### `procedure(proc_name)`

- **proc_name**: Schema-qualified procedure name
- **Returns**: `Result<ProcedureBuilder>`

The builder supports chained calls:

| Method | Description |
|--------|-------------|
| `.input(name, value)` | Add a named input parameter |
| `.output_int(name)` | Declare an INT output parameter |
| `.output_bigint(name)` | Declare a BIGINT output parameter |
| `.output_nvarchar(name, max_len)` | Declare an NVARCHAR output parameter (`0` for MAX) |
| `.output_bit(name)` | Declare a BIT output parameter |
| `.output_float(name)` | Declare a FLOAT (64-bit) output parameter |
| `.output_decimal(name, precision, scale)` | Declare a DECIMAL output parameter |
| `.output_raw(name, type_info)` | Escape hatch for uncommon types |
| `.execute()` | Execute and return `ProcedureResult` |

### `ProcedureResult`

| Field | Type | Description |
|-------|------|-------------|
| `return_value` | `i32` | Value from the procedure's `RETURN` statement (defaults to 0) |
| `rows_affected` | `u64` | Total rows affected by statements in the procedure |
| `output_params` | `Vec<OutputParam>` | Output parameters with names and values |
| `result_sets` | `Vec<ResultSet>` | Result sets from SELECT statements in the procedure |

#### Methods

- `get_return_value()` - Returns the procedure's return value
- `get_output(name)` - Find an output parameter by name (case-insensitive, strips `@` prefix)
- `first_result_set()` - Get the first result set if any
- `has_result_sets()` - Check if any result sets were produced

## Transaction Support

Both `call_procedure()` and `procedure()` work in transactions:

```rust,ignore
let mut tx = client.begin_transaction().await?;

let result = tx.call_procedure("dbo.InsertOrder", &[&customer_id, &total]).await?;
assert_eq!(result.return_value, 0);

let client = tx.commit().await?;
```

## Output Parameter Types

Output parameters are declared with a type that tells SQL Server what type to return. The server fills in the value and sends it back as a `ReturnValue` token.

```rust,ignore
// Common output types
builder.output_int("@count")                   // INT
builder.output_bigint("@total")                // BIGINT
builder.output_nvarchar("@message", 200)       // NVARCHAR(200)
builder.output_nvarchar("@data", 0)            // NVARCHAR(MAX)
builder.output_bit("@success")                 // BIT
builder.output_float("@average")               // FLOAT
builder.output_decimal("@amount", 18, 2)       // DECIMAL(18,2)

// For uncommon types, use output_raw with a TypeInfo
use tds_protocol::rpc::TypeInfo;
builder.output_raw("@guid", TypeInfo::uniqueidentifier())
```

## Security

All procedure names are validated before being sent to the server. Names must:

- Start with a letter or underscore
- Contain only alphanumerics, `_`, `@`, `#`, `$`
- Be 1-128 characters per part
- Have at most 4 dot-separated parts (server.catalog.schema.object)

Parameter values are sent as typed TDS RPC parameters and are never interpolated into SQL text, preventing SQL injection.

## Error Handling

```rust,ignore
// Nonexistent procedure
let result = client.call_procedure("dbo.NoSuchProc", &[]).await;
assert!(result.is_err()); // Server error: "Could not find stored procedure"

// Invalid procedure name (caught before any network call)
let result = client.procedure("invalid;name");
assert!(result.is_err()); // InvalidIdentifier error
```

## How It Works

Under the hood, stored procedure calls use TDS RPC (Remote Procedure Call) requests via `RpcRequest::named()`. This is the same protocol mechanism used by `sp_executesql` for parameterized queries, but it directly invokes the named procedure without SQL text parsing. The server responds with a token stream containing result sets, output parameter values (`ReturnValue` tokens), the procedure's return value (`ReturnStatus` token), and completion status (`DoneProc` token).
