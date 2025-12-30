//! TDS protocol version definitions.

use core::fmt;

/// TDS protocol version.
///
/// Represents the version of the TDS protocol used for communication
/// with SQL Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TdsVersion(u32);

impl TdsVersion {
    /// TDS 7.0 (SQL Server 7.0)
    pub const V7_0: Self = Self(0x70000000);

    /// TDS 7.1 (SQL Server 2000)
    pub const V7_1: Self = Self(0x71000000);

    /// TDS 7.1 Revision 1 (SQL Server 2000 SP1)
    pub const V7_1_REV1: Self = Self(0x71000001);

    /// TDS 7.2 (SQL Server 2005)
    pub const V7_2: Self = Self(0x72090002);

    /// TDS 7.3A (SQL Server 2008)
    pub const V7_3A: Self = Self(0x730A0003);

    /// TDS 7.3B (SQL Server 2008 R2)
    pub const V7_3B: Self = Self(0x730B0003);

    /// TDS 7.4 (SQL Server 2012+)
    pub const V7_4: Self = Self(0x74000004);

    /// TDS 8.0 (SQL Server 2022+ strict encryption mode)
    pub const V8_0: Self = Self(0x08000000);

    /// Create a new TDS version from raw bytes.
    #[must_use]
    pub const fn new(version: u32) -> Self {
        Self(version)
    }

    /// Get the raw version value.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this version supports TDS 8.0 strict encryption.
    #[must_use]
    pub const fn is_tds_8(self) -> bool {
        // TDS 8.0 uses a different version format
        self.0 == Self::V8_0.0
    }

    /// Check if this version requires pre-login encryption negotiation.
    ///
    /// TDS 7.x versions negotiate encryption during pre-login.
    /// TDS 8.0 requires TLS before any TDS traffic.
    #[must_use]
    pub const fn requires_prelogin_encryption_negotiation(self) -> bool {
        !self.is_tds_8()
    }

    /// Check if this version is TDS 7.3 (SQL Server 2008/2008 R2).
    ///
    /// Returns true for both TDS 7.3A (SQL Server 2008) and TDS 7.3B (SQL Server 2008 R2).
    #[must_use]
    pub const fn is_tds_7_3(self) -> bool {
        self.0 == Self::V7_3A.0 || self.0 == Self::V7_3B.0
    }

    /// Check if this version is TDS 7.4 (SQL Server 2012+).
    #[must_use]
    pub const fn is_tds_7_4(self) -> bool {
        self.0 == Self::V7_4.0
    }

    /// Check if this version supports DATE, TIME, DATETIME2, and DATETIMEOFFSET types.
    ///
    /// These types were introduced in TDS 7.3 (SQL Server 2008).
    /// Returns true for TDS 7.3+, TDS 7.4, and TDS 8.0.
    #[must_use]
    pub const fn supports_date_time_types(self) -> bool {
        // TDS 7.3A is 0x730A0003, TDS 7.4 is 0x74000004, TDS 8.0 is 0x08000000
        // Due to TDS 8.0's different encoding, we check explicitly
        self.is_tds_8() || self.0 >= Self::V7_3A.0
    }

    /// Check if this version supports session recovery (connection resiliency).
    ///
    /// Session recovery was introduced in TDS 7.4 (SQL Server 2012).
    #[must_use]
    pub const fn supports_session_recovery(self) -> bool {
        self.is_tds_8() || self.0 >= Self::V7_4.0
    }

    /// Check if this version supports column encryption (Always Encrypted).
    ///
    /// Column encryption was introduced in SQL Server 2016 (still TDS 7.4).
    /// This checks protocol capability, not SQL Server version.
    #[must_use]
    pub const fn supports_column_encryption(self) -> bool {
        // Column encryption is a feature extension available in TDS 7.4+
        self.is_tds_8() || self.0 >= Self::V7_4.0
    }

    /// Check if this version supports UTF-8 (introduced in SQL Server 2019).
    #[must_use]
    pub const fn supports_utf8(self) -> bool {
        self.is_tds_8() || self.0 >= Self::V7_4.0
    }

    /// Check if this is a legacy version (TDS 7.2 or earlier).
    ///
    /// Legacy versions (SQL Server 2005 and earlier) have different behaviors
    /// for some protocol aspects. This driver's minimum supported version is
    /// TDS 7.3 for full functionality.
    #[must_use]
    pub const fn is_legacy(self) -> bool {
        // V7_2 is 0x72090002, anything less than V7_3A is legacy
        !self.is_tds_8() && self.0 < Self::V7_3A.0
    }

