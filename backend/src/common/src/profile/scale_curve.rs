use strum::{Display, EnumIter};

use crate::profile::values::{EngineeringF64, TransmissionI32};

pub struct ScalingEntry {
    pub min: TransmissionI32,
    pub max: TransmissionI32,
    pub mult: f64,
}

#[repr(u8)] // Specifies the underlying type as u8
#[derive(Display, Debug, Clone, Copy, PartialEq, EnumIter)]
pub enum IndependentVariableUnit {
    NotDefined = 0,
    NotApplicable = 1,
    TimeMs = 4,
    Celsius = 23,
    Voltage = 29,
    FrequencyHz = 33,
    Watts = 38,
    PriceHundredthsLocalCurrency = 100,
    VoltagePct = 129,
    FrequencyPct = 133,
    WattsPct = 138,
    FrequencyDeviation = 233,
}

impl TryFrom<EngineeringF64> for IndependentVariableUnit {
    type Error = String;

    fn try_from(value: EngineeringF64) -> Result<Self, Self::Error> {
        if value.0.fract() != 0.0 || value.0 < 0.0 || value.0 > 255.0 {
            return Err(format!(
                "Invalid independent variable unit {}, expected a whole number between 0 and 255",
                value.0
            ));
        }
        match value.0 as u8 {
            0 => Ok(Self::NotDefined),
            1 => Ok(Self::NotApplicable),
            4 => Ok(Self::TimeMs),
            23 => Ok(Self::Celsius),
            29 => Ok(Self::Voltage),
            33 => Ok(Self::FrequencyHz),
            38 => Ok(Self::Watts),
            100 => Ok(Self::PriceHundredthsLocalCurrency),
            129 => Ok(Self::VoltagePct),
            133 => Ok(Self::FrequencyPct),
            138 => Ok(Self::WattsPct),
            233 => Ok(Self::FrequencyDeviation),
            _ => Err(format!("Unknown independent variable unit {}", value.0)),
        }
    }
}

#[repr(u8)] // Specifies the underlying type as u8
#[derive(Display, Debug, Clone, Copy, PartialEq, EnumIter)]
pub enum DependentVariableUnit {
    NotDefined = 0,
    NotApplicable = 1,
    VarsPctVarMax = 2,
    VarsPctVarAvailable = 3,
    /// Unused according to the spreadsheet.
    VarsPctWMax = 4,
    WattsPctWMax = 5,
    WattsPctFrozenActive = 6,
    PowerFactor = 7,
    VoltsPctVRef = 8,
    FrequencyPctNominal = 9,
    // not found in Table 17 as shown in spreadsheet
    Voltage = 29,
    Frequency = 33,
    Watts = 38,
}

impl TryFrom<EngineeringF64> for DependentVariableUnit {
    type Error = String;

    fn try_from(value: EngineeringF64) -> Result<Self, Self::Error> {
        if value.0.fract() != 0.0 || value.0 < 0.0 || value.0 > 255.0 {
            return Err(format!(
                "Invalid dependent variable unit {}, expected a whole number between 0 and 255",
                value.0
            ));
        }
        match value.0 as u8 {
            0 => Ok(Self::NotDefined),
            1 => Ok(Self::NotApplicable),
            2 => Ok(Self::VarsPctVarMax),
            3 => Ok(Self::VarsPctVarAvailable),
            4 => Ok(Self::VarsPctWMax),
            5 => Ok(Self::WattsPctWMax),
            6 => Ok(Self::WattsPctFrozenActive),
            7 => Ok(Self::PowerFactor),
            8 => Ok(Self::VoltsPctVRef),
            9 => Ok(Self::FrequencyPctNominal),
            29 => Ok(Self::Voltage),
            33 => Ok(Self::Frequency),
            38 => Ok(Self::Watts),
            _ => Err(format!("Unknown dependent variable unit {}", value.0)),
        }
    }
}

