// TODO: This module belongs in a separate control station module (not common).
// It is currently here for convenience but should be moved when the control station
// is extracted into its own workspace crate.

use serde::{Deserialize, Serialize};
use strum::EnumIter;
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum CompatibilityType {
    Compatible = 0,
    Bounding = 1,
    Additive = 2,
    Replacing = 3,
    Exclusive = 4,
    Override = 5,
}

/// DER functions in the same order as the "Compatibility of functions" table (58).
#[derive(
    Debug, Eq, Hash, PartialEq, Clone, EnumIter, Copy, Serialize, Deserialize, strum::EnumCount,
)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum ModeType {
    VoltageRideThrough,
    FrequencyRideThrough,
    /// AKA "Charge-Discharge"
    SetActivePower,
    ActivePowerLimiting,
    FrequencyWatt,
    VoltWatt,
    CoordinatedChargeDischarge,
    PeakPowerLimiting,
    /// AKA "Active Power Response"
    ActivePowerFollowing,
    AutomaticGenerationControl,
    ActivePowerSmoothing,
    FrequencyWattCurve,
    DynamicVoltWatt,
    ConstantVars,
    ConstantPowerFactor,
    VoltVar,
    WattVar,
    PowerFactorCorrection,
    DynamicReactiveCurrent,
    Price,
}

impl ModeType {
    /// Maps each ModeType variant to its corresponding list of purpose strings.
    ///
    /// It's conceptually natural to map from purpose to ModeType, but this setup
    /// ensures all ModeType variants are covered.
    pub fn to_purposes(self) -> &'static [&'static str] {
        match self {
            ModeType::ActivePowerFollowing => &["generation following"],
            ModeType::ActivePowerLimiting => &["active power limit"],
            ModeType::ActivePowerSmoothing => &["active power smoothing"],
            ModeType::AutomaticGenerationControl => &["agc"],
            ModeType::ConstantPowerFactor => &["constant pf"],
            ModeType::ConstantVars => &["const vars", "constant vars"],
            ModeType::CoordinatedChargeDischarge => &["coord charge-dischg"],
            ModeType::DynamicReactiveCurrent => &["dyn react curr supp"],
            ModeType::DynamicVoltWatt => &["dyn volt-watt"],
            ModeType::FrequencyRideThrough => &["freq ride-through"],
            ModeType::FrequencyWatt => &["freq-watt"],
            ModeType::FrequencyWattCurve => &["freq-watt curve"],
            ModeType::PowerFactorCorrection => &["pf correct"],
            ModeType::SetActivePower => &["set active power"],
            ModeType::VoltageRideThrough => &["volt ride-through"],
            ModeType::VoltVar => &["volt-var"],
            ModeType::VoltWatt => &["volt-watt"],
            ModeType::WattVar => &["watt-var"],
            ModeType::Price => &["pricing signal"],
            ModeType::PeakPowerLimiting => &["peak power limiting"],
        }
    }
}

/// Compatibility matrix matching "Table 58—Compatibility of functions"
#[allow(dead_code)]
const MODE_COUNT: usize = <ModeType as strum::EnumCount>::COUNT;
#[allow(dead_code)]
#[rustfmt::skip]
static COMPATIBILITY_MATRIX: [[&str; MODE_COUNT - 1]; MODE_COUNT - 1] = [ // Minus one to exclude Price, which isn't covered in Table 58
     // Voltage Ride-Through
    [ "C", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov"],
    // Frequency Ride-Through
    [  "",  "C", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov", "Ov"],
    // Charge-Discharge (Set Active Power) 
    [  "",   "",  "C",  "B",  "R",  "R",  "R",  "B",  "R",  "R",  "R",  "R",  "R",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Active Power Limiting (Generation and Consumption)
    [  "",   "",   "",  "C",  "B",  "B",  "B",  "B",  "B",  "B",  "B",  "B",  "B",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Frequency-Watt (Droop or Sensitivity) 
    [  "",   "",   "",   "",  "C",  "A",  "A",  "B",  "A", "Ex",  "A",  "A",  "A",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Volt-Watt 
    [  "",   "",   "",   "",   "",  "C",  "A",  "B",  "A", "Ex",  "A",  "A",  "A",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Coordinated Charge-Discharge 
    [  "",   "",   "",   "",   "",   "",  "C",  "B",  "A", "Ex",  "A",  "A",  "A",  "C",  "C",  "C",  "C",  "C",  "C"], 
    // Peak Power Limiting 
    [  "",   "",   "",   "",   "",   "",   "",  "C",  "B", "Ex",  "B",  "B",  "B",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Active Power Following
    [  "",   "",   "",   "",   "",   "",   "",   "",  "C", "Ex",  "A",  "A",  "A",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Automatic Generation Control
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",  "C", "Ex", "Ex", "Ex",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Active Power Smoothing
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "A",  "A",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Frequency-Watt Curve
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "A",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Dynamic Volt-Watt
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "C",  "C",  "C",  "C",  "C",  "C"],
    // Constant Vars
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "R",  "R",  "R",  "R",  "R"],
    // Constant Power Factor
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "R",  "R",  "R",  "R"],
    // Volt-Var Control
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "A",  "A",  "A"],
    // Watt-Var
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "A",  "A"],
    // Power Factor Correction
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C",  "A"],
    // Dynamic Reactive Current
    [  "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",   "",  "C"],
];
