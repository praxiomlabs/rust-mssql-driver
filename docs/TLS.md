# TLS Configuration Guide

This document covers TLS/SSL configuration for secure SQL Server connections.

## Overview

rust-mssql-driver uses **rustls** for TLS, providing:

- Pure Rust implementation (no OpenSSL dependency)
- Memory safety guarantees
- TLS 1.2 and TLS 1.3 support
- Both TDS 7.x and TDS 8.0 encryption modes

## TLS Negotiation Modes

### TDS 7.x (Standard Encryption)

For SQL Server 2019 and earlier, or SQL Server 2022+ with `Encrypt=true`:

```
TCP Connect → PreLogin (cleartext) → TLS Handshake → Login7 (encrypted)
```

The PreLogin packet is sent unencrypted, but all subsequent traffic (including credentials) is encrypted.

### TDS 8.0 (Strict Mode)

For SQL Server 2022+ with `Encrypt=strict`:

```
TCP Connect → TLS Handshake → PreLogin (encrypted) → Login7 (encrypted)
```

**All** TDS traffic is encrypted, including the initial PreLogin exchange.

## SQL Server Version Requirements

| SQL Server Version | Min TLS | Recommended | TDS 8.0 Support |
|-------------------|---------|-------------|-----------------|
| SQL Server 2008/2008 R2 | TLS 1.0* | N/A | No |
| SQL Server 2012 | TLS 1.0* | N/A | No |
| SQL Server 2014 | TLS 1.0* | N/A | No |
| SQL Server 2016 | TLS 1.0* | TLS 1.2 | No |
| SQL Server 2017 | TLS 1.0 | TLS 1.2 | No |
| SQL Server 2019 | TLS 1.0 | TLS 1.2 | No |
| SQL Server 2022 | TLS 1.2 | TLS 1.3 | Yes |
| Azure SQL | TLS 1.2 | TLS 1.2 | Varies |

\* **Legacy servers (2008-2016):** These versions typically only support TLS 1.0/1.1, which rustls does not support. Use `Encrypt=no_tls` for unencrypted connections on trusted networks.

**Note:** SQL Server 2016-2019 support TLS 1.0 by default but can be configured to require TLS 1.2. Always use TLS 1.2 or higher in production where possible.

## Configuration Options

### Connection String

```
# Standard encryption (TDS 7.x) - all SQL Server versions
Encrypt=true;TrustServerCertificate=false

# Strict encryption (TDS 8.0) - SQL Server 2022+ only
Encrypt=strict;TrustServerCertificate=false

# Development only - disable certificate validation
Encrypt=true;TrustServerCertificate=true
```

### Programmatic Configuration

```rust
use mssql_tls::{TlsConfig, TlsVersion, TlsConnector};

// Secure production configuration
let tls_config = TlsConfig::new()
    .min_protocol_version(TlsVersion::Tls12)
    .max_protocol_version(TlsVersion::Tls13);

// TDS 8.0 strict mode (SQL Server 2022+)
let tls_config = TlsConfig::new()
    .strict_mode(true)
    .min_protocol_version(TlsVersion::Tls13);

// Development only - skip certificate validation
let tls_config = TlsConfig::new()
    .trust_server_certificate(true);  // WARNING: Insecure!
```

## Encryption Modes

### `Encrypt=false` (Not Recommended)

No TLS encryption. All traffic including credentials is sent in plaintext.

**Security Risk:** Network sniffing, credential theft, data interception.

```
# Never use in production
Encrypt=false
```

### `Encrypt=no_tls` (Legacy Servers Only)

Completely disables TLS for SQL Server 2008-2016 instances that don't support TLS 1.2+.

**Security Risk:** Network sniffing, credential theft, data interception.

```
# For legacy SQL Server (2008-2016) on trusted networks only
Server=legacy-server;User Id=sa;Password=pwd;Encrypt=no_tls
```

**When to use:**
- SQL Server 2008-2016 that cannot be upgraded
- Isolated, trusted network environments
- Development/testing against legacy instances

