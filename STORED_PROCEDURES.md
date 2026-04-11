# Stored Procedure Execution Support

## Overview

This implementation adds comprehensive stored procedure execution support to rust-mssql-driver, featuring a **simplified API** where OUTPUT parameters are automatically detected and do not need to be provided by the user.

## 🚀 Key Features

### 1. Simplified API (Recommended)

**Only provide INPUT parameters - OUTPUT parameters are auto-detected:**

```rust
// Before (verbose):
client.execute_procedure(
    "dbo.sp_CalculateStats",
    &[&7i32, &None::<i32>, &None::<i32>, &None::<i32>],
).await?;

// After (clean):
client.execute_procedure("dbo.sp_CalculateStats", &[&7i32]).await?;
```

### 2. Traditional API (Backward Compatible)

**Still supports explicit parameter provision:**

```rust
client.execute_procedure(
    "dbo.sp_CalculateStats",
    &[&7i32, &None::<i32>, &None::<i32>, &None::<i32>],
).await?;
```

### 3. Automatic OUTPUT Parameter Detection

The driver automatically discovers OUTPUT parameters by querying `sp_sproc_columns` metadata - no manual marking required.

### 4. Complete Result Encapsulation

Returns `ExecuteResult` struct containing all critical execution data:

```rust
pub struct ExecuteResult<'a> {
    /// OUTPUT parameters (index 0 = RETURN value, index 1+ = OUTPUT parameters)
    pub output_params: Vec<OutputParam>,

    /// Number of rows affected
    pub rows_affected: u64,

    /// Result set (if any)
    pub result_set: Option<QueryStream<'a>>,
}
```

### 5. Built-in RETURN Value Handling

Fully complies with SQL Server specification: every stored procedure returns an integer value (default: 0), automatically included as the first element in `output_params`:

```rust
if let Some(rv) = result.get_return_value() {
    let status: i32 = rv.value.as_i32()?;
    println!("Procedure returned status: {}", status);
}
```

### 6. Type-State Pattern

Implements compile-time connection state enforcement, supporting both `Client<Ready>` and `Client<InTransaction>` with identical signatures.

### 7. Zero-Copy Result Sets

Uses `Arc<Bytes>` internally for zero-copy result sets, optimizing memory efficiency.

### 8. Timeout Support

Built-in timeout support for long-running stored procedures:

```rust
use std::time::Duration;

let result = client
    .execute_procedure_with_timeout("dbo.sp_LongRunning", &[&data], Duration::from_secs(5))
    .await?;
```

### 9. Multiple Result Sets

Full support for stored procedures returning multiple result sets:

```rust
let mut result = client
    .execute_procedure_multiple("dbo.sp_GetMultipleReports", &[&min_score])
    .await?;

// Process first result set
while let Some(row) = result.next_row().await? {
    // Process rows...
}

// Move to next result set
while result.next_result().await? {
    // Process next result set...
}
```

## 📖 Usage Examples

### Basic OUTPUT Parameters (Simplified API)

```rust
// SQL: CREATE PROCEDURE dbo.CalculateSum
//      @a INT, @b INT, @result INT OUTPUT
//      AS SET @result = @a + @b;

let result = client
    .execute_procedure("dbo.CalculateSum", &[&10i32, &20i32])
    .await?;

let sum = result.get_output("@result").unwrap().value.as_i32()?;
println!("Sum: {}", sum); // 30
```

### Result Sets + OUTPUT Parameters

```rust
// SQL: CREATE PROCEDURE dbo.GetUserStats
//      @min_score INT,
//      @row_count INT OUTPUT
//      AS
//      BEGIN
//          SELECT Id, Name, Score FROM Users WHERE Score >= @min_score;
//          SET @row_count = @@ROWCOUNT;
//      END

let result = client
    .execute_procedure("dbo.GetUserStats", &[&90i32])
    .await?;

// Process result set
if let Some(mut stream) = result.result_set {
    while let Some(Ok(row)) = stream.next() {
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        let score: i32 = row.get(2)?;
        println!("{}: {} (score: {})", id, name, score);
    }
}

// Get OUTPUT parameter
let count = result.get_output("@row_count").unwrap().value.as_i32()?;
println!("Total: {} rows", count);
```

