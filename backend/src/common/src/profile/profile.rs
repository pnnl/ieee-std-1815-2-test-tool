use std::collections::{HashMap, HashSet};

use crate::profile::enums::AiEnum;
use crate::profile::{
    ai_meter::{ApparentPowerCalcMethod, CircuitPhases, ConnectionPointType, DerIoInclusionState},
    scale_curve::{DependentVariableUnit, IndependentVariableUnit},
    validation::{Validate, Validated, ValidationError, ValidationErrors},
    values::{EngineeringF64, TransmissionI32},
};
use serde::{Deserialize, Serialize};
use strum::{Display, IntoEnumIterator};
use strum_macros::EnumIter;
use test_tool_macros::AiEnumFields;

/// DNP3 event class assignment for a point (Class1, Class2, Class3, or None).
/// Event class represents priority: Class1 > Class2 > Class3.
/// None means you are not expected to generate events for those points
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum EventClass {
    Class1,
    Class2,
    Class3,
    #[default]
    None,
}

/// A binary output (BO) point defined in the PICS profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BoPoint {
    pub point_index: u16,
    pub name: String,
    pub state_0: String,
    pub state_1: String,
    pub iec_61850_uid: String,
    pub assoc_bi: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

/// A binary input (BI) point defined in the PICS profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiPoint {
    pub point_index: u16,
    pub name: String,
    pub event_class: EventClass,
    pub state_0: String,
    pub state_1: String,
    pub iec_61850_uid: String,
    pub assoc_bo: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

/// An analog output (AO) point defined in the PICS profile. Do not construct it directly; instead, use AoPoint::new.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AoPoint {
    pub point_index: u16,
    pub name: String,
    minimum: TransmissionI32,
    maximum: TransmissionI32,
    multiplier: f64,
    pub offset: EngineeringF64,
    pub units: String,
    pub iec_61850_uid: String,
    pub assoc_ai: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

impl AoPoint {
    /// Construct an [`AoPoint`], validating that `multiplier` is non-zero.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        point_index: u16,
        name: String,
        minimum: TransmissionI32,
        maximum: TransmissionI32,
        multiplier: f64,
        offset: EngineeringF64,
        units: String,
        iec_61850_uid: String,
        assoc_ai: Option<String>,
        purpose: String,
        mandatory_1815: bool,
        mandatory_1547: bool,
    ) -> Result<Self, ValidationErrors> {
        let new_point = Self {
            point_index,
            name,
            minimum,
            maximum,
            multiplier,
            offset,
            units,
            iec_61850_uid,
            assoc_ai,
            purpose,
            mandatory_1815,
            mandatory_1547,
        };

        match new_point.validate() {
            Ok(_) => Ok(new_point),
            Err(e) => Err(e),
        }
    }

    pub fn multiplier(&self) -> f64 {
        self.multiplier
    }

    pub fn eng_maximum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.maximum, self.multiplier, self.offset)
    }
    pub fn eng_minimum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.minimum, self.multiplier, self.offset)
    }

    pub fn minimum(&self) -> TransmissionI32 {
        self.minimum
    }

    pub fn maximum(&self) -> TransmissionI32 {
        self.maximum
    }

    pub fn set_transmission_bounds(
        &mut self,
        new_min: TransmissionI32,
        new_max: TransmissionI32,
    ) -> Result<(), ValidationErrors> {
        let old_min = self.minimum;
        let old_max = self.maximum;
        self.minimum = new_min;
        self.maximum = new_max;
        match self.validate() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.minimum = old_min;
                self.maximum = old_max;
                Err(e)
            }
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let errors = self.collect_errors();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn collect_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        if self.minimum > self.maximum {
            errors.push(ValidationError {
                point: format!("AO{}", self.point_index),
                message: "Minimum value cannot be greater than maximum value".to_string(),
            });
        }

        if self.multiplier == 0.0 {
            errors.push(ValidationError {
                point: format!("AO{}", self.point_index),
                message: "Multiplier cannot be zero".to_string(),
            });
        }

        errors
    }
}

/// An analog input (AI) point defined in the PICS profile. Do not construct it directly; instead, use AiPoint::new.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiPoint {
    pub point_index: u16,
    pub name: String,
    pub event_class: EventClass,
    minimum: TransmissionI32,
    pub maximum: TransmissionI32,
    multiplier: f64,
    pub offset: EngineeringF64,
    pub units: String,
    pub iec_61850_uid: String,
    /// The default value of the point, in engineering units.
    value: EngineeringF64,
    pub assoc_ao: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

impl AiPoint {
    pub fn eng_maximum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.maximum, self.multiplier, self.offset)
    }
    pub fn eng_minimum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.minimum, self.multiplier, self.offset)
    }
    pub fn value(&self) -> EngineeringF64 {
        self.value
    }

    pub fn minimum(&self) -> TransmissionI32 {
        self.minimum
    }

    pub fn maximum(&self) -> TransmissionI32 {
        self.maximum
    }

    pub fn full_index(&self) -> String {
        format!("AI{}", self.point_index)
    }

    pub fn set_value(&mut self, new_value: EngineeringF64) -> Result<(), ValidationErrors> {
        let old_value = self.value;
        self.value = new_value;
        match self.validate() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.value = old_value; // rollback
                Err(e)
            }
        }
    }

    pub fn set_eng_bounds(
        &mut self,
        new_min: EngineeringF64,
        new_max: EngineeringF64,
    ) -> Result<(), ValidationErrors> {
        let old_min = self.minimum;
        let old_max = self.maximum;
        let new_transmission_min =
            TransmissionI32::try_from_engineering(new_min, self.multiplier, self.offset)?;
        let new_transmission_max =
            TransmissionI32::try_from_engineering(new_max, self.multiplier, self.offset)?;
        self.minimum = new_transmission_min;
        self.maximum = new_transmission_max;
        match self.validate() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.minimum = old_min; // rollback
                self.maximum = old_max; // rollback
                Err(e)
            }
        }
    }

    /// Set the value without validation. Use with caution.
    pub fn set_value_unsafe(&mut self, new_value: EngineeringF64) {
        self.value = new_value;
    }

    /// Construct an [`AiPoint`], validating that `multiplier` is non-zero.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        point_index: u16,
        name: String,
        event_class: EventClass,
        minimum: TransmissionI32,
        maximum: TransmissionI32,
        multiplier: f64,
        offset: EngineeringF64,
        units: String,
        iec_61850_uid: String,
        value: EngineeringF64,
        assoc_ao: Option<String>,
        purpose: String,
        mandatory_1815: bool,
        mandatory_1547: bool,
    ) -> Result<Self, ValidationErrors> {
        let new_point = Self {
            point_index,
            name,
            event_class,
            minimum,
            maximum,
            multiplier,
            offset,
            units,
            iec_61850_uid,
            value,
            assoc_ao,
            purpose,
            mandatory_1815,
            mandatory_1547,
        };

        match new_point.validate() {
            Ok(_) => Ok(new_point),
            Err(e) => Err(e),
        }
    }

    pub fn multiplier(&self) -> f64 {
        self.multiplier
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        match self.collect_errors() {
            errors if errors.is_empty() => Ok(()),
            errors => Err(errors),
        }
    }

    fn collect_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.minimum > self.maximum {
            errors.push(ValidationError {
                point: format!("AI{}", self.point_index),
                message: "Minimum value cannot be greater than maximum value".to_string(),
            });
        }

        if self.multiplier == 0.0 {
            errors.push(ValidationError {
                point: format!("AI{}", self.point_index),
                message: "Multiplier cannot be zero".to_string(),
            });
        }

        if self.value < self.eng_minimum() || self.value > self.eng_maximum() {
            errors.push(ValidationError {
                point: format!("AI{}", self.point_index),
                message: format!(
                    "Value must be between {} and {}, but is {}",
                    self.eng_minimum(),
                    self.eng_maximum(),
                    self.value
                ),
            });
        }
        errors
    }
}

