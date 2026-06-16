//! Differential type matrix (#203): cross-check the driver's decode against
//! values the **server** computes from literals.
//!
//! For each supported type and its boundary values we let SQL Server produce
//! the authoritative wire encoding (`SELECT CAST(<literal> AS <type>)`), then
//! assert the driver decodes it to the expected Rust value. The server is the
//! oracle, so this catches the class of decode bugs (SortId mapping,
//! DATETIMEOFFSET-UTC, NUMERIC overflow) that feature-driven tests miss because
//! they only exercise the types a feature happens to use.
//!
//! Built incrementally, split by dimension so each slice lands and fails
//! legibly. Dimensions:
//!
//! - **core scalar boundaries** — bit / integer / float (min/max/zero/neg).
//! - **collation sweep** — VARCHAR across codepages, exercising both the LCID
//!   path (Windows collations, `sort_id == 0`, incl. DBCS) and the SortId path
//!   (SQL collations, `sort_id != 0`; the #158/#187 class).
//! - **NUMERIC precision/scale grid** — exact decode across the grid, plus the
//!   overflow boundary: values beyond `rust_decimal`'s range must error, not
//!   silently degrade (the #157/#188/#196 class).
//! - **temporal** — DATE / TIME / DATETIME2 / legacy DATETIME / SMALLDATETIME
//!   across scales and range boundaries, and DATETIMEOFFSET across offsets
//!   (instant + offset preserved; the DATETIMEOFFSET-UTC class).
//! - **encode-back** — the encode direction: bind a value as a parameter and
//!   compare the server's `CAST(@P1 AS VARBINARY)` to the same literal cast to
//!   the inferred type, so an encode bug shows as a byte mismatch.
//!
//! Run against a live server:
//! ```text
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!   cargo test -p mssql-client --test type_matrix --all-features -- --ignored
//! ```

#![allow(clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config, FromSql, Ready, ToSql};

/// Build test configuration from the environment, mirroring the other live
/// integration tests. Targets `master`; `CAST` needs no user database.
fn get_test_config() -> Config {
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("MSSQL_PORT").unwrap_or_else(|_| "1433".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "YourStrong@Passw0rd".into());

    let conn_str = format!(
        "Server={host},{port};Database=master;User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt=true"
    );
    Config::from_connection_string(&conn_str).expect("valid connection string")
}

// ---------------------------------------------------------------------------
// Dimension 1: core scalar boundaries
// ---------------------------------------------------------------------------