### RETURN Statement Support

```rust
// SQL: CREATE PROCEDURE dbo.CheckStatus
//      @id INT
//      AS RETURN @id * 10;

let result = client
    .execute_procedure("dbo.CheckStatus", &[&5i32])
    .await?;

let status = result.get_return_value().unwrap().value.as_i32()?;
println!("Status: {}", status); // 50
```

### Transaction Integration

```rust
let mut tx = client.begin_transaction().await?;

let result = tx
    .execute_procedure("dbo.UpdateUser", &[&123i32, &"John"])
    .await?;

let new_id = result.get_output("@newId").unwrap().value.as_i32()?;
tx.commit().await?;
```

### Multiple OUTPUT Parameters

```rust
// SQL: CREATE PROCEDURE dbo.sp_CalculateStats
//      @input INT,
//      @doubled INT OUTPUT,
//      @tripled INT OUTPUT,
//      @squared INT OUTPUT
//      AS
//      BEGIN
//          SET @doubled = @input * 2;
//          SET @tripled = @input * 3;
//          SET @squared = @input * @input;
//      END

let result = client
    .execute_procedure("dbo.sp_CalculateStats", &[&7i32])
    .await?;

let doubled = result.get_output("@doubled").unwrap();
let tripled = result.get_output("@tripled").unwrap();
let squared = result.get_output("@squared").unwrap();

println!("Doubled: {}", doubled.value.as_i32()?);  // 14
println!("Tripled: {}", tripled.value.as_i32()?);  // 21
println!("Squared: {}", squared.value.as_i32()?);  // 49
```

### Only OUTPUT Parameters (No INPUT)

```rust
// SQL: CREATE PROCEDURE dbo.sp_GetConstant
//      @result INT OUTPUT
//      AS SET @result = 42;

let params: &[&(dyn mssql_client::ToSql + Sync)] = &[];
let result = client
    .execute_procedure("dbo.sp_GetConstant", params)
    .await?;

let value = result.get_output("@result").unwrap().value.as_i32()?;
println!("Constant: {}", value); // 42
```

### Timeout Support

```rust
use std::time::Duration;

// SQL: CREATE PROCEDURE dbo.sp_LongRunning
//      @seconds INT
//      AS WAITFOR DELAY '00:00:00:' + CAST(@seconds AS VARCHAR);

// Set timeout to prevent long-running procedures
let result = client
    .execute_procedure_with_timeout("dbo.sp_LongRunning", &[&10i32], Duration::from_secs(3))
    .await;

match result {
    Ok(result) => println!("Completed: {:?}", result.get_output("@status")),
    Err(e) => println!("Timeout or error: {}", e),
}
```

### Multiple Result Sets

```rust
// SQL: CREATE PROCEDURE dbo.sp_GetUserReports
//      @min_score INT
//      AS
//      BEGIN
//          -- First result set: User summary
//          SELECT Id, Name, 'Summary' AS Type FROM Users WHERE Score >= @min_score;
//
//          -- Second result set: Detailed scores
//          SELECT Id, Name, Score FROM Users WHERE Score >= @min_score;
//      END

let mut result = client
    .execute_procedure_multiple("dbo.sp_GetUserReports", &[&90i32])
    .await?;

// Process first result set
println!("Summary Report:");
while let Some(row) = result.next_row().await? {
    let id: i32 = row.get(0)?;
    let name: String = row.get(1)?;
    let report_type: String = row.get(2)?;
    println!("  - {id}: {name} ({report_type})");
}

// Move to second result set
if result.next_result().await? {
    println!("Detailed Scores:");
    while let Some(row) = result.next_row().await? {
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        let score: i32 = row.get(2)?;
        println!("  - {id}: {name} - {score}");
    }
}

println!("Total result sets: {}", result.result_count());
```