/// A counter (CTR) point defined in the PICS profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CtrPoint {
    pub point_index: u16,
    pub name: String,
    pub counter_event_class: EventClass,
    pub frozen_counter_exists: bool,
    pub frozen_counter_event_class: EventClass,
    pub iec_61850_uid: String,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

// ---------------------------------------------------------------------------
// Equipment and functional group wrapper structs
// ---------------------------------------------------------------------------

/// BI points belonging to the Meter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiMeter {
    pub active_power_too_high: BiPoint,
    pub active_power_too_low: BiPoint,
    pub reactive_power_too_high: BiPoint,
    pub reactive_power_too_low: BiPoint,
    pub power_factor_too_high: BiPoint,
    pub power_factor_too_low: BiPoint,
    pub phase_a_voltage_too_high: BiPoint,
    pub phase_a_voltage_too_low: BiPoint,
    pub phase_b_voltage_too_high: BiPoint,
    pub phase_b_voltage_too_low: BiPoint,
    pub phase_c_voltage_too_high: BiPoint,
    pub phase_c_voltage_too_low: BiPoint,
    pub communication_error: BiPoint,
}

impl BiMeter {
    pub fn iter_points(&self) -> Vec<&BiPoint> {
        vec![
            &self.active_power_too_high,
            &self.active_power_too_low,
            &self.reactive_power_too_high,
            &self.reactive_power_too_low,
            &self.power_factor_too_high,
            &self.power_factor_too_low,
            &self.phase_a_voltage_too_high,
            &self.phase_a_voltage_too_low,
            &self.phase_b_voltage_too_high,
            &self.phase_b_voltage_too_low,
            &self.phase_c_voltage_too_high,
            &self.phase_c_voltage_too_low,
            &self.communication_error,
        ]
    }
}

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

/// BI points belonging to the Inverter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiInverter {
    pub active_power_too_high: BiPoint,
    pub active_power_too_low: BiPoint,
    pub reactive_power_too_high: BiPoint,
    pub reactive_power_too_low: BiPoint,
    pub frequency_too_high: BiPoint,
    pub frequency_too_low: BiPoint,
    pub dc_input_power_too_high: BiPoint,
    pub dc_input_power_too_low: BiPoint,
    pub dc_current_too_high: BiPoint,
    pub dc_current_too_low: BiPoint,
    pub dc_voltage_too_high: BiPoint,
    pub dc_voltage_too_low: BiPoint,
    pub power_factor_excitation: BiPoint,
    pub communication_error: BiPoint,
    pub local_control_mode: BiPoint,
    pub dc_contactor_closed: BiPoint,
    pub ground_fault_alarm: BiPoint,
    pub dc_over_voltage_alarm: BiPoint,
    pub dc_under_voltage_alarm: BiPoint,
    pub ac_disconnect_warning: BiPoint,
    pub dc_disconnect_warning: BiPoint,
    pub grid_disconnect_warning: BiPoint,
    pub cabinet_open_warning: BiPoint,
    pub manual_shutdown_warning: BiPoint,
    pub over_temperature_alarm: BiPoint,
    pub under_temperature_alarm: BiPoint,
    pub over_frequency_alarm: BiPoint,
    pub under_frequency_alarm: BiPoint,
    pub ac_over_voltage_alarm: BiPoint,
    pub ac_under_voltage_alarm: BiPoint,
    pub blown_string_fuse_alarm: BiPoint,
    pub memory_loss_alarm: BiPoint,
    pub hardware_test_failure: BiPoint,
    pub other_alarm: BiPoint,
    pub other_warning: BiPoint,
}

impl BiInverter {
    pub fn iter_points(&self) -> Vec<&BiPoint> {
        vec![
            &self.active_power_too_high,
            &self.active_power_too_low,
            &self.reactive_power_too_high,
            &self.reactive_power_too_low,
            &self.frequency_too_high,
            &self.frequency_too_low,
            &self.dc_input_power_too_high,
            &self.dc_input_power_too_low,
            &self.dc_current_too_high,
            &self.dc_current_too_low,
            &self.dc_voltage_too_high,
            &self.dc_voltage_too_low,
            &self.power_factor_excitation,
            &self.communication_error,
            &self.local_control_mode,
            &self.dc_contactor_closed,
            &self.ground_fault_alarm,
            &self.dc_over_voltage_alarm,
            &self.dc_under_voltage_alarm,
            &self.ac_disconnect_warning,
            &self.dc_disconnect_warning,
            &self.grid_disconnect_warning,
            &self.cabinet_open_warning,
            &self.manual_shutdown_warning,
            &self.over_temperature_alarm,
            &self.under_temperature_alarm,
            &self.over_frequency_alarm,
            &self.under_frequency_alarm,
            &self.ac_over_voltage_alarm,
            &self.ac_under_voltage_alarm,
            &self.blown_string_fuse_alarm,
            &self.memory_loss_alarm,
            &self.hardware_test_failure,
            &self.other_alarm,
            &self.other_warning,
        ]
    }
}

/// BI points belonging to the Battery equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiBattery {
    pub status_of_storage: BiPoint,
    pub communication_error: BiPoint,
    pub local_control_mode: BiPoint,
    pub dc_contactor_closed: BiPoint,
    pub is_charging: BiPoint,
    pub is_discharging: BiPoint,
    pub external_voltage_too_high: BiPoint,
    pub external_voltage_too_low: BiPoint,
    pub internal_voltage_too_high: BiPoint,
    pub internal_voltage_too_low: BiPoint,
    pub over_temperature_alarm: BiPoint,
    pub under_temperature_alarm: BiPoint,
    pub temperature_imbalance_alarm: BiPoint,
    pub over_temperature_warning: BiPoint,
    pub under_temperature_warning: BiPoint,
    pub temperature_imbalance_warning: BiPoint,
    pub over_charge_current_alarm: BiPoint,
    pub over_discharge_current_alarm: BiPoint,
    pub over_charge_current_warning: BiPoint,
    pub over_discharge_current_warning: BiPoint,
    pub voltage_imbalance_warning: BiPoint,
    pub current_imbalance_warning: BiPoint,
    pub over_voltage_alarm: BiPoint,
    pub under_voltage_alarm: BiPoint,
    pub over_voltage_warning: BiPoint,
    pub under_voltage_warning: BiPoint,
    pub over_soc_max_alarm: BiPoint,
    pub under_soc_min_alarm: BiPoint,
    pub over_soc_max_warning: BiPoint,
    pub under_soc_min_warning: BiPoint,
    pub contactor_failure: BiPoint,
    pub fan_error: BiPoint,
    pub ground_fault: BiPoint,
    pub door_open_alarm: BiPoint,
    pub configuration_error: BiPoint,
    pub configuration_warning: BiPoint,
    pub other_alarm: BiPoint,
    pub other_warning: BiPoint,
    pub fire_alarm: BiPoint,
    pub fire_supervisory_warning: BiPoint,
    pub fire_trouble_warning: BiPoint,
    pub fire_power_fault_warning: BiPoint,
    pub chiller_alarm: BiPoint,
    pub chiller_warning: BiPoint,
    pub air_handler_alarm: BiPoint,
    pub air_handler_warning: BiPoint,
    pub fluid_alarm: BiPoint,
    pub fluid_warning: BiPoint,
    pub gas_alarm: BiPoint,
    pub gas_warning: BiPoint,
    pub electrolyte_alarm: BiPoint,
    pub electrolyte_warning: BiPoint,
    pub electrical_alarm: BiPoint,
    pub electrical_warning: BiPoint,
}

