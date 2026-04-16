//! RPC parameter round-trip integration tests.
//!
//! Exercises each `SqlValue` variant through `sp_executesql` by sending
//! `SELECT @p1` with a value and asserting the decoded result matches the
//! input exactly. The goal is to catch encoding bugs in
//! `sql_value_to_rpc_param` — the same class of bug the bulk insert
//! integration tests surfaced in the `BulkLoad` path.
//!
//! These tests are ignored by default. Run with:
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourPassword
//!
//! cargo test -p mssql-client --test rpc_roundtrip -- --ignored
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config};

fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host},{port};Database={database};User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt={encrypt}"
    );

    Config::from_connection_string(&conn_str).ok()
}

async fn connect() -> Client<mssql_client::Ready> {
    let config = get_test_config().expect("SQL Server config required");
    Client::connect(config).await.expect("Failed to connect")
}

// =============================================================================
// Integer Types
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_bool() {
    let mut client = connect().await;
    for &input in &[true, false] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("Expected one row").expect("row err");
        let got: bool = row.get(0).expect("get bool");
        assert_eq!(got, input, "bool round-trip mismatch");
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_tinyint() {
    let mut client = connect().await;
    for &input in &[0u8, 1, 127, 200, 255] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: u8 = row.get(0).expect("get u8");
        assert_eq!(got, input, "tinyint round-trip mismatch for {input}");
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_smallint() {
    let mut client = connect().await;
    for &input in &[i16::MIN, -1, 0, 1, i16::MAX] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: i16 = row.get(0).expect("get i16");
        assert_eq!(got, input, "smallint round-trip mismatch for {input}");
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_bigint() {
    let mut client = connect().await;
    for &input in &[i64::MIN, -1_000_000_000_000, -1, 0, 1, 1_000_000_000_000, i64::MAX] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: i64 = row.get(0).expect("get i64");
        assert_eq!(got, input, "bigint round-trip mismatch for {input}");
    }
}

// =============================================================================
// Floating-Point Types
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_f32() {
    let mut client = connect().await;
    for &input in &[
        0.0_f32,
        1.0,
        -1.0,
        std::f32::consts::PI,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
    ] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: f32 = row.get(0).expect("get f32");
        assert_eq!(got.to_bits(), input.to_bits(), "f32 round-trip mismatch for {input}");
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_f64() {
    let mut client = connect().await;
    for &input in &[
        0.0_f64,
        1.0,
        -1.0,
        std::f64::consts::PI,
        std::f64::consts::E,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
    ] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: f64 = row.get(0).expect("get f64");
        assert_eq!(got.to_bits(), input.to_bits(), "f64 round-trip mismatch for {input}");
    }
}

// =============================================================================
// String Types
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_string_ascii() {
    let mut client = connect().await;
    for input in ["", "a", "hello world", "!@#$%^&*()_+-=[]{}|;':\",./<>?"] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: String = row.get(0).expect("get string");
        assert_eq!(got, input, "string round-trip mismatch for {input:?}");
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_string_unicode_bmp() {
    let mut client = connect().await;
    // Multi-byte UTF-16 code units in the BMP (Basic Multilingual Plane)
    for input in ["世界", "Héllo wörld", "Привет мир", "مرحبا بالعالم", "🌍"] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: String = row.get(0).expect("get string");
        assert_eq!(got, input, "string unicode round-trip mismatch for {input:?}");
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_string_supplementary() {
    // Supplementary plane characters use surrogate pairs in UTF-16 — exercises
    // the fix from 1.12 (NVARCHAR length must count code units, not chars).
    let mut client = connect().await;
    for input in ["🌍", "👨‍👩‍👧‍👦", "𐐷", "Hello 🌍 World", "🚀🎉💯"] {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .expect("Query failed");
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: String = row.get(0).expect("get string");
        assert_eq!(got, input, "string supplementary round-trip mismatch for {input:?}");
    }
}

// =============================================================================
// Binary Types
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_binary() {
    let mut client = connect().await;

    // Exercise a range of sizes that span the default 4096-byte packet boundary,
    // the 8000-byte VARBINARY(n) limit, and the VARBINARY(MAX) / PLP path above it.
    for size in [
        0usize, 1, 6, 100, 1000, 4000, 4080, 4096, 5000, 7999, 8000, 8001, 16_000, 100_000,
    ] {
        let input: Vec<u8> = (0..=255).cycle().take(size).collect();
        let rows = client
            .query("SELECT @p1 AS v", &[&input.as_slice()])
            .await
            .unwrap_or_else(|e| panic!("Query failed for binary size {size}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: Vec<u8> = row.get(0).expect("get binary");
        assert_eq!(got.len(), size, "binary length mismatch at size {size}");
        assert_eq!(got, input, "binary content mismatch at size {size}");
    }
}

// =============================================================================
// Date / Time Types (chrono feature — default)
// =============================================================================

#[cfg(feature = "chrono")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_date() {
    use chrono::NaiveDate;

    let mut client = connect().await;
    let cases = [
        NaiveDate::from_ymd_opt(1, 1, 1).unwrap(),     // DATE epoch
        NaiveDate::from_ymd_opt(1753, 1, 1).unwrap(),  // DATETIME epoch edge
        NaiveDate::from_ymd_opt(1899, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
        NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(), // DATE max
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .unwrap_or_else(|e| panic!("Query failed for date {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: NaiveDate = row.get(0).expect("get date");
        assert_eq!(got, input, "date round-trip mismatch for {input}");
    }
}

#[cfg(feature = "chrono")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_time() {
    use chrono::NaiveTime;

    let mut client = connect().await;
    let cases = [
        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(12, 30, 45).unwrap(),
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_900).unwrap(), // 7-digit TIME max
        NaiveTime::from_hms_nano_opt(1, 2, 3, 456_789_100).unwrap(),
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .unwrap_or_else(|e| panic!("Query failed for time {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: NaiveTime = row.get(0).expect("get time");
        assert_eq!(got, input, "time round-trip mismatch for {input}");
    }
}

#[cfg(feature = "chrono")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_datetime() {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    let mut client = connect().await;
    let cases = [
        // 7-digit DATETIME2 precision
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
            NaiveTime::from_hms_nano_opt(12, 34, 56, 789_123_400).unwrap(),
        ),
        // Midnight
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        ),
        // Pre-1900
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(1800, 6, 15).unwrap(),
            NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
        ),
        // DATETIME2 max
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(),
            NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_900).unwrap(),
        ),
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .unwrap_or_else(|e| panic!("Query failed for datetime {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: NaiveDateTime = row.get(0).expect("get datetime");
        assert_eq!(got, input, "datetime round-trip mismatch for {input}");
    }
}

#[cfg(feature = "chrono")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_datetimeoffset() {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};

    let mut client = connect().await;
    let cases = [
        // UTC
        DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 4, 16)
                .unwrap()
                .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            FixedOffset::east_opt(0).unwrap(),
        ),
        // Positive offset
        DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 4, 16)
                .unwrap()
                .and_time(NaiveTime::from_hms_nano_opt(9, 30, 0, 123_456_700).unwrap()),
            FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap(), // +05:30 (India)
        ),
        // Negative offset
        DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2000, 1, 1)
                .unwrap()
                .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            FixedOffset::west_opt(8 * 3600).unwrap(), // -08:00 (Pacific)
        ),
        // Extreme offset (+14:00)
        DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2020, 6, 1)
                .unwrap()
                .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            FixedOffset::east_opt(14 * 3600).unwrap(),
        ),
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .unwrap_or_else(|e| panic!("Query failed for dto {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: DateTime<FixedOffset> = row.get(0).expect("get dto");
        assert_eq!(got, input, "datetimeoffset round-trip mismatch for {input}");
    }
}

