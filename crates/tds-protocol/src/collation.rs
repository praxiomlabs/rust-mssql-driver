//! Collation encoding support for SQL Server VARCHAR decoding.
//!
//! This module provides mappings from SQL Server collation LCIDs (Locale IDs)
//! to their corresponding character encodings, enabling proper decoding of
//! non-UTF-8 VARCHAR data.
//!
//! # Supported Encodings
//!
//! The following encoding families are supported based on the collation's LCID:
//!
//! | Code Page | Encoding | Languages |
//! |-----------|----------|-----------|
//! | 874 | Windows-874 (TIS-620) | Thai |
//! | 932 | Shift_JIS | Japanese |
//! | 936 | GBK/GB18030 | Simplified Chinese |
//! | 949 | EUC-KR | Korean |
//! | 950 | Big5 | Traditional Chinese |
//! | 1250 | Windows-1250 | Central/Eastern European |
//! | 1251 | Windows-1251 | Cyrillic |
//! | 1252 | Windows-1252 | Western European (default) |
//! | 1253 | Windows-1253 | Greek |
//! | 1254 | Windows-1254 | Turkish |
//! | 1255 | Windows-1255 | Hebrew |
//! | 1256 | Windows-1256 | Arabic |
//! | 1257 | Windows-1257 | Baltic |
//! | 1258 | Windows-1258 | Vietnamese |
//!
//! # UTF-8 Collations
//!
//! SQL Server 2019+ supports UTF-8 collations (suffix `_UTF8`). These are
//! detected by checking the collation flags. When a UTF-8 collation is used,
//! no encoding conversion is needed as the data is already UTF-8.
//!
//! # References
//!
//! - [MS-LCID: Windows Language Code Identifier Reference](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/)
//! - [Code Page Identifiers](https://learn.microsoft.com/en-us/windows/win32/intl/code-page-identifiers)

#[cfg(feature = "encoding")]
use encoding_rs::Encoding;

/// Flag bit indicating UTF-8 collation (SQL Server 2019+).
/// This is bit 27 (0x0800_0000) in the collation info field.
pub const COLLATION_FLAG_UTF8: u32 = 0x0800_0000;

/// Mask to extract the primary LCID from the collation info.
/// The LCID is stored in the lower 20 bits.
pub const LCID_MASK: u32 = 0x000F_FFFF;

/// Mask to extract the primary language ID (lower 16 bits of LCID).
pub const PRIMARY_LANGUAGE_MASK: u32 = 0x0000_FFFF;

/// Returns whether the collation uses UTF-8 encoding.
///
/// SQL Server 2019+ supports UTF-8 collations with the `_UTF8` suffix.
/// These collations set bit 27 in the collation info field.
#[inline]
pub fn is_utf8_collation(lcid: u32) -> bool {
    lcid & COLLATION_FLAG_UTF8 != 0
}

