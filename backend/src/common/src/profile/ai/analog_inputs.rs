use std::cmp::min;

use serde::{Deserialize, Serialize};

use crate::{
    profile::{
        AiBattery, AiCurve, AiDer, AiInverter, AiMeter, AiPoint, AiSchedule, AiScheduleBC,
        DEFAULT_MAX_DATABASE_ENTRIES,
        validation::{ValidationError, ValidationErrors},
    },
    uids::ai_uid::AiUid,
};

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

    pub(crate) fn collect_validation_errors(&self) -> crate::profile::validation::ValidationErrors {
        let mut errors = ValidationErrors::new();

        for curve in self.curves.iter() {
            errors.extend(curve.collect_errors())
        }

        // Schedules
        if let Some(point) = self.point_by_uid(AiUid::Scheduling_FSCC_Schd) {
            errors.extend(collect_index_errors(
                point,
                min(
                    self.schedules.len() + 1,
                    DEFAULT_MAX_DATABASE_ENTRIES.into(),
                ),
            ));
        }

        if let Some(point) = self.point_by_uid(AiUid::BC_Scheduling_BC_FSCC_Schd) {
            errors.extend(collect_index_errors(
                point,
                min(
                    self.schedules_bc.len() + 1,
                    DEFAULT_MAX_DATABASE_ENTRIES.into(),
                ),
            ));
        }

        for schedule in self.schedules.iter() {
            errors.extend(schedule.collect_validation_errors())
        }

        // Meters

        for meter in self.meters.iter() {
            errors.extend(meter.collect_errors());
        }

        for point in self.all_ai_points_full() {
            errors.extend(point.collect_errors())
        }

        errors
    }

    pub fn point_by_uid(&self, uid: AiUid) -> Option<&AiPoint> {
        self.all_ai_points_full()
            .into_iter()
            .find(|point| point.point_index == uid as u16)
    }

    pub fn add_curve(&mut self) {
        let curve = AiCurve::default();
        self.curves.push(curve);
    }
}

fn collect_index_errors(index_point: &AiPoint, max_index: usize) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    let selected_index = index_point.value().0;
    // Example: there are 4 schedules. The index is one-based, and you can select
    // a new schedule to edit, so the max index would be 5.
    if selected_index as usize > max_index {
        errors.push(ValidationError {
            point: index_point.full_index(),
            message: format!(
                "Selected index {} is out of bounds. Max expected index is {}.",
                selected_index, max_index
            ),
        });
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{
        EventClass, PicsProfile,
        values::{EngineeringF64, TransmissionI32},
    };

    fn point(index: u16) -> AiPoint {
        AiPoint::new(
            index,
            String::new(),
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
        .expect("test point must be valid")
    }

    #[test]
    fn profile_accepts_basic_range_boundaries_and_selectors() {
        let indices = [
            0,
            328,
            533,
            1999,
            2000,
            2001,
            2300,
            2999,
            3000,
            3500,
            4999,
            30000,
            u16::MAX,
        ];
        let mut profile = PicsProfile::load_full_profile().into_unvalidated();
        profile.ai.points = indices.into_iter().map(point).collect();
        profile
            .into_validated()
            .expect("basic points and selectors must be accepted");
    }

    #[test]
    fn profile_returns_errors_for_all_invalid_basic_points_and_groups() {
        let invalid_indices = [329, 532, 2002, 2299, 3001, 3499, 5000, 29999];
        let curve = AiCurve {
            curve_type: point(329),
            number_of_points: point(329),
            x_units: point(331),
            y_units: point(332),
            x_values: vec![],
            y_values: vec![],
        };
        let mut profile = PicsProfile::load_full_profile().into_unvalidated();
        profile.ai.points = invalid_indices.into_iter().map(point).collect();
        profile.ai.curves = vec![curve];
        let errors = profile
            .into_validated()
            .expect_err("invalid basic points and a duplicate curve index must be rejected");

        for index in invalid_indices {
            assert!(
                errors.errors.iter().any(|error| {
                    error.point == format!("AI{index}")
                        && error.message.contains("not a basic point")
                }),
                "missing basic point validation error for AI{index}"
            );
        }
        assert!(
            errors
                .errors
                .iter()
                .any(|error| error.message == "Duplicate AI point index")
        );
    }

    #[test]
    fn profile_uses_custom_boundaries_for_base_points_and_selectors() {
        let mut profile = PicsProfile::load_full_profile().into_unvalidated();
        profile.key.curves.ai.start = 400;
        profile.key.system_meter.ai.start = 600;
        profile.key.extensions.ai.start = 650;
        profile.key.schedules_bc.ai.start = 2100;
        profile.key.schedules.ai.start = 3100;
        profile.ai.points = [399, 400, 600, 2100, 2101, 3100]
            .into_iter()
            .map(point)
            .collect();
        let mut profile = profile
            .into_validated()
            .expect("base points and selectors must follow the Key sheet")
            .into_unvalidated();

        let invalid_indices = [401, 599, 2102, 3101];
        profile.ai.points = invalid_indices.into_iter().map(point).collect();
        let errors = profile
            .into_validated()
            .expect_err("functional points cannot be stored as base points");
        for index in invalid_indices {
            assert!(
                errors
                    .errors
                    .iter()
                    .any(|error| error.point == format!("AI{index}")),
                "missing validation error for AI{index}",
            );
        }
    }
}
