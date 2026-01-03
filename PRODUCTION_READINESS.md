# Production Readiness Checklist

This document defines the criteria for declaring rust-mssql-driver production-ready (v1.0).

## Status Overview

| Category | Status | Progress |
|----------|--------|----------|
| Core Functionality | Complete | 98% |
| Test Coverage | In Progress | ~46% (line), ~52% (function) |
| Documentation | Complete | 98% |
| Security | Complete | 90% |
| Operations | Complete | 98% |
| Performance | Complete | 90% |

**Overall Readiness: 85%** (estimated)

> **Note:** Test coverage is measured via `cargo llvm-cov`. Many code paths require
> live SQL Server connections to exercise (network I/O, authentication, etc.).
> Unit-testable code has higher coverage (~75%+), while integration-dependent
> code awaits CI with Docker SQL Server.

---

## 1. Core Functionality Criteria

### 1.1 Connection Management
- [x] TCP/IP connections to SQL Server
- [x] TLS encryption (TDS 7.4 mode)
- [x] TDS 8.0 strict encryption mode
- [x] Connection timeout handling
- [x] Graceful connection close
- [x] Azure SQL redirect handling
- [x] Connection recovery (health checks, automatic reconnection, pool warm-up)

### 1.2 Query Execution
- [x] Simple queries (SQL batch)
- [x] Parameterized queries (sp_executesql)
- [x] Streaming result sets
- [x] Multiple result sets per query
- [x] Row count reporting
- [x] Query cancellation (ATTENTION signal)

### 1.3 Transactions
- [x] BEGIN/COMMIT/ROLLBACK
- [x] Savepoints (SAVE TRANSACTION / ROLLBACK TO)
- [x] Isolation level configuration
- [x] Type-state transaction safety

### 1.4 Data Types
- [x] All standard SQL Server types
- [x] chrono DateTime support
- [x] rust_decimal Decimal support
- [x] uuid UUID support
- [x] serde_json JSON support
- [x] NULL handling
- [x] Collation-aware VARCHAR decoding (v0.5.0+)
- [ ] Streaming LOB support (currently buffered up to 100MB)

### 1.5 Connection Pooling
- [x] Configurable pool size
- [x] Connection health checking
- [x] sp_reset_connection on return
- [x] Timeout on connection acquisition
- [x] Pool metrics exposed (PoolStatus, PoolMetrics)

### 1.6 Authentication
- [x] SQL Authentication
- [x] Azure AD Token Authentication
- [x] Azure Managed Identity (`azure-identity` feature)
- [x] Kerberos/GSSAPI (`integrated-auth` feature)
- [x] Windows SSPI (`sspi-auth` feature)
- [x] Client Certificate Authentication

### 1.7 Always Encrypted
- [x] AEAD_AES_256_CBC_HMAC_SHA256 encryption
- [x] RSA-OAEP key unwrapping
- [x] Azure Key Vault provider (`azure-identity` feature)
- [x] Windows Certificate Store provider (`sspi-auth` feature)
- [x] Custom KeyStoreProvider trait

---

## 2. Test Coverage Criteria

**Target: 60% line coverage** (revised from 80% - see note in Blockers section)

### 2.1 Unit Tests
- [x] Protocol encoding/decoding
- [x] Type conversions
- [x] Connection string parsing
- [x] Error handling paths (`tests/error_handling.rs`)

### 2.2 Integration Tests
- [x] Mock TDS server tests
- [x] Real SQL Server tests (via CI with Docker)
- [x] Azure SQL Database tests (`tests/azure_sql.rs`, requires live Azure)

### 2.3 Property-Based Tests
- [x] proptest for type encoding
- [x] Protocol fuzzing (11 fuzz targets)

### 2.4 Edge Case Tests
- [x] NULL handling edge cases (`tests/edge_cases.rs`)
- [x] Unicode boundary conditions (`tests/edge_cases.rs`)
- [x] Large dataset handling (`tests/edge_cases.rs`)
- [x] Timeout scenarios (`tests/timeout_scenarios.rs`)
- [x] Connection exhaustion (`tests/timeout_scenarios.rs`)