// From Table 16 - Scaling for generic curve (x-values)
impl IndependentVariableUnit {
    pub const fn scaling(self) -> Option<ScalingEntry> {
        match self {
            IndependentVariableUnit::NotDefined => None,
            IndependentVariableUnit::NotApplicable => None,
            IndependentVariableUnit::TimeMs => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(i32::MAX),
                mult: 1.0,
            }),
            IndependentVariableUnit::Celsius => Some(ScalingEntry {
                min: TransmissionI32(-5000),
                max: TransmissionI32(5000),
                mult: 0.1,
            }),
            IndependentVariableUnit::Voltage => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(i32::MAX),
                mult: 1.0,
            }),
            IndependentVariableUnit::FrequencyHz => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(7000),
                mult: 0.01,
            }),
            IndependentVariableUnit::Watts => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(i32::MAX),
                mult: 1.0,
            }),
            IndependentVariableUnit::PriceHundredthsLocalCurrency => Some(ScalingEntry {
                min: TransmissionI32(i32::MIN),
                max: TransmissionI32(i32::MAX),
                mult: 0.1,
            }),
            IndependentVariableUnit::VoltagePct => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(10000),
                mult: 0.1,
            }),
            IndependentVariableUnit::FrequencyPct => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(10000),
                mult: 0.1,
            }),
            IndependentVariableUnit::WattsPct => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(1000),
                mult: 0.1,
            }),
            IndependentVariableUnit::FrequencyDeviation => Some(ScalingEntry {
                min: TransmissionI32(-70000),
                max: TransmissionI32(70000),
                mult: 0.0001,
            }),
        }
    }

    /// Convert a u8 discriminant value to the corresponding variant, if known.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::NotDefined),
            1 => Some(Self::NotApplicable),
            4 => Some(Self::TimeMs),
            23 => Some(Self::Celsius),
            29 => Some(Self::Voltage),
            33 => Some(Self::FrequencyHz),
            38 => Some(Self::Watts),
            100 => Some(Self::PriceHundredthsLocalCurrency),
            129 => Some(Self::VoltagePct),
            133 => Some(Self::FrequencyPct),
            138 => Some(Self::WattsPct),
            233 => Some(Self::FrequencyDeviation),
            _ => None,
        }
    }
}

// From Table 17 - Scaling for generic curve (y-values)
impl DependentVariableUnit {
    pub const fn scaling(self) -> Option<ScalingEntry> {
        match self {
            DependentVariableUnit::NotDefined => None,
            DependentVariableUnit::NotApplicable => None,
            DependentVariableUnit::VarsPctVarMax => Some(ScalingEntry {
                min: TransmissionI32(-1000),
                max: TransmissionI32(1000),
                mult: 0.1,
            }),
            DependentVariableUnit::VarsPctVarAvailable => Some(ScalingEntry {
                min: TransmissionI32(-1000),
                max: TransmissionI32(1000),
                mult: 0.1,
            }),
            DependentVariableUnit::VarsPctWMax => Some(ScalingEntry {
                min: TransmissionI32(-1000),
                max: TransmissionI32(1000),
                mult: 0.1,
            }),
            DependentVariableUnit::WattsPctWMax => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(7000),
                mult: 0.01,
            }),
            DependentVariableUnit::WattsPctFrozenActive => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(1000),
                mult: 1.0,
            }),
            DependentVariableUnit::PowerFactor => Some(ScalingEntry {
                min: TransmissionI32(-100),
                max: TransmissionI32(100),
                mult: 0.01,
            }),
            DependentVariableUnit::VoltsPctVRef => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(10000),
                mult: 0.1,
            }),
            DependentVariableUnit::FrequencyPctNominal => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(10000),
                mult: 0.1,
            }),
            DependentVariableUnit::Voltage => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(i32::MAX),
                mult: 1.0,
            }),
            DependentVariableUnit::Frequency => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(7000),
                mult: 0.01,
            }),
            DependentVariableUnit::Watts => Some(ScalingEntry {
                min: TransmissionI32(0),
                max: TransmissionI32(i32::MAX),
                mult: 1.0,
            }),
        }
    }

    /// Convert a u8 discriminant value to the corresponding variant, if known.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::NotDefined),
            1 => Some(Self::NotApplicable),
            2 => Some(Self::VarsPctVarMax),
            3 => Some(Self::VarsPctVarAvailable),
            4 => Some(Self::VarsPctWMax),
            5 => Some(Self::WattsPctWMax),
            6 => Some(Self::WattsPctFrozenActive),
            7 => Some(Self::PowerFactor),
            8 => Some(Self::VoltsPctVRef),
            9 => Some(Self::FrequencyPctNominal),
            29 => Some(Self::Voltage),
            33 => Some(Self::Frequency),
            38 => Some(Self::Watts),
            _ => None,
        }
    }
}
