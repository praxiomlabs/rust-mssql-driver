//! Compile-time public-API self-containment guard. See Cargo.toml.
//!
//! Every item below is named through `mssql_client` only. If any requires a
//! direct dependency on an internal crate, this crate fails to build.
#![allow(dead_code)]

use mssql_client::{
    AuthError, CertificateDer, CodecError, Collation, Config, Error, FromSql, KeyStoreProvider,
    ProtocolError, Row, SqlValue, TlsConfig, ToSql, TypeError,
};

// --- Derive macros (the actual self-containment break this guard exists for) ---
#[derive(mssql_client::FromRow)]
struct Loaded {
    id: i32,
    name: String,
}

#[derive(mssql_client::ToParams)]
struct Params {
    id: i32,
    name: String,
}

#[derive(mssql_client::Tvp)]
#[mssql(type_name = "dbo.MyType")]
struct TvpRow {
    a: i32,
    b: String,
}

// --- Error sub-types must be nameable (Decision A) ---
fn classify(e: &Error) -> &'static str {
    match e {
        Error::Type(_) => "type",
        Error::Authentication(_) => "auth",
        Error::ProtocolError(_) => "protocol",
        Error::Codec(_) => "codec",
        _ => "other",
    }
}

fn name_sub_errors(
    _: Option<TypeError>,
    _: Option<AuthError>,
    _: Option<ProtocolError>,
    _: Option<CodecError>,
) {
}

// --- TypeError is the FromSql/ToSql trait error: implementable without mssql-types ---
struct MyId(i32);
impl FromSql for MyId {
    fn from_sql(value: &SqlValue) -> Result<Self, TypeError> {
        Ok(MyId(i32::from_sql(value)?))
    }
}

// --- Additive re-exports (this session) must be nameable ---
fn touch_tls() -> TlsConfig {
    TlsConfig::new().add_root_certificate(CertificateDer::from(vec![0u8; 4]))
}

fn touch_collation(col: &mssql_client::Column) -> Option<Collation> {
    col.collation
}

fn touch_provider<P: KeyStoreProvider>(_: P) {}

fn touch_config() -> Config {
    Config::new()
}

fn use_to_sql<T: ToSql>(_: &T) {}
fn use_row(_: &Row) {}
