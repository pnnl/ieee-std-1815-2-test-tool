pub use crate::profile::{
    ActionType, AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs, CtrPoint, CurveType,
    EventClass, KeySheet,
};
#[cfg(test)]
use crate::profile::{AiCurve, AoPoint, BiPoint, BoPoint, values::EngineeringF64};
use crate::profile::{
    AiPoint,
    validation::{Validate, Validated, ValidationError, ValidationErrors},
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// KeySheet
// ---------------------------------------------------------------------------

/// Start index for a single point type within a section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SectionInfo {
    pub start: u16,
}

/// Groups the per-point-type start indices for a single section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SectionPoints {
    pub bo: SectionInfo,
    pub bi: SectionInfo,
    pub ao: SectionInfo,
    pub ai: SectionInfo,
    pub ctr: SectionInfo,
}

/// Per-instance metadata for a single point type within an equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EquipmentInfo {
    pub count: u16,
    pub start: u16,
    pub points_per: u16,
}

/// Groups the per-point-type equipment metadata for a single equipment group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EquipmentPoints {
    pub bo: EquipmentInfo,
    pub bi: EquipmentInfo,
    pub ao: EquipmentInfo,
    pub ai: EquipmentInfo,
    pub ctr: EquipmentInfo,
}

// ---------------------------------------------------------------------------
// Top-level profile
// ---------------------------------------------------------------------------

/// Top-level deserialized representation of a PICS profile document. Unvalidated.
/// use PicsProfile::new to get Validated<PicsProfile>.
#[allow(clippy::manual_non_exhaustive)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PicsProfile {
    #[serde(rename = "Key")]
    pub key: KeySheet,
    #[serde(rename = "BO")]
    pub bo: BinaryOutputs,
    #[serde(rename = "BI")]
    pub bi: BinaryInputs,
    #[serde(rename = "AO")]
    pub ao: AnalogOutputs,
    #[serde(rename = "AI")]
    pub ai: AnalogInputs,
    #[serde(rename = "CTR")]
    pub ctr: Vec<CtrPoint>,
    #[serde(skip)]
    _private_to_force_new_constructor: (),
}

impl PicsProfile {
    pub fn new(
        key: KeySheet,
        bo: BinaryOutputs,
        bi: BinaryInputs,
        ao: AnalogOutputs,
        ai: AnalogInputs,
        ctr: Vec<CtrPoint>,
    ) -> Result<Validated<Self>, ValidationErrors> {
        let new_profile = Self {
            key,
            bo,
            bi,
            ao,
            ai,
            ctr,
            _private_to_force_new_constructor: (),
        };

        Validated::try_new(new_profile)
    }

    pub fn into_validated(self) -> Result<Validated<Self>, ValidationErrors> {
        Validated::try_new(self)
    }

    /// Update an AI point and validate the complete profile.
    ///
    /// The method uses `point_index` to find the point. If the update fails or makes
    /// the profile invalid, the method restores the initial point.
    ///
    /// Validation of the complete profile can decrease performance. To improve
    /// performance, validate only the point and the structures that contain it.
    pub fn set_ai_point(
        &mut self,
        point: &AiPoint,
        update: impl FnOnce(&mut AiPoint) -> Result<(), ValidationErrors>,
    ) -> Result<(), ValidationErrors> {
        let (initial_point, updated_point_index) = {
            let Some(target_point) = self
                .ai
                .all_ai_points_full_mut()
                .into_iter()
                .find(|candidate_point| candidate_point.point_index == point.point_index)
            else {
                return Err(ValidationErrors::from_error(ValidationError {
                    point: point.full_index(),
                    message: "Point was not found in the profile".to_string(),
                }));
            };

            let initial_point = target_point.clone();
            if let Err(errors) = update(target_point) {
                *target_point = initial_point;
                return Err(errors);
            }
            (initial_point, target_point.point_index)
        };

        if let Err(errors) = self.validate() {
            let target_point = self
                .ai
                .all_ai_points_full_mut()
                .into_iter()
                .find(|candidate_point| candidate_point.point_index == updated_point_index)
                .expect("updated AI point must be in the profile");
            *target_point = initial_point;
            return Err(errors);
        }

        Ok(())
    }

