//! In-memory curve database built from PicsProfile curve data.

use std::collections::{HashMap, HashSet};

use crate::profile::validation::{Validated, ValidationErrors};
use crate::profile::values::TransmissionI32;
use crate::uids::ai_uid::AiUid;

use crate::profile::indexed_db::{AiValue, DatabaseEntry, IndexedEntryDatabase};
use crate::profile::{AiCurve, PicsProfile};

/// Current values and AI indices for a single curve.
#[derive(Debug, Clone)]
pub struct CurveEntry {
    /// AI index and value for the curve type.
    pub curve_type: AiValue,
    /// AI index and value for the number-of-points.
    pub number_of_points: AiValue,
    /// AI index and value for X units.
    pub x_units: AiValue,
    /// AI index and value for Y units.
    pub y_units: AiValue,
    /// AI index and value for X data points (parallel with y_values).
    pub x_values: Vec<AiValue>,
    /// AI index and value for Y data points (parallel with x_values).
    pub y_values: Vec<AiValue>,
    /// AI indices that have been explicitly written via a control operation.
    written: HashSet<u16>,
}

impl TryFrom<&AiCurve> for CurveEntry {
    type Error = ValidationErrors;

    fn try_from(curve: &AiCurve) -> Result<Self, ValidationErrors> {
        Ok(Self {
            curve_type: AiValue::try_from(&curve.curve_type)?,
            number_of_points: AiValue::try_from(&curve.number_of_points)?,
            x_units: AiValue::try_from(&curve.x_units)?,
            y_units: AiValue::try_from(&curve.y_units)?,
            x_values: curve
                .x_values
                .iter()
                .map(AiValue::try_from)
                .collect::<Result<_, _>>()?,
            y_values: curve
                .y_values
                .iter()
                .map(AiValue::try_from)
                .collect::<Result<_, _>>()?,
            written: HashSet::new(),
        })
    }
}

impl DatabaseEntry for CurveEntry {
    fn all_values(&self) -> Vec<AiValue> {
        let mut out = vec![
            self.curve_type,
            self.number_of_points,
            self.x_units,
            self.y_units,
        ];
        out.extend_from_slice(&self.x_values);
        out.extend_from_slice(&self.y_values);
        out
    }

    fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) -> bool {
        for field in [
            &mut self.curve_type,
            &mut self.number_of_points,
            &mut self.x_units,
            &mut self.y_units,
        ] {
            if field.index == ai_index {
                field.value = new_value;
                self.written.insert(ai_index);
                return true;
            }
        }
        for field in self.x_values.iter_mut().chain(self.y_values.iter_mut()) {
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
            curve_type: AiValue::new(self.curve_type.index, 0),
            number_of_points: AiValue::new(self.number_of_points.index, 0),
            x_units: AiValue::new(self.x_units.index, 0),
            y_units: AiValue::new(self.y_units.index, 0),
            x_values: self
                .x_values
                .iter()
                .map(|v| AiValue::new(v.index, 0))
                .collect(),
            y_values: self
                .y_values
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

/// In-memory database of all curve entries, keyed by 1-based curve number.
pub struct CurveDatabase {
    inner_db: IndexedEntryDatabase<CurveEntry>,
    /// UID of the curve-edit-selector readback point.
    selector_ai_uid: AiUid,
    /// Currently selected curve number (1-based). Defaults to 1.
    current_entry: u16,
}

impl CurveDatabase {
    /// Build a CurveDatabase from the curves in a PicsProfile.
    ///
    /// `max_entries` sets the upper bound for dynamic entry creation. Entries from the
    /// profile are loaded; additional entries up to `max_entries` are created on demand
    /// when `set_active_entry` is called for a number not yet in the database.
    ///
    /// If the profile contains no curve entries, the database is built empty: reads
    /// return `None`, `set_active_entry` returns `None` (no template to clone blanks
    /// from), and `update_value` is a no-op. Profiles without curve features are valid
    /// input.
    pub fn from_profile(
        profile: &Validated<PicsProfile>,
        max_entries: u16,
        selector_ai_uid: AiUid,
    ) -> Self {
        let mut db = IndexedEntryDatabase::<CurveEntry>::new(HashMap::new(), max_entries);

        for (idx, curve) in profile.ai.curves.iter().enumerate() {
            let curve_number = (idx as u16) + 1;
            let entry = CurveEntry::try_from(curve).unwrap_or_else(|e| {
                panic!(
                    "invalid curve data in profile at index {}: {:?}, error: {:?}",
                    idx, curve, e
                )
            });
            db.insert(curve_number, entry);
        }
        db.set_current_entry_points(1);

        Self {
            inner_db: db,
            selector_ai_uid,
            current_entry: 1,
        }
    }

    /// All AI values for the currently selected curve entry, kept in sync with any updates.
    pub fn current_entry_points(&self) -> Vec<AiValue> {
        self.inner_db.current_entry_points()
    }

    /// Look up a curve entry by 1-based curve number.
    pub fn get(&self, curve_number: u16) -> Option<&CurveEntry> {
        self.inner_db.get(curve_number)
    }

    /// Look up a curve entry mutably by 1-based curve number.
    pub fn get_mut(&mut self, curve_number: u16) -> Option<&mut CurveEntry> {
        self.inner_db.get_mut(curve_number)
    }

    /// Set the currently active curve number (1-based) and return all AI values for that curve.
    ///
    /// If `entry_number` is within range but has not been loaded from the profile, a blank
    /// entry is created automatically (all values 0, written tracker empty). Returns `None`
    /// only if `entry_number` is out of range or no template entry exists.
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

    /// Update the stored raw transmitted value for `ai_index` in the currently active curve entry.
    /// Does nothing if the AI index is not found in the active entry.
    pub fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) {
        self.inner_db
            .update_ai_value(self.current_entry, ai_index, new_value);
    }

    /// Currently active curve number (1-based).
    pub fn current_entry(&self) -> u16 {
        self.current_entry
    }

    /// Return curve 1, used as the template for creating blank entries with matching AI indices.
    pub fn template_entry(&self) -> Option<&CurveEntry> {
        self.inner_db.template_entry()
    }

    /// UID of the curve-edit-selector readback point.
    pub fn selector_ai_uid(&self) -> AiUid {
        self.selector_ai_uid
    }

    /// Maximum number of curves this database supports.
    pub fn max_curves(&self) -> u16 {
        self.inner_db.max_entries()
    }

    /// Number of curves in the database.
    pub fn len(&self) -> usize {
        self.inner_db.len()
    }

    /// Returns `true` if there are no curves.
    pub fn is_empty(&self) -> bool {
        self.inner_db.is_empty()
    }

    /// Return only the AI values that have been explicitly written for the given curve number.
    pub fn received_values(&self, curve_number: u16) -> Option<Vec<AiValue>> {
        self.inner_db
            .get(curve_number)
            .map(|curve_entry| curve_entry.received_values())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{
        AiPoint, CurveType,
        pics_profile::{
            AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs, EquipmentInfo,
            EquipmentPoints, EventClass, KeySheet, SectionInfo, SectionPoints,
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

    fn make_curve() -> AiCurve {
        AiCurve {
            curve_type: make_ai_point(329, CurveType::NotDefined as u8 as f64),
            number_of_points: make_ai_point(330, 0.0),
            x_units: make_ai_point(331, 0.0),
            y_units: make_ai_point(332, 0.0),
            x_values: vec![make_ai_point(333, 1.0), make_ai_point(335, 2.0)],
            y_values: vec![make_ai_point(334, 10.0), make_ai_point(336, 20.0)],
        }
    }

    fn make_curve_2() -> AiCurve {
        AiCurve {
            curve_type: make_ai_point(329, CurveType::NotDefined as u8 as f64),
            number_of_points: make_ai_point(330, 2.0),
            x_units: make_ai_point(331, 0.0),
            y_units: make_ai_point(332, 0.0),
            x_values: vec![make_ai_point(333, 3.0), make_ai_point(335, 4.0)],
            y_values: vec![make_ai_point(334, 30.0), make_ai_point(336, 40.0)],
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

    fn make_profile_with_curve() -> Validated<PicsProfile> {
        make_profile_with_curves(vec![make_curve()])
    }

    fn make_profile_with_two_curves() -> Validated<PicsProfile> {
        make_profile_with_curves(vec![make_curve(), make_curve_2()])
    }

    fn make_profile_with_curves(curves: Vec<AiCurve>) -> Validated<PicsProfile> {
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
                curves,
                schedules_bc: vec![],
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
        let profile = make_profile_with_curve();
        let db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert!(db.get(1).is_some());
        assert_eq!(db.get(1).unwrap().curve_type.index, 329);
    }

    #[test]
    fn test_from_profile_only_adds_profile_entries_initially() {
        let profile = make_profile_with_curve();
        let db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        // Only entry 1 loaded from profile; others are not yet created
        assert_eq!(db.len(), 1);
        assert!(db.get(1).is_some());
        assert!(db.get(2).is_none());
        assert!(db.get(4).is_none());
    }

    #[test]
    fn test_from_profile_with_two_curves() {
        let profile = make_profile_with_two_curves();
        let db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert_eq!(db.len(), 2);
        assert_eq!(db.get(1).unwrap().x_values[0].value, TransmissionI32(1));
        assert_eq!(db.get(2).unwrap().x_values[0].value, TransmissionI32(3));
        assert!(db.get(3).is_none());
    }

    #[test]
    fn test_update_value_x_value() {
        let profile = make_profile_with_two_curves();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        db.set_active_entry(2);
        db.update_value(333, TransmissionI32(42));
        assert_eq!(db.get(2).unwrap().x_values[0].value, TransmissionI32(42));
        // Curve 1 is unaffected
        assert_eq!(db.get(1).unwrap().x_values[0].value, TransmissionI32(1));
    }

    #[test]
    fn test_update_value_curve_type() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 2, AiUid::Curve_DGSMn_InCrv);
        db.set_active_entry(1);
        db.update_value(329, TransmissionI32(CurveType::VoltageWatt as i32));
        assert_eq!(
            db.get(1).unwrap().curve_type.value,
            TransmissionI32(CurveType::VoltageWatt as i32)
        );
    }

    #[test]
    fn test_update_value_unknown_index_is_noop() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 2, AiUid::Curve_DGSMn_InCrv);
        // Index 999 does not exist — should not panic
        db.update_value(999, TransmissionI32(99));
    }

    #[test]
    fn test_update_value_marks_written() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 2, AiUid::Curve_DGSMn_InCrv);
        db.update_value(329, TransmissionI32(CurveType::VoltageWatt as i32));
        let written = db.received_values(1).unwrap();
        assert!(written.iter().any(|v| v.index == 329));
        // x_values[0] (333) was not written
        assert!(!written.iter().any(|v| v.index == 333));
    }

    #[test]
    fn test_set_active_entry_returns_points() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        let points = db.set_active_entry(1).expect("entry 1 should exist");
        assert!(points.iter().any(|v| v.index == 329));
    }

    #[test]
    fn test_set_active_entry_out_of_range_returns_none() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert!(db.set_active_entry(0).is_none());
        assert!(db.set_active_entry(5).is_none());
    }

    #[test]
    fn test_set_active_entry_creates_blank_entry() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        // Entry 2 is within max but not in profile — should be created blank
        let points = db
            .set_active_entry(2)
            .expect("blank entry 2 should be created");
        assert_eq!(db.current_entry(), 2);
        assert_eq!(db.len(), 2);
        // Blank entry has same AI indices as the template entry but value 0
        assert!(
            points
                .iter()
                .any(|v| v.index == 329 && v.value == TransmissionI32(CurveType::NotDefined as i32))
        );
        // Nothing written yet
        assert!(db.received_values(2).unwrap().is_empty());
    }

    #[test]
    fn test_set_active_entry_blank_then_update() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        db.set_active_entry(2);
        db.update_value(329, TransmissionI32(CurveType::TemperatureMode as i32));
        assert_eq!(
            db.get(2).unwrap().curve_type.value,
            TransmissionI32(CurveType::TemperatureMode as i32)
        );
        let written = db.received_values(2).unwrap();
        assert!(written.iter().any(
            |v| v.index == 329 && v.value == TransmissionI32(CurveType::TemperatureMode as i32)
        ));
        // Entry 1 unaffected
        assert_eq!(
            db.get(1).unwrap().curve_type.value,
            TransmissionI32(CurveType::NotDefined as i32)
        );
    }

    #[test]
    fn test_current_entry_defaults_to_1() {
        let profile = make_profile_with_curve();
        let db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert_eq!(db.current_entry(), 1);
    }

    #[test]
    fn test_max_curves_capped() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert_eq!(db.max_curves(), 4);
        assert!(db.set_active_entry(5).is_none());
    }

    #[test]
    fn test_template_entry_returns_entry_1() {
        let profile = make_profile_with_curve();
        let db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        let template = db.template_entry().unwrap();
        assert_eq!(template.curve_type.index, 329);
    }

    #[test]
    fn test_all_pairs_contains_all_fields() {
        let profile = make_profile_with_curve();
        let db = CurveDatabase::from_profile(&profile, 1, AiUid::Curve_DGSMn_InCrv);
        let values = db.get(1).unwrap().all_values();
        // 4 header fields + 2 x-values + 2 y-values = 8
        assert_eq!(values.len(), 8);
        assert!(values.iter().any(|v| v.index == 329)); // curve_type
        assert!(values.iter().any(|v| v.index == 333)); // x_values[0]
    }

    #[test]
    fn test_blank_entry_has_same_vector_structure() {
        let profile = make_profile_with_curve();
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        db.set_active_entry(2);
        // Blank entry should have same number of fields as the template entry
        assert_eq!(
            db.get(2).unwrap().all_values().len(),
            db.get(1).unwrap().all_values().len()
        );
        // All values should be 0
        assert!(
            db.get(2)
                .unwrap()
                .all_values()
                .iter()
                .all(|v| v.value == TransmissionI32(0))
        );
    }

    #[test]
    fn test_from_profile_with_empty_curves_does_not_panic() {
        let profile = make_profile_with_curves(vec![]);
        let db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert_eq!(db.len(), 0);
        assert!(db.is_empty());
        assert!(db.get(1).is_none());
        assert!(db.template_entry().is_none());
        assert!(db.current_entry_points().is_empty());
        assert_eq!(db.max_curves(), 4);
    }

    #[test]
    fn test_empty_db_set_active_entry_returns_none() {
        let profile = make_profile_with_curves(vec![]);
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        assert!(db.set_active_entry(1).is_none());
        assert!(db.set_active_entry(2).is_none());
    }

    #[test]
    fn test_empty_db_update_value_is_noop() {
        let profile = make_profile_with_curves(vec![]);
        let mut db = CurveDatabase::from_profile(&profile, 4, AiUid::Curve_DGSMn_InCrv);
        db.update_value(329, TransmissionI32(5));
        assert!(db.is_empty());
    }
}
