# Stress Testing Guide

This document describes how to run stress tests and memory analysis on rust-mssql-driver.

## Overview

Stress testing helps identify:

- Memory leaks
- Performance degradation under load
- Resource exhaustion issues
- Concurrency bugs

## Running Stress Tests

### Basic Stress Tests (No SQL Server)

```bash
# Run all non-ignored stress tests
cargo test --package mssql-client --test stress_tests

# Run with verbose output
cargo test --package mssql-client --test stress_tests -- --nocapture
```

### Live Stress Tests (Requires SQL Server)

```bash
# Set environment variables
export MSSQL_TEST_HOST=localhost
export MSSQL_TEST_PORT=1433
export MSSQL_TEST_USER=sa
export MSSQL_TEST_PASSWORD=YourPassword123!

# Run all stress tests including live ones
cargo test --package mssql-client --test stress_tests -- --ignored --nocapture
```

## Memory Leak Detection

### Using Miri

Miri is Rust's official undefined behavior detector. It can find memory leaks in safe and unsafe code.

```bash
# Install Miri (requires nightly)
rustup +nightly component add miri

# Run tests under Miri
cargo +nightly miri test --package mssql-client

# Run specific stress tests under Miri
cargo +nightly miri test --package mssql-client --test stress_tests
```

**Note:** Miri is slow but thorough. Use it for targeted testing.

### Using Valgrind

Valgrind provides detailed memory analysis including leak detection.

```bash
# Install cargo-valgrind
cargo install cargo-valgrind

# Run stress tests under Valgrind
cargo valgrind test --package mssql-client --test stress_tests

# With full leak check
valgrind --leak-check=full \
    --show-leak-kinds=all \
    --track-origins=yes \
    target/debug/deps/stress_tests-*
```

### Using DHAT (Heap Profiler)

DHAT provides heap allocation profiling.

```bash
# Install dhat feature and run
cargo test --package mssql-client --test stress_tests --features dhat-heap

# Or manually with valgrind's dhat tool
valgrind --tool=dhat target/debug/deps/stress_tests-*
```

## Performance Benchmarks

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench --workspace

# Run specific benchmark groups
cargo bench --package mssql-client -- connection_string
cargo bench --package mssql-types -- types
cargo bench --package tds-protocol -- protocol
```

### Benchmark Output

Benchmark results are saved to `target/criterion/`. View HTML reports:

```bash
# After running benchmarks
open target/criterion/report/index.html
```

### Establishing Baselines

```bash
# Save baseline for comparison
cargo bench --workspace -- --save-baseline main

# Compare against baseline after changes
cargo bench --workspace -- --baseline main
```

## Concurrent Load Testing

### Pool Stress Test

Tests connection pool behavior under concurrent load:

```bash
# Requires SQL Server
MSSQL_TEST_HOST=localhost \
MSSQL_TEST_PASSWORD=YourPassword123! \
cargo test --package mssql-client --test stress_tests stress_concurrent_pool_usage -- --ignored --nocapture
```

### Custom Load Test

Create a simple load test script:

```rust
use mssql_driver_pool::{Pool, PoolConfig};
use mssql_client::Config;
use std::time::{Duration, Instant};
use tokio;

#[tokio::main]
async fn main() {
    let config = Config::from_connection_string(
        "Server=localhost;Database=test;User Id=sa;Password=secret;TrustServerCertificate=true"
    ).unwrap();

    let pool = Pool::builder()
        .min_size(5)
        .max_size(20)
        .build(config)
        .await
        .unwrap();

    let start = Instant::now();
    let mut handles = Vec::new();

    // Spawn 100 concurrent tasks
    for _ in 0..100 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let mut conn = pool.get().await.unwrap();
                conn.query("SELECT 1", &[]).await.unwrap();
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!("Completed 10,000 queries in {:?}", start.elapsed());
}
```

## Interpreting Results

### Memory Leak Indicators

**Valgrind output:**
```
==12345== LEAK SUMMARY:
==12345==    definitely lost: 0 bytes in 0 blocks     ← Should be 0
==12345==    indirectly lost: 0 bytes in 0 blocks     ← Should be 0
==12345==      possibly lost: 0 bytes in 0 blocks     ← May be false positives
==12345==    still reachable: 1,024 bytes in 2 blocks ← Global state, acceptable
```

**Expected results:**
- `definitely lost`: 0 (any non-zero is a real leak)
- `indirectly lost`: 0 (usually related to definitely lost)
- `possibly lost`: May be non-zero due to Rust runtime
- `still reachable`: Acceptable (thread-local storage, global allocators)

### Performance Baselines

Target metrics for the driver:

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| Config parsing | < 5 μs | Connection string |
| Pool acquire | < 100 μs | When connection available |
| Simple query | < 1 ms | Network-bound |
| Type conversion | < 100 ns | Per value |
| Arc clone | < 10 ns | Reference counting |

## CI Integration

The CI pipeline includes:

1. **Miri tests** - Detects undefined behavior
2. **Valgrind tests** (optional) - Detects memory leaks
3. **Criterion benchmarks** - Tracks performance regressions

See `.github/workflows/ci.yml` for the full configuration.

## Known Issues

### Valgrind False Positives

Valgrind may report leaks in:

- `std::thread::current()` - Known Rust runtime issue
- Global allocators - Expected behavior
- Thread-local storage - Cleaned up at thread exit

### Miri Limitations

Miri cannot test:

- Async code with complex runtimes (limited Tokio support)
- FFI calls to native libraries
- Actual network I/O

## Troubleshooting

### Out of Memory During Stress Test

Reduce iteration counts or run fewer concurrent tasks:

```bash
# Limit test parallelism
cargo test --package mssql-client --test stress_tests -- --test-threads=1
```

### Valgrind Too Slow

Use `--trace-children=no` and focus on specific tests:

```bash
valgrind --trace-children=no \
    target/debug/deps/stress_tests-* stress_config_parsing
```

### Miri Unsupported Operation

Some operations aren't supported by Miri. Skip those tests:

```rust
#[cfg_attr(miri, ignore)]
#[test]
fn test_with_ffi() {
    // Uses unsupported FFI
}
```

## References

- [Miri Documentation](https://github.com/rust-lang/miri)
- [Valgrind Quick Start](https://valgrind.org/docs/manual/quick-start.html)
- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
