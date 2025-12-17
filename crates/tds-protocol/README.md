# tds-protocol

Pure Rust implementation of the MS-TDS (Tabular Data Stream) protocol used by Microsoft SQL Server.

## Overview

This crate provides low-level, `no_std`-compatible protocol primitives for TDS versions 7.4 through 8.0. It is intentionally IO-agnostic and contains no networking or async runtime code.

## Features

- **`no_std` compatible** - Works in embedded environments with `alloc`
- **Zero unsafe code** - Memory-safe by construction
- **Protocol complete** - Supports TDS 7.4, 7.4.1, and 8.0
- **Well-tested** - Fuzz-tested packet parsing

## Usage

This crate is primarily used internally by `mssql-client`. Direct usage is for advanced scenarios:

```rust
use tds_protocol::{PacketHeader, PacketType, PacketStatus, PreLogin};

// Create a packet header
let header = PacketHeader::new(
    PacketType::PreLogin,
    PacketStatus::END_OF_MESSAGE,
    100,
);

// Encode to bytes
let bytes = header.encode_to_bytes();

// Create PreLogin message
let prelogin = PreLogin::new()
    .with_encryption(EncryptionLevel::Required);
let encoded = prelogin.encode();
```

## Modules

| Module | Description |
|--------|-------------|
| `packet` | TDS packet header encoding/decoding |
| `prelogin` | Pre-login negotiation messages |
| `login7` | LOGIN7 authentication packet |
| `token` | Response token parsing (DONE, ERROR, COLMETADATA, ROW, etc.) |
| `rpc` | Remote procedure call encoding |
| `sql_batch` | SQL batch request encoding |
| `types` | TDS type identifiers and flags |
| `version` | TDS version definitions |

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `std` | Yes | Enable standard library |
| `alloc` | No | Enable allocation without std |

## Protocol References

- [MS-TDS Protocol Specification](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/)
- [TDS 8.0 Specification](https://docs.microsoft.com/en-us/sql/relational-databases/security/networking/tds-8-and-tls-1-3)

## License

MIT OR Apache-2.0
