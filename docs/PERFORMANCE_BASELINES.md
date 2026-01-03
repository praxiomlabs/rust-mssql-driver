# Performance Baselines

This document establishes performance baselines for rust-mssql-driver operations.

## Measurement Methodology

Benchmarks are run using [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) with:

- **Warm-up:** 3 seconds
- **Measurement:** 5 seconds per benchmark
- **Sample size:** 100 iterations minimum
- **Statistical analysis:** Bootstrap confidence intervals

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench --workspace

# Run specific crate benchmarks
cargo bench --package mssql-client
cargo bench --package mssql-types
cargo bench --package tds-protocol

# Save baseline
cargo bench --workspace -- --save-baseline v0.5.0

# Compare against baseline
cargo bench --workspace -- --baseline v0.5.0
```

---

## Client Operations

### Connection String Parsing

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Simple connection string | < 2 μs | < 5 μs | `Server=host;Database=db;...` |
| With port | < 2 μs | < 5 μs | `Server=host,1433;...` |
| With named instance | < 3 μs | < 6 μs | `Server=host\INSTANCE;...` |
| Full Azure string | < 5 μs | < 10 μs | All options specified |

**Benchmark:** `cargo bench --package mssql-client -- connection_string`

### Config Builder

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Minimal config | < 500 ns | < 1 μs | Host + database only |
| Full config | < 1 μs | < 2 μs | All options set |

**Benchmark:** `cargo bench --package mssql-client -- config_builder`

---

## Type Conversions

### FromSql Conversions

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| i32 from INT | < 10 ns | < 50 ns | Direct conversion |
| i64 from BIGINT | < 10 ns | < 50 ns | Direct conversion |
| String from NVARCHAR | < 100 ns | < 500 ns | Includes allocation |
| Option<T> from non-null | < 15 ns | < 50 ns | Wrapper overhead |
| Option<T> from NULL | < 5 ns | < 20 ns | Short-circuit |
| f64 from FLOAT | < 10 ns | < 50 ns | Direct conversion |
| bool from BIT | < 5 ns | < 20 ns | Trivial conversion |

**Benchmark:** `cargo bench --package mssql-client -- from_sql`

### SqlValue Operations

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Create Int | < 5 ns | < 20 ns | Stack allocation |
| Create String | < 50 ns | < 200 ns | Heap allocation |
| Create Null | < 2 ns | < 10 ns | Zero-size |
| is_null check | < 2 ns | < 10 ns | Pattern match |

**Benchmark:** `cargo bench --package mssql-client -- sql_value`

---

## Memory Operations

### Arc<Bytes> Pattern (ADR-004)

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Clone small (< 100 bytes) | < 5 ns | < 20 ns | Reference count only |
| Clone medium (1 KB) | < 5 ns | < 20 ns | Reference count only |
| Clone large (100 KB) | < 5 ns | < 20 ns | Reference count only |
| Slice access | < 5 ns | < 20 ns | Zero-copy |

**Key insight:** Arc clone time is constant regardless of buffer size.

**Benchmark:** `cargo bench --package mssql-client -- arc_bytes`

---

## Protocol Operations

### TDS Encoding

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Encode small packet | < 500 ns | < 1 μs | < 1 KB payload |
| Encode medium packet | < 2 μs | < 5 μs | ~4 KB payload |
| Encode large packet | < 10 μs | < 25 μs | ~32 KB payload |

**Benchmark:** `cargo bench --package tds-protocol -- encode`

### TDS Decoding

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Decode row token | < 200 ns | < 500 ns | Per row |
| Decode column metadata | < 100 ns | < 300 ns | Per column |
| Decode result set | < 1 μs | < 3 μs | Complete parse |

**Benchmark:** `cargo bench --package tds-protocol -- decode`

---

## Network Operations

### Connection Establishment

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| TCP connect (localhost) | < 1 ms | < 5 ms | OS dependent |
| TLS handshake | < 10 ms | < 50 ms | Certificate validation |
| Full login sequence | < 50 ms | < 200 ms | TDS login7 + response |

**Note:** Network operations are inherently variable. These targets assume localhost or low-latency network.

### Query Execution

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Simple SELECT 1 | < 1 ms | < 5 ms | Minimal round-trip |
| Parameterized query | < 2 ms | < 10 ms | sp_executesql overhead |
| First row time | < 5 ms | < 20 ms | Time to first row |

---

## Pool Operations

### Connection Pool

| Operation | Target | Acceptable | Notes |
|-----------|--------|------------|-------|
| Acquire (available) | < 50 μs | < 200 μs | From idle pool |
| Acquire (create new) | < 100 ms | < 500 ms | Full connection |
| Release | < 10 μs | < 50 μs | Return to pool |
| Health check | < 1 ms | < 5 ms | SELECT 1 validation |

---

## Latency Percentiles

For production monitoring, track these percentiles:

| Percentile | Description | Alert Threshold |
|------------|-------------|-----------------|
| p50 | Median latency | 2x target |
| p95 | 95th percentile | 5x target |
| p99 | 99th percentile | 10x target |
| p99.9 | Tail latency | 20x target |

### Example: Query Latency Targets

| Percentile | Simple Query | Complex Query |
|------------|--------------|---------------|
| p50 | 1 ms | 10 ms |
| p95 | 5 ms | 50 ms |
| p99 | 20 ms | 200 ms |
| p99.9 | 100 ms | 1 s |

---

## Regression Detection

### CI Integration

The CI pipeline runs benchmarks and fails if:

1. Any benchmark regresses by > 20%
2. Memory usage increases significantly
3. New allocations appear in hot paths

### Investigating Regressions

```bash
# Compare against baseline
cargo bench --workspace -- --baseline main

# Generate flamegraph
cargo flamegraph --bench client -- --bench

# Profile with perf
perf record cargo bench --package mssql-client
perf report
```

---

## Hardware Reference

Baseline measurements were taken on:

- **CPU:** AMD Ryzen 9 5900X / Apple M1 Pro
- **Memory:** 32 GB DDR4-3200 / 16 GB LPDDR5
- **OS:** Ubuntu 22.04 / macOS 14
- **Rust:** 1.85.0

Adjust expectations for different hardware configurations.

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.5.0 | 2026-01-01 | Initial baselines established |

---

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [ADR-004: Arc<Bytes> Pattern](../ARCHITECTURE.md)
