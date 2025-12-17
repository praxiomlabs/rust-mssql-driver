# mssql-testing

Test infrastructure for SQL Server driver development.

## Overview

This crate provides utilities for integration testing against SQL Server instances, including testcontainers support for spinning up real SQL Server instances and a mock TDS server for unit tests that don't require Docker.

## Features

- **SQL Server containers** - Managed test containers via testcontainers
- **Mock TDS server** - Simulate SQL Server responses without Docker
- **Packet recording** - Capture and replay TDS traffic for regression tests
- **Test fixtures** - Common test data and helpers
- **Connection helpers** - Simplified test connection setup

## Mock Server

The mock server simulates TDS protocol responses for unit testing:

```rust
use mssql_testing::mock_server::{MockTdsServer, MockResponse, MockColumn, ScalarValue};

#[tokio::test]
async fn test_with_mock_server() {
    let server = MockTdsServer::builder()
        .with_response(
            "SELECT * FROM users WHERE id = 1",
            MockResponse::rows(
                vec![MockColumn::int("id"), MockColumn::nvarchar("name", 50)],
                vec![vec![ScalarValue::Int(1), ScalarValue::String("Alice".into())]],
            ),
        )
        .with_response(
            "SELECT COUNT(*) FROM users",
            MockResponse::scalar(ScalarValue::Int(42)),
        )
        .build()
        .await
        .unwrap();

    let addr = server.addr();
    // Connect your client to addr...
}
```

## SQL Server Containers

Spin up real SQL Server instances for integration tests:

```rust
use mssql_testing::SqlServerContainer;
use testcontainers::clients::Cli;

#[tokio::test]
async fn test_with_real_server() {
    let docker = Cli::default();
    let container = docker.run(SqlServerContainer::default());
    let port = container.get_host_port_ipv4(1433);

    // Connect to localhost:port with sa/YourStrong!Passw0rd
}
```

### Container Versions

```rust
// SQL Server 2019
let container = SqlServerContainer::sql_server_2019();

// SQL Server 2022
let container = SqlServerContainer::sql_server_2022();

// Custom image
let container = SqlServerContainer::new("mcr.microsoft.com/mssql/server:2022-latest");
```

## Packet Recording

Record and replay TDS packets for regression testing:

```rust
use mssql_testing::mock_server::{PacketRecorder, RecordedPacket};

// Record packets during a test run
let recorder = PacketRecorder::new();
// ... run operations ...
let packets: Vec<RecordedPacket> = recorder.packets();

// Save for later replay
let json = serde_json::to_string(&packets)?;

// Replay recorded packets
let server = MockTdsServer::builder()
    .with_recorded_packets(packets)
    .build()
    .await?;
```

## Test Fixtures

Common test data and helpers:

```rust
use mssql_testing::fixtures::{test_connection_string, test_database_setup};

// Get a test connection string (from env or default)
let conn_str = test_connection_string();

// Set up a test database with schema
test_database_setup(&mut client, r#"
    CREATE TABLE users (
        id INT PRIMARY KEY,
        name NVARCHAR(100)
    );
    INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob');
"#).await?;
```

## Modules

| Module | Description |
|--------|-------------|
| `container` | SQL Server testcontainers support |
| `mock_server` | Mock TDS server implementation |
| `fixtures` | Test data and setup helpers |

## Key Types

| Type | Description |
|------|-------------|
| `SqlServerContainer` | Testcontainers SQL Server image |
| `MockTdsServer` | Mock TDS protocol server |
| `MockServerBuilder` | Builder for configuring mock server |
| `MockResponse` | Pre-configured response for a query |
| `MockColumn` | Column metadata for mock responses |
| `ScalarValue` | Typed scalar values for mock data |
| `PacketRecorder` | Records TDS packets for replay |
| `RecordedPacket` | Captured TDS packet data |

## Mock Response Types

```rust
// Empty result (no rows)
MockResponse::empty()

// Single scalar value
MockResponse::scalar(ScalarValue::Int(42))

// Rows with columns
MockResponse::rows(
    vec![MockColumn::int("count")],
    vec![vec![ScalarValue::Int(100)]],
)

// Error response
MockResponse::error(8134, "Divide by zero error")

// Affected row count (for INSERT/UPDATE/DELETE)
MockResponse::affected_rows(5)
```

## Best Practices

1. **Use mock server for unit tests** - Fast, no Docker dependency
2. **Use containers for integration tests** - Real SQL Server behavior
3. **Record packets for regression tests** - Catch protocol changes
4. **Clean up test databases** - Use fixtures for consistent state

## License

MIT OR Apache-2.0