    /// Get the minimum version between this version and another.
    ///
    /// Useful for version negotiation where the client and server
    /// agree on the lowest common version.
    ///
    /// Note: TDS 8.0 uses a different encoding (0x08000000) which is numerically
    /// lower than TDS 7.x versions, but semantically higher. This method handles
    /// that special case correctly.
    #[must_use]
    pub const fn min(self, other: Self) -> Self {
        // Special handling for TDS 8.0 which has a different encoding
        // TDS 8.0 (0x08000000) is numerically lower but semantically higher than TDS 7.x
        if self.is_tds_8() && !other.is_tds_8() {
            // self is TDS 8.0, other is TDS 7.x - return TDS 7.x as the "lower" version
            other
        } else if !self.is_tds_8() && other.is_tds_8() {
            // self is TDS 7.x, other is TDS 8.0 - return TDS 7.x as the "lower" version
            self
        } else if self.0 <= other.0 {
            // Both are same type (both 7.x or both 8.0), compare numerically
            self
        } else {
            other
        }
    }

    /// Get the SQL Server version name for this TDS version.
    ///
    /// Returns a human-readable string describing the SQL Server version
    /// that corresponds to this TDS protocol version.
    #[must_use]
    pub const fn sql_server_version_name(&self) -> &'static str {
        match self.0 {
            0x70000000 => "SQL Server 7.0",
            0x71000000 | 0x71000001 => "SQL Server 2000",
            0x72090002 => "SQL Server 2005",
            0x730A0003 => "SQL Server 2008",
            0x730B0003 => "SQL Server 2008 R2",
            0x74000004 => "SQL Server 2012+",
            0x08000000 => "SQL Server 2022+ (strict mode)",
            _ => "Unknown SQL Server version",
        }
    }

    /// Parse a TDS version from a string representation.
    ///
    /// Accepts formats like:
    /// - "7.3", "7.3A", "7.3a", "7.3B", "7.3b" for TDS 7.3
    /// - "7.4" for TDS 7.4
    /// - "8.0", "8" for TDS 8.0
    ///
    /// Returns None if the string cannot be parsed.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "7.0" => Some(Self::V7_0),
            "7.1" => Some(Self::V7_1),
            "7.2" => Some(Self::V7_2),
            "7.3" | "7.3a" => Some(Self::V7_3A),
            "7.3b" => Some(Self::V7_3B),
            "7.4" => Some(Self::V7_4),
            "8.0" | "8" => Some(Self::V8_0),
            _ => None,
        }
    }

    /// Get the major version number.
    ///
    /// Returns 7 for TDS 7.x versions, 8 for TDS 8.0.
    ///
    /// Note: This extracts the major version from the wire format. All TDS 7.x
    /// versions return 7, and TDS 8.0 returns 8.
    #[must_use]
    pub const fn major(self) -> u8 {
        if self.is_tds_8() {
            8
        } else {
            // TDS 7.x versions encode major version in high nibble of first byte
            // 0x7X... where X encodes the sub-version (0, 1, 2, 3, 4)
            7
        }
    }

    /// Get the minor version number.
    ///
    /// Returns the TDS sub-version: 0, 1, 2, 3, or 4 for TDS 7.x, and 0 for TDS 8.0.
    ///
    /// Note: The wire format uses different encoding for different versions.
    /// This method extracts the logical minor version (e.g., 3 for TDS 7.3).
    #[must_use]
    pub const fn minor(self) -> u8 {
        match self.0 {
            0x70000000 => 0,              // TDS 7.0
            0x71000000 | 0x71000001 => 1, // TDS 7.1, 7.1 Rev 1
            0x72090002 => 2,              // TDS 7.2
            0x730A0003 | 0x730B0003 => 3, // TDS 7.3A, 7.3B
            0x74000004 => 4,              // TDS 7.4
            0x08000000 => 0,              // TDS 8.0
            _ => {
                // For unknown versions, extract from first byte's low nibble
                // This is a best-effort fallback
                ((self.0 >> 24) & 0x0F) as u8
            }
        }
    }

    /// Get the revision suffix for TDS 7.3 versions.
    ///
    /// Returns Some('A') for TDS 7.3A (SQL Server 2008),
    /// Some('B') for TDS 7.3B (SQL Server 2008 R2),
    /// and None for all other versions.
    #[must_use]
    pub const fn revision_suffix(self) -> Option<char> {
        match self.0 {
            0x730A0003 => Some('A'),
            0x730B0003 => Some('B'),
            _ => None,
        }
    }
}

