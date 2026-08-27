use crate::profile::AoPoint;
use serde::{Deserialize, Serialize};

/// AO points belonging to the Meter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AoMeter {
    pub active_power_high_threshold: AoPoint,
    pub active_power_low_threshold: AoPoint,
    pub reactive_power_high_threshold: AoPoint,
    pub reactive_power_low_threshold: AoPoint,
    pub power_factor_high_threshold: AoPoint,
    pub power_factor_low_threshold: AoPoint,
    pub phase_a_volts_high_threshold: AoPoint,
    pub phase_a_volts_low_threshold: AoPoint,
    pub phase_b_volts_high_threshold: AoPoint,
    pub phase_b_volts_low_threshold: AoPoint,
    pub phase_c_volts_high_threshold: AoPoint,
    pub phase_c_volts_low_threshold: AoPoint,
}

impl AoMeter {
    pub fn iter_points(&self) -> Vec<&AoPoint> {
        vec![
            &self.active_power_high_threshold,
            &self.active_power_low_threshold,
            &self.reactive_power_high_threshold,
            &self.reactive_power_low_threshold,
            &self.power_factor_high_threshold,
            &self.power_factor_low_threshold,
            &self.phase_a_volts_high_threshold,
            &self.phase_a_volts_low_threshold,
            &self.phase_b_volts_high_threshold,
            &self.phase_b_volts_low_threshold,
            &self.phase_c_volts_high_threshold,
            &self.phase_c_volts_low_threshold,
        ]
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AoPoint> {
        vec![
            &mut self.active_power_high_threshold,
            &mut self.active_power_low_threshold,
            &mut self.reactive_power_high_threshold,
            &mut self.reactive_power_low_threshold,
            &mut self.power_factor_high_threshold,
            &mut self.power_factor_low_threshold,
            &mut self.phase_a_volts_high_threshold,
            &mut self.phase_a_volts_low_threshold,
            &mut self.phase_b_volts_high_threshold,
            &mut self.phase_b_volts_low_threshold,
            &mut self.phase_c_volts_high_threshold,
            &mut self.phase_c_volts_low_threshold,
        ]
    }
}
