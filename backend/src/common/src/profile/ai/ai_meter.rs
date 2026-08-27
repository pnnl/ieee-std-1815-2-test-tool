use serde::{Deserialize, Serialize};
use test_tool_macros::{AiEnumFields, ai_enum};

use crate::profile::{
    AiPoint,
    enums::AiEnum,
    validation::{ValidationErrors, collect_low_high_threshold_errors},
};

#[ai_enum]
pub enum ConnectionPointType {
    Unknown = 0,
    DerToLocalEps = 1,
    InternalToDer = 2,
    LocalEpsWithLoadToAreaEps = 3,
    LocalEpsWithoutLoadToAreaEps = 4,
    LoadToLocalEps = 5,
    ExternalToDerBeyondPcc = 6,
    ExternalToDerWithinLocalEps = 7,
    AuxiliaryDerLoad = 8,
    GroupOfDersToAreaEps = 9,
    Other = 99,
}

#[ai_enum]
pub enum DerIoInclusionState {
    Unknown = 0,
    ExcludeDerIo = 1,
    IncludeDerIo = 2,
    Other = 3,
}

#[ai_enum]
pub enum CircuitPhases {
    Unknown = 0,
    SinglePhase = 1,
    SplitPhase = 2,
    TwoPhase = 3,
    ThreePhaseDelta = 4,
    ThreePhaseWye = 5,
    ThreePhaseWyeGrounded = 6,
    ThreePhaseThreeWireInverterType = 7,
    ThreePhaseFourWireInverterType = 8,
}

#[ai_enum]
pub enum ApparentPowerCalcMethod {
    Unknown = 0,
    Vector = 1,
    Arithmetic = 2,
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

    pub fn collect_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        // TODO: validate these:
        // pub active_power: AiPoint,
        // pub active_power_a: AiPoint,
        // pub active_power_b: AiPoint,
        // pub active_power_c: AiPoint,
        // pub reactive_power: AiPoint,
        // pub reactive_power_a: AiPoint,
        // pub reactive_power_b: AiPoint,
        // pub reactive_power_c: AiPoint,
        // pub power_factor: AiPoint,
        // pub apparent_power: AiPoint,
        // pub phase_a_volts: AiPoint,
        // pub phase_a_angle: AiPoint,
        // pub phase_b_volts: AiPoint,
        // pub phase_b_angle: AiPoint,
        // pub phase_c_volts: AiPoint,
        // pub phase_c_angle: AiPoint,
        // pub avg_line_to_line_voltage: AiPoint,
        // pub current_a: AiPoint,
        // pub current_b: AiPoint,
        // pub current_c: AiPoint,
        errors.extend(self.collect_enum_errors());

        errors.extend(collect_low_high_threshold_errors(
            &self.active_power_low_threshold,
            &self.active_power_high_threshold,
        ));
        errors.extend(collect_low_high_threshold_errors(
            &self.reactive_power_low_threshold,
            &self.reactive_power_high_threshold,
        ));
        errors.extend(collect_low_high_threshold_errors(
            &self.power_factor_low_threshold,
            &self.power_factor_high_threshold,
        ));
        errors.extend(collect_low_high_threshold_errors(
            &self.phase_a_volts_low_threshold,
            &self.phase_a_volts_high_threshold,
        ));
        errors.extend(collect_low_high_threshold_errors(
            &self.phase_b_volts_low_threshold,
            &self.phase_b_volts_high_threshold,
        ));
        errors.extend(collect_low_high_threshold_errors(
            &self.phase_c_volts_low_threshold,
            &self.phase_c_volts_high_threshold,
        ));

        errors
    }
}
