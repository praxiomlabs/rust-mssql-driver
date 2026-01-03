# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.5.x   | :white_check_mark: |
| 0.4.x   | :white_check_mark: |
| 0.3.x   | :x: (end of life)  |
| 0.2.x   | :x: (end of life)  |
| 0.1.x   | :x: (end of life)  |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**DO NOT** open a public GitHub issue for security vulnerabilities.

Instead, please report security issues by emailing:

**security@rust-mssql-driver.dev** (or open a private security advisory on GitHub)

### What to Include

Please include the following information in your report:

1. **Description**: A clear description of the vulnerability
2. **Impact**: What an attacker could achieve by exploiting it
3. **Reproduction**: Steps to reproduce the issue
4. **Affected Versions**: Which versions are affected
5. **Suggested Fix**: If you have ideas for how to fix it (optional)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 7 days
- **Fix Development**: Depends on severity
- **Disclosure**: Coordinated with reporter

### Severity Levels

| Level | Response Time | Examples |
|-------|---------------|----------|
| Critical | 24-48 hours | Remote code execution, credential leak |
| High | 7 days | SQL injection, authentication bypass |
| Medium | 30 days | Information disclosure, DoS |
| Low | 90 days | Minor issues with limited impact |

## Threat Model

This section documents the security architecture, trust boundaries, and threat mitigations implemented by rust-mssql-driver.

### Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        YOUR APPLICATION                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    rust-mssql-driver                             │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │   │
│  │  │ Config/Auth │  │ Connection  │  │ Query Execution         │  │   │
│  │  │ (Tier 1)    │  │ Pool        │  │ (Parameterized)         │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘  │   │
│  └──────────────────────────────┬──────────────────────────────────┘   │
└─────────────────────────────────┼──────────────────────────────────────┘
                                  │ TLS 1.2/1.3 (Trust Boundary 1)
                                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         NETWORK                                          │
│    Assumed Hostile: Man-in-the-middle, packet inspection, replay        │
└─────────────────────────────────┼───────────────────────────────────────┘
                                  │ TLS 1.2/1.3 (Trust Boundary 2)
                                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        SQL SERVER                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Authentication │ Authorization │ Query Processing │ Storage    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  Trust Boundary 3: Server-side access control, DBAs, backup access      │
└─────────────────────────────────────────────────────────────────────────┘
```

### Protected Assets

| Asset | Location | Protection Mechanism |
|-------|----------|---------------------|
| Credentials (password) | Client memory | `zeroize` feature, no logging |
| Credentials (token) | Client memory | `SecretString`, no logging |
| Connection string | Config | Sanitized in errors/logs |
| Data in transit | Network | TLS 1.2/1.3 encryption |
| Data in driver memory | Client | `Arc<Bytes>` (not persisted) |
| Query parameters | Protocol | RPC binding (never interpolated) |

### Threat Actors

| Actor | Capability | Mitigations |
|-------|------------|-------------|
| **Network Attacker** | Packet inspection, MITM, replay | TLS encryption, certificate validation |
| **Malicious Application** | Access to driver API | Parameterized queries prevent injection |
| **Compromised Dependencies** | Supply chain attack | `cargo-deny`, RustSec auditing |
| **Malicious DBA** | Server-side access | *Out of scope* - see Always Encrypted note |
| **Memory Forensics** | Access to process memory | `zeroize` feature for credentials |

### Attack Surface Analysis

#### 1. Network Layer

| Attack | Risk | Mitigation |
|--------|------|------------|
| Eavesdropping | **HIGH** if unencrypted | TLS mandatory for production |
| Man-in-the-middle | **HIGH** if cert not validated | Certificate validation by default |
| Replay attacks | LOW | TLS provides replay protection |
| Protocol downgrade | MEDIUM | TDS 8.0 strict mode prevents |

**Configuration:**
```
# Maximum security (TDS 8.0)
Encrypt=strict;TrustServerCertificate=false