**⚠️ Warning:** This option transmits all data including credentials in plaintext. Only use on isolated networks where TLS 1.2+ is not available.

### `Encrypt=true` (Standard)

TLS encryption with certificate validation. PreLogin is sent cleartext, but Login7 and all subsequent traffic is encrypted.

```
# Recommended for SQL Server 2019 and earlier
Encrypt=true;TrustServerCertificate=false
```

### `Encrypt=strict` (Maximum Security)

TDS 8.0 strict mode. TLS handshake occurs before any TDS traffic.

```
# Recommended for SQL Server 2022+
Encrypt=strict;TrustServerCertificate=false
```

**Benefits:**
- All TDS traffic encrypted (including PreLogin)
- Prevents protocol downgrade attacks
- Required for some compliance standards

## Certificate Validation

### Server Certificate Validation

By default, the driver validates the server certificate against the Mozilla root CA store.

```rust
// Default: validates against Mozilla root CAs
let tls_config = TlsConfig::new();

// The certificate must:
// 1. Be signed by a trusted CA
// 2. Not be expired
// 3. Match the server hostname (SNI)
```

### Custom Certificate Authority

For self-signed certificates or internal CAs:

```rust
use rustls::pki_types::CertificateDer;
use std::fs;

// Load your CA certificate
let ca_cert = fs::read("path/to/ca.crt")?;
let cert = CertificateDer::from(ca_cert);

let tls_config = TlsConfig::new()
    .add_root_certificate(cert);
```

### Hostname Verification

The server's hostname must match the certificate's Common Name (CN) or Subject Alternative Name (SAN).

```rust
// Override hostname for certificate validation
let tls_config = TlsConfig::new()
    .with_server_name("actual-hostname.example.com");
```

### TrustServerCertificate (Development Only)

Disables certificate validation entirely. **Never use in production.**

```rust
// WARNING: This is insecure!
let tls_config = TlsConfig::new()
    .trust_server_certificate(true);
```

When enabled, the driver logs a warning:

```
WARN TrustServerCertificate is enabled - certificate validation is DISABLED.
     This is insecure and should only be used for development/testing.
     Connections are vulnerable to man-in-the-middle attacks.
```

## Client Certificate Authentication

TDS 8.0 supports mutual TLS (client certificate authentication):

```rust
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs;

// Load client certificate and key
let cert_pem = fs::read("client.crt")?;
let key_pem = fs::read("client.key")?;

let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut &cert_pem[..])
    .collect::<Result<_, _>>()?;

let key = rustls_pemfile::private_key(&mut &key_pem[..])?
    .expect("no private key found");

let tls_config = TlsConfig::new()
    .strict_mode(true)
    .with_client_auth(certs, key);
```

## TLS Version Selection

### Minimum Version

```rust
// Require TLS 1.2 minimum (recommended)
let tls_config = TlsConfig::new()
    .min_protocol_version(TlsVersion::Tls12);

// Require TLS 1.3 minimum (strictest)
let tls_config = TlsConfig::new()
    .min_protocol_version(TlsVersion::Tls13);
```

### Version Range

```rust
// Allow TLS 1.2 and 1.3
let tls_config = TlsConfig::new()
    .min_protocol_version(TlsVersion::Tls12)
    .max_protocol_version(TlsVersion::Tls13);
```

## Common Issues

### Certificate Errors

**Error:** `certificate verify failed`

**Causes:**
1. Self-signed certificate without custom CA
2. Expired certificate
3. Hostname mismatch

**Solutions:**
```rust
// Option 1: Add your CA certificate
let tls_config = TlsConfig::new()
    .add_root_certificate(your_ca_cert);

// Option 2: Override hostname
let tls_config = TlsConfig::new()
    .with_server_name("certificate-hostname");

// Option 3 (dev only): Skip validation
let tls_config = TlsConfig::new()
    .trust_server_certificate(true);
```

### TLS Handshake Timeout

**Error:** `TLS handshake timed out`

