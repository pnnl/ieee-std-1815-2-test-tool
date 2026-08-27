use crate::profile::BiPoint;
use serde::{Deserialize, Serialize};

/// BI points belonging to the Meter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiMeter {
    pub active_power_too_high: BiPoint,
    pub active_power_too_low: BiPoint,
    pub reactive_power_too_high: BiPoint,
    pub reactive_power_too_low: BiPoint,
    pub power_factor_too_high: BiPoint,
    pub power_factor_too_low: BiPoint,
    pub phase_a_voltage_too_high: BiPoint,
    pub phase_a_voltage_too_low: BiPoint,
    pub phase_b_voltage_too_high: BiPoint,
    pub phase_b_voltage_too_low: BiPoint,
    pub phase_c_voltage_too_high: BiPoint,
    pub phase_c_voltage_too_low: BiPoint,
    pub communication_error: BiPoint,
}

impl BiMeter {
    pub fn iter_points(&self) -> Vec<&BiPoint> {
        vec![
            &self.active_power_too_high,
            &self.active_power_too_low,
            &self.reactive_power_too_high,
            &self.reactive_power_too_low,
            &self.power_factor_too_high,
            &self.power_factor_too_low,
            &self.phase_a_voltage_too_high,
            &self.phase_a_voltage_too_low,
            &self.phase_b_voltage_too_high,
            &self.phase_b_voltage_too_low,
            &self.phase_c_voltage_too_high,
            &self.phase_c_voltage_too_low,
            &self.communication_error,
        ]
    }
}
