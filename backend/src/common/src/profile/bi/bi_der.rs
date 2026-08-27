use crate::profile::BiPoint;
use serde::{Deserialize, Serialize};

/// BI points belonging to the DER equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiDer {
    pub maintenance_operational_state: BiPoint,
    pub has_p1_alarms: BiPoint,
    pub has_p2_alarms: BiPoint,
    pub has_p3_alarms: BiPoint,
}

impl BiDer {
    pub fn iter_points(&self) -> Vec<&BiPoint> {
        vec![
            &self.maintenance_operational_state,
            &self.has_p1_alarms,
            &self.has_p2_alarms,
            &self.has_p3_alarms,
        ]
    }
}
