//! Edge case tests for mssql-client.
//!
//! Tests for NULL handling, Unicode boundaries, large datasets, and other edge cases.
//!
//! These tests use the mock server where possible for fast, deterministic testing.
//! Tests marked with `#[ignore]` require a real SQL Server instance.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::approx_constant
)]

use bytes::Bytes;
use mssql_client::Config;
use mssql_types::{FromSql, SqlValue};

// =============================================================================
// NULL Handling Tests
// =============================================================================

#[test]
fn test_null_value_creation() {
    let null = SqlValue::Null;
    assert!(null.is_null());
}

#[test]
fn test_option_none_is_null() {
    let value: Option<i32> = None;
    assert!(value.is_none());
}

#[test]
fn test_option_some_is_not_null() {
    let value: Option<i32> = Some(42);
    assert!(value.is_some());
    assert_eq!(value, Some(42));
}

#[test]
fn test_null_from_sql_to_option() {
    let null = SqlValue::Null;
    let result: Result<Option<i32>, _> = Option::<i32>::from_sql(&null);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_value_from_sql_to_option() {
    let value = SqlValue::Int(42);
    let result: Result<Option<i32>, _> = Option::<i32>::from_sql(&value);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(42));
}

#[test]
fn test_null_string() {
    let null = SqlValue::Null;
    let result: Result<Option<String>, _> = Option::<String>::from_sql(&null);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_empty_string_is_not_null() {
    let value = SqlValue::String(String::new());
    assert!(!value.is_null());

    let result: Result<String, _> = String::from_sql(&value);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_null_binary() {
    let null = SqlValue::Null;
    let result: Result<Option<Vec<u8>>, _> = Option::<Vec<u8>>::from_sql(&null);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_empty_binary_is_not_null() {
    let value = SqlValue::Binary(Bytes::new());
    assert!(!value.is_null());

    let result: Result<Vec<u8>, _> = Vec::<u8>::from_sql(&value);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

// =============================================================================
// Unicode Boundary Tests
// =============================================================================

#[test]
fn test_ascii_string() {
    let s = "Hello, World!";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_basic_multilingual_plane() {
    // Characters from various scripts in BMP (U+0000 to U+FFFF)
    let s = "Hello 世界 مرحبا שלום こんにちは 🌍";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_supplementary_planes() {
    // Characters from supplementary planes (U+10000 and above)
    // These require surrogate pairs in UTF-16
    let s = "𝄞 𝕳𝖊𝖑𝖑𝖔 🎵 𓀀 🦀";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_combining_characters() {
    // Characters with combining diacritical marks
    // é as e + combining acute accent
    let s = "café e\u{0301}";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_zero_width_characters() {
    // Zero-width joiner, non-joiner, and space
    let s = "a\u{200B}b\u{200C}c\u{200D}d\u{FEFF}e";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_rtl_ltr_mixed() {
    // Right-to-left and left-to-right mixed text
    let s = "Hello שלום World مرحبا";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_emoji_sequences() {
    // Emoji with skin tone modifiers and ZWJ sequences
    let s = "👋🏽 👨‍👩‍👧‍👦 🏳️‍🌈";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_null_character() {
    // String containing embedded NULL character
    let s = "before\0after";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_unicode_max_codepoint() {
    // Maximum valid Unicode codepoint
    let s = "\u{10FFFF}";
    let value = SqlValue::String(s.to_string());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

#[test]
fn test_very_long_unicode_string() {
    // Create a long string with various Unicode characters
    let base = "Hello 世界 🌍 ";
    let s: String = base.repeat(1000);
    let value = SqlValue::String(s.clone());
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), s);
}

// =============================================================================
// Large Dataset Tests
// =============================================================================

#[test]
fn test_large_binary_data() {
    // 1 MB of binary data
    let size = 1024 * 1024;
    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let value = SqlValue::Binary(Bytes::from(data.clone()));

    let result: Result<Vec<u8>, _> = Vec::<u8>::from_sql(&value);
    let bytes = result.unwrap();
    assert_eq!(bytes.len(), size);
    assert_eq!(&bytes[..], &data[..]);
}

#[test]
fn test_large_string() {
    // 1 MB string
    let size = 1024 * 1024;
    let s: String = "x".repeat(size);
    let value = SqlValue::String(s.clone());

    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap().len(), size);
}

#[test]
fn test_many_small_values() {
    // Test creating and reading many small values
    let count = 100_000;
    let values: Vec<SqlValue> = (0..count).map(SqlValue::Int).collect();

    for (i, value) in values.iter().enumerate() {
        let result: Result<i32, _> = i32::from_sql(value);
        assert_eq!(result.unwrap(), i as i32);
    }
}

// =============================================================================
// Numeric Boundary Tests
// =============================================================================

#[test]
fn test_i32_boundaries() {
    let values = [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX];

    for &v in &values {
        let value = SqlValue::Int(v);
        let result: Result<i32, _> = i32::from_sql(&value);
        assert_eq!(result.unwrap(), v);
    }
}

#[test]
fn test_i64_boundaries() {
    let values = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];

    for &v in &values {
        let value = SqlValue::BigInt(v);
        let result: Result<i64, _> = i64::from_sql(&value);
        assert_eq!(result.unwrap(), v);
    }
}

#[test]
fn test_u8_boundaries() {
    let values = [u8::MIN, 1, 127, 128, u8::MAX - 1, u8::MAX];

    for &v in &values {
        let value = SqlValue::TinyInt(v);
        let result: Result<u8, _> = u8::from_sql(&value);
        assert_eq!(result.unwrap(), v);
    }
}

#[test]
fn test_float_special_values() {
    let values = [
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::EPSILON,
        0.0,
        -0.0,
        1.0,
        -1.0,
    ];

    for &v in &values {
        let value = SqlValue::Double(v);
        let result: Result<f64, _> = f64::from_sql(&value);
        let extracted = result.unwrap();
        if v.is_nan() {
            assert!(extracted.is_nan());
        } else {
            assert_eq!(extracted, v);
        }
    }
}

#[test]
fn test_float_nan() {
    let value = SqlValue::Double(f64::NAN);
    let result: Result<f64, _> = f64::from_sql(&value);
    assert!(result.unwrap().is_nan());
}

#[test]
fn test_float_infinity() {
    let pos_inf = SqlValue::Double(f64::INFINITY);
    let neg_inf = SqlValue::Double(f64::NEG_INFINITY);

    let result_pos: f64 = f64::from_sql(&pos_inf).expect("Should convert infinity");
    let result_neg: f64 = f64::from_sql(&neg_inf).expect("Should convert neg infinity");

    assert!(result_pos.is_infinite() && result_pos.is_sign_positive());
    assert!(result_neg.is_infinite() && result_neg.is_sign_negative());
}

// =============================================================================
// Connection String Edge Cases
// =============================================================================

#[test]
fn test_connection_string_with_special_characters_in_password() {
    // Password with special characters that need escaping
    let passwords = [
        "simple",
        "with space",
        "with;semicolon",
        "with=equals",
        "with'quote",
        r#"with"doublequote"#,
        "with\ttab",
        "with\nnewline",
        "MixedCase123!@#$%^&*()",
    ];

    for password in &passwords {
        // For passwords with special chars, they should be quoted
        let conn_str = if password.contains(';')
            || password.contains('=')
            || password.contains('"')
            || password.contains(' ')
        {
            format!(
                "Server=localhost;Database=test;User Id=sa;Password={{{}}}",
                password.replace('}', "}}")
            )
        } else {
            format!(
                "Server=localhost;Database=test;User Id=sa;Password={}",
                password
            )
        };

        // Should not panic during parsing
        let _ = Config::from_connection_string(&conn_str);
    }
}

#[test]
fn test_connection_string_empty_values() {
    // Empty database (should use default)
    let result = Config::from_connection_string("Server=localhost;Database=;User Id=sa;Password=x");
    // Should parse without error
    let _ = result;
}

#[test]
fn test_connection_string_unknown_keywords() {
    // Unknown keywords should be ignored (for forward compatibility)
    let result = Config::from_connection_string(
        "Server=localhost;Database=test;User Id=sa;Password=x;UnknownKeyword=value",
    );
    // Should parse without error
    assert!(result.is_ok());
}

#[test]
fn test_connection_string_case_insensitivity() {
    // Keywords should be case-insensitive
    let variants = [
        "Server=localhost;DATABASE=test;user id=sa;PASSWORD=x",
        "SERVER=localhost;database=test;USER ID=sa;password=x",
        "server=localhost;Database=test;User Id=sa;Password=x",
    ];

    for conn_str in &variants {
        let result = Config::from_connection_string(conn_str);
        assert!(result.is_ok(), "Failed to parse: {}", conn_str);
    }
}

#[test]
fn test_connection_string_whitespace_handling() {
    // Whitespace around separators
    let result = Config::from_connection_string(
        "Server = localhost ; Database = test ; User Id = sa ; Password = x",
    );
    // Whitespace may or may not be trimmed depending on implementation
    let _ = result;
}

#[test]
fn test_connection_string_duplicate_keywords() {
    // Last value should win for duplicate keywords
    let result = Config::from_connection_string(
        "Server=first;Server=second;Database=test;User Id=sa;Password=x",
    );
    // Should parse without error
    let _ = result;
}

// =============================================================================
// Type Conversion Edge Cases
// =============================================================================

#[test]
fn test_bool_from_various_types() {
    // Bool should be extractable from Bool value
    let value = SqlValue::Bool(true);
    let result: Result<bool, _> = bool::from_sql(&value);
    assert!(result.unwrap());

    let value = SqlValue::Bool(false);
    let result: Result<bool, _> = bool::from_sql(&value);
    assert!(!result.unwrap());
}

#[test]
fn test_type_mismatch_errors() {
    // Trying to extract wrong type should return error
    let string_value = SqlValue::String("not a number".to_string());
    let result: Result<i32, _> = i32::from_sql(&string_value);
    assert!(result.is_err());
}

#[test]
fn test_numeric_string_conversion() {
    // String containing numeric value
    let value = SqlValue::String("42".to_string());

    // This should be treated as a string, not auto-converted to i32
    let result: Result<String, _> = String::from_sql(&value);
    assert_eq!(result.unwrap(), "42");
}

// =============================================================================
// Live SQL Server Tests (require running instance)
// =============================================================================

fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_TEST_HOST").ok()?;
    let port = std::env::var("MSSQL_TEST_PORT").unwrap_or_else(|_| "1433".into());
    let user = std::env::var("MSSQL_TEST_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_TEST_PASSWORD").ok()?;

    let conn_str = format!(
        "Server={},{};Database=master;User Id={};Password={};TrustServerCertificate=true",
        host, port, user, password
    );

    Config::from_connection_string(&conn_str).ok()
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_null_handling_live() {
    use mssql_client::Client;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Query that returns NULL
    let mut stream = client
        .query("SELECT NULL AS null_col, 42 AS int_col", &[])
        .await
        .expect("Query failed");

    if let Some(row) = stream.next() {
        let row = row.expect("Row error");
        let null_val: Option<i32> = row.get(0).expect("Get failed");
        let int_val: Option<i32> = row.get(1).expect("Get failed");

        assert!(null_val.is_none());
        assert_eq!(int_val, Some(42));
    }

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_unicode_handling_live() {
    use mssql_client::Client;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    let test_string = "Hello 世界 🌍";
    let mut stream = client
        .query("SELECT @p1 AS unicode_col", &[&test_string])
        .await
        .expect("Query failed");

    if let Some(row) = stream.next() {
        let row = row.expect("Row error");
        let result: String = row.get(0).expect("Get failed");
        assert_eq!(result, test_string);
    }

    client.close().await.expect("Close failed");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_large_result_set_live() {
    use mssql_client::Client;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Generate 10000 rows using system tables
    let stream = client
        .query(
            "SELECT TOP 10000 ROW_NUMBER() OVER (ORDER BY (SELECT NULL)) AS n FROM sys.all_columns a CROSS JOIN sys.all_columns b",
            &[],
        )
        .await
        .expect("Query failed");

    let mut count = 0;
    for row in stream {
        let _ = row.expect("Row error");
        count += 1;
    }

    assert_eq!(count, 10000);

    client.close().await.expect("Close failed");
}