    /// For use with user-input data only. For internal construction, always use `PicsProfile::new()`.
    pub fn new_unsafe_for_collecting_errors(
        key: KeySheet,
        bo: BinaryOutputs,
        bi: BinaryInputs,
        ao: AnalogOutputs,
        ai: AnalogInputs,
        ctr: Vec<CtrPoint>,
    ) -> (Self, ValidationErrors) {
        let new_profile = Self {
            key,
            bo,
            bi,
            ao,
            ai,
            ctr,
            _private_to_force_new_constructor: (),
        };

        match new_profile.validate() {
            Ok(_) => (new_profile.clone(), ValidationErrors::new()),
            Err(errors) => (new_profile.clone(), errors),
        }
    }

    pub fn load_full_profile() -> Validated<PicsProfile> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/profiles/full.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        PicsProfile::into_validated(
            serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display())),
        )
        .expect("failed to validate full.json profile")
    }
}

impl Validate for PicsProfile {
    fn collect_validation_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        errors.extend(self.ai.collect_validation_errors());

        // Validate individual points
        for point in self.ao.all_ao_points() {
            errors.extend(point.collect_errors())
        }

        errors
    }
}

#[cfg(test)]
mod profile_index_tests {
    use super::*;
    use crate::profile::{profile_index::ProfileIndex, values::TransmissionI32};

    fn make_ai(index: u16, name: &str) -> AiPoint {
        AiPoint::new(
            index,
            name.to_string(),
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
        .expect("test AiPoint must be valid")
    }

    fn make_ao(index: u16, name: &str, assoc_ai: Option<&str>) -> AoPoint {
        AoPoint::new(
            index,
            name.to_string(),
            TransmissionI32(0),
            TransmissionI32(100),
            1.0,
            EngineeringF64(0.0),
            String::new(),
            String::new(),
            assoc_ai.map(String::from),
            String::new(),
            false,
            false,
        )
        .expect("test AoPoint must be valid")
    }

    fn make_bi(index: u16, name: &str) -> BiPoint {
        BiPoint {
            point_index: index,
            name: name.to_string(),
            event_class: EventClass::None,
            state_0: String::new(),
            state_1: String::new(),
            iec_61850_uid: String::new(),
            assoc_bo: None,
            purpose: String::new(),
            mandatory_1815: false,
            mandatory_1547: false,
        }
    }

    fn make_bo(index: u16, assoc_bi: Option<&str>) -> BoPoint {
        BoPoint {
            point_index: index,
            name: String::new(),
            state_0: String::new(),
            state_1: String::new(),
            iec_61850_uid: String::new(),
            assoc_bi: assoc_bi.map(String::from),
            purpose: String::new(),
            mandatory_1815: false,
            mandatory_1547: false,
        }
    }

    fn make_minimal_profile() -> Validated<PicsProfile> {
        use crate::profile::pics_profile::{
            AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs, EquipmentInfo,
            EquipmentPoints, KeySheet, SectionInfo, SectionPoints,
        };
        let empty_section = SectionPoints {
            bo: SectionInfo { start: 0 },
            bi: SectionInfo { start: 0 },
            ao: SectionInfo { start: 0 },
            ai: SectionInfo { start: 0 },
            ctr: SectionInfo { start: 0 },
        };
        let empty_equipment = EquipmentPoints {
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
        };
        PicsProfile::new(
            KeySheet {
                config: empty_section.clone(),
                functions: empty_section.clone(),
                curves: empty_section.clone(),
                system_meter: empty_section.clone(),
                extensions: empty_section.clone(),
                experimental: empty_section.clone(),
                vendor: empty_section.clone(),
                discovery: empty_section.clone(),
                schedules_bc: empty_section.clone(),
                schedules_bc_status: empty_section.clone(),
                schedules: empty_section.clone(),
                schedules_status: empty_section.clone(),
                max_points: 0,
                meter: empty_equipment.clone(),
                der: empty_equipment.clone(),
                inverter: empty_equipment.clone(),
                battery: empty_equipment,
            },
            BinaryOutputs {
                points: vec![make_bo(0, Some("BI11")), make_bo(1, None)],
            },
            BinaryInputs {
                points: vec![make_bi(11, "BI11"), make_bi(12, "BI12")],
                meters: vec![],
                ders: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            AnalogOutputs {
                points: vec![
                    make_ao(10, "AO10", Some("AI20")),
                    make_ao(11, "AO11", None),
                    make_ao(244, "AO244", Some("AI328")), // curve selector
                ],
                meters: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            AnalogInputs {
                points: vec![make_ai(20, "AI20"), make_ai(328, "AI328")],
                curves: vec![AiCurve {
                    curve_type: make_ai(329, "AI329"),
                    number_of_points: make_ai(330, "AI330"),
                    x_units: make_ai(331, "AI331"),
                    y_units: make_ai(332, "AI332"),
                    x_values: vec![],
                    y_values: vec![],
                }],
                schedules_bc: vec![],
                schedules: vec![],
                meters: vec![],
                ders: vec![],
                inverters: vec![],
                batteries: vec![],
            },
            vec![],
        )
        .expect("test profile must be valid")
    }

    #[test]
    fn test_ao_to_ai_base_point() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(idx.ao_to_ai.get(&10), Some(&20));
    }

    #[test]
    fn test_ao_without_assoc_ai_not_in_map() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert!(!idx.ao_to_ai.contains_key(&11));
    }

    #[test]
    fn test_ao_to_ai_curve_selector_resolved() {
        // AO 244 → AI 328 which is in AI.curves, not AI.points
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(
            idx.ao_to_ai.get(&244),
            Some(&328),
            "curve selector AI must be resolved"
        );
    }

    #[test]
    fn test_ai_to_ao_reverse_mapping() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(idx.ai_to_ao.get(&20), Some(&10));
        assert_eq!(idx.ai_to_ao.get(&328), Some(&244));
    }

