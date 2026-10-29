use std::collections::HashSet;

use common::profile::validation::{Validated, ValidationError, ValidationErrors};
use common::profile::values::{EngineeringF64, TransmissionI32};
use common::profile::{
    AiPoint, AnalogInputs, AnalogOutputs, BinaryInputs, BinaryOutputs, MultiplexerNumber,
    PicsProfile,
};
use common::uids::ai_uid::AiUid;
use common::uids::ao_uid::AoUid;
use rand::Rng;
use tracing::{debug, info};

use crate::scenarios::ScenarioId;

/// Returns true if the AI point is a multiplex edit selector readback point.
/// Modifying these points during scenario profile updates invalidates database multiplexing,
/// so they are bypassed when randomizing or expanding point values.
pub fn is_multiplex_selector_ai(point_index: u16) -> bool {
    point_index == AiUid::Curve_DGSMn_InCrv as u16
        || point_index == AiUid::BC_Scheduling_BC_FSCC_Schd as u16
        || point_index == AiUid::Scheduling_FSCC_Schd as u16
}

pub fn is_multiplex_selector_ao(point_index: u16) -> bool {
    point_index == AoUid::Curve_DGSMn_InCrv as u16
        || point_index == AoUid::BC_Scheduling_BC_FSCC_Schd as u16
        || point_index == AoUid::Scheduling_FSCC_Schd as u16
}

/// Transform a `PicsProfile` for the given scenario.
///
/// Each scenario requires a different profile to send to the outstation:
/// - `Configuration` / `Functional` / `FunctionalRepeat`: return the profile unchanged.
/// - `ModifyPoints`: randomize every AI point value within its `[minimum, maximum]` range.
/// - `Unsupported`: return a profile containing only the points present in `full_profile` but
///   absent from the submitted profile.
/// - `LowerBounds`: set every AI point value to `minimum - 1`.
/// - `UpperBounds`: set every AI point value to `maximum + 1`.
/// - `MaxCurves`: append a clone of the last curve in the profile.
/// - `MaxSchedules`: append a clone of each last schedule (BC and IEEE).
///
/// `full_profile` is the reference profile containing all possible points (loaded from
/// `profiles/full.json`). It is only used by the `Unsupported` scenario.
pub fn update_profile(
    scenario_id: &ScenarioId,
    profile: Validated<PicsProfile>,
    full_profile: &Validated<PicsProfile>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    // Unvalidated since some scenarios produce invalid profiles.
    match scenario_id {
        ScenarioId::Configuration | ScenarioId::Functional | ScenarioId::FunctionalRepeat => {
            Ok(profile)
        }

        ScenarioId::ModifyPoints => modify_points(profile),
        ScenarioId::Unsupported => generate_unsupported_profile(profile, full_profile),
        ScenarioId::BelowLowerBounds => set_ai_out_of_bounds(profile, BoundsDirection::Lower),
        ScenarioId::AboveUpperBounds => set_ai_out_of_bounds(profile, BoundsDirection::Upper),
        ScenarioId::MaxCurves => append_clone_of_last_curve(profile),
        ScenarioId::MaxSchedules => append_clones_of_last_schedules(profile),
    }
}