# Standard security (TDS 7.x)
Encrypt=true;TrustServerCertificate=false
```

#### 2. Protocol Layer

| Attack | Risk | Mitigation |
|--------|------|------------|
| SQL Injection | **CRITICAL** | Parameterized queries only |
| Malformed packets | LOW | Fuzz-tested parsers |
| Token confusion | LOW | Type-state pattern |
| Buffer overflow | MINIMAL | Memory-safe Rust |

#### 3. Authentication Layer

| Attack | Risk | Mitigation |
|--------|------|------------|
| Credential theft (memory) | MEDIUM | `zeroize` feature |
| Credential theft (logs) | LOW | Credentials never logged |
| Brute force | LOW | Server-side lockout policies |
| Token replay | LOW | Short-lived tokens, TLS |

#### 4. Configuration Layer

| Attack | Risk | Mitigation |
|--------|------|------------|
| Connection string injection | MEDIUM | Input validation, builder API |
| Identifier injection | MEDIUM | `validate_identifier()` for savepoints |
| Misconfiguration | HIGH | Warnings for insecure options |

### STRIDE Analysis

| Threat | Category | Driver Mitigation |
|--------|----------|-------------------|
| **S**poofing identity | Authentication | TLS certificates, SQL/Azure AD auth |
| **T**ampering with data | Integrity | TLS integrity checks, parameterized queries |
| **R**epudiation | Non-repudiation | *Application responsibility* - use audit tables |
| **I**nformation disclosure | Confidentiality | TLS encryption, credential sanitization |
| **D**enial of service | Availability | Connection timeouts, pool limits |
| **E**levation of privilege | Authorization | *Server responsibility* - use least privilege |

### Security Guarantees

#### What We Protect Against

| Threat | Protection Level | How |
|--------|------------------|-----|
| Network eavesdropping | ✅ Strong | TLS 1.2/1.3 encryption |
| MITM attacks | ✅ Strong | Certificate validation |
| SQL injection via parameters | ✅ Strong | RPC protocol binding |
| Credential logging | ✅ Strong | Never logged at any level |
| Credential memory exposure | ✅ Moderate | `zeroize` feature (opt-in) |
| Savepoint injection | ✅ Strong | Identifier validation |
| Protocol parsing bugs | ✅ Moderate | Fuzz testing, memory safety |

#### What We Do NOT Protect Against

| Threat | Reason | Recommendation |
|--------|--------|----------------|
| Application-level SQL injection | Dynamic SQL is application code | Always use parameterized queries |
| Weak passwords | Server policy | Use strong passwords, consider AAD |
| Malicious DBAs | Server trust boundary | See Always Encrypted section |
| Server compromise | Server trust boundary | Network segmentation, monitoring |
| Denial of service (server) | Server capacity | Rate limiting, resource governor |
| Data at rest encryption | Server/storage layer | Use TDE, Always Encrypted |

### Always Encrypted Considerations

**Status:** Full support in v0.3.0 (cryptography ✅, key providers ✅)

The driver supports Always Encrypted client-side encryption via the `always-encrypted` feature:

**Implemented:**
- AEAD_AES_256_CBC_HMAC_SHA256 encryption/decryption
- RSA-OAEP key unwrapping for CEK decryption
- CEK caching with TTL expiration
- `InMemoryKeyStore` for development/testing
- `KeyStoreProvider` trait for custom implementations
- `AzureKeyVaultProvider` for Azure Key Vault (`azure-identity` feature)
- `WindowsCertStoreProvider` for Windows Certificate Store (`sspi-auth` feature, Windows only)

Always Encrypted provides **client-side encryption** for data that remains encrypted even on the SQL Server. This protects against threats from the server side:

| Threat | Standard TLS | Always Encrypted |
|--------|--------------|------------------|
| Network attackers | ✅ Protected | ✅ Protected |
| Compromised DBAs | ❌ Exposed | ✅ Protected |
| Server memory access | ❌ Exposed | ✅ Protected |
| Backup theft | ❌ Exposed | ✅ Protected |

**If your threat model includes malicious DBAs or server compromise:**
1. Use the `always-encrypted` feature with `InMemoryKeyStore` for dev/test
2. Use `AzureKeyVaultProvider` for Azure Key Vault integration
3. Use `WindowsCertStoreProvider` for Windows Certificate Store (Windows only)
4. Implement the `KeyStoreProvider` trait for custom key storage
5. Do NOT use T-SQL `ENCRYPTBYKEY` - keys exist on the server

See [ARCHITECTURE.md § ADR-013](ARCHITECTURE.md) for details.

### Secure Defaults

The driver ships with secure defaults:

| Setting | Default | Insecure Option |
|---------|---------|-----------------|
| TLS | Enabled | `Encrypt=false` (logs warning) |
| Certificate validation | Enabled | `TrustServerCertificate=true` (logs warning) |
| Parameter binding | Required | N/A - no raw SQL interpolation API |
| Connection timeout | 15 seconds | `Connect Timeout=0` (infinite) |
| Command timeout | 30 seconds | `Command Timeout=0` (infinite) |

## Security Considerations

### TLS/SSL Requirements

#### Minimum TLS Version

| Environment | Minimum Version | Recommendation |
|-------------|-----------------|----------------|
| Production | TLS 1.2 | TLS 1.3 preferred |
| Development | TLS 1.2 | TLS 1.2 acceptable |
| PCI DSS Compliant | TLS 1.2 | Required per PCI DSS 3.2.1+ |
| FedRAMP | TLS 1.2 | TLS 1.3 recommended |

**Default configuration:**
- Minimum: TLS 1.2
- Maximum: TLS 1.3

```rust
use mssql_tls::{TlsConfig, TlsVersion};

// Require TLS 1.3 only (maximum security)
let config = TlsConfig::new()
    .min_protocol_version(TlsVersion::Tls13);
