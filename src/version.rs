use core::fmt;
use std::str::FromStr;

use anyhow::Context;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OSVersion {
    pub major: u16,
    pub minor: u8,
    pub patch: u8,
}

impl OSVersion {
    pub const MIN: Self = Self::from_packed(u32::MIN);
    pub const MAX: Self = Self::from_packed(u32::MAX);

    pub const fn from_packed(packed: u32) -> Self {
        let major = ((packed >> 16) & 0xFFFF) as u16;
        let minor = ((packed >> 8) & 0xFF) as u8;
        let patch = (packed & 0xFF) as u8;

        Self {
            major,
            minor,
            patch,
        }
    }
}

impl FromStr for OSVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (major, rest) = s.split_once('.').context("missing version separator")?;
        let major: u16 = major.parse()?;
        if let Some((minor, patch)) = rest.split_once('.') {
            let minor: u8 = minor.parse()?;
            let patch: u8 = patch.parse()?;
            Ok(Self {
                major,
                minor,
                patch,
            })
        } else {
            let minor: u8 = rest.parse()?;
            Ok(Self {
                major,
                minor,
                patch: 0,
            })
        }
    }
}

impl fmt::Display for OSVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if self.patch != 0 {
            write!(f, ".{}", self.patch)?;
        }
        Ok(())
    }
}