## 🎯 Supported Data Types

### INPUT Parameters (Rust → SQL Server)

| Rust Type | SQL Server Type | Notes |
|-----------|-----------------|-------|
| `bool` | BIT | Boolean values |
| `i8` | TINYINT | 8-bit integer |
| `i16` | SMALLINT | 16-bit integer |
| `i32` | INT | 32-bit integer |
| `i64` | BIGINT | 64-bit integer |
| `f32` | REAL | 32-bit float |
| `f64` | FLOAT | 64-bit float |
| `&str`, `String` | NVARCHAR | Unicode strings |
| `Vec<u8>`, `&[u8]` | VARBINARY | Binary data |
| `Option<T>` | Any NULL | Nullable values |
| `uuid::Uuid` | UNIQUEIDENTIFIER | UUID values (feature: uuid) |
| `chrono::NaiveDate` | DATE | Date values (feature: chrono) |
| `chrono::NaiveTime` | TIME | Time values (feature: chrono) |
| `chrono::NaiveDateTime` | DATETIME2 | DateTime values (feature: chrono) |
| `rust_decimal::Decimal` | DECIMAL | Decimal values (feature: decimal) |

### OUTPUT Parameters (SQL Server → Rust)

| SQL Server Type | Rust Representation | Notes |
|-----------------|---------------------|-------|
| BIT | `SqlValue::Bool(bool)` | Boolean values |
| TINYINT | `SqlValue::TinyInt(u8)` | 8-bit unsigned |
| SMALLINT | `SqlValue::SmallInt(i16)` | 16-bit signed |
| INT | `SqlValue::Int(i32)` | 32-bit signed |
| BIGINT | `SqlValue::BigInt(i64)` | 64-bit signed |
| REAL | `SqlValue::Float(f32)` | 32-bit float |
| FLOAT | `SqlValue::Double(f64)` | 64-bit float |
| NVARCHAR | `SqlValue::String(String)` | Unicode strings |
| VARBINARY | `SqlValue::Binary(Bytes)` | Binary data |
| NULL | `SqlValue::Null` | Null value |
| UNIQUEIDENTIFIER | `SqlValue::Uuid(uuid::Uuid)` | UUID values |
| DATETIME2 | `SqlValue::String` | May be string-decoded |
| DECIMAL | `SqlValue::String` | May be string-decoded |

**Note:** Some complex types (DATETIME, DECIMAL) may be decoded as strings due to TDS protocol limitations.

## 🧪 Testing

### Test Coverage

Our comprehensive test suite includes **20 integration tests** covering:

#### Basic Functionality
- ✅ Only OUTPUT parameters (no INPUT)
- ✅ Only INPUT parameters (no OUTPUT)
- ✅ Multiple OUTPUT parameters
- ✅ RETURN values
- ✅ NULL OUTPUT parameters

#### Advanced Scenarios
- ✅ Result sets + OUTPUT parameters
- ✅ Multiple result sets
- ✅ Transaction integration
- ✅ Parameter count mismatch errors
- ✅ Timeout functionality
- ✅ Timeout in transactions
- ✅ Multiple result sets in transactions

#### Data Type Support
- ✅ String OUTPUT parameters
- ✅ Boolean INPUT/OUTPUT parameters
- ✅ Decimal INPUT/OUTPUT parameters
- ✅ DateTime INPUT/OUTPUT parameters
- ✅ Binary INPUT/OUTPUT parameters
- ✅ Various numeric types (TINYINT, SMALLINT, INT, BIGINT)

#### API Compatibility
- ✅ Simplified API (auto-detect OUTPUT)
- ✅ Traditional API (explicit parameters)
- ✅ Error handling and validation

### Running Tests

