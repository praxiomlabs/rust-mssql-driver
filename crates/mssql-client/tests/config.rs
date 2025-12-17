//! Connection string parsing edge case tests (TEST-011).
//!
//! Tests edge cases that users commonly encounter with connection strings.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::Config;

// ============================================================================
// Basic Parsing Tests
// ============================================================================

#[test]
fn test_empty_connection_string() {
    // Empty string should parse to defaults
    let config = Config::from_connection_string("");
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.host, "localhost");
}

#[test]
fn test_whitespace_only_connection_string() {
    let config = Config::from_connection_string("   \t\n  ");
    assert!(config.is_ok());
}

#[test]
fn test_single_semicolon() {
    let config = Config::from_connection_string(";");
    assert!(config.is_ok());
}

#[test]
fn test_multiple_semicolons() {
    let config = Config::from_connection_string(";;;");
    assert!(config.is_ok());
}

// ============================================================================
// Key-Value Edge Cases
// ============================================================================

#[test]
fn test_key_without_value() {
    let result = Config::from_connection_string("Server=");
    // Empty value should be treated as empty string
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.host, "");
}

#[test]
fn test_missing_equals_sign() {
    let result = Config::from_connection_string("Serverlocalhost;");
    // Should fail - no equals sign
    assert!(result.is_err());
}

#[test]
fn test_multiple_equals_in_value() {
    // Password with equals sign
    let _config =
        Config::from_connection_string("Server=localhost;Password=pass=word=with=equals;").unwrap();

    // The current implementation splits on first '=', so everything after is the value
    // Note: This may need fixing depending on desired behavior
}

#[test]
fn test_case_insensitive_keys() {
    let config1 = Config::from_connection_string("SERVER=host1;").unwrap();
    let config2 = Config::from_connection_string("server=host1;").unwrap();
    let config3 = Config::from_connection_string("Server=host1;").unwrap();

    assert_eq!(config1.host, config2.host);
    assert_eq!(config2.host, config3.host);
}

#[test]
fn test_alternative_key_names() {
    // "Data Source" is an alternative to "Server"
    let config1 = Config::from_connection_string("Server=host1;").unwrap();
    let config2 = Config::from_connection_string("Data Source=host1;").unwrap();
    let config3 = Config::from_connection_string("Host=host1;").unwrap();

    assert_eq!(config1.host, "host1");
    assert_eq!(config2.host, "host1");
    assert_eq!(config3.host, "host1");

    // "Initial Catalog" is an alternative to "Database"
    let config4 = Config::from_connection_string("Database=db1;").unwrap();
    let config5 = Config::from_connection_string("Initial Catalog=db1;").unwrap();

    assert_eq!(config4.database, config5.database);

    // User Id alternatives - just verify they parse correctly
    let _config6 = Config::from_connection_string("User Id=user1;").unwrap();
    let _config7 = Config::from_connection_string("UID=user1;").unwrap();
    let _config8 = Config::from_connection_string("User=user1;").unwrap();

    // Password alternatives - just verify they parse correctly
    let _config9 = Config::from_connection_string("Password=pass1;").unwrap();
    let _config10 = Config::from_connection_string("PWD=pass1;").unwrap();
}

// ============================================================================
// Server Address Formats
// ============================================================================

#[test]
fn test_server_with_port() {
    let config = Config::from_connection_string("Server=myserver,1434;").unwrap();
    assert_eq!(config.host, "myserver");
    assert_eq!(config.port, 1434);
}

#[test]
fn test_server_with_instance() {
    let config = Config::from_connection_string("Server=myserver\\SQLEXPRESS;").unwrap();
    assert_eq!(config.host, "myserver");
    assert_eq!(config.instance, Some("SQLEXPRESS".to_string()));
}

#[test]
fn test_server_ipv4() {
    let config = Config::from_connection_string("Server=192.168.1.100;").unwrap();
    assert_eq!(config.host, "192.168.1.100");
}

#[test]
fn test_server_ipv4_with_port() {
    let config = Config::from_connection_string("Server=192.168.1.100,1434;").unwrap();
    assert_eq!(config.host, "192.168.1.100");
    assert_eq!(config.port, 1434);
}

#[test]
fn test_azure_server_name() {
    let config = Config::from_connection_string("Server=myserver.database.windows.net;").unwrap();
    assert_eq!(config.host, "myserver.database.windows.net");
}

#[test]
fn test_invalid_port_number() {
    let result = Config::from_connection_string("Server=localhost,abc;");
    assert!(result.is_err());
}

#[test]
fn test_port_overflow() {
    let result = Config::from_connection_string("Server=localhost,999999;");
    assert!(result.is_err());
}

// ============================================================================
// Boolean Value Parsing
// ============================================================================