impl BiBattery {
    pub fn iter_points(&self) -> Vec<&BiPoint> {
        vec![
            &self.status_of_storage,
            &self.communication_error,
            &self.local_control_mode,
            &self.dc_contactor_closed,
            &self.is_charging,
            &self.is_discharging,
            &self.external_voltage_too_high,
            &self.external_voltage_too_low,
            &self.internal_voltage_too_high,
            &self.internal_voltage_too_low,
            &self.over_temperature_alarm,
            &self.under_temperature_alarm,
            &self.temperature_imbalance_alarm,
            &self.over_temperature_warning,
            &self.under_temperature_warning,
            &self.temperature_imbalance_warning,
            &self.over_charge_current_alarm,
            &self.over_discharge_current_alarm,
            &self.over_charge_current_warning,
            &self.over_discharge_current_warning,
            &self.voltage_imbalance_warning,
            &self.current_imbalance_warning,
            &self.over_voltage_alarm,
            &self.under_voltage_alarm,
            &self.over_voltage_warning,
            &self.under_voltage_warning,
            &self.over_soc_max_alarm,
            &self.under_soc_min_alarm,
            &self.over_soc_max_warning,
            &self.under_soc_min_warning,
            &self.contactor_failure,
            &self.fan_error,
            &self.ground_fault,
            &self.door_open_alarm,
            &self.configuration_error,
            &self.configuration_warning,
            &self.other_alarm,
            &self.other_warning,
            &self.fire_alarm,
            &self.fire_supervisory_warning,
            &self.fire_trouble_warning,
            &self.fire_power_fault_warning,
            &self.chiller_alarm,
            &self.chiller_warning,
            &self.air_handler_alarm,
            &self.air_handler_warning,
            &self.fluid_alarm,
            &self.fluid_warning,
            &self.gas_alarm,
            &self.gas_warning,
            &self.electrolyte_alarm,
            &self.electrolyte_warning,
            &self.electrical_alarm,
            &self.electrical_warning,
        ]
    }
}

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

#[repr(u8)] // Specifies the underlying type as u8
#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum CurveType {
    NotDefined = 0,
    Unknown = 1,
    VoltVAr = 2,
    FrequencyWatt = 3,
    WattVAr = 4,
    VoltageWatt = 5,
    RemainConnected = 6,
    TemperatureMode = 7,
    PricingSignalMode = 8,
    HVRTMustTrip = 9,
    HVRTMomentaryCessation = 10,
    LVRTMustTrip = 11,
    LVRTMomentaryCessation = 12,
    HFRTMustTrip = 13,
    HFRTMomentaryCessation = 14,
    LFRTMustTrip = 15,
    LFRTMomentaryCessation = 16,
}

impl CurveType {
    pub fn max_value() -> i64 {
        CurveType::iter()
            .map(|curve| curve as i64)
            .max()
            .expect("CurveType enum should have at least one variant")
    }

    pub fn is_compatible_with_x_unit(&self, unit: IndependentVariableUnit) -> bool {
        unit == match self {
            CurveType::NotDefined => IndependentVariableUnit::NotDefined,
            CurveType::Unknown => IndependentVariableUnit::NotDefined,
            CurveType::VoltVAr => IndependentVariableUnit::Voltage,
            CurveType::FrequencyWatt => IndependentVariableUnit::FrequencyHz,
            CurveType::WattVAr => IndependentVariableUnit::Watts,
            CurveType::VoltageWatt => IndependentVariableUnit::VoltagePct,
            CurveType::RemainConnected => IndependentVariableUnit::TimeMs,
            CurveType::TemperatureMode => IndependentVariableUnit::Celsius,
            CurveType::PricingSignalMode => IndependentVariableUnit::PriceHundredthsLocalCurrency,
            CurveType::HVRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::HVRTMomentaryCessation => IndependentVariableUnit::TimeMs,
            CurveType::LVRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::LVRTMomentaryCessation => IndependentVariableUnit::TimeMs,
            CurveType::HFRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::HFRTMomentaryCessation => IndependentVariableUnit::TimeMs,
            CurveType::LFRTMustTrip => IndependentVariableUnit::TimeMs,
            CurveType::LFRTMomentaryCessation => IndependentVariableUnit::TimeMs,
        }
    }

    pub fn is_compatible_with_y_unit(&self, unit: DependentVariableUnit) -> bool {
        let valid_units = match self {
            CurveType::NotDefined => vec![DependentVariableUnit::NotDefined],
            CurveType::Unknown => vec![DependentVariableUnit::NotApplicable],
            CurveType::VoltVAr => vec![DependentVariableUnit::VarsPctVarMax],
            CurveType::FrequencyWatt => vec![DependentVariableUnit::WattsPctFrozenActive],
            CurveType::WattVAr => vec![
                DependentVariableUnit::VarsPctVarAvailable, // Via Table 51
                DependentVariableUnit::VarsPctVarMax,
                DependentVariableUnit::VarsPctWMax,
            ], // TODO: this is lot listed
            CurveType::VoltageWatt => vec![DependentVariableUnit::WattsPctWMax],
            CurveType::RemainConnected => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::TemperatureMode => vec![unit], // This profile does not require that an outstation support any particular Y-value units (e.g., watts) for a function type of <7> temperature or <8> price signal.
            CurveType::PricingSignalMode => vec![unit],
            CurveType::HVRTMustTrip => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::HVRTMomentaryCessation => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::LVRTMustTrip => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::LVRTMomentaryCessation => vec![DependentVariableUnit::VoltsPctVRef],
            CurveType::HFRTMustTrip => vec![DependentVariableUnit::FrequencyPctNominal],
            CurveType::HFRTMomentaryCessation => vec![DependentVariableUnit::FrequencyPctNominal],
            CurveType::LFRTMustTrip => vec![DependentVariableUnit::FrequencyPctNominal],
            CurveType::LFRTMomentaryCessation => vec![DependentVariableUnit::FrequencyPctNominal],
        };
        valid_units.contains(&unit)
    }

    pub fn compatible_x_units(&self) -> Vec<IndependentVariableUnit> {
        IndependentVariableUnit::iter()
            .filter(|unit| self.is_compatible_with_x_unit(*unit))
            .collect()
    }

    pub fn compatible_y_units(&self) -> Vec<DependentVariableUnit> {
        DependentVariableUnit::iter()
            .filter(|unit| self.is_compatible_with_y_unit(*unit))
            .collect()
    }
}

impl TryFrom<EngineeringF64> for CurveType {
    type Error = &'static str;

    fn try_from(value: EngineeringF64) -> Result<Self, Self::Error> {
        let int_value = value.0 as u8; // Convert the f64 to u8

        if value.0 != int_value as f64 {
            return Err("Value is not an integer in range of CurveType");
        }

        CurveType::try_from(int_value)
    }
}

impl TryFrom<u8> for CurveType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CurveType::NotDefined),
            1 => Ok(CurveType::Unknown),
            2 => Ok(CurveType::VoltVAr),
            3 => Ok(CurveType::FrequencyWatt),
            4 => Ok(CurveType::WattVAr),
            5 => Ok(CurveType::VoltageWatt),
            6 => Ok(CurveType::RemainConnected),
            7 => Ok(CurveType::TemperatureMode),
            8 => Ok(CurveType::PricingSignalMode),
            9 => Ok(CurveType::HVRTMustTrip),
            10 => Ok(CurveType::HVRTMomentaryCessation),
            11 => Ok(CurveType::LVRTMustTrip),
            12 => Ok(CurveType::LVRTMomentaryCessation),
            13 => Ok(CurveType::HFRTMustTrip),
            14 => Ok(CurveType::HFRTMomentaryCessation),
            15 => Ok(CurveType::LFRTMustTrip),
            16 => Ok(CurveType::LFRTMomentaryCessation),
            _ => Err("Invalid CurveType value"),
        }
    }
}

