//! End-to-end Kerberos/GSSAPI validation for the `integrated-auth` feature.
//!
//! This test drives the real `IntegratedAuth::initialize()` path against a
//! live MIT Kerberos KDC. It builds the service principal name with the
//! default constructor (`MSSQLSvc/<host>:<port>`) exactly as the connection
//! path does (`mssql-client` `connect.rs` calls `IntegratedAuth::new(host,
//! port)`), asks the KDC for a service ticket, and produces the initial
//! SPNEGO token.
//!
//! It is the only test that can prove the GSSAPI *name-type* is correct: a
//! name-type mismatch is invisible offline because `gss_import_name` accepts
//! any bytes — only the KDC rejects a wrongly-typed principal when a ticket
//! is requested. A `GSS_NT_HOSTBASED_SERVICE` name-type applied to the
//! slash/port principal `MSSQLSvc/<host>:<port>` was the root cause of the
//! integrated-auth path never authenticating; this test goes red on that bug
//! and green once the krb5 principal name-type is used.
//!
//! Requires infrastructure CI does not provide (an MIT KDC with the
//! `MSSQLSvc/<host>:<port>` SPN registered and a `kinit`'d ticket cache), so
//! it is `#[ignore]`d and excluded from the CI integration job, mirroring the
//! Azure/cert live tests.
//!
//! Run it (with a KDC configured and a ticket cached):
//!
//! ```bash
//! MSSQL_KERBEROS_HOST=localhost MSSQL_KERBEROS_PORT=1433 \
//!   KRB5CCNAME=/tmp/krb5cc_kdctest \
//!   cargo nextest run -p mssql-auth --features integrated-auth \
//!   --run-ignored ignored-only -E 'test(kerberos)'
//! ```
#![cfg(feature = "integrated-auth")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_auth::IntegratedAuth;

/// The host whose `MSSQLSvc/<host>:<port>` SPN is registered in the KDC.
///
/// The test fails loudly (rather than silently passing) when run without the
/// required configuration: an `#[ignore]`d test that is explicitly invoked but
/// lacks its infrastructure must fail, never report green.
fn kerberos_host() -> String {
    std::env::var("MSSQL_KERBEROS_HOST").expect(
        "MSSQL_KERBEROS_HOST must be set to run the Kerberos live test \
         (point it at the host whose MSSQLSvc/<host>:<port> SPN is registered \
         in the KDC, with a kinit'd ticket in KRB5CCNAME)",
    )
}

fn kerberos_port() -> u16 {
    std::env::var("MSSQL_KERBEROS_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1433)
}

/// The default constructor must build a working SPN: with the krb5 principal
/// name-type, `initialize()` acquires a service ticket for
/// `MSSQLSvc/<host>:<port>` and emits a SPNEGO token. Before the name-type fix
/// this returned an error — the KDC cannot resolve the host-based
/// interpretation of a slash/port principal — so this test goes red on the
/// bug and green on the fix.
#[test]
#[ignore = "Requires a live Kerberos KDC with the MSSQLSvc SPN registered and a kinit'd ticket"]
fn kerberos_default_constructor_acquires_service_ticket() {
    let host = kerberos_host();
    let port = kerberos_port();

    let auth = IntegratedAuth::new(&host, port);
    let token = auth.initialize().unwrap_or_else(|e| {
        panic!(
            "IntegratedAuth::new({host:?}, {port}).initialize() failed to obtain a \
             service ticket for MSSQLSvc/{host}:{port}: {e}. With GSS_NT_HOSTBASED_SERVICE \
             this is the expected failure (the bug); with GSS_NT_KRB5_PRINCIPAL it should \
             succeed against a KDC that has the SPN registered."
        )
    });

    assert!(
        token.len() > 50,
        "a real SPNEGO initial token (AP-REQ) is substantial; got {} bytes, \
         which suggests no service ticket was obtained",
        token.len()
    );
}
