use serde::{Deserialize, Serialize};

use crate::profile::{EquipmentPoints, SectionPoints};

/// The group containing an AI point in a flat spreadsheet, before selectors
/// are moved from functional groups into the base point collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPointGroup {
    Base,
    Curve,
    ScheduleBC,
    Schedule,
    Meter,
    Der,
    Inverter,
    Battery,
}

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
    /// Classify a flat AI point using this profile's section boundaries.
    pub fn ai_point_group(&self, index: u16) -> AiPointGroup {
        if index >= self.experimental.ai.start {
            AiPointGroup::Base
        } else if (self.curves.ai.start..self.system_meter.ai.start).contains(&index) {
            AiPointGroup::Curve
        } else if (self.schedules_bc.ai.start..self.schedules_bc_status.ai.start).contains(&index) {
            AiPointGroup::ScheduleBC
        } else if (self.schedules_bc_status.ai.start..self.schedules.ai.start).contains(&index) {
            AiPointGroup::Base
        } else if (self.schedules.ai.start..self.schedules_status.ai.start).contains(&index) {
            AiPointGroup::Schedule
        } else if (self.schedules_status.ai.start..self.meter.ai.start).contains(&index) {
            AiPointGroup::Base
        } else if index >= self.battery.ai.start {
            AiPointGroup::Battery
        } else if index >= self.inverter.ai.start {
            AiPointGroup::Inverter
        } else if index >= self.der.ai.start {
            AiPointGroup::Der
        } else if index >= self.meter.ai.start {
            AiPointGroup::Meter
        } else {
            AiPointGroup::Base
        }
    }

    /// Base points also include functional edit selectors and the running BC
    /// schedule index, which the loader moves out of their functional groups.
    pub fn is_base_ai_point(&self, index: u16) -> bool {
        match self.ai_point_group(index) {
            AiPointGroup::Base => true,
            AiPointGroup::Curve => index == self.curves.ai.start,
            AiPointGroup::ScheduleBC => index
                .checked_sub(self.schedules_bc.ai.start)
                .is_some_and(|offset| offset <= 1),
            AiPointGroup::Schedule => index == self.schedules.ai.start,
            _ => false,
        }
    }
}
