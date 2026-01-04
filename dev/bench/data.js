window.BENCHMARK_DATA = {
  "lastUpdate": 1767485620524,
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
      }
    ]
  }
}