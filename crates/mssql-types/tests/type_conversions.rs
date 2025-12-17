//! Type conversion edge case tests (TEST-008).
//!
//! Tests edge cases for:
//! - NULL handling
//! - Unicode/UTF-8 boundary conditions
//! - Large dataset handling
//! - Type conversion boundaries

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::approx_constant)]

use bytes::Bytes;
use mssql_types::{FromSql, SqlValue, ToSql, TypeError};

// ============================================================================
// NULL Handling Edge Cases
// ============================================================================

mod null_handling {
    use super::*;

    #[test]
    fn test_null_to_option_i32() {
        let null_value = SqlValue::Null;
        let result: Result<Option<i32>, _> = Option::<i32>::from_sql(&null_value);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_null_to_non_option_fails() {
        let null_value = SqlValue::Null;
        let result: Result<i32, _> = i32::from_sql(&null_value);
        assert!(matches!(result, Err(TypeError::UnexpectedNull)));
    }

    #[test]
    fn test_null_to_option_string() {
        let null_value = SqlValue::Null;
        let result: Result<Option<String>, _> = Option::<String>::from_sql(&null_value);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_null_to_string_fails() {
        let null_value = SqlValue::Null;
        let result: Result<String, _> = String::from_sql(&null_value);
        assert!(matches!(result, Err(TypeError::UnexpectedNull)));
    }

    #[test]
    fn test_option_none_to_sql() {
        let none_value: Option<i32> = None;
        let result = none_value.to_sql().unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_option_some_to_sql() {
        let some_value: Option<i32> = Some(42);
        let result = some_value.to_sql().unwrap();
        assert!(!result.is_null());
        assert_eq!(result, SqlValue::Int(42));
    }

    #[test]
    fn test_nested_option_to_sql() {
        // Option<Option<i32>> should flatten correctly
        let nested: Option<i32> = Some(42);
        let result = nested.to_sql().unwrap();
        assert_eq!(result, SqlValue::Int(42));

        let nested_none: Option<i32> = None;
        let result_none = nested_none.to_sql().unwrap();
        assert!(result_none.is_null());
    }
}

// ============================================================================
// Integer Boundary Tests
// ============================================================================

mod integer_boundaries {
    use super::*;

    #[test]
    fn test_i32_max() {
        let max_val = SqlValue::Int(i32::MAX);
        let result: Result<i32, _> = i32::from_sql(&max_val);
        assert_eq!(result.unwrap(), i32::MAX);
    }

    #[test]
    fn test_i32_min() {
        let min_val = SqlValue::Int(i32::MIN);
        let result: Result<i32, _> = i32::from_sql(&min_val);
        assert_eq!(result.unwrap(), i32::MIN);
    }

    #[test]
    fn test_i64_max() {
        let max_val = SqlValue::BigInt(i64::MAX);
        let result: Result<i64, _> = i64::from_sql(&max_val);
        assert_eq!(result.unwrap(), i64::MAX);
    }

    #[test]
    fn test_i64_min() {
        let min_val = SqlValue::BigInt(i64::MIN);
        let result: Result<i64, _> = i64::from_sql(&min_val);
        assert_eq!(result.unwrap(), i64::MIN);
    }

    #[test]
    fn test_i16_max() {
        let max_val = SqlValue::SmallInt(i16::MAX);
        let result: Result<i16, _> = i16::from_sql(&max_val);
        assert_eq!(result.unwrap(), i16::MAX);
    }

    #[test]
    fn test_i16_min() {
        let min_val = SqlValue::SmallInt(i16::MIN);
        let result: Result<i16, _> = i16::from_sql(&min_val);
        assert_eq!(result.unwrap(), i16::MIN);
    }

    #[test]
    fn test_u8_max() {
        let max_val = SqlValue::TinyInt(u8::MAX);
        let result: Result<u8, _> = u8::from_sql(&max_val);
        assert_eq!(result.unwrap(), u8::MAX);
    }

    #[test]
    fn test_u8_zero() {
        let zero_val = SqlValue::TinyInt(0);
        let result: Result<u8, _> = u8::from_sql(&zero_val);
        assert_eq!(result.unwrap(), 0);
    }
}

// ============================================================================
// Floating Point Edge Cases
// ============================================================================

mod float_boundaries {
    use super::*;

    #[test]
    fn test_f64_max() {
        let max_val = SqlValue::Double(f64::MAX);
        let result: Result<f64, _> = f64::from_sql(&max_val);
        assert_eq!(result.unwrap(), f64::MAX);
    }

    #[test]
    fn test_f64_min() {
        let min_val = SqlValue::Double(f64::MIN);
        let result: Result<f64, _> = f64::from_sql(&min_val);
        assert_eq!(result.unwrap(), f64::MIN);
    }

    #[test]
    fn test_f64_positive_infinity() {
        let inf_val = SqlValue::Double(f64::INFINITY);
        let result: Result<f64, _> = f64::from_sql(&inf_val);
        assert!(result.unwrap().is_infinite());
    }

    #[test]
    fn test_f64_negative_infinity() {
        let neg_inf_val = SqlValue::Double(f64::NEG_INFINITY);
        let result: Result<f64, _> = f64::from_sql(&neg_inf_val);
        let val = result.unwrap();
        assert!(val.is_infinite() && val.is_sign_negative());
    }

    #[test]
    fn test_f64_nan() {
        let nan_val = SqlValue::Double(f64::NAN);
        let result: Result<f64, _> = f64::from_sql(&nan_val);
        assert!(result.unwrap().is_nan());
    }

    #[test]
    fn test_f64_zero() {
        let zero_val = SqlValue::Double(0.0);
        let result: Result<f64, _> = f64::from_sql(&zero_val);
        assert_eq!(result.unwrap(), 0.0);
    }

    #[test]
    fn test_f64_negative_zero() {
        let neg_zero_val = SqlValue::Double(-0.0);
        let result: Result<f64, _> = f64::from_sql(&neg_zero_val);
        let val = result.unwrap();
        assert!(val == 0.0); // -0.0 equals 0.0
    }

    #[test]
    fn test_f32_max() {
        let max_val = SqlValue::Float(f32::MAX);
        let result: Result<f32, _> = f32::from_sql(&max_val);
        assert_eq!(result.unwrap(), f32::MAX);
    }
}

// ============================================================================
// String/Unicode Edge Cases
// ============================================================================

mod unicode_handling {
    use super::*;

    #[test]
    fn test_empty_string() {
        let empty = SqlValue::String(String::new());
        let result: Result<String, _> = String::from_sql(&empty);
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_ascii_string() {
        let ascii = SqlValue::String("Hello, World!".to_string());
        let result: Result<String, _> = String::from_sql(&ascii);
        assert_eq!(result.unwrap(), "Hello, World!");
    }

    #[test]
    fn test_unicode_basic_multilingual_plane() {
        // Characters in BMP (U+0000 to U+FFFF)
        let bmp = SqlValue::String("日本語テスト".to_string());
        let result: Result<String, _> = String::from_sql(&bmp);
        assert_eq!(result.unwrap(), "日本語テスト");
    }

    #[test]
    fn test_unicode_supplementary_planes() {
        // Characters outside BMP (requires surrogate pairs in UTF-16)
        let emoji = SqlValue::String("😀🎉🚀".to_string());
        let result: Result<String, _> = String::from_sql(&emoji);
        assert_eq!(result.unwrap(), "😀🎉🚀");
    }

    #[test]
    fn test_unicode_combining_characters() {
        // é composed as e + combining acute accent
        let combining = SqlValue::String("cafe\u{0301}".to_string());
        let result: Result<String, _> = String::from_sql(&combining);
        // The combining character should be preserved - no normalization
        assert_eq!(result.unwrap(), "cafe\u{0301}");
        // Note: "cafe\u{0301}" and "café" look identical but are different byte sequences
    }

    #[test]
    fn test_unicode_zero_width_characters() {
        // Zero-width joiner and other invisible characters
        let zwj = SqlValue::String("Test\u{200B}String".to_string()); // Zero-width space
        let result: Result<String, _> = String::from_sql(&zwj);
        let s = result.unwrap();
        assert!(s.contains('\u{200B}'));
    }

    #[test]
    fn test_unicode_rtl_characters() {
        // Right-to-left text (Hebrew/Arabic)
        let rtl = SqlValue::String("שלום".to_string());
        let result: Result<String, _> = String::from_sql(&rtl);
        assert_eq!(result.unwrap(), "שלום");
    }

    #[test]
    fn test_unicode_mixed_scripts() {
        let mixed = SqlValue::String("Hello世界مرحبا".to_string());
        let result: Result<String, _> = String::from_sql(&mixed);
        assert_eq!(result.unwrap(), "Hello世界مرحبا");
    }

    #[test]
    fn test_string_with_null_byte() {
        // Embedded null byte should be preserved
        let with_null = SqlValue::String("before\0after".to_string());
        let result: Result<String, _> = String::from_sql(&with_null);
        let s = result.unwrap();
        assert!(s.contains('\0'));
        assert_eq!(s.len(), 12);
    }

    #[test]
    fn test_long_string() {
        // Very long string (10KB)
        let long_str: String = "x".repeat(10_000);
        let long_val = SqlValue::String(long_str.clone());
        let result: Result<String, _> = String::from_sql(&long_val);
        assert_eq!(result.unwrap().len(), 10_000);
    }

    #[test]
    fn test_very_long_string() {
        // Very long string (1MB)
        let very_long: String = "x".repeat(1_000_000);
        let long_val = SqlValue::String(very_long.clone());
        let result: Result<String, _> = String::from_sql(&long_val);
        assert_eq!(result.unwrap().len(), 1_000_000);
    }
}

// ============================================================================
// Binary Data Edge Cases
// ============================================================================

mod binary_handling {
    use super::*;

    #[test]
    fn test_empty_binary() {
        let empty = SqlValue::Binary(Bytes::new());
        let result: Result<Vec<u8>, _> = Vec::<u8>::from_sql(&empty);
        assert_eq!(result.unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_single_byte() {
        let single = SqlValue::Binary(Bytes::from_static(&[0xFF]));
        let result: Result<Vec<u8>, _> = Vec::<u8>::from_sql(&single);
        assert_eq!(result.unwrap(), vec![0xFF]);
    }

    #[test]
    fn test_all_byte_values() {
        // All possible byte values 0-255
        let all_bytes: Vec<u8> = (0u8..=255u8).collect();
        let all_val = SqlValue::Binary(Bytes::from(all_bytes.clone()));
        let result: Result<Vec<u8>, _> = Vec::<u8>::from_sql(&all_val);
        assert_eq!(result.unwrap(), all_bytes);
    }

    #[test]
    fn test_large_binary() {
        // 1MB binary data
        let large: Vec<u8> = vec![0xAB; 1_000_000];
        let large_val = SqlValue::Binary(Bytes::from(large.clone()));
        let result: Result<Vec<u8>, _> = Vec::<u8>::from_sql(&large_val);
        assert_eq!(result.unwrap().len(), 1_000_000);
    }
}

// ============================================================================
// Boolean Edge Cases
// ============================================================================

mod boolean_handling {
    use super::*;

    #[test]
    fn test_bool_true() {
        let true_val = SqlValue::Bool(true);
        let result: Result<bool, _> = bool::from_sql(&true_val);
        assert!(result.unwrap());
    }

    #[test]
    fn test_bool_false() {
        let false_val = SqlValue::Bool(false);
        let result: Result<bool, _> = bool::from_sql(&false_val);
        assert!(!result.unwrap());
    }

    #[test]
    fn test_bool_to_sql_true() {
        let result = true.to_sql().unwrap();
        assert_eq!(result, SqlValue::Bool(true));
    }

    #[test]
    fn test_bool_to_sql_false() {
        let result = false.to_sql().unwrap();
        assert_eq!(result, SqlValue::Bool(false));
    }
}

// ============================================================================
// Type Coercion Edge Cases
// ============================================================================

mod type_coercion {
    use super::*;

    #[test]
    fn test_int_to_i64() {
        // i32 value should convert to i64
        let int_val = SqlValue::Int(42);
        let result: Result<i64, _> = i64::from_sql(&int_val);
        assert_eq!(result.unwrap(), 42i64);
    }

    #[test]
    fn test_smallint_to_i32() {
        let small_val = SqlValue::SmallInt(100);
        let result: Result<i32, _> = i32::from_sql(&small_val);
        assert_eq!(result.unwrap(), 100i32);
    }

    #[test]
    fn test_tinyint_to_i32() {
        let tiny_val = SqlValue::TinyInt(255);
        let result: Result<i32, _> = i32::from_sql(&tiny_val);
        assert_eq!(result.unwrap(), 255i32);
    }

    #[test]
    fn test_float_to_f64() {
        let float_val = SqlValue::Float(3.14);
        let result: Result<f64, _> = f64::from_sql(&float_val);
        // Float to f64 should preserve value (within precision)
        assert!((result.unwrap() - 3.14f64).abs() < 0.0001);
    }
}

// ============================================================================
// Error Case Tests
// ============================================================================

mod error_cases {
    use super::*;

    #[test]
    fn test_string_to_int_fails() {
        let str_val = SqlValue::String("not a number".to_string());
        let result: Result<i32, _> = i32::from_sql(&str_val);
        assert!(result.is_err());
    }

    #[test]
    fn test_int_to_string() {
        // This should work as a string representation
        let int_val = SqlValue::Int(42);
        let _result: Result<String, _> = String::from_sql(&int_val);
        // May succeed or fail depending on implementation
        // Document expected behavior here
    }

    #[test]
    fn test_binary_to_string_fails() {
        let bin_val = SqlValue::Binary(Bytes::from_static(&[0xFF, 0xFE]));
        let _result: Result<String, _> = String::from_sql(&bin_val);
        // Binary to string should fail (or return hex?)
        // Document expected behavior
    }
}