#[test]
fn test_trust_server_certificate_true_values() {
    // Various "true" representations
    let cases = ["true", "True", "TRUE", "yes", "Yes", "YES", "1"];

    for case in cases {
        let conn_str = format!("TrustServerCertificate={};", case);
        let config = Config::from_connection_string(&conn_str).unwrap();
        assert!(config.trust_server_certificate, "Failed for: {}", case);
    }
}

#[test]
fn test_trust_server_certificate_false_values() {
    // Various "false" representations
    let cases = ["false", "False", "FALSE", "no", "No", "NO", "0"];

    for case in cases {
        let conn_str = format!("TrustServerCertificate={};", case);
        let config = Config::from_connection_string(&conn_str).unwrap();
        assert!(!config.trust_server_certificate, "Failed for: {}", case);
    }
}

#[test]
fn test_mars_boolean_values() {
    let config_true = Config::from_connection_string("MARS=true;").unwrap();
    assert!(config_true.mars);

    let config_false = Config::from_connection_string("MARS=false;").unwrap();
    assert!(!config_false.mars);
}

// ============================================================================
// Timeout Value Parsing
// ============================================================================

#[test]
fn test_connect_timeout_parsing() {
    let config = Config::from_connection_string("Connect Timeout=30;").unwrap();
    assert_eq!(config.connect_timeout.as_secs(), 30);

    let config2 = Config::from_connection_string("Connection Timeout=60;").unwrap();
    assert_eq!(config2.connect_timeout.as_secs(), 60);
}

#[test]
fn test_command_timeout_parsing() {
    let config = Config::from_connection_string("Command Timeout=120;").unwrap();
    assert_eq!(config.command_timeout.as_secs(), 120);
}

#[test]
fn test_invalid_timeout_value() {
    let result = Config::from_connection_string("Connect Timeout=abc;");
    assert!(result.is_err());
}

#[test]
fn test_negative_timeout_rejected() {
    // Negative timeout should fail to parse as u64
    let result = Config::from_connection_string("Connect Timeout=-1;");
    assert!(result.is_err());
}

// ============================================================================
// Packet Size Parsing
// ============================================================================

#[test]
fn test_packet_size_parsing() {
    let config = Config::from_connection_string("Packet Size=8192;").unwrap();
    assert_eq!(config.packet_size, 8192);
}

#[test]
fn test_invalid_packet_size() {
    let result = Config::from_connection_string("Packet Size=invalid;");
    assert!(result.is_err());
}

// ============================================================================
// Encryption Settings
// ============================================================================

#[test]
fn test_encrypt_strict() {
    let config = Config::from_connection_string("Encrypt=strict;").unwrap();
    assert!(config.strict_mode);
}

#[test]
fn test_encrypt_strict_case_insensitive() {
    let config = Config::from_connection_string("Encrypt=STRICT;").unwrap();
    assert!(config.strict_mode);
}

// ============================================================================
// Special Character Handling
// ============================================================================

#[test]
fn test_whitespace_in_values() {
    let config = Config::from_connection_string("Server=  localhost  ;").unwrap();
    // Whitespace should be trimmed
    assert_eq!(config.host, "localhost");
}

#[test]
fn test_whitespace_around_equals() {
    let config = Config::from_connection_string("Server = localhost ;").unwrap();
    assert_eq!(config.host, "localhost");
}

// ============================================================================
// Unknown Keys (Forward Compatibility)
// ============================================================================

#[test]
fn test_unknown_keys_ignored() {
    // Unknown keys should be ignored for forward compatibility
    let config = Config::from_connection_string(
        "Server=localhost;UnknownOption=value;FutureFeature=enabled;",
    );
    assert!(config.is_ok());
}

// ============================================================================
// Complex Connection Strings
// ============================================================================

#[test]
fn test_full_azure_connection_string() {
    let conn_str = "Server=myserver.database.windows.net;\
                    Database=mydb;\
                    User Id=admin@myserver;\
                    Password=P@ssw0rd!;\
                    Encrypt=strict;\
                    TrustServerCertificate=false;\
                    Connect Timeout=30;\
                    Application Name=MyApp;";

    let config = Config::from_connection_string(conn_str).unwrap();

    assert_eq!(config.host, "myserver.database.windows.net");
    assert_eq!(config.database, Some("mydb".to_string()));
    assert!(config.strict_mode);
    assert!(!config.trust_server_certificate);
    assert_eq!(config.connect_timeout.as_secs(), 30);
    assert_eq!(config.application_name, "MyApp");
}

#[test]
fn test_connection_string_without_trailing_semicolon() {
    let config = Config::from_connection_string("Server=localhost;Database=test").unwrap();

    assert_eq!(config.host, "localhost");
    assert_eq!(config.database, Some("test".to_string()));
}

#[test]
fn test_repeated_keys_last_wins() {
    // When a key appears multiple times, the last value should win
    let config =
        Config::from_connection_string("Server=first;Server=second;Server=third;").unwrap();

    assert_eq!(config.host, "third");
}