/// Run `SELECT CAST(<literal> AS <sql_type>)` and assert the driver decodes the
/// single returned cell to `expected`. The server computes the wire bytes from
/// the literal, so a mismatch is a driver decode bug.
async fn assert_cast_decode<T>(
    client: &mut Client<Ready>,
    sql_type: &str,
    literal: &str,
    expected: T,
) where
    T: FromSql + PartialEq + std::fmt::Debug,
{
    let sql = format!("SELECT CAST({literal} AS {sql_type})");
    let rows = client
        .query(&sql, &[])
        .await
        .unwrap_or_else(|e| panic!("query failed for {sql}: {e}"));
    let row = rows
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no row for {sql}"))
        .unwrap_or_else(|e| panic!("row error for {sql}: {e}"));
    let got: T = row
        .get(0)
        .unwrap_or_else(|e| panic!("decode failed for {sql}: {e}"));
    assert_eq!(
        got, expected,
        "decode mismatch: CAST({literal} AS {sql_type}) decoded to {got:?}, expected {expected:?}"
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn bit_boundaries() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    assert_cast_decode(&mut client, "BIT", "0", false).await;
    assert_cast_decode(&mut client, "BIT", "1", true).await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn tinyint_boundaries() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (literal, expected) in [("0", 0u8), ("1", 1u8), ("255", 255u8)] {
        assert_cast_decode(&mut client, "TINYINT", literal, expected).await;
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn smallint_boundaries() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (literal, expected) in [
        ("-32768", i16::MIN),
        ("-1", -1i16),
        ("0", 0i16),
        ("32767", i16::MAX),
    ] {
        assert_cast_decode(&mut client, "SMALLINT", literal, expected).await;
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn int_boundaries() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (literal, expected) in [
        ("-2147483648", i32::MIN),
        ("-1", -1i32),
        ("0", 0i32),
        ("2147483647", i32::MAX),
    ] {
        assert_cast_decode(&mut client, "INT", literal, expected).await;
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn bigint_boundaries() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (literal, expected) in [
        ("-9223372036854775808", i64::MIN),
        ("-1", -1i64),
        ("0", 0i64),
        ("9223372036854775807", i64::MAX),
    ] {
        assert_cast_decode(&mut client, "BIGINT", literal, expected).await;
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn real_values() {
    // Exactly binary-representable values to keep equality exact.
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (literal, expected) in [("0", 0.0f32), ("1.25", 1.25f32), ("-1.25", -1.25f32)] {
        assert_cast_decode(&mut client, "REAL", literal, expected).await;
    }
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn float_values() {
    // Exactly binary-representable values, incl. 2^53 (last exact integer in f64).
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (literal, expected) in [
        ("0", 0.0f64),
        ("1.5", 1.5f64),
        ("-1.5", -1.5f64),
        ("9007199254740992", 9_007_199_254_740_992.0f64),
    ] {
        assert_cast_decode(&mut client, "FLOAT", literal, expected).await;
    }
}

// ---------------------------------------------------------------------------
// Dimension 2: collation sweep (VARCHAR codepage decode)
// ---------------------------------------------------------------------------

/// Which collation-metadata path the entry is meant to exercise. SQL Server
/// sends collation either by Windows LCID (`sort_id == 0`) or by legacy SQL
/// SortId (`sort_id != 0`); the driver maps each to a codepage through a
/// different table, so the sweep must cover both — and assert which one it hit
/// so a server/driver change can't silently move an entry off the intended path.
#[cfg(feature = "encoding")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum CollPath {
    Lcid,
    SortId,
}

/// `CAST(N'<sample>' COLLATE <collation> AS VARCHAR(200))` makes the server
/// transcode the Unicode sample into the collation's single-byte codepage; the
/// driver must read the collation metadata and decode those bytes back to the
/// same string. Samples use codepage-distinctive characters, so a wrong-codepage
/// decode produces a different string (or `?`) and fails.
#[cfg(feature = "encoding")]
async fn assert_varchar_collation(
    client: &mut Client<Ready>,
    collation: &str,
    sample: &str,
    path: CollPath,
) {
    let sql = format!("SELECT CAST(N'{sample}' COLLATE {collation} AS VARCHAR(200))");
    let rows = client
        .query(&sql, &[])
        .await
        .unwrap_or_else(|e| panic!("query failed for {collation}: {e}"));
    let row = rows
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no row for {collation}"))
        .unwrap_or_else(|e| panic!("row error for {collation}: {e}"));

    let meta = row.columns()[0]
        .collation
        .unwrap_or_else(|| panic!("no collation metadata for {collation}"));
    match path {
        CollPath::Lcid => assert_eq!(
            meta.sort_id, 0,
            "{collation} expected an LCID-based collation but sort_id={}",
            meta.sort_id
        ),
        CollPath::SortId => assert_ne!(
            meta.sort_id, 0,
            "{collation} expected a SortId-based collation but sort_id==0 (lcid={})",
            meta.lcid
        ),
    }

    let got: String = row
        .get(0)
        .unwrap_or_else(|e| panic!("decode failed for {collation}: {e}"));
    assert_eq!(
        got, sample,
        "collation decode mismatch for {collation} ({path:?}): got {got:?}, expected {sample:?}"
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "encoding")]
async fn collation_sweep() {
    use CollPath::{Lcid, SortId};

    // (collation, codepage-distinctive sample, intended metadata path)
    let cases = [
        // SortId path — SQL collations carry a legacy sort_id (#158/#187 class).
        ("SQL_Latin1_General_CP1_CI_AS", "Café", SortId), // 1252
        ("SQL_Latin1_General_CP1251_CI_AS", "Привет", SortId), // 1251 Cyrillic
        ("SQL_Latin1_General_CP1253_CI_AS", "Ελλάδα", SortId), // 1253 Greek
        ("SQL_Latin1_General_CP1254_CI_AS", "ışık", SortId), // 1254 Turkish (ı, ş)
        ("SQL_Latin1_General_CP1255_CI_AS", "שלום", SortId), // 1255 Hebrew
        ("SQL_Latin1_General_CP1256_CI_AS", "مرحبا", SortId), // 1256 Arabic
        ("SQL_Croatian_CP1250_CI_AS", "čćđ", SortId),     // 1250 Central European
        ("SQL_Estonian_CP1257_CI_AS", "ūēī", SortId),     // 1257 Baltic
        // LCID path — Windows collations, sort_id == 0, including DBCS codepages.
        ("Latin1_General_CI_AS", "Café", Lcid),     // 1252
        ("Cyrillic_General_CI_AS", "Привет", Lcid), // 1251
        ("Greek_CI_AS", "Ελλάδα", Lcid),            // 1253
        ("Japanese_CI_AS", "あいう", Lcid),         // 932 DBCS
        ("Chinese_PRC_CI_AS", "中文", Lcid),        // 936 DBCS
        ("Korean_Wansung_CI_AS", "한국어", Lcid),   // 949 DBCS
    ];

    let mut client = Client::connect(get_test_config()).await.expect("connect");
    for (collation, sample, path) in cases {
        assert_varchar_collation(&mut client, collation, sample, path).await;
    }
}

// ---------------------------------------------------------------------------
// Dimension 3: NUMERIC / DECIMAL precision-scale grid
// ---------------------------------------------------------------------------

/// Parse a decimal literal for the expected side of a NUMERIC assertion.
#[cfg(feature = "decimal")]
fn dec(s: &str) -> rust_decimal::Decimal {
    s.parse()
        .unwrap_or_else(|e| panic!("bad decimal literal {s}: {e}"))
}

/// Representable NUMERIC values across the precision/scale grid decode exactly.
/// `dec(lit)` is the expected value parsed from the same literal the server
/// casts, so a mismatch is a mantissa/scale decode bug.
#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "decimal")]
async fn numeric_grid() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    let cases = [
        ("NUMERIC(1,0)", "0"),
        ("NUMERIC(1,0)", "9"),
        ("NUMERIC(1,0)", "-9"),
        ("NUMERIC(5,2)", "123.45"),
        ("NUMERIC(5,2)", "-123.45"),
        ("NUMERIC(5,2)", "0"),
        ("NUMERIC(10,4)", "123456.7890"),
        ("NUMERIC(10,4)", "-0.0001"),
        ("NUMERIC(18,0)", "123456789012345678"),
        ("NUMERIC(18,9)", "123456789.123456789"),
        ("NUMERIC(38,10)", "1234567890.1234567890"),
        // rust_decimal range boundaries
        ("NUMERIC(28,0)", "9999999999999999999999999999"), // NUMERIC(28) precision max (28 nines)
        ("NUMERIC(28,28)", "0.9999999999999999999999999999"), // max scale (28; the AE ceiling)
        // The actual magnitude ceiling: rust_decimal::Decimal::MAX = 2^96 - 1
        // (29 digits, ~7.9e28 — larger than 28 nines), which must decode exactly.
        // The just-past-MAX value is checked in `numeric_overflow_errors`.
        ("NUMERIC(29,0)", "79228162514264337593543950335"),
    ];
    for (ty, lit) in cases {
        assert_cast_decode(&mut client, ty, lit, dec(lit)).await;
    }
}

/// NUMERIC values beyond `rust_decimal`'s 96-bit / scale-28 range must surface
/// as an **error**, never a silently-degraded value. This is the
/// #157/#188/#196 bug class (silent NUMERIC truncation). The error may arise at
/// query, row, or column-decode time; what matters is that no wrong value is
/// produced.
#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "decimal")]
async fn numeric_overflow_errors() {
    let mut client = Client::connect(get_test_config()).await.expect("connect");
    let cases = [
        // Decimal::MAX + 1 (2^96): one past the magnitude ceiling, the exact boundary.
        ("NUMERIC(29,0)", "79228162514264337593543950336"),
        ("NUMERIC(29,0)", "99999999999999999999999999999"), // 29 nines, well past the mantissa
        ("NUMERIC(38,0)", "99999999999999999999999999999999999999"), // max NUMERIC magnitude
        ("NUMERIC(38,38)", "0.99999999999999999999999999999999999999"), // scale 38 > 28
    ];
    for (ty, lit) in cases {
        let sql = format!("SELECT CAST({lit} AS {ty})");
        let produced = match client.query(&sql, &[]).await {
            Err(_) => None,
            Ok(rows) => match rows.into_iter().next() {
                None => panic!("no row for {sql}"),
                Some(Err(_)) => None,
                Some(Ok(row)) => row.get::<rust_decimal::Decimal>(0).ok(),
            },
        };
        assert!(
            produced.is_none(),
            "{sql} must error on overflow, but silently decoded {produced:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Dimension 4: temporal types (DATE / TIME / DATETIME2 / DATETIMEOFFSET /
// legacy DATETIME / SMALLDATETIME), across scales and offsets.
// ---------------------------------------------------------------------------

/// DATETIMEOFFSET must round-trip both the instant **and** the original offset.
/// chrono's `DateTime` equality only compares the instant, so we compare the
/// RFC3339 rendering (instant + offset + fractional) — the check that would
/// catch the DATETIMEOFFSET-UTC bug class (wrong instant) *and* an offset that
/// got normalized away.
#[cfg(feature = "chrono")]
async fn assert_dto(
    client: &mut Client<Ready>,
    literal: &str,
    cast_type: &str,
    expected_rfc3339: &str,
) {
    use chrono::{DateTime, FixedOffset};
    let sql = format!("SELECT CAST('{literal}' AS {cast_type})");
    let rows = client
        .query(&sql, &[])
        .await
        .unwrap_or_else(|e| panic!("query failed for {sql}: {e}"));
    let row = rows
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no row for {sql}"))
        .unwrap_or_else(|e| panic!("row error for {sql}: {e}"));
    let got: DateTime<FixedOffset> = row
        .get(0)
        .unwrap_or_else(|e| panic!("decode failed for {sql}: {e}"));
    let expected = DateTime::parse_from_rfc3339(expected_rfc3339).expect("valid expected rfc3339");
    assert_eq!(
        got.to_rfc3339(),
        expected.to_rfc3339(),
        "DATETIMEOFFSET mismatch for {sql}: got {got}, expected {expected}"
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "chrono")]
async fn date_boundaries() {
    let d = |y, m, dd| chrono::NaiveDate::from_ymd_opt(y, m, dd).expect("valid date");
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    assert_cast_decode(&mut c, "DATE", "'0001-01-01'", d(1, 1, 1)).await;
    assert_cast_decode(&mut c, "DATE", "'9999-12-31'", d(9999, 12, 31)).await;
    assert_cast_decode(&mut c, "DATE", "'2000-02-29'", d(2000, 2, 29)).await;
    assert_cast_decode(&mut c, "DATE", "'2023-06-15'", d(2023, 6, 15)).await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "chrono")]
async fn time_scales() {
    let t = |h, m, s, n| chrono::NaiveTime::from_hms_nano_opt(h, m, s, n).expect("valid time");
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    assert_cast_decode(&mut c, "TIME(0)", "'00:00:00'", t(0, 0, 0, 0)).await;
    assert_cast_decode(&mut c, "TIME(0)", "'23:59:59'", t(23, 59, 59, 0)).await;
    assert_cast_decode(
        &mut c,
        "TIME(1)",
        "'12:34:56.1'",
        t(12, 34, 56, 100_000_000),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "TIME(3)",
        "'12:34:56.123'",
        t(12, 34, 56, 123_000_000),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "TIME(7)",
        "'12:34:56.1234567'",
        t(12, 34, 56, 123_456_700),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "TIME(7)",
        "'23:59:59.9999999'",
        t(23, 59, 59, 999_999_900),
    )
    .await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "chrono")]
async fn datetime2_scales() {
    let dt = |y, mo, d, h, mi, s, n| {
        chrono::NaiveDate::from_ymd_opt(y, mo, d)
            .expect("valid date")
            .and_hms_nano_opt(h, mi, s, n)
            .expect("valid time")
    };
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    assert_cast_decode(
        &mut c,
        "DATETIME2(0)",
        "'2023-06-15 14:30:00'",
        dt(2023, 6, 15, 14, 30, 0, 0),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "DATETIME2(7)",
        "'0001-01-01 00:00:00'",
        dt(1, 1, 1, 0, 0, 0, 0),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "DATETIME2(7)",
        "'9999-12-31 23:59:59.9999999'",
        dt(9999, 12, 31, 23, 59, 59, 999_999_900),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "DATETIME2(7)",
        "'2023-06-15 14:30:00.1234567'",
        dt(2023, 6, 15, 14, 30, 0, 123_456_700),
    )
    .await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "chrono")]
async fn legacy_datetime_smalldatetime() {
    // Whole-second / whole-minute values are exact in these low-resolution types
    // (DATETIME = 1/300s, SMALLDATETIME = 1 minute), avoiding rounding ambiguity.
    let dt = |y, mo, d, h, mi, s| {
        chrono::NaiveDate::from_ymd_opt(y, mo, d)
            .expect("valid date")
            .and_hms_opt(h, mi, s)
            .expect("valid time")
    };
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    // DATETIME range: 1753-01-01 .. 9999-12-31
    assert_cast_decode(
        &mut c,
        "DATETIME",
        "'2023-06-15 14:30:00'",
        dt(2023, 6, 15, 14, 30, 0),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "DATETIME",
        "'1753-01-01 00:00:00'",
        dt(1753, 1, 1, 0, 0, 0),
    )
    .await;
    // SMALLDATETIME range: 1900-01-01 .. 2079-06-06, minute resolution
    assert_cast_decode(
        &mut c,
        "SMALLDATETIME",
        "'2023-06-15 14:30:00'",
        dt(2023, 6, 15, 14, 30, 0),
    )
    .await;
    assert_cast_decode(
        &mut c,
        "SMALLDATETIME",
        "'1900-01-01 00:00:00'",
        dt(1900, 1, 1, 0, 0, 0),
    )
    .await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "chrono")]
async fn datetimeoffset_offsets() {
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    // Offset sweep at scale 7.
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00 +00:00",
        "DATETIMEOFFSET(7)",
        "2023-06-15T14:30:00+00:00",
    )
    .await;
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00 +05:30",
        "DATETIMEOFFSET(7)",
        "2023-06-15T14:30:00+05:30",
    )
    .await;
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00 -08:00",
        "DATETIMEOFFSET(7)",
        "2023-06-15T14:30:00-08:00",
    )
    .await;
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00 +14:00",
        "DATETIMEOFFSET(7)",
        "2023-06-15T14:30:00+14:00",
    )
    .await;
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00 -12:00",
        "DATETIMEOFFSET(7)",
        "2023-06-15T14:30:00-12:00",
    )
    .await;
    // Fractional + offset, and scale 0.
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00.1234567 +05:30",
        "DATETIMEOFFSET(7)",
        "2023-06-15T14:30:00.1234567+05:30",
    )
    .await;
    assert_dto(
        &mut c,
        "2023-06-15 14:30:00 -08:00",
        "DATETIMEOFFSET(0)",
        "2023-06-15T14:30:00-08:00",
    )
    .await;
}