### 2.5 Performance Tests
- [x] Benchmark suite with criterion (3 crates have benchmarks)
- [x] Baseline performance documented (`docs/PERFORMANCE_BASELINES.md`)
- [x] CI regression detection (`.github/workflows/benchmarks.yml`)

---

## 3. Documentation Criteria

### 3.1 User Documentation
- [x] README with quick start
- [x] API documentation (docs.rs)
- [x] Connection string reference (`docs/CONNECTION_STRINGS.md`)
- [x] Production deployment guide (`docs/DEPLOYMENT.md`)
- [x] Troubleshooting guide (`docs/TROUBLESHOOTING.md`)
- [x] Migration guide from Tiberius (`docs/MIGRATION_FROM_TIBERIUS.md`)

### 3.2 Operational Documentation
- [x] Error code reference (`docs/ERRORS.md`)
- [x] Pool metrics interpretation guide (`docs/POOL_METRICS.md`)
- [x] Timeout configuration guide (`docs/TIMEOUTS.md`)
- [x] Retry/backoff recommendations (`docs/RETRY_STRATEGY.md`)
- [x] Memory allocation patterns (`docs/MEMORY.md`)
- [x] Operations guide with graceful shutdown (`docs/OPERATIONS.md`)
- [x] Stress testing guide (`docs/STRESS_TESTING.md`)
- [x] Performance baselines (`docs/PERFORMANCE_BASELINES.md`)

### 3.3 Security Documentation
- [x] SECURITY.md with reporting process
- [x] Threat model documentation (in SECURITY.md)
- [x] TLS configuration guide (`docs/TLS.md`)

---

## 4. Security Criteria

### 4.1 Code Security
- [x] No unsafe code (or audited unsafe)
- [x] Input validation (savepoint names)
- [x] No SQL injection in internal queries
- [x] Credentials not logged

### 4.2 Dependency Security
- [x] cargo-deny configured
- [x] No known vulnerabilities
- [ ] Third-party security audit (recommended but not blocking for v1.0)

> **Security Audit Status:** A formal third-party security audit is recommended
> before production deployment in high-security environments. The codebase has
> been developed with security best practices (no unsafe code, input validation,
> credential protection, TLS enforcement) and passes cargo-deny checks. An audit
> would validate these practices but is not an absolute blocker for v1.0.

### 4.3 Operational Security
- [x] TLS certificate validation
- [x] TDS 8.0 strict mode support
- [x] zeroize feature for credential wiping

---

## 5. Operations Criteria

### 5.1 Observability
- [x] Structured logging (tracing)
- [x] OpenTelemetry integration (`otel` feature)
- [x] Pool metrics documented (`docs/POOL_METRICS.md`)
- [x] Error categorization documented (`docs/ERRORS.md`)

### 5.2 Reliability
- [x] Connection recovery documented (`docs/CONNECTION_RECOVERY.md`)
- [x] Pool recovery after SQL Server restart (via health checks)
- [x] Graceful shutdown behavior documented (`docs/OPERATIONS.md`)

### 5.3 Performance
- [x] Memory allocation patterns documented (`docs/MEMORY.md`)
- [x] Stress testing framework (`tests/stress_tests.rs`, `docs/STRESS_TESTING.md`)
- [x] Performance baselines established (`docs/PERFORMANCE_BASELINES.md`)

---

## 6. CI/CD Criteria

### 6.1 Continuous Integration
- [x] Build verification
- [x] Unit test execution
- [x] Integration tests with real SQL Server (Docker in CI)
- [x] Cross-platform testing (Linux, macOS, Windows)
- [x] MSRV verification (1.85)
- [x] Miri for unsafe code detection
- [x] Semver checks (advisory for pre-1.0)
- [x] Code coverage reporting (Codecov)

### 6.2 Release Process
- [x] Semantic versioning
- [x] Changelog maintained
- [x] Release documentation
- [x] Automated release workflow (`.github/workflows/release.yml`)

---

