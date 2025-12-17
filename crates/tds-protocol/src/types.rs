//! TDS data type definitions.
//!
//! This module defines the SQL Server data types as they appear in the TDS protocol.

/// TDS data type identifiers.
///
/// These correspond to the type bytes sent in column metadata and parameter definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TypeId {
    // Fixed-length types (no length prefix)
    /// Null type.
    Null = 0x1F,
    /// 8-bit signed integer.
    Int1 = 0x30,
    /// Bit (boolean).
    Bit = 0x32,
    /// 16-bit signed integer.
    Int2 = 0x34,
    /// 32-bit signed integer.
    Int4 = 0x38,
    /// 64-bit signed integer.
    Int8 = 0x7F,
    /// 4-byte datetime.
    DateTimeN = 0x6F,
    /// 32-bit floating point.
    Float4 = 0x3B,
    /// 64-bit floating point.
    Float8 = 0x3E,
    /// 8-byte money.
    Money = 0x3C,
    /// 4-byte money.
    Money4 = 0x7A,
    /// 4-byte datetime.
    DateTime = 0x3D,
    /// 4-byte small datetime.
    DateTime4 = 0x3A,

    // Variable-length types (with length prefix)
    /// Variable-length GUID.
    Guid = 0x24,
    /// Variable-length integer.
    IntN = 0x26,
    /// Variable-length decimal.
    Decimal = 0x37,
    /// Variable-length numeric.
    Numeric = 0x3F,
    /// Variable-length bit.
    BitN = 0x68,
    /// Variable-length decimal (newer).
    DecimalN = 0x6A,
    /// Variable-length numeric (newer).
    NumericN = 0x6C,
    /// Variable-length float.
    FloatN = 0x6D,
    /// Variable-length money.
    MoneyN = 0x6E,

    // Byte-counted types
    /// Fixed-length character.
    Char = 0x2F,
    /// Variable-length character.
    VarChar = 0x27,
    /// Fixed-length binary.
    Binary = 0x2D,
    /// Variable-length binary.
    VarBinary = 0x25,

    // Counted types with 2-byte length
    /// Large variable-length character.
    BigVarChar = 0xA7,
    /// Large variable-length binary.
    BigVarBinary = 0xA5,
    /// Large fixed-length character.
    BigChar = 0xAF,
    /// Large fixed-length binary.
    BigBinary = 0xAD,

    // Unicode types
    /// Fixed-length Unicode character.
    NChar = 0xEF,
    /// Variable-length Unicode character.
    NVarChar = 0xE7,

    // Large object types (PLP - Partially Length-Prefixed)
    /// Text (deprecated, use varchar(max)).
    Text = 0x23,
    /// Image (deprecated, use varbinary(max)).
    Image = 0x22,
    /// NText (deprecated, use nvarchar(max)).
    NText = 0x63,

    // Date/time types (SQL Server 2008+)
    /// Date (3 bytes).
    Date = 0x28,
    /// Time with variable precision.
    Time = 0x29,
    /// DateTime2 with variable precision.
    DateTime2 = 0x2A,
    /// DateTimeOffset with variable precision.
    DateTimeOffset = 0x2B,

    // Special types
    /// SQL Variant.
    Variant = 0x62,
    /// User-defined type.
    Udt = 0xF0,
    /// XML type.
    Xml = 0xF1,
    /// Table-valued parameter.
    Tvp = 0xF3,
}

impl TypeId {
    /// Create a type ID from a raw byte.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x1F => Some(Self::Null),
            0x30 => Some(Self::Int1),
            0x32 => Some(Self::Bit),
            0x34 => Some(Self::Int2),
            0x38 => Some(Self::Int4),
            0x7F => Some(Self::Int8),
            0x6F => Some(Self::DateTimeN),
            0x3B => Some(Self::Float4),
            0x3E => Some(Self::Float8),
            0x3C => Some(Self::Money),
            0x7A => Some(Self::Money4),
            0x3D => Some(Self::DateTime),
            0x3A => Some(Self::DateTime4),
            0x24 => Some(Self::Guid),
            0x26 => Some(Self::IntN),
            0x37 => Some(Self::Decimal),
            0x3F => Some(Self::Numeric),
            0x68 => Some(Self::BitN),
            0x6A => Some(Self::DecimalN),
            0x6C => Some(Self::NumericN),
            0x6D => Some(Self::FloatN),
            0x6E => Some(Self::MoneyN),
            0x2F => Some(Self::Char),
            0x27 => Some(Self::VarChar),
            0x2D => Some(Self::Binary),
            0x25 => Some(Self::VarBinary),
            0xA7 => Some(Self::BigVarChar),
            0xA5 => Some(Self::BigVarBinary),
            0xAF => Some(Self::BigChar),
            0xAD => Some(Self::BigBinary),
            0xEF => Some(Self::NChar),
            0xE7 => Some(Self::NVarChar),
            0x23 => Some(Self::Text),
            0x22 => Some(Self::Image),
            0x63 => Some(Self::NText),
            0x28 => Some(Self::Date),
            0x29 => Some(Self::Time),
            0x2A => Some(Self::DateTime2),
            0x2B => Some(Self::DateTimeOffset),
            0x62 => Some(Self::Variant),
            0xF0 => Some(Self::Udt),
            0xF1 => Some(Self::Xml),
            0xF3 => Some(Self::Tvp),
            _ => None,
        }
    }

    /// Check if this is a fixed-length type.
    #[must_use]
    pub const fn is_fixed_length(&self) -> bool {
        matches!(
            self,
            Self::Null
                | Self::Int1
                | Self::Bit
                | Self::Int2
                | Self::Int4
                | Self::Int8
                | Self::Float4
                | Self::Float8
                | Self::Money
                | Self::Money4
                | Self::DateTime
                | Self::DateTime4
        )
    }

    /// Check if this is a variable-length type.
    #[must_use]
    pub const fn is_variable_length(&self) -> bool {
        !self.is_fixed_length()
    }

    /// Check if this type uses PLP (Partially Length-Prefixed) encoding.
    #[must_use]
    pub const fn is_plp(&self) -> bool {
        matches!(self, Self::Text | Self::Image | Self::NText | Self::Xml)
    }

    /// Check if this is a Unicode type.
    #[must_use]
    pub const fn is_unicode(&self) -> bool {
        matches!(self, Self::NChar | Self::NVarChar | Self::NText)
    }

    /// Check if this is a date/time type.
    #[must_use]
    pub const fn is_datetime(&self) -> bool {
        matches!(
            self,
            Self::DateTime
                | Self::DateTime4
                | Self::DateTimeN
                | Self::Date
                | Self::Time
                | Self::DateTime2
                | Self::DateTimeOffset
        )
    }

    /// Get the fixed size of this type in bytes, if applicable.
    #[must_use]
    pub const fn fixed_size(&self) -> Option<usize> {
        match self {
            Self::Null => Some(0),
            Self::Int1 => Some(1),
            Self::Bit => Some(1),
            Self::Int2 => Some(2),
            Self::Int4 => Some(4),
            Self::Int8 => Some(8),
            Self::Float4 => Some(4),
            Self::Float8 => Some(8),
            Self::Money => Some(8),
            Self::Money4 => Some(4),
            Self::DateTime => Some(8),
            Self::DateTime4 => Some(4),
            Self::Date => Some(3),
            _ => None,
        }
    }
}

