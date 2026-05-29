# Type-State Pattern Guide

This document explains the type-state pattern used in rust-mssql-driver and how to work with it effectively.

## What is Type-State?

Type-state is a design pattern where the compiler enforces valid state transitions at compile time. Instead of runtime checks, invalid operations become compile-time errors.

```text
// Traditional approach - runtime check
client.query(...)?;  // May fail if not connected

// Type-state approach - compile-time check
let connected_client = Client::connect(config).await?;  // Returns Client<Ready>
connected_client.query(...)?;  // Guaranteed to be connected
```

## Connection States

rust-mssql-driver uses five connection states:

```text
┌──────────────┐
│ Disconnected │
└──────┬───────┘
       │ connect()
       ▼
┌──────────────┐
│  Connected   │ (internal - users don't see this)
└──────┬───────┘
       │ authenticate()
       ▼
┌──────────────┐     begin_transaction()    ┌───────────────┐
│    Ready     │ ────────────────────────▶  │ InTransaction │
└──────┬───────┘                            └───────┬───────┘
       │                                            │
       │ query()                                    │ commit()/rollback()
       ▼                                            ▼
┌──────────────┐                            Back to Ready
│  Streaming   │
└──────┬───────┘
       │ stream exhausted
       ▼
Back to Ready (or InTransaction if in tx)
```

### State Descriptions

| State | Description | Available Operations |
|-------|-------------|---------------------|
| `Disconnected` | Initial state, no connection | `connect()` |
| `Connected` | TCP connected, pre-auth (internal) | `authenticate()` |
| `Ready` | Authenticated, ready for queries | `query()`, `execute()`, `begin_transaction()`, `close()` |
| `InTransaction` | Inside a transaction | `query()`, `execute()`, `commit()`, `rollback()`, `save_point()` |
| `Streaming` | Processing a result stream | Iterate/drain the stream |

## Basic Usage

### Connecting

```rust,no_run
use mssql_client::{Client, Config, Ready};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=db;User Id=sa;Password=Password123")?;
    // Client::connect returns Client<Ready>
    let _client: Client<Ready> = Client::connect(config).await?;
    Ok(())
}
```

### Querying

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=db;User Id=sa;Password=Password123")?;
    let mut client = Client::connect(config).await?;

    // query() is only available on Client<Ready>
    let rows = client.query("SELECT * FROM users", &[]).await?;

    for result in rows {
        let _row = result?;
        // Process row
    }
    Ok(())
}
```

### Transactions

```rust,no_run
use mssql_client::{Client, Config, InTransaction, Ready};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=db;User Id=sa;Password=Password123")?;
    let client = Client::connect(config).await?;

    // begin_transaction() transforms Client<Ready> into Client<InTransaction>
    let mut tx: Client<InTransaction> = client.begin_transaction().await?;

    // Execute within transaction
    tx.execute("INSERT INTO users (name) VALUES (@p1)", &[&"Alice"]).await?;

    // commit() transforms Client<InTransaction> back to Client<Ready>
    let _client: Client<Ready> = tx.commit().await?;
    Ok(())
}
```

## Compile-Time Safety

### Prevents Use-Before-Connect

```text
// This won't compile - can't query without connecting
let client = Client::<Disconnected>::new();
client.query("SELECT 1", &[]).await?;  // ERROR: no method `query` on Client<Disconnected>
```

### Prevents Nested Transactions

```text
let mut tx = client.begin_transaction().await?;

// This won't compile - can't begin transaction when already in one
tx.begin_transaction().await?;  // ERROR: no method `begin_transaction` on Client<InTransaction>
```

### Enforces Transaction Completion

```text
{
    let tx = client.begin_transaction().await?;
    tx.execute("INSERT ...", &[]).await?;
    // tx dropped without commit/rollback
}
// client is consumed - can't use it anymore without completing the transaction
```

## Working with Transactions

### Basic Transaction

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=db;User Id=sa;Password=Password123")?;
    let client = Client::connect(config).await?;

    // Begin transaction - client is moved into tx
    let mut tx = client.begin_transaction().await?;

    tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = 1", &[]).await?;
    tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = 2", &[]).await?;

    // Commit returns the client
    let mut client = tx.commit().await?;

    // Can use client again
    let _rows = client.query("SELECT * FROM accounts", &[]).await?;
    Ok(())
}
```

### Transaction with Rollback

```text
let mut tx = client.begin_transaction().await?;

match perform_operations(&mut tx).await {
    Ok(()) => {
        let client = tx.commit().await?;
        // Use client...
    }
    Err(e) => {
        let client = tx.rollback().await?;
        // Transaction rolled back, can use client again
    }
}
```

### Savepoints

