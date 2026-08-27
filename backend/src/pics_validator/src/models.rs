pub use common::profile::profile::{
    AiBattery, AiCurve, AiDer, AiInverter, AiMeter, AiPoint, AiSchedule, AiScheduleBC,
    AnalogInputs, AnalogOutputs, AoBattery, AoInverter, AoMeter, AoPoint, BiBattery, BiDer,
    BiInverter, BiMeter, BiPoint, BinaryInputs, BinaryOutputs, BoPoint, CtrPoint, EquipmentInfo,
    EquipmentPoints, EventClass, KeySheet, PicsProfile, SectionInfo, SectionPoints,
};

// ---------------------------------------------------------------------------
// Stubs (to be implemented in future)
// ---------------------------------------------------------------------------

/// Validate that uid matches the known IEC 61850 UID for point_index.
/// Currently a no-op stub; future implementation should check against a UID registry.
#[allow(dead_code)]
pub fn check_uid(_point_index: u16, _uid: &str) -> anyhow::Result<()> {
    Ok(())
}

/// Compute a derived field value based on point-specific settings.
/// Currently returns 0.0; future implementation should compute the actual value.
#[allow(dead_code)]
pub fn calculate_derived_value(_point_index: u16, _field: &str) -> f64 {
    0.0
}

/// Evaluate a conditional mandatory string (e.g., "C: If in microgrid configuration").
/// Currently returns true; future implementation should evaluate the condition.
pub fn evaluate_conditional_mandatory(_condition: &str) -> bool {
    // TODO: Review supported binary input points to see if the corresponding EUT capability
    // exists and should be used to dynamically determine if the feature is mandatory.
    true
}
