use serde::{Deserialize, Serialize};

use crate::profile::{EquipmentPoints, SectionPoints};

/// The Key sheet section containing an AI point, before the loader groups
/// points for storage and moves functional selectors into the base collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPointGroup {
    ConfigAndControl,
    Functions,
    Curve,
    SystemMeter,
    Extensions,
    ScheduleBC,
    ScheduleBCStatus,
    Schedule,
    ScheduleStatus,
    Meter,
    Der,
    Inverter,
    Battery,
    Experimental,
    Vendor,
    /// Start Index Points used for optional auto discovery.
    StartIndexPoints,
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
    /// Starting boundary of the Key sheet's Start Index Points section.
    pub start_index_points: SectionPoints,
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
        if index >= self.start_index_points.ai.start {
            AiPointGroup::StartIndexPoints
        } else if index >= self.vendor.ai.start {
            AiPointGroup::Vendor
        } else if index >= self.experimental.ai.start {
            AiPointGroup::Experimental
        } else if index >= self.battery.ai.start {
            AiPointGroup::Battery
        } else if index >= self.inverter.ai.start {
            AiPointGroup::Inverter
        } else if index >= self.der.ai.start {
            AiPointGroup::Der
        } else if index >= self.meter.ai.start {
            AiPointGroup::Meter
        } else if index >= self.schedules_status.ai.start {
            AiPointGroup::ScheduleStatus
        } else if index >= self.schedules.ai.start {
            AiPointGroup::Schedule
        } else if index >= self.schedules_bc_status.ai.start {
            AiPointGroup::ScheduleBCStatus
        } else if index >= self.schedules_bc.ai.start {
            AiPointGroup::ScheduleBC
        } else if index >= self.extensions.ai.start {
            AiPointGroup::Extensions
        } else if index >= self.system_meter.ai.start {
            AiPointGroup::SystemMeter
        } else if index >= self.curves.ai.start {
            AiPointGroup::Curve
        } else if index >= self.functions.ai.start {
            AiPointGroup::Functions
        } else {
            AiPointGroup::ConfigAndControl
        }
    }

    /// Base points also include functional edit selectors and the running BC
    /// schedule index, which the loader moves out of their functional groups.
    pub fn is_base_ai_point(&self, index: u16) -> bool {
        match self.ai_point_group(index) {
            AiPointGroup::ConfigAndControl
            | AiPointGroup::Functions
            | AiPointGroup::SystemMeter
            | AiPointGroup::Extensions
            | AiPointGroup::ScheduleBCStatus
            | AiPointGroup::ScheduleStatus
            | AiPointGroup::Experimental
            | AiPointGroup::Vendor
            | AiPointGroup::StartIndexPoints => true,
            AiPointGroup::Curve => index == self.curves.ai.start,
            AiPointGroup::ScheduleBC => index
                .checked_sub(self.schedules_bc.ai.start)
                .is_some_and(|offset| offset <= 1),
            AiPointGroup::Schedule => index == self.schedules.ai.start,
            AiPointGroup::Meter
            | AiPointGroup::Der
            | AiPointGroup::Inverter
            | AiPointGroup::Battery => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::PicsProfile;

    #[test]
    fn ai_groups_follow_key_sheet_sections() {
        let key = &PicsProfile::load_full_profile().key;
        // AI boundaries from the full workbook's Key tab, including the last
        // index before each subsequent section starts.
        for (start, end, group, base) in [
            (0, 70, AiPointGroup::ConfigAndControl, true),
            (71, 327, AiPointGroup::Functions, true),
            (328, 532, AiPointGroup::Curve, false),
            (533, 568, AiPointGroup::SystemMeter, true),
            (569, 1999, AiPointGroup::Extensions, true),
            (2000, 2299, AiPointGroup::ScheduleBC, false),
            (2300, 2999, AiPointGroup::ScheduleBCStatus, true),
            (3000, 3499, AiPointGroup::Schedule, false),
            (3500, 4999, AiPointGroup::ScheduleStatus, true),
            (5000, 9999, AiPointGroup::Meter, false),
            (10000, 14999, AiPointGroup::Der, false),
            (15000, 19999, AiPointGroup::Inverter, false),
            (20000, 29999, AiPointGroup::Battery, false),
            (30000, 49999, AiPointGroup::Experimental, true),
            (50000, 59999, AiPointGroup::Vendor, true),
            (60000, 65535, AiPointGroup::StartIndexPoints, true),
        ] {
            assert_eq!(key.ai_point_group(start), group, "AI{start}");
            assert_eq!(key.ai_point_group(end), group, "AI{end}");
            assert_eq!(key.is_base_ai_point(end), base, "AI{end}");
        }
    }

    #[test]
    fn functional_selectors_remain_base_points() {
        let key = &PicsProfile::load_full_profile().key;
        for (index, group) in [
            (328, AiPointGroup::Curve),
            (2000, AiPointGroup::ScheduleBC),
            (2001, AiPointGroup::ScheduleBC),
            (3000, AiPointGroup::Schedule),
        ] {
            assert_eq!(key.ai_point_group(index), group, "AI{index}");
            assert!(key.is_base_ai_point(index), "AI{index}");
        }
        for index in [329, 2002, 3001] {
            assert!(!key.is_base_ai_point(index), "AI{index}");
        }
    }
}
