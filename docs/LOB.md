# Large Object (LOB) Handling

This document describes how rust-mssql-driver handles Large Objects (LOBs) such as `VARCHAR(MAX)`, `NVARCHAR(MAX)`, `VARBINARY(MAX)`, and `XML`.

## Supported LOB Types

| SQL Server Type | Rust Type | Max Size | Notes |
|-----------------|-----------|----------|-------|
| `VARCHAR(MAX)` | `String` | 2 GB | UTF-8 converted from server encoding |
| `NVARCHAR(MAX)` | `String` | 2 GB | UTF-16LE decoded to UTF-8 |
| `VARBINARY(MAX)` | `Bytes` | 2 GB | Raw binary data |
| `TEXT` | `String` | 2 GB | Legacy, prefer VARCHAR(MAX) |
| `NTEXT` | `String` | 2 GB | Legacy, prefer NVARCHAR(MAX) |
| `IMAGE` | `Bytes` | 2 GB | Legacy, prefer VARBINARY(MAX) |
| `XML` | `String` | 2 GB | UTF-16LE decoded to UTF-8 |

## Current Implementation

### In-Memory Loading

LOBs are currently loaded entirely into memory when accessed:

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let rows = client.query("SELECT bin, txt, xml FROM lobs", &[]).await?;
    for row in rows {
        let row = row?;

        // Reading a VARBINARY(MAX) column
        let binary_data: Vec<u8> = row.get(0)?;

        // Reading a VARCHAR(MAX) column
        let text_data: String = row.get(1)?;

        // Reading XML
        let xml_data: String = row.get(2)?;
        let _ = (binary_data, text_data, xml_data);
    }
    Ok(())
}
```

### Memory Characteristics

```text
┌─────────────────────────────────────────────────────────────┐
│                   TDS Response Stream                        │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              LOB Data (chunked)                      │   │
│  │  [Chunk 1] [Chunk 2] [Chunk 3] ... [Chunk N]        │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ Reassembly
┌─────────────────────────────────────────────────────────────┐
│                   Complete LOB in Memory                     │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Bytes / String                    │   │
│  │              (up to 2 GB allocation)                 │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Current behavior:**
1. TDS chunks are received from the network
2. Chunks are reassembled into a single buffer
3. Complete LOB is returned to the caller
4. Memory is held until the value is dropped

### Memory Implications

| LOB Size | Memory Usage | Risk |
|----------|--------------|------|
| < 1 MB | Low | None |
| 1-100 MB | Moderate | Monitor memory |
| 100 MB - 1 GB | High | OOM possible |
| > 1 GB | Very High | OOM likely |

## Usage Patterns

### Reading LOBs

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let file_id = 1i32;

    // Simple LOB reading (QueryStream is a synchronous iterator)
    let mut stream = client.query(
        "SELECT document FROM files WHERE id = @p1",
        &[&file_id],
    ).await?;

    if let Some(row) = stream.next() {
        let row = row?;
        let document: Vec<u8> = row.get(0)?;
        // Process document...
        let _ = document;
    }
    Ok(())
}
```

### Streaming LOBs

For large binary data, use `get_stream()` to get a `BlobReader` that implements `AsyncRead`:

```rust,no_run
use mssql_client::{Client, Config};
use tokio::io::copy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let file_id = 1i32;

    let mut stream = client.query(
        "SELECT data FROM files WHERE id = @p1",
        &[&file_id],
    ).await?;

    if let Some(row) = stream.next() {
        let row = row?;

        // Get streaming reader for the VARBINARY(MAX) column
        if let Some(mut reader) = row.get_stream(0) {
            // Stream directly to any AsyncWrite sink
            let mut sink: Vec<u8> = Vec::new();
            copy(&mut reader, &mut sink).await?;
        }
    }
    Ok(())
}
```

The `BlobReader` provides:
- `len()` - Total size (for progress tracking)
- `bytes_read()` - Bytes consumed so far
- `remaining()` - Bytes left to read
- `rewind()` - Reset position for re-reading

### Writing LOBs

```rust,no_run
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;

    // Insert binary data
    let data: Vec<u8> = vec![0u8; 1024];
    client.execute(
        "INSERT INTO files (name, data) VALUES (@p1, @p2)",
        &[&"file.bin", &data.as_slice()],
    ).await?;

    // Insert text data
    let text: String = "large document contents".to_string();
    client.execute(
        "INSERT INTO documents (title, content) VALUES (@p1, @p2)",
        &[&"My Document", &text.as_str()],
    ).await?;
    Ok(())
}
```

### NULL Handling

```text
// LOBs can be NULL
let maybe_data: Option<Bytes> = row.get(0)?;

match maybe_data {
    Some(data) => process_data(data),
    None => handle_null(),
}
```

## Best Practices

### Memory Management

```text
// Process and drop promptly
{
    let data: Bytes = row.get(0)?;
    write_to_file(&data)?;
} // data dropped, memory freed

// Don't hold multiple large LOBs
let all_files: Vec<Bytes> = rows
    .iter()
    .map(|r| r.get::<Bytes>(0).unwrap())
    .collect();  // OOM risk!
