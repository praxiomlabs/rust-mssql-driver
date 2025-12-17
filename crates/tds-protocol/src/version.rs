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

    /// Get the major version number.
    #[must_use]
    pub const fn major(self) -> u8 {
        if self.is_tds_8() {
            8
        } else {
            ((self.0 >> 24) & 0xFF) as u8
        }
    }

    /// Get the minor version number.
    #[must_use]
    pub const fn minor(self) -> u8 {
        if self.is_tds_8() {
            0
        } else {
            ((self.0 >> 16) & 0xFF) as u8
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
    }

    #[test]
    fn test_tds_8_detection() {
        assert!(TdsVersion::V8_0.is_tds_8());
        assert!(!TdsVersion::V7_4.is_tds_8());
    }

    #[test]
    fn test_prelogin_requirement() {
        assert!(TdsVersion::V7_4.requires_prelogin_encryption_negotiation());
        assert!(!TdsVersion::V8_0.requires_prelogin_encryption_negotiation());
    }
}
