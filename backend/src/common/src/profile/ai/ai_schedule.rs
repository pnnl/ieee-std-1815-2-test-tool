use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::profile::{
    AiPoint,
    validation::{ValidationError, ValidationErrors, collect_duplicate_ai_errors},
};

fn days_and_millis_to_datetime(days: i64, millis: i64) -> Option<DateTime<Utc>> {
    let total_duration = Duration::days(days) + Duration::milliseconds(millis);
    DateTime::from_timestamp_millis(total_duration.num_milliseconds())
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

impl AiSchedule {
    pub fn start_datetime(&self) -> DateTime<Utc> {
        days_and_millis_to_datetime(
            self.start_date.value().0 as i64,
            self.start_time.value().0 as i64,
        )
        .unwrap_or_else(|| DateTime::from_timestamp_nanos(0))
    }

    pub fn end_datetime(&self) -> DateTime<Utc> {
        days_and_millis_to_datetime(
            self.stop_date.value().0 as i64,
            self.stop_time.value().0 as i64,
        )
        .unwrap_or_else(|| DateTime::from_timestamp_nanos(1))
    }

    pub fn duration(&self) -> Duration {
        let start = self.start_datetime();
        let end = self.end_datetime();
        end - start
    }

    pub fn iter_points(&self) -> Vec<&AiPoint> {
        let mut out = vec![
            &self.identity,
            &self.priority,
            &self.start_date,
            &self.start_time,
            &self.stop_date,
            &self.stop_time,
            &self.repeat_interval,
            &self.repeat_interval_units,
            &self.validation_state,
            &self.status,
            &self.number_of_points,
        ];
        out.extend(
            self.time_offsets
                .iter()
                .chain(self.action_types.iter())
                .chain(self.action_indexes.iter())
                .chain(self.values.iter()),
        );
        out
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let AiSchedule {
            identity,
            priority,
            start_date,
            start_time,
            stop_date,
            stop_time,
            repeat_interval,
            repeat_interval_units,
            validation_state,
            status,
            number_of_points,
            time_offsets,
            action_types,
            action_indexes,
            values,
        } = self;
        let mut out: Vec<&mut AiPoint> = vec![
            identity,
            priority,
            start_date,
            start_time,
            stop_date,
            stop_time,
            repeat_interval,
            repeat_interval_units,
            validation_state,
            status,
            number_of_points,
        ];
        out.extend(time_offsets.iter_mut());
        out.extend(action_types.iter_mut());
        out.extend(action_indexes.iter_mut());
        out.extend(values.iter_mut());
        out
    }

    fn display_name(&self) -> String {
        format!("Schedule {}", self.identity.value().0)
    }

    pub fn collect_validation_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        errors.extend(collect_duplicate_ai_errors(self.iter_points()));

        if days_and_millis_to_datetime(
            self.start_date.value().0 as i64,
            self.start_time.value().0 as i64,
        )
        .is_none()
        {
            errors.push(ValidationError {
                point: self.display_name(),
                message: format!(
                    "Invalid start date or time. Days since epoch: {}, Milliseconds since midnight: {}",
                    self.start_date.value().0,
                    self.start_time.value().0
                ),
            });
        }

        if days_and_millis_to_datetime(
            self.stop_date.value().0 as i64,
            self.stop_time.value().0 as i64,
        )
        .is_none()
        {
            errors.push(ValidationError {
                point: self.display_name(),
                message: format!(
                    "Invalid stop date or time. Days since epoch: {}, Milliseconds since midnight: {}",
                    self.stop_date.value().0,
                    self.stop_time.value().0
                ),
            });
        }

        if self.time_offsets.len() != self.action_types.len()
            || self.time_offsets.len() != self.action_indexes.len()
            || self.time_offsets.len() != self.values.len()
            || self.time_offsets.len() != self.number_of_points.value().0 as usize
        {
            errors.push(ValidationError {
                point: self.display_name(),
                message: format!(
                    "Time offsets, action types, action indexes, and values must have the same length. Lengths are time_offsets: {}, action_types: {}, action_indexes: {}, values: {}, number_of_points: {}",
                    self.time_offsets.len(),
                    self.action_types.len(),
                    self.action_indexes.len(),
                    self.values.len(),
                    self.number_of_points.value().0
                ),
            });
        }

        if self.time_offsets.len() > 100 {
            errors.push(ValidationError {
                point: self.display_name(),
                message: "Number of points exceeds maximum allowed (100)".to_string(),
            });
        }

        if self.start_datetime() > self.end_datetime() {
            errors.push(ValidationError {
                point: self.display_name(),
                message: "Start datetime must be before end datetime".to_string(),
            });
        }

        let mut previous_offset = -1.0;

        for (i, time_offset) in self.time_offsets.iter().enumerate() {
            if time_offset.value().0 < 0.0 {
                errors.push(ValidationError {
                    point: format!("{}.time_offsets[{}]", self.display_name(), i),
                    message: "Time offset must be non-negative".to_string(),
                });
            }

            if time_offset.value().0 * 1000.0 > self.duration().num_milliseconds() as f64 {
                errors.push(ValidationError {
                    point: format!("{}.time_offsets[{}]", self.display_name(), i),
                    message: format!(
                        "Time offset must be within the schedule duration. Offset: {}, Schedule duration: {}",
                        time_offset.value().0,
                        self.duration().num_milliseconds() as f64 / 1000.0
                    ),
                });
            }

            if time_offset.value().0 < previous_offset {
                errors.push(ValidationError {
                    point: format!("{}.time_offsets[{}]", self.display_name(), i),
                    message: format!(
                        "Time offsets must be in non-decreasing order. Current index and value: {}, {}. Previous value: {}",
                        i,
                        time_offset.value().0,
                        previous_offset
                    ),
                });
            }

            previous_offset = time_offset.value().0;
        }

        for (i, action_type) in self.action_types.iter().enumerate() {
            if action_type.value().0 < 0.0 || action_type.value().0 > 3.0 {
                errors.push(ValidationError {
                    point: format!("{}.action_types[{}]", self.display_name(), i),
                    message: "Action type must be 0, 1, 2, or 3".to_string(),
                });
            }
        }

        if Duration::milliseconds(self.start_time.value().0 as i64) > Duration::days(1) {
            errors.push(ValidationError {
                point: format!("{}.start_time", self.display_name()),
                message: "Start time must be less than 24 hours".to_string(),
            });
        }

        if Duration::milliseconds(self.start_time.value().0 as i64) > Duration::days(1) {
            errors.push(ValidationError {
                point: format!("{}.start_time", self.display_name()),
                message: "Start time must be less than 24 hours".to_string(),
            });
        }

        errors
    }
}
