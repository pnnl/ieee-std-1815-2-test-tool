use serde::{Deserialize, Serialize};

use crate::profile::{
    AiBattery, AiCurve, AiDer, AiInverter, AiMeter, AiPoint, AiSchedule, AiScheduleBC,
    validation::ValidationErrors,
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

        for schedule in self.schedules.iter() {
            errors.extend(schedule.collect_validation_errors())
        }

        for meter in self.meters.iter() {
            errors.extend(meter.collect_errors());
        }

        for point in self.all_ai_points_full() {
            errors.extend(point.collect_errors())
        }

        errors
    }
}
