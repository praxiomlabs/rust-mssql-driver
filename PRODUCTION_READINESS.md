# Production Readiness Checklist

This document defines the criteria for declaring rust-mssql-driver production-ready (v1.0).

## Status Overview

| Category | Status | Progress |
|----------|--------|----------|
| Core Functionality | In Progress | 90% |
| Test Coverage | In Progress | ~60% |
| Documentation | In Progress | 70% |
| Security | In Progress | 80% |
| Operations | Not Started | 0% |
| Performance | Not Started | 0% |

**Overall Readiness: 50%** (estimated)

---

## 1. Core Functionality Criteria

### 1.1 Connection Management
- [x] TCP/IP connections to SQL Server
- [x] TLS encryption (TDS 7.4 mode)
- [x] TDS 8.0 strict encryption mode
- [x] Connection timeout handling
- [x] Graceful connection close
- [x] Azure SQL redirect handling
- [ ] Connection recovery after network partition

### 1.2 Query Execution
- [x] Simple queries (SQL batch)
- [x] Parameterized queries (sp_executesql)
- [x] Streaming result sets
- [x] Multiple result sets per query
- [x] Row count reporting
- [ ] Query cancellation (ATTENTION signal)

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
- [ ] Streaming LOB support (currently buffered)

### 1.5 Connection Pooling
- [x] Configurable pool size
- [x] Connection health checking
- [x] sp_reset_connection on return
- [x] Timeout on connection acquisition
- [ ] Pool metrics exposed

---

## 2. Test Coverage Criteria

**Target: 80% line coverage**

### 2.1 Unit Tests
- [x] Protocol encoding/decoding
- [x] Type conversions
- [x] Connection string parsing
- [ ] Error handling paths

### 2.2 Integration Tests
- [x] Mock TDS server tests
- [ ] Real SQL Server 2019 tests
- [ ] Real SQL Server 2022 tests
- [ ] Azure SQL Database tests

### 2.3 Property-Based Tests
- [x] proptest for type encoding
- [ ] Protocol fuzzing (3 targets exist, expand to 10+)

### 2.4 Edge Case Tests
- [ ] NULL handling edge cases
- [ ] Unicode boundary conditions
- [ ] Large dataset handling
- [ ] Timeout scenarios
- [ ] Connection exhaustion

### 2.5 Performance Tests
- [ ] Benchmark suite with criterion
- [ ] Baseline performance established
- [ ] No significant regressions

---

## 3. Documentation Criteria

### 3.1 User Documentation
- [x] README with quick start
- [x] API documentation (docs.rs)
- [x] Connection string reference
- [ ] Production deployment guide
- [ ] Troubleshooting guide
- [ ] Migration guide (from Tiberius)

### 3.2 Operational Documentation
- [ ] Error code reference
- [ ] Metrics interpretation guide
- [ ] Timeout configuration guide
- [ ] Retry/backoff recommendations

### 3.3 Security Documentation
- [x] SECURITY.md with reporting process
- [ ] Threat model documentation
- [ ] TLS configuration guide

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
- [ ] Security audit completed

### 4.3 Operational Security
- [x] TLS certificate validation
- [x] TDS 8.0 strict mode support
- [ ] zeroize feature for credential wiping

---

## 5. Operations Criteria

### 5.1 Observability
- [x] Structured logging (tracing)
- [x] OpenTelemetry integration (optional)
- [ ] Pool metrics documented
- [ ] Error categorization documented

### 5.2 Reliability
- [ ] Connection recovery tested
- [ ] Pool recovery after SQL Server restart
- [ ] Graceful shutdown behavior documented

### 5.3 Performance
- [ ] Memory allocation patterns documented
- [ ] No memory leaks under stress
- [ ] Acceptable latency percentiles

---

## 6. CI/CD Criteria

### 6.1 Continuous Integration
- [x] Build verification
- [x] Unit test execution
- [ ] Integration tests with real SQL Server
- [ ] Cross-platform testing (Linux, macOS, Windows)
- [ ] MSRV verification (1.85)

### 6.2 Release Process
- [x] Semantic versioning
- [x] Changelog maintained
- [x] Release documentation
- [ ] Automated release workflow

---

## 7. Compatibility Criteria

### 7.1 SQL Server Versions
- [x] SQL Server 2022 (TDS 8.0)
- [x] SQL Server 2019 (TDS 7.4)
- [x] SQL Server 2017 (TDS 7.4)
- [x] SQL Server 2016 (TDS 7.4)
- [x] Azure SQL Database
- [x] Azure SQL Managed Instance

### 7.2 Rust Versions
- [x] MSRV 1.85 documented
- [ ] MSRV enforced in CI

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
| 80% test coverage | Testing | Not Met |
| Real SQL Server CI | Testing | Not Met |
| Security audit | Security | Not Scheduled |
| Production deployment guide | Documentation | Not Started |
| Error code reference | Documentation | Not Started |
| Pool metrics documentation | Operations | Not Started |
| Cross-platform CI | CI/CD | Not Met |

---

## Acceptable Limitations for 1.0

The following are documented as known limitations, acceptable for initial release:

| Limitation | Workaround | Future Plan |
|------------|------------|-------------|
| No MARS | Use connection pool | v2.0 |
| Buffered LOBs | Memory buffer <100MB | v1.1 |
| No Kerberos auth | SQL or Azure AD auth | v1.1 |
| No NTLM auth | SQL or Azure AD auth | v1.1 |
| No Always Encrypted | Application-layer encryption | v2.0 |

---

## Progress Tracking

### Metrics

| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Line Coverage | 80% | ~60% | Not Met |
| Doc Coverage | 100% public | TBD | Unknown |
| Fuzz Targets | 10+ | 3 | Not Met |
| Benchmark Suite | Complete | None | Not Met |
| Security Audit | Pass | N/A | Not Scheduled |

### Milestones

| Milestone | Target Date | Status |
|-----------|-------------|--------|
| 0.1.0 Release | 2025-12-16 | Ready |
| Test Coverage 80% | TBD | In Progress |
| Security Audit | TBD | Not Scheduled |
| 1.0.0 Release | TBD | Blocked |

---

## Verification Commands

```bash
# Check test coverage
cargo llvm-cov --workspace

# Run all tests
cargo test --workspace --all-features

# Check for security issues
cargo deny check

# Verify documentation
cargo doc --workspace --no-deps

# Run benchmarks
cargo bench --workspace

# MSRV verification
cargo +1.85 build --workspace
```

---

## Sign-off Requirements

Before 1.0 release, the following sign-offs are required:

- [ ] **Technical Lead**: All blockers resolved
- [ ] **Security Review**: No Critical/High findings
- [ ] **Documentation Review**: All required docs complete
- [ ] **Performance Review**: No regressions, acceptable latency