```bash
# Set environment variables (or use defaults: localhost/sa/YourStrong@Passw0rd/TestDB)
export MSSQL_HOST=localhost
export MSSQL_PASSWORD=YourPassword

# Run stored procedure tests
cargo test -p mssql-client --test stored_procedure -- --ignored
```

### Test Results

```
running 20 tests
test test_stored_procedure_output_params ... ok
test test_stored_procedure_only_input_params ... ok
test test_stored_procedure_multiple_outputs ... ok
test test_stored_procedure_return_statement ... ok
test test_stored_procedure_null_output_param ... ok
test test_stored_procedure_string_output_param ... ok
test test_stored_procedure_boolean_types ... ok
test test_stored_procedure_decimal_output ... ok
test test_stored_procedure_datetime_output ... ok
test test_stored_procedure_binary_output ... ok
test test_stored_procedure_result_set_and_outputs ... ok
test test_stored_procedure_multiple_result_sets ... ok
test test_stored_procedure_in_transaction ... ok
test test_stored_procedure_multiple_in_transaction ... ok
test test_stored_procedure_traditional_api ... ok
test test_stored_procedure_parameter_count_mismatch ... ok
test test_stored_procedure_various_numeric_types ... ok
test test_stored_procedure_with_timeout ... ok
test test_stored_procedure_timeout_expires ... ok
test test_stored_procedure_timeout_in_transaction ... ok

test result: ok. 20 passed; 0 failed
```

## ⚖️ Comparison with Competitors

| Feature | rust-mssql-driver | Tiberius | SQLx |
|----------|-------------------|----------|------|
| **API Simplicity** | 🟢 Simplified (auto-detect OUTPUT) | 🔴 Manual RPC construction | 🟡 Basic support |
| **Parameter Syntax** | `&[&(dyn ToSql + Sync)]` | `Vec<RpcParam>` | Same |
| **API Consistency** | ✅ Same as query/execute | ❌ Different | ✅ Consistent |
| **OUTPUT Detection** | ✅ Automatic | ❌ Manual marking | ❌ No support |
| **RETURN Values** | ✅ Always included | ⚠️ Manual parsing | ❌ No support |
| **Transaction Support** | ✅ Type-safe | ✅ Basic | ✅ Basic |
| **Zero-Copy** | ✅ Arc<Bytes> | ✅ Bytes | ✅ Zero-copy |
| **Timeout Support** | ✅ Built-in | ⚠️ Manual implementation | ⚠️ Manual |
| **Multiple Result Sets** | ✅ Full support | ⚠️ Manual parsing | ❌ Limited |
| **Test Coverage** | ✅ 20 comprehensive tests | ⚠️ Basic | ⚠️ Basic |

## 🔧 Technical Implementation

### How Simplified API Works

1. **Metadata Query**: Query `sp_sproc_columns` to get parameter metadata
2. **Parameter Classification**: Separate INPUT vs OUTPUT parameters
3. **Smart Matching**:
   - If provided params == INPUT count → Use simplified API
   - If provided params == total count → Use traditional API
   - Otherwise → Return clear error
4. **Auto-Fill OUTPUT**: OUTPUT parameters automatically filled with NULL values

### Parameter Metadata Querying

Uses `sp_sproc_columns` system stored procedure for excellent version compatibility (SQL Server 2000+):

```sql
EXEC sp_sproc_columns
    @procedure_name = 'MyProc',
    @procedure_owner = 'dbo'
```

Returns parameter information including:
- Parameter name (with @ prefix)
- Position (ordinal)
- Type (INPUT vs OUTPUT)
- Data type name
- Precision/Scale/Length

### Type Information Mapping

Maps SQL Server type names to TDS wire-format `RpcTypeInfo`:

```rust
match meta.type_name.to_uppercase().as_str() {
    "INT" => RpcTypeInfo::int(),
    "BIGINT" => RpcTypeInfo::bigint(),
    "NVARCHAR" => RpcTypeInfo::nvarchar(max_len),
    "BIT" => RpcTypeInfo::bit(),
    "DECIMAL" => RpcTypeInfo::decimal(precision, scale),
    // ... etc
}
```