// =============================================================================
// Decimal (decimal feature — default)
// =============================================================================

#[cfg(feature = "decimal")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_decimal() {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let mut client = connect().await;
    let cases = [
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("1").unwrap(),
        Decimal::from_str("-1").unwrap(),
        Decimal::from_str("0.01").unwrap(),
        Decimal::from_str("-0.01").unwrap(),
        Decimal::from_str("123.456").unwrap(),
        Decimal::from_str("-123.456").unwrap(),
        Decimal::from_str("-17.80").unwrap(), // Tiberius #368 regression
        Decimal::from_str("99999999999999999999999999.9999999999").unwrap(),
        Decimal::from_str("-99999999999999999999999999.9999999999").unwrap(),
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .unwrap_or_else(|e| panic!("Query failed for decimal {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: Decimal = row.get(0).expect("get decimal");
        assert_eq!(got, input, "decimal round-trip mismatch for {input}");
    }
}

// =============================================================================
// MONEY / SMALLMONEY / SMALLDATETIME (item 2.7 — unblocks 5.11)
// =============================================================================

#[cfg(feature = "decimal")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_money() {
    use mssql_client::Money;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let mut client = connect().await;

    // MONEY range: -922,337,203,685,477.5808 to +922,337,203,685,477.5807
    // Precision: 4 decimal places (scaled integer × 10_000).
    let cases = [
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("0.0001").unwrap(), // smallest positive unit
        Decimal::from_str("-0.0001").unwrap(),
        Decimal::from_str("1.00").unwrap(),
        Decimal::from_str("-1.00").unwrap(),
        Decimal::from_str("12345.6789").unwrap(),
        Decimal::from_str("-12345.6789").unwrap(),
        Decimal::from_str("922337203685477.5807").unwrap(), // MONEY max
        Decimal::from_str("-922337203685477.5808").unwrap(), // MONEY min
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&Money(input)])
            .await
            .unwrap_or_else(|e| panic!("Query failed for money {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: Decimal = row.get(0).expect("get decimal");
        assert_eq!(got, input, "money round-trip mismatch for {input}");
    }
}