/// AI points belonging to a Curve functional group.
/// The header fields capture the single per-curve metadata points.
/// The Vec fields hold one entry per curve point, parallel-indexed.
/// Note: the curve edit selector is stored in the base AiPoints array, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiCurve {
    ///  Curve Type.  Enumeration:
    ///  <0> Curve is not defined
    ///  <1> Not applicable / Unknown
    ///  <2> Volt-Var
    ///  <3> Frequency-Watt
    ///  <4> Watt-Var
    ///  <5> Voltage-Watt
    ///  <6> Remain Connected
    ///  <7> Temperature mode
    ///  <8> Pricing signal mode
    /// High Voltage ride-through curves
    ///  <9> HVRT Must Trip
    ///  <10> HVRT Momentary Cessation
    /// Low Voltage ride-through curves
    ///  <11> LVRT Must Trip
    ///  <12> LVRT Momentary Cessation
    /// High Frequency ride-through curves
    ///  <13> HFRT Must Trip
    ///  <14> HFRT Momentary Cessation
    /// Low Frequency ride-through curves
    ///  <15> LFRT Must Trip
    ///  <16> LFRT Momentary Cessation"
    pub curve_type: AiPoint,
    pub number_of_points: AiPoint,
    pub x_units: AiPoint,
    pub y_units: AiPoint,
    pub x_values: Vec<AiPoint>,
    pub y_values: Vec<AiPoint>,
}

impl AiCurve {
    pub fn iter_points(&self) -> Vec<&AiPoint> {
        let mut out = vec![
            &self.curve_type,
            &self.number_of_points,
            &self.x_units,
            &self.y_units,
        ];
        out.extend(self.x_values.iter().chain(self.y_values.iter()));
        out
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let AiCurve {
            curve_type,
            number_of_points,
            x_units,
            y_units,
            x_values,
            y_values,
        } = self;
        let mut out: Vec<&mut AiPoint> = vec![curve_type, number_of_points, x_units, y_units];
        out.extend(x_values.iter_mut());
        out.extend(y_values.iter_mut());
        out
    }
}

/// AI points belonging to a Backward Compatible Schedule functional group.
/// The header fields capture the single per-schedule metadata points.
/// The Vec fields hold one entry per schedule slot, parallel-indexed.
/// Note: the schedule edit selector is stored in the base AiPoints array, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiScheduleBC {
    pub identity: AiPoint,
    pub priority: AiPoint,
    pub schedule_type: AiPoint,
    pub start_date: AiPoint,
    pub start_time: AiPoint,
    pub repeat_interval: AiPoint,
    pub repeat_interval_units: AiPoint,
    pub validation_status: AiPoint,
    pub status: AiPoint,
    pub number_of_points: AiPoint,
    pub time_offsets: Vec<AiPoint>,
    pub values: Vec<AiPoint>,
}

impl AiScheduleBC {
    pub fn iter_points(&self) -> Vec<&AiPoint> {
        let mut out = vec![
            &self.identity,
            &self.priority,
            &self.schedule_type,
            &self.start_date,
            &self.start_time,
            &self.repeat_interval,
            &self.repeat_interval_units,
            &self.validation_status,
            &self.status,
            &self.number_of_points,
        ];
        out.extend(self.time_offsets.iter().chain(self.values.iter()));
        out
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let AiScheduleBC {
            identity,
            priority,
            schedule_type,
            start_date,
            start_time,
            repeat_interval,
            repeat_interval_units,
            validation_status,
            status,
            number_of_points,
            time_offsets,
            values,
        } = self;
        let mut out: Vec<&mut AiPoint> = vec![
            identity,
            priority,
            schedule_type,
            start_date,
            start_time,
            repeat_interval,
            repeat_interval_units,
            validation_status,
            status,
            number_of_points,
        ];
        out.extend(time_offsets.iter_mut());
        out.extend(values.iter_mut());
        out
    }
}

#[repr(u8)] // Specifies the underlying type as u8
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Null = 0,
    AO = 1,
    BO = 2,
    Stop = 3,
}

impl TryFrom<u8> for ActionType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ActionType::Null),
            1 => Ok(ActionType::AO),
            2 => Ok(ActionType::BO),
            3 => Ok(ActionType::Stop),
            _ => Err("Invalid ActionType value"),
        }
    }
}

/// AI points belonging to an IEEE 1815.2 Schedule functional group.
/// The header fields capture the single per-schedule metadata points.
/// The Vec fields hold one entry per schedule slot, parallel-indexed.
/// Note: the schedule edit selector is stored in the base AiPoints array, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiSchedule {
    pub identity: AiPoint,
    pub priority: AiPoint,
    /// The number of days since January 1, 1970, UTC.
    pub start_date: AiPoint,
    /// Delta time in milliseconds from the start date.
    pub start_time: AiPoint,
    /// The number of days since January 1, 1970, UTC.
    pub stop_date: AiPoint,
    /// Delta time in milliseconds from the start date.
    pub stop_time: AiPoint,
    pub repeat_interval: AiPoint,
    pub repeat_interval_units: AiPoint,
    pub validation_state: AiPoint,
    pub status: AiPoint,
    pub number_of_points: AiPoint,
    /// Seconds since the start time.
    pub time_offsets: Vec<AiPoint>,
    pub action_types: Vec<AiPoint>,
    pub action_indexes: Vec<AiPoint>,
    pub values: Vec<AiPoint>,
}

/// AI points belonging to the Meter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AiEnumFields)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiMeter {
    #[ai_enum_field(ConnectionPointType)]
    pub type_of_connection_point: AiPoint,
    #[ai_enum_field(DerIoInclusionState)]
    pub der_input_output_included: AiPoint,
    #[ai_enum_field(CircuitPhases)]
    pub type_of_circuit_phases: AiPoint,
    #[ai_enum_field(ApparentPowerCalcMethod)]
    pub apparent_power_calc_method: AiPoint,
    pub frequency: AiPoint,
    pub active_power: AiPoint,
    pub active_power_a: AiPoint,
    pub active_power_b: AiPoint,
    pub active_power_c: AiPoint,
    pub reactive_power: AiPoint,
    pub reactive_power_a: AiPoint,
    pub reactive_power_b: AiPoint,
    pub reactive_power_c: AiPoint,
    pub power_factor: AiPoint,
    pub apparent_power: AiPoint,
    pub phase_a_volts: AiPoint,
    pub phase_a_angle: AiPoint,
    pub phase_b_volts: AiPoint,
    pub phase_b_angle: AiPoint,
    pub phase_c_volts: AiPoint,
    pub phase_c_angle: AiPoint,
    pub avg_line_to_line_voltage: AiPoint,
    pub current_a: AiPoint,
    pub current_b: AiPoint,
    pub current_c: AiPoint,
    pub active_power_high_threshold: AiPoint,
    pub active_power_low_threshold: AiPoint,
    pub reactive_power_high_threshold: AiPoint,
    pub reactive_power_low_threshold: AiPoint,
    pub power_factor_high_threshold: AiPoint,
    pub power_factor_low_threshold: AiPoint,
    pub phase_a_volts_high_threshold: AiPoint,
    pub phase_a_volts_low_threshold: AiPoint,
    pub phase_b_volts_high_threshold: AiPoint,
    pub phase_b_volts_low_threshold: AiPoint,
    pub phase_c_volts_high_threshold: AiPoint,
    pub phase_c_volts_low_threshold: AiPoint,
}