## 7. Compatibility Criteria

### 7.1 SQL Server Versions
- [x] SQL Server 2022 (TDS 8.0)
- [x] SQL Server 2019 (TDS 7.4)
- [x] SQL Server 2017 (TDS 7.4)
- [x] SQL Server 2016 (TDS 7.4)
- [x] SQL Server 2008/2008 R2 (TDS 7.3, v0.4.0+)
- [x] Azure SQL Database
- [x] Azure SQL Managed Instance

### 7.2 Rust Versions
- [x] MSRV 1.85 documented
- [x] MSRV enforced in CI

### 7.3 Platforms
- [x] Linux x86_64
- [x] macOS x86_64
- [x] macOS aarch64
- [x] Windows x86_64

---

## Blockers for 1.0 Release

The following items MUST be completed before v1.0:

| Item | Category | Status |
|------|----------|--------|
| 60% line coverage | Testing | In Progress (~46%) |
| Security audit | Security | Not Scheduled |

> **Coverage Target Adjustment:** The 80% target has been revised to 60% as a more
> realistic goal given that significant code paths require live SQL Server for testing.
> The CI pipeline runs integration tests against Docker SQL Server, which exercises
> these code paths but isn't reflected in local coverage reports.

---

## Acceptable Limitations for 1.0

The following are documented as known limitations, acceptable for initial release:

| Limitation | Workaround | Future Plan |
|------------|------------|-------------|
| No MARS | Use connection pool | v2.0 |
| Buffered LOBs | Memory buffer <100MB | v1.1 |
| No NTLM auth | SQL, Azure AD, or Kerberos | No plan |

---

## Progress Tracking

### Metrics

| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Line Coverage | 60% | ~46% | In Progress |
| Function Coverage | 60% | ~52% | In Progress |
| Doc Coverage | 100% public | ~98% | Met |
| Fuzz Targets | 10+ | 11 | Met |
| Benchmark Suite | Complete | Documented | Met |
| Performance Baselines | Documented | Yes | Met |
| Stress Tests | Exist | Yes | Met |
| Security Audit | Pass | N/A | Not Scheduled |

> **Coverage Note:** Current coverage reflects unit tests only. Integration tests
> that run against live SQL Server (available in CI) exercise significant additional
> code paths including connection handling, query execution, and protocol parsing.
>
> **Modules with low unit test coverage by design:**
> - `client.rs` (15%): Main connection logic requires live SQL Server
> - `cancel.rs` (13%): Query cancellation requires active connections
> - `azure_identity_auth.rs` (8%): Requires Azure Managed Identity environment
> - `integrated_auth.rs` (34%): Requires Kerberos/GSSAPI setup
> - `sspi_auth.rs` (36%): Requires Windows SSPI or sspi-rs configuration
> - `cert_auth.rs` (15%): Requires certificate files and Azure service principal
>
> These modules are covered by integration tests in CI with Docker SQL Server.

### Milestones

| Milestone | Target Date | Status |
|-----------|-------------|--------|
| 0.1.0 Release | 2025-12-16 | Complete |
| 0.5.0 Release | 2026-01-01 | Complete |
| 0.5.1 Release | 2026-01-03 | In Progress |
| Test Coverage 60% | TBD | In Progress (46%) |
| Security Audit | TBD | Not Scheduled |
| 1.0.0 Release | TBD | Blocked |

---

## Verification Commands

```bash
# Check test coverage
cargo llvm-cov --workspace --all-features

# Run all tests
cargo nextest run --workspace --all-features

# Check for security issues
cargo deny check

# Verify documentation
cargo doc --workspace --no-deps --all-features

# Run benchmarks (once created)
cargo bench --workspace

# MSRV verification
cargo +1.85 check --workspace --all-features
```

---

## Sign-off Requirements

Before 1.0 release, the following sign-offs are required:

- [ ] **Technical Lead**: All blockers resolved
- [ ] **Security Review**: No Critical/High findings
- [ ] **Documentation Review**: All required docs complete
- [ ] **Performance Review**: Benchmarks established, no regressions
