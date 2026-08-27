//! In-memory IEEE 1815.2 schedule database built from PicsProfile schedule data.

use std::collections::{HashMap, HashSet};

use crate::profile::validation::{Validated, ValidationErrors};
use crate::profile::values::TransmissionI32;
use crate::uids::ai_uid::AiUid;

use super::indexed_db::{AiValue, DatabaseEntry, IndexedEntryDatabase};
use super::profile::{AiSchedule, PicsProfile};

/// Current values and AI indices for a single IEEE 1815.2 schedule.
#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    /// AI index and value for identity.
    pub identity: AiValue,
    /// AI index and value for priority.
    pub priority: AiValue,
    /// AI index and value for start date.
    pub start_date: AiValue,
    /// AI index and value for start time.
    pub start_time: AiValue,
    /// AI index and value for stop date.
    pub stop_date: AiValue,
    /// AI index and value for stop time.
    pub stop_time: AiValue,
    /// AI index and value for repeat interval.
    pub repeat_interval: AiValue,
    /// AI index and value for repeat interval units.
    pub repeat_interval_units: AiValue,
    /// AI index and value for validation state.
    pub validation_state: AiValue,
    /// AI index and value for status.
    pub status: AiValue,
    /// AI index and value for number-of-points.
    pub number_of_points: AiValue,
    /// AI index and value for time offset slots.
    pub time_offsets: Vec<AiValue>,
    /// AI index and value for action type slots.
    pub action_types: Vec<AiValue>,
    /// AI index and value for action index slots.
    pub action_indexes: Vec<AiValue>,
    /// AI index and value for value slots.
    pub values: Vec<AiValue>,
    /// AI indices that have been explicitly written via a control operation.
    written: HashSet<u16>,
}

impl TryFrom<AiSchedule> for ScheduleEntry {
    type Error = ValidationErrors;

    fn try_from(schedule: AiSchedule) -> Result<Self, Self::Error> {
        Ok(Self {
            identity: AiValue::try_from(&schedule.identity)?,
            priority: AiValue::try_from(&schedule.priority)?,
            start_date: AiValue::try_from(&schedule.start_date)?,
            start_time: AiValue::try_from(&schedule.start_time)?,
            stop_date: AiValue::try_from(&schedule.stop_date)?,
            stop_time: AiValue::try_from(&schedule.stop_time)?,
            repeat_interval: AiValue::try_from(&schedule.repeat_interval)?,
            repeat_interval_units: AiValue::try_from(&schedule.repeat_interval_units)?,
            validation_state: AiValue::try_from(&schedule.validation_state)?,
            status: AiValue::try_from(&schedule.status)?,
            number_of_points: AiValue::try_from(&schedule.number_of_points)?,
            time_offsets: schedule
                .time_offsets
                .iter()
                .map(AiValue::try_from)
                .collect::<Result<_, _>>()?,
            action_types: schedule
                .action_types
                .iter()
                .map(AiValue::try_from)
                .collect::<Result<_, _>>()?,
            action_indexes: schedule
                .action_indexes
                .iter()
                .map(AiValue::try_from)
                .collect::<Result<_, _>>()?,
            values: schedule
                .values
                .iter()
                .map(AiValue::try_from)
                .collect::<Result<_, _>>()?,
            written: HashSet::new(),
        })
    }
}

impl DatabaseEntry for ScheduleEntry {
    fn all_values(&self) -> Vec<AiValue> {
        let mut out = vec![
            self.identity,
            self.priority,
            self.start_date,
            self.start_time,
            self.stop_date,
            self.stop_time,
            self.repeat_interval,
            self.repeat_interval_units,
            self.validation_state,
            self.status,
            self.number_of_points,
        ];
        out.extend_from_slice(&self.time_offsets);
        out.extend_from_slice(&self.action_types);
        out.extend_from_slice(&self.action_indexes);
        out.extend_from_slice(&self.values);
        out
    }

    fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) -> bool {
        for field in [
            &mut self.identity,
            &mut self.priority,
            &mut self.start_date,
            &mut self.start_time,
            &mut self.stop_date,
            &mut self.stop_time,
            &mut self.repeat_interval,
            &mut self.repeat_interval_units,
            &mut self.validation_state,
            &mut self.status,
            &mut self.number_of_points,
        ] {
            if field.index == ai_index {
                field.value = new_value;
                self.written.insert(ai_index);
                return true;
            }
        }
        for field in self
            .time_offsets
            .iter_mut()
            .chain(self.action_types.iter_mut())
            .chain(self.action_indexes.iter_mut())
            .chain(self.values.iter_mut())
        {
            if field.index == ai_index {
                field.value = new_value;
                self.written.insert(ai_index);
                return true;
            }
        }
        false
    }

    fn blank(&self) -> Self {
        Self {
            identity: AiValue::new(self.identity.index, 0),
            priority: AiValue::new(self.priority.index, 0),
            start_date: AiValue::new(self.start_date.index, 0),
            start_time: AiValue::new(self.start_time.index, 0),
            stop_date: AiValue::new(self.stop_date.index, 0),
            stop_time: AiValue::new(self.stop_time.index, 0),
            repeat_interval: AiValue::new(self.repeat_interval.index, 0),
            repeat_interval_units: AiValue::new(self.repeat_interval_units.index, 0),
            validation_state: AiValue::new(self.validation_state.index, 0),
            status: AiValue::new(self.status.index, 0),
            number_of_points: AiValue::new(self.number_of_points.index, 0),
            time_offsets: self
                .time_offsets
                .iter()
                .map(|v| AiValue::new(v.index, 0))
                .collect(),
            action_types: self
                .action_types
                .iter()
                .map(|v| AiValue::new(v.index, 0))
                .collect(),
            action_indexes: self
                .action_indexes
                .iter()
                .map(|v| AiValue::new(v.index, 0))
                .collect(),
            values: self
                .values
                .iter()
                .map(|v| AiValue::new(v.index, 0))
                .collect(),
            written: HashSet::new(),
        }
    }

    fn received_values(&self) -> Vec<AiValue> {
        self.all_values()
            .into_iter()
            .filter(|v| self.written.contains(&v.index))
            .collect()
    }
}

/// In-memory database of all IEEE 1815.2 schedule entries, keyed by 1-based schedule number.
pub struct ScheduleDatabase {
    inner_db: IndexedEntryDatabase<ScheduleEntry>,
    /// UID of the schedule-to-edit-selector readback point.
    selector_ai_uid: AiUid,
    /// Currently selected schedule number (1-based). Defaults to 1.
    current_entry: u16,
}

impl ScheduleDatabase {
    /// Build a ScheduleDatabase from the IEEE 1815.2 schedules in a PicsProfile.
    ///
    /// `max_schedules` sets the upper bound for selector validation; entries beyond those
    /// in the profile are created blank on demand when `set_active_entry` is called.
    ///
    /// If the profile contains no schedules, the database is built empty: reads return
    /// `None`, `set_active_entry` returns `None` (no template to clone blanks from), and
    /// `update_value` is a no-op. Profiles without schedule features are valid input.
    pub fn from_profile(
        profile: &Validated<PicsProfile>,
        max_schedules: u16,
        selector_ai_uid: AiUid,
    ) -> Self {
        let mut db = IndexedEntryDatabase::new(HashMap::new(), max_schedules);

        for (idx, schedule) in profile.ai.schedules.iter().enumerate() {
            let schedule_number = (idx as u16) + 1;
            let entry = ScheduleEntry::try_from(schedule.clone()).unwrap_or_else(|e| {
                panic!(
                    "schedule {} does not have valid values. Error: {}",
                    schedule_number, e
                )
            });
            db.insert(schedule_number, entry);
        }
        db.set_current_entry_points(1);

        Self {
            inner_db: db,
            selector_ai_uid,
            current_entry: 1,
        }
    }

    /// All AI values for the currently selected schedule entry, kept in sync with any updates.
    pub fn current_entry_points(&self) -> Vec<AiValue> {
        self.inner_db.current_entry_points()
    }

    /// Look up a schedule entry by 1-based schedule number.
    pub fn get(&self, schedule_number: u16) -> Option<&ScheduleEntry> {
        self.inner_db.get(schedule_number)
    }