/// Returns the encoding for a given LCID, if known.
///
/// This function maps SQL Server collation LCIDs to their corresponding
/// character encodings from the `encoding_rs` crate.
///
/// # Arguments
///
/// * `lcid` - The locale ID from the SQL Server collation
///
/// # Returns
///
/// * `Some(&Encoding)` - The corresponding encoding if the LCID is recognized
/// * `None` - If the LCID is not recognized or uses UTF-8
///
/// # UTF-8 Handling
///
/// UTF-8 collations (SQL Server 2019+) return `None` because no transcoding
/// is needed - the data is already valid UTF-8.
#[cfg(feature = "encoding")]
pub fn encoding_for_lcid(lcid: u32) -> Option<&'static Encoding> {
    // UTF-8 collations don't need transcoding
    if is_utf8_collation(lcid) {
        return None;
    }

    // Extract the primary language ID
    let primary_lang = lcid & PRIMARY_LANGUAGE_MASK;

    // Map LCID to encoding based on Windows code page assignments
    // Reference: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/
    match primary_lang {
        // Japanese (Code Page 932 - Shift_JIS)
        0x0411 => Some(encoding_rs::SHIFT_JIS),

        // Chinese - Simplified (Code Page 936 - GBK/GB18030)
        // Includes: zh-CN, zh-SG
        0x0804 | // Chinese (Simplified, PRC)
        0x1004   // Chinese (Simplified, Singapore)
        => Some(encoding_rs::GB18030),

        // Chinese - Traditional (Code Page 950 - Big5)
        // Includes: zh-TW, zh-HK, zh-MO
        0x0404 | // Chinese (Traditional, Taiwan)
        0x0C04 | // Chinese (Traditional, Hong Kong SAR)
        0x1404   // Chinese (Traditional, Macao SAR)
        => Some(encoding_rs::BIG5),

        // Korean (Code Page 949 - EUC-KR)
        0x0412 => Some(encoding_rs::EUC_KR),

        // Thai (Code Page 874 - Windows-874/TIS-620)
        0x041E => Some(encoding_rs::WINDOWS_874),

        // Vietnamese (Code Page 1258)
        0x042A => Some(encoding_rs::WINDOWS_1258),

        // Central/Eastern European (Code Page 1250)
        // Includes: Czech, Polish, Hungarian, Croatian, Slovak, Slovenian, Romanian, Albanian
        0x0405 | // Czech
        0x0415 | // Polish
        0x040E | // Hungarian
        0x041A | // Croatian
        0x081A | // Serbian (Latin)
        0x141A | // Bosnian (Latin)
        0x101A | // Croatian (Bosnia and Herzegovina)
        0x041B | // Slovak
        0x0424 | // Slovenian
        0x0418 | // Romanian
        0x041C   // Albanian
        => Some(encoding_rs::WINDOWS_1250),

        // Cyrillic (Code Page 1251)
        // Includes: Russian, Ukrainian, Belarusian, Bulgarian, Macedonian, Serbian Cyrillic
        0x0419 | // Russian
        0x0422 | // Ukrainian
        0x0423 | // Belarusian
        0x0402 | // Bulgarian
        0x042F | // Macedonian
        0x0C1A | // Serbian (Cyrillic)
        0x201A | // Bosnian (Cyrillic)
        0x0440 | // Kyrgyz
        0x0843 | // Uzbek (Cyrillic)
        0x0444 | // Tatar
        0x0450 | // Mongolian (Cyrillic)
        0x0485   // Sakha
        => Some(encoding_rs::WINDOWS_1251),

        // Greek (Code Page 1253)
        0x0408 => Some(encoding_rs::WINDOWS_1253),

        // Turkish (Code Page 1254)
        0x041F | // Turkish
        0x042C   // Azerbaijani (Latin)
        => Some(encoding_rs::WINDOWS_1254),

        // Hebrew (Code Page 1255)
        0x040D => Some(encoding_rs::WINDOWS_1255),

        // Arabic (Code Page 1256)
        // Includes all Arabic variants and Farsi/Persian, Urdu, etc.
        0x0401 | // Arabic (Saudi Arabia)
        0x0801 | // Arabic (Iraq)
        0x0C01 | // Arabic (Egypt)
        0x1001 | // Arabic (Libya)
        0x1401 | // Arabic (Algeria)
        0x1801 | // Arabic (Morocco)
        0x1C01 | // Arabic (Tunisia)
        0x2001 | // Arabic (Oman)
        0x2401 | // Arabic (Yemen)
        0x2801 | // Arabic (Syria)
        0x2C01 | // Arabic (Jordan)
        0x3001 | // Arabic (Lebanon)
        0x3401 | // Arabic (Kuwait)
        0x3801 | // Arabic (UAE)
        0x3C01 | // Arabic (Bahrain)
        0x4001 | // Arabic (Qatar)
        0x0429 | // Farsi/Persian
        0x0420 | // Urdu
        0x048C | // Dari
        0x0463   // Pashto
        => Some(encoding_rs::WINDOWS_1256),

        // Baltic (Code Page 1257)
        0x0425..=0x0427   // Lithuanian
        => Some(encoding_rs::WINDOWS_1257),

        // Western European (Code Page 1252) - Default for most European languages
        // Includes: English, French, German, Spanish, Italian, Portuguese, Dutch, etc.
        0x0409 | // English (United States)
        0x0809 | // English (United Kingdom)
        0x0C09 | // English (Australia)
        0x1009 | // English (Canada)
        0x1409 | // English (New Zealand)
        0x1809 | // English (Ireland)
        0x040C | // French (France)
        0x080C | // French (Belgium)
        0x0C0C | // French (Canada)
        0x100C | // French (Switzerland)
        0x140C | // French (Luxembourg)
        0x0407 | // German (Germany)
        0x0807 | // German (Switzerland)
        0x0C07 | // German (Austria)
        0x1007 | // German (Luxembourg)
        0x1407 | // German (Liechtenstein)
        0x040A | // Spanish (Traditional Sort)
        0x080A | // Spanish (Mexico)
        0x0C0A | // Spanish (Modern Sort)
        0x100A | // Spanish (Guatemala)
        0x140A | // Spanish (Costa Rica)
        0x180A | // Spanish (Panama)
        0x1C0A | // Spanish (Dominican Republic)
        0x200A | // Spanish (Venezuela)
        0x240A | // Spanish (Colombia)
        0x280A | // Spanish (Peru)
        0x2C0A | // Spanish (Argentina)
        0x300A | // Spanish (Ecuador)
        0x340A | // Spanish (Chile)
        0x380A | // Spanish (Uruguay)
        0x3C0A | // Spanish (Paraguay)
        0x400A | // Spanish (Bolivia)
        0x440A | // Spanish (El Salvador)
        0x480A | // Spanish (Honduras)
        0x4C0A | // Spanish (Nicaragua)
        0x500A | // Spanish (Puerto Rico)
        0x0410 | // Italian (Italy)
        0x0810 | // Italian (Switzerland)
        0x0816 | // Portuguese (Portugal)
        0x0416 | // Portuguese (Brazil)
        0x0413 | // Dutch (Netherlands)
        0x0813 | // Dutch (Belgium)
        0x0406 | // Danish
        0x0414 | // Norwegian (Bokmål)
        0x0814 | // Norwegian (Nynorsk)
        0x041D | // Swedish
        0x081D | // Swedish (Finland)
        0x040B | // Finnish
        0x040F | // Icelandic
        0x0403 | // Catalan
        0x0456 | // Galician
        0x042D | // Basque
        0x0436 | // Afrikaans
        0x0421 | // Indonesian
        0x043E | // Malay (Malaysia)
        0x0441   // Swahili
        => Some(encoding_rs::WINDOWS_1252),

        // Unknown LCID - return None, caller should use Windows-1252 as fallback
        _ => None,
    }
}