impl AiMeter {
    pub fn iter_points(&self) -> Vec<&AiPoint> {
        vec![
            &self.type_of_connection_point,
            &self.der_input_output_included,
            &self.type_of_circuit_phases,
            &self.apparent_power_calc_method,
            &self.frequency,
            &self.active_power,
            &self.active_power_a,
            &self.active_power_b,
            &self.active_power_c,
            &self.reactive_power,
            &self.reactive_power_a,
            &self.reactive_power_b,
            &self.reactive_power_c,
            &self.power_factor,
            &self.apparent_power,
            &self.phase_a_volts,
            &self.phase_a_angle,
            &self.phase_b_volts,
            &self.phase_b_angle,
            &self.phase_c_volts,
            &self.phase_c_angle,
            &self.avg_line_to_line_voltage,
            &self.current_a,
            &self.current_b,
            &self.current_c,
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

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let Self {
            type_of_connection_point,
            der_input_output_included,
            type_of_circuit_phases,
            apparent_power_calc_method,
            frequency,
            active_power,
            active_power_a,
            active_power_b,
            active_power_c,
            reactive_power,
            reactive_power_a,
            reactive_power_b,
            reactive_power_c,
            power_factor,
            apparent_power,
            phase_a_volts,
            phase_a_angle,
            phase_b_volts,
            phase_b_angle,
            phase_c_volts,
            phase_c_angle,
            avg_line_to_line_voltage,
            current_a,
            current_b,
            current_c,
            active_power_high_threshold,
            active_power_low_threshold,
            reactive_power_high_threshold,
            reactive_power_low_threshold,
            power_factor_high_threshold,
            power_factor_low_threshold,
            phase_a_volts_high_threshold,
            phase_a_volts_low_threshold,
            phase_b_volts_high_threshold,
            phase_b_volts_low_threshold,
            phase_c_volts_high_threshold,
            phase_c_volts_low_threshold,
        } = self;

        vec![
            type_of_connection_point,
            der_input_output_included,
            type_of_circuit_phases,
            apparent_power_calc_method,
            frequency,
            active_power,
            active_power_a,
            active_power_b,
            active_power_c,
            reactive_power,
            reactive_power_a,
            reactive_power_b,
            reactive_power_c,
            power_factor,
            apparent_power,
            phase_a_volts,
            phase_a_angle,
            phase_b_volts,
            phase_b_angle,
            phase_c_volts,
            phase_c_angle,
            avg_line_to_line_voltage,
            current_a,
            current_b,
            current_c,
            active_power_high_threshold,
            active_power_low_threshold,
            reactive_power_high_threshold,
            reactive_power_low_threshold,
            power_factor_high_threshold,
            power_factor_low_threshold,
            phase_a_volts_high_threshold,
            phase_a_volts_low_threshold,
            phase_b_volts_high_threshold,
            phase_b_volts_low_threshold,
            phase_c_volts_high_threshold,
            phase_c_volts_low_threshold,
        ]
    }
}

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

/// AI points belonging to the Inverter equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiInverter {
    pub apparent_power_calc_method: AiPoint,
    pub active_power_target: AiPoint,
    pub reactive_power_target: AiPoint,
    pub active_power: AiPoint,
    pub reactive_power: AiPoint,
    pub power_factor: AiPoint,
    pub apparent_power: AiPoint,
    pub dc_input_power: AiPoint,
    pub dc_voltage: AiPoint,
    pub dc_current: AiPoint,
    pub avg_line_to_neutral_voltage: AiPoint,
    pub voltage_phase_a_to_b: AiPoint,
    pub voltage_phase_b_to_c: AiPoint,
    pub voltage_phase_c_to_a: AiPoint,
    pub ac_current: AiPoint,
    pub current_phase_a: AiPoint,
    pub current_phase_b: AiPoint,
    pub current_phase_c: AiPoint,
    pub internal_temperature: AiPoint,
    pub heat_sink_temperature: AiPoint,
    pub transformer_temperature: AiPoint,
    pub active_power_high_threshold: AiPoint,
    pub active_power_low_threshold: AiPoint,
    pub reactive_power_high_threshold: AiPoint,
    pub reactive_power_low_threshold: AiPoint,
    pub frequency_high_threshold: AiPoint,
    pub frequency_low_threshold: AiPoint,
    pub dc_input_power_high_threshold: AiPoint,
    pub dc_input_power_low_threshold: AiPoint,
    pub dc_current_high_threshold: AiPoint,
    pub dc_current_low_threshold: AiPoint,
    pub dc_voltage_high_threshold: AiPoint,
    pub dc_voltage_low_threshold: AiPoint,
    pub link_to_der_unit: AiPoint,
}

impl AiInverter {
    pub fn iter_points(&self) -> Vec<&AiPoint> {
        vec![
            &self.apparent_power_calc_method,
            &self.active_power_target,
            &self.reactive_power_target,
            &self.active_power,
            &self.reactive_power,
            &self.power_factor,
            &self.apparent_power,
            &self.dc_input_power,
            &self.dc_voltage,
            &self.dc_current,
            &self.avg_line_to_neutral_voltage,
            &self.voltage_phase_a_to_b,
            &self.voltage_phase_b_to_c,
            &self.voltage_phase_c_to_a,
            &self.ac_current,
            &self.current_phase_a,
            &self.current_phase_b,
            &self.current_phase_c,
            &self.internal_temperature,
            &self.heat_sink_temperature,
            &self.transformer_temperature,
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
            &self.link_to_der_unit,
        ]
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let Self {
            apparent_power_calc_method,
            active_power_target,
            reactive_power_target,
            active_power,
            reactive_power,
            power_factor,
            apparent_power,
            dc_input_power,
            dc_voltage,
            dc_current,
            avg_line_to_neutral_voltage,
            voltage_phase_a_to_b,
            voltage_phase_b_to_c,
            voltage_phase_c_to_a,
            ac_current,
            current_phase_a,
            current_phase_b,
            current_phase_c,
            internal_temperature,
            heat_sink_temperature,
            transformer_temperature,
            active_power_high_threshold,
            active_power_low_threshold,
            reactive_power_high_threshold,
            reactive_power_low_threshold,
            frequency_high_threshold,
            frequency_low_threshold,
            dc_input_power_high_threshold,
            dc_input_power_low_threshold,
            dc_current_high_threshold,
            dc_current_low_threshold,
            dc_voltage_high_threshold,
            dc_voltage_low_threshold,
            link_to_der_unit,
        } = self;

        vec![
            apparent_power_calc_method,
            active_power_target,
            reactive_power_target,
            active_power,
            reactive_power,
            power_factor,
            apparent_power,
            dc_input_power,
            dc_voltage,
            dc_current,
            avg_line_to_neutral_voltage,
            voltage_phase_a_to_b,
            voltage_phase_b_to_c,
            voltage_phase_c_to_a,
            ac_current,
            current_phase_a,
            current_phase_b,
            current_phase_c,
            internal_temperature,
            heat_sink_temperature,
            transformer_temperature,
            active_power_high_threshold,
            active_power_low_threshold,
            reactive_power_high_threshold,
            reactive_power_low_threshold,
            frequency_high_threshold,
            frequency_low_threshold,
            dc_input_power_high_threshold,
            dc_input_power_low_threshold,
            dc_current_high_threshold,
            dc_current_low_threshold,
            dc_voltage_high_threshold,
            dc_voltage_low_threshold,
            link_to_der_unit,
        ]
    }
}

/// AI points belonging to the Battery equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiBattery {
    pub type_of_storage: AiPoint,
    pub nameplate_actual_capacity: AiPoint,
    pub effective_capacity: AiPoint,
    pub minimum_reserve: AiPoint,
    pub maximum_reserve: AiPoint,
    pub battery_state: AiPoint,
    pub actual_state_of_charge: AiPoint,
    pub state_of_health: AiPoint,
    pub external_voltage: AiPoint,
    pub internal_voltage: AiPoint,
    pub current: AiPoint,
    pub power: AiPoint,
    pub min_cell_voltage: AiPoint,
    pub max_cell_voltage: AiPoint,
    pub min_temperature: AiPoint,
    pub max_temperature: AiPoint,
    pub external_ambient_temperature: AiPoint,
    pub internal_ambient_temperature: AiPoint,
    pub charge_current_limit: AiPoint,
    pub discharge_current_limit: AiPoint,
    pub min_voltage_limit: AiPoint,
    pub max_voltage_limit: AiPoint,
    pub connected_string_count: AiPoint,
    pub external_voltage_high_threshold: AiPoint,
    pub external_voltage_low_threshold: AiPoint,
    pub internal_voltage_high_threshold: AiPoint,
    pub internal_voltage_low_threshold: AiPoint,
    pub link_to_inverter: AiPoint,
}