/// Column flags from COLMETADATA.
#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnFlags {
    /// Column is nullable.
    pub nullable: bool,
    /// Column allows case-sensitive comparison.
    pub case_sensitive: bool,
    /// Column is updateable.
    pub updateable: Updateable,
    /// Column is an identity column.
    pub identity: bool,
    /// Column is computed.
    pub computed: bool,
    /// Column has fixed-length CLR type.
    pub fixed_len_clr_type: bool,
    /// Column is sparse.
    pub sparse_column_set: bool,
    /// Column is encrypted (Always Encrypted).
    pub encrypted: bool,
    /// Column is hidden.
    pub hidden: bool,
    /// Column is a key column.
    pub key: bool,
    /// Column is nullable but unknown at query time.
    pub nullable_unknown: bool,
}

/// Update mode for a column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Updateable {
    /// Column is read-only.
    #[default]
    ReadOnly,
    /// Column is read-write.
    ReadWrite,
    /// Updateability unknown.
    Unknown,
}

impl ColumnFlags {
    /// Parse column flags from the 2-byte flags field.
    #[must_use]
    pub fn from_bits(flags: u16) -> Self {
        Self {
            nullable: (flags & 0x0001) != 0,
            case_sensitive: (flags & 0x0002) != 0,
            updateable: match (flags >> 2) & 0x03 {
                0 => Updateable::ReadOnly,
                1 => Updateable::ReadWrite,
                _ => Updateable::Unknown,
            },
            identity: (flags & 0x0010) != 0,
            computed: (flags & 0x0020) != 0,
            fixed_len_clr_type: (flags & 0x0100) != 0,
            sparse_column_set: (flags & 0x0200) != 0,
            encrypted: (flags & 0x0400) != 0,
            hidden: (flags & 0x2000) != 0,
            key: (flags & 0x4000) != 0,
            nullable_unknown: (flags & 0x8000) != 0,
        }
    }

    /// Convert flags back to bits.
    #[must_use]
    pub fn to_bits(&self) -> u16 {
        let mut flags = 0u16;
        if self.nullable {
            flags |= 0x0001;
        }
        if self.case_sensitive {
            flags |= 0x0002;
        }
        flags |= match self.updateable {
            Updateable::ReadOnly => 0,
            Updateable::ReadWrite => 1 << 2,
            Updateable::Unknown => 2 << 2,
        };
        if self.identity {
            flags |= 0x0010;
        }
        if self.computed {
            flags |= 0x0020;
        }
        if self.fixed_len_clr_type {
            flags |= 0x0100;
        }
        if self.sparse_column_set {
            flags |= 0x0200;
        }
        if self.encrypted {
            flags |= 0x0400;
        }
        if self.hidden {
            flags |= 0x2000;
        }
        if self.key {
            flags |= 0x4000;
        }
        if self.nullable_unknown {
            flags |= 0x8000;
        }
        flags
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_type_id_from_u8() {
        assert_eq!(TypeId::from_u8(0x38), Some(TypeId::Int4));
        assert_eq!(TypeId::from_u8(0xE7), Some(TypeId::NVarChar));
        assert_eq!(TypeId::from_u8(0x99), None);
    }

    #[test]
    fn test_fixed_length_detection() {
        assert!(TypeId::Int4.is_fixed_length());
        assert!(TypeId::Float8.is_fixed_length());
        assert!(!TypeId::NVarChar.is_fixed_length());
    }

    #[test]
    fn test_column_flags_roundtrip() {
        let flags = ColumnFlags {
            nullable: true,
            identity: true,
            key: true,
            ..Default::default()
        };
        let bits = flags.to_bits();
        let restored = ColumnFlags::from_bits(bits);
        assert_eq!(flags.nullable, restored.nullable);
        assert_eq!(flags.identity, restored.identity);
        assert_eq!(flags.key, restored.key);
    }
}
