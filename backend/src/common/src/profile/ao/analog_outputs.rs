use serde::{Deserialize, Serialize};

use crate::profile::{AoBattery, AoInverter, AoMeter, AoPoint};

/// All analog output points, grouped by base and equipment type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AnalogOutputs {
    pub points: Vec<AoPoint>,
    pub meters: Vec<AoMeter>,
    pub inverters: Vec<AoInverter>,
    pub batteries: Vec<AoBattery>,
}

impl AnalogOutputs {
    /// Collect all AO points from all sub-groups (base + equipment).
    pub fn all_ao_points(&self) -> Vec<&AoPoint> {
        let mut out: Vec<&AoPoint> = self.points.iter().collect();
        for meter in &self.meters {
            out.extend(meter.iter_points());
        }
        for inv in &self.inverters {
            out.extend(inv.iter_points());
        }
        for bat in &self.batteries {
            out.extend(bat.iter_points());
        }
        out
    }

    /// Collect mutable references to all AO points from all sub-groups (base + equipment).
    pub fn all_ao_points_mut(&mut self) -> Vec<&mut AoPoint> {
        let mut out: Vec<&mut AoPoint> = self.points.iter_mut().collect();
        for meter in &mut self.meters {
            out.extend(meter.iter_points_mut());
        }
        for inv in &mut self.inverters {
            out.extend(inv.iter_points_mut());
        }
        for bat in &mut self.batteries {
            out.extend(bat.iter_points_mut());
        }
        out
    }
}
