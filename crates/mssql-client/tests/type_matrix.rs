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
//! legibly. Dimensions covered so far:
//!
//! - **core scalar boundaries** — bit / integer / float (min/max/zero/neg).
//! - **collation sweep** — VARCHAR across codepages, exercising both the LCID
//!   path (Windows collations, `sort_id == 0`, incl. DBCS) and the SortId path
//!   (SQL collations, `sort_id != 0`; the #158/#187 class).
//!
//! Planned follow-on: NUMERIC precision/scale grid, temporal types (DATE / TIME
//! / DATETIME2 / DATETIMEOFFSET at every offset and scale), and an encode-back
//! assertion (parameter round-trip vs `CAST(... AS VARBINARY)`).
//!
//! Run against a live server:
//! ```text
//! MSSQL_HOST=localhost MSSQL_USER=sa MSSQL_PASSWORD='YourStrong@Passw0rd' \
//!   cargo test -p mssql-client --test type_matrix --all-features -- --ignored
//! ```

#![allow(clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config, FromSql, Ready};

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
