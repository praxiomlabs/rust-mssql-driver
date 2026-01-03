//! Codec utilities for TDS protocol encoding and decoding.
//!
//! This module provides low-level encoding and decoding utilities used
//! throughout the TDS protocol implementation.

use bytes::{Buf, BufMut};

use crate::prelude::*;

/// Read a length-prefixed UTF-16LE string.
///
/// The format is: 1-byte length (in characters) followed by UTF-16LE bytes.
pub fn read_b_varchar(src: &mut impl Buf) -> Option<String> {
    if src.remaining() < 1 {
        return None;
    }
    let len = src.get_u8() as usize;
    read_utf16_string(src, len)
}

/// Read a length-prefixed UTF-16LE string with 2-byte length.
///
/// The format is: 2-byte length (in characters) followed by UTF-16LE bytes.
pub fn read_us_varchar(src: &mut impl Buf) -> Option<String> {
    if src.remaining() < 2 {
        return None;
    }
    let len = src.get_u16_le() as usize;
    read_utf16_string(src, len)
}

/// Read a UTF-16LE string of specified character length.
pub fn read_utf16_string(src: &mut impl Buf, char_count: usize) -> Option<String> {
    let byte_count = char_count * 2;
    if src.remaining() < byte_count {
        return None;
    }

    let mut chars = Vec::with_capacity(char_count);
    for _ in 0..char_count {
        chars.push(src.get_u16_le());
    }

    String::from_utf16(&chars).ok()
}

/// Write a length-prefixed UTF-16LE string (1-byte length).
pub fn write_b_varchar(dst: &mut impl BufMut, s: &str) {
    let chars: Vec<u16> = s.encode_utf16().collect();
    let len = chars.len().min(255) as u8;
    dst.put_u8(len);
    for &c in &chars[..len as usize] {
        dst.put_u16_le(c);
    }
}

/// Write a length-prefixed UTF-16LE string (2-byte length).
pub fn write_us_varchar(dst: &mut impl BufMut, s: &str) {
    let chars: Vec<u16> = s.encode_utf16().collect();
    let len = chars.len().min(65535) as u16;
    dst.put_u16_le(len);
    for &c in &chars[..len as usize] {
        dst.put_u16_le(c);
    }
}

/// Write a UTF-16LE string without length prefix.
pub fn write_utf16_string(dst: &mut impl BufMut, s: &str) {
    for c in s.encode_utf16() {
        dst.put_u16_le(c);
    }
}

/// Read a null-terminated ASCII string.
pub fn read_null_terminated_ascii(src: &mut impl Buf) -> Option<String> {
    let mut bytes = Vec::new();
    while src.has_remaining() {
        let b = src.get_u8();
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

/// Calculate the byte length of a UTF-16 encoded string.
#[must_use]
pub fn utf16_byte_len(s: &str) -> usize {
    s.encode_utf16().count() * 2
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_b_varchar_roundtrip() {
        let original = "Hello, 世界!";
        let mut buf = BytesMut::new();
        write_b_varchar(&mut buf, original);

        let mut cursor = buf.freeze();
        let decoded = read_b_varchar(&mut cursor).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_us_varchar_roundtrip() {
        let original = "Test string with Unicode: αβγ";
        let mut buf = BytesMut::new();
        write_us_varchar(&mut buf, original);

        let mut cursor = buf.freeze();
        let decoded = read_us_varchar(&mut cursor).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_utf16_byte_len() {
        assert_eq!(utf16_byte_len("Hello"), 10);
        assert_eq!(utf16_byte_len("世界"), 4);
    }
}