/// If any selected scenario requires boundary headroom (e.g. BelowLowerBounds, AboveUpperBounds),
/// adjusts AO and AI points across all groups in the base profile that are pinned to `i32::MIN` or `i32::MAX`
/// so that extended out-of-bounds values can be represented and tested within 32-bit DNP3 transmission limits.
/// If no boundary scenarios are selected, returns the profile unmodified.
pub fn prepare_base_profile_for_scenarios(
    profile: Validated<PicsProfile>,
    scenario_ids: &[String],
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let requires_headroom = scenario_ids.iter().any(|id| {
        id.parse::<ScenarioId>()
            .map(|s| s.requires_boundary_headroom())
            .unwrap_or(false)
    });

    if !requires_headroom {
        debug!("No boundary scenarios selected; preserving base profile unmodified.");
        return Ok(profile);
    }

    info!(
        "Boundary scenario selected; adjusting base profile bounds to provide headroom for testing."
    );
    let mut unvalidated = profile.into_unvalidated();

    // Adjust AO points
    for ao in unvalidated.ao.all_ao_points_mut() {
        let mut min = ao.minimum();
        let mut max = ao.maximum();
        let mut changed = false;
        if min == TransmissionI32(i32::MIN) {
            min = TransmissionI32(i32::MIN + 1);
            changed = true;
        }
        if max == TransmissionI32(i32::MAX) {
            max = TransmissionI32(i32::MAX - 1);
            changed = true;
        }
        if changed {
            ao.set_transmission_bounds(min, max)?;
        }
    }

    // Adjust all AI points, including equipment and every curve/schedule entry.
    for ai in unvalidated.ai.all_ai_points_full_mut() {
        if is_multiplex_selector_ai(ai.point_index) {
            continue;
        }
        let mut min = ai.minimum();
        let mut max = ai.maximum();
        let mut changed = false;
        if min == TransmissionI32(i32::MIN) {
            min = TransmissionI32(i32::MIN + 1);
            changed = true;
        }
        if max == TransmissionI32(i32::MAX) {
            max = TransmissionI32(i32::MAX - 1);
            changed = true;
        }
        if changed {
            let eng_min = EngineeringF64::from_transmitted(min, ai.multiplier(), ai.offset);
            let eng_max = EngineeringF64::from_transmitted(max, ai.multiplier(), ai.offset);
            // Clamp while the old bounds still apply: set_eng_bounds validates the value.
            if ai.value() < eng_min {
                ai.set_value(eng_min)?;
            } else if ai.value() > eng_max {
                ai.set_value(eng_max)?;
            }
            ai.set_eng_bounds(eng_min, eng_max)?;
        }
    }

    unvalidated.into_validated()
}

// ---------------------------------------------------------------------------
// ModifyPoints
// ---------------------------------------------------------------------------
/// Set each AI point to a random value within its engineering limits.
fn modify_points(
    profile: Validated<PicsProfile>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    modify_points_with_rng(profile, &mut rand::thread_rng())
}