#[cfg(feature = "decimal")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_smallmoney() {
    use mssql_client::SmallMoney;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let mut client = connect().await;

    // SMALLMONEY range: -214,748.3648 to +214,748.3647 (scaled integer × 10_000 → i32).
    let cases = [
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("0.0001").unwrap(),
        Decimal::from_str("-0.0001").unwrap(),
        Decimal::from_str("100.50").unwrap(),
        Decimal::from_str("-100.50").unwrap(),
        Decimal::from_str("214748.3647").unwrap(), // SMALLMONEY max
        Decimal::from_str("-214748.3648").unwrap(), // SMALLMONEY min
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&SmallMoney(input)])
            .await
            .unwrap_or_else(|e| panic!("Query failed for smallmoney {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: Decimal = row.get(0).expect("get decimal");
        assert_eq!(got, input, "smallmoney round-trip mismatch for {input}");
    }
}

#[cfg(feature = "decimal")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_money_truncation() {
    // MONEY has 4-decimal precision. Values with more decimals are truncated
    // toward zero during encoding. Verify the truncation behaviour is consistent.
    use mssql_client::Money;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let mut client = connect().await;
    // Send 0.12345 → MONEY truncates to 0.1234
    let input = Decimal::from_str("0.12345").unwrap();
    let expected = Decimal::from_str("0.1234").unwrap();
    let rows = client
        .query("SELECT @p1 AS v", &[&Money(input)])
        .await
        .expect("Query failed");
    let row = rows.into_iter().next().expect("row").expect("row err");
    let got: Decimal = row.get(0).expect("get decimal");
    assert_eq!(got, expected, "money 5-decimal should truncate to 4-decimal");
}

#[cfg(feature = "chrono")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_smalldatetime() {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use mssql_client::SmallDateTime;

    let mut client = connect().await;

    // SMALLDATETIME range: 1900-01-01 00:00 through 2079-06-06 23:59.
    // Precision: 1 minute (seconds are rounded to nearest minute on the wire).
    let cases = [
        // Epoch
        (
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
        ),
        // Minute precision — exact input, exact output.
        (
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
                NaiveTime::from_hms_opt(12, 34, 0).unwrap(),
            ),
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
                NaiveTime::from_hms_opt(12, 34, 0).unwrap(),
            ),
        ),
        // Seconds 0..29 round DOWN.
        (
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2020, 6, 15).unwrap(),
                NaiveTime::from_hms_opt(10, 20, 29).unwrap(),
            ),
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2020, 6, 15).unwrap(),
                NaiveTime::from_hms_opt(10, 20, 0).unwrap(),
            ),
        ),
        // Seconds ≥30 round UP.
        (
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2020, 6, 15).unwrap(),
                NaiveTime::from_hms_opt(10, 20, 30).unwrap(),
            ),
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2020, 6, 15).unwrap(),
                NaiveTime::from_hms_opt(10, 21, 0).unwrap(),
            ),
        ),
        // Near max — 2079-06-06 at 23:59 is the documented SMALLDATETIME upper bound.
        (
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2079, 6, 6).unwrap(),
                NaiveTime::from_hms_opt(23, 59, 0).unwrap(),
            ),
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2079, 6, 6).unwrap(),
                NaiveTime::from_hms_opt(23, 59, 0).unwrap(),
            ),
        ),
    ];

    for (input, expected) in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&SmallDateTime(input)])
            .await
            .unwrap_or_else(|e| panic!("Query failed for smalldatetime {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: NaiveDateTime = row.get(0).expect("get datetime");
        assert_eq!(got, expected, "smalldatetime round-trip for {input}");
    }
}

// =============================================================================
// UUID (uuid feature — default)
// =============================================================================

#[cfg(feature = "uuid")]
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_uuid() {
    use uuid::Uuid;

    let mut client = connect().await;
    let cases = [
        // Asymmetric UUID — catches byte-order bugs (cf. item 1.8)
        Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap(),
        Uuid::nil(),
        Uuid::max(),
        Uuid::new_v4(),
        Uuid::parse_str("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11").unwrap(),
    ];

    for input in cases {
        let rows = client
            .query("SELECT @p1 AS v", &[&input])
            .await
            .unwrap_or_else(|e| panic!("Query failed for uuid {input}: {e}"));
        let row = rows.into_iter().next().expect("row").expect("row err");
        let got: Uuid = row.get(0).expect("get uuid");
        assert_eq!(got, input, "uuid round-trip mismatch for {input}");
    }
}

// =============================================================================
// NULL
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_null_int() {
    let mut client = connect().await;
    let input: Option<i32> = None;
    let rows = client
        .query("SELECT @p1 AS v", &[&input])
        .await
        .expect("Query failed");
    let row = rows.into_iter().next().expect("row").expect("row err");
    let got: Option<i32> = row.get(0).expect("get option");
    assert!(got.is_none(), "NULL int round-trip: expected None, got {got:?}");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_rpc_roundtrip_null_string() {
    let mut client = connect().await;
    let input: Option<String> = None;
    let rows = client
        .query("SELECT @p1 AS v", &[&input])
        .await
        .expect("Query failed");
    let row = rows.into_iter().next().expect("row").expect("row err");
    let got: Option<String> = row.get(0).expect("get option");
    assert!(got.is_none(), "NULL string round-trip: expected None, got {got:?}");
}
