use serde::{Deserialize, Serialize};

use crate::profile::AoPoint;

/// AO points belonging to the Battery equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AoBattery {
    pub external_voltage_high_threshold: AoPoint,
    pub external_voltage_low_threshold: AoPoint,
    pub internal_voltage_high_threshold: AoPoint,
    pub internal_voltage_low_threshold: AoPoint,
}

impl AoBattery {
    pub fn iter_points(&self) -> Vec<&AoPoint> {
        vec![
            &self.external_voltage_high_threshold,
            &self.external_voltage_low_threshold,
            &self.internal_voltage_high_threshold,
            &self.internal_voltage_low_threshold,
        ]
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AoPoint> {
        vec![
            &mut self.external_voltage_high_threshold,
            &mut self.external_voltage_low_threshold,
            &mut self.internal_voltage_high_threshold,
            &mut self.internal_voltage_low_threshold,
        ]
    }
}
