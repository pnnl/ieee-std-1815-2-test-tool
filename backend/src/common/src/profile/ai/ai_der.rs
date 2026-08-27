use crate::profile::AiPoint;
use serde::{Deserialize, Serialize};

/// AI points belonging to the DER equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiDer {
    pub unit_type: AiPoint,
    pub nameplate_energy_capacity: AiPoint,
    pub normal_operating_performance_category: AiPoint,
    pub abnormal_operating_performance_category: AiPoint,
    pub max_apparent_generation_power: AiPoint,
    pub max_apparent_charging_power: AiPoint,
    pub operational_time: AiPoint,
    pub connection_time: AiPoint,
    pub available_active_generation_power: AiPoint,
    pub available_active_charging_power: AiPoint,
    pub available_reactive_injection_power: AiPoint,
    pub available_reactive_absorption_power: AiPoint,
    pub non_impacting_injection_vars: AiPoint,
    pub non_impacting_absorption_vars: AiPoint,
    pub link_to_meter: AiPoint,
}

impl AiDer {
    pub fn iter_points(&self) -> Vec<&AiPoint> {
        vec![
            &self.unit_type,
            &self.nameplate_energy_capacity,
            &self.normal_operating_performance_category,
            &self.abnormal_operating_performance_category,
            &self.max_apparent_generation_power,
            &self.max_apparent_charging_power,
            &self.operational_time,
            &self.connection_time,
            &self.available_active_generation_power,
            &self.available_active_charging_power,
            &self.available_reactive_injection_power,
            &self.available_reactive_absorption_power,
            &self.non_impacting_injection_vars,
            &self.non_impacting_absorption_vars,
            &self.link_to_meter,
        ]
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let Self {
            unit_type,
            nameplate_energy_capacity,
            normal_operating_performance_category,
            abnormal_operating_performance_category,
            max_apparent_generation_power,
            max_apparent_charging_power,
            operational_time,
            connection_time,
            available_active_generation_power,
            available_active_charging_power,
            available_reactive_injection_power,
            available_reactive_absorption_power,
            non_impacting_injection_vars,
            non_impacting_absorption_vars,
            link_to_meter,
        } = self;

        vec![
            unit_type,
            nameplate_energy_capacity,
            normal_operating_performance_category,
            abnormal_operating_performance_category,
            max_apparent_generation_power,
            max_apparent_charging_power,
            operational_time,
            connection_time,
            available_active_generation_power,
            available_active_charging_power,
            available_reactive_injection_power,
            available_reactive_absorption_power,
            non_impacting_injection_vars,
            non_impacting_absorption_vars,
            link_to_meter,
        ]
    }
}