### Error Handling

Comprehensive error handling for:

- **Parameter count mismatch**: Clear error message showing expected vs actual
- **Metadata query failure**: Graceful handling of system stored procedure errors
- **Type conversion errors**: Detailed type mismatch information
- **Protocol errors**: Proper TDS protocol error propagation

### Known Limitations

1. **Table-Valued Parameters (TVP)**: Not currently supported in stored procedures
2. **Boolean OUTPUT Decoding**: TDS type 0x68 has known decoding limitations (handled gracefully)
3. **Complex Type Decoding**: Some types (DATETIME, DECIMAL) may be decoded as strings
4. **Metadata Caching**: Each call queries metadata; future versions may add LRU caching

## 🚀 Migration Guide

### From Tiberius

**Tiberius:**
```rust
let mut params = vec![
    RpcParam::new("@input", RpcTypeInfo::int(), input_value),
    RpcParam::new("@output", RpcTypeInfo::int(), Bytes::new()).with_output(),
];

let rpc = RpcRequest::named("dbo.MyProc").params(&params);
let result = client.execute(rpc).await?;
```

**rust-mssql-driver (Simplified):**
```rust
let result = client
    .execute_procedure("dbo.MyProc", &[&input_value])
    .await?;

let output = result.get_output("@output").unwrap();
```

### From Manual SQL

**Before:**
```rust
// Execute SQL manually
client.execute("DECLARE @result INT; EXEC dbo.MyProc @input, @result OUTPUT; SELECT @result", &[&input]).await?;
```

**After:**
```rust
// Clean API with automatic OUTPUT handling
let result = client.execute_procedure("dbo.MyProc", &[&input]).await?;
let output = result.get_output("@result").unwrap();
```

## 📚 Additional Resources

- **Example Code**:
  - `crates/mssql-client/examples/stored_procedure_simplified.rs` - Simplified API demonstration
  - `crates/mssql-client/examples/stored_procedure_timeout.rs` - Timeout functionality examples
  - `crates/mssql-client/examples/stored_procedure_multiple.rs` - Multiple result sets examples
- **Integration Tests**: `crates/mssql-client/tests/stored_procedure.rs`
- **API Documentation**: `docs/STORED_PROCEDURE_API.md`
- **Implementation**:
  - `crates/mssql-client/src/client/mod.rs` - Main API
  - `crates/mssql-client/src/client/params.rs` - Parameter conversion
  - `crates/mssql-client/src/client/response.rs` - Response parsing
  - `crates/mssql-client/src/stream.rs` - Type definitions

## 🎯 Best Practices

1. **Use Simplified API**: Let the driver auto-detect OUTPUT parameters
2. **Check RETURN Values**: Always check `get_return_value()` for procedure status
3. **Handle NULL Values**: Use `Option<T>` for nullable OUTPUT parameters
4. **Transaction Safety**: Use `begin_transaction()` for multi-step operations
5. **Error Handling**: Properly handle `Error::Protocol` for parameter mismatches
6. **Timeout Management**: Set appropriate timeouts for long-running procedures
7. **Multiple Result Sets**: Use `execute_procedure_multiple` for procedures with multiple SELECTs
8. **Resource Cleanup**: Always consume all result sets to avoid connection pool issues

## 🔮 Future Enhancements

- [x] Timeout support for long-running procedures ✅ (Completed in v0.7.0)
- [x] Multiple result sets support ✅ (Completed in v0.7.0)
- [ ] Metadata caching with LRU eviction
- [ ] Improved complex type decoding (DATETIME, DECIMAL)
- [ ] Table-Valued Parameter (TVP) support
- [ ] Performance benchmarks and optimization
- [ ] Prepared procedure caching
- [ ] Async streaming for large result sets
- [ ] Cancelation token support for long-running queries
