//! Connection string parsing edge case tests (TEST-011).
//!
//! Tests edge cases that users commonly encounter with connection strings.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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
        let conn_str = format!("TrustServerCertificate={case};");
        let config = Config::from_connection_string(&conn_str).unwrap();
        assert!(config.trust_server_certificate, "Failed for: {case}");
    }
}

#[test]
fn test_trust_server_certificate_false_values() {
    // Various "false" representations
    let cases = ["false", "False", "FALSE", "no", "No", "NO", "0"];

    for case in cases {
        let conn_str = format!("TrustServerCertificate={case};");
        let config = Config::from_connection_string(&conn_str).unwrap();
        assert!(!config.trust_server_certificate, "Failed for: {case}");
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

// ============================================================================
// FEDAUTH Credential Gate (issues #159, #155)
// ============================================================================
//
// Azure AD credentials are wired into the login sequence (LOGIN7 FEDAUTH
// feature extension, SecurityToken workflow — #155 Phase 1), so they are no
// longer rejected wholesale. The gate now fails fast only on configurations
// that cannot produce a valid (or safe) FEDAUTH login. All tests use an
// unresolvable host: the error must arrive before any network I/O.

/// A bearer token must never be sent over a plaintext connection.
#[tokio::test]
async fn test_azure_credentials_rejected_with_no_tls() {
    let config = Config::from_connection_string(
        "Server=host-that-must-never-be-contacted.invalid;Encrypt=no_tls",
    )
    .unwrap()
    .credentials(mssql_client::Credentials::AzureAccessToken {
        token: "test-token".into(),
    });

    let err = mssql_client::Client::connect(config)
        .await
        .expect_err("FEDAUTH over no_tls must be rejected at connect time");

    let msg = err.to_string();
    assert!(
        matches!(err, mssql_client::Error::Config(_)),
        "expected Error::Config, got: {msg}"
    );
    assert!(
        msg.contains("no_tls") && msg.contains("plaintext"),
        "error should explain the plaintext-token hazard: {msg}"
    );
}

/// The FEDAUTH token length must not be zero (MS-TDS §2.2.6.4); an empty
/// token is a configuration mistake caught before any network I/O.
#[tokio::test]
async fn test_azure_credentials_rejected_with_empty_token() {
    let config = Config::new()
        .host("host-that-must-never-be-contacted.invalid")
        .credentials(mssql_client::Credentials::AzureAccessToken { token: "".into() });

    let err = mssql_client::Client::connect(config)
        .await
        .expect_err("an empty Azure access token must be rejected");

    let msg = err.to_string();
    assert!(
        matches!(err, mssql_client::Error::Config(_)),
        "expected Error::Config, got: {msg}"
    );
    assert!(
        msg.contains("empty"),
        "error should say the token is empty: {msg}"
    );
}

/// The FEDAUTH feature extension requires the LOGIN7 FeatureExt block, which
/// exists only in TDS 7.4+.
#[tokio::test]
async fn test_azure_credentials_rejected_below_tds_74() {
    let config = Config::from_connection_string(
        "Server=host-that-must-never-be-contacted.invalid;TdsVersion=7.3",
    )
    .unwrap()
    .credentials(mssql_client::Credentials::AzureAccessToken {
        token: "test-token".into(),
    });

    let err = mssql_client::Client::connect(config)
        .await
        .expect_err("FEDAUTH below TDS 7.4 must be rejected");

    let msg = err.to_string();
    assert!(
        matches!(err, mssql_client::Error::Config(_)),
        "expected Error::Config, got: {msg}"
    );
    assert!(
        msg.contains("7.4"),
        "error should name the TDS floor: {msg}"
    );
}

// ============================================================================
// Authentication keyword (ADR-002, #155)
// ============================================================================

#[test]
fn test_authentication_sql_password_keeps_sql_credentials() {
    let config = Config::from_connection_string(
        "Server=s;User Id=sa;Password=pw;Authentication=SqlPassword",
    )
    .unwrap();

    match config.credentials {
        mssql_client::Credentials::SqlServer { username, password } => {
            assert_eq!(username.as_ref(), "sa");
            assert_eq!(password.as_ref(), "pw");
        }
        other => panic!("expected SqlServer credentials, got {other:?}"),
    }
}

#[test]
fn test_authentication_unsupported_ad_value_errors_with_token_guidance() {
    // All three interactive Entra values share one match arm in the parser;
    // cover each so a future split of that arm cannot silently drop one.
    for value in [
        "ActiveDirectoryPassword",
        "ActiveDirectoryInteractive",
        "ActiveDirectoryDeviceCodeFlow",
    ] {
        let err = Config::from_connection_string(&format!(
            "Server=s;User Id=u;Password=p;Authentication={value}"
        ))
        .expect_err("interactive Entra value should not be supported");

        let msg = err.to_string();
        assert!(msg.contains("not supported"), "{value}: {msg}");
        assert!(
            msg.contains("azure_token"),
            "{value} should point to the bring-your-own-token escape hatch: {msg}"
        );
    }
}

#[test]
fn test_authentication_invalid_value_errors() {
    let err = Config::from_connection_string("Server=s;Authentication=BogusMethod")
        .expect_err("unknown Authentication value must error");

    let msg = err.to_string();
    assert!(msg.contains("invalid Authentication value"), "{msg}");
}

#[cfg(feature = "azure-identity")]
mod authentication_azure_identity {
    use super::*;

    #[test]
    fn test_service_principal_parses_client_and_tenant() {
        // Spaced ADO.NET form, and Authentication placed BEFORE the
        // credentials it reinterprets — order must not matter.
        let config = Config::from_connection_string(
            "Server=s;Authentication=Active Directory Service Principal;\
             User Id=client-guid@tenant-guid;Password=s3cret",
        )
        .unwrap();

        match config.credentials {
            mssql_client::Credentials::AzureServicePrincipal {
                tenant_id,
                client_id,
                client_secret,
            } => {
                assert_eq!(client_id.as_ref(), "client-guid");
                assert_eq!(tenant_id.as_ref(), "tenant-guid");
                assert_eq!(client_secret.as_ref(), "s3cret");
            }
            other => panic!("expected AzureServicePrincipal, got {other:?}"),
        }
    }

    #[test]
    fn test_service_principal_requires_tenant_in_user_id() {
        let err = Config::from_connection_string(
            "Server=s;User Id=client-only;Password=s3cret;\
             Authentication=ActiveDirectoryServicePrincipal",
        )
        .expect_err("missing tenant id must error");

        let msg = err.to_string();
        assert!(
            msg.contains("<client-id>@<tenant-id>"),
            "error must show the expected User Id format: {msg}"
        );
    }

    #[test]
    fn test_service_principal_requires_secret() {
        let err = Config::from_connection_string(
            "Server=s;User Id=c@t;Authentication=ActiveDirectoryServicePrincipal",
        )
        .expect_err("missing client secret must error");

        assert!(err.to_string().contains("client secret"), "{err}");
    }

    #[test]
    fn test_managed_identity_system_assigned() {
        let config = Config::from_connection_string(
            "Server=s;Authentication=ActiveDirectoryManagedIdentity",
        )
        .unwrap();

        assert!(matches!(
            config.credentials,
            mssql_client::Credentials::AzureManagedIdentity { client_id: None }
        ));
    }

    #[test]
    fn test_managed_identity_user_assigned_via_user_id_and_msi_alias() {
        let config = Config::from_connection_string(
            "Server=s;User Id=uami-client-id;Authentication=Active Directory MSI",
        )
        .unwrap();

        match config.credentials {
            mssql_client::Credentials::AzureManagedIdentity {
                client_id: Some(id),
            } => assert_eq!(id.as_ref(), "uami-client-id"),
            other => panic!("expected user-assigned AzureManagedIdentity, got {other:?}"),
        }
    }
}

#[cfg(any(feature = "integrated-auth", feature = "sspi-auth"))]
#[test]
fn test_authentication_conflicts_with_integrated_security() {
    let err = Config::from_connection_string(
        "Server=s;Integrated Security=true;Authentication=SqlPassword",
    )
    .expect_err("Authentication + Integrated Security must error");

    assert!(err.to_string().contains("Integrated Security"), "{err}");
}