**Causes:**
1. Firewall blocking TLS traffic
2. SQL Server not configured for encryption
3. Network latency

**Solutions:**
1. Check firewall rules for port 1433
2. Verify SQL Server encryption settings
3. Increase TLS timeout in configuration

### Protocol Version Mismatch

**Error:** `no protocols configured` or `handshake failure`

**Causes:**
1. SQL Server doesn't support required TLS version
2. Version range configured incorrectly

**Solutions:**
```rust
// Allow broader version range
let tls_config = TlsConfig::new()
    .min_protocol_version(TlsVersion::Tls12)
    .max_protocol_version(TlsVersion::Tls13);
```

### TDS 8.0 Not Supported

**Error:** `strict encryption mode requires SQL Server 2022+`

**Cause:** Using `Encrypt=strict` with older SQL Server.

**Solution:** Use `Encrypt=true` instead:
```
# For SQL Server 2019 and earlier
Encrypt=true;TrustServerCertificate=false
```

## Azure SQL Database

Azure SQL Database has specific TLS requirements:

### Connection String

```
Server=yourserver.database.windows.net;
Database=yourdb;
User Id=user@yourserver;
Password=password;
Encrypt=true;
TrustServerCertificate=false
```

### Certificate Validation

Azure SQL uses certificates signed by public CAs. No custom CA configuration needed.

### Minimum TLS Version

Azure SQL requires TLS 1.2 minimum. The driver default is compatible.

### Gateway Redirects

Azure SQL may redirect connections. The driver handles this automatically, performing new TLS handshakes with redirect targets.

## Security Recommendations

### Production Checklist

- [ ] `Encrypt=true` or `Encrypt=strict`
- [ ] `TrustServerCertificate=false`
- [ ] TLS 1.2 minimum (`min_protocol_version: Tls12`)
- [ ] Server certificate from trusted CA
- [ ] Certificate hostname matches server

### Compliance Considerations

| Standard | TLS Requirement |
|----------|-----------------|
| PCI DSS | TLS 1.2+ required |
| HIPAA | Encryption required, TLS 1.2+ recommended |
| SOC 2 | Encryption required |
| FedRAMP | TLS 1.2+ with FIPS 140-2 |

### Defense in Depth

1. **Network Level:** Use TLS encryption
2. **Certificate Level:** Validate server certificates
3. **Protocol Level:** Use TDS 8.0 when available
4. **Application Level:** Parameterized queries (prevent injection)

## API Reference

### TlsConfig

```rust
pub struct TlsConfig {
    /// Skip certificate validation (development only)
    pub trust_server_certificate: bool,

    /// Custom root certificates
    pub root_certificates: Vec<CertificateDer<'static>>,

    /// Client certificate for mutual TLS
    pub client_auth: Option<ClientAuth>,

    /// Server name for certificate validation
    pub server_name: Option<String>,

    /// Minimum TLS version
    pub min_protocol_version: TlsVersion,

    /// Maximum TLS version
    pub max_protocol_version: TlsVersion,

    /// Enable TDS 8.0 strict mode
    pub strict_mode: bool,
}
```

### TlsVersion

```rust
pub enum TlsVersion {
    /// TLS 1.2
    Tls12,
    /// TLS 1.3
    Tls13,
}
```

### TlsNegotiationMode

```rust
pub enum TlsNegotiationMode {
    /// TDS 7.x: TLS after PreLogin
    PostPreLogin,

    /// TDS 8.0: TLS before any TDS traffic
    Strict,
}
```

## Comparison with Other Drivers

| Feature | rust-mssql-driver | Tiberius | ODBC |
|---------|-------------------|----------|------|
| TLS Library | rustls (pure Rust) | native-tls | OpenSSL/SChannel |
| TDS 8.0 Strict | Yes | No | Yes |
| Client Certs | Yes | Limited | Yes |
| Custom CA | Yes | Yes | Platform-specific |
| Memory Safety | Guaranteed | Guaranteed | C library |
