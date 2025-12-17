# mssql-tls

TLS negotiation layer for SQL Server connections.

## Overview

This crate handles the complexity of TLS negotiation for both TDS 7.x (pre-login encryption negotiation) and TDS 8.0 (strict TLS-first mode). It uses rustls for a pure-Rust, memory-safe TLS implementation.

## TDS Version Differences

### TDS 7.x (SQL Server 2019 and earlier)

```text
TCP Connect -> PreLogin (cleartext) -> TLS Handshake -> Login7 (encrypted)
```

### TDS 8.0 (SQL Server 2022+ strict mode)

```text
TCP Connect -> TLS Handshake -> PreLogin (encrypted) -> Login7 (encrypted)
```

## Features

- **TLS 1.2 and TLS 1.3** - Modern protocol support via rustls
- **Server certificate validation** - Mozilla root CA store
- **Hostname verification** - Prevents MITM attacks
- **Custom CA support** - For internal certificate authorities
- **Client certificate authentication** - Mutual TLS (TDS 8.0)

## Usage

### Default Configuration

```rust
use mssql_tls::{default_tls_config, TlsConnector};

// Secure default configuration
let config = default_tls_config()?;
let connector = TlsConnector::new(config);
```

### Builder Pattern

```rust
use mssql_tls::{TlsConfig, TlsVersion};

let config = TlsConfig::builder()
    .strict_mode(true)              // TDS 8.0
    .min_protocol_version(TlsVersion::Tls13)
    .hostname_verification(true)
    .build()?;
```

### Trust Server Certificate (Development Only)

```rust
// WARNING: Disables certificate validation - development only!
let config = TlsConfig::builder()
    .trust_server_certificate(true)
    .build()?;
```

### Custom Certificate Authority

```rust
let config = TlsConfig::builder()
    .ca_certificate_path("/path/to/ca.pem")
    .build()?;
```

### Client Certificate Authentication

```rust
use mssql_tls::ClientAuth;

let config = TlsConfig::builder()
    .strict_mode(true)  // Required for client certs
    .client_auth(ClientAuth::Certificate {
        cert_path: "/path/to/client.pem".into(),
        key_path: "/path/to/client-key.pem".into(),
    })
    .build()?;
```

## Negotiation Modes

| Mode | When Used | Description |
|------|-----------|-------------|
| `PostPreLogin` | TDS 7.x, `Encrypt=true` | TLS after PreLogin exchange |
| `Strict` | TDS 8.0, `Encrypt=strict` | TLS immediately after TCP |

```rust
use mssql_tls::TlsNegotiationMode;

let mode = TlsNegotiationMode::from_encrypt_mode(encrypt_strict);

if mode.is_tls_first() {
    // TDS 8.0: TLS handshake before any TDS traffic
}
```

## Modules

| Module | Description |
|--------|-------------|
| `config` | TLS configuration builder |
| `connector` | TLS connection establishment |
| `error` | TLS error types |

## Key Types

| Type | Description |
|------|-------------|
| `TlsConfig` | TLS configuration options |
| `TlsConnector` | Establishes TLS connections |
| `TlsVersion` | TLS protocol versions |
| `TlsNegotiationMode` | When TLS handshake occurs |
| `ClientAuth` | Client authentication options |
| `TlsStream` | Encrypted stream (re-exported from tokio-rustls) |

## Security Considerations

### Certificate Validation

By default, this crate validates server certificates using the Mozilla root certificate store. This provides:

- **Identity verification** - Server is who it claims to be
- **MITM protection** - Encrypted channel to correct server
- **Trust chain validation** - Certificate signed by trusted CA

### TrustServerCertificate

The `trust_server_certificate` option disables validation and logs a warning. Use only for:

- Development environments
- Testing with self-signed certificates
- When you understand the security implications

**Never use in production without explicit security review.**

### TDS 8.0 Strict Mode

SQL Server 2022+ supports strict TLS mode where:

- All traffic is encrypted, including PreLogin
- TLS 1.3 can be required
- Client certificate authentication is supported

## Error Handling

```rust
use mssql_tls::TlsError;

match connector.connect(stream, hostname).await {
    Ok(tls_stream) => { /* use encrypted stream */ }
    Err(TlsError::HandshakeFailed(e)) => {
        // TLS handshake failed
    }
    Err(TlsError::CertificateInvalid(e)) => {
        // Server certificate validation failed
    }
    Err(TlsError::HostnameVerificationFailed) => {
        // Certificate CN doesn't match hostname
    }
    Err(e) => {
        // Other errors
    }
}
```

## License

MIT OR Apache-2.0
