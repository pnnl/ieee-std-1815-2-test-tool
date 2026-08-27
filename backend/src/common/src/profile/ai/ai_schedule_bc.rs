use crate::profile::AiPoint;
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
