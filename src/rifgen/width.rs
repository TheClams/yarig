use std::fmt::Display;

use crate::error::RifErrorKind;


#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DataWidth {
    W8(u8),
    W16(u8),
    W32(u8),
    W64(u8),
    W128(u8),
}

impl DataWidth {

    pub fn value(&self) -> u8 {
        match self {
            DataWidth::W8(v)   => *v,
            DataWidth::W16(v)  => *v,
            DataWidth::W32(v)  => *v,
            DataWidth::W64(v)  => *v,
            DataWidth::W128(v) => *v,
        }
    }

    pub fn addr_mask(&self) -> u64 {
        match self {
            DataWidth::W8(_)   => 0,
            DataWidth::W16(_)  => 1,
            DataWidth::W32(_)  => 3,
            DataWidth::W64(_)  => 7,
            DataWidth::W128(_) => 15,
        }
    }

    pub fn nb_byte(&self) -> u8 {
        match self {
            DataWidth::W8(_)   => 1,
            DataWidth::W16(_)  => 2,
            DataWidth::W32(_)  => 4,
            DataWidth::W64(_)  => 8,
            DataWidth::W128(_) => 16,
        }
    }
}

impl Default for DataWidth {
    fn default() -> Self {
        DataWidth::W32(32)
    }
}

impl Display for DataWidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl TryFrom<u8> for DataWidth {
    type Error = RifErrorKind;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            4..=8    => Ok(DataWidth::W8(value)),
            9..=16   => Ok(DataWidth::W16(value)),
            17..=32  => Ok(DataWidth::W32(value)),
            33..=64  => Ok(DataWidth::W64(value)),
            65..=128 => Ok(DataWidth::W128(value)),
            _ => Err(RifErrorKind::AddrWidth)
        }
    }
}

impl PartialOrd for DataWidth {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value().partial_cmp(&other.value())
    }
}
