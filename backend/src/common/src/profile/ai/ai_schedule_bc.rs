use crate::profile::{
    AiPoint,
    validation::{
        ValidationError, ValidationErrors, collect_duplicate_ai_errors, is_whole_number_in_range,
    },
};
use serde::{Deserialize, Serialize};

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
    pub fn collect_validation_errors(&self) -> ValidationErrors {
        let mut errors = collect_duplicate_ai_errors(self.iter_points());
        let count = self.number_of_points.value();
        if !is_whole_number_in_range(count, 0, 100) {
            errors.push(ValidationError {
                point: self.number_of_points.full_index(),
                message: format!(
                    "BC schedule point count must be a whole number between 0 and 100, got {count}"
                ),
            });
        }
        if self.time_offsets.len() != self.values.len() || count.0 != self.time_offsets.len() as f64
        {
            errors.push(ValidationError {
                point: self.number_of_points.full_index(),
                message: format!(
                    "BC schedule time offsets, values, and point count must have the same length. \
                     Lengths are time_offsets: {}, values: {}, number_of_points: {count}",
                    self.time_offsets.len(),
                    self.values.len(),
                ),
            });
        }
        if self.time_offsets.len() > 100 || self.values.len() > 100 {
            errors.push(ValidationError {
                point: self.number_of_points.full_index(),
                message: "BC schedule exceeds the maximum of 100 points".to_string(),
            });
        }
        errors
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{PicsProfile, values::EngineeringF64};

    fn schedule() -> AiScheduleBC {
        PicsProfile::load_full_profile().ai.schedules_bc[0].clone()
    }

    #[test]
    fn rejects_invalid_point_counts() {
        for count in [-1.0, 0.5, 1.5, 101.0, 910615532.0, f64::NAN, f64::INFINITY] {
            let mut schedule = schedule();
            schedule
                .number_of_points
                .set_value_unsafe(EngineeringF64(count));
            assert!(
                !schedule.collect_validation_errors().is_empty(),
                "BC schedule accepted point count {count}"
            );
        }
    }

    #[test]
    fn rejects_unpaired_arrays_and_count_mismatches() {
        let mut schedule = schedule();
        schedule.values.clear();
        assert!(!schedule.collect_validation_errors().is_empty());
        schedule.time_offsets.clear();
        assert!(!schedule.collect_validation_errors().is_empty());
    }

    #[test]
    fn accepts_matching_counts_including_zero_and_one_hundred() {
        for count in [0, 1, 100] {
            let mut schedule = schedule();
            let time = schedule.time_offsets[0].clone();
            let value = schedule.values[0].clone();
            schedule.time_offsets.clear();
            schedule.values.clear();
            for slot in 0..count {
                let mut time = time.clone();
                time.point_index = 2012 + slot * 2;
                schedule.time_offsets.push(time);
                let mut value = value.clone();
                value.point_index = 2013 + slot * 2;
                schedule.values.push(value);
            }
            schedule
                .number_of_points
                .set_value(EngineeringF64(count.into()))
                .unwrap();
            assert!(schedule.collect_validation_errors().is_empty());
        }
    }

    #[test]
    fn rejects_more_than_one_hundred_paired_points() {
        let mut schedule = schedule();
        schedule
            .time_offsets
            .resize(101, schedule.time_offsets[0].clone());
        schedule.values.resize(101, schedule.values[0].clone());
        schedule
            .number_of_points
            .set_value(EngineeringF64(101.0))
            .unwrap();
        assert!(
            schedule
                .collect_validation_errors()
                .to_string()
                .contains("maximum of 100")
        );
    }

    #[test]
    fn profile_validation_rejects_and_restores_the_reported_bad_count() {
        let mut profile = PicsProfile::load_full_profile().into_unvalidated();
        let original = profile.ai.schedules_bc[0].number_of_points.clone();
        let errors = profile
            .set_ai_point(&original, |point| {
                point.set_value(EngineeringF64(910615532.0))
            })
            .unwrap_err();
        assert!(
            errors
                .errors
                .iter()
                .any(|e| e.point == original.full_index())
        );
        assert_eq!(profile.ai.schedules_bc[0].number_of_points, original);
        assert!(profile.into_validated().is_ok());
    }
}
