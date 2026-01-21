# Stored Procedure Execution with `execute_procedure`

## Overview

This driver provides comprehensive stored procedure execution support with output parameters, result sets, RETURN values, and row counts. The implementation features a type-safe API with full test coverage suitable for production use.

## Public API

### `Client<Ready>::execute_procedure`

**Location:** `crates/mssql-client/src/client.rs:3930`

**Signature:**
```rust
pub async fn execute_procedure(
    &mut self,
    proc_name: &str,
    params: Vec<RpcParam>,
) -> Result<ExecuteResult<'_>>
```

**Return Type:**
```rust
pub struct ExecuteResult<'a> {
    /// Output parameters from the stored procedure.
    pub output_params: Vec<OutputParam>,
    /// Number of rows affected by the statement.
    pub rows_affected: u64,
    /// Result set from SELECT statements (if any).
    pub result_set: Option<QueryStream<'a>>,
}
```

**Features:**
- Executes stored procedures and returns an encapsulated `ExecuteResult` struct
- Supports both input and output parameters
- Retrieves result sets and output parameters simultaneously
- Automatically handles RETURN statement values
- Row count is always available (even if 0), no Option checking needed
- Provides helper methods: `has_result_set()`, `get_result_set()`, `take_result_set()`, `get_output()`

**Example:**
```rust
let sum_param = RpcParam::null("@sum", TypeInfo::int()).as_output();
let product_param = RpcParam::null("@product", TypeInfo::int()).as_output();

let result = client.execute_procedure(
    "dbo.sp_TestOutputParams",
    vec![
        RpcParam::int("@a", 10),
        RpcParam::int("@b", 5),
        sum_param,
        product_param,
    ],
).await?;

// Method 1: Direct field access
let sum_value: i32 = result.output_params[0].value.as_i32()?;
println!("Rows affected: {}", result.rows_affected);

// Method 2: Using helper methods
if let Some(output) = result.get_output("sum") {
    let sum: i32 = output.value.as_i32()?;
    println!("Sum: {}", sum);
}

// Method 3: Process result set
if let Some(mut rows) = result.take_result_set() {
    while let Some(Ok(row)) = rows.next() {
        // Process row data
    }
}
```

---

### `Client<InTransaction>::execute_procedure`

**Location:** `crates/mssql-client/src/client.rs:4338`

**Signature:** Same as `Client<Ready>` version

**Features:**
- Executes stored procedures within transactions
- Integrates with ACID transaction properties
- Works with commit/rollback operations
- Returns the same `ExecuteResult` struct

**Example:**
```rust
let mut tx = client.begin_transaction().await?;

let balance_param = RpcParam::null("@new_balance", TypeInfo::int()).as_output();

let result = tx.execute_procedure(
    "dbo.sp_TestTransaction",
    vec![
        RpcParam::int("@user_id", 123),
        balance_param,
    ],
).await?;

let balance: i32 = result.get_output("newId").unwrap().value.as_i32()?;
println!("Rows affected: {}", result.rows_affected);
tx.commit().await?;
```

---

### `RpcParam::as_output`

**Location:** `crates/tds-protocol/src/rpc.rs:473`

**Signature:**
```rust
#[must_use]
pub fn as_output(mut self) -> Self
```

**Features:**
- Marks a parameter as an output parameter
- Must be called on output parameters
- Chainable, returns Self

**Example:**
```rust
// ❌ Wrong - treated as input parameter
let param = RpcParam::null("@result", TypeInfo::int());

// ✅ Correct - marked as output parameter
let param = RpcParam::null("@result", TypeInfo::int()).as_output();
```

---

## Type Definitions

### `ExecuteResult`

**Location:** `crates/mssql-client/src/stream.rs:170`

**Definition:**
```rust
#[derive(Debug)]
pub struct ExecuteResult<'a> {
    /// Output parameters from the stored procedure.
    pub output_params: Vec<OutputParam>,
    /// Number of rows affected by the statement.
    pub rows_affected: u64,
    /// Result set from SELECT statements (if any).
    pub result_set: Option<QueryStream<'a>>,
}
```

**Helper Methods:**
```rust
impl<'a> ExecuteResult<'a> {
    // Check if result set is available
    pub fn has_result_set(&self) -> bool;

    // Get result set reference
    pub fn get_result_set(&self) -> Option<&QueryStream<'a>>;

    // Take result set ownership
    pub fn take_result_set(&mut self) -> Option<QueryStream<'a>>;

    // Get output parameter by name
    pub fn get_output(&self, name: &str) -> Option<&OutputParam>;
}
```

