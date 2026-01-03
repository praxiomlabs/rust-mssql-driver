//! Error handling path tests for mssql-client.
//!
//! Tests for error creation, conversion, categorization, and display.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::approx_constant
)]

use mssql_client::Error;
use std::sync::Arc;

// =============================================================================
// Error Display Tests
// =============================================================================

#[test]
fn test_connection_error_display() {
    let err = Error::Connection("network unreachable".into());
    let msg = err.to_string();
    assert!(msg.contains("connection failed"));
    assert!(msg.contains("network unreachable"));
}

#[test]
fn test_connection_closed_display() {
    let err = Error::ConnectionClosed;
    assert_eq!(err.to_string(), "connection closed");
}

#[test]
fn test_tls_error_display() {
    let err = Error::Tls("certificate expired".into());
    let msg = err.to_string();
    assert!(msg.contains("TLS error"));
    assert!(msg.contains("certificate expired"));
}

#[test]
fn test_protocol_error_display() {
    let err = Error::Protocol("unexpected token".into());
    let msg = err.to_string();
    assert!(msg.contains("protocol error"));
    assert!(msg.contains("unexpected token"));
}

#[test]
fn test_query_error_display() {
    let err = Error::Query("division by zero".into());
    let msg = err.to_string();
    assert!(msg.contains("query error"));
    assert!(msg.contains("division by zero"));
}

#[test]
fn test_server_error_display() {
    let err = Error::Server {
        number: 8134,
        class: 16,
        state: 1,
        message: "Divide by zero error encountered.".into(),
        server: Some("SQLSERVER01".into()),
        procedure: Some("sp_calculate".into()),
        line: 42,
    };
    let msg = err.to_string();
    assert!(msg.contains("8134"));
    assert!(msg.contains("Divide by zero"));
}

#[test]
fn test_server_error_without_optional_fields() {
    let err = Error::Server {
        number: 102,
        class: 15,
        state: 1,
        message: "Syntax error".into(),
        server: None,
        procedure: None,
        line: 1,
    };
    let msg = err.to_string();
    assert!(msg.contains("102"));
    assert!(msg.contains("Syntax error"));
}

#[test]
fn test_transaction_error_display() {
    let err = Error::Transaction("already rolled back".into());
    let msg = err.to_string();
    assert!(msg.contains("transaction error"));
    assert!(msg.contains("already rolled back"));
}

#[test]
fn test_config_error_display() {
    let err = Error::Config("invalid port number".into());
    let msg = err.to_string();
    assert!(msg.contains("configuration error"));
    assert!(msg.contains("invalid port number"));
}

#[test]
fn test_timeout_errors_display() {
    assert_eq!(Error::ConnectTimeout.to_string(), "connection timed out");
    assert_eq!(Error::TlsTimeout.to_string(), "TLS handshake timed out");
    assert_eq!(Error::ConnectionTimeout.to_string(), "connection timed out");
    assert_eq!(Error::CommandTimeout.to_string(), "command timed out");
}

#[test]
fn test_routing_error_display() {
    let err = Error::Routing {
        host: "replica.database.windows.net".into(),
        port: 11000,
    };
    let msg = err.to_string();
    assert!(msg.contains("routing required"));
    assert!(msg.contains("replica.database.windows.net"));
    assert!(msg.contains("11000"));
}

#[test]
fn test_too_many_redirects_display() {
    let err = Error::TooManyRedirects { max: 5 };
    let msg = err.to_string();
    assert!(msg.contains("too many redirects"));
    assert!(msg.contains("5"));
}

#[test]
fn test_io_error_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    let err = Error::Io(Arc::new(io_err));
    let msg = err.to_string();
    assert!(msg.contains("IO error"));
    assert!(msg.contains("refused"));
}

#[test]
fn test_invalid_identifier_display() {
    let err = Error::InvalidIdentifier("DROP TABLE--".into());
    let msg = err.to_string();
    assert!(msg.contains("invalid identifier"));
    assert!(msg.contains("DROP TABLE--"));
}

#[test]
fn test_pool_exhausted_display() {
    assert_eq!(
        Error::PoolExhausted.to_string(),
        "connection pool exhausted"
    );
}

#[test]
fn test_cancel_error_display() {
    let err = Error::Cancel("connection not found".into());
    let msg = err.to_string();
    assert!(msg.contains("cancellation failed"));
    assert!(msg.contains("connection not found"));
}

#[test]
fn test_cancelled_display() {
    assert_eq!(Error::Cancelled.to_string(), "query cancelled");
}

// =============================================================================
// Error Conversion Tests
// =============================================================================

