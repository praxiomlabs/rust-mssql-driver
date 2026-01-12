window.BENCHMARK_DATA = {
  "lastUpdate": 1768254521681,
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
      }
    ]
  }
}