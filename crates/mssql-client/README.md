# mssql-client

High-level async SQL Server client with type-state connection management.

## Overview

This is the primary public API surface for the rust-mssql-driver project. It provides a type-safe, ergonomic interface for working with SQL Server databases using modern async Rust patterns.

## Features

- **Type-state pattern**: Compile-time enforcement of connection states
- **Async/await**: Built on Tokio for efficient async I/O
- **Prepared statements**: Automatic caching with LRU eviction
- **Transactions**: Full transaction support with savepoints
- **Azure support**: Automatic routing and failover handling
- **Streaming results**: Memory-efficient processing of large result sets
- **Bulk insert**: High-performance bulk data loading
- **Table-valued parameters**: Pass collections to stored procedures

## Type-State Connection Management

The client uses a compile-time type-state pattern that ensures invalid operations are caught at compile time:

```text
Disconnected -> Ready (via connect())
Ready -> InTransaction (via begin_transaction())
Ready -> Streaming (via query that returns a stream)
InTransaction -> Ready (via commit() or rollback())
```

## Usage

```rust
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=test;User Id=sa;Password=Password123;"
    )?;

    let mut client = Client::connect(config).await?;

    // Execute a query with parameters
    let rows = client
        .query("SELECT * FROM users WHERE id = @p1", &[&1])
        .await?;

    for row in rows {
        let name: String = row.get(0)?;
        println!("User: {}", name);
    }

    Ok(())
}
```

## Transactions with Savepoints

```rust
let mut tx = client.begin_transaction().await?;
tx.execute("INSERT INTO users (name) VALUES (@p1)", &[&"Alice"]).await?;

// Create a savepoint for partial rollback
let sp = tx.save_point("before_update").await?;
tx.execute("UPDATE users SET active = 1", &[]).await?;

// Rollback to savepoint if needed
// tx.rollback_to(&sp).await?;

tx.commit().await?;
```

## Streaming Large Results

```rust
use futures::StreamExt;

let mut stream = client
    .query_stream("SELECT * FROM large_table", &[])
    .await?;

while let Some(row) = stream.next().await {
    let row = row?;
    // Process row without loading entire result into memory
}
```

## Bulk Insert

```rust
use mssql_client::{BulkInsert, BulkColumn};

let bulk = BulkInsert::builder("dbo.users")
    .column(BulkColumn::new("id", "INT"))
    .column(BulkColumn::new("name", "NVARCHAR(100)"))
    .build();

let result = client.bulk_insert(bulk, rows).await?;
println!("Inserted {} rows", result.rows_affected);
```

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `chrono` | Yes | Date/time type support via chrono |
| `uuid` | Yes | UUID type support |
| `decimal` | Yes | Decimal type support via rust_decimal |
| `encoding` | Yes | Collation-aware VARCHAR decoding |
| `json` | No | JSON type support via serde_json |
| `otel` | No | OpenTelemetry instrumentation |
| `zeroize` | No | Secure credential wiping |
| `always-encrypted` | No | Client-side encryption with key providers |

## Modules

| Module | Description |
|--------|-------------|
| `client` | Main `Client` type and connection management |
| `config` | Connection configuration and parsing |
| `query` | Query building and execution |
| `row` | Row and column access |
| `stream` | Result streaming types |
| `transaction` | Transaction and savepoint handling |
| `bulk` | Bulk insert operations |
| `tvp` | Table-valued parameters |
| `from_row` | Row-to-struct mapping trait |
| `to_params` | Struct-to-parameters trait |
| `statement_cache` | Prepared statement caching |
| `instrumentation` | OpenTelemetry integration |

## Examples

See the `examples/` directory for complete examples:

- `basic.rs` - Simple queries and parameter binding
- `transactions.rs` - Transaction handling with savepoints
- `bulk_insert.rs` - High-performance bulk loading
- `derive_macros.rs` - Using `#[derive(FromRow)]` and `#[derive(ToParams)]`
- `streaming.rs` - Processing large result sets

## License

MIT OR Apache-2.0