fn modify_points_with_rng(
    profile: Validated<PicsProfile>,
    random_number_generator: &mut impl Rng,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    // TODO: BoPoint has no value field. To change BO and BI values, add a value
    // field to BiPoint, which is the BI readback point.
    let mut unvalidated_profile = profile.into_unvalidated();
    let mut modified_point_count = 0;
    let ai_points: Vec<_> = unvalidated_profile
        .ai
        .all_ai_points_with_multiplexer_numbers()
        .into_iter()
        .map(|(multiplexer, point)| (multiplexer, point.clone()))
        .collect();
    for (multiplexer_number, point) in ai_points {
        if is_multiplex_selector_ai(point.point_index) {
            debug!(
                "Skipping multiplex edit selector AI point '{}' during modify_points",
                point.full_index()
            );
            continue;
        }
        match set_scenario_ai_point(
            &mut unvalidated_profile,
            multiplexer_number,
            &point,
            |point| set_random_ai_value(point, random_number_generator),
        ) {
            Ok(_) => {
                modified_point_count += 1;
            }
            Err(errors) => {
                debug!(
                    ?multiplexer_number,
                    "Cannot set a random value for AI point '{}': '{}'.",
                    point.full_index(),
                    errors
                        .errors
                        .into_iter()
                        .map(|error| error.message)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }
    info!(
        "Set random values for {} AI points in the profile",
        modified_point_count
    );
    unvalidated_profile.into_validated()
}

fn set_scenario_ai_point(
    profile: &mut PicsProfile,
    multiplexer: Option<MultiplexerNumber>,
    point: &AiPoint,
    update: impl FnOnce(&mut AiPoint) -> Result<(), ValidationErrors>,
) -> Result<(), ValidationErrors> {
    match multiplexer {
        Some(number) => profile.set_multiplexed_ai_point(number, point, update),
        None => profile.set_ai_point(point, update),
    }
}

fn set_random_ai_value(
    point: &mut AiPoint,
    random_number_generator: &mut impl Rng,
) -> Result<(), ValidationErrors> {
    let random_engineering_value =
        random_number_generator.gen_range(point.eng_minimum().0..=point.eng_maximum().0);
    point.set_value(EngineeringF64(random_engineering_value))
}

// ---------------------------------------------------------------------------
// LowerBounds / UpperBounds
// ---------------------------------------------------------------------------

enum BoundsDirection {
    Lower,
    Upper,
}

fn set_ai_out_of_bounds(
    profile: Validated<PicsProfile>,
    direction: BoundsDirection,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let mut unvalidated_profile = profile.into_unvalidated();
    let mut modified_point_count = 0;

    let ai_points: Vec<_> = unvalidated_profile
        .ai
        .all_ai_points_with_multiplexer_numbers()
        .into_iter()
        .map(|(multiplexer, point)| (multiplexer, point.clone()))
        .collect();

    for (multiplexer, point) in ai_points {
        if is_multiplex_selector_ai(point.point_index) {
            debug!(
                "Skipping multiplex edit selector AI point '{}' during set_ai_out_of_bounds. Corresponding AO: '{}'",
                point.full_index(),
                point
                    .assoc_ao
                    .clone()
                    .unwrap_or("(No associated AO)".to_string()),
            );
            continue;
        }
        let engineering_maximum =
            EngineeringF64::from_transmitted(point.maximum(), point.multiplier(), point.offset);
        let engineering_minimum =
            EngineeringF64::from_transmitted(point.minimum(), point.multiplier(), point.offset);
        let extended_maximum = engineering_maximum + EngineeringF64(1.0);
        let extended_minimum = engineering_minimum - EngineeringF64(1.0);
        let maximum_transmission_value = TransmissionI32::preview_from_engineering(
            extended_maximum,
            point.multiplier(),
            point.offset,
        );
        let minimum_transmission_value = TransmissionI32::preview_from_engineering(
            extended_minimum,
            point.multiplier(),
            point.offset,
        );

        if maximum_transmission_value > i32::MAX as f64
            || minimum_transmission_value < i32::MIN as f64
        {
            tracing::debug!(
                "AI{}: the new limits [{}, {}] are outside the i32 range. The value does not change. Associated AO: '{}'",
                point.point_index,
                extended_minimum.0,
                extended_maximum.0,
                point.assoc_ao.unwrap_or_default()
            );
            continue;
        }

        let value_outside_limits = match direction {
            BoundsDirection::Lower => extended_minimum,
            BoundsDirection::Upper => extended_maximum,
        };

        if let Err(err) =
            set_scenario_ai_point(&mut unvalidated_profile, multiplexer, &point, |p| {
                p.set_eng_bounds(extended_minimum, extended_maximum)?;
                p.set_value(value_outside_limits)
            })
        {
            debug!(
                ?multiplexer,
                "Cannot set either eng bounds or value for AI point '{}': {:?}. Associated AO: '{}'",
                point.full_index(),
                err,
                point.assoc_ao.unwrap_or_default()
            );
            continue;
        }

        debug!(
            ?multiplexer,
            "Set AI point '{}' to value '{}' outside its limits [{}, {}]. Associated AO: '{}'",
            point.full_index(),
            point.value(),
            point.eng_minimum().0,
            point.eng_maximum().0,
            point.assoc_ao.unwrap_or_default()
        );
        modified_point_count += 1;
    }
    let validated_profile = unvalidated_profile.into_validated()?;
    info!(
        "Set {} AI point values outside their limits in the profile",
        modified_point_count
    );
    Ok(validated_profile)
}

/// Return a profile containing only the points present in `full_profile` but
/// absent from the submitted profile.
fn generate_unsupported_profile(
    profile: Validated<PicsProfile>,
    full_profile: &Validated<PicsProfile>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    // Build point_index sets for each base section of the submitted profile.
    let submitted_ao: HashSet<u16> = profile.ao.points.iter().map(|p| p.point_index).collect();
    let submitted_bo: HashSet<u16> = profile.bo.points.iter().map(|p| p.point_index).collect();
    let submitted_ai: HashSet<u16> = profile.ai.points.iter().map(|p| p.point_index).collect();
    let submitted_bi: HashSet<u16> = profile.bi.points.iter().map(|p| p.point_index).collect();

    // Build flat point_index sets covering ALL AI points inside submitted curves/schedules.
    // A curve/schedule is considered "present" in the submitted profile if ANY of its
    // AI point indices appear in these sets; it is "unsupported" (and included in the
    // output) only when NONE of its AI point indices appear.
    let submitted_curve_ai: HashSet<u16> = profile
        .ai
        .curves
        .iter()
        .flat_map(|c| c.iter_points())
        .map(|p| p.point_index)
        .collect();

    let submitted_sched_bc_ai: HashSet<u16> = profile
        .ai
        .schedules_bc
        .iter()
        .flat_map(|s| s.iter_points())
        .map(|p| p.point_index)
        .collect();

    let submitted_sched_ai: HashSet<u16> = profile
        .ai
        .schedules
        .iter()
        .flat_map(|s| s.iter_points())
        .map(|p| p.point_index)
        .collect();

    PicsProfile::new(
        profile.key.clone(),
        BinaryOutputs {
            points: full_profile
                .bo
                .points
                .iter()
                .filter(|p| !submitted_bo.contains(&p.point_index))
                .cloned()
                .collect(),
        },
        BinaryInputs {
            points: full_profile
                .bi
                .points
                .iter()
                .filter(|p| !submitted_bi.contains(&p.point_index))
                .cloned()
                .collect(),
            meters: full_profile
                .bi
                .meters
                .iter()
                .skip(profile.bi.meters.len())
                .cloned()
                .collect(),
            ders: full_profile
                .bi
                .ders
                .iter()
                .skip(profile.bi.ders.len())
                .cloned()
                .collect(),
            inverters: full_profile
                .bi
                .inverters
                .iter()
                .skip(profile.bi.inverters.len())
                .cloned()
                .collect(),
            batteries: full_profile
                .bi
                .batteries
                .iter()
                .skip(profile.bi.batteries.len())
                .cloned()
                .collect(),
        },
        AnalogOutputs {
            points: full_profile
                .ao
                .points
                .iter()
                .filter(|p| !submitted_ao.contains(&p.point_index))
                .cloned()
                .collect(),
            // Equipment sub-groups: positional — include full entries beyond what submitted has.
            meters: full_profile
                .ao
                .meters
                .iter()
                .skip(profile.ao.meters.len())
                .cloned()
                .collect(),
            inverters: full_profile
                .ao
                .inverters
                .iter()
                .skip(profile.ao.inverters.len())
                .cloned()
                .collect(),
            batteries: full_profile
                .ao
                .batteries
                .iter()
                .skip(profile.ao.batteries.len())
                .cloned()
                .collect(),
        },
        AnalogInputs {
            points: full_profile
                .ai
                .points
                .iter()
                .filter(|p| !submitted_ai.contains(&p.point_index))
                .cloned()
                .collect(),
            // Curves/schedules: include if NONE of their AI point indices appear in submitted.
            curves: full_profile
                .ai
                .curves
                .iter()
                .filter(|c| {
                    !c.iter_points()
                        .iter()
                        .any(|p| submitted_curve_ai.contains(&p.point_index))
                })
                .cloned()
                .collect(),
            schedules_bc: full_profile
                .ai
                .schedules_bc
                .iter()
                .filter(|s| {
                    !s.iter_points()
                        .iter()
                        .any(|p| submitted_sched_bc_ai.contains(&p.point_index))
                })
                .cloned()
                .collect(),
            schedules: full_profile
                .ai
                .schedules
                .iter()
                .filter(|s| {
                    !s.iter_points()
                        .iter()
                        .any(|p| submitted_sched_ai.contains(&p.point_index))
                })
                .cloned()
                .collect(),
            meters: full_profile
                .ai
                .meters
                .iter()
                .skip(profile.ai.meters.len())
                .cloned()
                .collect(),
            ders: full_profile
                .ai
                .ders
                .iter()
                .skip(profile.ai.ders.len())
                .cloned()
                .collect(),
            inverters: full_profile
                .ai
                .inverters
                .iter()
                .skip(profile.ai.inverters.len())
                .cloned()
                .collect(),
            batteries: full_profile
                .ai
                .batteries
                .iter()
                .skip(profile.ai.batteries.len())
                .cloned()
                .collect(),
        },
        vec![],
    )
}

// ---------------------------------------------------------------------------
// MaxCurves / MaxSchedules
// ---------------------------------------------------------------------------

fn append_clone_of_last_curve(
    profile: Validated<PicsProfile>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let mut new_profile = profile.into_unvalidated();

    match new_profile.ai.curves.last().cloned() {
        Some(last_curve) => {
            new_profile.ai.curves.push(last_curve);
            Ok(new_profile.into_validated()?)
        }
        None => Err(ValidationErrors::from_error(ValidationError {
            point: "".to_string(),
            message: "MaxCurves scenario requires at least one curve in the profile".to_string(),
        })),
    }
}

fn append_clones_of_last_schedules(
    profile: Validated<PicsProfile>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let mut new_profile = profile.into_unvalidated();

    let has_bc = new_profile.ai.schedules_bc.last().cloned().map(|last| {
        new_profile.ai.schedules_bc.push(last);
    });
    let has_sched = new_profile.ai.schedules.last().cloned().map(|last| {
        new_profile.ai.schedules.push(last);
    });
    if has_bc.is_none() && has_sched.is_none() {
        Err(ValidationErrors::from_error(ValidationError {
            point: "".to_string(),
            message: "MaxSchedules scenario requires at least one schedule in the profile"
                .to_string(),
        }))
    } else {
        Ok(new_profile.into_validated()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_full_profile() -> Validated<PicsProfile> {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
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

    #[test]
    fn test_configuration_identity() {
        let profile = load_full_profile();
        let result = update_profile(&ScenarioId::Configuration, profile.clone(), &profile).unwrap();
        assert_eq!(result, profile);
    }

    #[test]
    fn test_modify_points_changes_values() {
        let profile = load_full_profile();
        let modified =
            update_profile(&ScenarioId::ModifyPoints, profile.clone(), &profile).unwrap();

        // Every point's value should remain within its declared bounds.
        for (original_pt, modified_pt) in profile.ai.points.iter().zip(modified.ai.points.iter()) {
            let eng_val = modified_pt.value();
            assert!(
                eng_val >= original_pt.eng_minimum() && eng_val <= original_pt.eng_maximum(),
                "AI{} value {} is outside [{}, {}]",
                modified_pt.point_index,
                eng_val,
                original_pt.minimum(),
                original_pt.maximum()
            );
        }

        // At least one value with a non-degenerate range should have actually changed.
        let changed = profile
            .ai
            .points
            .iter()
            .zip(modified.ai.points.iter())
            .any(|(orig, modp)| orig.minimum() < orig.maximum() && orig.value() != modp.value());
        assert!(
            changed,
            "modify_points should change at least one AI point value"
        );
    }

    #[test]
    fn test_modify_points_changes_curve_values() {
        let profile = load_full_profile();
        if profile.ai.curves.is_empty() {
            return; // nothing to test
        }
        let modified =
            update_profile(&ScenarioId::ModifyPoints, profile.clone(), &profile).unwrap();

        // At least one curve AI point with a non-degenerate range should have changed.
        let changed =
            profile
                .ai
                .curves
                .iter()
                .zip(modified.ai.curves.iter())
                .any(|(orig_c, mod_c)| {
                    orig_c
                        .iter_points()
                        .iter()
                        .zip(mod_c.iter_points().iter())
                        .any(|(orig_pt, mod_pt)| {
                            orig_pt.minimum() < orig_pt.maximum && orig_pt.value() != mod_pt.value()
                        })
                });
        assert!(
            changed,
            "modify_points should change at least one curve AI point value"
        );
    }

    fn assert_selectors_preserved(original: &PicsProfile, modified: &PicsProfile) {
        for point in original
            .ai
            .points
            .iter()
            .filter(|p| is_multiplex_selector_ai(p.point_index))
        {
            let updated = modified
                .ai
                .points
                .iter()
                .find(|p| p.point_index == point.point_index)
                .unwrap();
            assert_eq!(updated, point);
        }
    }

    #[test]
    fn test_modify_points_updates_every_multiplexer_entry() {
        use rand::SeedableRng;

        let profile = load_full_profile();
        let modified =
            modify_points_with_rng(profile.clone(), &mut rand::rngs::StdRng::seed_from_u64(1))
                .unwrap();
        for (before, after) in profile.ai.curves.iter().zip(&modified.ai.curves) {
            assert_ne!(before.x_values[0].value(), after.x_values[0].value());
        }
        for (before, after) in profile
            .ai
            .schedules_bc
            .iter()
            .zip(&modified.ai.schedules_bc)
        {
            assert_ne!(before.identity.value(), after.identity.value());
        }
        for (before, after) in profile.ai.schedules.iter().zip(&modified.ai.schedules) {
            assert_ne!(before.identity.value(), after.identity.value());
        }
        assert_selectors_preserved(&profile, &modified);
    }

    #[test]
    fn test_boundary_scenarios_update_every_multiplexer_entry() {
        let full_profile = load_full_profile();
        let mut profile = full_profile.clone().into_unvalidated();
        // A single coordinate with undefined units can expand its bounds without
        // violating the curve scaling table or consistency between coordinates.
        for curve in &mut profile.ai.curves {
            curve.curve_type.set_value(EngineeringF64(0.0)).unwrap();
            curve.x_units.set_value(EngineeringF64(0.0)).unwrap();
            curve.y_units.set_value(EngineeringF64(0.0)).unwrap();
            curve
                .number_of_points
                .set_value(EngineeringF64(1.0))
                .unwrap();
            curve.x_values.truncate(1);
            curve.y_values.truncate(1);
        }
        let prepared = prepare_base_profile_for_scenarios(
            profile.into_validated().unwrap(),
            &["lower_bounds".to_string(), "upper_bounds".to_string()],
        )
        .unwrap();
        for scenario in [ScenarioId::BelowLowerBounds, ScenarioId::AboveUpperBounds] {
            let modified = update_profile(&scenario, prepared.clone(), &full_profile).unwrap();
            let before = prepared
                .ai
                .curves
                .iter()
                .map(|c| &c.x_values[0])
                .chain(prepared.ai.schedules_bc.iter().map(|s| &s.identity))
                .chain(prepared.ai.schedules.iter().map(|s| &s.identity));
            let after = modified
                .ai
                .curves
                .iter()
                .map(|c| &c.x_values[0])
                .chain(modified.ai.schedules_bc.iter().map(|s| &s.identity))
                .chain(modified.ai.schedules.iter().map(|s| &s.identity));
            for (before, after) in before.zip(after) {
                match scenario {
                    ScenarioId::BelowLowerBounds => assert!(
                        after.value() < before.eng_minimum(),
                        "{}",
                        before.full_index()
                    ),
                    ScenarioId::AboveUpperBounds => assert!(
                        after.value() > before.eng_maximum(),
                        "{}",
                        before.full_index()
                    ),
                    _ => unreachable!(),
                }
            }
            assert_selectors_preserved(&prepared, &modified);
        }
    }

    #[test]
    fn test_lower_bounds_sets_values_below_original_minimum() {
        let profile = load_full_profile();
        let modified =
            update_profile(&ScenarioId::BelowLowerBounds, profile.clone(), &profile).unwrap();
        let mut modification_count = 0;
        for (original_pt, modified_pt) in profile.ai.points.iter().zip(modified.ai.points.iter()) {
            if modified_pt.value() < original_pt.eng_minimum() {
                modification_count += 1;
            }
        }
        assert!(
            modification_count >= 100,
            "Expected to modify at least 100 AI points"
        );
    }

    #[test]
    fn test_upper_bounds_sets_values_above_original_maximum() {
        let profile = load_full_profile();
        let modified =
            update_profile(&ScenarioId::AboveUpperBounds, profile.clone(), &profile).unwrap();
        let mut modification_count = 0;
        for (original_pt, modified_pt) in profile.ai.points.iter().zip(modified.ai.points.iter()) {
            if modified_pt.value() > original_pt.eng_maximum() {
                modification_count += 1;
            }
        }
        assert!(
            modification_count >= 100,
            "Expected to modify at least 100 AI points"
        );
    }

    #[test]
    fn test_max_curves_adds_one_curve() {
        let profile = load_full_profile();
        let original_count = profile.ai.curves.len();
        if original_count > 0 {
            let modified =
                update_profile(&ScenarioId::MaxCurves, profile, &load_full_profile()).unwrap();
            assert_eq!(modified.ai.curves.len(), original_count + 1);
        }
    }

    #[test]
    fn test_max_curves_errors_when_empty() {
        let mut profile = load_full_profile().into_unvalidated();
        profile.ai.curves.clear();
        let result = update_profile(
            &ScenarioId::MaxCurves,
            profile.into_validated().unwrap(),
            &load_full_profile(),
        );
        assert!(
            result.is_err(),
            "MaxCurves should return Err when profile has no curves"
        );
    }

    #[test]
    fn test_max_schedules_adds_one_schedule() {
        let profile = load_full_profile();
        let has_bc = !profile.ai.schedules_bc.is_empty();
        let has_sched = !profile.ai.schedules.is_empty();
        if !has_bc && !has_sched {
            return; // nothing to test
        }
        let orig_bc = profile.ai.schedules_bc.len();
        let orig_sched = profile.ai.schedules.len();
        let modified =
            update_profile(&ScenarioId::MaxSchedules, profile, &load_full_profile()).unwrap();
        if has_bc {
            assert_eq!(modified.ai.schedules_bc.len(), orig_bc + 1);
        }
        if has_sched {
            assert_eq!(modified.ai.schedules.len(), orig_sched + 1);
        }
    }

    #[test]
    fn test_unsupported_returns_diff() {
        let full_profile = load_full_profile();
        let mut submitted = full_profile.clone().into_unvalidated();
        if !submitted.ao.points.is_empty() {
            let removed = submitted.ao.points.remove(0);
            let result = update_profile(
                &ScenarioId::Unsupported,
                submitted.into_validated().unwrap(),
                &full_profile,
            )
            .unwrap();
            assert!(
                result
                    .ao
                    .points
                    .iter()
                    .any(|p| p.point_index == removed.point_index),
                "Unsupported scenario should include base AO points not in submitted profile"
            );
        }
    }

    #[test]
    fn test_unsupported_includes_missing_curves() {
        let full_profile = load_full_profile();
        if full_profile.ai.curves.is_empty() {
            return; // nothing to test
        }
        let mut submitted = full_profile.clone().into_unvalidated();
        submitted.ai.curves.clear();
        let result = update_profile(
            &ScenarioId::Unsupported,
            submitted.into_validated().unwrap(),
            &full_profile,
        )
        .unwrap();
        assert_eq!(
            result.ai.curves.len(),
            full_profile.ai.curves.len(),
            "All curves from full.json should appear in unsupported result when submitted has none"
        );
    }

    #[test]
    fn test_unsupported_excludes_present_curves() {
        let full_profile = load_full_profile();
        let submitted = full_profile.clone();
        let result = update_profile(&ScenarioId::Unsupported, submitted, &full_profile).unwrap();
        assert_eq!(
            result.ai.curves.len(),
            0,
            "No curves should appear in unsupported result when submitted matches full.json"
        );
    }

    #[test]
    fn test_unsupported_equipment_positional() {
        let full_profile = load_full_profile();
        let submitted = full_profile.clone();
        let result = update_profile(&ScenarioId::Unsupported, submitted, &full_profile).unwrap();
        assert_eq!(result.ao.meters.len(), 0);
        assert_eq!(result.bi.meters.len(), 0);
        assert_eq!(result.ai.meters.len(), 0);
    }

    #[test]
    fn test_prepare_base_profile_skips_when_no_boundary_scenarios() {
        let full_profile = load_full_profile();
        let non_boundary_scenarios = vec![
            "configuration".to_string(),
            "modify_points".to_string(),
            "functional".to_string(),
        ];
        let result =
            prepare_base_profile_for_scenarios(full_profile.clone(), &non_boundary_scenarios)
                .unwrap();
        assert_eq!(result, full_profile);
    }

    #[test]
    fn test_prepare_base_profile_adjusts_when_boundary_scenario_selected() {
        let full_profile = load_full_profile();
        for scenario in ["lower_bounds", "upper_bounds"] {
            let result =
                prepare_base_profile_for_scenarios(full_profile.clone(), &[scenario.to_string()])
                    .unwrap();
            for ao in result.ao.all_ao_points() {
                assert_ne!(
                    ao.minimum(),
                    TransmissionI32(i32::MIN),
                    "AO{} should not be i32::MIN after boundary preparation",
                    ao.point_index
                );
                assert_ne!(
                    ao.maximum(),
                    TransmissionI32(i32::MAX),
                    "AO{} should not be i32::MAX after boundary preparation",
                    ao.point_index
                );
            }
            for (original, ai) in full_profile
                .ai
                .all_ai_points_full()
                .into_iter()
                .zip(result.ai.all_ai_points_full())
            {
                if is_multiplex_selector_ai(ai.point_index) {
                    assert_eq!(ai, original, "multiplex selectors must be preserved");
                    continue;
                }
                assert_ne!(
                    ai.minimum(),
                    TransmissionI32(i32::MIN),
                    "{}",
                    ai.full_index()
                );
                assert_ne!(
                    ai.maximum(),
                    TransmissionI32(i32::MAX),
                    "{}",
                    ai.full_index()
                );
                assert_eq!(ai.value(), original.value(), "{}", ai.full_index());
            }

            // Prepared profiles are serialized for the reference stations, which validate again.
            let json = serde_json::to_string(&result).unwrap();
            let reloaded: PicsProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(reloaded.into_validated().unwrap(), result);
        }
    }

    #[test]
    fn test_prepare_base_profile_clamps_values_at_integer_extremes() {
        let mut profile = load_full_profile().into_unvalidated();
        let meter = &mut profile.ai.meters[0];
        let meter_min = meter.active_power.eng_minimum();
        meter.active_power.set_value(meter_min).unwrap();
        let meter_max = meter.reactive_power.eng_maximum();
        meter.reactive_power.set_value(meter_max).unwrap();

        let result = prepare_base_profile_for_scenarios(
            profile.into_validated().unwrap(),
            &["lower_bounds".to_string()],
        )
        .unwrap();
        let meter = &result.ai.meters[0];
        assert!(meter.active_power.value() > meter_min);
        assert_eq!(meter.active_power.value(), meter.active_power.eng_minimum());
        assert!(meter.reactive_power.value() < meter_max);
        assert_eq!(
            meter.reactive_power.value(),
            meter.reactive_power.eng_maximum()
        );
        result.into_unvalidated().into_validated().unwrap();
    }
}