/// Returns the Windows code page number for a given LCID.
///
/// This is useful for error messages and debugging.
#[cfg(feature = "encoding")]
pub fn code_page_for_lcid(lcid: u32) -> Option<u16> {
    if is_utf8_collation(lcid) {
        return Some(65001); // UTF-8
    }

    let primary_lang = lcid & PRIMARY_LANGUAGE_MASK;

    match primary_lang {
        0x0411 => Some(932),                   // Japanese - Shift_JIS
        0x0804 | 0x1004 => Some(936),          // Chinese Simplified - GBK
        0x0404 | 0x0C04 | 0x1404 => Some(950), // Chinese Traditional - Big5
        0x0412 => Some(949),                   // Korean - EUC-KR
        0x041E => Some(874),                   // Thai
        0x042A => Some(1258),                  // Vietnamese

        // Code Page 1250 - Central European
        0x0405 | 0x0415 | 0x040E | 0x041A | 0x081A | 0x141A | 0x101A | 0x041B | 0x0424 | 0x0418
        | 0x041C => Some(1250),

        // Code Page 1251 - Cyrillic
        0x0419 | 0x0422 | 0x0423 | 0x0402 | 0x042F | 0x0C1A | 0x201A | 0x0440 | 0x0843 | 0x0444
        | 0x0450 | 0x0485 => Some(1251),

        0x0408 => Some(1253),          // Greek
        0x041F | 0x042C => Some(1254), // Turkish, Azerbaijani
        0x040D => Some(1255),          // Hebrew

        // Code Page 1256 - Arabic
        0x0401 | 0x0801 | 0x0C01 | 0x1001 | 0x1401 | 0x1801 | 0x1C01 | 0x2001 | 0x2401 | 0x2801
        | 0x2C01 | 0x3001 | 0x3401 | 0x3801 | 0x3C01 | 0x4001 | 0x0429 | 0x0420 | 0x048C
        | 0x0463 => Some(1256),

        // Code Page 1257 - Baltic
        0x0425..=0x0427 => Some(1257),

        // Default to Code Page 1252 for Western European
        _ => Some(1252),
    }
}

/// Returns the encoding name for display/logging purposes.
#[cfg(feature = "encoding")]
pub fn encoding_name_for_lcid(lcid: u32) -> &'static str {
    if is_utf8_collation(lcid) {
        return "UTF-8";
    }

    match encoding_for_lcid(lcid) {
        Some(enc) => enc.name(),
        None => "windows-1252", // Default fallback
    }
}

