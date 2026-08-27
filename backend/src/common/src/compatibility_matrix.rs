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

#[derive(Debug, Eq, Hash, PartialEq, Clone, EnumIter, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum ModeType {
    VoltageRideThrough,
    FrequencyRideThrough,
    SetActivePower,
    ActivePowerLimiting,
    FrequencyWatt,
    VoltWatt,
    CoordinatedChargeDischarge,
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
        }
    }
}

#[allow(dead_code)]
const MODE_COUNT: usize = ModeType::DynamicReactiveCurrent as usize + 1;
#[allow(dead_code)]
static COMPATIBILITY_MATRIX: [[i32; MODE_COUNT]; MODE_COUNT] = [
    [0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    [5, 0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    [5, 5, 0, 1, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0],
    [5, 5, 5, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 0, 2, 2, 2, 4, 2, 2, 2, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 2, 0, 2, 2, 4, 2, 2, 2, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 2, 2, 0, 2, 4, 2, 2, 2, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 2, 2, 2, 0, 4, 2, 2, 2, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 4, 4, 4, 4, 0, 4, 4, 4, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 2, 2, 2, 2, 4, 0, 2, 2, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 2, 2, 2, 2, 4, 4, 0, 2, 0, 0, 0, 0, 0, 0],
    [5, 5, 3, 1, 2, 2, 2, 2, 4, 2, 2, 0, 0, 0, 0, 0, 0, 0],
    [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3],
    [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 3, 3, 3, 3],
    [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 0, 3, 3, 3],
    [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 0, 3, 3],
    [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 0, 3],
    [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 0],
];
