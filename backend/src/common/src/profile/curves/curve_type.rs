use crate::profile::scale_curve::{DependentVariableUnit, IndependentVariableUnit};
use crate::profile::values::EngineeringF64;
use strum::IntoEnumIterator;
use strum_macros::Display;
use strum_macros::EnumIter;

#[repr(u8)] // Specifies the underlying type as u8
#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum CurveType {
    NotDefined = 0,
    Unknown = 1,
    VoltVAr = 2,
    FrequencyWatt = 3,
    WattVAr = 4,
    VoltageWatt = 5,
    RemainConnected = 6,
    TemperatureMode = 7,
    PricingSignalMode = 8,
    HVRTMustTrip = 9,
    HVRTMomentaryCessation = 10,
    LVRTMustTrip = 11,
    LVRTMomentaryCessation = 12,
    HFRTMustTrip = 13,
    HFRTMomentaryCessation = 14,
    LFRTMustTrip = 15,
    LFRTMomentaryCessation = 16,
}

impl CurveType {
    pub fn max_value() -> i64 {
        CurveType::iter()
            .map(|curve| curve as i64)
            .max()
            .expect("CurveType enum should have at least one variant")
    }

    pub fn is_compatible_with_x_unit(&self, unit: IndependentVariableUnit) -> bool {
        unit == match self {
            CurveType::NotDefined => IndependentVariableUnit::NotDefined,
            CurveType::Unknown => IndependentVariableUnit::NotDefined,
            CurveType::VoltVAr => IndependentVariableUnit::Voltage,
            CurveType::FrequencyWatt => IndependentVariableUnit::FrequencyHz,
            CurveType::WattVAr => IndependentVariableUnit::Watts,
            CurveType::VoltageWatt => IndependentVariableUnit::VoltagePct,
            CurveType::RemainConnected => IndependentVariableUnit::TimeMs,
            CurveType::TemperatureMode => IndependentVariableUnit::Celsius,
            CurveType::PricingSignalMode => IndependentVariableUnit::PriceHundredthsLocalCurrency,
            CurveType::HVRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::HVRTMomentaryCessation => IndependentVariableUnit::TimeMs,
            CurveType::LVRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::LVRTMomentaryCessation => IndependentVariableUnit::TimeMs,
            CurveType::HFRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::HFRTMomentaryCessation => IndependentVariableUnit::TimeMs,
            CurveType::LFRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::LFRTMomentaryCessation => IndependentVariableUnit::TimeMs,
        }
    }

    pub fn is_compatible_with_y_unit(&self, unit: DependentVariableUnit) -> bool {
        let valid_units = match self {
            CurveType::NotDefined => vec![DependentVariableUnit::NotDefined],
            CurveType::Unknown => vec![DependentVariableUnit::NotApplicable],
            CurveType::VoltVAr => vec![DependentVariableUnit::VarsPctVarMax],
            CurveType::FrequencyWatt => vec![DependentVariableUnit::WattsPctFrozenActive],
            CurveType::WattVAr => vec![
                DependentVariableUnit::VarsPctVarAvailable, // Via Table 51
                DependentVariableUnit::VarsPctVarMax,
                DependentVariableUnit::VarsPctWMax,
            ], // TODO: this is lot listed
            CurveType::VoltageWatt => vec![DependentVariableUnit::WattsPctWMax],
            CurveType::RemainConnected => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::TemperatureMode => vec![unit], // This profile does not require that an outstation support any particular Y-value units (e.g., watts) for a function type of <7> temperature or <8> price signal.
            CurveType::PricingSignalMode => vec![unit],
            CurveType::HVRTMustTrip => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::HVRTMomentaryCessation => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::LVRTMustTrip => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::LVRTMomentaryCessation => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::HFRTMustTrip => vec![DependentVariableUnit::FrequencyPctNominal],
            CurveType::HFRTMomentaryCessation => vec![DependentVariableUnit::FrequencyPctNominal],
            CurveType::LFRTMustTrip => vec![DependentVariableUnit::FrequencyPctNominal],
            CurveType::LFRTMomentaryCessation => vec![DependentVariableUnit::FrequencyPctNominal],
        };
        valid_units.contains(&unit)
    }

    pub fn compatible_x_units(&self) -> Vec<IndependentVariableUnit> {
        IndependentVariableUnit::iter()
            .filter(|unit| self.is_compatible_with_x_unit(*unit))
            .collect()
    }

    pub fn compatible_y_units(&self) -> Vec<DependentVariableUnit> {
        DependentVariableUnit::iter()
            .filter(|unit| self.is_compatible_with_y_unit(*unit))
            .collect()
    }
}

impl TryFrom<EngineeringF64> for CurveType {
    type Error = &'static str;

    fn try_from(value: EngineeringF64) -> Result<Self, Self::Error> {
        let int_value = value.0 as u8; // Convert the f64 to u8

        if value.0 != int_value as f64 {
            return Err("Value is not an integer in range of CurveType");
        }

        CurveType::try_from(int_value)
    }
}

impl TryFrom<u8> for CurveType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CurveType::NotDefined),
            1 => Ok(CurveType::Unknown),
            2 => Ok(CurveType::VoltVAr),
            3 => Ok(CurveType::FrequencyWatt),
            4 => Ok(CurveType::WattVAr),
            5 => Ok(CurveType::VoltageWatt),
            6 => Ok(CurveType::RemainConnected),
            7 => Ok(CurveType::TemperatureMode),
            8 => Ok(CurveType::PricingSignalMode),
            9 => Ok(CurveType::HVRTMustTrip),
            10 => Ok(CurveType::HVRTMomentaryCessation),
            11 => Ok(CurveType::LVRTMustTrip),
            12 => Ok(CurveType::LVRTMomentaryCessation),
            13 => Ok(CurveType::HFRTMustTrip),
            14 => Ok(CurveType::HFRTMomentaryCessation),
            15 => Ok(CurveType::LFRTMustTrip),
            16 => Ok(CurveType::LFRTMomentaryCessation),
            _ => Err("Invalid CurveType value"),
        }
    }
}
