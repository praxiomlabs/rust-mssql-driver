//! Type conversion error types.

use thiserror::Error;

/// Errors that can occur during type conversion.
#[derive(Debug, Error)]
pub enum TypeError {
    /// Value is null when non-null was expected.
    #[error("unexpected null value")]
    UnexpectedNull,

    /// Type mismatch during conversion.
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        /// Expected type name.
        expected: &'static str,
        /// Actual type name.
        actual: String,
    },

    /// Value is out of range for target type.
    #[error("value out of range for {target_type}")]
    OutOfRange {
        /// Target type name.
        target_type: &'static str,
    },

    /// Invalid encoding in string data.
    #[error("invalid string encoding: {0}")]
    InvalidEncoding(String),

    /// Invalid binary data.
    #[error("invalid binary data: {0}")]
    InvalidBinary(String),

    /// Invalid date/time value.
    #[error("invalid date/time: {0}")]
    InvalidDateTime(String),

    /// Invalid decimal value.
    #[error("invalid decimal: {0}")]
    InvalidDecimal(String),

    /// Invalid UUID value.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),

    /// Truncation occurred during conversion.
    #[error("value truncated: {0}")]
    Truncation(String),

    /// Unsupported type conversion.
    #[error("unsupported conversion from {from} to {to}")]
    UnsupportedConversion {
        /// Source type.
        from: String,
        /// Target type.
        to: &'static str,
    },

    /// Buffer too small for value.
    #[error("buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall {
        /// Bytes needed.
        needed: usize,
        /// Bytes available.
        available: usize,
    },
}