impl AiBattery {
    pub fn iter_points(&self) -> Vec<&AiPoint> {
        vec![
            &self.type_of_storage,
            &self.nameplate_actual_capacity,
            &self.effective_capacity,
            &self.minimum_reserve,
            &self.maximum_reserve,
            &self.battery_state,
            &self.actual_state_of_charge,
            &self.state_of_health,
            &self.external_voltage,
            &self.internal_voltage,
            &self.current,
            &self.power,
            &self.min_cell_voltage,
            &self.max_cell_voltage,
            &self.min_temperature,
            &self.max_temperature,
            &self.external_ambient_temperature,
            &self.internal_ambient_temperature,
            &self.charge_current_limit,
            &self.discharge_current_limit,
            &self.min_voltage_limit,
            &self.max_voltage_limit,
            &self.connected_string_count,
            &self.external_voltage_high_threshold,
            &self.external_voltage_low_threshold,
            &self.internal_voltage_high_threshold,
            &self.internal_voltage_low_threshold,
            &self.link_to_inverter,
        ]
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let Self {
            type_of_storage,
            nameplate_actual_capacity,
            effective_capacity,
            minimum_reserve,
            maximum_reserve,
            battery_state,
            actual_state_of_charge,
            state_of_health,
            external_voltage,
            internal_voltage,
            current,
            power,
            min_cell_voltage,
            max_cell_voltage,
            min_temperature,
            max_temperature,
            external_ambient_temperature,
            internal_ambient_temperature,
            charge_current_limit,
            discharge_current_limit,
            min_voltage_limit,
            max_voltage_limit,
            connected_string_count,
            external_voltage_high_threshold,
            external_voltage_low_threshold,
            internal_voltage_high_threshold,
            internal_voltage_low_threshold,
            link_to_inverter,
        } = self;

        vec![
            type_of_storage,
            nameplate_actual_capacity,
            effective_capacity,
            minimum_reserve,
            maximum_reserve,
            battery_state,
            actual_state_of_charge,
            state_of_health,
            external_voltage,
            internal_voltage,
            current,
            power,
            min_cell_voltage,
            max_cell_voltage,
            min_temperature,
            max_temperature,
            external_ambient_temperature,
            internal_ambient_temperature,
            charge_current_limit,
            discharge_current_limit,
            min_voltage_limit,
            max_voltage_limit,
            connected_string_count,
            external_voltage_high_threshold,
            external_voltage_low_threshold,
            internal_voltage_high_threshold,
            internal_voltage_low_threshold,
            link_to_inverter,
        ]
    }
}

// ---------------------------------------------------------------------------
// Sub-group structs
// ---------------------------------------------------------------------------

/// All binary output points in the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BinaryOutputs {
    pub points: Vec<BoPoint>,
}

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

/// All analog input points, grouped by base and functional/equipment type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AnalogInputs {
    /// The BASE points; to access *all* points, use all_ai_points_full() or all_ai_points_full_mut().
    pub points: Vec<AiPoint>, // TODO: make private to ensure correctness after changes
    pub curves: Vec<AiCurve>,
    pub schedules_bc: Vec<AiScheduleBC>,
    pub schedules: Vec<AiSchedule>,
    pub meters: Vec<AiMeter>,
    pub ders: Vec<AiDer>,
    pub inverters: Vec<AiInverter>,
    pub batteries: Vec<AiBattery>,
}

impl AnalogInputs {
    /// Collect all AI points from base + functional + equipment sub-groups.
    ///
    /// **Note:** Curve, schedule BC, and schedule AI points are NOT included here — use
    /// `CurveDatabase`, `ScheduleBCDatabase`, or `ScheduleDatabase` to access those.
    /// For a complete index covering all AI sub-groups, use `ProfileIndex`.
    pub fn base_ai_points(&self) -> Vec<&AiPoint> {
        let mut out: Vec<&AiPoint> = self.points.iter().collect();
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

    /// Collect every AI point across all sub-groups, including curves, schedule BC, and schedules.
    ///
    /// This is the complete set used by `ProfileIndex`. Prefer `ProfileIndex` for repeated lookups.
    pub fn all_ai_points_full(&self) -> Vec<&AiPoint> {
        let mut out = self.base_ai_points();
        for curve in &self.curves {
            out.extend(curve.iter_points());
        }
        for sched in &self.schedules_bc {
            out.extend(sched.iter_points());
        }
        for sched in &self.schedules {
            out.extend(sched.iter_points());
        }
        out
    }

    /// Collect mutable references to every AI point across all sub-groups.
    pub fn all_ai_points_full_mut(&mut self) -> Vec<&mut AiPoint> {
        let mut points: Vec<&mut AiPoint> = self.points.iter_mut().collect();
        for meter in &mut self.meters {
            points.extend(meter.iter_points_mut());
        }
        for der_unit in &mut self.ders {
            points.extend(der_unit.iter_points_mut());
        }
        for inverter in &mut self.inverters {
            points.extend(inverter.iter_points_mut());
        }
        for battery in &mut self.batteries {
            points.extend(battery.iter_points_mut());
        }
        for curve in &mut self.curves {
            points.extend(curve.iter_points_mut());
        }
        for schedule in &mut self.schedules_bc {
            points.extend(schedule.iter_points_mut());
        }
        for schedule in &mut self.schedules {
            points.extend(schedule.iter_points_mut());
        }
        points
    }
}

// ---------------------------------------------------------------------------
// KeySheet
// ---------------------------------------------------------------------------

/// Start index for a single point type within a section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SectionInfo {
    pub start: u16,
}

/// Groups the per-point-type start indices for a single section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SectionPoints {
    pub bo: SectionInfo,
    pub bi: SectionInfo,
    pub ao: SectionInfo,
    pub ai: SectionInfo,
    pub ctr: SectionInfo,
}

/// Per-instance metadata for a single point type within an equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EquipmentInfo {
    pub count: u16,
    pub start: u16,
    pub points_per: u16,
}

/// Groups the per-point-type equipment metadata for a single equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EquipmentPoints {
    pub bo: EquipmentInfo,
    pub bi: EquipmentInfo,
    pub ao: EquipmentInfo,
    pub ai: EquipmentInfo,
    pub ctr: EquipmentInfo,
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

// ---------------------------------------------------------------------------
// Top-level profile
// ---------------------------------------------------------------------------

/// Top-level deserialized representation of a PICS profile document. Unvalidated.
/// use PicsProfile::new to get Validated<PicsProfile>.
#[allow(clippy::manual_non_exhaustive)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PicsProfile {
    #[serde(rename = "Key")]
    pub key: KeySheet,
    #[serde(rename = "BO")]
    pub bo: BinaryOutputs,
    #[serde(rename = "BI")]
    pub bi: BinaryInputs,
    #[serde(rename = "AO")]
    pub ao: AnalogOutputs,
    #[serde(rename = "AI")]
    pub ai: AnalogInputs,
    #[serde(rename = "CTR")]
    pub ctr: Vec<CtrPoint>,
    #[serde(skip)]
    _private_to_force_new_constructor: (),
}

impl PicsProfile {
    pub fn new(
        key: KeySheet,
        bo: BinaryOutputs,
        bi: BinaryInputs,
        ao: AnalogOutputs,
        ai: AnalogInputs,
        ctr: Vec<CtrPoint>,
    ) -> Result<Validated<Self>, ValidationErrors> {
        let new_profile = Self {
            key,
            bo,
            bi,
            ao,
            ai,
            ctr,
            _private_to_force_new_constructor: (),
        };

        Validated::try_new(new_profile)
    }

    pub fn into_validated(self) -> Result<Validated<Self>, ValidationErrors> {
        Validated::try_new(self)
    }

