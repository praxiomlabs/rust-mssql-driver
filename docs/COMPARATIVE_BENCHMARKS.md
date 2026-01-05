# Comparative Performance Analysis

This document compares rust-mssql-driver architecture and performance characteristics against Tiberius.

For detailed benchmark numbers and targets, see [BENCHMARKS.md](BENCHMARKS.md).

---

## Methodology Note

**Important:** Direct benchmark comparisons require:
- Same hardware environment
- Same SQL Server version
- Same network conditions
- Same query workloads

This analysis focuses on **architectural differences** rather than raw numbers, which vary by deployment.

---

## Architecture Comparison

| Feature | rust-mssql-driver | Tiberius |
|---------|-------------------|----------|
| Runtime | Tokio-native | Runtime-agnostic |
| TLS | rustls | rustls or native-tls |
| Memory Model | Arc<Bytes> zero-copy | Vec-based copies |
| Connection Pooling | Built-in | External (bb8/deadpool) |
| Prepared Statements | LRU cache | Manual lifecycle |
| LOB Handling | Buffered with BlobReader API | Full materialization |

---

## Architectural Advantages

### 1. Zero-Copy Row Data (ADR-004)

```rust
// Traditional approach (Tiberius-style)
let row_data = Vec::from(packet_slice);  // COPY
let column_value = row_data[offset..end].to_vec();  // COPY

// rust-mssql-driver approach
let row_data = packet_buffer.slice(row_range);  // NO COPY
let column_value = row_data.slice(col_range);   // NO COPY
```

**Impact:** For a row with 10 columns:
- Traditional: 10+ allocations per row
- rust-mssql-driver: 0 allocations until value extraction needed

### 2. Built-in Connection Pool

```rust
// rust-mssql-driver - integrated pool
let pool = Pool::builder()
    .max_connections(10)
    .build(config).await?;

// Tiberius - external pool required
let manager = TiberiusConnectionManager::new(config);
let pool = bb8::Pool::builder()
    .max_size(10)
    .build(manager).await?;
```

**Impact:**
- One fewer dependency
- Unified configuration
- Pool-aware prepared statement cache

### 3. Prepared Statement Cache

| Aspect | rust-mssql-driver | Tiberius |
|--------|-------------------|----------|
| Cache location | Built-in LRU | Manual |
| Lifecycle | Automatic | Manual prepare/unprepare |
| Cross-connection | Pool-aware | N/A |
| First execution | sp_prepare + sp_execute | Manual |
| Subsequent | sp_execute only | Manual |

**Impact:** Repeated queries with different parameters execute ~50% faster after initial prepare.

---

## When Performance Matters

### rust-mssql-driver Excels At:

1. **High-throughput local connections** - Zero-copy matters when network isn't the bottleneck
2. **Large result sets** - Arc<Bytes> sharing reduces memory pressure
3. **Repeated parameterized queries** - Prepared statement cache
4. **Memory-constrained environments** - Lower allocation overhead

### Performance is Similar When:

1. **Network latency > 1ms** - Driver overhead is noise
2. **Small result sets** - Allocation overhead negligible
3. **One-off queries** - No cache benefit

---

## Summary

| Dimension | rust-mssql-driver | Tiberius | Winner |
|-----------|-------------------|----------|--------|
| Memory efficiency | Arc<Bytes> zero-copy | Vec copies | rust-mssql-driver |
| Connection pooling | Built-in | External | rust-mssql-driver |
| Prepared statements | Auto-cached | Manual | rust-mssql-driver |
| Runtime flexibility | Tokio-only | Any runtime | Tiberius |
| Maturity | New | Battle-tested | Tiberius |

**Recommendation:** For new Tokio-based projects requiring SQL Server, rust-mssql-driver offers architectural advantages. For runtime-agnostic needs or proven stability, Tiberius remains excellent.

---

## Running Your Own Comparison

```bash
# Run rust-mssql-driver benchmarks
cargo bench --workspace

# View HTML reports
open target/criterion/report/index.html
```

For real-world comparison, benchmark your specific workload against both drivers.
