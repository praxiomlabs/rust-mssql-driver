# Memory Allocation Patterns

This document describes the memory allocation strategies used in rust-mssql-driver for optimal performance.

## Design Philosophy

rust-mssql-driver prioritizes:

1. **Zero-copy where possible** - Avoid copying data between buffers
2. **Shared ownership** - Use `Arc<Bytes>` to share row data
3. **Predictable allocation** - Avoid unbounded growth
4. **Cache-friendly access** - Contiguous memory for row data

## Row Data: Arc<Bytes> Pattern

### How It Works

When rows are received from SQL Server:

```text
┌─────────────────────────────────────────────────────────────┐
│                    Network Buffer                            │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┐  │
│  │  Row 1   │  Row 2   │  Row 3   │  Row 4   │  Row 5   │  │
│  └──────────┴──────────┴──────────┴──────────┴──────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ Bytes::copy_from_slice
┌─────────────────────────────────────────────────────────────┐
│                    Arc<Bytes> (shared)                       │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┐  │
│  │  Row 1   │  Row 2   │  Row 3   │  Row 4   │  Row 5   │  │
│  └────┬─────┴────┬─────┴────┬─────┴────┬─────┴────┬─────┘  │
└───────┼──────────┼──────────┼──────────┼──────────┼────────┘
        │          │          │          │          │
        ▼          ▼          ▼          ▼          ▼
   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
   │ Row 1  │ │ Row 2  │ │ Row 3  │ │ Row 4  │ │ Row 5  │
   │ slice  │ │ slice  │ │ slice  │ │ slice  │ │ slice  │
   └────────┘ └────────┘ └────────┘ └────────┘ └────────┘
```

Each `Row` holds:
- `Arc<Bytes>` reference to shared buffer
- Offset and length for its slice
- Column metadata (offsets within row)

### Benefits

| Aspect | Benefit |
|--------|---------|
| Memory usage | Single allocation for multiple rows |
| Copy overhead | Zero copies after initial receive |
| Thread safety | `Arc` enables safe sharing |
| Cache locality | Contiguous data for sequential access |

### Code Pattern

```text
// Internal implementation (simplified)
pub struct Row {
    data: Arc<Bytes>,       // Shared buffer
    offset: usize,          // Start of this row
    length: usize,          // Length of this row
    columns: Vec<ColumnMeta>, // Column positions
}

impl Row {
    pub fn get<T: FromSql>(&self, index: usize) -> Result<T, Error> {
        let col = &self.columns[index];
        // Slice into shared buffer - no copy
        let slice = &self.data[self.offset + col.offset..][..col.length];
        T::from_bytes(slice)
    }
}
```

### Memory Lifecycle

```text
1. TDS packet received → BytesMut (tokio buffer)
2. Packet parsed → Bytes::freeze() (immutable, refcounted)
3. Row created → Arc<Bytes> clone (ref increment, no copy)
4. Row accessed → Slice reference (no allocation)
5. Row dropped → Arc refcount decremented
6. Last Row dropped → Bytes deallocated
```

## String Handling

### UTF-16 to UTF-8 Conversion

SQL Server uses UTF-16LE for strings. Conversion happens on access:

```rust
// When you call row.get::<String>(0)?
// 1. Slice UTF-16 bytes from Arc<Bytes> (no copy)
// 2. Decode to Rust String (allocation)
// 3. Return owned String
```

**Allocation:** One `String` allocation per column access.

**Optimization:** For repeated access, store the result:

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let rows = client.query("SELECT name FROM users", &[]).await?;
    for row in rows {
        let row = row?;

        // Inefficient - decodes twice
        let name1 = row.get::<String>(0)?;
        let name2 = row.get::<String>(0)?;
        let _ = (name1, name2);

        // Efficient - decode once, clone if needed
        let name = row.get::<String>(0)?;
        let name_copy = name.clone();
        let _ = (name, name_copy);
    }
    Ok(())
}
```

### Binary Data

Binary columns return `Bytes` which shares the underlying buffer:

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let rows = client.query("SELECT data FROM files", &[]).await?;
    for row in rows {
        let row = row?;
        let data: Vec<u8> = row.get(0)?;
        let _ = data;
    }
    Ok(())
}
```

## Connection Pool Memory

### Pool Structure

```text
┌─────────────────────────────────────────────┐
│                    Pool                      │
│  ┌─────────────────────────────────────┐   │
│  │        Connections Vec              │   │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  │   │
│  │  │Conn1│ │Conn2│ │Conn3│ │Conn4│  │   │
│  │  └─────┘ └─────┘ └─────┘ └─────┘  │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │      Availability Semaphore         │   │
│  │         (capacity: 4)               │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │       Statement Cache (LRU)         │   │
│  │  capacity: 100 statements           │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

### Per-Connection Memory

| Component | Typical Size | Notes |
|-----------|--------------|-------|
| TLS state | ~50KB | Cached session |
| Read buffer | 4KB-64KB | Packet size |
| Write buffer | 4KB-64KB | Packet size |
| Statement cache | ~10KB | Per 100 statements |
| Metadata cache | ~5KB | Column definitions |

**Estimate:** ~100-200KB per connection.

### Pool Sizing

```rust
// Memory estimate for pool
let connections = 20;
let per_conn_kb = 150;
let pool_overhead_kb = 50;