    /// Update an AI point and validate the complete profile.
    ///
    /// The method uses `point_index` to find the point. If the update fails or makes
    /// the profile invalid, the method restores the initial point.
    ///
    /// Validation of the complete profile can decrease performance. To improve
    /// performance, validate only the point and the structures that contain it.
    pub fn set_ai_point(
        &mut self,
        point: &AiPoint,
        update: impl FnOnce(&mut AiPoint) -> Result<(), ValidationErrors>,
    ) -> Result<(), ValidationErrors> {
        let (initial_point, updated_point_index) = {
            let Some(target_point) = self
                .ai
                .all_ai_points_full_mut()
                .into_iter()
                .find(|candidate_point| candidate_point.point_index == point.point_index)
            else {
                return Err(ValidationErrors::from_error(ValidationError {
                    point: point.full_index(),
                    message: "Point was not found in the profile".to_string(),
                }));
            };

            let initial_point = target_point.clone();
            if let Err(errors) = update(target_point) {
                *target_point = initial_point;
                return Err(errors);
            }
            (initial_point, target_point.point_index)
        };

        if let Err(errors) = self.validate() {
            let target_point = self
                .ai
                .all_ai_points_full_mut()
                .into_iter()
                .find(|candidate_point| candidate_point.point_index == updated_point_index)
                .expect("updated AI point must be in the profile");
            *target_point = initial_point;
            return Err(errors);
        }

        Ok(())
    }

    /// For use with user-input data only. For internal construction, always use `PicsProfile::new()`.
    pub fn new_unsafe_for_collecting_errors(
        key: KeySheet,
        bo: BinaryOutputs,
        bi: BinaryInputs,
        ao: AnalogOutputs,
        ai: AnalogInputs,
        ctr: Vec<CtrPoint>,
    ) -> (Self, ValidationErrors) {
        let new_profile = Self {
            key,
            bo,
            bi,
            ao,
            ai,
            ctr,
            _private_to_force_new_constructor: (),
        };

        match new_profile.validate() {
            Ok(_) => (new_profile.clone(), ValidationErrors::new()),
            Err(errors) => (new_profile.clone(), errors),
        }
    }

    pub fn load_full_profile() -> Validated<PicsProfile> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/profiles/full.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        PicsProfile::into_validated(
            serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display())),
        )
        .expect("failed to validate full.json profile")
    }
}

impl Validate for PicsProfile {
    fn collect_validation_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        for curve in self.ai.curves.iter() {
            errors.extend(curve.collect_errors())
        }

        for schedule in self.ai.schedules.iter() {
            errors.extend(schedule.collect_validation_errors())
        }

        for meter in self.ai.meters.iter() {
            errors.extend(meter.collect_errors());
        }

        // Validate individual points
        for point in self.ao.all_ao_points() {
            errors.extend(point.collect_errors())
        }

        for point in self.ai.all_ai_points_full() {
            errors.extend(point.collect_errors())
        }

        errors
    }
}

// ---------------------------------------------------------------------------
// ProfileIndex — precomputed O(1) lookup maps derived from PicsProfile
// ---------------------------------------------------------------------------

/// Precomputed index derived from a `PicsProfile` for fast O(1) lookups.
///
/// Build once at startup with [`ProfileIndex::from_profile`] and share via `Arc`.
/// Eliminates per-request `Vec` allocations and linear scans in control handlers.
///
/// Also fixes a correctness issue: the curve/schedule selector AI points
/// (AI 328, AI 2000, AI 3000) live in sub-groups excluded by `base_ai_points()`.
/// The association maps here include those points, so AO 244 → AI 328 etc.
/// resolve correctly.
pub struct ProfileIndex {
    /// AO index → associated AI index (from `assoc_ai` field on each AO point).
    /// Includes curve/schedule selector associations.
    pub ao_to_ai: HashMap<u16, u16>,
    /// AI index → associated AO index (reverse of `ao_to_ai`).
    pub ai_to_ao: HashMap<u16, u16>,
    /// BO index → associated BI index (from `assoc_bi` field on each BO point).
    pub bo_to_bi: HashMap<u16, u16>,
    /// BI index → associated BO index (reverse of `bo_to_bi`).
    pub bi_to_bo: HashMap<u16, u16>,
    /// All AI points (base + curves + schedules + equipment) keyed by point index.
    pub ai_points: HashMap<u16, AiPoint>,
    /// All BI points keyed by point index.
    pub bi_points: HashMap<u16, BiPoint>,
    /// All AO points keyed by point index.
    pub ao_points: HashMap<u16, AoPoint>,
    /// All BO points keyed by point index.
    pub bo_points: HashMap<u16, BoPoint>,
    /// AI indices belonging to base/equipment sub-groups only (excludes curves and schedules).
    ///
    /// Used to distinguish plain AO→AI pairs (which the control loop syncs) from
    /// multiplexed curve/schedule AO→AI pairs (which the curve/schedule writers manage).
    pub base_ai_indices: HashSet<u16>,
}

impl ProfileIndex {
    /// Build a `ProfileIndex` from a `PicsProfile`.
    ///
    /// Iterates all point sub-groups once; all subsequent lookups are O(1).
    pub fn from_profile(profile: &Validated<PicsProfile>) -> Self {
        // --- Collect all AI points (including curves + schedules) ---
        let mut ai_points: HashMap<u16, AiPoint> = HashMap::new();
        for pt in profile.ai.all_ai_points_full() {
            ai_points.insert(pt.point_index, pt.clone());
        }

        // --- Collect all BI points ---
        let mut bi_points: HashMap<u16, BiPoint> = HashMap::new();
        for pt in profile.bi.all_bi_points() {
            bi_points.insert(pt.point_index, pt.clone());
        }

        // --- Collect all AO and BO points ---
        let mut ao_points: HashMap<u16, AoPoint> = HashMap::new();
        for pt in profile.ao.all_ao_points() {
            ao_points.insert(pt.point_index, pt.clone());
        }
        let mut bo_points: HashMap<u16, BoPoint> = HashMap::new();
        for pt in &profile.bo.points {
            bo_points.insert(pt.point_index, pt.clone());
        }

        // --- Build bidirectional association maps ---
        // assoc_ai and assoc_bi fields use "AI{n}" / "BI{n}" index strings (e.g. "AI277"),
        // not descriptive names, so parse the numeric index directly.
        let mut ao_to_ai: HashMap<u16, u16> = HashMap::new();
        let mut ai_to_ao: HashMap<u16, u16> = HashMap::new();
        for pt in profile.ao.all_ao_points() {
            if let Some(assoc) = &pt.assoc_ai {
                if let Some(idx_str) = assoc.strip_prefix("AI") {
                    if let Ok(ai_idx) = idx_str.parse::<u16>() {
                        ao_to_ai.insert(pt.point_index, ai_idx);
                        ai_to_ao.insert(ai_idx, pt.point_index);
                    }
                }
            }
        }

        let mut bo_to_bi: HashMap<u16, u16> = HashMap::new();
        let mut bi_to_bo: HashMap<u16, u16> = HashMap::new();
        for pt in &profile.bo.points {
            if let Some(assoc) = &pt.assoc_bi {
                if let Some(idx_str) = assoc.strip_prefix("BI") {
                    if let Ok(bi_idx) = idx_str.parse::<u16>() {
                        bo_to_bi.insert(pt.point_index, bi_idx);
                        bi_to_bo.insert(bi_idx, pt.point_index);
                    }
                }
            }
        }

        let base_ai_indices: HashSet<u16> = profile
            .ai
            .base_ai_points()
            .into_iter()
            .map(|pt| pt.point_index)
            .collect();

        Self {
            ao_to_ai,
            ai_to_ao,
            bo_to_bi,
            bi_to_bo,
            ai_points,
            bi_points,
            ao_points,
            bo_points,
            base_ai_indices,
        }
    }
}