impl Default for TdsVersion {
    fn default() -> Self {
        Self::V7_4
    }
}

impl fmt::Display for TdsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_tds_8() {
            write!(f, "TDS 8.0")
        } else if let Some(suffix) = self.revision_suffix() {
            // TDS 7.3A or 7.3B
            write!(f, "TDS {}.{}{}", self.major(), self.minor(), suffix)
        } else {
            write!(f, "TDS {}.{}", self.major(), self.minor())
        }
    }
}

impl From<u32> for TdsVersion {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<TdsVersion> for u32 {
    fn from(version: TdsVersion) -> Self {
        version.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(TdsVersion::V7_4 > TdsVersion::V7_3B);
        assert!(TdsVersion::V7_3B > TdsVersion::V7_3A);
        assert!(TdsVersion::V7_3A > TdsVersion::V7_2);
    }

    #[test]
    fn test_tds_8_detection() {
        assert!(TdsVersion::V8_0.is_tds_8());
        assert!(!TdsVersion::V7_4.is_tds_8());
        assert!(!TdsVersion::V7_3A.is_tds_8());
    }

    #[test]
    fn test_prelogin_requirement() {
        assert!(TdsVersion::V7_4.requires_prelogin_encryption_negotiation());
        assert!(TdsVersion::V7_3A.requires_prelogin_encryption_negotiation());
        assert!(TdsVersion::V7_3B.requires_prelogin_encryption_negotiation());
        assert!(!TdsVersion::V8_0.requires_prelogin_encryption_negotiation());
    }

    #[test]
    fn test_is_tds_7_3() {
        assert!(TdsVersion::V7_3A.is_tds_7_3());
        assert!(TdsVersion::V7_3B.is_tds_7_3());
        assert!(!TdsVersion::V7_4.is_tds_7_3());
        assert!(!TdsVersion::V7_2.is_tds_7_3());
        assert!(!TdsVersion::V8_0.is_tds_7_3());
    }

    #[test]
    fn test_is_tds_7_4() {
        assert!(TdsVersion::V7_4.is_tds_7_4());
        assert!(!TdsVersion::V7_3A.is_tds_7_4());
        assert!(!TdsVersion::V7_3B.is_tds_7_4());
        assert!(!TdsVersion::V8_0.is_tds_7_4());
    }

    #[test]
    fn test_supports_date_time_types() {
        // TDS 7.3+ supports DATE, TIME, DATETIME2, DATETIMEOFFSET
        assert!(TdsVersion::V7_3A.supports_date_time_types());
        assert!(TdsVersion::V7_3B.supports_date_time_types());
        assert!(TdsVersion::V7_4.supports_date_time_types());
        assert!(TdsVersion::V8_0.supports_date_time_types());
        // TDS 7.2 and earlier don't support these types
        assert!(!TdsVersion::V7_2.supports_date_time_types());
        assert!(!TdsVersion::V7_1.supports_date_time_types());
    }

    #[test]
    fn test_supports_session_recovery() {
        // Session recovery was introduced in TDS 7.4
        assert!(TdsVersion::V7_4.supports_session_recovery());
        assert!(TdsVersion::V8_0.supports_session_recovery());
        assert!(!TdsVersion::V7_3A.supports_session_recovery());
        assert!(!TdsVersion::V7_3B.supports_session_recovery());
    }

    #[test]
    fn test_is_legacy() {
        assert!(TdsVersion::V7_2.is_legacy());
        assert!(TdsVersion::V7_1.is_legacy());
        assert!(TdsVersion::V7_0.is_legacy());
        assert!(!TdsVersion::V7_3A.is_legacy());
        assert!(!TdsVersion::V7_3B.is_legacy());
        assert!(!TdsVersion::V7_4.is_legacy());
        assert!(!TdsVersion::V8_0.is_legacy());
    }

