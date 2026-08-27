use serde::{Deserialize, Serialize};

use crate::profile::{BiBattery, BiDer, BiInverter, BiMeter, BiPoint};

/// All binary input points, grouped by base and equipment type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BinaryInputs {
    pub points: Vec<BiPoint>,
    pub meters: Vec<BiMeter>,
    pub ders: Vec<BiDer>,
    pub inverters: Vec<BiInverter>,
    pub batteries: Vec<BiBattery>,
}

impl BinaryInputs {
    /// Collect all BI points from all sub-groups (base + equipment).
    pub fn all_bi_points(&self) -> Vec<&BiPoint> {
        let mut out: Vec<&BiPoint> = self.points.iter().collect();
        for meter in &self.meters {
            out.extend(meter.iter_points());
        }
        for der in &self.ders {
            out.extend(der.iter_points());
        }
        for inv in &self.inverters {
            out.extend(inv.iter_points());
        }
        for bat in &self.batteries {
            out.extend(bat.iter_points());
        }
        out
    }
}