    #[test]
    fn test_set_ai_point_rejects_an_invalid_profile() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let curve_type_point = profile.ai.curves[0].curve_type.clone();

        let errors = profile
            .set_ai_point(&curve_type_point, |point| {
                point.set_value(EngineeringF64(CurveType::VoltVAr as u8 as f64))
            })
            .expect_err("the curve type is not compatible with the initial units");

        assert!(!errors.is_empty());
        assert_eq!(
            profile.ai.curves[0].curve_type.value(),
            curve_type_point.value(),
            "the method must restore the point after an invalid update"
        );
        profile
            .validate()
            .expect("the profile must be valid after the method restores the point");
    }

    #[test]
    fn test_set_ai_point_accepts_a_valid_update() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let point = profile.ai.points[0].clone();

        profile
            .set_ai_point(&point, |point| point.set_value(EngineeringF64(50.0)))
            .expect("the profile must accept a value in the permitted range");

        assert_eq!(profile.ai.points[0].value(), EngineeringF64(50.0));
    }

    #[test]
    fn test_set_ai_point_returns_an_error_when_point_is_missing() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let missing_point = make_ai(999, "AI999");

        let errors = profile
            .set_ai_point(&missing_point, |_| Ok(()))
            .expect_err("a missing point must cause a validation error");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors.errors[0].point, "AI999");
    }

    #[test]
    fn test_bo_to_bi_and_reverse() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert_eq!(idx.bo_to_bi.get(&0), Some(&11));
        assert!(!idx.bo_to_bi.contains_key(&1));
        assert_eq!(idx.bi_to_bo.get(&11), Some(&0));
    }

    #[test]
    fn test_ai_points_includes_curve_point() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        // AI 328 is in curves, not base points — must still be in ai_points
        assert!(idx.ai_points.contains_key(&328));
        assert_eq!(idx.ai_points[&328].name, "AI328");
    }

    #[test]
    fn test_ao_points_and_bo_points_populated() {
        let profile = make_minimal_profile();
        let idx = ProfileIndex::from_profile(&profile);
        assert!(idx.ao_points.contains_key(&10));
        assert!(idx.bo_points.contains_key(&0));
        assert!(idx.bo_points.contains_key(&1));
    }

    #[test]
    fn test_curve_type_roundtrip() {
        for val in 0..=16 {
            let curve_type = CurveType::try_from(val).unwrap();
            assert_eq!(curve_type as u8, val);
        }
        assert!(CurveType::try_from(17).is_err());
    }

    #[test]
    fn test_action_type_roundtrip() {
        for val in 0..=3 {
            let action_type = ActionType::try_from(val).unwrap();
            assert_eq!(action_type as u8, val);
        }
        assert!(ActionType::try_from(4).is_err());
    }
}