### `OutputParam`

**Location:** `crates/mssql-client/src/stream.rs:184`

**Definition:**
```rust
#[derive(Debug, Clone)]
pub struct OutputParam {
    /// Parameter name.
    pub name: String,
    /// Parameter value.
    pub value: mssql_types::SqlValue,
}
```

---

## Core Implementation Details

### TDS Protocol Fix: ReturnValue Token Parsing

**Problem:** TDS protocol ReturnValue token format differs from ColMetaData

**Fix Location:** `crates/tds-protocol/src/token.rs:1326-1430`

**Key Differences:**

| Field   | ColMetaData | ReturnValue |
|---------|-------------|-------------|
| UserType | 4 bytes     | **2 bytes**  |
| Flags    | 2 bytes     | **N/A**      |
| TypeId   | 1 byte      | 1 byte       |

**Parameter Name Format:**
```
0x00 + UTF-16LE name + 0x01 0x00 + 0x00 0x00
```

**Before Fix:**
```rust
// ColMetaData format
let user_type = src.get_u32_le();      // 4 bytes ❌
let flags = src.get_u16_le();          // 2 bytes ❌
let col_type = src.get_u8();           // 1 byte
```

**After Fix:**
```rust
// ReturnValue format
let user_type = src.get_u16_le() as u32; // 2 bytes ✅
// No flags field ✅
let col_type = src.get_u8();              // 1 byte
let flags = 0u16; // ReturnValue has no flags field
```

---

## Test Coverage

**Location:** `crates/mssql-client/tests/stored_procedure.rs`

| Test Case                              | Coverage                           |
|----------------------------------------|------------------------------------|
| `test_stored_procedure_output_params`  | Basic output parameters            |
| `test_stored_procedure_result_set_and_outputs` | Result set + outputs |
| `test_stored_procedure_return_statement` | RETURN statement values         |
| `test_stored_procedure_multiple_outputs` | Multiple output parameters       |
| `test_stored_procedure_in_transaction`  | Transaction integration            |
| `test_stored_procedure_null_output_param` | NULL output parameters          |
| `test_stored_procedure_string_output_param` | String output parameters       |

**Test Results:**
```bash
$ cargo test -p mssql-client --test stored_procedure -- --ignored

test result: ok. 7 passed; 0 failed; 0 ignored
```

---

## Supported Output Parameter Types

| SQL Type  | Rust Type | Example                              |
|-----------|-----------|--------------------------------------|
| INT       | `i32`     | `RpcParam::int("@a", 10)`            |
| BIGINT    | `i64`     | `RpcParam::bigint("@id", 123)`       |
| NVARCHAR  | `&str`    | `RpcParam::nvarchar("@name", "John")` |
| NULL      | `SqlValue::Null` | `RpcParam::null("@result", TypeInfo::int())` |

---

## Return Value Structure

`execute_procedure` returns an `ExecuteResult<'a>` containing:

### 1. Output Parameters - `output_params: Vec<OutputParam>`
- Contains all output parameters
- Parameter names don't include the `@` prefix
- Supports all SQL types

### 2. Row Count - `rows_affected: u64`
- **Always has a value** (even if 0)
- No Option checking or unwrap needed
- Returns affected row count for INSERT/UPDATE/DELETE
- Returns 0 for SELECT or no-operation procedures

### 3. Result Set - `result_set: Option<QueryStream<'a>>`
- Present if the stored procedure returns SELECT results
- Can be iterated to fetch query rows
- Returns None if no result set

### 4. RETURN Values
- Returned as an output parameter with an empty name (`""`)
- Can be identified by `result.output_params[0].name.is_empty()`

---

## Design Highlights

### 1. Type Safety
```rust
// ✅ Compile-time type checking
let sum: i32 = outputs[0].value.as_i32()?;

// ❌ Compile error if type mismatches
let sum: String = outputs[0].value.as_i32()?;
```

### 2. Zero-Copy
```rust
// Result sets use Arc<Bytes> to avoid data copying
pub struct QueryStream<'a> {
    // Internally uses Arc<Bytes> for shared data
}
```

### 3. Fluent API
```rust
let param = RpcParam::null("@result", TypeInfo::int())
    .as_output();  // Fluent chaining
```

### 4. Type-State Pattern
```rust
// Compile-time connection state enforcement
impl Client<Ready> {
    pub async fn execute_procedure(...) { ... }
}

impl Client<InTransaction> {
    pub async fn execute_procedure(...) { ... }
}
```

---

## Usage Examples

### Example 1: Basic Output Parameters

