use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, EnumIter, ToSchema, Copy)]
pub enum CurveType {
    NotDefined,
    NotApplicableUnknown,
    VoltVar,
    FrequencyWatt,
    WattVar,
    VoltageWatt,
    RemainConnected,
    TemperatureMode,
    PricingSignalMode,
    HvrtMustTrip,
    HvrtMomentaryCessation,
    LvrtMustTrip,
    LvrtMomentaryCessation,
    HfrtMustTrip,
    HfrtMomentaryCessation,
    LfrtMustTrip,
    LfrtMomentaryCessation,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct CurveTypeEntry {
    pub variant: CurveType,
    pub display_name: String,
    pub std_number: u8,
}

fn curve_type_std_number(curve_type: &CurveType) -> u8 {
    match curve_type {
        CurveType::NotDefined => 0,
        CurveType::NotApplicableUnknown => 1,
        CurveType::VoltVar => 2,
        CurveType::FrequencyWatt => 3,
        CurveType::WattVar => 4,
        CurveType::VoltageWatt => 5,
        CurveType::RemainConnected => 6,
        CurveType::TemperatureMode => 7,
        CurveType::PricingSignalMode => 8,
        CurveType::HvrtMustTrip => 9,
        CurveType::HvrtMomentaryCessation => 10,
        CurveType::LvrtMustTrip => 11,
        CurveType::LvrtMomentaryCessation => 12,
        CurveType::HfrtMustTrip => 13,
        CurveType::HfrtMomentaryCessation => 14,
        CurveType::LfrtMustTrip => 15,
        CurveType::LfrtMomentaryCessation => 16,
    }
}

fn curve_type_display_name(curve_type: &CurveType) -> &'static str {
    match curve_type {
        CurveType::NotDefined => "Not Defined",
        CurveType::NotApplicableUnknown => "Not Applicable / Unknown",
        CurveType::VoltVar => "Volt-VAR",
        CurveType::FrequencyWatt => "Frequency-Watt",
        CurveType::WattVar => "Watt-VAR",
        CurveType::VoltageWatt => "Voltage-Watt",
        CurveType::RemainConnected => "Remain Connected",
        CurveType::TemperatureMode => "Temperature Mode",
        CurveType::PricingSignalMode => "Pricing Signal Mode",
        CurveType::HvrtMustTrip => "HVRT Must Trip",
        CurveType::HvrtMomentaryCessation => "HVRT Momentary Cessation",
        CurveType::LvrtMustTrip => "LVRT Must Trip",
        CurveType::LvrtMomentaryCessation => "LVRT Momentary Cessation",
        CurveType::HfrtMustTrip => "HFRT Must Trip",
        CurveType::HfrtMomentaryCessation => "HFRT Momentary Cessation",
        CurveType::LfrtMustTrip => "LFRT Must Trip",
        CurveType::LfrtMomentaryCessation => "LFRT Momentary Cessation",
    }
}

pub fn build_curve_types() -> Vec<CurveTypeEntry> {
    CurveType::iter()
        .map(|variant| CurveTypeEntry {
            display_name: curve_type_display_name(&variant).to_string(),
            variant,
            std_number: curve_type_std_number(&variant),
        })
        .collect()
}
