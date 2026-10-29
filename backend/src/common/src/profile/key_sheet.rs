use serde::{Deserialize, Serialize};

use crate::profile::{EquipmentPoints, SectionPoints};

/// Index and count metadata for all point groups in the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct KeySheet {
    pub config: SectionPoints,
    pub functions: SectionPoints,
    pub curves: SectionPoints,
    pub system_meter: SectionPoints,
    pub extensions: SectionPoints,
    pub experimental: SectionPoints,
    pub vendor: SectionPoints,
    pub discovery: SectionPoints,
    pub schedules_bc: SectionPoints,
    pub schedules_bc_status: SectionPoints,
    pub schedules: SectionPoints,
    pub schedules_status: SectionPoints,
    pub max_points: u16,
    pub meter: EquipmentPoints,
    pub der: EquipmentPoints,
    pub inverter: EquipmentPoints,
    pub battery: EquipmentPoints,
}

impl KeySheet {
    /// Base AI storage excludes functional and equipment ranges, except for
    /// their edit selectors and the running BC schedule index.
    pub fn is_base_ai_point(&self, index: u16) -> bool {
        index < self.curves.ai.start
            || (self.system_meter.ai.start..self.schedules_bc.ai.start).contains(&index)
            || (self.schedules_bc_status.ai.start..self.schedules.ai.start).contains(&index)
            || (self.schedules_status.ai.start..self.meter.ai.start).contains(&index)
            || index >= self.experimental.ai.start
            || index == self.curves.ai.start
            || index
                .checked_sub(self.schedules_bc.ai.start)
                .is_some_and(|offset| offset <= 1)
            || index == self.schedules.ai.start
    }
}