#[cfg(all(test, feature = "encoding"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_detection() {
        // UTF-8 collation flag
        assert!(is_utf8_collation(0x0800_0409)); // English with UTF-8
        assert!(!is_utf8_collation(0x0409)); // English without UTF-8
    }

    #[test]
    fn test_japanese_encoding() {
        let enc = encoding_for_lcid(0x0411);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "Shift_JIS");
        assert_eq!(code_page_for_lcid(0x0411), Some(932));
    }

    #[test]
    fn test_chinese_simplified_encoding() {
        let enc = encoding_for_lcid(0x0804);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "gb18030");
        assert_eq!(code_page_for_lcid(0x0804), Some(936));
    }

    #[test]
    fn test_chinese_traditional_encoding() {
        let enc = encoding_for_lcid(0x0404);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "Big5");
        assert_eq!(code_page_for_lcid(0x0404), Some(950));
    }

    #[test]
    fn test_korean_encoding() {
        let enc = encoding_for_lcid(0x0412);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "EUC-KR");
        assert_eq!(code_page_for_lcid(0x0412), Some(949));
    }

    #[test]
    fn test_cyrillic_encoding() {
        // Russian
        let enc = encoding_for_lcid(0x0419);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1251");
        assert_eq!(code_page_for_lcid(0x0419), Some(1251));

        // Ukrainian
        let enc = encoding_for_lcid(0x0422);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1251");
    }

    #[test]
    fn test_western_european_encoding() {
        // English (US)
        let enc = encoding_for_lcid(0x0409);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1252");
        assert_eq!(code_page_for_lcid(0x0409), Some(1252));

        // French
        let enc = encoding_for_lcid(0x040C);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1252");

        // German
        let enc = encoding_for_lcid(0x0407);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1252");
    }

    #[test]
    fn test_greek_encoding() {
        let enc = encoding_for_lcid(0x0408);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1253");
        assert_eq!(code_page_for_lcid(0x0408), Some(1253));
    }

    #[test]
    fn test_turkish_encoding() {
        let enc = encoding_for_lcid(0x041F);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1254");
        assert_eq!(code_page_for_lcid(0x041F), Some(1254));
    }

    #[test]
    fn test_hebrew_encoding() {
        let enc = encoding_for_lcid(0x040D);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1255");
        assert_eq!(code_page_for_lcid(0x040D), Some(1255));
    }

    #[test]
    fn test_arabic_encoding() {
        // Arabic (Saudi Arabia)
        let enc = encoding_for_lcid(0x0401);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1256");
        assert_eq!(code_page_for_lcid(0x0401), Some(1256));

        // Farsi/Persian
        let enc = encoding_for_lcid(0x0429);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1256");
    }

    #[test]
    fn test_baltic_encoding() {
        // Estonian
        let enc = encoding_for_lcid(0x0425);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1257");
        assert_eq!(code_page_for_lcid(0x0425), Some(1257));

        // Lithuanian
        let enc = encoding_for_lcid(0x0427);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1257");
    }

    #[test]
    fn test_thai_encoding() {
        let enc = encoding_for_lcid(0x041E);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-874");
        assert_eq!(code_page_for_lcid(0x041E), Some(874));
    }

    #[test]
    fn test_vietnamese_encoding() {
        let enc = encoding_for_lcid(0x042A);
        assert!(enc.is_some());
        assert_eq!(enc.unwrap().name(), "windows-1258");
        assert_eq!(code_page_for_lcid(0x042A), Some(1258));
    }

    #[test]
    fn test_unknown_lcid_fallback() {
        // Unknown LCID should return None (caller uses Windows-1252)
        let enc = encoding_for_lcid(0x9999);
        assert!(enc.is_none());
        // But code page should default to 1252
        assert_eq!(code_page_for_lcid(0x9999), Some(1252));
    }

    #[test]
    fn test_encoding_name() {
        assert_eq!(encoding_name_for_lcid(0x0411), "Shift_JIS");
        assert_eq!(encoding_name_for_lcid(0x0419), "windows-1251");
        assert_eq!(encoding_name_for_lcid(0x0800_0409), "UTF-8");
        assert_eq!(encoding_name_for_lcid(0x9999), "windows-1252"); // fallback
    }

    #[test]
    fn test_decode_chinese_text() {
        let enc = encoding_for_lcid(0x0804).unwrap();
        // "中文" in GB18030 encoding
        let gb_bytes = [0xD6, 0xD0, 0xCE, 0xC4];
        let (decoded, _, had_errors) = enc.decode(&gb_bytes);
        assert!(!had_errors);
        assert_eq!(decoded, "中文");
    }

    #[test]
    fn test_decode_cyrillic_text() {
        let enc = encoding_for_lcid(0x0419).unwrap();
        // "Привет" in Windows-1251
        let cp1251_bytes = [0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2];
        let (decoded, _, had_errors) = enc.decode(&cp1251_bytes);
        assert!(!had_errors);
        assert_eq!(decoded, "Привет");
    }

    #[test]
    fn test_decode_japanese_text() {
        let enc = encoding_for_lcid(0x0411).unwrap();
        // "日本語" in Shift_JIS
        let sjis_bytes = [0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA];
        let (decoded, _, had_errors) = enc.decode(&sjis_bytes);
        assert!(!had_errors);
        assert_eq!(decoded, "日本語");
    }
}
