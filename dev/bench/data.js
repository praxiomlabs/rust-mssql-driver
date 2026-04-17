window.BENCHMARK_DATA = {
  "lastUpdate": 1776465972480,
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
          "id": "7760f0c6892cb451d67eb2b3a00c3dde7179815d",
          "message": "Release v0.9.0 (#84)\n\n* ci: run on dev branch, add workflow_dispatch, add token health check\n\nPost-v0.7.0 release hygiene improvements addressing three pain points\nsurfaced during the 0.7.0 release cycle:\n\n## 1. CI on dev branch\n\nci.yml and benchmarks.yml now trigger on pushes to dev, not just main.\nThe 42-commit backlog that shipped in v0.7.0 had zero CI coverage between\nthe last main-branch CI run (2026-01-13) and the release PR (#69). That's\nhow the mock_server TLS test failure on macOS/Windows and the xtask\n--no-dev-deps bug were both latent for months before surfacing at release\ntime. Running CI on every dev push ensures cross-platform and feature-flag\nissues are caught within minutes of landing.\n\nsecurity-audit.yml also now triggers on dev pushes (not just main) when\nCargo.toml, Cargo.lock, deny.toml, or .cargo/audit.toml change. This\nmeans transitive security advisories are caught as soon as a dep bump\nlands on dev, not only after it reaches main.\n\nBoth workflows use concurrency cancel-in-progress for non-main branches\nto avoid wasted CI cycles when multiple commits land in quick succession.\nMain always runs to completion to preserve the full audit trail per\ncommit.\n\n## 2. Manual workflow dispatch\n\nAdded workflow_dispatch to ci.yml and benchmarks.yml. The\nsecurity-audit.yml workflow already had it. This allows maintainers to\nmanually retrigger a workflow from the Actions tab without needing to\npush a dummy commit, which was awkward during the v0.7.0 release when\nwe needed to re-run after the token was updated.\n\n## 3. Token health check workflow\n\nNew .github/workflows/token-health.yml that runs weekly (Monday 09:15\nUTC, staggered after Security Audit) and verifies the\nCARGO_REGISTRY_TOKEN secret still authenticates against\nhttps://crates.io/api/v1/me. On failure, opens (or updates) a tracking\nissue labelled `security` with rotation instructions.\n\nThis directly prevents the exact failure mode hit during the v0.7.0\nrelease: the token had silently expired between v0.6.0 and v0.7.0 and\nthe release workflow only discovered this when it tried to upload\ntds-protocol and was rejected with HTTP 403. A weekly health check\nwould have surfaced the problem up to 7 days earlier, giving plenty\nof time to rotate before a real release attempt.\n\nThe check uses curl against the /api/v1/me endpoint (not\n`cargo publish --dry-run`, which does not actually authenticate\nunless it reaches the upload step). The call is cheap, non-destructive,\nand unambiguous: 200 means the token works, anything else means it\ndoesn't.\n\nRefs #63 (security audit reliability)\nRefs #64 (v0.7.0 release pain)\n\n* feat(tooling): add release-status, release-preflight, and xtask release-notes\n\nThree new release-observability tools that directly address pain points\nfrom the 0.7.0 release cycle:\n\n## just release-status\n\nDashboard recipe that reports everything you need to know before cutting\na release, in one command:\n- Current branch and workspace version\n- Last tag, date, days elapsed\n- dev vs main divergence (detects release candidates AND drift)\n- Latest run status for CI, Security Audit, Benchmarks, and the new\n  Token Health Check workflow\n- Open dependabot PRs with per-PR failing-check counts\n- Open contributor PRs (uses author.is_bot filter, not login name, so\n  \"app/dependabot\" is correctly excluded)\n- Open issue count\n- Local working-copy cleanliness and unpushed commits\n\nThis answers the question \"should I release?\" at a glance. Before 0.7.0\nwe walked into the release with 42 accumulated commits and no visibility\ninto the size of the backlog.\n\n## just release-preflight\n\nSequential gate check that runs every Cardinal Rule from RELEASING.md:\nworking-copy clean, version refs consistent, cargo audit clean, cargo\ndeny clean, no WIP markers, metadata valid, URLs valid, tier-0 package\ndry-run succeeds. Captures the full verification checklist that we\npreviously had to remember to run manually.\n\nIntegrates with the upcoming scripts/check-doc-consistency.sh linter\n(conditional — only runs if the script is present and executable).\n\n## cargo xtask release-notes\n\nGenerates a CHANGELOG draft from git log. Reads conventional commits\nbetween the last v*.*.* tag and HEAD, parses headers of the form\n\"type(scope)?!?: subject\", groups by bucket (feat → Added, fix → Fixed,\netc.), detects breaking changes via \"!\" suffix or \"BREAKING CHANGE\"\nfooter, and emits markdown suitable for pasting into CHANGELOG.md.\n\nNon-conforming commits are placed under an \"Other\" bucket with a\nreview prompt rather than being dropped silently. Merge commits are\nskipped.\n\nTested against the v0.6.0..v0.7.0 range — correctly identifies both\nbreaking changes (d4ef523 \"remove deprecated items before 1.0\" and\n7722158 \"add #[non_exhaustive] to 33 public enums\"), categorizes ~42\ncommits across Added / Fixed / Changed / Documentation / CI / Chores\nsections, and produces a draft that's ~80% ready to commit.\n\nUses the system 'date' command for \"today's date\" to avoid adding\nchrono as a new xtask dependency.\n\n* docs(community): add issue/PR templates, CoC, CODEOWNERS, MAINTAINERS.md\n\nRaise the contributor-facing surface area of the project. For a repo\nwith 13 stars and three real contributors (@VincentMeilinger, @tracker1,\n@c5soft) showing up organically, having none of these files was a real\nfriction point.\n\n## Issue templates (.github/ISSUE_TEMPLATE/)\n\n- config.yml: disables blank issues, adds contact_links pointing at\n  Security Advisories (private security reports), Discussions\n  (conversational questions), docs.rs (API reference), and the\n  Tiberius migration guide.\n- bug_report.yml: structured YAML form with required fields for driver\n  version, feature flags (multi-select of every feature in the workspace),\n  Rust version, OS, SQL Server version, TDS version, logs, and repro\n  code. Matches the kind of detail we actually need to debug TDS-level\n  issues.\n- feature_request.yml: use case first, API proposal second, alternatives\n  considered, area of codebase (multi-select of workspace crates),\n  breaking-change assessment.\n- question.yml: routes to docs.rs, connection strings, migration guide,\n  Discussions first; falls back to a minimal structured form.\n\nAll templates use YAML form format (not markdown) so required fields\nare actually enforced and the resulting issues are well-structured.\n\n## Pull request template (.github/pull_request_template.md)\n\nSections:\n- Summary and linked issues (with \"Closes #\" pattern)\n- Type of change checklist\n- Test plan (fmt/clippy/test commands + integration test hook)\n- Documentation updates checklist (rustdoc, CHANGELOG, README, docs/, ARCHITECTURE)\n- **Breaking changes section with explicit MSRV note**:\n  \"MSRV bumps are NOT a breaking change per STABILITY.md § MSRV\n  Increase Policy\". This prevents the exact confusion that caused\n  the CONTRIBUTING.md ↔ STABILITY.md contradiction we fixed during\n  the 0.7.0 cycle.\n- Security considerations (for auth/TLS/SQL-gen PRs)\n- Pre-1.0 policy reference link\n\n## CODE_OF_CONDUCT.md\n\nCONTRIBUTING.md already referenced \"the Rust Code of Conduct\" by URL\nbut the repo had no CoC file. Added the full Rust CoC text with\nproject-specific contact info pointing at the private Security\nAdvisory channel for CoC violations.\n\n## .github/CODEOWNERS\n\nAuto-requests review from the primary maintainer for PRs touching\nspecific areas. Today this is all @jkindrix, but the file is\nstructured so that adding future maintainers (as described in\nMAINTAINERS.md) is a one-line change per area. Ownership is granular:\nprotocol layer, auth, TLS, pool, client, types, derive, testing,\nxtask, workflows, release policy docs, security policy docs, and\ndependency/supply-chain files each get their own entry.\n\n## MAINTAINERS.md\n\nDocuments:\n- Who the current maintainers are\n- What maintainers are responsible for (review, triage, releases,\n  stewardship, continuity)\n- How to contact maintainers for different purposes (issues,\n  discussions, security reports, CoC reports)\n- How to become a maintainer (criteria, trial process)\n- Current decision-making model with a pointer to ARCHITECTURE.md ADRs\n  for architectural decisions\n- An empty Emeritus section for eventual use\n\nIncludes language around the \"correctness-first, quality-over-speed\"\nphilosophy so new maintainers know what they're signing up for.\n\n* fix(testing): fix mock server TLS close_notify (closes #70) + add doc linter\n\nTwo related fixes that both address pre-existing latent bugs surfaced\nduring the v0.7.0 release cycle.\n\n## Mock server TLS close_notify (closes #70)\n\n`handle_connection` now explicitly calls `tls_stream.shutdown().await`\nbefore the stream goes out of scope. Without this, dropping the\n`TlsStream` closes the underlying TCP half abruptly, and rustls on\nstricter platforms (macOS, Windows) reports\n\"peer closed connection without sending TLS close_notify\" on whatever\nread was in flight, even if the server had already written the\nresponse. Linux's rustls build tolerated this silently, which is why\nthe bug was latent until the full CI matrix ran on the v0.7.0\nrelease PR.\n\nThe shutdown call is wrapped in `let _ =` because the session is\nalready over at that point — we only care about best-effort clean\nclose. A real driver would log the error; the mock server is\ntest-only infrastructure.\n\nRemoved the `#[cfg(target_os = \"linux\")]` gate on\n`test_mock_server_tls_full_connection` — it now runs on all three\nplatforms in CI. Local `cargo test -p mssql-testing --test mock_fidelity`\npasses with 13 tests (1 ignored as before).\n\n## Documentation consistency linter\n\nNew `scripts/check-doc-consistency.sh` validates invariants across the\nproject's documentation and config files. This codifies a class of bug\nthat produced the CONTRIBUTING.md ↔ STABILITY.md MSRV-policy\ncontradiction discovered during the 0.7.0 release.\n\nChecks (22 total on the current tree):\n\n- **MSRV consistency**: rust-toolchain.toml, xtask/Cargo.toml,\n  README.md badge, CLAUDE.md, ARCHITECTURE.md, STABILITY.md,\n  CONTRIBUTING.md prerequisites table, RELEASING.md header,\n  Justfile variable — all must reference the same MSRV value\n  that workspace Cargo.toml declares.\n- **CHANGELOG ↔ workspace version**: latest non-[Unreleased] entry\n  must match workspace.package.version.\n- **MSRV policy agreement**: STABILITY.md must explicitly state\n  that MSRV bumps are non-breaking, AND CONTRIBUTING.md must NOT\n  list 'Increasing MSRV' under 'Definitely Breaking'. This prevents\n  the specific contradiction that was discovered post-v0.6.0.\n- **Workspace crate version inheritance**: every crate in crates/\n  must either inherit workspace version or explicitly pin the same\n  workspace version (no silent drift).\n- **deny.toml ↔ .cargo/audit.toml sync**: advisory ignore lists in\n  the two files must be identical. cargo-deny reads deny.toml,\n  cargo-audit reads audit.toml, and they must stay in sync or\n  we end up with different results in CI vs locally.\n\nThe linter is pure bash, uses only standard tools (grep, awk, sed,\ncomm), and runs in <100ms. No new dependencies.\n\nAdded `just doc-consistency` recipe that invokes the script with\na graceful no-op if the script is missing, and wired it into\n`just release-check` as an additional gate.\n\nOutput supports `--verbose` for full per-check reporting and is\ncolor-coded via ANSI escapes (suppressed when NO_COLOR is set or\nnot running on a TTY).\n\n* docs: update RELEASING, CONTRIBUTING, README, CLAUDE; add DEPENDENCY_POLICY\n\nComprehensive documentation refresh that captures everything learned\nduring the v0.7.0 release cycle and the post-release hygiene sprint.\n\n## RELEASING.md\n\nAdded a new \"Lesson 12: The v0.7.0 Incidents\" section documenting\nthree distinct problems that surfaced during the 0.7.0 release —\nCARGO_REGISTRY_TOKEN expiry discovered at publish time, pre-existing\nlatent bugs surfaced at release PR time due to dev-branch CI gap,\nand the CONTRIBUTING.md ↔ STABILITY.md MSRV policy contradiction.\n\nEach incident is written up with \"What went wrong\", \"Result\", and\n\"Solution\" subsections naming the concrete mitigations we put in\nplace in this release hygiene sprint.\n\nAdded a new \"Token Health\" top-level section explaining:\n- How the weekly token-health.yml workflow works\n- Step-by-step manual rotation procedure\n- What to do if a release is in flight when the token fails\n  (the exact recovery path used during v0.7.0)\n- Why the worst-case failure mode is \"zero crates published, rerun\n  after rotation\" rather than a partial publish\n\n## CONTRIBUTING.md\n\n- Added \"First Contribution (Quick Path)\" section with 7-step\n  recipe from clone to green CI\n- Added \"When Your PR Needs Review\" section explaining CODEOWNERS\n  auto-routing, expected response times, the draft-PR-for-big-changes\n  pattern, and why large feature PRs take longer\n- Updated Pull Request Process to reference the new PR template\n  and require `cargo test --all-features`, `cargo clippy --all-features`,\n  and `just doc-consistency` as mandatory pre-submit gates\n- Extended the \"Build Automation\" section with the new xtask\n  subcommand (`release-notes`) and a dedicated \"Release-Adjacent\n  Just Recipes\" table covering release-status, release-preflight,\n  release-check, doc-consistency, ci-status-all, and tag\n- Updated Code of Conduct section to link to the new\n  CODE_OF_CONDUCT.md file (previously the reference was to a URL\n  with no local file)\n\n## README.md\n\n- Rewrote the Contributing section with bullet pointers to:\n  first-contribution path, issue templates, PR template, ADR process,\n  CoC, and MAINTAINERS.md\n- Added a new Community section with links to Discussions, Issues,\n  and the private Security Advisory channel\n\n## CLAUDE.md\n\nTwo new sections:\n\n1. **Process and Governance** — enumerates every contributor-facing\n   document (README, CONTRIBUTING, CODE_OF_CONDUCT, MAINTAINERS,\n   CODEOWNERS, issue/PR templates), every release and policy document\n   (RELEASING, STABILITY, SECURITY, VERSION_REFS, DEPENDENCY_POLICY),\n   and the full list of release observability tooling (just recipes\n   and xtask commands) added post-v0.7.0. Also lists all CI/CD\n   workflows with their triggers.\n\n2. **Conventions for AI Assistants** — explicit notes that MSRV\n   bumps are NOT breaking changes (STABILITY.md is authoritative,\n   the linter catches contradictions), that fixing beats ignoring\n   for security advisories (with the v0.7.0 MSRV-bump precedent),\n   that the release recipes exist to prevent the exact mistakes\n   that caused past incidents, that the Cardinal Rules in RELEASING.md\n   are non-negotiable, that dev-branch CI now exists so push\n   confidently, and that future AI sessions must update CLAUDE.md\n   when they add new infrastructure.\n\nUpdated the Development Tooling section to list all actually-required\ntools (just, gh, cargo-deny, cargo-hack, cargo-nextest, cargo-audit,\ncargo-machete, cargo-semver-checks) and point at `just setup-tools`.\n\nRemoved the now-obsolete cargo-hakari section (the workspace-hack\ncrate was removed in v0.7.0 — see commit ce2852a).\n\nUpdated the Document References section with the full list of\nprimary references including the new DEPENDENCY_POLICY.md.\n\n## docs/DEPENDENCY_POLICY.md (new)\n\nCaptures tribal knowledge about dependency management decisions into\ndiscoverable documentation. Sections:\n\n- **Philosophy** — 5 principles (minimize surface area, correctness\n  over convenience, modern Rust not cutting-edge, pure Rust where\n  it matters, minimize feature flags)\n- **Adding a new dependency** — 8-criterion checklist\n- **Taking a dependency upgrade** — patch/minor/major handling,\n  when to defer, bundled bumps (OpenTelemetry, Tokio)\n- **Handling security advisories** — three cases with explicit\n  guidance and the v0.7.0 MSRV-bump precedent documented as\n  Case B. Explains the MSRV-vs-ignore tradeoff we actually made\n  for the time crate.\n- **deny.toml and .cargo/audit.toml sync** — how the linter\n  enforces this\n- **Removing a dependency** — 5-step cleanup workflow\n- **New license requirements** — how we evaluate unfamiliar\n  SPDX license strings, with the MIT-0 precedent from v0.7.0\n- **Maintenance schedule** — weekly dependabot, weekly security\n  audit, weekly token health, per-release cargo update cadence,\n  quarterly major-version review\n\nReferenced from CLAUDE.md, CONTRIBUTING.md, and RELEASING.md.\n\n* fix(testing): re-gate mock TLS full connection test to Linux only\n\nThe explicit tls_stream.shutdown().await added in cee9751 did not fix\nthe macOS/Windows failure. CI confirmed: same \"peer closed connection\nwithout sending TLS close_notify\" error on both platforms.\n\nRoot cause analysis (updated in #70): the error occurs during the\nLoginAck header read, meaning the client sees EOF BEFORE the server's\nresponse arrives. The shutdown() fix only addresses connection-close\ntime; the actual issue is a timing/buffering race in the mock server's\nTLS path where the TCP buffer flush doesn't reach the client before\nthe read polls on macOS/Windows.\n\nThe production driver is NOT affected — it uses a different connection\narchitecture (Connection<T> with framed I/O) that doesn't have this\ntiming dependency.\n\nRe-gating with comprehensive investigation notes in the test doc\ncomment so the next person who picks this up (#70) knows exactly what\nwas tried and why it didn't work.\n\nThe shutdown() call is kept in the mock server — it's correct for\nclean TLS close even though it doesn't fix this particular race.\n\n* fix(deps): resolve security audit + bump dev dependencies and CI actions\n\nAdvisory handling:\n- Add RUSTSEC-2026-0097 (rand 0.8.5 unsoundness) ignore — Case C per\n  DEPENDENCY_POLICY: no stable fix available, log feature not enabled,\n  unsoundness conditions unmet. Blocked on rsa 0.10 stable (#21).\n- Remove stale RUSTSEC-2026-0066 ignore — resolved by testcontainers\n  0.27 (pulls astral-tokio-tar 0.6.0 which includes the fix).\n- Update RUSTSEC-2025-0134 ignore reason — rustls-pemfile is a direct\n  dep of mssql-auth, not just transitive via bollard.\n\nDependency bumps (all dev-only or patch-level):\n- rustls 0.23.37 → 0.23.38 (patch)\n- tokio 1.51.0 → 1.51.1 (patch)\n- testcontainers 0.25.2 → 0.27.2 (dev-only, drops rustls-pemfile\n  from bollard path, fixes astral-tokio-tar advisory)\n- criterion 0.7.0 → 0.8.2 (dev-only benchmarks)\n- bollard 0.19.4 → 0.20.2 (transitive via testcontainers)\n\nCI action bumps:\n- codecov/codecov-action v5 → v6 (Node 24 runtime)\n- softprops/action-gh-release v2 → v3 (Node 24 runtime)\n- actions/github-script v8 → v9 (both security-audit.yml and\n  token-health.yml, for consistency)\n\n* fix(testing): use multi-thread runtime for TLS full connection test (#70)\n\nThe test_mock_server_tls_full_connection test was gated to Linux only\ndue to a timing race: the mock server's raw write_all() + flush() on\nthe TLS-over-PreLogin stream didn't reliably deliver data before the\nsingle-thread runtime's cooperative scheduler yielded on macOS/Windows.\n\nFix: Use #[tokio::test(flavor = \"multi_thread\", worker_threads = 2)]\nwhich matches production usage and eliminates the scheduling dependency\nbetween server and client tasks. Remove the #[cfg(target_os = \"linux\")]\ngate so the test runs on all CI platforms.\n\nCloses #70\n\n* fix(testing): fix mock TLS cross-platform race in TlsPreloginWrapper (#70)\n\nRoot cause: the client completes the TLS handshake and sends Login7 as\nraw TLS (ApplicationData 0x17) before the server-side TlsPreloginWrapper\nhas switched to pass-through mode. On macOS/Windows, TCP coalesces these\nbytes into one read, so the server's wrapper (still in handshake mode)\ninterprets the raw TLS record header as a TDS PreLogin header and fails\nwith InvalidContentType.\n\nFix: auto-detect non-PreLogin bytes during handshake mode. When the\nwrapper reads a header byte that isn't 0x12 (PreLogin), it means the\npeer has already switched to raw TLS. The wrapper auto-transitions to\npass-through mode and feeds the already-read header bytes back to the\ncaller via a prefix buffer, so rustls can process them as the TLS record\nthey actually are.\n\nThe production client-side wrapper (mssql-tls) is NOT affected because\nthe client always finishes the handshake first — the server never sends\nraw TLS before the client's wrapper has switched.\n\nDiagnosed with targeted experiments on macOS and Windows machines that\nrevealed the server never reached send_login_response — it failed during\nread_packet for Login7 because the TLS stream received corrupted data\nfrom the wrapper.\n\nRemove the #[cfg(target_os = \"linux\")] gate so the test runs on all CI\nplatforms.\n\nCloses #70\n\n* feat(auth): bump Azure SDK to azure_core/identity 0.34, keyvault_keys 0.13\n\nUnified bump of all three coupled Azure SDK crates:\n- azure_core 0.30 → 0.34\n- azure_identity 0.30 → 0.34\n- azure_security_keyvault_keys 0.9 → 0.13\n\nAPI adaptations in mssql-auth:\n- cert_auth.rs: ClientCertificateCredential::new() now takes SecretBytes\n  instead of Secret for the certificate parameter\n- azure_keyvault.rs: key_version moved from options structs to a required\n  method parameter on unwrap_key(), sign(), verify(). CMK paths must now\n  include the key version (which Always Encrypted paths always do).\n  RequestContent<T> is now used for request bodies instead of .try_into()\n  conversion.\n- azure_identity_auth.rs: No changes needed — TokenCredential trait,\n  ManagedIdentityCredential, and ClientSecretCredential APIs unchanged.\n\nSupersedes dependabot PRs #76, #79, #81 which could not be merged\nindividually because the three crates share trait types.\n\n* refactor(client): extract validation to shared module (stored proc prep)\n\nMove validate_identifier() and validate_qualified_identifier() from\nclient/mod.rs and bulk.rs to a shared crate::validation module. Both\ncall sites now use the shared implementation, eliminating duplication.\n\nThis prepares for stored procedure support, which needs\nvalidate_qualified_identifier() for procedure name validation (same\nsecurity requirement as savepoint and bulk insert identifier validation).\n\n* feat(protocol): add col_type to ReturnValue and ProcedureResult type\n\nAdd `col_type: u8` field to ReturnValue struct so downstream code can\nconstruct a ColumnData for parse_column_value() without re-parsing.\nAdd `#[non_exhaustive]` to ReturnValue since it's a protocol-layer struct.\n\nAdd ProcedureResult type in stream.rs with return_value, rows_affected,\noutput_params, and result_sets fields. Includes accessor methods:\nget_output() (case-insensitive, @-prefix tolerant), get_return_value(),\nfirst_result_set(), has_result_sets().\n\nPart of stored procedure support (Step 4b).\n\n* feat(client): implement stored procedure support\n\nAdd complete stored procedure API with two-tier design:\n\n1. Simple convenience method for input-only calls:\n   client.call_procedure(\"dbo.MyProc\", &[&1i32, &\"hello\"]).await?\n\n2. Full builder for named/output parameters:\n   client.procedure(\"dbo.CalculateSum\")?\n       .input(\"@a\", &10i32)\n       .output_int(\"@result\")\n       .execute().await?\n\nImplementation details:\n- ProcedureBuilder with typed output methods (output_int, output_bigint,\n  output_nvarchar, output_bit, output_float, output_decimal, output_raw)\n- read_procedure_result() parser handles all TDS tokens: ColMetaData,\n  Row, NbcRow, DoneInProc, ReturnValue, ReturnStatus, DoneProc\n- ReturnValue decoding reuses parse_column_value() via ColumnData bridge\n- Extract convert_single_param() from convert_params() to share\n  SqlValue->RpcParam logic between query params and procedure builder\n- Methods on impl<S: ConnectionState> — works in both Ready and\n  InTransaction states with zero duplication\n- All procedure names validated via validate_qualified_identifier()\n- Send+Sync compile-time assertions for ProcedureResult and ProcedureBuilder\n- Visibility: send_rpc() and read_procedure_result() promoted to pub(crate)\n\nPart of stored procedure support (Steps 4c-4f).\n\n* docs(stored-procs): add tests, docs, and CHANGELOG entry\n\n- Integration tests: 11 tests covering simple calls, input params,\n  return values, multiple result sets, rows_affected, OUTPUT params\n  (int, nvarchar), builder with result sets and output, transactions,\n  error handling, and schema-qualified names\n- Unit tests: ProcedureResult defaults, get_output case-insensitive\n  and @-prefix stripping, result sets with return value\n- ARCHITECTURE.md: add section 4.7 covering RPC flow, token handling,\n  ReturnValue decoding, and security\n- docs/STORED_PROCEDURES.md: user guide with quick start, API reference,\n  transaction support, output types, security, and error handling\n- CHANGELOG.md: add stored procedure feature under [Unreleased], credit\n  @c5soft for PR #71's influence on API design\n\nPart of stored procedure support (Step 4g).\n\n* fix(docs): resolve broken ProcedureBuilder rustdoc link\n\nUse fully qualified path `crate::procedure::ProcedureBuilder` in the\ndoc comment for `Client::procedure()` so rustdoc can resolve it across\nmodule boundaries.\n\n* fix(client): add Clone derive to ProcedureResult and ResultSet\n\nProcedureResult was missing Clone, which the plan specified. ResultSet\nalso lacked Clone despite all its fields (Vec<Column>, VecDeque<Row>)\nbeing Clone. Both now derive Clone for consistency with OutputParam and\nExecuteResult.\n\n* feat(client): add SQL Browser instance resolution (#66)\n\nAdd automatic TCP port resolution for named SQL Server instances via\nthe SQL Server Browser service (SSRP protocol, MC-SQLR spec).\n\nWhen connecting with a named instance (e.g., Server=localhost\\SQLEXPRESS),\nthe driver now queries the Browser service on UDP 1434 to discover the\nTCP port before establishing the TCP connection. This is transparent to\nthe user — no API changes needed.\n\nNew module: crate::browser — implements CLNT_UCAST_INST request and\nSVR_RESP response parsing per the MC-SQLR specification. Handles:\n- \".\" as localhost (common for .\\SQLEXPRESS)\n- Timeout when Browser service is not running\n- Missing TCP port (instance may only support Named Pipes)\n- Malformed responses\n\nNew error variant: Error::BrowserResolution with instance name and\nreason for clear diagnostics.\n\nIntegration point: Client::try_connect() resolves the instance port\nbefore TCP connect when config.instance is Some.\n\nRequested by @tracker1 in #66.\n\nCloses #66\n\n* feat(auth): add native Windows SSPI for integrated authentication (#65)\n\nThe sspi-rs crate (pure Rust SSPI) cannot acquire credentials from the\ncurrent Windows logon session without explicit username/password. This\nmeans integrated auth (`Integrated Security=true`) was silently broken\non Windows for all account types — domain, local, and Microsoft\nAccounts.\n\nAdd a `NativeSspiAuth` provider that calls the actual Windows SSPI APIs\n(AcquireCredentialsHandleW / InitializeSecurityContextW from\nsecur32.dll) via the `windows` crate. The native SSPI subsystem has\ndirect access to LSASS cached credentials, supporting all account types\ntransparently — including Microsoft Accounts on Windows 11, which is\nthe specific scenario reported in #65.\n\nOn Windows with `sspi-auth` enabled, `Client::connect()` now uses\n`NativeSspiAuth` for integrated auth. On non-Windows, the existing\nsspi-rs path is preserved. `SspiAuth` (sspi-rs) remains available for\nexplicit credential scenarios on all platforms.\n\nCloses #65\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(auth): resolve windows-certstore compilation errors with windows 0.62 (#83)\n\nThe windows-certstore module had 9 compilation errors due to API\nchanges in the windows crate 0.62. This went undetected because the\nmodule is behind #[cfg(windows)] and CI runs on Linux.\n\nFixes applied:\n- BOOL: moved from Win32::Foundation to windows::core (now a newtype\n  struct in windows-result, re-exported through windows-core)\n- CRYPT_HASH_BLOB: replaced with CRYPT_INTEGER_BLOB (same layout,\n  renamed in windows 0.62)\n- CryptAcquireCertificatePrivateKey: params changed from &mut refs to\n  raw pointers with dedicated types (HCRYPTPROV_OR_NCRYPT_KEY_HANDLE,\n  CERT_KEY_SPEC, BOOL)\n- NCryptSignHash/NCryptVerifySignature: flags param type changed from\n  BCRYPT_FLAGS to NCRYPT_FLAGS (wrap via NCRYPT_FLAGS(BCRYPT_PAD_*.0))\n- CertCloseStore: first param now Option<HCERTSTORE>\n- NCryptFreeObject: use NCRYPT_HANDLE constructor instead of primitive\n  cast from NCRYPT_KEY_HANDLE\n- CertFreeCertificateContext: return type changed to must-use BOOL\n\nCloses #83\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* feat(client): add FILESTREAM BLOB access support (#67)\n\nAdd async read/write access to SQL Server FILESTREAM BLOBs via the\nWin32 OpenSqlFilestream API. This enables Rust applications to stream\nlarge binary data stored in SQL Server FILESTREAM columns using\ntokio-compatible async I/O.\n\nThe implementation uses runtime dynamic loading (LoadLibrary +\nGetProcAddress) to find OpenSqlFilestream in the OLE DB driver DLLs:\n  - msoledbsql19.dll (OLE DB Driver 19, newest)\n  - msoledbsql.dll (OLE DB Driver 18)\n  - sqlncli11.dll (SQL Server Native Client, deprecated fallback)\n\nThis avoids a compile-time dependency on any specific DLL and provides\na clear error when no FILESTREAM driver is installed.\n\nThe returned Win32 HANDLE is wrapped in tokio::fs::File for\nAsyncRead + AsyncWrite support.\n\nAPI:\n  - FileStream::open(path, access, txn_context) — low-level open\n  - Client<InTransaction>::open_filestream(path, access) — convenience\n    method that automatically fetches the transaction context\n  - FileStreamAccess enum: Read, Write, ReadWrite\n\nGated behind `#[cfg(all(windows, feature = \"filestream\"))]`. The\n`filestream` feature enables `tokio/fs` for the async file wrapper.\n\nCloses #67\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* test(client): add integration tests for SSPI and FILESTREAM (#65, #67)\n\nAdd end-to-end integration tests that verify both Windows-only features\nagainst a real SQL Server instance:\n\nSSPI tests (windows_sspi.rs):\n- Connect with integrated auth, verify NTLM/Kerberos auth_scheme\n- Execute query and transaction within SSPI session\n- Verify SQL Server version string\n\nFILESTREAM tests (windows_filestream.rs):\n- Read a FILESTREAM BLOB and verify content matches\n- Write to a FILESTREAM BLOB and read back to verify\n- Verify DLL loading produces OpenSqlFilestream error (not DLL error)\n\nAlso improves FILESTREAM error messages: OpenSqlFilestream failures now\ninclude the human-readable Win32 error message via FormatMessageW\ninstead of just the numeric error code.\n\nAll tests are gated with #[ignore] and require a Windows machine with\nSQL Server configured for Windows Authentication and FILESTREAM.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* docs(filestream): add documentation, CHANGELOG, and API improvements (#67)\n\nComplete the remaining work items from #67:\n\nDocumentation:\n- Add docs/FILESTREAM.md with full setup guide, usage examples,\n  troubleshooting, and API reference\n- Add CHANGELOG entry under [Unreleased] for FILESTREAM and\n  windows-certstore fix\n- Add filestream to README feature status and feature flags tables\n- Add filestream section to docs/FEATURES.md\n- Add FILESTREAM section to CLAUDE.md\n\nAPI improvements:\n- Add open_options module (NONE, SEQUENTIAL_SCAN, ASYNC) exposing\n  Win32 open flags for advanced users\n- Add FileStream::open_with_options() for custom open flags\n- Add #[non_exhaustive] to FileStreamAccess enum\n- Re-export open_options as filestream_options from lib.rs\n- Document async I/O strategy and future IOCP optimization path\n\nCode fixes:\n- Fix module doc: secur32.dll -> OLE DB Driver (was incorrect)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* docs: document Windows C++ build tools requirement\n\nThe default TLS feature depends on ring/aws-lc-sys which require a C\ncompiler. This is standard for Rust projects using rustls, but not\nobvious to developers on a fresh Windows machine without Visual Studio.\n\n- Add Windows C++ Build Tools section to CONTRIBUTING.md prerequisites\n  and step-by-step setup\n- Add installation note to README.md pointing to the setup docs\n- Note that sspi-auth and filestream features work without C++ tools\n  when TLS is disabled\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: fix formatting in Windows-specific modules\n\ncargo fmt on the test files and modules authored on Windows,\nwhich had minor formatting differences from the CI linter.\n\n* docs: update LIMITATIONS.md for v0.8.0+ features\n\nUpdate \"Last updated\" date from January 2026 to April 2026.\nCorrect the \"Cross-Platform\" design principle to reflect that\nWindows-specific features (SSPI, FILESTREAM, CertStore) are now\nsupported behind feature flags, not excluded.\n\n* refactor: replace unwrap() with expect() in library code\n\nSystematic audit of all unwrap() calls in non-test library code.\nReplaced with expect() containing context strings that document\nthe invariant being relied upon.\n\nChanges:\n- column_parser.rs: 16 chrono date/time construction unwraps →\n  expect() with context (e.g., \"epoch 1900-01-01 is valid\",\n  \"SMALLDATETIME minutes should be 0-1439\")\n- params.rs: 3 chrono epoch date unwraps → expect()\n- mock_server.rs: 1 TLS acceptor unwrap → expect()\n\nRemaining unwrap() calls (3) are in mssql-derive proc macros\nwhere clippy::expect_used is denied at the crate level. These\nare field.ident.as_ref().unwrap() on named struct fields, which\nis a guaranteed invariant in proc macro context.\n\nAddresses the code quality concern from the external project review.\n\n* test(protocol,auth): add unit tests for safety-critical token and auth paths\n\ntds-protocol (18 new tests):\n- ReturnValue: INT output, NVARCHAR output, NULL, UDF status (0x02)\n- ReturnStatus: success (0), negative return codes\n- DoneProc: roundtrip, parser, error flag\n- DoneInProc: roundtrip, parser with MORE|COUNT flags\n- ServerError: realistic decode, severity helpers, parser integration\n- Multi-token streams: stored proc response, error mid-stream,\n  ReturnValue->ReturnStatus->DoneProc sequence\n- EOF/truncation edge cases for ReturnStatus, DoneProc, ServerError\n\nmssql-auth (16 new tests):\n- AuthError: transient/terminal classification, mutual exclusion,\n  ambiguous error classification, Display output, Send+Sync bounds\n- AuthMethod: all-variants classification, Certificate variant\n- AuthData: SqlServer, FedAuth, Sspi, None variants, Debug output\n- AuthProvider trait: default method coverage via mock provider\n\n* refactor: audit panic-family macros in library code\n\nSystematic audit of all unreachable!() calls in non-test library code\n(5 total). No panic!, todo!, or unimplemented! macros were found.\n\nConverted to error (1):\n- params.rs: TvpColumnType wildcard arm now returns\n  TypeError::UnsupportedConversion instead of panicking. TvpColumnType\n  is #[non_exhaustive], so a new upstream variant would previously crash;\n  it now returns a descriptive error. Changed convert_tvp_column_type to\n  return Result<TvpWireType> with fallible collect at the call site.\n\nDocumented as justified (4):\n- column_parser.rs: inner match on Money|Money4|MoneyN is bounded by\n  the outer match arm — genuinely unreachable.\n- params.rs: inner match on chrono types is bounded by the outer match\n  arm — genuinely unreachable.\n- tvp.rs (2): encode_tvp_int and encode_tvp_float size parameters are\n  always derived from TvpWireType enum variants (1/2/4/8 and 4/8).\n  Added # Panics doc sections and descriptive messages.\n\n* feat(client): wire Always Encrypted decryption into query execution\n\nIntegrate the existing Always Encrypted cryptographic infrastructure\ninto the query execution flow, enabling transparent decryption of\nencrypted column values when Column Encryption Setting=Enabled.\n\nProtocol layer (tds-protocol):\n- Extended CryptoMetadata with base_user_type, base_col_type, and\n  base_type_info fields per MS-TDS 2.2.7.4\n- Added ColMetaData.cek_table and ColumnData.crypto_metadata fields\n- Added decode_encrypted() and decode_column_encrypted() for parsing\n  encrypted column metadata from the wire\n- Extracted decode_type_info/decode_collation as pub(crate) for reuse\n\nClient layer (mssql-client):\n- New column_decryptor module: async-to-sync bridge that pre-resolves\n  CEK encryptors at ColMetaData time, then decrypts synchronously per row\n- Updated read_query_response() with full decryption support\n- Added convert_raw_row_decrypted() and decrypt_column() to column_parser\n- User-facing column metadata shows base type, not wire type (BigVarBinary)\n- Connection string: Column Encryption Setting=Enabled parsing\n- Login7: COLUMNENCRYPTION feature extension (0x04) sent when enabled\n- EncryptionContext stored on Client struct\n\nSecurity:\n- Key material never logged in error messages\n- AEAD decryption verifies HMAC (constant-time) before decrypting\n- Corrupted/tampered ciphertext returns Err, never garbled data\n\nStill deferred:\n- Parameter encryption (requires sp_describe_parameter_encryption)\n- Procedure/multi-result decryption wiring (follow-up using same pattern)\n- Enclave computations (COLUMNENCRYPTION_VERSION 2/3)\n\n* fix(client): wire Always Encrypted decryption into procedure and multi-result readers\n\nread_procedure_result() and read_multi_result_response() were missing\nthe decryption branch that read_query_response() already had. This meant\nencrypted columns returned raw ciphertext (silent data corruption) when\naccessed via call_procedure() or query_multiple().\n\nApplied the same pattern from read_query_response(): pre-resolve a\nColumnDecryptor at ColMetaData time, then branch on it for each\nRow/NbcRow token to call convert_raw_row_decrypted() or the plain\nvariant. All three response readers now have symmetric decryption\nsupport.\n\n* fix(client): address silent error swallowing in stream and bulk modules\n\nstream.rs: Replace filter_map(|r| r.ok()) in test with explicit unwrap()\nso test failures surface instead of being silently dropped. Add comment\nto collect_current() clarifying that unwrap_or_default() is on Option,\nnot Result — no errors are being swallowed.\n\nbulk.rs: Document that .parse().ok() chains in parse_sql_type() are\nintentional best-effort parsing. When a type parameter like \"VARCHAR(100)\"\nhas a malformed length, falling through to the SQL Server default (8000)\nis safer than rejecting the operation when the base type is valid.\n\n* docs: update CHANGELOG, STABILITY, CLAUDE, README for v0.9.0 release prep\n\nCHANGELOG: Add missing Unreleased items — Always Encrypted decryption\nintegration, unwrap/panic audits, 34 new unit tests, LIMITATIONS.md\nupdate, procedure/multi-result decryption fix, silent error swallowing\nfix.\n\nSTABILITY: Add FILESTREAM, Always Encrypted, and KeyStoreProvider APIs\nto the stable surface table. Add filestream and encoding feature flags\nto the feature stability table.\n\nCLAUDE.md: Document the Always Encrypted decryption wiring — the\ncolumn_decryptor module, the CEK pre-resolution pattern, and how\ndecryption flows through all three response readers.\n\nREADME: Update always-encrypted feature description to mention\ntransparent column decryption.\n\nNew: docs/ALWAYS_ENCRYPTED.md user guide covering quick start, key store\nproviders, connection string keywords, encryption types, and security\nconsiderations.\n\n* fix(client): normalize (local) host alias to 127.0.0.1 for named instances\n\nServer=(local)\\SQLEXPRESS is a standard ADO.NET connection string format.\nPreviously only \".\" was normalized to localhost — \"(local)\" was passed\nthrough as-is, causing DNS resolution failure.\n\nNow both \".\" and \"(local)\" are normalized to 127.0.0.1 in both the\nSQL Browser resolver and the TCP connect path.\n\nReported by @tracker1 in #66.\n\n* feat(client): ADO.NET connection string conformance and LOGIN7 wire fixes\n\nRewrite the connection string parser to conform to the Microsoft\nADO.NET SqlConnection.ConnectionString specification:\n\nParser correctness:\n- Quoted value support: Password=\"my;pass\" and Password='it''s complex'\n  now work per spec. Previously semicolons in passwords were silently\n  truncated, causing misleading login failures.\n- tcp: prefix stripping: Server=tcp:host.database.windows.net,1433\n  (Azure Portal format) now works. np: and lpc: prefixes return errors.\n- Boolean validation: invalid values like TrustServerCertificate=banana\n  now error instead of silently defaulting to false.\n\nNew keywords (with LOGIN7 wire integration):\n- ApplicationIntent (ReadOnly/ReadWrite) wired to READONLY_INTENT bit\n- Workstation ID / WSID wired to LOGIN7 HostName field\n- Current Language / Language wired to LOGIN7 Language field\n- ConnectRetryCount/Interval wired to RetryPolicy\n- Server aliases: Addr, Address, Network Address\n- Timeout alias for Connect Timeout\n\nKnown keyword handling:\n- Pool keywords (Max Pool Size, etc.) logged at info with guidance\n- 30+ ADO.NET keywords recognized at info instead of silent debug\n\nLOGIN7 bug fix:\n- HostName field now sends actual client machine name instead of\n  server hostname. The MS-TDS spec defines this as \"the name of\n  the client machine\" — we were sending config.host (the server).\n\n* docs: document connection string conformance improvements\n\nUpdate CONNECTION_STRINGS.md:\n- Add quoted value syntax section with examples\n- Add tcp: prefix and unsupported protocol documentation\n- Add Addr, Address, Network Address as Server aliases\n- Add Timeout alias for Connect Timeout\n- Add ApplicationIntent, Workstation ID, Language keywords\n- Add ConnectRetryCount/Interval section\n- Add \"Recognized but Not Supported\" table for known ADO.NET keywords\n\nUpdate CHANGELOG.md with full feature list and LOGIN7 HostName fix.\n\n* fix(client): address remaining connection string conformance gaps\n\nFive issues identified during post-implementation review:\n\n1. Protocol prefix case sensitivity: tcp:/np:/lpc: checks now fully\n   case-insensitive via lowercase comparison. Previously only exact\n   \"tcp:\" and \"TCP:\" were handled — \"Tcp:\" would not be stripped.\n\n2. CLAUDE.md updated with connection string parser documentation per\n   convention #6 (\"Update CLAUDE.md when you add new infrastructure\").\n\n3. STABILITY.md: ApplicationIntent, Config::workstation_id(), and\n   Config::language() added to the stable API surface table.\n\n4. Empty value handling: Database=; now stores None instead of\n   Some(\"\"), matching ADO.NET reset-to-default semantics. Applied\n   to all Option<String> fields (database, instance, workstation_id,\n   language) via new non_empty() helper.\n\n5. Encrypt=Mandatory and Encrypt=Optional now recognized as aliases\n   for true/false, matching Microsoft.Data.SqlClient v5+ behavior.\n\n* docs: cross-reference ALWAYS_ENCRYPTED.md, update FEATURES.md and milestones\n\n- CLAUDE.md Document References: add CONNECTION_STRINGS.md,\n  ALWAYS_ENCRYPTED.md, and STORED_PROCEDURES.md\n- PRODUCTION_READINESS.md: add v0.9.0 milestone entry\n- docs/FEATURES.md: update always-encrypted description to mention\n  transparent column decryption and link to the user guide\n\n* fix(deps): resolve RUSTSEC-2026-0098 and RUSTSEC-2026-0099 (rustls-webpki)\n\ncargo update -p rustls-webpki: 0.103.10 → 0.103.12\n\n- RUSTSEC-2026-0098: Name constraints for URI names were incorrectly accepted\n- RUSTSEC-2026-0099: Name constraints accepted for wildcard certificates\n\nBoth are X.509 name constraint validation bugs. While exploitation requires\ncertificate misissuance (low practical risk), they block release preflight.\n\n* chore: release v0.9.0\n\n- Bump workspace version 0.8.0 → 0.9.0\n- CHANGELOG.md: rename [Unreleased] → [0.9.0] - 2026-04-15\n- Update version references in README.md, RELEASING.md, docs/FEATURES.md,\n  docs/FILESTREAM.md\n- typos.toml: add secur32.dll and \"tru\" test value to exceptions\n\n---------\n\nCo-authored-by: Justin Kindrix <justin.kindrix@nuby.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-04-15T16:02:05-05:00",
          "tree_id": "2073e283ff34772872df0519398f92118bad9a7b",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/7760f0c6892cb451d67eb2b3a00c3dde7179815d"
        },
        "date": 1776287686635,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 869,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 912,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 994,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 1815,
            "range": "± 6",
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
            "value": 97,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 124,
            "range": "± 4",
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
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 508,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 3619,
            "range": "± 199",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 258,
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
            "value": 140,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 648,
            "range": "± 10",
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
            "range": "± 1",
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
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/medium",
            "value": 856,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2813,
            "range": "± 137",
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
          "id": "fcbc5a2c3e45091ddc0e044b3c52a6571c1c8ae4",
          "message": "Release v0.10.0 (#89)\n\n* ci: run on dev branch, add workflow_dispatch, add token health check\n\nPost-v0.7.0 release hygiene improvements addressing three pain points\nsurfaced during the 0.7.0 release cycle:\n\n## 1. CI on dev branch\n\nci.yml and benchmarks.yml now trigger on pushes to dev, not just main.\nThe 42-commit backlog that shipped in v0.7.0 had zero CI coverage between\nthe last main-branch CI run (2026-01-13) and the release PR (#69). That's\nhow the mock_server TLS test failure on macOS/Windows and the xtask\n--no-dev-deps bug were both latent for months before surfacing at release\ntime. Running CI on every dev push ensures cross-platform and feature-flag\nissues are caught within minutes of landing.\n\nsecurity-audit.yml also now triggers on dev pushes (not just main) when\nCargo.toml, Cargo.lock, deny.toml, or .cargo/audit.toml change. This\nmeans transitive security advisories are caught as soon as a dep bump\nlands on dev, not only after it reaches main.\n\nBoth workflows use concurrency cancel-in-progress for non-main branches\nto avoid wasted CI cycles when multiple commits land in quick succession.\nMain always runs to completion to preserve the full audit trail per\ncommit.\n\n## 2. Manual workflow dispatch\n\nAdded workflow_dispatch to ci.yml and benchmarks.yml. The\nsecurity-audit.yml workflow already had it. This allows maintainers to\nmanually retrigger a workflow from the Actions tab without needing to\npush a dummy commit, which was awkward during the v0.7.0 release when\nwe needed to re-run after the token was updated.\n\n## 3. Token health check workflow\n\nNew .github/workflows/token-health.yml that runs weekly (Monday 09:15\nUTC, staggered after Security Audit) and verifies the\nCARGO_REGISTRY_TOKEN secret still authenticates against\nhttps://crates.io/api/v1/me. On failure, opens (or updates) a tracking\nissue labelled `security` with rotation instructions.\n\nThis directly prevents the exact failure mode hit during the v0.7.0\nrelease: the token had silently expired between v0.6.0 and v0.7.0 and\nthe release workflow only discovered this when it tried to upload\ntds-protocol and was rejected with HTTP 403. A weekly health check\nwould have surfaced the problem up to 7 days earlier, giving plenty\nof time to rotate before a real release attempt.\n\nThe check uses curl against the /api/v1/me endpoint (not\n`cargo publish --dry-run`, which does not actually authenticate\nunless it reaches the upload step). The call is cheap, non-destructive,\nand unambiguous: 200 means the token works, anything else means it\ndoesn't.\n\nRefs #63 (security audit reliability)\nRefs #64 (v0.7.0 release pain)\n\n* feat(tooling): add release-status, release-preflight, and xtask release-notes\n\nThree new release-observability tools that directly address pain points\nfrom the 0.7.0 release cycle:\n\n## just release-status\n\nDashboard recipe that reports everything you need to know before cutting\na release, in one command:\n- Current branch and workspace version\n- Last tag, date, days elapsed\n- dev vs main divergence (detects release candidates AND drift)\n- Latest run status for CI, Security Audit, Benchmarks, and the new\n  Token Health Check workflow\n- Open dependabot PRs with per-PR failing-check counts\n- Open contributor PRs (uses author.is_bot filter, not login name, so\n  \"app/dependabot\" is correctly excluded)\n- Open issue count\n- Local working-copy cleanliness and unpushed commits\n\nThis answers the question \"should I release?\" at a glance. Before 0.7.0\nwe walked into the release with 42 accumulated commits and no visibility\ninto the size of the backlog.\n\n## just release-preflight\n\nSequential gate check that runs every Cardinal Rule from RELEASING.md:\nworking-copy clean, version refs consistent, cargo audit clean, cargo\ndeny clean, no WIP markers, metadata valid, URLs valid, tier-0 package\ndry-run succeeds. Captures the full verification checklist that we\npreviously had to remember to run manually.\n\nIntegrates with the upcoming scripts/check-doc-consistency.sh linter\n(conditional — only runs if the script is present and executable).\n\n## cargo xtask release-notes\n\nGenerates a CHANGELOG draft from git log. Reads conventional commits\nbetween the last v*.*.* tag and HEAD, parses headers of the form\n\"type(scope)?!?: subject\", groups by bucket (feat → Added, fix → Fixed,\netc.), detects breaking changes via \"!\" suffix or \"BREAKING CHANGE\"\nfooter, and emits markdown suitable for pasting into CHANGELOG.md.\n\nNon-conforming commits are placed under an \"Other\" bucket with a\nreview prompt rather than being dropped silently. Merge commits are\nskipped.\n\nTested against the v0.6.0..v0.7.0 range — correctly identifies both\nbreaking changes (d4ef523 \"remove deprecated items before 1.0\" and\n7722158 \"add #[non_exhaustive] to 33 public enums\"), categorizes ~42\ncommits across Added / Fixed / Changed / Documentation / CI / Chores\nsections, and produces a draft that's ~80% ready to commit.\n\nUses the system 'date' command for \"today's date\" to avoid adding\nchrono as a new xtask dependency.\n\n* docs(community): add issue/PR templates, CoC, CODEOWNERS, MAINTAINERS.md\n\nRaise the contributor-facing surface area of the project. For a repo\nwith 13 stars and three real contributors (@VincentMeilinger, @tracker1,\n@c5soft) showing up organically, having none of these files was a real\nfriction point.\n\n## Issue templates (.github/ISSUE_TEMPLATE/)\n\n- config.yml: disables blank issues, adds contact_links pointing at\n  Security Advisories (private security reports), Discussions\n  (conversational questions), docs.rs (API reference), and the\n  Tiberius migration guide.\n- bug_report.yml: structured YAML form with required fields for driver\n  version, feature flags (multi-select of every feature in the workspace),\n  Rust version, OS, SQL Server version, TDS version, logs, and repro\n  code. Matches the kind of detail we actually need to debug TDS-level\n  issues.\n- feature_request.yml: use case first, API proposal second, alternatives\n  considered, area of codebase (multi-select of workspace crates),\n  breaking-change assessment.\n- question.yml: routes to docs.rs, connection strings, migration guide,\n  Discussions first; falls back to a minimal structured form.\n\nAll templates use YAML form format (not markdown) so required fields\nare actually enforced and the resulting issues are well-structured.\n\n## Pull request template (.github/pull_request_template.md)\n\nSections:\n- Summary and linked issues (with \"Closes #\" pattern)\n- Type of change checklist\n- Test plan (fmt/clippy/test commands + integration test hook)\n- Documentation updates checklist (rustdoc, CHANGELOG, README, docs/, ARCHITECTURE)\n- **Breaking changes section with explicit MSRV note**:\n  \"MSRV bumps are NOT a breaking change per STABILITY.md § MSRV\n  Increase Policy\". This prevents the exact confusion that caused\n  the CONTRIBUTING.md ↔ STABILITY.md contradiction we fixed during\n  the 0.7.0 cycle.\n- Security considerations (for auth/TLS/SQL-gen PRs)\n- Pre-1.0 policy reference link\n\n## CODE_OF_CONDUCT.md\n\nCONTRIBUTING.md already referenced \"the Rust Code of Conduct\" by URL\nbut the repo had no CoC file. Added the full Rust CoC text with\nproject-specific contact info pointing at the private Security\nAdvisory channel for CoC violations.\n\n## .github/CODEOWNERS\n\nAuto-requests review from the primary maintainer for PRs touching\nspecific areas. Today this is all @jkindrix, but the file is\nstructured so that adding future maintainers (as described in\nMAINTAINERS.md) is a one-line change per area. Ownership is granular:\nprotocol layer, auth, TLS, pool, client, types, derive, testing,\nxtask, workflows, release policy docs, security policy docs, and\ndependency/supply-chain files each get their own entry.\n\n## MAINTAINERS.md\n\nDocuments:\n- Who the current maintainers are\n- What maintainers are responsible for (review, triage, releases,\n  stewardship, continuity)\n- How to contact maintainers for different purposes (issues,\n  discussions, security reports, CoC reports)\n- How to become a maintainer (criteria, trial process)\n- Current decision-making model with a pointer to ARCHITECTURE.md ADRs\n  for architectural decisions\n- An empty Emeritus section for eventual use\n\nIncludes language around the \"correctness-first, quality-over-speed\"\nphilosophy so new maintainers know what they're signing up for.\n\n* fix(testing): fix mock server TLS close_notify (closes #70) + add doc linter\n\nTwo related fixes that both address pre-existing latent bugs surfaced\nduring the v0.7.0 release cycle.\n\n## Mock server TLS close_notify (closes #70)\n\n`handle_connection` now explicitly calls `tls_stream.shutdown().await`\nbefore the stream goes out of scope. Without this, dropping the\n`TlsStream` closes the underlying TCP half abruptly, and rustls on\nstricter platforms (macOS, Windows) reports\n\"peer closed connection without sending TLS close_notify\" on whatever\nread was in flight, even if the server had already written the\nresponse. Linux's rustls build tolerated this silently, which is why\nthe bug was latent until the full CI matrix ran on the v0.7.0\nrelease PR.\n\nThe shutdown call is wrapped in `let _ =` because the session is\nalready over at that point — we only care about best-effort clean\nclose. A real driver would log the error; the mock server is\ntest-only infrastructure.\n\nRemoved the `#[cfg(target_os = \"linux\")]` gate on\n`test_mock_server_tls_full_connection` — it now runs on all three\nplatforms in CI. Local `cargo test -p mssql-testing --test mock_fidelity`\npasses with 13 tests (1 ignored as before).\n\n## Documentation consistency linter\n\nNew `scripts/check-doc-consistency.sh` validates invariants across the\nproject's documentation and config files. This codifies a class of bug\nthat produced the CONTRIBUTING.md ↔ STABILITY.md MSRV-policy\ncontradiction discovered during the 0.7.0 release.\n\nChecks (22 total on the current tree):\n\n- **MSRV consistency**: rust-toolchain.toml, xtask/Cargo.toml,\n  README.md badge, CLAUDE.md, ARCHITECTURE.md, STABILITY.md,\n  CONTRIBUTING.md prerequisites table, RELEASING.md header,\n  Justfile variable — all must reference the same MSRV value\n  that workspace Cargo.toml declares.\n- **CHANGELOG ↔ workspace version**: latest non-[Unreleased] entry\n  must match workspace.package.version.\n- **MSRV policy agreement**: STABILITY.md must explicitly state\n  that MSRV bumps are non-breaking, AND CONTRIBUTING.md must NOT\n  list 'Increasing MSRV' under 'Definitely Breaking'. This prevents\n  the specific contradiction that was discovered post-v0.6.0.\n- **Workspace crate version inheritance**: every crate in crates/\n  must either inherit workspace version or explicitly pin the same\n  workspace version (no silent drift).\n- **deny.toml ↔ .cargo/audit.toml sync**: advisory ignore lists in\n  the two files must be identical. cargo-deny reads deny.toml,\n  cargo-audit reads audit.toml, and they must stay in sync or\n  we end up with different results in CI vs locally.\n\nThe linter is pure bash, uses only standard tools (grep, awk, sed,\ncomm), and runs in <100ms. No new dependencies.\n\nAdded `just doc-consistency` recipe that invokes the script with\na graceful no-op if the script is missing, and wired it into\n`just release-check` as an additional gate.\n\nOutput supports `--verbose` for full per-check reporting and is\ncolor-coded via ANSI escapes (suppressed when NO_COLOR is set or\nnot running on a TTY).\n\n* docs: update RELEASING, CONTRIBUTING, README, CLAUDE; add DEPENDENCY_POLICY\n\nComprehensive documentation refresh that captures everything learned\nduring the v0.7.0 release cycle and the post-release hygiene sprint.\n\n## RELEASING.md\n\nAdded a new \"Lesson 12: The v0.7.0 Incidents\" section documenting\nthree distinct problems that surfaced during the 0.7.0 release —\nCARGO_REGISTRY_TOKEN expiry discovered at publish time, pre-existing\nlatent bugs surfaced at release PR time due to dev-branch CI gap,\nand the CONTRIBUTING.md ↔ STABILITY.md MSRV policy contradiction.\n\nEach incident is written up with \"What went wrong\", \"Result\", and\n\"Solution\" subsections naming the concrete mitigations we put in\nplace in this release hygiene sprint.\n\nAdded a new \"Token Health\" top-level section explaining:\n- How the weekly token-health.yml workflow works\n- Step-by-step manual rotation procedure\n- What to do if a release is in flight when the token fails\n  (the exact recovery path used during v0.7.0)\n- Why the worst-case failure mode is \"zero crates published, rerun\n  after rotation\" rather than a partial publish\n\n## CONTRIBUTING.md\n\n- Added \"First Contribution (Quick Path)\" section with 7-step\n  recipe from clone to green CI\n- Added \"When Your PR Needs Review\" section explaining CODEOWNERS\n  auto-routing, expected response times, the draft-PR-for-big-changes\n  pattern, and why large feature PRs take longer\n- Updated Pull Request Process to reference the new PR template\n  and require `cargo test --all-features`, `cargo clippy --all-features`,\n  and `just doc-consistency` as mandatory pre-submit gates\n- Extended the \"Build Automation\" section with the new xtask\n  subcommand (`release-notes`) and a dedicated \"Release-Adjacent\n  Just Recipes\" table covering release-status, release-preflight,\n  release-check, doc-consistency, ci-status-all, and tag\n- Updated Code of Conduct section to link to the new\n  CODE_OF_CONDUCT.md file (previously the reference was to a URL\n  with no local file)\n\n## README.md\n\n- Rewrote the Contributing section with bullet pointers to:\n  first-contribution path, issue templates, PR template, ADR process,\n  CoC, and MAINTAINERS.md\n- Added a new Community section with links to Discussions, Issues,\n  and the private Security Advisory channel\n\n## CLAUDE.md\n\nTwo new sections:\n\n1. **Process and Governance** — enumerates every contributor-facing\n   document (README, CONTRIBUTING, CODE_OF_CONDUCT, MAINTAINERS,\n   CODEOWNERS, issue/PR templates), every release and policy document\n   (RELEASING, STABILITY, SECURITY, VERSION_REFS, DEPENDENCY_POLICY),\n   and the full list of release observability tooling (just recipes\n   and xtask commands) added post-v0.7.0. Also lists all CI/CD\n   workflows with their triggers.\n\n2. **Conventions for AI Assistants** — explicit notes that MSRV\n   bumps are NOT breaking changes (STABILITY.md is authoritative,\n   the linter catches contradictions), that fixing beats ignoring\n   for security advisories (with the v0.7.0 MSRV-bump precedent),\n   that the release recipes exist to prevent the exact mistakes\n   that caused past incidents, that the Cardinal Rules in RELEASING.md\n   are non-negotiable, that dev-branch CI now exists so push\n   confidently, and that future AI sessions must update CLAUDE.md\n   when they add new infrastructure.\n\nUpdated the Development Tooling section to list all actually-required\ntools (just, gh, cargo-deny, cargo-hack, cargo-nextest, cargo-audit,\ncargo-machete, cargo-semver-checks) and point at `just setup-tools`.\n\nRemoved the now-obsolete cargo-hakari section (the workspace-hack\ncrate was removed in v0.7.0 — see commit ce2852a).\n\nUpdated the Document References section with the full list of\nprimary references including the new DEPENDENCY_POLICY.md.\n\n## docs/DEPENDENCY_POLICY.md (new)\n\nCaptures tribal knowledge about dependency management decisions into\ndiscoverable documentation. Sections:\n\n- **Philosophy** — 5 principles (minimize surface area, correctness\n  over convenience, modern Rust not cutting-edge, pure Rust where\n  it matters, minimize feature flags)\n- **Adding a new dependency** — 8-criterion checklist\n- **Taking a dependency upgrade** — patch/minor/major handling,\n  when to defer, bundled bumps (OpenTelemetry, Tokio)\n- **Handling security advisories** — three cases with explicit\n  guidance and the v0.7.0 MSRV-bump precedent documented as\n  Case B. Explains the MSRV-vs-ignore tradeoff we actually made\n  for the time crate.\n- **deny.toml and .cargo/audit.toml sync** — how the linter\n  enforces this\n- **Removing a dependency** — 5-step cleanup workflow\n- **New license requirements** — how we evaluate unfamiliar\n  SPDX license strings, with the MIT-0 precedent from v0.7.0\n- **Maintenance schedule** — weekly dependabot, weekly security\n  audit, weekly token health, per-release cargo update cadence,\n  quarterly major-version review\n\nReferenced from CLAUDE.md, CONTRIBUTING.md, and RELEASING.md.\n\n* fix(testing): re-gate mock TLS full connection test to Linux only\n\nThe explicit tls_stream.shutdown().await added in cee9751 did not fix\nthe macOS/Windows failure. CI confirmed: same \"peer closed connection\nwithout sending TLS close_notify\" error on both platforms.\n\nRoot cause analysis (updated in #70): the error occurs during the\nLoginAck header read, meaning the client sees EOF BEFORE the server's\nresponse arrives. The shutdown() fix only addresses connection-close\ntime; the actual issue is a timing/buffering race in the mock server's\nTLS path where the TCP buffer flush doesn't reach the client before\nthe read polls on macOS/Windows.\n\nThe production driver is NOT affected — it uses a different connection\narchitecture (Connection<T> with framed I/O) that doesn't have this\ntiming dependency.\n\nRe-gating with comprehensive investigation notes in the test doc\ncomment so the next person who picks this up (#70) knows exactly what\nwas tried and why it didn't work.\n\nThe shutdown() call is kept in the mock server — it's correct for\nclean TLS close even though it doesn't fix this particular race.\n\n* fix(deps): resolve security audit + bump dev dependencies and CI actions\n\nAdvisory handling:\n- Add RUSTSEC-2026-0097 (rand 0.8.5 unsoundness) ignore — Case C per\n  DEPENDENCY_POLICY: no stable fix available, log feature not enabled,\n  unsoundness conditions unmet. Blocked on rsa 0.10 stable (#21).\n- Remove stale RUSTSEC-2026-0066 ignore — resolved by testcontainers\n  0.27 (pulls astral-tokio-tar 0.6.0 which includes the fix).\n- Update RUSTSEC-2025-0134 ignore reason — rustls-pemfile is a direct\n  dep of mssql-auth, not just transitive via bollard.\n\nDependency bumps (all dev-only or patch-level):\n- rustls 0.23.37 → 0.23.38 (patch)\n- tokio 1.51.0 → 1.51.1 (patch)\n- testcontainers 0.25.2 → 0.27.2 (dev-only, drops rustls-pemfile\n  from bollard path, fixes astral-tokio-tar advisory)\n- criterion 0.7.0 → 0.8.2 (dev-only benchmarks)\n- bollard 0.19.4 → 0.20.2 (transitive via testcontainers)\n\nCI action bumps:\n- codecov/codecov-action v5 → v6 (Node 24 runtime)\n- softprops/action-gh-release v2 → v3 (Node 24 runtime)\n- actions/github-script v8 → v9 (both security-audit.yml and\n  token-health.yml, for consistency)\n\n* fix(testing): use multi-thread runtime for TLS full connection test (#70)\n\nThe test_mock_server_tls_full_connection test was gated to Linux only\ndue to a timing race: the mock server's raw write_all() + flush() on\nthe TLS-over-PreLogin stream didn't reliably deliver data before the\nsingle-thread runtime's cooperative scheduler yielded on macOS/Windows.\n\nFix: Use #[tokio::test(flavor = \"multi_thread\", worker_threads = 2)]\nwhich matches production usage and eliminates the scheduling dependency\nbetween server and client tasks. Remove the #[cfg(target_os = \"linux\")]\ngate so the test runs on all CI platforms.\n\nCloses #70\n\n* fix(testing): fix mock TLS cross-platform race in TlsPreloginWrapper (#70)\n\nRoot cause: the client completes the TLS handshake and sends Login7 as\nraw TLS (ApplicationData 0x17) before the server-side TlsPreloginWrapper\nhas switched to pass-through mode. On macOS/Windows, TCP coalesces these\nbytes into one read, so the server's wrapper (still in handshake mode)\ninterprets the raw TLS record header as a TDS PreLogin header and fails\nwith InvalidContentType.\n\nFix: auto-detect non-PreLogin bytes during handshake mode. When the\nwrapper reads a header byte that isn't 0x12 (PreLogin), it means the\npeer has already switched to raw TLS. The wrapper auto-transitions to\npass-through mode and feeds the already-read header bytes back to the\ncaller via a prefix buffer, so rustls can process them as the TLS record\nthey actually are.\n\nThe production client-side wrapper (mssql-tls) is NOT affected because\nthe client always finishes the handshake first — the server never sends\nraw TLS before the client's wrapper has switched.\n\nDiagnosed with targeted experiments on macOS and Windows machines that\nrevealed the server never reached send_login_response — it failed during\nread_packet for Login7 because the TLS stream received corrupted data\nfrom the wrapper.\n\nRemove the #[cfg(target_os = \"linux\")] gate so the test runs on all CI\nplatforms.\n\nCloses #70\n\n* feat(auth): bump Azure SDK to azure_core/identity 0.34, keyvault_keys 0.13\n\nUnified bump of all three coupled Azure SDK crates:\n- azure_core 0.30 → 0.34\n- azure_identity 0.30 → 0.34\n- azure_security_keyvault_keys 0.9 → 0.13\n\nAPI adaptations in mssql-auth:\n- cert_auth.rs: ClientCertificateCredential::new() now takes SecretBytes\n  instead of Secret for the certificate parameter\n- azure_keyvault.rs: key_version moved from options structs to a required\n  method parameter on unwrap_key(), sign(), verify(). CMK paths must now\n  include the key version (which Always Encrypted paths always do).\n  RequestContent<T> is now used for request bodies instead of .try_into()\n  conversion.\n- azure_identity_auth.rs: No changes needed — TokenCredential trait,\n  ManagedIdentityCredential, and ClientSecretCredential APIs unchanged.\n\nSupersedes dependabot PRs #76, #79, #81 which could not be merged\nindividually because the three crates share trait types.\n\n* refactor(client): extract validation to shared module (stored proc prep)\n\nMove validate_identifier() and validate_qualified_identifier() from\nclient/mod.rs and bulk.rs to a shared crate::validation module. Both\ncall sites now use the shared implementation, eliminating duplication.\n\nThis prepares for stored procedure support, which needs\nvalidate_qualified_identifier() for procedure name validation (same\nsecurity requirement as savepoint and bulk insert identifier validation).\n\n* feat(protocol): add col_type to ReturnValue and ProcedureResult type\n\nAdd `col_type: u8` field to ReturnValue struct so downstream code can\nconstruct a ColumnData for parse_column_value() without re-parsing.\nAdd `#[non_exhaustive]` to ReturnValue since it's a protocol-layer struct.\n\nAdd ProcedureResult type in stream.rs with return_value, rows_affected,\noutput_params, and result_sets fields. Includes accessor methods:\nget_output() (case-insensitive, @-prefix tolerant), get_return_value(),\nfirst_result_set(), has_result_sets().\n\nPart of stored procedure support (Step 4b).\n\n* feat(client): implement stored procedure support\n\nAdd complete stored procedure API with two-tier design:\n\n1. Simple convenience method for input-only calls:\n   client.call_procedure(\"dbo.MyProc\", &[&1i32, &\"hello\"]).await?\n\n2. Full builder for named/output parameters:\n   client.procedure(\"dbo.CalculateSum\")?\n       .input(\"@a\", &10i32)\n       .output_int(\"@result\")\n       .execute().await?\n\nImplementation details:\n- ProcedureBuilder with typed output methods (output_int, output_bigint,\n  output_nvarchar, output_bit, output_float, output_decimal, output_raw)\n- read_procedure_result() parser handles all TDS tokens: ColMetaData,\n  Row, NbcRow, DoneInProc, ReturnValue, ReturnStatus, DoneProc\n- ReturnValue decoding reuses parse_column_value() via ColumnData bridge\n- Extract convert_single_param() from convert_params() to share\n  SqlValue->RpcParam logic between query params and procedure builder\n- Methods on impl<S: ConnectionState> — works in both Ready and\n  InTransaction states with zero duplication\n- All procedure names validated via validate_qualified_identifier()\n- Send+Sync compile-time assertions for ProcedureResult and ProcedureBuilder\n- Visibility: send_rpc() and read_procedure_result() promoted to pub(crate)\n\nPart of stored procedure support (Steps 4c-4f).\n\n* docs(stored-procs): add tests, docs, and CHANGELOG entry\n\n- Integration tests: 11 tests covering simple calls, input params,\n  return values, multiple result sets, rows_affected, OUTPUT params\n  (int, nvarchar), builder with result sets and output, transactions,\n  error handling, and schema-qualified names\n- Unit tests: ProcedureResult defaults, get_output case-insensitive\n  and @-prefix stripping, result sets with return value\n- ARCHITECTURE.md: add section 4.7 covering RPC flow, token handling,\n  ReturnValue decoding, and security\n- docs/STORED_PROCEDURES.md: user guide with quick start, API reference,\n  transaction support, output types, security, and error handling\n- CHANGELOG.md: add stored procedure feature under [Unreleased], credit\n  @c5soft for PR #71's influence on API design\n\nPart of stored procedure support (Step 4g).\n\n* fix(docs): resolve broken ProcedureBuilder rustdoc link\n\nUse fully qualified path `crate::procedure::ProcedureBuilder` in the\ndoc comment for `Client::procedure()` so rustdoc can resolve it across\nmodule boundaries.\n\n* fix(client): add Clone derive to ProcedureResult and ResultSet\n\nProcedureResult was missing Clone, which the plan specified. ResultSet\nalso lacked Clone despite all its fields (Vec<Column>, VecDeque<Row>)\nbeing Clone. Both now derive Clone for consistency with OutputParam and\nExecuteResult.\n\n* feat(client): add SQL Browser instance resolution (#66)\n\nAdd automatic TCP port resolution for named SQL Server instances via\nthe SQL Server Browser service (SSRP protocol, MC-SQLR spec).\n\nWhen connecting with a named instance (e.g., Server=localhost\\SQLEXPRESS),\nthe driver now queries the Browser service on UDP 1434 to discover the\nTCP port before establishing the TCP connection. This is transparent to\nthe user — no API changes needed.\n\nNew module: crate::browser — implements CLNT_UCAST_INST request and\nSVR_RESP response parsing per the MC-SQLR specification. Handles:\n- \".\" as localhost (common for .\\SQLEXPRESS)\n- Timeout when Browser service is not running\n- Missing TCP port (instance may only support Named Pipes)\n- Malformed responses\n\nNew error variant: Error::BrowserResolution with instance name and\nreason for clear diagnostics.\n\nIntegration point: Client::try_connect() resolves the instance port\nbefore TCP connect when config.instance is Some.\n\nRequested by @tracker1 in #66.\n\nCloses #66\n\n* feat(auth): add native Windows SSPI for integrated authentication (#65)\n\nThe sspi-rs crate (pure Rust SSPI) cannot acquire credentials from the\ncurrent Windows logon session without explicit username/password. This\nmeans integrated auth (`Integrated Security=true`) was silently broken\non Windows for all account types — domain, local, and Microsoft\nAccounts.\n\nAdd a `NativeSspiAuth` provider that calls the actual Windows SSPI APIs\n(AcquireCredentialsHandleW / InitializeSecurityContextW from\nsecur32.dll) via the `windows` crate. The native SSPI subsystem has\ndirect access to LSASS cached credentials, supporting all account types\ntransparently — including Microsoft Accounts on Windows 11, which is\nthe specific scenario reported in #65.\n\nOn Windows with `sspi-auth` enabled, `Client::connect()` now uses\n`NativeSspiAuth` for integrated auth. On non-Windows, the existing\nsspi-rs path is preserved. `SspiAuth` (sspi-rs) remains available for\nexplicit credential scenarios on all platforms.\n\nCloses #65\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(auth): resolve windows-certstore compilation errors with windows 0.62 (#83)\n\nThe windows-certstore module had 9 compilation errors due to API\nchanges in the windows crate 0.62. This went undetected because the\nmodule is behind #[cfg(windows)] and CI runs on Linux.\n\nFixes applied:\n- BOOL: moved from Win32::Foundation to windows::core (now a newtype\n  struct in windows-result, re-exported through windows-core)\n- CRYPT_HASH_BLOB: replaced with CRYPT_INTEGER_BLOB (same layout,\n  renamed in windows 0.62)\n- CryptAcquireCertificatePrivateKey: params changed from &mut refs to\n  raw pointers with dedicated types (HCRYPTPROV_OR_NCRYPT_KEY_HANDLE,\n  CERT_KEY_SPEC, BOOL)\n- NCryptSignHash/NCryptVerifySignature: flags param type changed from\n  BCRYPT_FLAGS to NCRYPT_FLAGS (wrap via NCRYPT_FLAGS(BCRYPT_PAD_*.0))\n- CertCloseStore: first param now Option<HCERTSTORE>\n- NCryptFreeObject: use NCRYPT_HANDLE constructor instead of primitive\n  cast from NCRYPT_KEY_HANDLE\n- CertFreeCertificateContext: return type changed to must-use BOOL\n\nCloses #83\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* feat(client): add FILESTREAM BLOB access support (#67)\n\nAdd async read/write access to SQL Server FILESTREAM BLOBs via the\nWin32 OpenSqlFilestream API. This enables Rust applications to stream\nlarge binary data stored in SQL Server FILESTREAM columns using\ntokio-compatible async I/O.\n\nThe implementation uses runtime dynamic loading (LoadLibrary +\nGetProcAddress) to find OpenSqlFilestream in the OLE DB driver DLLs:\n  - msoledbsql19.dll (OLE DB Driver 19, newest)\n  - msoledbsql.dll (OLE DB Driver 18)\n  - sqlncli11.dll (SQL Server Native Client, deprecated fallback)\n\nThis avoids a compile-time dependency on any specific DLL and provides\na clear error when no FILESTREAM driver is installed.\n\nThe returned Win32 HANDLE is wrapped in tokio::fs::File for\nAsyncRead + AsyncWrite support.\n\nAPI:\n  - FileStream::open(path, access, txn_context) — low-level open\n  - Client<InTransaction>::open_filestream(path, access) — convenience\n    method that automatically fetches the transaction context\n  - FileStreamAccess enum: Read, Write, ReadWrite\n\nGated behind `#[cfg(all(windows, feature = \"filestream\"))]`. The\n`filestream` feature enables `tokio/fs` for the async file wrapper.\n\nCloses #67\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* test(client): add integration tests for SSPI and FILESTREAM (#65, #67)\n\nAdd end-to-end integration tests that verify both Windows-only features\nagainst a real SQL Server instance:\n\nSSPI tests (windows_sspi.rs):\n- Connect with integrated auth, verify NTLM/Kerberos auth_scheme\n- Execute query and transaction within SSPI session\n- Verify SQL Server version string\n\nFILESTREAM tests (windows_filestream.rs):\n- Read a FILESTREAM BLOB and verify content matches\n- Write to a FILESTREAM BLOB and read back to verify\n- Verify DLL loading produces OpenSqlFilestream error (not DLL error)\n\nAlso improves FILESTREAM error messages: OpenSqlFilestream failures now\ninclude the human-readable Win32 error message via FormatMessageW\ninstead of just the numeric error code.\n\nAll tests are gated with #[ignore] and require a Windows machine with\nSQL Server configured for Windows Authentication and FILESTREAM.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* docs(filestream): add documentation, CHANGELOG, and API improvements (#67)\n\nComplete the remaining work items from #67:\n\nDocumentation:\n- Add docs/FILESTREAM.md with full setup guide, usage examples,\n  troubleshooting, and API reference\n- Add CHANGELOG entry under [Unreleased] for FILESTREAM and\n  windows-certstore fix\n- Add filestream to README feature status and feature flags tables\n- Add filestream section to docs/FEATURES.md\n- Add FILESTREAM section to CLAUDE.md\n\nAPI improvements:\n- Add open_options module (NONE, SEQUENTIAL_SCAN, ASYNC) exposing\n  Win32 open flags for advanced users\n- Add FileStream::open_with_options() for custom open flags\n- Add #[non_exhaustive] to FileStreamAccess enum\n- Re-export open_options as filestream_options from lib.rs\n- Document async I/O strategy and future IOCP optimization path\n\nCode fixes:\n- Fix module doc: secur32.dll -> OLE DB Driver (was incorrect)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* docs: document Windows C++ build tools requirement\n\nThe default TLS feature depends on ring/aws-lc-sys which require a C\ncompiler. This is standard for Rust projects using rustls, but not\nobvious to developers on a fresh Windows machine without Visual Studio.\n\n- Add Windows C++ Build Tools section to CONTRIBUTING.md prerequisites\n  and step-by-step setup\n- Add installation note to README.md pointing to the setup docs\n- Note that sspi-auth and filestream features work without C++ tools\n  when TLS is disabled\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: fix formatting in Windows-specific modules\n\ncargo fmt on the test files and modules authored on Windows,\nwhich had minor formatting differences from the CI linter.\n\n* docs: update LIMITATIONS.md for v0.8.0+ features\n\nUpdate \"Last updated\" date from January 2026 to April 2026.\nCorrect the \"Cross-Platform\" design principle to reflect that\nWindows-specific features (SSPI, FILESTREAM, CertStore) are now\nsupported behind feature flags, not excluded.\n\n* refactor: replace unwrap() with expect() in library code\n\nSystematic audit of all unwrap() calls in non-test library code.\nReplaced with expect() containing context strings that document\nthe invariant being relied upon.\n\nChanges:\n- column_parser.rs: 16 chrono date/time construction unwraps →\n  expect() with context (e.g., \"epoch 1900-01-01 is valid\",\n  \"SMALLDATETIME minutes should be 0-1439\")\n- params.rs: 3 chrono epoch date unwraps → expect()\n- mock_server.rs: 1 TLS acceptor unwrap → expect()\n\nRemaining unwrap() calls (3) are in mssql-derive proc macros\nwhere clippy::expect_used is denied at the crate level. These\nare field.ident.as_ref().unwrap() on named struct fields, which\nis a guaranteed invariant in proc macro context.\n\nAddresses the code quality concern from the external project review.\n\n* test(protocol,auth): add unit tests for safety-critical token and auth paths\n\ntds-protocol (18 new tests):\n- ReturnValue: INT output, NVARCHAR output, NULL, UDF status (0x02)\n- ReturnStatus: success (0), negative return codes\n- DoneProc: roundtrip, parser, error flag\n- DoneInProc: roundtrip, parser with MORE|COUNT flags\n- ServerError: realistic decode, severity helpers, parser integration\n- Multi-token streams: stored proc response, error mid-stream,\n  ReturnValue->ReturnStatus->DoneProc sequence\n- EOF/truncation edge cases for ReturnStatus, DoneProc, ServerError\n\nmssql-auth (16 new tests):\n- AuthError: transient/terminal classification, mutual exclusion,\n  ambiguous error classification, Display output, Send+Sync bounds\n- AuthMethod: all-variants classification, Certificate variant\n- AuthData: SqlServer, FedAuth, Sspi, None variants, Debug output\n- AuthProvider trait: default method coverage via mock provider\n\n* refactor: audit panic-family macros in library code\n\nSystematic audit of all unreachable!() calls in non-test library code\n(5 total). No panic!, todo!, or unimplemented! macros were found.\n\nConverted to error (1):\n- params.rs: TvpColumnType wildcard arm now returns\n  TypeError::UnsupportedConversion instead of panicking. TvpColumnType\n  is #[non_exhaustive], so a new upstream variant would previously crash;\n  it now returns a descriptive error. Changed convert_tvp_column_type to\n  return Result<TvpWireType> with fallible collect at the call site.\n\nDocumented as justified (4):\n- column_parser.rs: inner match on Money|Money4|MoneyN is bounded by\n  the outer match arm — genuinely unreachable.\n- params.rs: inner match on chrono types is bounded by the outer match\n  arm — genuinely unreachable.\n- tvp.rs (2): encode_tvp_int and encode_tvp_float size parameters are\n  always derived from TvpWireType enum variants (1/2/4/8 and 4/8).\n  Added # Panics doc sections and descriptive messages.\n\n* feat(client): wire Always Encrypted decryption into query execution\n\nIntegrate the existing Always Encrypted cryptographic infrastructure\ninto the query execution flow, enabling transparent decryption of\nencrypted column values when Column Encryption Setting=Enabled.\n\nProtocol layer (tds-protocol):\n- Extended CryptoMetadata with base_user_type, base_col_type, and\n  base_type_info fields per MS-TDS 2.2.7.4\n- Added ColMetaData.cek_table and ColumnData.crypto_metadata fields\n- Added decode_encrypted() and decode_column_encrypted() for parsing\n  encrypted column metadata from the wire\n- Extracted decode_type_info/decode_collation as pub(crate) for reuse\n\nClient layer (mssql-client):\n- New column_decryptor module: async-to-sync bridge that pre-resolves\n  CEK encryptors at ColMetaData time, then decrypts synchronously per row\n- Updated read_query_response() with full decryption support\n- Added convert_raw_row_decrypted() and decrypt_column() to column_parser\n- User-facing column metadata shows base type, not wire type (BigVarBinary)\n- Connection string: Column Encryption Setting=Enabled parsing\n- Login7: COLUMNENCRYPTION feature extension (0x04) sent when enabled\n- EncryptionContext stored on Client struct\n\nSecurity:\n- Key material never logged in error messages\n- AEAD decryption verifies HMAC (constant-time) before decrypting\n- Corrupted/tampered ciphertext returns Err, never garbled data\n\nStill deferred:\n- Parameter encryption (requires sp_describe_parameter_encryption)\n- Procedure/multi-result decryption wiring (follow-up using same pattern)\n- Enclave computations (COLUMNENCRYPTION_VERSION 2/3)\n\n* fix(client): wire Always Encrypted decryption into procedure and multi-result readers\n\nread_procedure_result() and read_multi_result_response() were missing\nthe decryption branch that read_query_response() already had. This meant\nencrypted columns returned raw ciphertext (silent data corruption) when\naccessed via call_procedure() or query_multiple().\n\nApplied the same pattern from read_query_response(): pre-resolve a\nColumnDecryptor at ColMetaData time, then branch on it for each\nRow/NbcRow token to call convert_raw_row_decrypted() or the plain\nvariant. All three response readers now have symmetric decryption\nsupport.\n\n* fix(client): address silent error swallowing in stream and bulk modules\n\nstream.rs: Replace filter_map(|r| r.ok()) in test with explicit unwrap()\nso test failures surface instead of being silently dropped. Add comment\nto collect_current() clarifying that unwrap_or_default() is on Option,\nnot Result — no errors are being swallowed.\n\nbulk.rs: Document that .parse().ok() chains in parse_sql_type() are\nintentional best-effort parsing. When a type parameter like \"VARCHAR(100)\"\nhas a malformed length, falling through to the SQL Server default (8000)\nis safer than rejecting the operation when the base type is valid.\n\n* docs: update CHANGELOG, STABILITY, CLAUDE, README for v0.9.0 release prep\n\nCHANGELOG: Add missing Unreleased items — Always Encrypted decryption\nintegration, unwrap/panic audits, 34 new unit tests, LIMITATIONS.md\nupdate, procedure/multi-result decryption fix, silent error swallowing\nfix.\n\nSTABILITY: Add FILESTREAM, Always Encrypted, and KeyStoreProvider APIs\nto the stable surface table. Add filestream and encoding feature flags\nto the feature stability table.\n\nCLAUDE.md: Document the Always Encrypted decryption wiring — the\ncolumn_decryptor module, the CEK pre-resolution pattern, and how\ndecryption flows through all three response readers.\n\nREADME: Update always-encrypted feature description to mention\ntransparent column decryption.\n\nNew: docs/ALWAYS_ENCRYPTED.md user guide covering quick start, key store\nproviders, connection string keywords, encryption types, and security\nconsiderations.\n\n* fix(client): normalize (local) host alias to 127.0.0.1 for named instances\n\nServer=(local)\\SQLEXPRESS is a standard ADO.NET connection string format.\nPreviously only \".\" was normalized to localhost — \"(local)\" was passed\nthrough as-is, causing DNS resolution failure.\n\nNow both \".\" and \"(local)\" are normalized to 127.0.0.1 in both the\nSQL Browser resolver and the TCP connect path.\n\nReported by @tracker1 in #66.\n\n* feat(client): ADO.NET connection string conformance and LOGIN7 wire fixes\n\nRewrite the connection string parser to conform to the Microsoft\nADO.NET SqlConnection.ConnectionString specification:\n\nParser correctness:\n- Quoted value support: Password=\"my;pass\" and Password='it''s complex'\n  now work per spec. Previously semicolons in passwords were silently\n  truncated, causing misleading login failures.\n- tcp: prefix stripping: Server=tcp:host.database.windows.net,1433\n  (Azure Portal format) now works. np: and lpc: prefixes return errors.\n- Boolean validation: invalid values like TrustServerCertificate=banana\n  now error instead of silently defaulting to false.\n\nNew keywords (with LOGIN7 wire integration):\n- ApplicationIntent (ReadOnly/ReadWrite) wired to READONLY_INTENT bit\n- Workstation ID / WSID wired to LOGIN7 HostName field\n- Current Language / Language wired to LOGIN7 Language field\n- ConnectRetryCount/Interval wired to RetryPolicy\n- Server aliases: Addr, Address, Network Address\n- Timeout alias for Connect Timeout\n\nKnown keyword handling:\n- Pool keywords (Max Pool Size, etc.) logged at info with guidance\n- 30+ ADO.NET keywords recognized at info instead of silent debug\n\nLOGIN7 bug fix:\n- HostName field now sends actual client machine name instead of\n  server hostname. The MS-TDS spec defines this as \"the name of\n  the client machine\" — we were sending config.host (the server).\n\n* docs: document connection string conformance improvements\n\nUpdate CONNECTION_STRINGS.md:\n- Add quoted value syntax section with examples\n- Add tcp: prefix and unsupported protocol documentation\n- Add Addr, Address, Network Address as Server aliases\n- Add Timeout alias for Connect Timeout\n- Add ApplicationIntent, Workstation ID, Language keywords\n- Add ConnectRetryCount/Interval section\n- Add \"Recognized but Not Supported\" table for known ADO.NET keywords\n\nUpdate CHANGELOG.md with full feature list and LOGIN7 HostName fix.\n\n* fix(client): address remaining connection string conformance gaps\n\nFive issues identified during post-implementation review:\n\n1. Protocol prefix case sensitivity: tcp:/np:/lpc: checks now fully\n   case-insensitive via lowercase comparison. Previously only exact\n   \"tcp:\" and \"TCP:\" were handled — \"Tcp:\" would not be stripped.\n\n2. CLAUDE.md updated with connection string parser documentation per\n   convention #6 (\"Update CLAUDE.md when you add new infrastructure\").\n\n3. STABILITY.md: ApplicationIntent, Config::workstation_id(), and\n   Config::language() added to the stable API surface table.\n\n4. Empty value handling: Database=; now stores None instead of\n   Some(\"\"), matching ADO.NET reset-to-default semantics. Applied\n   to all Option<String> fields (database, instance, workstation_id,\n   language) via new non_empty() helper.\n\n5. Encrypt=Mandatory and Encrypt=Optional now recognized as aliases\n   for true/false, matching Microsoft.Data.SqlClient v5+ behavior.\n\n* docs: cross-reference ALWAYS_ENCRYPTED.md, update FEATURES.md and milestones\n\n- CLAUDE.md Document References: add CONNECTION_STRINGS.md,\n  ALWAYS_ENCRYPTED.md, and STORED_PROCEDURES.md\n- PRODUCTION_READINESS.md: add v0.9.0 milestone entry\n- docs/FEATURES.md: update always-encrypted description to mention\n  transparent column decryption and link to the user guide\n\n* fix(deps): resolve RUSTSEC-2026-0098 and RUSTSEC-2026-0099 (rustls-webpki)\n\ncargo update -p rustls-webpki: 0.103.10 → 0.103.12\n\n- RUSTSEC-2026-0098: Name constraints for URI names were incorrectly accepted\n- RUSTSEC-2026-0099: Name constraints accepted for wildcard certificates\n\nBoth are X.509 name constraint validation bugs. While exploitation requires\ncertificate misissuance (low practical risk), they block release preflight.\n\n* fix(bulk): correct SMALLDATETIME type ID and remove phantom max_errors field\n\nSMALLDATETIME was mapped to 0x3F (Numeric) instead of 0x3A (DateTime4)\nin parse_sql_type, causing incorrect COLMETADATA on the wire during bulk\ninsert. Also fixed the matching reference in encode_column_metadata.\n\nRemoved BulkOptions::max_errors which was declared but never emitted in\nthe INSERT BULK SQL statement — a phantom config field with no effect.\n\nAdded regression tests for SMALLDATETIME and DATETIME type ID mapping.\n\n* fix(types): add TIME, DATETIME2, DATETIMEOFFSET to SQL_VARIANT parser\n\nparse_sql_variant() was missing match arms for base types 0x29 (TIME),\n0x2A (DATETIME2), and 0x2B (DATETIMEOFFSET). These silently fell through\nto the default arm and returned raw binary bytes instead of typed values.\n\nEach new arm correctly handles scale property bytes, time interval\ncomputation via time_bytes_for_scale/intervals_to_time, and date epoch\noffset. Chrono feature-gated with Null fallback when chrono is disabled.\n\n* feat(api): make Row::from_values public for testing ergonomics\n\nUsers need to construct Row objects for unit testing their FromRow\nimplementations and application logic. Previously pub(crate), now pub.\nAddresses the same pain point as Tiberius #383.\n\n* fix(derive): infer SQL type from inner T in Option<T> for TVP fields\n\nThe Tvp derive macro mapped all Option<T> fields to NVARCHAR(MAX)\nregardless of T. Now recursively unwraps Option and infers the SQL type\nfrom the inner type, so Option<i32> correctly maps to INT.\n\n* test: add decimal round-trip, pre-1900 date, and proptest coverage\n\n- 7 targeted decimal round-trip tests verifying negative values (-17.80,\n  -0.01, -99999999.99), positive, zero, and boundary cases\n- Pre-1900 date encoding/decoding tests (1753-01-01, epoch, max date,\n  SMALLDATETIME at 1900 epoch)\n- 4 proptest property tests for decimal and date encode invariants,\n  exercising the previously-unused proptest dependency\n\n* chore(deps): remove unused proptest dev-dependency from tds-protocol\n\nproptest was declared as a dev-dep but never invoked in any tds-protocol\ntest. Removed the dep and updated the cargo-machete ignore list.\n\n* docs: add cancel safety, DDL, derive macros, and comparison guides\n\n- CANCEL_SAFETY.md: CancelHandle usage, unsafe drop path, pool safety\n  net, safe timeout pattern\n- DDL.md: automatic SQL batch routing, simple_query, multi-statement\n  batches, comparison with Tiberius\n- DERIVE_MACROS.md: FromRow, ToParams, Tvp attribute reference with\n  examples\n- COMPARISON.md: feature matrix vs Tiberius/odbc-api/sqlx-oldapi backed\n  by competitor issue numbers\n\n* fix(pool): detect in-flight requests to prevent dirty connection reuse\n\nAdd an `in_flight` flag to `Client<S>` that tracks whether a request has\nbeen sent but the response has not yet been fully read from the wire.\nThe flag is set before sending any SQL batch or RPC, and cleared after\n`read_message()` succeeds in each response reader.\n\n`PooledConnection::drop()` now checks `is_in_flight()` and discards the\nconnection instead of returning it to the pool. This prevents the\nprimary cancel-safety hazard: a `tokio::select!` or timeout dropping a\nquery future mid-flight, leaving unread response data in the TCP buffer.\n\n* fix(params): use native TDS binary encoding for date/time RPC parameters\n\nDate, Time, DateTime, and DateTimeOffset values were sent as NVARCHAR\nstrings via implicit SQL Server conversion. This lost sub-millisecond\nprecision, was sensitive to server culture settings, and prevented index\nseeks on temporal columns.\n\nNow uses mssql_types::encode functions to produce native TDS binary\nformat (DATE/0x28, TIME/0x29, DATETIME2/0x2A, DATETIMEOFFSET/0x2B)\nwith correct scale=7 metadata. Also adds RpcTypeInfo::time() and\nRpcTypeInfo::datetimeoffset() constructors, fixes DATE value encoding\nto include the required length prefix, and adds TIME/DATETIMEOFFSET\nto sp_executesql parameter declarations.\n\n* feat(client): wire ConnectRetryCount/Interval into connection retry loop\n\nThe retry policy (parsed from ConnectRetryCount/ConnectRetryInterval\nconnection string keywords) was stored but never used. Client::connect()\nnow retries transient connection errors (IO, timeout, Azure service\nbusy, etc.) using the configured RetryPolicy with exponential backoff.\n\nEach retry resets to the original host/port and re-enters the redirect\nloop. Non-transient errors (config, auth, syntax) are returned\nimmediately without retry. The overall timeout accounts for the\nadditional retry attempts.\n\n* feat(bulk): wire BulkInsert transport to Client API\n\nAdd Client::bulk_insert() method that sends the INSERT BULK statement,\ncreates a BulkWriter for streaming rows, then transmits accumulated\nBulkLoad (0x07) packets and reads the server response on finish().\n\n- BulkWriter<'a, S> holds &mut Client<S>, works in Ready and InTransaction\n- send_row() buffers rows synchronously; finish() does the async I/O\n- send_and_read_bulk_load() handles BulkLoad packet framing via Connection\n- Update COMPARISON.md bulk insert status from \"Partial\" to \"Yes\"\n\n* feat(client): add query_named/execute_named for ToParams bridge\n\nBridge the gap between ToParams/NamedParam and the Client API by\nadding query_named() and execute_named() methods that accept\n&[NamedParam] directly.\n\n- Refactor convert_single_param into sql_value_to_rpc_param (SqlValue)\n  + convert_single_param (ToSql wrapper)\n- Add convert_named_params for NamedParam → RpcParam with @ prefix\n- Both methods on impl<S: ConnectionState> for Ready + InTransaction\n- Fix to_params module doc to use execute_named instead of execute\n\n* docs(examples): update bulk_insert to use BulkWriter API\n\nReplace the old take_packets()/print pattern with the actual\nClient::bulk_insert() → BulkWriter → finish() flow. The example\nnow sends data to the server and verifies the row count.\n\n* feat(client): implement MultiSubnetFailover parallel TCP connect\n\nWhen MultiSubnetFailover=True, the driver resolves the server hostname\nto all IP addresses and races parallel TCP connections via JoinSet.\nFirst successful connection wins, remaining are cancelled. This reduces\nconnection time for AlwaysOn AG listeners spanning multiple subnets.\n\n- Add multi_subnet_failover field to Config with builder + conn string\n- Move MultiSubnetFailover from \"recognized but unsupported\" to parsed\n- Add connect_parallel() with JoinSet-based parallel TCP racing\n- Single-address optimization skips JoinSet overhead\n- Update CONNECTION_STRINGS.md and CLAUDE.md\n\n* test(fuzz): expand type_roundtrip to cover Decimal, UUID, and date/time types\n\nAdd 7 new FuzzSqlValue variants: Decimal, Uuid, Date, Time, DateTime,\nDateTimeOffset, Xml. Replace basic is_null/Debug-only testing with\nactual encode→decode roundtrip verification that frames encoded bytes\nwith TDS length prefixes and checks type discriminant preservation.\n\nCovers the #1 source of encoding bugs across TDS drivers (Decimal) and\nthe temporal types previously missing from fuzz coverage.\n\n* feat(client): add SendStringParametersAsUnicode config option\n\nWhen set to false, string parameters are sent as VARCHAR (0xA7) with\nWindows-1252 encoding instead of NVARCHAR (0xE7) with UTF-16LE. This\nallows SQL Server to use index seeks on VARCHAR columns, which are\nblocked by the implicit NVARCHAR→VARCHAR conversion that occurs with\nUnicode parameters.\n\n- Add Config::send_string_parameters_as_unicode field (default: true)\n- Parse SendStringParametersAsUnicode connection string keyword\n- Add TypeInfo::varchar()/varchar_max() and RpcParam::varchar()\n- Thread send_unicode flag through parameter conversion pipeline\n- Uses encoding_rs::WINDOWS_1252 when encoding feature is enabled,\n  falls back to Latin-1 otherwise\n\n* fix(client): encode Decimal params as native TDS type instead of NVARCHAR\n\nSqlValue::Decimal was being converted to a string via d.to_string() and\nsent as NVARCHAR, which loses precision with locale-dependent formatting,\nprevents index seeks on DECIMAL/NUMERIC columns, and risks rejection on\nculturally-sensitive SQL Server instances.\n\nNow uses mssql_types::encode::encode_decimal() for native binary encoding\nwith DECIMAL(38, scale) type info, matching the wire format used for all\nother numeric types.\n\n* fix(fuzz): update broken fuzz targets to match current tds-protocol API\n\nparse_rpc.rs, collation_decode.rs, and parse_login7.rs all used APIs\nthat no longer exist (ProcId::Name/Id, Collation::decode/flags/\ndecode_varchar, Login7Response). Rewrote to exercise current API:\n\n- parse_rpc: builds RpcRequests with named/sp_executesql paths,\n  encodes with arbitrary int/bigint/nvarchar/varchar params\n- collation_decode: constructs Collation structs and tests encoding\n  lookup, UTF-8 detection, code page, and encoding name\n- parse_login7: builds Login7 requests with arbitrary auth/config\n  fields and encodes them\n\n* fix(client): use server collation for VARCHAR params instead of hardcoded Latin1\n\nWhen SendStringParametersAsUnicode=false, VARCHAR parameters were always\nencoded with Windows-1252 and Latin1_General_CI_AS collation bytes,\nregardless of the server's actual default collation. This could silently\ncorrupt extended characters (0x80-0xFF) on servers with non-Latin\ncollations (e.g., Chinese_PRC_CI_AS, Cyrillic).\n\nNow the driver captures the server's default collation from the\nSqlCollation ENVCHANGE token during login and uses it to:\n- Select the correct character encoding via Collation::encoding()\n- Emit the correct collation bytes in the TypeInfo wire format\n- Handle UTF-8 collations (SQL Server 2019+) by using string bytes directly\n\nFalls back to the previous Latin1_General_CI_AS behavior when the server\ndoes not send a collation ENVCHANGE (backward compatible).\n\n* fix(example): rewrite derive_macros to use actual derive macros\n\nThe example previously used manual `impl ToParams` and `impl Tvp` blocks\nwith comments like \"With the derive macro, you would write...\". Now it\nuses the actual `#[derive(FromRow)]`, `#[derive(ToParams)]`, and\n`#[derive(Tvp)]` macros from mssql-derive.\n\nThe FromRow demo constructs real Rows via Row::from_values() and calls\nFromRow::from_row(), including a NULL column test. ToParams and Tvp\ndemos exercise the derived methods directly.\n\n* test(derive): add trybuild compile-fail tests for derive macros\n\nAdd 6 compile-fail test cases verifying that FromRow, ToParams, and Tvp\nderive macros produce clear errors for invalid usage:\n\n- FromRow/ToParams/Tvp on enums (must be structs)\n- FromRow/Tvp on tuple structs (must have named fields)\n- Tvp without required #[mssql(type_name = \"...\")] attribute\n\nUses trybuild (new workspace dev-dependency) with pinned .stderr\nsnapshots for error message stability.\n\n* feat(pool): add test_while_idle for background idle connection health checking\n\nThe reaper task now optionally health-checks idle connections during each\ntick when test_while_idle is enabled (default: false). Connections are\npopped one at a time, pinged with the configured health check query, and\ndiscarded if they fail or timeout (10s). This catches stale connections\nfrom firewall timeouts and Azure idle drops before checkout.\n\nUses the existing ConnectionMetadata::needs_health_check() infrastructure\nto avoid re-checking connections within the same health_check_interval.\n\n* feat(client): add in_params() helper for IN clause parameter generation\n\nGenerates SQL fragments like (@p1, @p2, @p3) with configurable start\nindex for composing IN clauses with other positional parameters. Exported\nfrom mssql_client::in_params for ergonomic access.\n\n* test(bulk): add integration tests and fix BulkLoad protocol bugs\n\nAdd 15 integration tests for the BulkWriter API against live SQL Server:\n- Basic INT/NVARCHAR insert with round-trip verification\n- Zero rows, large batch (1000 rows), NULL handling\n- Multiple data types (INT, TINYINT, SMALLINT, BIGINT, BIT, DECIMAL,\n  FLOAT, NVARCHAR, DATE, VARBINARY, UNIQUEIDENTIFIER)\n- Bulk options (TABLOCK, CHECK_CONSTRAINTS, FIRE_TRIGGERS)\n- Schema-qualified table names, transactions (commit + rollback)\n- Trigger interaction (fire vs not-fire), connection reuse after bulk\n\nFix three BulkLoad protocol bugs discovered during testing:\n\n1. Identifier validator rejected # prefix for temp table names.\n   Regex now allows # and @ as first character.\n\n2. COLMETADATA encoding mismatched server expectations. Adopt the\n   Tiberius pattern: query server metadata via SELECT TOP 0, echo\n   the raw COLMETADATA bytes in the BulkLoad message.\n\n3. Row value encoding: fixed-length types (INT 0x38) must not include\n   a length prefix in row data, while nullable types (INTN 0x26) must.\n   Track is_fixed per column from server metadata.\n\nKnown gap: NVARCHAR(MAX) PLP encoding in BulkLoad is not yet working\n(test skipped with explanatory ignore message).\n\n* test(client): add trigger row count tests and document double-counting behavior\n\nAdd 3 integration tests investigating trigger row count accuracy (items 5.4/3.8):\n\n- test_trigger_insert_row_count: INSERT with AFTER INSERT trigger.\n  Confirms rows_affected=6 (3 user + 3 trigger) — double-counted.\n- test_trigger_update_row_count: UPDATE with AFTER UPDATE trigger.\n  Confirms rows_affected=4 (2 user + 2 trigger) — double-counted.\n- test_trigger_nocount_row_count: INSERT with SET NOCOUNT ON trigger.\n  Confirms rows_affected=2 (user only) — correct.\n\nInvestigation conclusion: Without SET NOCOUNT ON, SQL Server sends\nDoneInProc tokens with DONE_COUNT for trigger DML. The driver\naccumulates all Done/DoneProc/DoneInProc counts into rows_affected.\nThis matches ADO.NET and Tiberius behavior — it's a well-known SQL\nServer characteristic, not a driver bug. The standard mitigation is\nSET NOCOUNT ON in trigger bodies.\n\n* fix(types): correct UUID byte-order in GUID column decoding\n\ncolumn_parser.rs returned GUID bytes as SqlValue::Binary without\nconverting from SQL Server's mixed-endian wire format back to RFC 4122\nbig-endian. This caused UUID round-trips (write→read) to return a\ndifferent UUID with the first 3 groups byte-swapped.\n\nFix both the regular TypeId::Guid path and the SQL_VARIANT 0x24 path\nto swap bytes back to RFC format, returning SqlValue::Uuid when the\nuuid feature is enabled. Add uuid as an optional dependency of\nmssql-client (matching the chrono/decimal pattern).\n\nRestore exact UUID equality assertion in bulk_insert test and add a\nparameterized query round-trip test in protocol_conformance.\n\n* test(pool): add cancel safety integration test for in-flight detection\n\nValidates that dropping a query future mid-flight (via timeout) causes\nthe pool to discard the connection rather than returning it for reuse.\nThe test checks out a connection, starts a long WAITFOR query, cancels\nit via tokio::time::timeout, drops the connection, then checks out a\nnew one and verifies it returns correct data.\n\n* fix(bulk): emit fixed type IDs for NOT NULL columns in hand-crafted COLMETADATA\n\nSQL Server's BulkLoad protocol rejects nullable type IDs (0x26 INTN,\n0x68 BITN, etc.) for NOT NULL target columns with error 4816\n(\"Invalid column type from bcp client\"). The server itself uses\nfixed-width type IDs (0x38 Int4, 0x32 Bit, etc.) for NOT NULL columns\nin its own SELECT TOP 0 COLMETADATA output.\n\nFix write_colmetadata() to emit the fixed-width variant when a\nBulkColumn is marked not nullable, and update fixed_len tracking so\nrow values are encoded without the per-row length prefix. Added\nnullable_to_fixed_type() helper for the INTN/BITN/FLTN/MONEYN/DATETIMEN\n→ fixed-ID mapping.\n\nAdd Client::bulk_insert_without_schema_discovery() — a new public\nmethod that skips the SELECT TOP 0 round-trip and uses hand-crafted\nCOLMETADATA from the user-provided BulkColumn specs. Unblocks the\nstated goal in item 1.9: \"SELECT TOP 0 can become optional (schema\ndiscovery) rather than mandatory.\"\n\nTests: added unit test verifying NOT NULL columns use fixed TypeIds and\nhave no Nullable flag, plus integration test that exercises the\nend-to-end hand-crafted path against a live server with a mix of\nNOT NULL and NULL columns.\n\n* feat(pool): wire DatabaseMetrics OTel bridge into pool lifecycle\n\nBridges the pool's internal lifecycle events to the DatabaseMetrics OTel\ncollector defined in mssql-client::instrumentation. When the mssql-client\notel feature is enabled, the pool now emits:\n\n- Gauges (db.client.connections.usage/idle/max) on every create, close,\n  checkout, checkin, and reaper eviction via a record_pool_status helper\n- Counter (db.client.connections.create.total) on warm-up, get(), and\n  replacement connection creation after health-check failure\n- Counter (db.client.connections.close.total) on reaper lifetime/idle\n  expiration, health-check discards (checkout and idle-scan paths), and\n  PooledConnection::drop when in-flight or in-transaction is detected\n- Histogram (db.client.connections.wait_time) with acquisition elapsed\n  time on every successful checkout (including try_get)\n\nDatabaseMetrics is always present on PoolInner; it's a no-op struct when\nthe otel feature is off, so call sites don't need cfg-gating. A new\nmssql-driver-pool otel feature forwards to mssql-client/otel so pool\nconsumers don't need to re-declare the dependency.\n\nAdds PoolBuilder::pool_name(name) to set the db.client.pool.name label\nfor distinguishing multiple pools in the same process.\n\n* fix(bulk): correct MONEY and DATETIME wire encoding; add type-coverage tests\n\nTwo encoding bugs were found via integration tests against a live SQL\nServer and are fixed here.\n\nMONEY / SMALLMONEY (type_id 0x6E) were encoded as the DECIMAL wire\nformat (length-prefixed, sign byte, unsigned mantissa) but SQL Server\nexpects the scaled-integer format documented in MS-TDS §2.2.5.5.1.2:\n- SMALLMONEY (max_length 4): signed 32-bit LE, scaled by 10_000.\n- MONEY (max_length 8): signed 64-bit value written as high 32 bits LE\n  then low 32 bits LE, scaled by 10_000.\n\nThe Decimal encoding branch now dispatches on col.type_id: 0x6E routes\nto a new encode_money_value helper, everything else keeps the existing\nDECIMAL encoding. Excess precision past 4 decimal places is truncated\ntoward zero.\n\nDATETIME (type_id 0x6F max_length 8) and SMALLDATETIME (type_id 0x6F\nmax_length 4) were encoded using the DATETIME2 format (time-then-date\nwith scale) regardless of column type. DATETIMEN has its own legacy\nformat:\n- DATETIME: 4-byte days since 1900-01-01 (i32 LE) + 4-byte time in\n  1/300s units since midnight (u32 LE).\n- SMALLDATETIME: 2-byte days since 1900 (u16 LE, range 1900-2079) +\n  2-byte minutes since midnight (u16 LE, rounding 30s up).\n\nThe DateTime encoding branch now dispatches on col.type_id: 0x6F uses\nthe new encode_datetime_legacy / encode_smalldatetime helpers selected\nby max_length, DATETIME2 (0x2A) keeps the existing scale-aware format.\n\nSix integration tests added to exercise the fixes and related scenarios\nflagged as untested in the work-items audit:\n- test_bulk_insert_money_and_smallmoney — MONEY/SMALLMONEY round-trip\n  including SMALLMONEY range edges (±214748.3647/3648) and MONEY min\n  (-922337203685477.5808).\n- test_bulk_insert_smalldatetime — round-trip including epoch\n  (1900-01-01) and NULL.\n- test_bulk_insert_datetime_vs_datetime2_precision — DATETIME 3.33ms\n  rounding, DATETIME2(7) exact 100ns preservation, DATETIME2(3) ms.\n- test_bulk_insert_mixed_column_ordering — DATE/TIME/DATETIME2/\n  DATETIMEOFFSET interleaved with INT/BIT/NVARCHAR to catch offset\n  bugs in the column encoding loop.\n- test_bulk_insert_all_types_null — NULL across 20 types in one table\n  (BIT, all int widths, REAL, FLOAT, DECIMAL, MONEY, SMALLMONEY,\n  VARCHAR, NVARCHAR, VARBINARY, DATE, TIME, DATETIME, SMALLDATETIME,\n  DATETIME2, DATETIMEOFFSET, UUID).\n- test_bulk_insert_datetime_legacy_edge_cases — DATETIME at 1753-01-01\n  (type range min, negative days offset) plus NULL handling.\n\nKnown gaps that are NOT addressed here and remain tracked as work-item\n3.5 sub-tasks: NVARCHAR(MAX)/VARBINARY(MAX)/VARCHAR(MAX) PLP encoding\n(integration fails with \"premature end-of-message\"), and TEXT/NTEXT\n(0x23/0x63) which fall through to the NVARCHAR branch and receive the\nwrong length-prefix width.\n\n* test(pool): set min_connections(0) in status tracking test\n\nThe test asserts status.available == 0 at pool startup, but\nPoolConfig::default() uses min_connections: 1 which warms up one\nconnection during Pool::new. Explicitly set min_connections(0) so\nthe startup assertion holds.\n\n* fix(bulk): use PLP_UNKNOWN_LEN marker for NVARCHAR(MAX)/VARBINARY(MAX)\n\nSQL Server's BulkLoad (0x07) parser rejected concrete 8-byte ULONGLONGLEN\nvalues in the PLP row-data encoding with error 4804 (\"premature end-of-\nmessage\") even though MS-TDS 2.2.5.2.3 permits both forms. Emit the\nPLP_UNKNOWN_LEN marker (0xFFFFFFFFFFFFFFFE) instead — the server uses the\nchunk lengths and terminator to detect the end. Matches Tiberius behavior.\n\nAdded integration tests covering:\n- NVARCHAR(MAX): 4000-char string (multi-packet), short string, empty string\n- VARBINARY(MAX): 10,000-byte blob, small blob, empty blob, NULL\n\nUpdated the three unit tests that pinned the exact PLP byte layout.\n\n* fix(bulk): route VARCHAR columns through collation code page\n\nBulkColumn now carries an optional Collation populated from server\nCOLMETADATA during bulk_insert(). SqlValue::String encoding branches\non type_id so VARCHAR/CHAR/BIGCHAR columns transcode via the\ncollation's code page instead of always emitting UTF-16 (which landed\neach surrogate half in a separate single-byte slot and doubled\nDATALENGTH for ASCII).\n\nExtracted the shared str → single-byte transcoder into\ntds_protocol::collation::encode_str_for_collation so the RPC parameter\npath (RpcParam::varchar_with_collation) and the bulk insert path reuse\none implementation. VARCHAR(MAX) uses encode_plp_binary on the\ncodepage-encoded bytes, matching the existing framing.\n\nIntegration tests cover VARCHAR/CHAR/VARCHAR(MAX) ASCII round-trip\nwith DATALENGTH verification and Windows-1252 extended characters\n(café / grüße / naïve résumé) that were silently corrupted before.\n\n* fix(rpc): count UTF-16 code units for NVARCHAR param length metadata\n\nRpcParam::nvarchar used valu…",
          "timestamp": "2026-04-17T17:33:10-05:00",
          "tree_id": "70e32c87c8d48656700a853d60e3b905704a2a3f",
          "url": "https://github.com/praxiomlabs/rust-mssql-driver/commit/fcbc5a2c3e45091ddc0e044b3c52a6571c1c8ae4"
        },
        "date": 1776465971510,
        "tool": "cargo",
        "benches": [
          {
            "name": "connection_string/simple",
            "value": 865,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_port",
            "value": 916,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/with_instance",
            "value": 1130,
            "range": "± 63",
            "unit": "ns/iter"
          },
          {
            "name": "connection_string/azure_full",
            "value": 1790,
            "range": "± 24",
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
            "value": 87,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "config_builder/full",
            "value": 117,
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
            "value": 77,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/medium",
            "value": 468,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/long",
            "value": 2750,
            "range": "± 200",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_encode/unicode",
            "value": 237,
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
            "value": 149,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "utf16_decode/long",
            "value": 700,
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
            "value": 66,
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
            "value": 902,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sql_batch_encode/large",
            "value": 2441,
            "range": "± 143",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}