use std::fmt::{self, Display};

use crate::profile::{AiValue, BiValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointIndex {
    Ai(u16),
    Bi(u16),
}

impl Display for PointIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ai(index) => write!(f, "AI{index}"),
            Self::Bi(index) => write!(f, "BI{index}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointValue {
    Ai(AiValue),
    Bi(BiValue),
}

impl Display for PointValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ai(point) => Display::fmt(&point.value, f),
            Self::Bi(point) => Display::fmt(&point.value, f),
        }
    }
}

impl PointValue {
    /// Extract the analog value, returning `None` for a binary value.
    pub fn as_ai(self) -> Option<AiValue> {
        match self {
            Self::Ai(point) => Some(point),
            Self::Bi(_) => None,
        }
    }

    pub fn key(&self) -> PointIndex {
        match self {
            Self::Ai(point) => PointIndex::Ai(point.index),
            Self::Bi(point) => PointIndex::Bi(point.index),
        }
    }
}