// ---------------------------------------------------------------------------
// Dimension 5: encode-back (parameter encode vs server literal)
// ---------------------------------------------------------------------------

/// Differential **encode** check: bind `value` as the parameter `@P1`, and have
/// the server re-emit it as VARBINARY alongside the same literal cast to the
/// same type. If the driver encoded the parameter correctly, the server decodes
/// it to exactly the literal's value and the two VARBINARY blobs match. `sql_type`
/// must be the type the driver infers for `V` (see the encode-type probe), so the
/// two sides are the same SQL type.
async fn assert_encode_roundtrip<V>(
    client: &mut Client<Ready>,
    value: &V,
    literal: &str,
    sql_type: &str,
) where
    V: ToSql + Sync,
{
    let sql = format!(
        "SELECT CAST(@P1 AS VARBINARY(8000)), CAST(CAST({literal} AS {sql_type}) AS VARBINARY(8000))"
    );
    let rows = client
        .query(&sql, &[value])
        .await
        .unwrap_or_else(|e| panic!("query failed for {sql}: {e}"));
    let row = rows
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no row for {sql}"))
        .unwrap_or_else(|e| panic!("row error for {sql}: {e}"));
    let encoded: Vec<u8> = row
        .get(0)
        .unwrap_or_else(|e| panic!("decode @P1 bytes for {sql}: {e}"));
    let expected: Vec<u8> = row
        .get(1)
        .unwrap_or_else(|e| panic!("decode literal bytes for {sql}: {e}"));
    assert_eq!(
        encoded, expected,
        "encode mismatch for {sql_type}: driver-encoded @P1 {encoded:02X?} != literal {literal} {expected:02X?}"
    );
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn encode_back_scalars() {
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    // integers
    assert_encode_roundtrip(&mut c, &0i32, "0", "INT").await;
    assert_encode_roundtrip(&mut c, &(-1i32), "-1", "INT").await;
    assert_encode_roundtrip(&mut c, &i32::MAX, "2147483647", "INT").await;
    assert_encode_roundtrip(&mut c, &i32::MIN, "-2147483648", "INT").await;
    assert_encode_roundtrip(&mut c, &i64::MIN, "-9223372036854775808", "BIGINT").await;
    assert_encode_roundtrip(&mut c, &i64::MAX, "9223372036854775807", "BIGINT").await;
    assert_encode_roundtrip(&mut c, &i16::MIN, "-32768", "SMALLINT").await;
    assert_encode_roundtrip(&mut c, &i16::MAX, "32767", "SMALLINT").await;
    // bool
    assert_encode_roundtrip(&mut c, &true, "1", "BIT").await;
    assert_encode_roundtrip(&mut c, &false, "0", "BIT").await;
    // floats
    assert_encode_roundtrip(&mut c, &1.5f64, "1.5", "FLOAT").await;
    assert_encode_roundtrip(&mut c, &(-1.5f64), "-1.5", "FLOAT").await;
    assert_encode_roundtrip(&mut c, &1.25f32, "1.25", "REAL").await;
    // nvarchar (codepage-distinctive char so an encode bug shows)
    assert_encode_roundtrip(&mut c, &"héllo".to_string(), "N'héllo'", "NVARCHAR(4000)").await;
    assert_encode_roundtrip(&mut c, &String::new(), "N''", "NVARCHAR(4000)").await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "decimal")]
async fn encode_back_decimal() {
    // The driver declares DECIMAL(38, value.scale()); match the literal's scale.
    let d = |s: &str| s.parse::<rust_decimal::Decimal>().expect("decimal");
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    assert_encode_roundtrip(&mut c, &d("123.45"), "123.45", "DECIMAL(38,2)").await;
    assert_encode_roundtrip(&mut c, &d("-123.45"), "-123.45", "DECIMAL(38,2)").await;
    assert_encode_roundtrip(&mut c, &d("0"), "0", "DECIMAL(38,0)").await;
    assert_encode_roundtrip(
        &mut c,
        &d("9999999999999999999999999999"),
        "9999999999999999999999999999",
        "DECIMAL(38,0)",
    )
    .await;
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
#[cfg(feature = "uuid")]
async fn encode_back_uuid() {
    let g = "12345678-90ab-cdef-1234-567890abcdef"
        .parse::<uuid::Uuid>()
        .expect("uuid");
    let mut c = Client::connect(get_test_config()).await.expect("connect");
    assert_encode_roundtrip(
        &mut c,
        &g,
        "'12345678-90AB-CDEF-1234-567890ABCDEF'",
        "UNIQUEIDENTIFIER",
    )
    .await;
}