```

#### TLS 1.2 vs TLS 1.3 Behavior

| Feature | TLS 1.2 | TLS 1.3 |
|---------|---------|---------|
| Handshake round trips | 2 | 1 |
| Forward secrecy | Optional (ECDHE) | Mandatory |
| 0-RTT resumption | No | Yes (with caution) |
| Legacy cipher support | Yes | No (safer defaults) |
| TDS 7.x compatibility | Required | Supported |
| TDS 8.0 strict mode | Supported | Recommended |

**When to use TLS 1.3:**
- SQL Server 2022+ in strict mode (`Encrypt=strict`)
- Modern cloud deployments (Azure SQL)
- Maximum security requirements

**When TLS 1.2 is needed:**
- Older SQL Server versions (2016, 2017, 2019)
- Legacy environments without TLS 1.3 support
- Corporate environments with TLS 1.2-only proxies

#### TDS 8.0 Strict Mode

SQL Server 2022+ supports TDS 8.0 strict mode, which provides:

```
Standard TDS 7.x:  TCP -> PreLogin (clear) -> TLS -> Login7
TDS 8.0 Strict:    TCP -> TLS -> PreLogin (encrypted) -> Login7
```

Benefits of strict mode:
- **No cleartext traffic**: Even PreLogin is encrypted
- **Prevents downgrade**: Server rejects non-TLS connections
- **Client cert auth**: Enables certificate-based authentication

```
# Connection string for strict mode
Server=host;Encrypt=strict;TrustServerCertificate=false
```

#### TLS Implementation Details

- This driver uses **rustls** for TLS, which is memory-safe and audited
- TLS 1.2 and TLS 1.3 are supported
- TDS 8.0 strict encryption mode is fully supported
- `TrustServerCertificate=true` disables certificate validation (use only in development)

**Cipher Suites (TLS 1.3):**
- TLS_AES_256_GCM_SHA384
- TLS_AES_128_GCM_SHA256
- TLS_CHACHA20_POLY1305_SHA256

**Cipher Suites (TLS 1.2):**
- TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
- TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
- TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256

#### Certificate Validation

| Option | Default | Security |
|--------|---------|----------|
| `TrustServerCertificate=false` | Yes | Certificate chain validated |
| `TrustServerCertificate=true` | No | **INSECURE** - logs warning |
| Custom CA certificate | No | Enterprise CA support |
| Client certificate | No | Mutual TLS (TDS 8.0) |

```rust
// Custom CA certificate
let config = TlsConfig::new()
    .add_root_certificate(cert);

// Client certificate authentication (TDS 8.0)
let config = TlsConfig::new()
    .with_client_auth(client_certs, private_key)
    .strict_mode(true);
```

### SQL Injection Prevention

- Always use parameterized queries (`@p1`, `@p2`, etc.)
- Savepoint names are validated against injection attacks
- Table and column names cannot be parameterized (validate user input separately)

### Credential Handling

- Passwords are never logged
- Connection strings are sanitized in error messages
- Optional `zeroize` feature securely wipes credentials from memory

### What We Do NOT Protect Against

- Application-level SQL injection (you must use parameters)
- Weak passwords
- Misconfigured SQL Server instances
- Network attacks (use TLS)
- Compromised SQL Server (use Always Encrypted `always-encrypted` feature)

## Security Best Practices

### Connection Strings

```rust
// Good: Use TLS encryption
"Server=host;Encrypt=strict;TrustServerCertificate=false"

// Bad: No encryption in production
"Server=host;Encrypt=false"

// Bad: Skipping certificate validation in production
"Server=host;TrustServerCertificate=true"
```

### Parameterized Queries

```rust
// Good: Parameterized
client.query("SELECT * FROM users WHERE id = @p1", &[&user_id]).await?;

// Bad: String interpolation (SQL injection risk!)
client.query(&format!("SELECT * FROM users WHERE id = {}", user_id), &[]).await?;
```

### Error Handling

```rust
// Good: Don't expose internal errors to users
match result {
    Err(e) => {
        tracing::error!("Database error: {:?}", e);
        return Err(UserFacingError::InternalError);
    }
}

// Bad: Exposing database errors directly
client.query(sql, &[]).await.map_err(|e| format!("{:?}", e))?;
```

## Security Features

### Enabled by Default

- TLS certificate validation
- Parameterized query support
- Savepoint name validation
- Connection timeout protection

### Optional (Feature Flags)

- `zeroize` - Secure credential wiping
- `always-encrypted` - Client-side encryption with Azure Key Vault and Windows CertStore providers
- `otel` - Security event tracing (with care around sensitive data)

## Dependency Security

We use `cargo-deny` to audit dependencies:

```bash
cargo deny check
```

This checks for:
- Known vulnerabilities (RustSec Advisory Database)
- Unmaintained crates
- License compliance

## Audit History

| Date | Auditor | Scope | Result |
|------|---------|-------|--------|
| *Pending* | *TBD* | Full security audit | *Planned* |

## Acknowledgments

We thank the following individuals for responsibly disclosing security issues:

*No vulnerabilities reported yet*

---

## Contact

For security-related questions that are not vulnerabilities, you may open a public issue with the `security-question` label.

For vulnerability reports, use the private channels described above.
