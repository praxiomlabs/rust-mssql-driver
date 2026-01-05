# Test Failure Audit

This document tracks all test failures found during SQL Server version compatibility testing.

## Test Environment

| SQL Server | Version | Port | Encryption |
|------------|---------|------|------------|
| 2012 SP4 | 11.0.7001.0 | 1433 | `no_tls` |
| 2014 RTM | 12.0.2000.8 | 1434 | `no_tls` |
| 2016 SP3 | 13.0.6404.1 | 1435 | `no_tls` |
| 2022 CU22 | 16.0.4225.2 | Docker | TLS |

---

## version_compatibility.rs Failures

### 1. `test_version_detection` (Lines 171-183) ✅ FIXED

**Failed on:** SQL Server 2014, 2016

**Root cause:** Assertion only checks for 2017/2019/2022 in version string.

**Category:** Test assertion bug

**Fix applied:** Added 2008, 2012, 2014, 2016 to known versions list.

---

### 2. `test_product_version` (Lines 188-228) ✅ FIXED

**Failed on:** SQL Server 2012, 2014, 2016

**Root cause:**
- Assertion requires `major >= 14` (SQL Server 2017+)
- `SERVERPROPERTY('ProductMajorVersion')` returns NULL in SQL Server 2014 RTM

**Category:** Test assertion bug

**Fix applied:**
1. Parse major version from ProductVersion string (e.g., "11.0.7001.0" → 11)
2. Changed assertion to `>= 10` (SQL Server 2008+)

---

### 3. `test_sql_2017_features` (Lines 573-607) ✅ FIXED

**Failed on:** SQL Server 2012, 2014, 2016

**Root cause:** STRING_AGG function was introduced in SQL Server 2017.

**Category:** Expected behavior (feature not available)

**Fix applied:** Check SQL Server major version, skip if < 14.

---

### 4. `test_sql_2019_features` (Lines 655-690) ✅ FIXED

**Failed on:** SQL Server 2014

**Root cause:** `SERVERPROPERTY('ProductMajorVersion')` returns NULL in 2014 RTM.

**Category:** Test assertion bug

**Fix applied:** Parse major version from ProductVersion string.

---

### 5. `test_tds_version_negotiation` (Lines 894-941) ✅ FIXED

**Failed on:** SQL Server 2014

**Root cause:** Same NULL ProductMajorVersion issue.

**Category:** Test assertion bug

**Fix applied:** Parse major version from ProductVersion string.

---

## integration.rs Failures

### TLS Encryption Tests (Expected Behavior)

The following tests fail on legacy SQL Server instances (2012, 2014, 2016) because
they require TLS, which these servers don't support with modern TLS libraries:

- `test_connection_with_encryption_false` - Expects TLS negotiation to work
- `test_connection_with_encryption_true` - Expects TLS to be available
- `test_encrypted_query_roundtrip` - Requires encrypted connection

**Category:** Expected behavior (legacy servers don't support modern TLS)

**Status:** No fix needed - these servers require `Encrypt=no_tls` option.

---

### 6. `test_tvp_basic_int_list` (Line ~1988)

**Failed on:** SQL Server 2022 (fresh database)

**Root cause:** Test requires pre-created table type `dbo.IntIdList` which doesn't exist.

**Category:** Test setup issue

**Fix:** Create the table type in test setup, or skip test if type doesn't exist.

---

### 7. `test_tvp_bulk_insert` (Line ~2237)

**Failed on:** SQL Server 2022 (fresh database)

**Root cause:** Test requires pre-created table type `dbo.BulkRowList` which doesn't exist.

**Category:** Test setup issue

**Fix:** Create the table type in test setup, or skip test if type doesn't exist.

---

### 8. `test_tvp_empty_table` (Line ~2062)

**Failed on:** SQL Server 2022

**Root cause:** "Must declare the table variable @p1" - TVP not being recognized as table type.

**Category:** Needs investigation - possible driver bug or test issue

**Fix:** Investigate TVP parameter handling.

---

### 9. `test_tvp_multi_column` (Line ~2147)

**Failed on:** SQL Server 2022

**Root cause:** Same as above - TVP parameter not recognized.

**Category:** Needs investigation - possible driver bug or test issue

**Fix:** Investigate TVP parameter handling.

---

## Summary by Category

| Category | Count | Status |
|----------|-------|--------|
| Test assertion bug | 5 | ✅ All fixed |
| Expected behavior (version-specific) | 1 | ✅ Fixed with version check |
| Expected behavior (TLS on legacy) | 3 | ✅ Documented (no fix needed) |
| TVP test issues | 4 | ⚠️ Needs investigation (separate issue) |

---

## Test Results After Fixes

### version_compatibility.rs

| SQL Server | Passed | Failed | Notes |
|------------|--------|--------|-------|
| 2012 SP4 | 18/18 | 0 | ✅ All pass |
| 2014 RTM | 18/18 | 0 | ✅ All pass |
| 2016 SP3 | 18/18 | 0 | ✅ All pass |
| 2022 CU22 | 18/18 | 0 | ✅ All pass |

### integration.rs

| SQL Server | Passed | Failed | Notes |
|------------|--------|--------|-------|
| 2012 SP4 | 56/63 | 7 | 3 TLS (expected) + 4 TVP |
| 2014 RTM | 56/63 | 7 | 3 TLS (expected) + 4 TVP |
| 2016 SP3 | 56/63 | 7 | 3 TLS (expected) + 4 TVP |
| 2022 CU22 | 59/63 | 4 | 4 TVP only |

---

## Remaining Work

### TVP Tests (Separate Issue)

The 4 TVP test failures require further investigation:

1. Table types created in `master` database may have permission issues
2. "Must declare the table variable @p1" error suggests potential driver bug in TVP parameter encoding
3. Should be tracked as a separate issue from Issue #25

---

## Priority Order

1. ✅ **High:** Fixed all test assertion bugs (5 tests)
2. ✅ **High:** Documented expected version differences (TLS)
3. ⚠️ **Medium:** TVP tests need investigation (separate issue)

