pub use crate::profile::{
    ActionType, AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs, CtrPoint, CurveType,
    EventClass, KeySheet,
};
#[cfg(test)]
use crate::profile::{AiCurve, AoPoint, BiPoint, BoPoint, values::EngineeringF64};
use crate::profile::{
    AiPoint, MultiplexerNumber,
    validation::{Validate, Validated, ValidationError, ValidationErrors},
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// KeySheet
// ---------------------------------------------------------------------------

/// Start point index for a single point type within a section.
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

    /// Update a non-multiplexed AI point and validate the complete profile.
    ///
    /// The method uses the point index (e.g. AI333) to find the point. If the update fails or makes
    /// the profile invalid, the method restores the initial point. Multiplexed points
    /// must use `set_multiplexed_ai_point`, even if only one entry exists.
    ///
    /// Validation of the complete profile can decrease performance. To improve
    /// performance, validate only the point and the structures that contain it.
    pub fn set_ai_point(
        &mut self,
        point: &AiPoint,
        update: impl FnOnce(&mut AiPoint) -> Result<(), ValidationErrors>,
    ) -> Result<(), ValidationErrors> {
        if self.ai.is_point_multiplexed(point.point_index) {
            return Err(ValidationErrors::from_error(ValidationError {
                point: point.full_index(),
                message: "Point is multiplexed; use set_multiplexed_ai_point with a family and entry number".to_string(),
            }));
        }
        self.update_ai_point(None, point, update)
    }

    /// Update an AI point in the specified family and 1-based entry.
    ///
    /// Validates the complete profile and restores the original point if the callback
    /// or validation fails. Missing entries, missing points, and duplicate matches
    /// return an error without invoking the callback.
    pub fn set_multiplexed_ai_point(
        &mut self,
        multiplexer: MultiplexerNumber,
        point: &AiPoint,
        update: impl FnOnce(&mut AiPoint) -> Result<(), ValidationErrors>,
    ) -> Result<(), ValidationErrors> {
        self.update_ai_point(Some(multiplexer), point, update)
    }

    fn update_ai_point(
        &mut self,
        multiplexer: Option<MultiplexerNumber>,
        point: &AiPoint,
        update: impl FnOnce(&mut AiPoint) -> Result<(), ValidationErrors>,
    ) -> Result<(), ValidationErrors> {
        let scope = match multiplexer {
            Some(number) => format!("{number:?}"),
            None => "non-multiplexed points".to_string(),
        };
        let make_error = |message: String| {
            ValidationErrors::from_error(ValidationError {
                point: point.full_index(),
                message,
            })
        };
        let (initial_point, position) = {
            let mut points = self
                .ai
                .ai_points_for_update_mut(multiplexer)
                .ok_or_else(|| {
                    make_error(format!(
                        "Invalid multiplexer entry {scope}; entry numbers are 1-based and must exist"
                    ))
                })?;
            let mut target_indices = points
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.point_index == point.point_index)
                .map(|(position, _)| position);
            let position = target_indices
                .next()
                .ok_or_else(|| make_error(format!("Point was not found in {scope}")))?;
            if target_indices.next().is_some() {
                return Err(make_error(format!(
                    "Multiple points have the same point index in {scope}"
                )));
            }

            let target_point = &mut *points[position];
            let initial_point = target_point.clone();
            if let Err(errors) = update(target_point) {
                *target_point = initial_point;
                return Err(errors);
            }
            (initial_point, position)
        };

        if let Err(errors) = self.validate() {
            // The callback can change point_index, so restore by structural position.
            let mut points = self
                .ai
                .ai_points_for_update_mut(multiplexer)
                .expect("updated AI entry must still be in the profile");
            *points[position] = initial_point;
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

    fn make_ai(point_index: u16, name: &str) -> AiPoint {
        AiPoint::new(
            point_index,
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

    fn make_ao(point_index: u16, name: &str, assoc_ai: Option<&str>) -> AoPoint {
        AoPoint::new(
            point_index,
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

    fn make_bi(point_index: u16, name: &str) -> BiPoint {
        BiPoint {
            point_index,
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

    fn make_bo(point_index: u16, assoc_bi: Option<&str>) -> BoPoint {
        BoPoint {
            point_index,
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
    fn test_set_multiplexed_ai_point_rejects_an_invalid_profile() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let original = profile.clone();
        let curve_type_point = profile.ai.curves[0].curve_type.clone();

        let errors = profile
            .set_multiplexed_ai_point(MultiplexerNumber::Curve(1), &curve_type_point, |point| {
                // A changed point index must not redirect rollback to the base point at AI20.
                point.point_index = 20;
                point.set_value(EngineeringF64(CurveType::VoltVAr as u8 as f64))
            })
            .expect_err("the curve type is not compatible with the initial units");

        assert!(!errors.is_empty());
        assert_eq!(profile, original);
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
    fn test_set_ai_point_rejects_multiplexed_points_before_callback() {
        for entry_count in [1, 4] {
            let mut profile = PicsProfile::load_full_profile().into_unvalidated();
            profile.ai.curves.truncate(entry_count);
            profile.ai.schedules_bc.truncate(entry_count);
            profile.ai.schedules.truncate(entry_count);
            let original = profile.clone();
            let points = [
                profile.ai.curves[0].x_values[0].clone(),
                profile.ai.schedules_bc[0].identity.clone(),
                profile.ai.schedules[0].identity.clone(),
            ];
            for point in points {
                let errors = profile
                    .set_ai_point(&point, |_| {
                        panic!("multiplexed access must be rejected before the callback")
                    })
                    .unwrap_err();
                assert!(
                    errors.errors[0]
                        .message
                        .contains("set_multiplexed_ai_point")
                );
                assert_eq!(profile, original);
            }
        }
    }

    #[test]
    fn test_set_ai_point_accepts_equipment_and_selector_points() {
        let mut profile = PicsProfile::load_full_profile().into_unvalidated();
        let mut points = vec![profile.ai.meters[0].active_power.clone()];
        points.extend(
            profile
                .ai
                .points
                .iter()
                .filter(|p| [328, 2000, 3000].contains(&p.point_index))
                .cloned(),
        );
        assert_eq!(points.len(), 4);
        for point in points {
            profile
                .set_ai_point(&point, |p| p.set_value(EngineeringF64(2.0)))
                .unwrap();
            let updated = profile
                .ai
                .base_ai_points()
                .into_iter()
                .find(|p| p.point_index == point.point_index)
                .unwrap();
            assert_eq!(updated.value(), EngineeringF64(2.0));
        }
        profile.validate().unwrap();
    }

    #[test]
    fn test_set_multiplexed_ai_point_updates_only_selected_entry() {
        let original = PicsProfile::load_full_profile().into_unvalidated();
        for (multiplexer, point) in [
            (
                MultiplexerNumber::Curve(2),
                original.ai.curves[1].x_values[0].clone(),
            ),
            (
                MultiplexerNumber::ScheduleBC(2),
                original.ai.schedules_bc[1].identity.clone(),
            ),
            (
                MultiplexerNumber::Schedule(2),
                original.ai.schedules[1].identity.clone(),
            ),
        ] {
            let mut profile = original.clone();
            let mut expected = original.clone();
            let new_value = point.value() + EngineeringF64(1.0);
            let target = match multiplexer {
                MultiplexerNumber::Curve(_) => &mut expected.ai.curves[1].x_values[0],
                MultiplexerNumber::ScheduleBC(_) => &mut expected.ai.schedules_bc[1].identity,
                MultiplexerNumber::Schedule(_) => &mut expected.ai.schedules[1].identity,
            };
            target.set_value(new_value).unwrap();
            profile
                .set_multiplexed_ai_point(multiplexer, &point, |p| p.set_value(new_value))
                .unwrap();
            assert_eq!(profile, expected);
            profile.validate().unwrap();
        }
    }

    #[test]
    fn test_set_multiplexed_ai_point_rejects_invalid_targets_before_callback() {
        let mut profile = PicsProfile::load_full_profile().into_unvalidated();
        let original = profile.clone();
        let curve_point = profile.ai.curves[0].x_values[0].clone();
        let bc_point = profile.ai.schedules_bc[0].identity.clone();
        let schedule_point = profile.ai.schedules[0].identity.clone();
        for (family, point, wrong_point) in [
            (
                MultiplexerNumber::Curve as fn(u16) -> MultiplexerNumber,
                &curve_point,
                &bc_point,
            ),
            (MultiplexerNumber::ScheduleBC, &bc_point, &schedule_point),
            (MultiplexerNumber::Schedule, &schedule_point, &curve_point),
        ] {
            for number in [0, 5, u16::MAX] {
                assert!(
                    profile
                        .set_multiplexed_ai_point(family(number), point, |_| {
                            panic!("invalid entries must be rejected before the callback")
                        })
                        .is_err()
                );
            }
            for missing in [
                wrong_point,
                &make_ai(999, "missing"),
                &profile.ai.points[0].clone(),
            ] {
                assert!(profile.set_multiplexed_ai_point(family(1), missing, |_| {
                    panic!("points outside the selected entry must be rejected before the callback")
                }).is_err());
            }
        }
        assert_eq!(profile, original);
    }

    #[test]
    fn test_point_setters_reject_duplicate_matches_before_callback() {
        let mut profile = make_minimal_profile().into_unvalidated();
        let base_point = profile.ai.points[0].clone();
        profile.ai.points.push(base_point.clone());
        let curve_point = profile.ai.curves[0].curve_type.clone();
        profile.ai.curves[0].x_values.push(curve_point.clone());
        let original = profile.clone();
        let errors = profile
            .set_ai_point(&base_point, |_| {
                panic!("duplicate matches must be rejected before the callback")
            })
            .unwrap_err();
        assert!(errors.errors[0].message.contains("Multiple points"));
        let errors = profile
            .set_multiplexed_ai_point(MultiplexerNumber::Curve(1), &curve_point, |_| {
                panic!("duplicate matches must be rejected before the callback")
            })
            .unwrap_err();
        assert!(errors.errors[0].message.contains("Multiple points"));
        assert_eq!(profile, original);
    }

    #[test]
    fn test_point_setters_restore_exact_target_after_callback_or_validation_failure() {
        let original = PicsProfile::load_full_profile().into_unvalidated();
        for (multiplexer, point) in [
            (None, original.ai.points[0].clone()),
            (
                Some(MultiplexerNumber::Curve(2)),
                original.ai.curves[1].x_values[0].clone(),
            ),
            (
                Some(MultiplexerNumber::ScheduleBC(2)),
                original.ai.schedules_bc[1].identity.clone(),
            ),
            (
                Some(MultiplexerNumber::Schedule(2)),
                original.ai.schedules[1].identity.clone(),
            ),
        ] {
            for callback_fails in [false, true] {
                let mut profile = original.clone();
                let callback_error = ValidationErrors::from_error(ValidationError {
                    point: point.full_index(),
                    message: "callback failed".to_string(),
                });
                let update = |target: &mut AiPoint| {
                    target.point_index = 0;
                    target.name.push_str(" changed");
                    target.set_value_unsafe(target.eng_maximum() + EngineeringF64(1.0));
                    if callback_fails {
                        Err(callback_error.clone())
                    } else {
                        Ok(())
                    }
                };
                let errors = match multiplexer {
                    Some(number) => profile.set_multiplexed_ai_point(number, &point, update),
                    None => profile.set_ai_point(&point, update),
                }
                .unwrap_err();
                if callback_fails {
                    assert_eq!(errors, callback_error);
                }
                assert_eq!(profile, original);
                profile.validate().unwrap();
            }
        }
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