```text
let mut tx = client.begin_transaction().await?;

tx.execute("INSERT INTO orders ...", &[]).await?;

// Create savepoint
let sp = tx.save_point("before_items").await?;

tx.execute("INSERT INTO order_items ...", &[]).await?;

// If something goes wrong, rollback to savepoint
if items_invalid {
    tx.rollback_to(&sp).await?;
}

tx.commit().await?;
```

### Isolation Levels

```text
use mssql_client::IsolationLevel;

let mut tx = client
    .begin_transaction()
    .isolation_level(IsolationLevel::Serializable)
    .await?;
```

## Advanced Patterns

### Generic Functions Over States

Write functions that work with any connection state:

```text
use mssql_client::{Client, ConnectionState, Ready, InTransaction};

// Function that works with any queryable state
async fn get_user_count<S>(client: &mut Client<S>) -> Result<i32, Error>
where
    S: ConnectionState + CanQuery,  // CanQuery is a marker trait
{
    let rows = client.query("SELECT COUNT(*) FROM users", &[]).await?;
    // ...
}

// Works with both Ready and InTransaction
get_user_count(&mut client).await?;
get_user_count(&mut tx).await?;
```

### Returning Client from Functions

When writing helper functions, be explicit about state transitions:

```text
// This function consumes a Ready client and returns it
async fn perform_queries(
    client: Client<Ready>
) -> Result<(Vec<User>, Client<Ready>), Error> {
    let mut client = client;
    let rows = client.query("SELECT * FROM users", &[]).await?;
    let users = parse_users(rows)?;
    Ok((users, client))
}

// Usage
let (users, client) = perform_queries(client).await?;
```

### Transaction Wrapper Pattern

```text
async fn in_transaction<F, T>(
    client: Client<Ready>,
    f: F,
) -> Result<(T, Client<Ready>), Error>
where
    F: FnOnce(&mut Client<InTransaction>) -> BoxFuture<'_, Result<T, Error>>,
{
    let mut tx = client.begin_transaction().await?;

    match f(&mut tx).await {
        Ok(result) => {
            let client = tx.commit().await?;
            Ok((result, client))
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}

// Usage
let (result, client) = in_transaction(client, |tx| {
    Box::pin(async move {
        tx.execute("INSERT ...", &[]).await?;
        Ok(42)
    })
}).await?;
```

## Common Pitfalls

### Forgetting to Use Returned Client

```text
// WRONG - client is consumed
let tx = client.begin_transaction().await?;
tx.commit().await?;  // Returns new client, but we ignore it
// client is no longer valid!

// CORRECT
let tx = client.begin_transaction().await?;
let client = tx.commit().await?;  // Capture returned client
// client is valid again
```

### Trying to Reuse Moved Client

```text
// WRONG
let tx = client.begin_transaction().await?;
client.query(...);  // ERROR: client was moved into tx

// CORRECT
let tx = client.begin_transaction().await?;
tx.query(...).await?;  // Use tx, not client
```

### Mixing States

```text
// WRONG - can't store different states in same variable
let client = Client::connect(config).await?;
let client = client.begin_transaction().await?;  // Now it's Client<InTransaction>
let rows = client.query(...).await?;  // OK, InTransaction allows query
let client = client.commit().await?;  // Back to Client<Ready>
// Above is actually correct, but be aware of state changes
```

## Benefits

1. **Compile-Time Guarantees**
   - No "connection not open" runtime errors
   - No "already in transaction" runtime errors
   - Invalid operations are caught during compilation

2. **Self-Documenting Code**
   - Function signatures show required state
   - State transitions are explicit

3. **Memory Safety**
   - Client ownership enforces single-user access
   - No data races on connection state

4. **IDE Support**
   - Autocomplete shows only valid methods
   - Type errors point to invalid operations

## Comparison with Other Drivers

| Feature | rust-mssql-driver | Tiberius | sqlx |
|---------|-------------------|----------|------|
| Type-state | Yes (compile-time) | No | No |
| Transaction safety | Compile-time | Runtime | Runtime |
| State transitions | Explicit | Implicit | Implicit |
| Invalid op detection | Compile error | Runtime error | Runtime error |

## Trade-offs

### Advantages
- Impossible to misuse the API
- Documentation in types
- Zero runtime overhead for state checks

### Disadvantages
- More complex type signatures
- Harder to store in collections
- Learning curve for users unfamiliar with pattern

## When to Use Type-State

Use type-state when:
- State-dependent operations are common
- Invalid state usage causes bugs
- You want compile-time guarantees

Consider alternatives when:
- States are highly dynamic
- You need to store many connections in heterogeneous collections
- Simplicity is more important than safety
