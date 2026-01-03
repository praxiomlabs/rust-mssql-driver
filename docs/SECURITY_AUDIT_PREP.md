# Security Audit Preparation Checklist

This document outlines the preparation steps for a formal security audit of rust-mssql-driver.

## Pre-Audit Status

| Area | Status | Notes |
|------|--------|-------|
| Code review | Complete | All code reviewed during development |
| Static analysis | Complete | Clippy, cargo-deny configured |
| Dependency audit | Complete | cargo-audit, weekly CI scans |
| Fuzz testing | Complete | 11 fuzz targets covering protocol parsing |
| Documentation | Complete | SECURITY.md, threat model documented |

---

## 1. Automated Security Tooling

### 1.1 Static Analysis (Complete)

- [x] **Clippy** - Strict linting enabled (`-D warnings`)
- [x] **cargo-deny** - Dependency auditing configured
  - License compliance
  - Known vulnerability detection
  - Banned crate detection
- [x] **cargo-audit** - Weekly scheduled scans

### 1.2 Fuzz Testing (Complete)

11 fuzz targets cover critical parsing code:

| Target | Coverage |
|--------|----------|
| `parse_packet` | TDS packet header parsing |
| `parse_token` | Token stream parsing |
| `parse_prelogin` | Prelogin packet parsing |
| `parse_login7` | Login7 response parsing |
| `parse_rpc` | RPC request/response parsing |
| `parse_env_change` | Environment change token parsing |
| `decode_value` | Type value decoding |
| `type_roundtrip` | Type encode/decode roundtrip |
| `collation_decode` | Collation parsing |
| `crypto_decode` | Always Encrypted parsing |
| `connection_string` | Connection string parsing |

### 1.3 Memory Safety

- [x] **No unsafe code** - `#![deny(unsafe_code)]` in all crates
- [x] **Miri** - CI runs Miri on tds-protocol crate
- [x] **Bounds checking** - All array accesses validated

---

## 2. Security-Critical Code Areas

### 2.1 Authentication (`mssql-auth`)

**Review Priority: HIGH**

- SQL Authentication (Login7 packet construction)
- Azure AD token handling
- Kerberos/GSSAPI integration
- SSPI (Windows) authentication
- Client certificate authentication

**Key Files:**
- `crates/mssql-auth/src/sql.rs`
- `crates/mssql-auth/src/azure.rs`
- `crates/mssql-auth/src/kerberos.rs`

**Security Questions:**
- [ ] Credentials wiped from memory after use? (`zeroize` feature)
- [ ] No credentials in logs or error messages?
- [ ] Token handling follows best practices?

### 2.2 TLS/Encryption (`mssql-tls`)

**Review Priority: HIGH**

- TDS 7.4 TLS negotiation (STARTTLS pattern)
- TDS 8.0 strict mode (TLS-first)
- Certificate validation
- TrustServerCertificate handling

**Key Files:**
- `crates/mssql-tls/src/connector.rs`
- `crates/mssql-tls/src/config.rs`

**Security Questions:**
- [ ] Default TLS configuration secure?
- [ ] Certificate validation cannot be bypassed accidentally?
- [ ] TrustServerCertificate generates warnings?

### 2.3 Always Encrypted (`mssql-auth/src/always_encrypted/`)

**Review Priority: HIGH**

- AEAD_AES_256_CBC_HMAC_SHA256 implementation
- RSA-OAEP key unwrapping
- Column Encryption Key (CEK) caching
- Azure Key Vault provider
- Windows Certificate Store provider

**Key Files:**
- `crates/mssql-auth/src/always_encrypted/crypto.rs`
- `crates/mssql-auth/src/always_encrypted/key_store.rs`
- `crates/mssql-auth/src/always_encrypted/azure_kv.rs`

**Security Questions:**
- [ ] Crypto implementation matches specification?
- [ ] Keys handled securely in memory?
- [ ] CEK cache has appropriate TTL?

### 2.4 Protocol Parsing (`tds-protocol`)

**Review Priority: MEDIUM**

- Packet header parsing
- Token stream parsing
- Type value decoding
- Buffer management

**Key Files:**
- `crates/tds-protocol/src/packet.rs`
- `crates/tds-protocol/src/token.rs`
- `crates/tds-protocol/src/types.rs`

**Security Questions:**
- [ ] All buffer reads bounds-checked?
- [ ] No integer overflow in length calculations?
- [ ] Malformed packets handled gracefully?

### 2.5 Input Validation

**Review Priority: MEDIUM**

