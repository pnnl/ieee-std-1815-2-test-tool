//! In-memory backward-compatible schedule database built from PicsProfile schedule BC data.

use std::collections::{HashMap, HashSet};

use crate::profile::validation::{Validated, ValidationErrors};
use crate::profile::values::TransmissionI32;
use crate::uids::ai_uid::AiUid;

use super::indexed_db::{AiValue, DatabaseEntry, IndexedEntryDatabase};
use super::profile::{AiScheduleBC, PicsProfile};

/// Current values and AI indices for a single backward-compatible schedule.
#[derive(Debug, Clone)]
pub struct ScheduleBCEntry {
    /// AI index and value for identity.
    pub identity: AiValue,
    /// AI index and value for priority.
    pub priority: AiValue,
    /// AI index and value for schedule type.
    pub schedule_type: AiValue,
    /// AI index and value for start date.
    pub start_date: AiValue,
    /// AI index and value for start time.
    pub start_time: AiValue,
    /// AI index and value for repeat interval.
    pub repeat_interval: AiValue,
    /// AI index and value for repeat interval units.
    pub repeat_interval_units: AiValue,
    /// AI index and value for validation status.
    pub validation_status: AiValue,
    /// AI index and value for status.
    pub status: AiValue,
    /// AI index and value for number-of-points.
    pub number_of_points: AiValue,
    /// AI index and value for time offset slots.
    pub time_offsets: Vec<AiValue>,
    /// AI index and value for value slots.
    pub values: Vec<AiValue>,
    /// AI indices that have been explicitly written via a control operation.
    written: HashSet<u16>,
}

impl TryFrom<&AiScheduleBC> for ScheduleBCEntry {
    type Error = ValidationErrors;