```

### Chunked Processing

For large LOBs, consider application-level chunking:

```sql
-- Store data in chunks
CREATE TABLE file_chunks (
    file_id INT,
    chunk_index INT,
    chunk_data VARBINARY(MAX),  -- Each chunk < 10MB
    PRIMARY KEY (file_id, chunk_index)
);
```

```rust,no_run
use mssql_client::{Client, Config};
use std::fs::File;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=db;User Id=sa;Password=Password123",
    )?;
    let mut client = Client::connect(config).await?;
    let file_id = 1i32;

    // Read in chunks
    let mut stream = client.query(
        "SELECT chunk_data FROM file_chunks WHERE file_id = @p1 ORDER BY chunk_index",
        &[&file_id],
    ).await?;

    let mut file = File::create("output.bin")?;
    while let Some(row) = stream.next() {
        let chunk: Vec<u8> = row?.get(0)?;
        file.write_all(&chunk)?;
        // chunk dropped, memory freed
    }
    Ok(())
}
```

### Size Limits

Protect against memory exhaustion:

```text
// Check size before loading
let size: i64 = row.get::<i64>("data_length")?;
if size > MAX_ALLOWED_SIZE {
    return Err(Error::TooLarge);
}
let data: Bytes = row.get("data")?;
```

Or use SQL Server to limit:

```sql
SELECT
    CASE
        WHEN DATALENGTH(data) > 100000000 THEN NULL  -- > 100MB
        ELSE data
    END as data
FROM large_table
```

## Limitations

### Current Limitations

| Limitation | Impact | Workaround |
|------------|--------|------------|
| No streaming read | Memory usage | Chunk at application level |
| No streaming write | Memory usage | Chunk at application level |
| Full materialization | Latency for large LOBs | Pre-check sizes |

### Size Constraints

| Constraint | Limit | Notes |
|------------|-------|-------|
| Max LOB size | 2 GB | SQL Server limit |
| Practical memory | System RAM | Watch for OOM |
| Network transfer | Variable | Large LOBs are slow |

## TDS Protocol Details

### LOB Encoding

LOBs use partial length prefixed (PLP) encoding in TDS:

```text
┌────────────────────────────────────────────────────┐
│                   PLP Format                        │
├────────────────────────────────────────────────────┤
│ Total Length (8 bytes)                             │
│   0xFFFFFFFFFFFFFFFE = Unknown length              │
│   0xFFFFFFFFFFFFFFFF = NULL                        │
│   Other = Exact total length                       │
├────────────────────────────────────────────────────┤
│ Chunk 1:                                           │
│   Length (4 bytes) + Data                          │
├────────────────────────────────────────────────────┤
│ Chunk 2:                                           │
│   Length (4 bytes) + Data                          │
├────────────────────────────────────────────────────┤
│ ...                                                │
├────────────────────────────────────────────────────┤
│ Terminator: Length = 0                             │
└────────────────────────────────────────────────────┘
```

### Chunk Sizes

SQL Server sends LOBs in chunks:
- Default chunk size: ~8000 bytes
- Chunk boundaries don't align with character boundaries
- UTF-16 characters may span chunks (handled by driver)

## BlobReader: Streaming API

The `BlobReader` type provides an `AsyncRead` interface for processing LOB data in chunks:

```rust,no_run
use mssql_client::blob::BlobReader;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, copy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Binary data, e.g. obtained via `row.get::<Vec<u8>>(0)?`
    let data: Bytes = Bytes::from(vec![0u8; 16384]);

    // Create a streaming reader
    let mut reader = BlobReader::from_bytes(data);

    // Option 1: Read in chunks
    let mut buffer = vec![0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 {
            break; // EOF
        }
        let _chunk = &buffer[..n];
    }

    // Option 2: Stream directly to any AsyncWrite sink
    reader.rewind();
    let mut sink: Vec<u8> = Vec::new();
    copy(&mut reader, &mut sink).await?;
    Ok(())
}
```

### BlobReader Features

| Feature | Description |
|---------|-------------|
| `len()` | Get total BLOB size |
| `bytes_read()` | Track progress |
| `remaining()` | Bytes left to read |
| `rewind()` | Reset to beginning |
| `is_exhausted()` | Check if fully read |
| `into_bytes()` | Consume and get underlying data |

### Memory Model

The current `BlobReader` implementation buffers the complete LOB internally (received as `Bytes` from SQL Server), but provides the streaming API. This enables:

- **Chunked processing** without additional allocations per read
- **Streaming to files** or other destinations via `AsyncRead`
- **Progress tracking** during large object processing
- **Compatible API** for future true-streaming implementation

Future versions may implement on-the-fly streaming from the TDS protocol layer

## Performance Considerations

### Read Performance

| LOB Size | Approx Time (1 Gbps) | Memory |
|----------|---------------------|---------|
| 1 MB | ~10ms | 1 MB |
| 10 MB | ~100ms | 10 MB |
| 100 MB | ~1s | 100 MB |
| 1 GB | ~10s | 1 GB |

### Write Performance

Writing large LOBs:
- Network is usually the bottleneck
- TDS chunking adds minimal overhead
- Parameter binding is efficient

### Optimization Tips

1. **Index LOB columns carefully** - Full LOB scans are slow
2. **Use filestream for very large files** - SQL Server FILESTREAM stores on disk
3. **Compress if possible** - Reduce network and memory
4. **Batch small LOBs** - Amortize network round-trips

## Comparison with Other Drivers

| Feature | rust-mssql-driver | Tiberius | ODBC |
|---------|-------------------|----------|------|
| In-memory LOB | Yes | Yes | Yes |
| Streaming read API | Yes (BlobReader) | No | Yes |
| Streaming write | Planned | No | Yes |
| Max size | 2 GB | 2 GB | 2 GB |
| Zero-copy slice | Yes | Partial | No |
| Progress tracking | Yes | No | Partial |