#[cfg(test)]
mod profile_index_tests {
    use super::*;

    fn make_ai(index: u16, name: &str) -> AiPoint {
        AiPoint::new(
            index,
            name.to_string(),
            EventClass::None,
            TransmissionI32(0),
            TransmissionI32(100),
            1.0,
            EngineeringF64(0.0),
            String::new(),
            String::new(),
            EngineeringF64(0.0),
            None,
            String::new(),
            false,
            false,
        )
        .expect("test AiPoint must be valid")
    }

    fn make_ao(index: u16, name: &str, assoc_ai: Option<&str>) -> AoPoint {
        AoPoint::new(
            index,
            name.to_string(),
            TransmissionI32(0),
            TransmissionI32(100),
            1.0,
            EngineeringF64(0.0),
            String::new(),
            String::new(),
            assoc_ai.map(String::from),
            String::new(),
            false,
            false,
        )
        .expect("test AoPoint must be valid")
    }

    fn make_bi(index: u16, name: &str) -> BiPoint {
        BiPoint {
            point_index: index,
            name: name.to_string(),
            event_class: EventClass::None,
            state_0: String::new(),
            state_1: String::new(),
            iec_61850_uid: String::new(),
            assoc_bo: None,
            purpose: String::new(),
            mandatory_1815: false,
            mandatory_1547: false,
        }
    }

    fn make_bo(index: u16, assoc_bi: Option<&str>) -> BoPoint {
        BoPoint {
            point_index: index,
            name: String::new(),
            state_0: String::new(),
            state_1: String::new(),
            iec_61850_uid: String::new(),
            assoc_bi: assoc_bi.map(String::from),
            purpose: String::new(),
            mandatory_1815: false,
            mandatory_1547: false,
        }
    }

    fn make_minimal_profile() -> Validated<PicsProfile> {
        use crate::profile::profile::{
            AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs, EquipmentInfo,
            EquipmentPoints, KeySheet, SectionInfo, SectionPoints,
        };
        let empty_section = SectionPoints {
            bo: SectionInfo { start: 0 },
            bi: SectionInfo { start: 0 },
            ao: SectionInfo { start: 0 },
            ai: SectionInfo { start: 0 },
            ctr: SectionInfo { start: 0 },
        };
        let empty_equipment = EquipmentPoints {
            bo: EquipmentInfo {
                count: 0,
                start: 0,
                points_per: 0,
            },
            bi: EquipmentInfo {
                count: 0,
                start: 0,
                points_per: 0,
            },
            ao: EquipmentInfo {
                count: 0,
                start: 0,
                points_per: 0,
            },
            ai: EquipmentInfo {
                count: 0,
                start: 0,
                points_per: 0,
            },
            ctr: EquipmentInfo {
                count: 0,
                start: 0,
                points_per: 0,
            },
        };
        PicsProfile::new(
            KeySheet {
                config: empty_section.clone(),
                functions: empty_section.clone(),
                curves: empty_section.clone(),
                system_meter: empty_section.clone(),
                extensions: empty_section.clone(),
                experimental: empty_section.clone(),
                vendor: empty_section.clone(),
                discovery: empty_section.clone(),
                schedules_bc: empty_section.clone(),
                schedules_bc_status: empty_section.clone(),
                schedules: empty_section.clone(),
                schedules_status: empty_section.clone(),
                max_points: 0,
                meter: empty_equipment.clone(),
                der: empty_equipment.clone(),
                inverter: empty_equipment.clone(),
                battery: empty_equipment,
            },
            BinaryOutputs {
                points: vec![make_bo(0, Some("BI11")), make_bo(1, None)],
            },
            BinaryInputs {
                points: vec![make_bi(11, "BI11"), make_bi(12, "BI12")],
                meters: vec![],
                ders: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            AnalogOutputs {
                points: vec![
                    make_ao(10, "AO10", Some("AI20")),
                    make_ao(11, "AO11", None),
                    make_ao(244, "AO244", Some("AI328")), // curve selector
                ],
                meters: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            AnalogInputs {
                points: vec![make_ai(20, "AI20"), make_ai(328, "AI328")],
                curves: vec![AiCurve {
                    curve_type: make_ai(329, "AI329"),
                    number_of_points: make_ai(330, "AI330"),
                    x_units: make_ai(331, "AI331"),
                    y_units: make_ai(332, "AI332"),
                    x_values: vec![],
                    y_values: vec![],
                }],
                schedules_bc: vec![],
                schedules: vec![],
                meters: vec![],
                ders: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            vec![],
        )
        .expect("test profile must be valid")
    }

    #[test]
    fn test_ao_to_ai_base_point() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(idx.ao_to_ai.get(&10), Some(&20));
    }

    #[test]
    fn test_ao_without_assoc_ai_not_in_map() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert!(!idx.ao_to_ai.contains_key(&11));
    }

    #[test]
    fn test_ao_to_ai_curve_selector_resolved() {
        // AO 244 → AI 328 which is in AI.curves, not AI.points
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(
            idx.ao_to_ai.get(&244),
            Some(&328),
            "curve selector AI must be resolved"
        );
    }

    #[test]
    fn test_ai_to_ao_reverse_mapping() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(idx.ai_to_ao.get(&20), Some(&10));
        assert_eq!(idx.ai_to_ao.get(&328), Some(&244));
    }

    #[test]
    fn test_set_ai_point_rejects_an_invalid_profile() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let curve_type_point = profile.ai.curves[0].curve_type.clone();

        let errors = profile
            .set_ai_point(&curve_type_point, |point| {
                point.set_value(EngineeringF64(CurveType::VoltVAr as u8 as f64))
            })
            .expect_err("the curve type is not compatible with the initial units");

        assert!(!errors.is_empty());
        assert_eq!(
            profile.ai.curves[0].curve_type.value(),
            curve_type_point.value(),
            "the method must restore the point after an invalid update"
        );
        profile
            .validate()
            .expect("the profile must be valid after the method restores the point");
    }

    #[test]
    fn test_set_ai_point_accepts_a_valid_update() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let point = profile.ai.points[0].clone();

        profile
            .set_ai_point(&point, |point| point.set_value(EngineeringF64(50.0)))
            .expect("the profile must accept a value in the permitted range");

        assert_eq!(profile.ai.points[0].value(), EngineeringF64(50.0));
    }

    #[test]
    fn test_set_ai_point_returns_an_error_when_point_is_missing() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let missing_point = make_ai(999, "AI999");

        let errors = profile
            .set_ai_point(&missing_point, |_| Ok(()))
            .expect_err("a missing point must cause a validation error");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors.errors[0].point, "AI999");
    }

    #[test]
    fn test_bo_to_bi_and_reverse() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(idx.bo_to_bi.get(&0), Some(&11));
        assert!(!idx.bo_to_bi.contains_key(&1));
        assert_eq!(idx.bi_to_bo.get(&11), Some(&0));
    }

    #[test]
    fn test_ai_points_includes_curve_point() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        // AI 328 is in curves, not base points — must still be in ai_points
        assert!(idx.ai_points.contains_key(&328));
        assert_eq!(idx.ai_points[&328].name, "AI328");
    }

    #[test]
    fn test_ao_points_and_bo_points_populated() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert!(idx.ao_points.contains_key(&10));
        assert!(idx.bo_points.contains_key(&0));
        assert!(idx.bo_points.contains_key(&1));
    }

    #[test]
    fn test_curve_type_roundtrip() {
        for val in 0..=16 {
            let curve_type = CurveType::try_from(val).unwrap();
            assert_eq!(curve_type as u8, val);
        }
        assert!(CurveType::try_from(17).is_err());
    }

    #[test]
    fn test_action_type_roundtrip() {
        for val in 0..=3 {
            let action_type = ActionType::try_from(val).unwrap();
            assert_eq!(action_type as u8, val);
        }
        assert!(ActionType::try_from(4).is_err());
    }
}