    /// Look up a schedule entry mutably by 1-based schedule number.
    pub fn get_mut(&mut self, schedule_number: u16) -> Option<&mut ScheduleEntry> {
        self.inner_db.get_mut(schedule_number)
    }

    /// Set the currently active schedule number (1-based) and return all AI values for that entry.
    ///
    /// If the entry does not exist but is within `max_schedules`, a blank entry is created.
    /// Returns `None` if `entry_number` is out of range or no template entry exists.
    /// The returned values should be pushed into the DNP3 database.
    ///
    /// # TODO
    /// When schedule execution is implemented, action_type values within the entry will
    /// determine whether each action_index and value maps to an AI, BI, or other point type.
    pub fn set_active_entry(&mut self, entry_number: u16) -> Option<Vec<AiValue>> {
        let points = self
            .inner_db
            .get_or_create_entry(entry_number)
            .map(|e| e.all_values())?;
        self.inner_db.set_current_entry_points(entry_number);
        self.current_entry = entry_number;
        Some(points)
    }

    /// Update the stored raw transmitted value for `ai_index` in the currently active schedule entry.
    /// Does nothing if the AI index is not found in the active entry.
    pub fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) {
        self.inner_db
            .update_ai_value(self.current_entry, ai_index, new_value);
    }

    /// Currently active schedule number (1-based).
    pub fn current_entry(&self) -> u16 {
        self.current_entry
    }

    /// Return schedule 1, used as the template for creating blank entries with matching AI indices.
    pub fn template_entry(&self) -> Option<&ScheduleEntry> {
        self.inner_db.template_entry()
    }

    /// UID of the schedule-to-edit-selector readback point.
    pub fn selector_ai_uid(&self) -> AiUid {
        self.selector_ai_uid
    }

    /// Maximum number of schedules this database supports.
    pub fn max_schedules(&self) -> u16 {
        self.inner_db.max_entries()
    }

    /// Number of schedules in the database.
    pub fn len(&self) -> usize {
        self.inner_db.len()
    }

    /// Returns `true` if there are no schedules.
    pub fn is_empty(&self) -> bool {
        self.inner_db.is_empty()
    }

    /// Return only the AI values that have been explicitly written for the given schedule number.
    pub fn written_values(&self, schedule_number: u16) -> Option<Vec<AiValue>> {
        self.inner_db
            .get(schedule_number)
            .map(|e| e.received_values())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ActionType;
    use crate::profile::indexed_db::DatabaseEntry;
    use crate::profile::profile::{
        AiPoint, AiSchedule, AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs,
        EquipmentInfo, EquipmentPoints, EventClass, KeySheet, SectionInfo, SectionPoints,
    };
    use crate::profile::validation::Validated;
    use crate::profile::values::EngineeringF64;

    fn make_ai_point(index: u16, value: f64) -> AiPoint {
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
            EngineeringF64(value),
            None,
            String::new(),
            false,
            false,
        )
        .expect("test AiPoint must be valid")
    }

    fn make_schedule() -> AiSchedule {
        AiSchedule {
            identity: make_ai_point(3001, 1.0),
            priority: make_ai_point(3002, 0.0),
            start_date: make_ai_point(3003, 0.0),
            start_time: make_ai_point(3004, 0.0),
            stop_date: make_ai_point(3005, 0.0),
            stop_time: make_ai_point(3006, 0.0),
            repeat_interval: make_ai_point(3007, 0.0),
            repeat_interval_units: make_ai_point(3008, 0.0),
            validation_state: make_ai_point(3009, 0.0),
            status: make_ai_point(3010, 0.0),
            number_of_points: make_ai_point(3011, 1.0),
            time_offsets: vec![make_ai_point(3012, 0.0)],
            action_types: vec![make_ai_point(3013, ActionType::Null as u8 as f64)],
            action_indexes: vec![make_ai_point(3014, 0.0)],
            values: vec![make_ai_point(3015, 5.0)],
        }
    }

    fn make_schedule_2() -> AiSchedule {
        AiSchedule {
            identity: make_ai_point(3001, 2.0),
            priority: make_ai_point(3002, 1.0),
            start_date: make_ai_point(3003, 0.0),
            start_time: make_ai_point(3004, 0.0),
            stop_date: make_ai_point(3005, 1.0),
            stop_time: make_ai_point(3006, 1.0),
            repeat_interval: make_ai_point(3007, 1.0),
            repeat_interval_units: make_ai_point(3008, 1.0),
            validation_state: make_ai_point(3009, 1.0),
            status: make_ai_point(3010, 1.0),
            number_of_points: make_ai_point(3011, 1.0),
            time_offsets: vec![make_ai_point(3012, 30.0)],
            action_types: vec![make_ai_point(3013, ActionType::AO as u8 as f64)],
            action_indexes: vec![make_ai_point(3014, 1.0)],
            values: vec![make_ai_point(3015, 7.0)],
        }
    }

    fn make_empty_section() -> SectionPoints {
        SectionPoints {
            bo: SectionInfo { start: 0 },
            bi: SectionInfo { start: 0 },
            ao: SectionInfo { start: 0 },
            ai: SectionInfo { start: 0 },
            ctr: SectionInfo { start: 0 },
        }
    }

    fn make_empty_equipment() -> EquipmentPoints {
        EquipmentPoints {
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
        }
    }

    fn make_profile_with_schedule() -> Validated<PicsProfile> {
        make_profile_with_schedules(vec![make_schedule()])
    }

    fn make_profile_with_two_schedules() -> Validated<PicsProfile> {
        make_profile_with_schedules(vec![make_schedule(), make_schedule_2()])
    }

    fn make_profile_with_schedules(schedules: Vec<AiSchedule>) -> Validated<PicsProfile> {
        PicsProfile::new(
            KeySheet {
                config: make_empty_section(),
                functions: make_empty_section(),
                curves: make_empty_section(),
                system_meter: make_empty_section(),
                extensions: make_empty_section(),
                experimental: make_empty_section(),
                vendor: make_empty_section(),
                discovery: make_empty_section(),
                schedules_bc: make_empty_section(),
                schedules_bc_status: make_empty_section(),
                schedules: make_empty_section(),
                schedules_status: make_empty_section(),
                max_points: 0,
                meter: make_empty_equipment(),
                der: make_empty_equipment(),
                inverter: make_empty_equipment(),
                battery: make_empty_equipment(),
            },
            BinaryOutputs::default(),
            BinaryInputs::default(),
            AnalogOutputs::default(),
            AnalogInputs {
                points: vec![],
                curves: vec![],
                schedules_bc: vec![],
                schedules,
                meters: vec![],
                ders: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            vec![],
        )
        .expect("test PicsProfile must be valid")
    }

    #[test]
    fn test_from_profile_creates_entry_1() {
        let profile = make_profile_with_schedule();
        let db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        assert!(db.get(1).is_some());
        assert_eq!(db.get(1).unwrap().identity.index, 3001);
    }

    #[test]
    fn test_from_profile_only_adds_profile_entries_initially() {
        let profile = make_profile_with_schedule();
        let db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        assert_eq!(db.len(), 1);
        assert_eq!(db.get(1).unwrap().values[0].value, TransmissionI32(5));
        assert!(db.get(2).is_none());
    }

    #[test]
    fn test_from_profile_with_two_schedules() {
        let profile = make_profile_with_two_schedules();
        let db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        assert_eq!(db.len(), 2);
        assert_eq!(db.get(1).unwrap().values[0].value, TransmissionI32(5));
        assert_eq!(db.get(2).unwrap().values[0].value, TransmissionI32(7));
        assert!(db.get(3).is_none());
    }

    #[test]
    fn test_update_value_data_point() {
        let profile = make_profile_with_two_schedules();
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        db.set_active_entry(2);
        db.update_value(3015, TransmissionI32(99));
        assert_eq!(db.get(2).unwrap().values[0].value.0, 99);
        assert_eq!(db.get(1).unwrap().values[0].value.0, 5);
    }

    #[test]
    fn test_update_value_header() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 2, AiUid::Scheduling_FSCC_Schd);
        db.set_active_entry(1);
        db.update_value(3002, TransmissionI32(3));
        assert_eq!(db.get(1).unwrap().priority.value, TransmissionI32(3));
    }

    #[test]
    fn test_update_value_unknown_index_is_noop() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 2, AiUid::Scheduling_FSCC_Schd);
        db.update_value(9999, TransmissionI32(99));
    }

    #[test]
    fn test_update_value_marks_written() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 2, AiUid::Scheduling_FSCC_Schd);
        db.update_value(3002, TransmissionI32(3));
        let written = db.written_values(1).unwrap();
        assert!(written.iter().any(|v| v.index == 3002));
        assert!(!written.iter().any(|v| v.index == 3001));
    }

    #[test]
    fn test_set_active_entry_returns_points() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        let points = db.set_active_entry(1).expect("entry 1 should exist");
        assert!(points.iter().any(|v| v.index == 3001));
    }

    #[test]
    fn test_set_active_entry_out_of_range_returns_none() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 2, AiUid::Scheduling_FSCC_Schd);
        assert!(db.set_active_entry(0).is_none());
        assert!(db.set_active_entry(3).is_none());
    }

    #[test]
    fn test_set_active_entry_creates_blank_entry() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        let points = db
            .set_active_entry(2)
            .expect("blank entry 2 should be created");
        assert_eq!(db.current_entry(), 2);
        assert_eq!(db.len(), 2);
        assert!(
            points
                .iter()
                .any(|v| v.index == 3001 && v.value == TransmissionI32(0))
        );
        assert!(db.written_values(2).unwrap().is_empty());
    }

    #[test]
    fn test_set_active_entry_blank_then_update() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        db.set_active_entry(2);
        db.update_value(3002, TransmissionI32(9));
        assert_eq!(db.get(2).unwrap().priority.value, TransmissionI32(9));
        let written = db.written_values(2).unwrap();
        assert!(
            written
                .iter()
                .any(|v| v.index == 3002 && v.value == TransmissionI32(9))
        );
        // Entry 1 unaffected
        assert_eq!(db.get(1).unwrap().priority.value, TransmissionI32(0));
    }

    #[test]
    fn test_current_entry_defaults_to_1() {
        let profile = make_profile_with_schedule();
        let db = ScheduleDatabase::from_profile(&profile, 2, AiUid::Scheduling_FSCC_Schd);
        assert_eq!(db.current_entry(), 1);
    }

    #[test]
    fn test_max_schedules_capped() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        assert_eq!(db.max_schedules(), 4);
        assert!(db.set_active_entry(5).is_none());
    }

    #[test]
    fn test_all_pairs_includes_all_field_types() {
        let profile = make_profile_with_schedule();
        let db = ScheduleDatabase::from_profile(&profile, 1, AiUid::Scheduling_FSCC_Schd);
        let values = db.get(1).unwrap().all_values();
        // 11 header fields + 1 time_offset + 1 action_type + 1 action_index + 1 value = 15
        assert_eq!(values.len(), 15);
    }

    #[test]
    fn test_blank_entry_has_same_vector_structure() {
        let profile = make_profile_with_schedule();
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        db.set_active_entry(2);
        assert_eq!(
            db.get(2).unwrap().all_values().len(),
            db.get(1).unwrap().all_values().len()
        );
        assert!(
            db.get(2)
                .unwrap()
                .all_values()
                .iter()
                .all(|v| v.value == TransmissionI32(0))
        );
    }

    #[test]
    fn test_from_profile_with_empty_schedules_does_not_panic() {
        let profile = make_profile_with_schedules(vec![]);
        let db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        assert_eq!(db.len(), 0);
        assert!(db.is_empty());
        assert!(db.get(1).is_none());
        assert!(db.template_entry().is_none());
        assert!(db.current_entry_points().is_empty());
        assert_eq!(db.max_schedules(), 4);
    }

    #[test]
    fn test_empty_db_set_active_entry_returns_none() {
        let profile = make_profile_with_schedules(vec![]);
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        assert!(db.set_active_entry(1).is_none());
        assert!(db.set_active_entry(2).is_none());
    }

    #[test]
    fn test_empty_db_update_value_is_noop() {
        let profile = make_profile_with_schedules(vec![]);
        let mut db = ScheduleDatabase::from_profile(&profile, 4, AiUid::Scheduling_FSCC_Schd);
        db.update_value(3002, TransmissionI32(5));
        assert!(db.is_empty());
    }
}