- Connection string parsing
- Savepoint name validation
- Parameter binding

**Key Files:**
- `crates/mssql-client/src/config.rs`
- `crates/mssql-client/src/transaction.rs`

**Security Questions:**
- [ ] SQL injection prevented in savepoint names?
- [ ] Connection string injection prevented?
- [ ] No special characters allow command injection?

---

## 3. Dependency Review

### 3.1 Critical Dependencies

| Dependency | Purpose | Version | Audit Status |
|------------|---------|---------|--------------|
| `rustls` | TLS implementation | 0.23 | RustCrypto reviewed |
| `ring` | Cryptographic primitives | 0.17 | Google-maintained |
| `rsa` | RSA operations | 0.9 | RustCrypto |
| `aes-gcm` | AES-GCM encryption | 0.10 | RustCrypto |
| `hmac` | HMAC operations | 0.12 | RustCrypto |
| `sha2` | SHA-256/384/512 | 0.10 | RustCrypto |

### 3.2 Dependency Configuration

```bash
# Verify no known vulnerabilities
cargo audit

# Check license compliance
cargo deny check licenses

# Check for banned crates
cargo deny check bans
```

---

## 4. Documentation Review

### 4.1 Security Documentation

- [x] **SECURITY.md** - Security policy and reporting process
- [x] **Threat model** - Documented in SECURITY.md
- [x] **TLS configuration guide** - `docs/TLS.md`
- [x] **Always Encrypted guide** - Integrated in auth documentation

### 4.2 Operational Security Guidance

- [x] Production deployment checklist (`docs/DEPLOYMENT.md`)
- [x] Security hardening recommendations (`docs/OPERATIONS.md`)
- [x] Error handling guidance (no sensitive data in errors)

---

## 5. Pre-Audit Actions

### 5.1 Code Preparation

- [ ] Run full clippy with all features enabled
- [ ] Run cargo-audit and resolve any findings
- [ ] Run all fuzz targets for extended duration (1+ hour each)
- [ ] Review all TODO/FIXME comments for security implications
- [ ] Verify no hardcoded credentials or keys

### 5.2 Documentation Preparation

- [ ] Ensure all security-critical code is documented
- [ ] Document all cryptographic algorithm choices
- [ ] Create architecture diagram for security auditor
- [ ] List all external network communications

### 5.3 Test Preparation

- [ ] Ensure security-related tests exist and pass
- [ ] Document manual security test procedures
- [ ] Prepare test SQL Server with various configurations

---

## 6. Audit Scope Recommendations

### In Scope

1. **Authentication mechanisms** - All auth providers
2. **TLS implementation** - Negotiation, certificate validation
3. **Always Encrypted** - Crypto implementation, key handling
4. **Protocol parsing** - Packet/token parsing for memory safety
5. **Input validation** - Connection strings, SQL parameters
6. **Dependency chain** - Critical security dependencies

### Out of Scope

1. SQL Server itself (Microsoft's responsibility)
2. Third-party key providers (Azure, Windows CertStore)
3. Performance characteristics (unless security-impacting)

---

## 7. Known Security Considerations

### 7.1 Documented Risks

| Risk | Mitigation | Status |
|------|------------|--------|
| TrustServerCertificate bypass | Logs warning, dev-only | Documented |
| Password in connection string | Recommend env vars | Documented |
| CEK cache exposure | TTL-based expiration | Implemented |
| Credential logging | No credentials in logs | Verified |

### 7.2 Design Decisions

1. **Pure Rust crypto** - No OpenSSL dependency, uses audited RustCrypto crates
2. **No unsafe code** - Memory safety through Rust's type system
3. **Defensive parsing** - All external data validated before use
4. **Fail-closed** - Invalid states result in errors, not undefined behavior

---

## 8. Contact for Security Audit

**Primary Contact:** [Project Maintainer]
**Repository:** https://github.com/praxiomlabs/rust-mssql-driver
**Security Policy:** SECURITY.md
**Bug Bounty:** Not currently offered

---

## Appendix: Security Testing Commands

```bash
# Full security scan
cargo audit
cargo deny check

# Extended fuzz testing (run overnight)
cargo +nightly fuzz run parse_packet -- -max_total_time=3600
cargo +nightly fuzz run parse_token -- -max_total_time=3600
cargo +nightly fuzz run crypto_decode -- -max_total_time=3600

# Miri (memory safety)
cargo +nightly miri test -p tds-protocol

# All features clippy
cargo clippy --all-features --all-targets -- -D warnings
```
