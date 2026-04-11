# Stored Procedure API Simplification

## Overview

The rust-mssql-driver now supports a simplified API for calling stored procedures where **OUTPUT parameters are automatically detected and do not need to be provided by the user**.

This improves the developer experience by reducing boilerplate code and eliminating the need to manually provide NULL placeholders for OUTPUT parameters.

## Traditional API (Still Supported)

For compatibility, the traditional API is still supported where you provide all parameters explicitly:

```rust
let result = client
    .execute_procedure(
        "dbo.sp_CalculateStats",
        &[&7i32, &None::<i32>, &None::<i32>, &None::<i32>],
    )
    .await?;
```

## Simplified API (Recommended)

With the simplified API, you only need to provide INPUT parameters. OUTPUT parameters are automatically detected and filled with NULL values:

```rust
let result = client
    .execute_procedure("dbo.sp_CalculateStats", &[&7i32])
    .await?;
```

## How It Works

1. **Automatic Metadata Detection**: The driver queries `sp_sproc_columns` to get parameter metadata
2. **Smart Parameter Matching**: The driver automatically detects whether you're using the simplified or traditional API:
   - If parameter count equals INPUT parameter count → Simplified API
   - If parameter count equals total parameter count → Traditional API
   - Otherwise → Returns an error
3. **OUTPUT Parameter Auto-Fill**: OUTPUT parameters are automatically filled with NULL values using correct type information from metadata

## Examples

### Example 1: OUTPUT Parameters Only

**Stored Procedure:**
```sql
CREATE PROCEDURE dbo.sp_SimpleOutput
    @result INT OUTPUT
AS
BEGIN
    SET @result = 42;
END
```

**Simplified API:**
```rust
let result = client
    .execute_procedure("dbo.sp_SimpleOutput", &[] as &[&(dyn ToSql + Sync)])
    .await?;

let output = result.get_output("@result").unwrap();
let value: i32 = output.value.as_i32()?;
assert_eq!(value, 42);
```

### Example 2: Mixed INPUT and OUTPUT Parameters

**Stored Procedure:**
```sql
CREATE PROCEDURE dbo.sp_MixedParams
    @input INT,
    @doubled INT OUTPUT,
    @tripled INT OUTPUT
AS
BEGIN
    SET @doubled = @input * 2;
    SET @tripled = @input * 3;
END
```

**Simplified API:**
```rust
let result = client
    .execute_procedure("dbo.sp_MixedParams", &[&7i32])
    .await?;

let doubled = result.get_output("@doubled").unwrap();
let tripled = result.get_output("@tripled").unwrap();

println!("Doubled: {}", doubled.value.as_i32()?);  // 14
println!("Tripled: {}", tripled.value.as_i32()?);  // 21
```

### Example 3: RETURN Value

**Stored Procedure:**
```sql
CREATE PROCEDURE dbo.sp_GetStatus
    @value INT
AS
BEGIN
    RETURN @value * 10;
END
```

**Simplified API:**
```rust
let result = client
    .execute_procedure("dbo.sp_GetStatus", &[&5i32])
    .await?;

if let Some(return_value) = result.get_return_value() {
    let status: i32 = return_value.value.as_i32()?;
    println!("Status: {}", status);  // 50
}
```

### Example 4: Result Set + OUTPUT Parameters

**Stored Procedure:**
```sql
CREATE PROCEDURE dbo.sp_SearchUsers
    @min_score INT,
    @row_count INT OUTPUT
AS
BEGIN
    SELECT Id, Name, Score FROM Users WHERE Score >= @min_score;
    SET @row_count = @@ROWCOUNT;
END
```

**Simplified API:**
```rust
let result = client
    .execute_procedure("dbo.sp_SearchUsers", &[&90i32])
    .await?;

// Get OUTPUT parameter
let row_count = result.get_output("@row_count").unwrap();
println!("Rows: {}", row_count.value.as_i32()?);

// Process result set
if let Some(mut stream) = result.result_set {
    while let Some(row_result) = stream.next() {
        let row = row_result?;
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        let score: i32 = row.get(2)?;
        println!("{}: {} (score: {})", id, name, score);
    }
}
```

## Error Handling

If you provide an incorrect number of parameters, you'll get a clear error message:

```rust
// Error: expected 1 (all parameters) or 4 (INPUT only), got 2
let result = client
    .execute_procedure("dbo.sp_CalculateStats", &[&1i32, &2i32])
    .await?;
```

## Migration Guide

### Before (v0.6.x and earlier)

```rust
// Verbose - must provide all parameters including OUTPUT
let result = client
    .execute_procedure(
        "dbo.sp_CalculateStats",
        &[&7i32, &None::<i32>, &None::<i32>, &None::<i32>],
    )
    .await?;
```

### After (v0.7.0+)

```rust
// Clean - only provide INPUT parameters
let result = client
    .execute_procedure("dbo.sp_CalculateStats", &[&7i32])
    .await?;
```

## Implementation Details

The simplified API is implemented in [`convert_params_for_procedure`](../crates/mssql-client/src/client/params.rs) with the following logic:

1. Query `sp_sproc_columns` to get parameter metadata
2. Count INPUT vs OUTPUT parameters
3. Compare provided parameter count with metadata:
   - Equal to INPUT count → Use simplified API
   - Equal to total count → Use traditional API
   - Otherwise → Return error
4. For OUTPUT parameters in simplified API, auto-generate NULL values with correct type info

## Backward Compatibility

The traditional API is fully supported. Existing code will continue to work without modifications:

```rust
// This still works in v0.7.0+
let result = client
    .execute_procedure(
        "dbo.sp_CalculateStats",
        &[&7i32, &None::<i32>, &None::<i32>, &None::<i32>],
    )
    .await?;
```

## Benefits

1. **Less Boilerplate**: No need to provide NULL placeholders for OUTPUT parameters
2. **Type Safety**: OUTPUT parameter types are automatically detected from metadata
3. **Better Error Messages**: Clear parameter count mismatch errors
4. **Backward Compatible**: Existing code continues to work
5. **Developer Friendly**: More intuitive API that matches how stored procedures are actually used

## See Also

- [Example Code](../crates/mssql-client/examples/stored_procedure_simplified.rs)
- [Integration Tests](../crates/mssql-client/tests/stored_procedure.rs)
- [Parameter Conversion Implementation](../crates/mssql-client/src/client/params.rs)
