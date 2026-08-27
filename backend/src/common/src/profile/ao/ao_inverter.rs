use serde::{Deserialize, Serialize};

use crate::profile::AoPoint;

/// AO points belonging to the Inverter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AoInverter {
    pub active_power_high_threshold: AoPoint,
    pub active_power_low_threshold: AoPoint,
    pub reactive_power_high_threshold: AoPoint,
    pub reactive_power_low_threshold: AoPoint,
    pub frequency_high_threshold: AoPoint,
    pub frequency_low_threshold: AoPoint,
    pub dc_input_power_high_threshold: AoPoint,
    pub dc_input_power_low_threshold: AoPoint,
    pub dc_current_high_threshold: AoPoint,
    pub dc_current_low_threshold: AoPoint,
    pub dc_voltage_high_threshold: AoPoint,
    pub dc_voltage_low_threshold: AoPoint,
}

impl AoInverter {
    pub fn iter_points(&self) -> Vec<&AoPoint> {
        vec![
            &self.active_power_high_threshold,
            &self.active_power_low_threshold,
            &self.reactive_power_high_threshold,
            &self.reactive_power_low_threshold,
            &self.frequency_high_threshold,
            &self.frequency_low_threshold,
            &self.dc_input_power_high_threshold,
            &self.dc_input_power_low_threshold,
            &self.dc_current_high_threshold,
            &self.dc_current_low_threshold,
            &self.dc_voltage_high_threshold,
            &self.dc_voltage_low_threshold,
        ]
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AoPoint> {
        vec![
            &mut self.active_power_high_threshold,
            &mut self.active_power_low_threshold,
            &mut self.reactive_power_high_threshold,
            &mut self.reactive_power_low_threshold,
            &mut self.frequency_high_threshold,
            &mut self.frequency_low_threshold,
            &mut self.dc_input_power_high_threshold,
            &mut self.dc_input_power_low_threshold,
            &mut self.dc_current_high_threshold,
            &mut self.dc_current_low_threshold,
            &mut self.dc_voltage_high_threshold,
            &mut self.dc_voltage_low_threshold,
        ]
    }
}
