# Migrating from Tiberius

This guide helps you migrate from [tiberius](https://github.com/prisma/tiberius) to rust-mssql-driver.

## Key Differences

| Feature | Tiberius | rust-mssql-driver |
|---------|----------|-------------------|
| Runtime | Any async runtime | Tokio only |
| Type-state | No | Yes (compile-time safety) |
| Connection pooling | External (bb8/deadpool) | Built-in |
| TDS 8.0 strict | Not supported | First-class |
| Prepared statements | Manual | Auto-cached LRU |
| Azure redirects | Manual handling | Automatic |
| Result streaming | Iterator | Stream with collect |
| Transaction safety | Runtime checks | Compile-time checks |

## Connection Setup

### Tiberius

```rust
use tiberius::{Client, Config, AuthMethod};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

// Parse connection string
let config = Config::from_ado_string(
    "Server=localhost;Database=mydb;User Id=sa;Password=Password123!;"
)?;

// Manually create TCP connection
let tcp = TcpStream::connect(config.get_addr()).await?;
tcp.set_nodelay(true)?;

// Connect with compat layer
let client = Client::connect(config, tcp.compat_write()).await?;
```

### rust-mssql-driver

```rust
use mssql_client::{Client, Config};

// One-step connection from connection string
let config = Config::from_connection_string(
    "Server=localhost;Database=mydb;User Id=sa;Password=Password123!;"
)?;

let client = Client::connect(config).await?;
```

**Key changes:**
- No manual TCP stream creation
- No `tokio_util::compat` layer needed
- Connection string parsing is type-safe

## Query Execution

### Simple Queries

**Tiberius:**
```rust
let stream = client.simple_query("SELECT 1 AS value").await?;
let row = stream.into_first_result().await?;
let value: i32 = row[0].get(0).unwrap();
```

**rust-mssql-driver:**
```rust
let mut stream = client.query("SELECT 1 AS value", &[]).await?;
let row = stream.next().await.unwrap()?;
let value: i32 = row.get(0)?;
```

### Parameterized Queries

**Tiberius:**
```rust
let stream = client.query(
    "SELECT * FROM users WHERE id = @P1",
    &[&1i32]
).await?;

let row = stream.into_row().await?.unwrap();
let name: &str = row.get(1).unwrap();
```

**rust-mssql-driver:**
```rust
let mut stream = client.query(
    "SELECT * FROM users WHERE id = @p1",
    &[&1i32]
).await?;

let row = stream.next().await.unwrap()?;
let name: String = row.get(1)?;
```

**Key changes:**
- Parameter syntax: `@P1` vs `@p1` (lowercase preferred, both work)
- String values are `String` not `&str` (no lifetimes to manage)
- Explicit `get()` error handling with `?`

### Execute (Non-Query)

**Tiberius:**
```rust
let result = client.execute(
    "INSERT INTO users (name) VALUES (@P1)",
    &[&"Alice"]
).await?;

let rows_affected = result.total();
```

**rust-mssql-driver:**
```rust
let rows_affected = client.execute(
    "INSERT INTO users (name) VALUES (@p1)",
    &[&"Alice"]
).await?;
```

**Key changes:**
- `execute()` directly returns row count
- No intermediate result type to call `.total()` on

## Result Processing

### Iterating Rows

**Tiberius:**
```rust
let stream = client.query("SELECT * FROM users", &[]).await?;
let rows = stream.into_first_result().await?;

for row in rows {
    let id: i32 = row.get(0).unwrap();
    let name: &str = row.get(1).unwrap();
}
```

**rust-mssql-driver:**
```rust
let mut stream = client.query("SELECT * FROM users", &[]).await?;

while let Some(row) = stream.next().await {
    let row = row?;
    let id: i32 = row.get(0)?;
    let name: String = row.get(1)?;
}

// Or collect all rows
let rows: Vec<Row> = stream.collect_all().await?;
```

**Key changes:**
- Async iteration with `stream.next().await`
- Explicit error handling per row
- `collect_all()` for buffered results

### Column Access by Name

**Tiberius:**
```rust
let name: &str = row.get("name").unwrap();
```

**rust-mssql-driver:**
```rust
let name: String = row.get_by_name("name")?;
```

### Multiple Result Sets

**Tiberius:**
```rust
let mut results = client.query("EXEC sp_multi_results", &[]).await?;

// First result set
while let Some(row) = results.next().await {
    // ...
}

// Advance to next result set
while let Some(row) = results.next().await {
    // ...
}
```

**rust-mssql-driver:**
```rust
let mut results = client.query("EXEC sp_multi_results", &[]).await?;

// First result set
while let Some(row) = results.next().await {
    let row = row?;
    // ...
}

// Advance to next result set
if results.next_result().await? {
    while let Some(row) = results.next().await {
        let row = row?;
        // ...
    }
}
```

## Transactions

### Basic Transaction

**Tiberius:**
```rust
// Manual transaction management
client.execute("BEGIN TRANSACTION", &[]).await?;

client.execute("INSERT INTO users (name) VALUES (@P1)", &[&"Alice"]).await?;

// If something goes wrong, must remember to rollback
match some_operation().await {
    Ok(_) => client.execute("COMMIT", &[]).await?,
    Err(e) => {
        client.execute("ROLLBACK", &[]).await?;
        return Err(e);
    }
};
```

**rust-mssql-driver:**
```rust
// Type-state enforced transactions
let tx = client.begin_transaction().await?;

tx.execute("INSERT INTO users (name) VALUES (@p1)", &[&"Alice"]).await?;

// Type system ensures commit/rollback
match some_operation(&mut tx).await {
    Ok(_) => {
        let client = tx.commit().await?;  // Returns Client<Ready>
        // Can continue using client
    }
    Err(e) => {
        let client = tx.rollback().await?;  // Returns Client<Ready>
        return Err(e);
    }
};
```

**Key changes:**
- `begin_transaction()` returns typed transaction
- `commit()` / `rollback()` return the client
- Cannot forget to complete transaction (compile error)
- Cannot use client while transaction is active (moved)

### Savepoints

**Tiberius:**
```rust
client.execute("BEGIN TRANSACTION", &[]).await?;
client.execute("INSERT INTO orders ...", &[]).await?;
client.execute("SAVE TRANSACTION before_items", &[]).await?;
client.execute("INSERT INTO items ...", &[]).await?;

// Rollback to savepoint
client.execute("ROLLBACK TRANSACTION before_items", &[]).await?;
client.execute("COMMIT", &[]).await?;
```

**rust-mssql-driver:**
```rust
let mut tx = client.begin_transaction().await?;
tx.execute("INSERT INTO orders ...", &[]).await?;

// Create savepoint (name validated for SQL injection)
let sp = tx.save_point("before_items").await?;
tx.execute("INSERT INTO items ...", &[]).await?;

// Rollback to savepoint
tx.rollback_to(&sp).await?;

let client = tx.commit().await?;
```

**Key changes:**
- Savepoint names are validated
- `SavePoint` handle ensures type safety
- Invalid savepoint names caught at runtime with clear error

## Connection Pooling

### Tiberius + bb8

```rust
use bb8::Pool;
use bb8_tiberius::ConnectionManager;

let manager = ConnectionManager::build(config)?;
let pool = Pool::builder().max_size(10).build(manager).await?;

let conn = pool.get().await?;
conn.query("SELECT 1", &[]).await?;
```

### rust-mssql-driver Built-in Pool

```rust
use mssql_driver_pool::Pool;

let pool = Pool::builder()
    .client_config(config)
    .max_connections(10)
    .min_connections(2)
    .connection_timeout(Duration::from_secs(30))
    .build()
    .await?;

let mut conn = pool.get().await?;
conn.query("SELECT 1", &[]).await?;

// Connection automatically returned on drop
// sp_reset_connection called automatically
```

**Key changes:**
- No external pool crate needed
- `sp_reset_connection` called automatically
- Health checks built-in
- Connection metrics available

## Azure SQL Routing

### Tiberius

```rust
// Must handle routing manually
loop {
    match Client::connect(config.clone(), tcp.compat_write()).await {
        Ok(client) => break client,
        Err(tiberius::error::Error::Routing { host, port }) => {
            // Create new TCP connection to redirect target
            let addr = format!("{}:{}", host, port);
            tcp = TcpStream::connect(&addr).await?;
            continue;
        }
        Err(e) => return Err(e.into()),
    }
}
```

### rust-mssql-driver

```rust
// Automatic - just connect
let client = Client::connect(config).await?;
// Azure SQL gateway redirects handled automatically
```

**Key change:** Zero manual redirect handling needed.

## Prepared Statements

### Tiberius

```rust
// No built-in caching - each execution prepares anew
for user_id in user_ids {
    // Implicit sp_executesql each time
    let stream = client.query(
        "SELECT * FROM users WHERE id = @P1",
        &[&user_id]
    ).await?;
}
```

### rust-mssql-driver

```rust
// Automatic LRU caching
for user_id in user_ids {
    // First call: sp_prepare + sp_execute
    // Subsequent calls: sp_execute only (cache hit)
    let mut stream = client.query(
        "SELECT * FROM users WHERE id = @p1",
        &[&user_id]
    ).await?;
}
```

## Error Handling

### Tiberius

```rust
use tiberius::error::Error;

match result {
    Err(Error::Server(e)) => {
        println!("SQL Error {}: {}", e.code(), e.message());
    }
    Err(Error::Io(e)) => {
        println!("IO Error: {}", e);
    }
    // ...
}
```

### rust-mssql-driver

```rust
use mssql_client::Error;

match result {
    Err(Error::Server { number, message, .. }) => {
        println!("SQL Error {}: {}", number, message);

        // Built-in retry classification
        if error.is_transient() {
            // Safe to retry
        }
    }
    Err(Error::Io(e)) => {
        println!("IO Error: {}", e);
    }
    // ...
}
```

**Key changes:**
- `is_transient()` and `is_terminal()` built-in
- Structured error with all server error fields
- Error classification for retry logic

## Type Mappings

### Key Differences

| SQL Type | Tiberius | rust-mssql-driver |
|----------|----------|-------------------|
| NVARCHAR | `&str` (borrowed) | `String` (owned) |
| VARBINARY | `&[u8]` (borrowed) | `Bytes` (shared) |
| NULL handling | `Option<T>` | `Option<T>` |
| DECIMAL | `Decimal` | `Decimal` |
| Date/Time | `chrono` types | `chrono` types |
| UNIQUEIDENTIFIER | `Uuid` | `Uuid` |

The main difference is owned vs borrowed string types. rust-mssql-driver uses owned types to avoid lifetime complexity.

## Feature Flags

### Tiberius

```toml
[dependencies]
tiberius = { version = "0.12", features = ["chrono", "rust_decimal"] }
```

### rust-mssql-driver

```toml
[dependencies]
mssql-client = { version = "0.10", features = ["chrono", "decimal", "uuid"] }
```

Available features:
- `chrono` - Date/time type support
- `decimal` - `rust_decimal::Decimal` support
- `uuid` - UUID support
- `json` - `serde_json::Value` support
- `otel` - OpenTelemetry instrumentation
- `zeroize` - Secure credential wiping

## Common Migration Issues

### 1. "Method not found" for transaction operations

**Problem:** Trying to call `query()` on client after `begin_transaction()`.

**Tiberius:**
```rust
let tx = client.begin_transaction().await?;
client.query("SELECT 1", &[]).await?;  // Still works (runtime tracking)
```

**rust-mssql-driver:**
```rust
let tx = client.begin_transaction().await?;
// client is moved into tx!
// client.query("SELECT 1", &[]).await?;  // Compile error!
tx.query("SELECT 1", &[]).await?;  // Use tx instead
```

### 2. Borrowed vs Owned Strings

**Problem:** Type mismatch when extracting string values.

**Tiberius:**
```rust
let name: &str = row.get("name").unwrap();
```

**rust-mssql-driver:**
```rust
let name: String = row.get_by_name("name")?;  // Owned
// Or borrow from the row:
let name: Option<Cow<'_, str>> = row.get_str(0);  // Borrowed from row's buffer
```

### 3. Missing `compat_write()`

**Problem:** No `compat` layer needed.

**Tiberius:**
```rust
use tokio_util::compat::TokioAsyncWriteCompatExt;
let client = Client::connect(config, tcp.compat_write()).await?;
```

**rust-mssql-driver:**
```rust
// Just connect directly - no compat needed
let client = Client::connect(config).await?;
```

### 4. Different Parameter Naming

**Problem:** Parameters not bound correctly.

**Tiberius:** `@P1`, `@P2` (uppercase)
**rust-mssql-driver:** `@p1`, `@p2` (lowercase) or `@P1` (both work)

### 5. Transaction Return Value

**Problem:** Forgetting to capture returned client.

```rust
// Wrong - client is lost
let tx = client.begin_transaction().await?;
tx.execute("INSERT ...", &[]).await?;
tx.commit().await?;  // Returns Client<Ready>, but we ignore it!

// Correct
let tx = client.begin_transaction().await?;
tx.execute("INSERT ...", &[]).await?;
let client = tx.commit().await?;  // Capture the returned client
```

## Quick Reference Card

| Tiberius | rust-mssql-driver |
|----------|-------------------|
| `Config::from_ado_string()` | `Config::from_connection_string()` |
| `Client::connect(config, tcp.compat_write())` | `Client::connect(config)` |
| `client.query(sql, &[&p1])` | `client.query(sql, &[&p1])` |
| `stream.into_first_result()` | `stream.collect_all()` |
| `row.get(0).unwrap()` | `row.get(0)?` |
| `@P1` | `@p1` |
| `bb8::Pool` + `bb8_tiberius` | `mssql_driver_pool::Pool` |
| `execute("BEGIN TRANSACTION")` | `begin_transaction()` |
| `execute("COMMIT")` | `commit()` → returns client |
| Manual Azure redirect | Automatic |
| No statement caching | Automatic LRU cache |
