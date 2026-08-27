use test_tool_macros::ai_enum;

use crate::profile::{
    enums::AiEnum,
    profile::AiMeter,
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

impl AiMeter {
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