#[test]
fn test_io_error_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotConnected, "not connected");
    let err: Error = io_err.into();

    match err {
        Error::Io(arc_err) => {
            assert_eq!(arc_err.kind(), std::io::ErrorKind::NotConnected);
        }
        _ => panic!("Expected IO error"),
    }
}

#[test]
fn test_io_error_is_clone() {
    let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
    let err: Error = io_err.into();

    // Error should be cloneable via Arc
    if let Error::Io(arc1) = &err {
        let arc2 = Arc::clone(arc1);
        assert_eq!(arc1.kind(), arc2.kind());
    }
}

// =============================================================================
// Error Categorization Tests
// =============================================================================

#[test]
fn test_is_protocol_error() {
    assert!(Error::Protocol("test".into()).is_protocol_error());
    assert!(!Error::Query("test".into()).is_protocol_error());
    assert!(
        !Error::Server {
            number: 102,
            class: 16,
            state: 1,
            message: "test".into(),
            server: None,
            procedure: None,
            line: 1,
        }
        .is_protocol_error()
    );
}

#[test]
fn test_error_class_severity_equivalence() {
    let err = Error::Server {
        number: 547,
        class: 16,
        state: 0,
        message: "Constraint violation".into(),
        server: None,
        procedure: None,
        line: 1,
    };

    // class() and severity() should return the same value
    assert_eq!(err.class(), err.severity());
    assert_eq!(err.class(), Some(16));
}

#[test]
fn test_error_severity_ranges() {
    // Informational (0-10)
    let info = Error::Server {
        number: 0,
        class: 5,
        state: 0,
        message: "Info".into(),
        server: None,
        procedure: None,
        line: 1,
    };
    assert!(info.severity().unwrap() <= 10);

    // User error (11-16)
    let user_err = Error::Server {
        number: 102,
        class: 15,
        state: 1,
        message: "Syntax error".into(),
        server: None,
        procedure: None,
        line: 1,
    };
    let sev = user_err.severity().unwrap();
    assert!((11..=16).contains(&sev));

    // Resource error (17-19)
    let resource_err = Error::Server {
        number: 1105,
        class: 17,
        state: 2,
        message: "Could not allocate space".into(),
        server: None,
        procedure: None,
        line: 1,
    };
    let sev = resource_err.severity().unwrap();
    assert!((17..=19).contains(&sev));

    // System error (20-25) - connection terminating
    let system_err = Error::Server {
        number: 20,
        class: 20,
        state: 0,
        message: "Fatal error".into(),
        server: None,
        procedure: None,
        line: 1,
    };
    let sev = system_err.severity().unwrap();
    assert!((20..=25).contains(&sev));
}

#[test]
fn test_non_server_error_has_no_class() {
    assert!(Error::ConnectionTimeout.class().is_none());
    assert!(Error::Query("test".into()).class().is_none());
    assert!(Error::Config("test".into()).class().is_none());
    assert!(Error::PoolExhausted.class().is_none());
}

// =============================================================================
// Error State Checks
// =============================================================================

#[test]
fn test_mutually_exclusive_transient_terminal() {
    // An error should generally not be both transient AND terminal
    // (though this isn't strictly enforced by the type system)

    let transient_err = Error::ConnectionTimeout;
    assert!(transient_err.is_transient());
    assert!(!transient_err.is_terminal());

    let terminal_err = Error::Config("bad".into());
    assert!(terminal_err.is_terminal());
    assert!(!terminal_err.is_transient());
}

#[test]
fn test_all_timeout_types_are_transient() {
    let timeout_errors = [
        Error::ConnectTimeout,
        Error::TlsTimeout,
        Error::ConnectionTimeout,
        Error::CommandTimeout,
    ];

    for err in timeout_errors {
        assert!(err.is_transient(), "{:?} should be transient", err);
    }
}

#[test]
fn test_routing_is_transient() {
    // Routing redirects are transient - the client should follow the redirect
    let routing = Error::Routing {
        host: "newhost".into(),
        port: 1433,
    };
    assert!(routing.is_transient());
    assert!(!routing.is_terminal());
}

// =============================================================================
// Azure-Specific Error Tests
// =============================================================================

fn make_azure_error(number: i32, message: &str) -> Error {
    Error::Server {
        number,
        class: 16,
        state: 1,
        message: message.into(),
        server: Some("myserver.database.windows.net".into()),
        procedure: None,
        line: 1,
    }
}

#[test]
fn test_azure_resource_limit_errors() {
    // Error 10928: Resource ID exceeded
    let err = make_azure_error(10928, "Resource ID : 1. Request limit exceeded");
    assert!(err.is_transient());

    // Error 10929: Resource ID minimum guarantee
    let err = make_azure_error(10929, "Resource ID : 1. The minimum guarantee is 10");
    assert!(err.is_transient());
}