    fn try_from(schedule: &AiScheduleBC) -> Result<Self, Self::Error> {
        Ok(Self {
            identity: AiValue::try_from(&schedule.identity)?,
            priority: AiValue::try_from(&schedule.priority)?,
            schedule_type: AiValue::try_from(&schedule.schedule_type)?,
            start_date: AiValue::try_from(&schedule.start_date)?,
            start_time: AiValue::try_from(&schedule.start_time)?,
            repeat_interval: AiValue::try_from(&schedule.repeat_interval)?,
            repeat_interval_units: AiValue::try_from(&schedule.repeat_interval_units)?,
            validation_status: AiValue::try_from(&schedule.validation_status)?,
            status: AiValue::try_from(&schedule.status)?,
            number_of_points: AiValue::try_from(&schedule.number_of_points)?,
            time_offsets: schedule
                .time_offsets
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

impl DatabaseEntry for ScheduleBCEntry {
    fn all_values(&self) -> Vec<AiValue> {
        let mut out = vec![
            self.identity,
            self.priority,
            self.schedule_type,
            self.start_date,
            self.start_time,
            self.repeat_interval,
            self.repeat_interval_units,
            self.validation_status,
            self.status,
            self.number_of_points,
        ];
        out.extend_from_slice(&self.time_offsets);
        out.extend_from_slice(&self.values);
        out
    }

    fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) -> bool {
        for field in [
            &mut self.identity,
            &mut self.priority,
            &mut self.schedule_type,
            &mut self.start_date,
            &mut self.start_time,
            &mut self.repeat_interval,
            &mut self.repeat_interval_units,
            &mut self.validation_status,
            &mut self.status,
            &mut self.number_of_points,
        ] {
            if field.index == ai_index {
                field.value = new_value;
                self.written.insert(ai_index);
                return true;
            }
        }
        for field in self.time_offsets.iter_mut().chain(self.values.iter_mut()) {
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
            schedule_type: AiValue::new(self.schedule_type.index, 0),
            start_date: AiValue::new(self.start_date.index, 0),
            start_time: AiValue::new(self.start_time.index, 0),
            repeat_interval: AiValue::new(self.repeat_interval.index, 0),
            repeat_interval_units: AiValue::new(self.repeat_interval_units.index, 0),
            validation_status: AiValue::new(self.validation_status.index, 0),
            status: AiValue::new(self.status.index, 0),
            number_of_points: AiValue::new(self.number_of_points.index, 0),
            time_offsets: self
                .time_offsets
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

/// In-memory database of all backward-compatible schedule entries, keyed by 1-based schedule number.
pub struct ScheduleBCDatabase {
    inner_db: IndexedEntryDatabase<ScheduleBCEntry>,
    /// UID of the schedule-to-edit-selector readback point.
    selector_ai_uid: AiUid,
    /// Currently selected schedule number (1-based). Defaults to 1.
    current_entry: u16,
}

impl ScheduleBCDatabase {
    /// Build a ScheduleBCDatabase from the backward-compatible schedules in a PicsProfile.
    ///
    /// `max_schedules` sets the upper bound for selector validation; entries beyond those
    /// in the profile are created blank on demand when `set_active_entry` is called.
    ///
    /// If the profile contains no schedules BC, the database is built empty: reads return
    /// `None`, `set_active_entry` returns `None` (no template to clone blanks from), and
    /// `update_value` is a no-op. Profiles without schedule BC features are valid input.
    pub fn from_profile(
        profile: &Validated<PicsProfile>,
        max_schedules: u16,
        selector_ai_uid: AiUid,
    ) -> Self {
        let mut db = IndexedEntryDatabase::<ScheduleBCEntry>::new(HashMap::new(), max_schedules);

        for (idx, schedule) in profile.ai.schedules_bc.iter().enumerate() {
            let schedule_number = (idx as u16) + 1;
            let entry = ScheduleBCEntry::try_from(schedule)
                .unwrap_or_else(|_| panic!("invalid schedule BC data in profile at index {}", idx));
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
    pub fn get(&self, schedule_number: u16) -> Option<&ScheduleBCEntry> {
        self.inner_db.get(schedule_number)
    }

    /// Look up a schedule entry mutably by 1-based schedule number.
    pub fn get_mut(&mut self, schedule_number: u16) -> Option<&mut ScheduleBCEntry> {
        self.inner_db.get_mut(schedule_number)
    }

    /// Set the currently active schedule number (1-based) and return all AI values for that entry.
    ///
    /// If the entry does not exist but is within `max_schedules`, a blank entry is created.
    /// Returns `None` if `entry_number` is out of range or no template entry exists.
    /// The returned values should be pushed into the DNP3 database.
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
    pub fn template_entry(&self) -> Option<&ScheduleBCEntry> {
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

    /// Number of backward-compatible schedules in the database.
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
    use crate::profile::{
        profile::{
            AiPoint, AiScheduleBC, AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs,
            EquipmentInfo, EquipmentPoints, EventClass, KeySheet, SectionInfo, SectionPoints,
        },
        validation::Validated,
        values::EngineeringF64,
    };

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

    fn make_schedule_bc() -> AiScheduleBC {
        AiScheduleBC {
            identity: make_ai_point(2002, 1.0),
            priority: make_ai_point(2003, 0.0),
            schedule_type: make_ai_point(2004, 0.0),
            start_date: make_ai_point(2005, 0.0),
            start_time: make_ai_point(2006, 0.0),
            repeat_interval: make_ai_point(2007, 0.0),
            repeat_interval_units: make_ai_point(2008, 0.0),
            validation_status: make_ai_point(2009, 0.0),
            status: make_ai_point(2010, 0.0),
            number_of_points: make_ai_point(2011, 0.0),
            time_offsets: vec![make_ai_point(2012, 0.0), make_ai_point(2014, 0.0)],
            values: vec![make_ai_point(2013, 0.0), make_ai_point(2015, 0.0)],
        }
    }

    fn make_schedule_bc_2() -> AiScheduleBC {
        AiScheduleBC {
            identity: make_ai_point(2002, 2.0),
            priority: make_ai_point(2003, 1.0),
            schedule_type: make_ai_point(2004, 3.0),
            start_date: make_ai_point(2005, 0.0),
            start_time: make_ai_point(2006, 0.0),
            repeat_interval: make_ai_point(2007, 0.0),
            repeat_interval_units: make_ai_point(2008, 0.0),
            validation_status: make_ai_point(2009, 0.0),
            status: make_ai_point(2010, 0.0),
            number_of_points: make_ai_point(2011, 1.0),
            time_offsets: vec![make_ai_point(2012, 60.0), make_ai_point(2014, 0.0)],
            values: vec![make_ai_point(2013, 5.0), make_ai_point(2015, 0.0)],
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

    fn make_profile_with_schedule_bc() -> Validated<PicsProfile> {
        make_profile_with_schedules_bc(vec![make_schedule_bc()])
    }

    fn make_profile_with_two_schedules_bc() -> Validated<PicsProfile> {
        make_profile_with_schedules_bc(vec![make_schedule_bc(), make_schedule_bc_2()])
    }

    fn make_profile_with_schedules_bc(schedules_bc: Vec<AiScheduleBC>) -> Validated<PicsProfile> {
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
                schedules_bc,
                schedules: vec![],
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
        let profile = make_profile_with_schedule_bc();
        let db = ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert!(db.get(1).is_some());
        assert_eq!(db.get(1).unwrap().identity.index, 2002);
    }

    #[test]
    fn test_from_profile_only_adds_profile_entries_initially() {
        let profile = make_profile_with_schedule_bc();
        let db = ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert_eq!(db.len(), 1);
        assert_eq!(db.get(1).unwrap().identity.value, TransmissionI32(1));
        assert!(db.get(2).is_none());
    }

    #[test]
    fn test_from_profile_with_two_schedules() {
        let profile = make_profile_with_two_schedules_bc();
        let db = ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert_eq!(db.len(), 2);
        assert_eq!(db.get(1).unwrap().time_offsets[0].value, TransmissionI32(0));
        assert_eq!(
            db.get(2).unwrap().time_offsets[0].value,
            TransmissionI32(60)
        );
        assert!(db.get(3).is_none());
    }

    #[test]
    fn test_update_value_time_offset() {
        let profile = make_profile_with_two_schedules_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        db.set_active_entry(2);
        db.update_value(2012, TransmissionI32(120));
        assert_eq!(
            db.get(2).unwrap().time_offsets[0].value,
            TransmissionI32(120)
        );
        assert_eq!(db.get(1).unwrap().time_offsets[0].value, TransmissionI32(0));
    }

    #[test]
    fn test_update_value_header() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 2, AiUid::BC_Scheduling_BC_FSCC_Schd);
        db.set_active_entry(1);
        db.update_value(2003, TransmissionI32(5));
        assert_eq!(db.get(1).unwrap().priority.value, TransmissionI32(5));
    }

    #[test]
    fn test_update_value_unknown_index_is_noop() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 2, AiUid::BC_Scheduling_BC_FSCC_Schd);
        db.update_value(9999, TransmissionI32(99));
    }

    #[test]
    fn test_update_value_marks_written() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 2, AiUid::BC_Scheduling_BC_FSCC_Schd);
        db.update_value(2003, TransmissionI32(5));
        let written = db.written_values(1).unwrap();
        assert!(written.iter().any(|v| v.index == 2003));
        assert!(!written.iter().any(|v| v.index == 2002));
    }

    #[test]
    fn test_set_active_entry_returns_points() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        let points = db.set_active_entry(1).expect("entry 1 should exist");
        assert!(points.iter().any(|v| v.index == 2002));
    }

    #[test]
    fn test_set_active_entry_out_of_range_returns_none() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 2, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert!(db.set_active_entry(0).is_none());
        assert!(db.set_active_entry(3).is_none());
    }

    #[test]
    fn test_set_active_entry_creates_blank_entry() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        let points = db
            .set_active_entry(2)
            .expect("blank entry 2 should be created");
        assert_eq!(db.current_entry(), 2);
        assert_eq!(db.len(), 2);
        assert!(
            points
                .iter()
                .any(|v| v.index == 2002 && v.value == TransmissionI32(0))
        );
        assert!(db.written_values(2).unwrap().is_empty());
    }

    #[test]
    fn test_set_active_entry_blank_then_update() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        db.set_active_entry(2);
        db.update_value(2003, TransmissionI32(8));
        assert_eq!(db.get(2).unwrap().priority.value, TransmissionI32(8));
        let written = db.written_values(2).unwrap();
        assert!(
            written
                .iter()
                .any(|v| v.index == 2003 && v.value == TransmissionI32(8))
        );
        assert_eq!(db.get(1).unwrap().priority.value, TransmissionI32(0));
    }

    #[test]
    fn test_current_entry_defaults_to_1() {
        let profile = make_profile_with_schedule_bc();
        let db = ScheduleBCDatabase::from_profile(&profile, 2, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert_eq!(db.current_entry(), 1);
    }

    #[test]
    fn test_max_schedules_capped() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert_eq!(db.max_schedules(), 4);
        assert!(db.set_active_entry(5).is_none());
    }

    #[test]
    fn test_blank_entry_has_same_vector_structure() {
        let profile = make_profile_with_schedule_bc();
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
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
    fn test_from_profile_with_empty_schedules_bc_does_not_panic() {
        let profile = make_profile_with_schedules_bc(vec![]);
        let db = ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        assert_eq!(db.len(), 0);
        assert!(db.is_empty());
        assert!(db.get(1).is_none());
        assert!(db.template_entry().is_none());
        assert!(db.current_entry_points().is_empty());
        assert_eq!(db.max_schedules(), 4);
    }

    #[test]
    fn test_empty_db_set_active_entry_returns_none() {
        let profile = make_profile_with_schedules_bc(vec![]);
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        // No template, so even in-range entries cannot be created.
        assert!(db.set_active_entry(1).is_none());
        assert!(db.set_active_entry(2).is_none());
    }

    #[test]
    fn test_empty_db_update_value_is_noop() {
        let profile = make_profile_with_schedules_bc(vec![]);
        let mut db =
            ScheduleBCDatabase::from_profile(&profile, 4, AiUid::BC_Scheduling_BC_FSCC_Schd);
        // Must not panic.
        db.update_value(2003, TransmissionI32(5));
        assert!(db.is_empty());
    }
}
