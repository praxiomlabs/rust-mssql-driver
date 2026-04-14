window.BENCHMARK_DATA = {
  "lastUpdate": 1776124858866,
  "repoUrl": "https://github.com/praxiomlabs/rust-mssql-driver",
  "entries": {
    "Rust Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "distinct": true,
          "id": "0d781860e315406c81d6cae51dbbf541d2e28383",
          "message": "fix(ci): clean working directory before benchmark gh-pages push\n\nThe benchmark action with auto-push tries to switch to gh-pages branch,\nbut Cargo.lock gets modified during benchmark runs. Git refuses to switch\nwith uncommitted changes. Reset Cargo.lock before the action runs.",
          "timestamp": "2026-01-03T23:53:06Z",
          "tree_id": "f8eac084fd87a9dbf6fe99d1cfd77a1e37530898",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/0d781860e315406c81d6cae51dbbf541d2e28383"
        },
        "date": 1767484946660,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 401,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 408,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 489,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 696,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 90,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 128,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 78,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 551,
            "range": "± 27",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3505,
            "range": "± 137",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 251,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 140,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 646,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 37,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 138,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 74,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 900,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2437,
            "range": "± 93",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "distinct": true,
          "id": "22d903f9638080545651b7655c59164ee2ec1d76",
          "message": "docs(release): add git hygiene protocol to prevent v0.5.1 pattern\n\n- Add Cardinal Rule #7: never commit directly to main during releases\n- Add dedicated \"Git Hygiene Protocol\" section with:\n  - Release branch workflow (branch → batch → local checks → push once)\n  - Explicit \"What NOT To Do\" showing v0.5.1 bad pattern\n  - \"Why This Matters\" cost table (CI runs, messy history)\n  - Quick reference checklist\n- Include Cargo.lock restoration from v0.5.1 manual publish recovery\n\nThis documents the lesson learned from v0.5.1 where multiple small\ncommits to main triggered repeated CI runs and created messy history.",
          "timestamp": "2026-01-04T00:05:50Z",
          "tree_id": "ad05770ea2c6af80a2b33b57225c603802614e41",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/22d903f9638080545651b7655c59164ee2ec1d76"
        },
        "date": 1767485620146,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 412,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 414,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 498,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 701,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 87,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 125,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 77,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 571,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3732,
            "range": "± 188",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 253,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 141,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 654,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 40,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 42,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 147,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 65,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 901,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2435,
            "range": "± 43",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "37b8ac58e6409302c613bf9c3a683ed5001b7810",
          "message": "fix: SQL Server 2008-2016 compatibility (TLS, version detection, TVP) (#28)\n\n* fix(protocol): correctly distinguish SQL Server version from TDS version\n\nPreLogin VERSION field from the server contains SQL Server product\nversion (e.g., 13.0 for SQL Server 2016), NOT TDS protocol version.\nThe old logging incorrectly displayed these as \"TDS 7.13\" or \"TDS 7.11\"\nwhich caused confusion (Issue #25).\n\nChanges:\n- Add SqlServerVersion type in version.rs to represent SQL Server\n  product versions with proper Display formatting\n- Add product_name() method mapping major versions to product names\n  (SQL Server 2000 through 2022)\n- Add max_tds_version() method to determine supported TDS version\n  from product version\n- Update PreLogin struct with server_version field (deprecate sub_build)\n- Fix client.rs logging to correctly show requested_tds_version,\n  server_product_version, server_product, and max_tds_version\n- Add warning when server's max TDS version is lower than requested\n\nFixes part of #25\n\n* feat(client): add DANGER_PLAINTEXT option for unencrypted connections\n\nAdd Tiberius-compatible option to completely disable TLS encryption.\nThis allows connecting to legacy SQL Server instances (2008 and earlier)\nthat only support TLS 1.0/1.1, which modern TLS libraries like rustls\ndon't support for security reasons.\n\nConnection string usage:\n  Encrypt=DANGER_PLAINTEXT\n\nBuilder API:\n  Config::new().danger_plaintext(true)\n\nWhen enabled:\n- Sends ENCRYPT_NOT_SUP in PreLogin packet\n- No TLS handshake occurs\n- All traffic including credentials is unencrypted\n- Logs a security warning at connection time\n\nThis should only be used for development/testing on trusted networks\nwith legacy SQL Server instances that cannot be upgraded.\n\nAddresses part of #25 (SQL Server 2012 TLS issue)\n\n* refactor(client): rename danger_plaintext to no_tls\n\nRename the unencrypted connection option from DANGER_PLAINTEXT to\nno_tls for cleaner connection string syntax.\n\nConnection string:\n  Encrypt=no_tls\n\nBuilder API:\n  Config::new().no_tls(true)\n\n* docs: document no_tls option and update encryption references\n\n- README.md: Add no_tls to Encrypt options table\n- ARCHITECTURE.md: Add no_tls to connection string keywords table\n- LIMITATIONS.md: Add TLS compatibility section explaining rustls TLS 1.2+\n  requirement and no_tls workaround for legacy SQL Server\n- CHANGELOG.md: Add unreleased changes for no_tls and SqlServerVersion\n\n* fix(tests): make version_compatibility tests work on SQL Server 2008-2016\n\nPreviously the version_compatibility tests only worked on SQL Server 2017+\ndue to incorrect assertions and use of ProductMajorVersion (which returns\nNULL in SQL Server 2014 RTM).\n\nFixed tests:\n- test_version_detection: Added 2008/2012/2014/2016 to known versions\n- test_product_version: Parse major version from ProductVersion string\n  instead of ProductMajorVersion; changed assertion from >= 14 to >= 10\n- test_sql_2017_features: Added version check to skip on < 2017\n- test_sql_2019_features: Parse from ProductVersion instead of\n  ProductMajorVersion\n- test_tds_version_negotiation: Same fix as above\n\nAlso fixed integration.rs to support MSSQL_PORT environment variable\nfor testing against SQL Server instances on non-default ports.\n\nAll 18 version_compatibility tests now pass on:\n- SQL Server 2012 SP4 (11.0.7001.0)\n- SQL Server 2014 RTM (12.0.2000.8)\n- SQL Server 2016 SP3 (13.0.6404.1)\n- SQL Server 2022 CU22 (16.0.4225.2)\n\nAdded docs/TEST_FAILURE_AUDIT.md documenting test analysis.\n\n* docs: add SQL Server version compatibility matrix\n\nDocuments supported SQL Server versions (2008-2022), TDS protocol versions,\nTLS requirements, and feature availability by version.\n\nKey information:\n- SQL Server 2008-2016 require Encrypt=no_tls due to TLS 1.2 requirement\n- ProductMajorVersion returns NULL in SQL Server 2014 RTM\n- Feature matrix showing STRING_AGG (2017+), APPROX_COUNT_DISTINCT (2019+)\n- Version detection mapping (major version to product name)\n\n* fix(tds-protocol): correctly declare TVP parameters with table type names\n\nTVP (Table-Valued Parameter) RPC calls were failing with \"Must declare\nthe table variable @p1\" because build_param_declarations() was generating\n\"@p1 sql_variant\" instead of \"@p1 dbo.IntIdList READONLY\".\n\nChanges:\n- Add tvp_type_name field to TypeInfo struct to carry the table type name\n- Add TypeInfo::tvp() constructor for TVP parameters\n- Update build_param_declarations() to handle type_id 0xF3 with proper\n  table type declaration format\n- Update encode_tvp_param() to pass full type name through TypeInfo::tvp()\n\nFixes table-valued parameter support on all SQL Server versions (2008-2022).\n\n* fix(tests): update integration tests for legacy SQL Server compatibility\n\n- Add should_skip_tls_tests() helper to detect legacy servers that don't\n  support TLS 1.2 (required by rustls)\n- Rewrite TVP tests to use inline queries instead of temporary stored\n  procedures (SQL Server limitation: temp procedures cannot reference\n  user-defined table types)\n- Update TLS encryption tests to skip on legacy servers\n\nTested on SQL Server 2008 R2, 2012, 2014, 2016, and 2022.\n\n* docs: update test audit and changelog for TVP and compatibility fixes\n\n- Update TEST_FAILURE_AUDIT.md with complete fix status for all test\n  failures found during SQL Server 2008-2016 compatibility testing\n- Add TVP parameter declaration fix to CHANGELOG.md\n\nAll 63 integration tests and 18 version compatibility tests now pass\non SQL Server 2008 R2, 2012, 2014, 2016, and 2022.",
          "timestamp": "2026-01-04T22:01:56-06:00",
          "tree_id": "5fd76cedf8b83d17efc2fbc4789b63203a18ff38",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/37b8ac58e6409302c613bf9c3a683ed5001b7810"
        },
        "date": 1767586392397,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 404,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 408,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 486,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 689,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 23,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 89,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 127,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 75,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 492,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3591,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 280,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 141,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 657,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 40,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 26,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 41,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 148,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 68,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 856,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2185,
            "range": "± 125",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cfd9f215016605386289ee7b4e36fee3ae8efeb5",
          "message": "fix(pool): implement sp_reset_connection using TDS RESETCONNECTION flag (#30)\n\nFixes #26 - sp_reset_connection was documented but not implemented.\n\nThe implementation uses the TDS protocol's RESETCONNECTION packet flag\n(0x08) rather than calling sp_reset_connection as a stored procedure.\nThis is the same mechanism used by ADO.NET and is more efficient as\nthe reset happens as part of the next request with no extra round-trip.\n\nChanges:\n- Add needs_reset flag to Client for lazy reset on next use\n- Add send_message_with_reset() to Connection for setting the flag\n- Update pool Drop to mark connections for reset on checkin\n- Remove unused reset_on_return config (redundant with sp_reset_connection)\n- Update pool README to document the TDS protocol mechanism",
          "timestamp": "2026-01-04T23:52:05-06:00",
          "tree_id": "3a060af4b221d9c4a5a21a7f8500c731f7f0964e",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/cfd9f215016605386289ee7b4e36fee3ae8efeb5"
        },
        "date": 1767593011373,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 403,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 408,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 486,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 687,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 89,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 128,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 542,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3578,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 274,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 141,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 646,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 42,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 147,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 71,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 901,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2436,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "distinct": true,
          "id": "a0216585b60883e7a39c9e836ac955a3d1213cc9",
          "message": "fix(ci): use git baseline for semver checks to avoid workspace dep issues",
          "timestamp": "2026-01-04T23:59:54-06:00",
          "tree_id": "ac15d73637b2687ff913dbe0b72b2d9dd24d0932",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/a0216585b60883e7a39c9e836ac955a3d1213cc9"
        },
        "date": 1767593475349,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 400,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 410,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 485,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 686,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 89,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 126,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 77,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 489,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3712,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 281,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 44,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 142,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 663,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 26,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 43,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 148,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 69,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 90,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 900,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2436,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "distinct": true,
          "id": "b0e1b496ed93bb60417c691304cbe188d5fe03da",
          "message": "fix(deps): remove circular dev-dependency between mssql-client and mssql-driver-pool\n\nThe dev-dependency created a publishing deadlock where neither crate\ncould be published first. Pool-dependent tests removed from mssql-client;\nthey can be restored to mssql-testing with proper imports in a follow-up.",
          "timestamp": "2026-01-05T00:11:45-06:00",
          "tree_id": "7ae96755214924bf562527ed0738c92f36a2a1b9",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/b0e1b496ed93bb60417c691304cbe188d5fe03da"
        },
        "date": 1767594188412,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 399,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 404,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 484,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 695,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 22,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 89,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 127,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 80,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 576,
            "range": "± 35",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3739,
            "range": "± 48",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 281,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 141,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 647,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 41,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 147,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 70,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 901,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2437,
            "range": "± 5",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "xander.xiao@gmail.com",
            "name": "c5soft",
            "username": "c5soft"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ce810ac0a63a7e66cb3859f8f6eecf086cf18150",
          "message": "fix(client): correct varchar decoding for non-UTF8 encodings and refactor Money parsing (#31)\n\n## Bug Fix: VARCHAR Decoding\n\nRemoves the UTF-8 fast-path check in `decode_varchar_string` that was incorrectly\ndecoding non-UTF8 encoded strings (GBK, Shift-JIS, etc.) as garbage.\n\nThe issue: `std::str::from_utf8()` can return `Ok` for non-UTF8 bytes that happen\nto be valid UTF-8 sequences, but the decoded string is incorrect.\n\nNow always tries collation-aware decoding first (when `encoding` feature enabled),\nthen falls back to lossy UTF-8.\n\n## Enhancement: Money Type Parsing\n\n- Adds `parse_money_value()` helper to consolidate Money/SmallMoney/MoneyN parsing\n- Returns `rust_decimal::Decimal` when `decimal` feature is enabled (important for\n  financial applications where f64 precision loss is unacceptable)\n- Falls back to f64 when `decimal` feature is not enabled\n\nCo-authored-by: c5soft <c5soft@189.cn>",
          "timestamp": "2026-01-12T15:35:45-06:00",
          "tree_id": "c4727a2ea968d9c1d9a7f71a546827d282b9ed43",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/ce810ac0a63a7e66cb3859f8f6eecf086cf18150"
        },
        "date": 1768254521370,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 401,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 408,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 485,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 689,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 22,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 92,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 138,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 84,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 573,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3165,
            "range": "± 148",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 273,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 142,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 664,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 26,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 44,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 148,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 67,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 901,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2437,
            "range": "± 8",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "distinct": true,
          "id": "1b05ea72be3042ba5f0907a3d51aa65b2516ad16",
          "message": "refactor(client): clean up commented code from PR #31\n\nRemove commented-out UTF-8 fast-path code and update comment to reflect\nthe new behavior (collation-aware decoding first).\n\nFollow-up to ce810ac.",
          "timestamp": "2026-01-12T15:37:04-06:00",
          "tree_id": "8c11a464b16f54da31b8486f7b5e9491d20ace7e",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/1b05ea72be3042ba5f0907a3d51aa65b2516ad16"
        },
        "date": 1768254608813,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 405,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 411,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 489,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 693,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 90,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 129,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 85,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 556,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3715,
            "range": "± 186",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 274,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 48,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 145,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 655,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 41,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 148,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 68,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 90,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 901,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2436,
            "range": "± 28",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "25314b9dbc8174c6b077995bc1e97fb37c5ea246",
          "message": "fix(deps): update dependencies to resolve security advisories (#44)\n\n- rkyv 0.7.45 → 0.7.46 (RUSTSEC-2026-0001: UB in Arc/Rc from_value on OOM)\n- lru 0.16.2 → 0.16.3 (RUSTSEC-2026-0002: IterMut Stacked Borrows violation)\n\nAlso updates 47 other dependencies to latest compatible versions.",
          "timestamp": "2026-01-12T15:45:57-06:00",
          "tree_id": "23816a17a18f65344f52ba47e7cff2416de03845",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/25314b9dbc8174c6b077995bc1e97fb37c5ea246"
        },
        "date": 1768255129363,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 391,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 395,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 476,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 681,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 32,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 91,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 139,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 1,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 1,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 85,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 568,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3650,
            "range": "± 47",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 289,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 46,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 124,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 589,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 8,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 8,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 48,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 28,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 36,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 8,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 1,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 47,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 15,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 157,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 69,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 102,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 1015,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2753,
            "range": "± 29",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a899459c4b993e065dd2fd8c259900a81a0e6aff",
          "message": "feat(auth): add PEM certificate support for CertificateAuth (#45)\n\nAdd `CertificateAuth::from_pem()` constructor that accepts PEM-encoded\ncertificate and private key files, common in Linux/Kubernetes environments.\n\nImplementation:\n- Parse PEM using rustls-pemfile (already in dependency tree)\n- Convert to PKCS#12 in-memory using p12 crate (pure Rust)\n- Pass converted certificate to existing Azure Identity SDK\n\nThis eliminates the manual `openssl pkcs12 -export` step for users\nwith PEM certificates.\n\nCloses #27",
          "timestamp": "2026-01-12T16:18:55-06:00",
          "tree_id": "e008673f146a8c665c080a53dff8d01dd163b0d0",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/a899459c4b993e065dd2fd8c259900a81a0e6aff"
        },
        "date": 1768257012044,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 402,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 410,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 490,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 698,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 91,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 128,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 78,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 534,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3776,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 289,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 144,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 661,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 39,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 26,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 42,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 146,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 69,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 858,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2436,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f186c0c68296c80f6e1e92aa58bd78c9e7635b9f",
          "message": "chore: release v0.6.0 (#46)\n\n- Add PEM certificate support via `CertificateAuth::from_pem()`\n- Add Decimal support for Money types with `decimal` feature\n- Fix VARCHAR decoding for non-UTF8 encodings (GBK, Shift-JIS, etc.)\n- Fix security vulnerabilities RUSTSEC-2026-0001 (rkyv), RUSTSEC-2026-0002 (lru)\n- Consolidate Money type parsing into single helper function",
          "timestamp": "2026-01-13T12:26:00-06:00",
          "tree_id": "3cbff8a6efc7e1352128e1ff4f2eb35e4fd878ca",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/f186c0c68296c80f6e1e92aa58bd78c9e7635b9f"
        },
        "date": 1768329442750,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 405,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 416,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 489,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 698,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 10,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 96,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 133,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 78,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 551,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3209,
            "range": "± 143",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 250,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 144,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 657,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 50,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 147,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 69,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 901,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2312,
            "range": "± 77",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "74f4ab42dd2ae02d45364de2b7202407ca48b30a",
          "message": "feat(client): make TLS dependencies optional via `tls` feature flag (#48)\n\nResolves #47\n\nTLS dependencies (rustls, webpki, etc.) are now behind an optional `tls`\nfeature that is enabled by default. This allows users who only need\n`Encrypt=no_tls` connections to reduce binary size by ~2-3 MB and\nimprove compilation times.\n\nChanges:\n- Add `tls` feature to mssql-client Cargo.toml (default enabled)\n- Make mssql-tls dependency optional\n- Wrap all TLS-related types, imports, and code paths with cfg(feature = \"tls\")\n- Add compile-time errors when TLS is required but feature is disabled\n- Create dedicated connect_no_tls method for plain TCP connections\n\nUse cases:\n- Enterprise internal networks with disabled encryption\n- Kubernetes clusters with service mesh encryption\n- Legacy SQL Server environments\n- Development/testing environments",
          "timestamp": "2026-01-13T16:00:54-06:00",
          "tree_id": "50bb2992c1c4d9e9ffdefb8199d067dcde0c6f17",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/74f4ab42dd2ae02d45364de2b7202407ca48b30a"
        },
        "date": 1768342329396,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 402,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 411,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 487,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 691,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 105,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 136,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 79,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 573,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3729,
            "range": "± 37",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 250,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 143,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 667,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 39,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 13,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 41,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 146,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 69,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 900,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2686,
            "range": "± 3",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ca3c9a70c225ae46b6493cc9c1c1524ea0e29fbd",
          "message": "Merge pull request #69 from praxiomlabs/release/v0.7.0\n\nRelease v0.7.0 — security, SSPI integrated auth, pre-1.0 hardening",
          "timestamp": "2026-04-07T17:58:18-05:00",
          "tree_id": "3a50d481f4ea0552b064faaa8e1edefd80168223",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/ca3c9a70c225ae46b6493cc9c1c1524ea0e29fbd"
        },
        "date": 1775603465611,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 409,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 403,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 497,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 721,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 32,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 124,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 1,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 1,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 84,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 533,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3630,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 274,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 46,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 170,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 860,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 8,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 8,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 48,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 27,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 36,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 8,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 1,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 46,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 15,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 154,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 73,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 101,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 965,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2611,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jkindrix@gmail.com",
            "name": "Justin Kindrix",
            "username": "jkindrix"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4ecfdf49a8cb25a6be05f0062146290301107572",
          "message": "Release v0.8.0 (#82)\n\n* ci: run on dev branch, add workflow_dispatch, add token health check\n\nPost-v0.7.0 release hygiene improvements addressing three pain points\nsurfaced during the 0.7.0 release cycle:\n\n## 1. CI on dev branch\n\nci.yml and benchmarks.yml now trigger on pushes to dev, not just main.\nThe 42-commit backlog that shipped in v0.7.0 had zero CI coverage between\nthe last main-branch CI run (2026-01-13) and the release PR (#69). That's\nhow the mock_server TLS test failure on macOS/Windows and the xtask\n--no-dev-deps bug were both latent for months before surfacing at release\ntime. Running CI on every dev push ensures cross-platform and feature-flag\nissues are caught within minutes of landing.\n\nsecurity-audit.yml also now triggers on dev pushes (not just main) when\nCargo.toml, Cargo.lock, deny.toml, or .cargo/audit.toml change. This\nmeans transitive security advisories are caught as soon as a dep bump\nlands on dev, not only after it reaches main.\n\nBoth workflows use concurrency cancel-in-progress for non-main branches\nto avoid wasted CI cycles when multiple commits land in quick succession.\nMain always runs to completion to preserve the full audit trail per\ncommit.\n\n## 2. Manual workflow dispatch\n\nAdded workflow_dispatch to ci.yml and benchmarks.yml. The\nsecurity-audit.yml workflow already had it. This allows maintainers to\nmanually retrigger a workflow from the Actions tab without needing to\npush a dummy commit, which was awkward during the v0.7.0 release when\nwe needed to re-run after the token was updated.\n\n## 3. Token health check workflow\n\nNew .github/workflows/token-health.yml that runs weekly (Monday 09:15\nUTC, staggered after Security Audit) and verifies the\nCARGO_REGISTRY_TOKEN secret still authenticates against\nhttps://crates.io/api/v1/me. On failure, opens (or updates) a tracking\nissue labelled `security` with rotation instructions.\n\nThis directly prevents the exact failure mode hit during the v0.7.0\nrelease: the token had silently expired between v0.6.0 and v0.7.0 and\nthe release workflow only discovered this when it tried to upload\ntds-protocol and was rejected with HTTP 403. A weekly health check\nwould have surfaced the problem up to 7 days earlier, giving plenty\nof time to rotate before a real release attempt.\n\nThe check uses curl against the /api/v1/me endpoint (not\n`cargo publish --dry-run`, which does not actually authenticate\nunless it reaches the upload step). The call is cheap, non-destructive,\nand unambiguous: 200 means the token works, anything else means it\ndoesn't.\n\nRefs #63 (security audit reliability)\nRefs #64 (v0.7.0 release pain)\n\n* feat(tooling): add release-status, release-preflight, and xtask release-notes\n\nThree new release-observability tools that directly address pain points\nfrom the 0.7.0 release cycle:\n\n## just release-status\n\nDashboard recipe that reports everything you need to know before cutting\na release, in one command:\n- Current branch and workspace version\n- Last tag, date, days elapsed\n- dev vs main divergence (detects release candidates AND drift)\n- Latest run status for CI, Security Audit, Benchmarks, and the new\n  Token Health Check workflow\n- Open dependabot PRs with per-PR failing-check counts\n- Open contributor PRs (uses author.is_bot filter, not login name, so\n  \"app/dependabot\" is correctly excluded)\n- Open issue count\n- Local working-copy cleanliness and unpushed commits\n\nThis answers the question \"should I release?\" at a glance. Before 0.7.0\nwe walked into the release with 42 accumulated commits and no visibility\ninto the size of the backlog.\n\n## just release-preflight\n\nSequential gate check that runs every Cardinal Rule from RELEASING.md:\nworking-copy clean, version refs consistent, cargo audit clean, cargo\ndeny clean, no WIP markers, metadata valid, URLs valid, tier-0 package\ndry-run succeeds. Captures the full verification checklist that we\npreviously had to remember to run manually.\n\nIntegrates with the upcoming scripts/check-doc-consistency.sh linter\n(conditional — only runs if the script is present and executable).\n\n## cargo xtask release-notes\n\nGenerates a CHANGELOG draft from git log. Reads conventional commits\nbetween the last v*.*.* tag and HEAD, parses headers of the form\n\"type(scope)?!?: subject\", groups by bucket (feat → Added, fix → Fixed,\netc.), detects breaking changes via \"!\" suffix or \"BREAKING CHANGE\"\nfooter, and emits markdown suitable for pasting into CHANGELOG.md.\n\nNon-conforming commits are placed under an \"Other\" bucket with a\nreview prompt rather than being dropped silently. Merge commits are\nskipped.\n\nTested against the v0.6.0..v0.7.0 range — correctly identifies both\nbreaking changes (d4ef523 \"remove deprecated items before 1.0\" and\n7722158 \"add #[non_exhaustive] to 33 public enums\"), categorizes ~42\ncommits across Added / Fixed / Changed / Documentation / CI / Chores\nsections, and produces a draft that's ~80% ready to commit.\n\nUses the system 'date' command for \"today's date\" to avoid adding\nchrono as a new xtask dependency.\n\n* docs(community): add issue/PR templates, CoC, CODEOWNERS, MAINTAINERS.md\n\nRaise the contributor-facing surface area of the project. For a repo\nwith 13 stars and three real contributors (@VincentMeilinger, @tracker1,\n@c5soft) showing up organically, having none of these files was a real\nfriction point.\n\n## Issue templates (.github/ISSUE_TEMPLATE/)\n\n- config.yml: disables blank issues, adds contact_links pointing at\n  Security Advisories (private security reports), Discussions\n  (conversational questions), docs.rs (API reference), and the\n  Tiberius migration guide.\n- bug_report.yml: structured YAML form with required fields for driver\n  version, feature flags (multi-select of every feature in the workspace),\n  Rust version, OS, SQL Server version, TDS version, logs, and repro\n  code. Matches the kind of detail we actually need to debug TDS-level\n  issues.\n- feature_request.yml: use case first, API proposal second, alternatives\n  considered, area of codebase (multi-select of workspace crates),\n  breaking-change assessment.\n- question.yml: routes to docs.rs, connection strings, migration guide,\n  Discussions first; falls back to a minimal structured form.\n\nAll templates use YAML form format (not markdown) so required fields\nare actually enforced and the resulting issues are well-structured.\n\n## Pull request template (.github/pull_request_template.md)\n\nSections:\n- Summary and linked issues (with \"Closes #\" pattern)\n- Type of change checklist\n- Test plan (fmt/clippy/test commands + integration test hook)\n- Documentation updates checklist (rustdoc, CHANGELOG, README, docs/, ARCHITECTURE)\n- **Breaking changes section with explicit MSRV note**:\n  \"MSRV bumps are NOT a breaking change per STABILITY.md § MSRV\n  Increase Policy\". This prevents the exact confusion that caused\n  the CONTRIBUTING.md ↔ STABILITY.md contradiction we fixed during\n  the 0.7.0 cycle.\n- Security considerations (for auth/TLS/SQL-gen PRs)\n- Pre-1.0 policy reference link\n\n## CODE_OF_CONDUCT.md\n\nCONTRIBUTING.md already referenced \"the Rust Code of Conduct\" by URL\nbut the repo had no CoC file. Added the full Rust CoC text with\nproject-specific contact info pointing at the private Security\nAdvisory channel for CoC violations.\n\n## .github/CODEOWNERS\n\nAuto-requests review from the primary maintainer for PRs touching\nspecific areas. Today this is all @jkindrix, but the file is\nstructured so that adding future maintainers (as described in\nMAINTAINERS.md) is a one-line change per area. Ownership is granular:\nprotocol layer, auth, TLS, pool, client, types, derive, testing,\nxtask, workflows, release policy docs, security policy docs, and\ndependency/supply-chain files each get their own entry.\n\n## MAINTAINERS.md\n\nDocuments:\n- Who the current maintainers are\n- What maintainers are responsible for (review, triage, releases,\n  stewardship, continuity)\n- How to contact maintainers for different purposes (issues,\n  discussions, security reports, CoC reports)\n- How to become a maintainer (criteria, trial process)\n- Current decision-making model with a pointer to ARCHITECTURE.md ADRs\n  for architectural decisions\n- An empty Emeritus section for eventual use\n\nIncludes language around the \"correctness-first, quality-over-speed\"\nphilosophy so new maintainers know what they're signing up for.\n\n* fix(testing): fix mock server TLS close_notify (closes #70) + add doc linter\n\nTwo related fixes that both address pre-existing latent bugs surfaced\nduring the v0.7.0 release cycle.\n\n## Mock server TLS close_notify (closes #70)\n\n`handle_connection` now explicitly calls `tls_stream.shutdown().await`\nbefore the stream goes out of scope. Without this, dropping the\n`TlsStream` closes the underlying TCP half abruptly, and rustls on\nstricter platforms (macOS, Windows) reports\n\"peer closed connection without sending TLS close_notify\" on whatever\nread was in flight, even if the server had already written the\nresponse. Linux's rustls build tolerated this silently, which is why\nthe bug was latent until the full CI matrix ran on the v0.7.0\nrelease PR.\n\nThe shutdown call is wrapped in `let _ =` because the session is\nalready over at that point — we only care about best-effort clean\nclose. A real driver would log the error; the mock server is\ntest-only infrastructure.\n\nRemoved the `#[cfg(target_os = \"linux\")]` gate on\n`test_mock_server_tls_full_connection` — it now runs on all three\nplatforms in CI. Local `cargo test -p mssql-testing --test mock_fidelity`\npasses with 13 tests (1 ignored as before).\n\n## Documentation consistency linter\n\nNew `scripts/check-doc-consistency.sh` validates invariants across the\nproject's documentation and config files. This codifies a class of bug\nthat produced the CONTRIBUTING.md ↔ STABILITY.md MSRV-policy\ncontradiction discovered during the 0.7.0 release.\n\nChecks (22 total on the current tree):\n\n- **MSRV consistency**: rust-toolchain.toml, xtask/Cargo.toml,\n  README.md badge, CLAUDE.md, ARCHITECTURE.md, STABILITY.md,\n  CONTRIBUTING.md prerequisites table, RELEASING.md header,\n  Justfile variable — all must reference the same MSRV value\n  that workspace Cargo.toml declares.\n- **CHANGELOG ↔ workspace version**: latest non-[Unreleased] entry\n  must match workspace.package.version.\n- **MSRV policy agreement**: STABILITY.md must explicitly state\n  that MSRV bumps are non-breaking, AND CONTRIBUTING.md must NOT\n  list 'Increasing MSRV' under 'Definitely Breaking'. This prevents\n  the specific contradiction that was discovered post-v0.6.0.\n- **Workspace crate version inheritance**: every crate in crates/\n  must either inherit workspace version or explicitly pin the same\n  workspace version (no silent drift).\n- **deny.toml ↔ .cargo/audit.toml sync**: advisory ignore lists in\n  the two files must be identical. cargo-deny reads deny.toml,\n  cargo-audit reads audit.toml, and they must stay in sync or\n  we end up with different results in CI vs locally.\n\nThe linter is pure bash, uses only standard tools (grep, awk, sed,\ncomm), and runs in <100ms. No new dependencies.\n\nAdded `just doc-consistency` recipe that invokes the script with\na graceful no-op if the script is missing, and wired it into\n`just release-check` as an additional gate.\n\nOutput supports `--verbose` for full per-check reporting and is\ncolor-coded via ANSI escapes (suppressed when NO_COLOR is set or\nnot running on a TTY).\n\n* docs: update RELEASING, CONTRIBUTING, README, CLAUDE; add DEPENDENCY_POLICY\n\nComprehensive documentation refresh that captures everything learned\nduring the v0.7.0 release cycle and the post-release hygiene sprint.\n\n## RELEASING.md\n\nAdded a new \"Lesson 12: The v0.7.0 Incidents\" section documenting\nthree distinct problems that surfaced during the 0.7.0 release —\nCARGO_REGISTRY_TOKEN expiry discovered at publish time, pre-existing\nlatent bugs surfaced at release PR time due to dev-branch CI gap,\nand the CONTRIBUTING.md ↔ STABILITY.md MSRV policy contradiction.\n\nEach incident is written up with \"What went wrong\", \"Result\", and\n\"Solution\" subsections naming the concrete mitigations we put in\nplace in this release hygiene sprint.\n\nAdded a new \"Token Health\" top-level section explaining:\n- How the weekly token-health.yml workflow works\n- Step-by-step manual rotation procedure\n- What to do if a release is in flight when the token fails\n  (the exact recovery path used during v0.7.0)\n- Why the worst-case failure mode is \"zero crates published, rerun\n  after rotation\" rather than a partial publish\n\n## CONTRIBUTING.md\n\n- Added \"First Contribution (Quick Path)\" section with 7-step\n  recipe from clone to green CI\n- Added \"When Your PR Needs Review\" section explaining CODEOWNERS\n  auto-routing, expected response times, the draft-PR-for-big-changes\n  pattern, and why large feature PRs take longer\n- Updated Pull Request Process to reference the new PR template\n  and require `cargo test --all-features`, `cargo clippy --all-features`,\n  and `just doc-consistency` as mandatory pre-submit gates\n- Extended the \"Build Automation\" section with the new xtask\n  subcommand (`release-notes`) and a dedicated \"Release-Adjacent\n  Just Recipes\" table covering release-status, release-preflight,\n  release-check, doc-consistency, ci-status-all, and tag\n- Updated Code of Conduct section to link to the new\n  CODE_OF_CONDUCT.md file (previously the reference was to a URL\n  with no local file)\n\n## README.md\n\n- Rewrote the Contributing section with bullet pointers to:\n  first-contribution path, issue templates, PR template, ADR process,\n  CoC, and MAINTAINERS.md\n- Added a new Community section with links to Discussions, Issues,\n  and the private Security Advisory channel\n\n## CLAUDE.md\n\nTwo new sections:\n\n1. **Process and Governance** — enumerates every contributor-facing\n   document (README, CONTRIBUTING, CODE_OF_CONDUCT, MAINTAINERS,\n   CODEOWNERS, issue/PR templates), every release and policy document\n   (RELEASING, STABILITY, SECURITY, VERSION_REFS, DEPENDENCY_POLICY),\n   and the full list of release observability tooling (just recipes\n   and xtask commands) added post-v0.7.0. Also lists all CI/CD\n   workflows with their triggers.\n\n2. **Conventions for AI Assistants** — explicit notes that MSRV\n   bumps are NOT breaking changes (STABILITY.md is authoritative,\n   the linter catches contradictions), that fixing beats ignoring\n   for security advisories (with the v0.7.0 MSRV-bump precedent),\n   that the release recipes exist to prevent the exact mistakes\n   that caused past incidents, that the Cardinal Rules in RELEASING.md\n   are non-negotiable, that dev-branch CI now exists so push\n   confidently, and that future AI sessions must update CLAUDE.md\n   when they add new infrastructure.\n\nUpdated the Development Tooling section to list all actually-required\ntools (just, gh, cargo-deny, cargo-hack, cargo-nextest, cargo-audit,\ncargo-machete, cargo-semver-checks) and point at `just setup-tools`.\n\nRemoved the now-obsolete cargo-hakari section (the workspace-hack\ncrate was removed in v0.7.0 — see commit ce2852a).\n\nUpdated the Document References section with the full list of\nprimary references including the new DEPENDENCY_POLICY.md.\n\n## docs/DEPENDENCY_POLICY.md (new)\n\nCaptures tribal knowledge about dependency management decisions into\ndiscoverable documentation. Sections:\n\n- **Philosophy** — 5 principles (minimize surface area, correctness\n  over convenience, modern Rust not cutting-edge, pure Rust where\n  it matters, minimize feature flags)\n- **Adding a new dependency** — 8-criterion checklist\n- **Taking a dependency upgrade** — patch/minor/major handling,\n  when to defer, bundled bumps (OpenTelemetry, Tokio)\n- **Handling security advisories** — three cases with explicit\n  guidance and the v0.7.0 MSRV-bump precedent documented as\n  Case B. Explains the MSRV-vs-ignore tradeoff we actually made\n  for the time crate.\n- **deny.toml and .cargo/audit.toml sync** — how the linter\n  enforces this\n- **Removing a dependency** — 5-step cleanup workflow\n- **New license requirements** — how we evaluate unfamiliar\n  SPDX license strings, with the MIT-0 precedent from v0.7.0\n- **Maintenance schedule** — weekly dependabot, weekly security\n  audit, weekly token health, per-release cargo update cadence,\n  quarterly major-version review\n\nReferenced from CLAUDE.md, CONTRIBUTING.md, and RELEASING.md.\n\n* fix(testing): re-gate mock TLS full connection test to Linux only\n\nThe explicit tls_stream.shutdown().await added in cee9751 did not fix\nthe macOS/Windows failure. CI confirmed: same \"peer closed connection\nwithout sending TLS close_notify\" error on both platforms.\n\nRoot cause analysis (updated in #70): the error occurs during the\nLoginAck header read, meaning the client sees EOF BEFORE the server's\nresponse arrives. The shutdown() fix only addresses connection-close\ntime; the actual issue is a timing/buffering race in the mock server's\nTLS path where the TCP buffer flush doesn't reach the client before\nthe read polls on macOS/Windows.\n\nThe production driver is NOT affected — it uses a different connection\narchitecture (Connection<T> with framed I/O) that doesn't have this\ntiming dependency.\n\nRe-gating with comprehensive investigation notes in the test doc\ncomment so the next person who picks this up (#70) knows exactly what\nwas tried and why it didn't work.\n\nThe shutdown() call is kept in the mock server — it's correct for\nclean TLS close even though it doesn't fix this particular race.\n\n* fix(deps): resolve security audit + bump dev dependencies and CI actions\n\nAdvisory handling:\n- Add RUSTSEC-2026-0097 (rand 0.8.5 unsoundness) ignore — Case C per\n  DEPENDENCY_POLICY: no stable fix available, log feature not enabled,\n  unsoundness conditions unmet. Blocked on rsa 0.10 stable (#21).\n- Remove stale RUSTSEC-2026-0066 ignore — resolved by testcontainers\n  0.27 (pulls astral-tokio-tar 0.6.0 which includes the fix).\n- Update RUSTSEC-2025-0134 ignore reason — rustls-pemfile is a direct\n  dep of mssql-auth, not just transitive via bollard.\n\nDependency bumps (all dev-only or patch-level):\n- rustls 0.23.37 → 0.23.38 (patch)\n- tokio 1.51.0 → 1.51.1 (patch)\n- testcontainers 0.25.2 → 0.27.2 (dev-only, drops rustls-pemfile\n  from bollard path, fixes astral-tokio-tar advisory)\n- criterion 0.7.0 → 0.8.2 (dev-only benchmarks)\n- bollard 0.19.4 → 0.20.2 (transitive via testcontainers)\n\nCI action bumps:\n- codecov/codecov-action v5 → v6 (Node 24 runtime)\n- softprops/action-gh-release v2 → v3 (Node 24 runtime)\n- actions/github-script v8 → v9 (both security-audit.yml and\n  token-health.yml, for consistency)\n\n* fix(testing): use multi-thread runtime for TLS full connection test (#70)\n\nThe test_mock_server_tls_full_connection test was gated to Linux only\ndue to a timing race: the mock server's raw write_all() + flush() on\nthe TLS-over-PreLogin stream didn't reliably deliver data before the\nsingle-thread runtime's cooperative scheduler yielded on macOS/Windows.\n\nFix: Use #[tokio::test(flavor = \"multi_thread\", worker_threads = 2)]\nwhich matches production usage and eliminates the scheduling dependency\nbetween server and client tasks. Remove the #[cfg(target_os = \"linux\")]\ngate so the test runs on all CI platforms.\n\nCloses #70\n\n* fix(testing): fix mock TLS cross-platform race in TlsPreloginWrapper (#70)\n\nRoot cause: the client completes the TLS handshake and sends Login7 as\nraw TLS (ApplicationData 0x17) before the server-side TlsPreloginWrapper\nhas switched to pass-through mode. On macOS/Windows, TCP coalesces these\nbytes into one read, so the server's wrapper (still in handshake mode)\ninterprets the raw TLS record header as a TDS PreLogin header and fails\nwith InvalidContentType.\n\nFix: auto-detect non-PreLogin bytes during handshake mode. When the\nwrapper reads a header byte that isn't 0x12 (PreLogin), it means the\npeer has already switched to raw TLS. The wrapper auto-transitions to\npass-through mode and feeds the already-read header bytes back to the\ncaller via a prefix buffer, so rustls can process them as the TLS record\nthey actually are.\n\nThe production client-side wrapper (mssql-tls) is NOT affected because\nthe client always finishes the handshake first — the server never sends\nraw TLS before the client's wrapper has switched.\n\nDiagnosed with targeted experiments on macOS and Windows machines that\nrevealed the server never reached send_login_response — it failed during\nread_packet for Login7 because the TLS stream received corrupted data\nfrom the wrapper.\n\nRemove the #[cfg(target_os = \"linux\")] gate so the test runs on all CI\nplatforms.\n\nCloses #70\n\n* feat(auth): bump Azure SDK to azure_core/identity 0.34, keyvault_keys 0.13\n\nUnified bump of all three coupled Azure SDK crates:\n- azure_core 0.30 → 0.34\n- azure_identity 0.30 → 0.34\n- azure_security_keyvault_keys 0.9 → 0.13\n\nAPI adaptations in mssql-auth:\n- cert_auth.rs: ClientCertificateCredential::new() now takes SecretBytes\n  instead of Secret for the certificate parameter\n- azure_keyvault.rs: key_version moved from options structs to a required\n  method parameter on unwrap_key(), sign(), verify(). CMK paths must now\n  include the key version (which Always Encrypted paths always do).\n  RequestContent<T> is now used for request bodies instead of .try_into()\n  conversion.\n- azure_identity_auth.rs: No changes needed — TokenCredential trait,\n  ManagedIdentityCredential, and ClientSecretCredential APIs unchanged.\n\nSupersedes dependabot PRs #76, #79, #81 which could not be merged\nindividually because the three crates share trait types.\n\n* refactor(client): extract validation to shared module (stored proc prep)\n\nMove validate_identifier() and validate_qualified_identifier() from\nclient/mod.rs and bulk.rs to a shared crate::validation module. Both\ncall sites now use the shared implementation, eliminating duplication.\n\nThis prepares for stored procedure support, which needs\nvalidate_qualified_identifier() for procedure name validation (same\nsecurity requirement as savepoint and bulk insert identifier validation).\n\n* feat(protocol): add col_type to ReturnValue and ProcedureResult type\n\nAdd `col_type: u8` field to ReturnValue struct so downstream code can\nconstruct a ColumnData for parse_column_value() without re-parsing.\nAdd `#[non_exhaustive]` to ReturnValue since it's a protocol-layer struct.\n\nAdd ProcedureResult type in stream.rs with return_value, rows_affected,\noutput_params, and result_sets fields. Includes accessor methods:\nget_output() (case-insensitive, @-prefix tolerant), get_return_value(),\nfirst_result_set(), has_result_sets().\n\nPart of stored procedure support (Step 4b).\n\n* feat(client): implement stored procedure support\n\nAdd complete stored procedure API with two-tier design:\n\n1. Simple convenience method for input-only calls:\n   client.call_procedure(\"dbo.MyProc\", &[&1i32, &\"hello\"]).await?\n\n2. Full builder for named/output parameters:\n   client.procedure(\"dbo.CalculateSum\")?\n       .input(\"@a\", &10i32)\n       .output_int(\"@result\")\n       .execute().await?\n\nImplementation details:\n- ProcedureBuilder with typed output methods (output_int, output_bigint,\n  output_nvarchar, output_bit, output_float, output_decimal, output_raw)\n- read_procedure_result() parser handles all TDS tokens: ColMetaData,\n  Row, NbcRow, DoneInProc, ReturnValue, ReturnStatus, DoneProc\n- ReturnValue decoding reuses parse_column_value() via ColumnData bridge\n- Extract convert_single_param() from convert_params() to share\n  SqlValue->RpcParam logic between query params and procedure builder\n- Methods on impl<S: ConnectionState> — works in both Ready and\n  InTransaction states with zero duplication\n- All procedure names validated via validate_qualified_identifier()\n- Send+Sync compile-time assertions for ProcedureResult and ProcedureBuilder\n- Visibility: send_rpc() and read_procedure_result() promoted to pub(crate)\n\nPart of stored procedure support (Steps 4c-4f).\n\n* docs(stored-procs): add tests, docs, and CHANGELOG entry\n\n- Integration tests: 11 tests covering simple calls, input params,\n  return values, multiple result sets, rows_affected, OUTPUT params\n  (int, nvarchar), builder with result sets and output, transactions,\n  error handling, and schema-qualified names\n- Unit tests: ProcedureResult defaults, get_output case-insensitive\n  and @-prefix stripping, result sets with return value\n- ARCHITECTURE.md: add section 4.7 covering RPC flow, token handling,\n  ReturnValue decoding, and security\n- docs/STORED_PROCEDURES.md: user guide with quick start, API reference,\n  transaction support, output types, security, and error handling\n- CHANGELOG.md: add stored procedure feature under [Unreleased], credit\n  @c5soft for PR #71's influence on API design\n\nPart of stored procedure support (Step 4g).\n\n* fix(docs): resolve broken ProcedureBuilder rustdoc link\n\nUse fully qualified path `crate::procedure::ProcedureBuilder` in the\ndoc comment for `Client::procedure()` so rustdoc can resolve it across\nmodule boundaries.\n\n* fix(client): add Clone derive to ProcedureResult and ResultSet\n\nProcedureResult was missing Clone, which the plan specified. ResultSet\nalso lacked Clone despite all its fields (Vec<Column>, VecDeque<Row>)\nbeing Clone. Both now derive Clone for consistency with OutputParam and\nExecuteResult.\n\n* feat(client): add SQL Browser instance resolution (#66)\n\nAdd automatic TCP port resolution for named SQL Server instances via\nthe SQL Server Browser service (SSRP protocol, MC-SQLR spec).\n\nWhen connecting with a named instance (e.g., Server=localhost\\SQLEXPRESS),\nthe driver now queries the Browser service on UDP 1434 to discover the\nTCP port before establishing the TCP connection. This is transparent to\nthe user — no API changes needed.\n\nNew module: crate::browser — implements CLNT_UCAST_INST request and\nSVR_RESP response parsing per the MC-SQLR specification. Handles:\n- \".\" as localhost (common for .\\SQLEXPRESS)\n- Timeout when Browser service is not running\n- Missing TCP port (instance may only support Named Pipes)\n- Malformed responses\n\nNew error variant: Error::BrowserResolution with instance name and\nreason for clear diagnostics.\n\nIntegration point: Client::try_connect() resolves the instance port\nbefore TCP connect when config.instance is Some.\n\nRequested by @tracker1 in #66.\n\nCloses #66\n\n* chore: release v0.8.0\n\nVersion bump 0.7.0 → 0.8.0 across workspace and all 9 crates.\n\nHighlights:\n- Stored procedure support (call_procedure + procedure builder)\n- SQL Browser instance resolution (.\\SQLEXPRESS)\n- Pool test_on_checkin health check\n- Azure SDK 0.34 bump\n- Mock TLS cross-platform fix\n- Advisory + dependency maintenance\n\nUpdated version references in: RELEASING.md, README.md,\nARCHITECTURE.md, SECURITY.md, STABILITY.md, CLAUDE.md,\ndocs/BENCHMARKS.md, docs/VERSION_REFS.md, CHANGELOG.md.\n\nFixed test_on_acquire → test_on_checkout in pool docs (3 files).\nAdded stored procedure APIs to STABILITY.md stable surface.",
          "timestamp": "2026-04-13T18:48:13-05:00",
          "tree_id": "76b8aa02cdef80aa435f639839b0b53b152fc1a5",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/4ecfdf49a8cb25a6be05f0062146290301107572"
        },
        "date": 1776124858319,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 407,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 402,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 487,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 727,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32_from_int",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64_from_bigint",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/string_from_string",
            "value": 22,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_some",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/option_i32_none",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64_from_double",
            "value": 9,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool_from_bool",
            "value": 12,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_small",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_medium",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/clone_large",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "arc_bytes/slice_medium",
            "value": 6,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/minimal",
            "value": 90,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 113,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_bigint",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/null_check_iter",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/short",
            "value": 78,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 571,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3785,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 281,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/short",
            "value": 41,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/medium",
            "value": 139,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 649,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i32",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/i64",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/f64",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/bool",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/String",
            "value": 38,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/str",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_Some",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "to_sql/Option_i32_None",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i32",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/i64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/f64",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/bool",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/String",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_Some",
            "value": 7,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "from_sql/Option_i32_None",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_int",
            "value": 4,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_string",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/create_null",
            "value": 3,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_value/is_null_check",
            "value": 0,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_encode",
            "value": 47,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "packet_header_decode",
            "value": 11,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_encode",
            "value": 147,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "prelogin_decode",
            "value": 66,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/simple",
            "value": 101,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 949,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2567,
            "range": "± 3",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}