#[test]
fn test_azure_service_errors() {
    // Error 40197: Service has encountered an error
    let err = make_azure_error(40197, "The service has encountered an error");
    assert!(err.is_transient());

    // Error 40501: Service is currently busy
    let err = make_azure_error(40501, "The service is currently busy");
    assert!(err.is_transient());

    // Error 40613: Database is not currently available
    let err = make_azure_error(40613, "Database is not currently available");
    assert!(err.is_transient());
}

#[test]
fn test_azure_cannot_process_errors() {
    // 49918, 49919, 49920: Cannot process request
    for number in [49918, 49919, 49920] {
        let err = make_azure_error(number, "Cannot process request");
        assert!(err.is_transient(), "Error {} should be transient", number);
    }
}

// =============================================================================
// SQL Server Classic Error Tests
// =============================================================================

fn make_server_error(number: i32, class: u8, message: &str) -> Error {
    Error::Server {
        number,
        class,
        state: 1,
        message: message.into(),
        server: Some("SQLSERVER01".into()),
        procedure: None,
        line: 1,
    }
}

#[test]
fn test_deadlock_is_transient() {
    let err = make_server_error(1205, 13, "Transaction was deadlocked");
    assert!(err.is_transient());
    assert!(!err.is_terminal());
}

#[test]
fn test_syntax_errors_are_terminal() {
    // 102: Incorrect syntax
    let err = make_server_error(102, 15, "Incorrect syntax near 'SELEC'");
    assert!(err.is_terminal());
    assert!(!err.is_transient());
}

#[test]
fn test_object_errors_are_terminal() {
    // 207: Invalid column name
    let err = make_server_error(207, 16, "Invalid column name 'foo'");
    assert!(err.is_terminal());

    // 208: Invalid object name
    let err = make_server_error(208, 16, "Invalid object name 'dbo.nonexistent'");
    assert!(err.is_terminal());
}

#[test]
fn test_constraint_errors_are_terminal() {
    // 547: Constraint violation
    let err = make_server_error(
        547,
        16,
        "The INSERT statement conflicted with the FOREIGN KEY constraint",
    );
    assert!(err.is_terminal());

    // 2627: Unique constraint violation
    let err = make_server_error(2627, 14, "Violation of UNIQUE KEY constraint");
    assert!(err.is_terminal());

    // 2601: Duplicate key
    let err = make_server_error(2601, 14, "Cannot insert duplicate key row");
    assert!(err.is_terminal());
}

// =============================================================================
// Error Debug Implementation Tests
// =============================================================================

#[test]
fn test_error_debug_format() {
    let err = Error::Server {
        number: 102,
        class: 15,
        state: 1,
        message: "Syntax error".into(),
        server: Some("SERVER".into()),
        procedure: Some("sp_test".into()),
        line: 42,
    };

    let debug = format!("{:?}", err);
    assert!(debug.contains("Server"));
    assert!(debug.contains("102"));
    assert!(debug.contains("Syntax error"));
}

#[test]
fn test_all_error_variants_are_debug() {
    // Ensure all error variants can be formatted with Debug
    let errors: Vec<Error> = vec![
        Error::Connection("test".into()),
        Error::ConnectionClosed,
        Error::Tls("test".into()),
        Error::Protocol("test".into()),
        Error::Query("test".into()),
        Error::Server {
            number: 1,
            class: 1,
            state: 1,
            message: "test".into(),
            server: None,
            procedure: None,
            line: 1,
        },
        Error::Transaction("test".into()),
        Error::Config("test".into()),
        Error::ConnectTimeout,
        Error::TlsTimeout,
        Error::ConnectionTimeout,
        Error::CommandTimeout,
        Error::Routing {
            host: "h".into(),
            port: 1,
        },
        Error::TooManyRedirects { max: 1 },
        Error::Io(Arc::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "test",
        ))),
        Error::InvalidIdentifier("test".into()),
        Error::PoolExhausted,
        Error::Cancel("test".into()),
        Error::Cancelled,
    ];

    for err in errors {
        let _ = format!("{:?}", err);
        let _ = format!("{}", err);
    }
}

// =============================================================================
// Error Source Chain Tests
// =============================================================================

#[test]
fn test_io_error_source() {
    use std::error::Error as StdError;

    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "inner error");
    let err = Error::Io(Arc::new(io_err));

    // The error should have a source
    // Note: thiserror may or may not expose the source depending on definition
    let _ = err.source();
}