let total_kb = connections * per_conn_kb + pool_overhead_kb;
// 20 * 150 + 50 = 3050 KB ≈ 3 MB
```

## Statement Cache

### LRU Eviction

Prepared statements are cached to avoid repeated `sp_prepare` calls:

```text
┌─────────────────────────────────────────┐
│           Statement Cache               │
│  ┌───────────────────────────────────┐ │
│  │ LRU Queue (most recent at head)   │ │
│  │                                   │ │
│  │  [stmt5] → [stmt3] → [stmt1] →   │ │
│  │  [stmt4] → [stmt2]               │ │
│  │                                   │ │
│  └───────────────────────────────────┘ │
│                                         │
│  On eviction: sp_unprepare called      │
└─────────────────────────────────────────┘
```

### Memory per Statement

| Component | Size | Notes |
|-----------|------|-------|
| SQL hash | 8 bytes | FxHash |
| Handle | 4 bytes | Server-assigned |
| SQL text | Variable | Stored for debugging |
| Metadata | ~200 bytes | Parameter types |

**Typical:** ~500 bytes per cached statement.

### Cache Configuration

```text
let config = StatementCacheConfig {
    capacity: 100,      // Max statements
    // Memory: ~50KB for 100 statements
};
```

## Streaming Large Results

### Buffered vs Streaming

**Buffered (default for small results):**

```rust,no_run
use mssql_client::{Client, Config, Row};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let stream = client.query("SELECT * FROM t", &[]).await?;

    // Collects all rows into memory
    let rows: Vec<Row> = stream.collect_all().await?;
    // Memory: O(total_data_size)
    let _ = rows;
    Ok(())
}
```

**Streaming (for large results):**

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let mut stream = client.query("SELECT * FROM t", &[]).await?;

    // Process one row at a time (QueryStream is a synchronous iterator)
    while let Some(row) = stream.next() {
        let row = row?;
        let _ = row;
        // row dropped here, memory freed
    }
    // Memory: O(packet_size) ≈ 4-64KB
    Ok(())
}
```

### Memory Comparison

| Approach | 1M rows × 1KB | Peak Memory |
|----------|---------------|-------------|
| Buffered | `collect_all()` | ~1 GB |
| Streaming | `while let` | ~64 KB |

### Recommendation

```text
// Small results (< 10K rows): Buffer for convenience
let rows = stream.collect_all().await?;

// Large results: Stream to avoid OOM
while let Some(row) = stream.next().await {
    process(row?);
}
```

## Large Objects (LOBs)

### Current Implementation

LOBs (VARCHAR(MAX), VARBINARY(MAX), etc.) are currently loaded into memory:

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let rows = client.query("SELECT data FROM lobs", &[]).await?;
    for row in rows {
        let row = row?;
        // Loads entire LOB into memory
        let data: Vec<u8> = row.get(0)?;
        // Memory: O(lob_size)
        let _ = data;
    }
    Ok(())
}
```

### Memory Limits

| LOB Type | Max Size | Notes |
|----------|----------|-------|
| VARCHAR(MAX) | 2 GB | As String |
| NVARCHAR(MAX) | 2 GB | As String (UTF-16 decoded) |
| VARBINARY(MAX) | 2 GB | As Bytes |
| XML | 2 GB | As String |

**Warning:** Large LOBs can cause OOM. For LOBs > 100MB, consider:
- Application-level chunking
- File storage with path in database
- Waiting for streaming LOB support (planned)

## Memory Profiling

### Using DHAT

```bash
cargo install dhat

# In Cargo.toml:
# [features]
# dhat = ["dep:dhat"]

# Run with profiling
cargo run --features dhat
```

### Allocation Hotspots

Typical allocation sources:

| Source | Frequency | Mitigation |
|--------|-----------|------------|
| String decoding | Per column | Cache decoded values |
| Row collection | Per row | Use streaming |
| Connection creation | Per connect | Use pool |
| Statement prepare | Per unique SQL | Statement cache |

## Best Practices

### Do

```text
// ✓ Use connection pool
let conn = pool.get().await?;

// ✓ Stream large results
while let Some(row) = stream.next().await { ... }

// ✓ Drop connections promptly
{
    let conn = pool.get().await?;
    // use conn
} // dropped here

// ✓ Reuse queries (enables statement caching)
for id in ids {
    conn.query("SELECT * FROM t WHERE id = @p1", &[&id]).await?;
}
```

### Don't

```text
// ✗ Create new connections per query
let conn = Client::connect(config).await?;  // Expensive!

// ✗ Collect huge result sets
let million_rows = stream.collect_all().await?;  // OOM risk

// ✗ Hold connections during slow operations
let conn = pool.get().await?;
slow_external_api_call().await;  // Blocks pool
conn.query(...).await?;

// ✗ Different SQL strings for same query
conn.query(&format!("SELECT * FROM t WHERE id = {}", id), &[]).await?;  // No caching!
```

## Tuning Parameters

| Parameter | Default | Effect on Memory |
|-----------|---------|------------------|
| `max_connections` | 10 | ~150KB per connection |
| `packet_size` | 4096 | Buffer size |
| `statement_cache_size` | 100 | ~500 bytes per statement |

### Low-Memory Configuration

```text
let pool = Pool::builder()
    .client_config(config)
    .max_connections(5)           // Fewer connections
    .min_connections(1)           // Don't pre-allocate
    .statement_cache_size(20)     // Smaller cache
    .build()
    .await?;
```

### High-Throughput Configuration

```text
let pool = Pool::builder()
    .client_config(config)
    .max_connections(50)          // More connections
    .min_connections(10)          // Pre-warm
    .statement_cache_size(500)    // Larger cache
    .build()
    .await?;
```