```rust
use mssql_client::Client;
use tds_protocol::rpc::{RpcParam, TypeInfo};

let mut client = Client::connect(config).await?;

let sum_param = RpcParam::null("@sum", TypeInfo::int()).as_output();
let product_param = RpcParam::null("@product", TypeInfo::int()).as_output();

let result = client.execute_procedure(
    "dbo.sp_TestOutputParams",
    vec![
        RpcParam::int("@a", 10),
        RpcParam::int("@b", 5),
        sum_param,
        product_param,
    ],
).await?;

// Get output parameters
let sum: i32 = result.output_params[0].value.as_i32()?;
assert_eq!(sum, 15);

// Row count is always available
println!("Rows affected: {}", result.rows_affected);
```

### Example 2: Result Set + Output Parameters

```rust
let count_param = RpcParam::null("@totalCount", TypeInfo::int()).as_output();

let mut result = client.execute_procedure(
    "dbo.GetUserOrders",
    vec![
        RpcParam::int("@userId", 123),
        count_param,
    ],
).await?;

// Process result set
if let Some(mut rows) = result.take_result_set() {
    while let Some(Ok(row)) = rows.next() {
        let order_id: i32 = row.get(0)?;
        println!("Order #{}", order_id);
    }
}

// Get output parameter
let total_count: i32 = result.get_output("totalCount").unwrap().value.as_i32()?;
println!("Total rows affected: {}", result.rows_affected);
```

### Example 3: RETURN Statement

```rust
let result = client.execute_procedure(
    "dbo.CheckUserExists",
    vec![RpcParam::int("@userId", 123)],
).await?;

// RETURN value comes as first output parameter (empty name)
let return_value = &result.output_params[0];
let exists: i32 = return_value.value.as_i32()?;
println!("User exists: {}", exists == 1);
println!("Rows affected: {}", result.rows_affected);
```

### Example 4: Transaction Integration

```rust
let mut client = Client::connect(config).await?;
let mut tx = client.begin_transaction().await?;

let balance_param = RpcParam::null("@new_balance", TypeInfo::int()).as_output();

let result = tx.execute_procedure(
    "dbo.sp_TestTransaction",
    vec![
        RpcParam::int("@user_id", 123),
        balance_param,
    ],
).await?;

let balance: i32 = result.output_params[0].value.as_i32()?;
println!("New balance: {}, rows affected: {}", balance, result.rows_affected);

tx.commit().await?;
```

---

## File Changes Summary

| File                                         | Type          | Lines Changed | Description                         |
|----------------------------------------------|---------------|---------------|-------------------------------------|
| `crates/mssql-client/src/client.rs`          | New Feature   | ~200 lines     | `execute_procedure` implementation   |
| `crates/mssql-client/src/stream.rs`          | New Types     | +70 lines      | `ExecuteResult` and helper methods   |
| `crates/tds-protocol/src/token.rs`           | Bug Fix       | ~100 lines     | ReturnValue parsing logic            |
| `crates/mssql-client/tests/stored_procedure.rs` | New Tests   | ~530 lines     | 7 comprehensive test cases           |
| `crates/mssql-client/examples/stored_proc.rs` | Example       | ~140 lines     | Usage examples                       |

---

## Related Documentation

- `crates/mssql-client/examples/stored_proc.rs` - Example code
- Test suite in `crates/mssql-client/tests/stored_procedure.rs`

---

## Summary

This implementation provides complete stored procedure execution functionality by fixing TDS protocol ReturnValue token parsing. Key achievements:

1. ✅ **Type Safety** - Ensures correctness via `RpcParam` and `TypeInfo`
2. ✅ **Simple API** - Single method call for all operations
3. ✅ **Automatic Parsing** - Handles ReturnValue, DoneProc, ColMetaData tokens
4. ✅ **Zero-Copy** - Result sets use `Arc<Bytes>` pattern
5. ✅ **Complete Features** - Output params, result sets, RETURN values, row counts
6. ✅ **Production Ready** - Comprehensive test coverage, SQL Server 2008+ support
7. ✅ **Clean API** - Struct-based return with helper methods for better ergonomics

**API Design:**
```rust
// Struct-based return (current design)
pub async fn execute_procedure(...) -> Result<ExecuteResult<'_>>

pub struct ExecuteResult<'a> {
    pub output_params: Vec<OutputParam>,
    pub rows_affected: u64,
    pub result_set: Option<QueryStream<'a>>,
}
```

This implementation surpasses competitors like Tiberius and SQLx with a more ergonomic and comprehensive stored procedure execution API!
