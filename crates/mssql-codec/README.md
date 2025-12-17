# mssql-codec

Async framing layer for TDS packet handling.

## Overview

This crate transforms raw byte streams into high-level TDS packets, handling packet reassembly across TCP segment boundaries and packet continuation for large messages. It sits between the raw TCP/TLS stream and the higher-level client.

## Architecture

```text
TCP Stream -> TdsCodec (packet framing) -> MessageAssembler -> Client
```

## Features

- **Packet reassembly** - Handles packets split across TCP segments
- **Message reassembly** - Combines multi-packet messages (EOM bit handling)
- **IO splitting** - Separate read/write halves for cancellation safety (ADR-005)
- **Tokio-util codec** - Integrates with tokio-util's codec framework
- **Zero-copy where possible** - Minimizes buffer copies

## Cancellation Safety

Per ADR-005, the connection splits the TCP stream into read and write halves. This allows sending Attention packets for query cancellation even while blocked reading a large result set.

```rust
use mssql_codec::Connection;

let conn = Connection::new(tcp_stream);
let cancel = conn.cancel_handle();

// Cancel from another task
tokio::spawn(async move {
    cancel.cancel().await?;
});
```

## Usage

This crate is primarily used internally by `mssql-client`. Direct usage is for advanced scenarios:

```rust
use mssql_codec::{TdsCodec, Packet, Message, MessageAssembler};
use tokio_util::codec::Framed;

// Create a framed codec over a TCP stream
let framed = Framed::new(tcp_stream, TdsCodec::new());

// Or use the Connection wrapper for more features
let conn = Connection::new(tcp_stream);
```

## Modules

| Module | Description |
|--------|-------------|
| `connection` | High-level connection with cancel support |
| `packet_codec` | TDS packet encoding/decoding |
| `framed` | `PacketReader` and `PacketWriter` types |
| `message` | Multi-packet message assembly |
| `error` | Codec error types |

## Key Types

| Type | Description |
|------|-------------|
| `Connection` | High-level connection with IO splitting |
| `CancelHandle` | Handle for canceling queries from another task |
| `TdsCodec` | Tokio codec for TDS packet framing |
| `Packet` | Single TDS packet |
| `Message` | Complete TDS message (possibly from multiple packets) |
| `MessageAssembler` | Assembles packets into complete messages |

## TDS Packet Structure

```text
+--------+--------+--------+--------+--------+--------+--------+--------+
| Type   | Status | Length (2)      | SPID (2)        | Pkt# | Window |
+--------+--------+--------+--------+--------+--------+--------+--------+
|                           Payload Data                                |
|                              ...                                      |
+-----------------------------------------------------------------------+
```

- **Type**: Packet type (PreLogin, Login7, SQLBatch, RPC, etc.)
- **Status**: Flags including EOM (End of Message)
- **Length**: Total packet length including header
- **SPID**: Server Process ID
- **Packet Number**: For multi-packet messages
- **Window**: Reserved (always 0)

## License

MIT OR Apache-2.0