    #[test]
    fn test_min_version() {
        assert_eq!(TdsVersion::V7_4.min(TdsVersion::V7_3A), TdsVersion::V7_3A);
        assert_eq!(TdsVersion::V7_3A.min(TdsVersion::V7_4), TdsVersion::V7_3A);
        assert_eq!(TdsVersion::V7_3A.min(TdsVersion::V7_3B), TdsVersion::V7_3A);
        // TDS 8.0 has special handling
        assert_eq!(TdsVersion::V8_0.min(TdsVersion::V7_4), TdsVersion::V7_4);
        assert_eq!(TdsVersion::V7_4.min(TdsVersion::V8_0), TdsVersion::V7_4);
    }

    #[test]
    fn test_sql_server_version_name() {
        assert_eq!(
            TdsVersion::V7_3A.sql_server_version_name(),
            "SQL Server 2008"
        );
        assert_eq!(
            TdsVersion::V7_3B.sql_server_version_name(),
            "SQL Server 2008 R2"
        );
        assert_eq!(
            TdsVersion::V7_4.sql_server_version_name(),
            "SQL Server 2012+"
        );
        assert_eq!(
            TdsVersion::V8_0.sql_server_version_name(),
            "SQL Server 2022+ (strict mode)"
        );
    }

    #[test]
    fn test_parse() {
        assert_eq!(TdsVersion::parse("7.3"), Some(TdsVersion::V7_3A));
        assert_eq!(TdsVersion::parse("7.3a"), Some(TdsVersion::V7_3A));
        assert_eq!(TdsVersion::parse("7.3A"), Some(TdsVersion::V7_3A));
        assert_eq!(TdsVersion::parse("7.3b"), Some(TdsVersion::V7_3B));
        assert_eq!(TdsVersion::parse("7.3B"), Some(TdsVersion::V7_3B));
        assert_eq!(TdsVersion::parse("7.4"), Some(TdsVersion::V7_4));
        assert_eq!(TdsVersion::parse("8.0"), Some(TdsVersion::V8_0));
        assert_eq!(TdsVersion::parse("8"), Some(TdsVersion::V8_0));
        assert_eq!(TdsVersion::parse(" 7.4 "), Some(TdsVersion::V7_4)); // Whitespace handling
        assert_eq!(TdsVersion::parse("invalid"), None);
        assert_eq!(TdsVersion::parse("9.0"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", TdsVersion::V7_0), "TDS 7.0");
        assert_eq!(format!("{}", TdsVersion::V7_1), "TDS 7.1");
        assert_eq!(format!("{}", TdsVersion::V7_2), "TDS 7.2");
        assert_eq!(format!("{}", TdsVersion::V7_3A), "TDS 7.3A");
        assert_eq!(format!("{}", TdsVersion::V7_3B), "TDS 7.3B");
        assert_eq!(format!("{}", TdsVersion::V7_4), "TDS 7.4");
        assert_eq!(format!("{}", TdsVersion::V8_0), "TDS 8.0");
    }

    #[test]
    fn test_major_minor() {
        // All TDS 7.x versions have major = 7
        assert_eq!(TdsVersion::V7_0.major(), 7);
        assert_eq!(TdsVersion::V7_1.major(), 7);
        assert_eq!(TdsVersion::V7_2.major(), 7);
        assert_eq!(TdsVersion::V7_3A.major(), 7);
        assert_eq!(TdsVersion::V7_3B.major(), 7);
        assert_eq!(TdsVersion::V7_4.major(), 7);
        assert_eq!(TdsVersion::V8_0.major(), 8);

        // Minor version extracts the logical sub-version
        assert_eq!(TdsVersion::V7_0.minor(), 0);
        assert_eq!(TdsVersion::V7_1.minor(), 1);
        assert_eq!(TdsVersion::V7_2.minor(), 2);
        assert_eq!(TdsVersion::V7_3A.minor(), 3);
        assert_eq!(TdsVersion::V7_3B.minor(), 3);
        assert_eq!(TdsVersion::V7_4.minor(), 4);
        assert_eq!(TdsVersion::V8_0.minor(), 0);
    }

    #[test]
    fn test_revision_suffix() {
        assert_eq!(TdsVersion::V7_0.revision_suffix(), None);
        assert_eq!(TdsVersion::V7_1.revision_suffix(), None);
        assert_eq!(TdsVersion::V7_2.revision_suffix(), None);
        assert_eq!(TdsVersion::V7_3A.revision_suffix(), Some('A'));
        assert_eq!(TdsVersion::V7_3B.revision_suffix(), Some('B'));
        assert_eq!(TdsVersion::V7_4.revision_suffix(), None);
        assert_eq!(TdsVersion::V8_0.revision_suffix(), None);
    }
}
