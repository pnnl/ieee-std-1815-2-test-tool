use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::Display as StrumDisplay;
use tracing::debug;

use crate::profile::validation::{ValidationError, ValidationErrors};

/// A scaled engineering value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EngineeringF64(pub f64);

impl Eq for EngineeringF64 {}

impl Ord for EngineeringF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl PartialOrd for EngineeringF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl EngineeringF64 {
    pub fn from_transmitted(
        transmitted_value: TransmissionI32,
        multiplier: f64,
        offset: EngineeringF64,
    ) -> Self {
        EngineeringF64(f64::from(transmitted_value.0) * multiplier) + offset
    }
}

// Implement add operation
impl std::ops::Add for EngineeringF64 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        EngineeringF64(self.0 + rhs.0)
    }
}

impl std::ops::Sub for EngineeringF64 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        EngineeringF64(self.0 - rhs.0)
    }
}

impl Display for EngineeringF64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct ScalingError {
    pub reason: ScalingErrorReason,
    pub input_engineering_value: f64,
    pub multiplier: f64,
    pub offset: EngineeringF64,
}

impl Display for ScalingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (input={}, multiplier={}, offset={})",
            self.reason, self.input_engineering_value, self.multiplier, self.offset
        )
    }
}

#[derive(Debug, StrumDisplay)]
pub enum ScalingErrorReason {
    DivisionByZero,
    NonFiniteMultiplier,
    NonFiniteOffset,
    SettingTransmissionOutOfi32Range { output_raw_value: f64 },
    NonIntegerResult { output_raw_value: f64 },
}

/// An unscaled, transmitted value
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TransmissionI32(pub i32);

impl TransmissionI32 {
    pub fn preview_from_engineering(
        engineering_value: EngineeringF64,
        multiplier: f64,
        offset: EngineeringF64,
    ) -> f64 {
        (engineering_value - offset).0 / multiplier
    }

    pub fn try_from_engineering(
        engineering_value: EngineeringF64,
        multiplier: f64,
        offset: EngineeringF64,
    ) -> Result<Self, ValidationErrors> {
        let make_error = |reason| {
            ValidationErrors::from_error(ValidationError {
                point: "".to_string(), // You might want to set the appropriate point here
                message: format!(
                    "{} (engineering_value={}, multiplier={}, offset={})",
                    reason, engineering_value.0, multiplier, offset
                ),
            })
        };

        if multiplier == 0.0 {
            return Err(make_error(ScalingErrorReason::DivisionByZero));
        }

        if !multiplier.is_finite() {
            return Err(make_error(ScalingErrorReason::NonFiniteMultiplier));
        }

        if !offset.0.is_finite() {
            return Err(make_error(ScalingErrorReason::NonFiniteOffset));
        }

        let raw_value =
            TransmissionI32::preview_from_engineering(engineering_value, multiplier, offset);
        if raw_value < i32::MIN as f64 || raw_value > i32::MAX as f64 {
            Err(make_error(
                ScalingErrorReason::SettingTransmissionOutOfi32Range {
                    output_raw_value: raw_value,
                },
            ))
        } else if raw_value.fract().abs() > 1e-7 {
            debug!(
                "Non-integer scaling result: input_engineering_value={}, multiplier={}, offset={}, output_raw_value={}",
                engineering_value.0, multiplier, offset, raw_value
            );
            Ok(TransmissionI32(raw_value.round() as i32))
        } else {
            Ok(TransmissionI32(raw_value as i32))
        }
    }
}

impl Display for TransmissionI32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u16> for TransmissionI32 {
    fn from(value: u16) -> Self {
        TransmissionI32(value.into())
    }
}

impl From<i16> for TransmissionI32 {
    fn from(value: i16) -> Self {
        TransmissionI32(value.into())
    }
}

impl From<i32> for TransmissionI32 {
    fn from(value: i32) -> Self {
        TransmissionI32(value)
    }
}
