//! Point-level conformance checks for the reference control station.
//!
//! All groups share profile, receipt/write, and analog readback checks. Curves
//! and schedules additionally compare their multiplexed readbacks with the profile.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use common::{
    conformance_testing::{
        logging::{log_conformance_result, log_point_check_result},
        message_structure::{CheckPoint, PointCheckResult, SunSpecComment, TestName},
        point_lists::{self, ConformancePoints},
    },
    profile::{AiPoint, AiValue, PointIndex, PointValue, ProfileIndex},
};
use tracing::{Level, span};

use crate::state::ControlStationState;

fn check_result(
    scenario_id: &str,
    test: TestName,
    point: &CheckPoint,
    check_id: &str,
    label: &str,
    passed: bool,
    message: String,
) -> PointCheckResult {
    PointCheckResult {
        scenario_id: scenario_id.into(),
        test,
        group_name: test.display_name().into(),
        point: point.clone(),
        sequence: 0,
        timestamp: 0,
        check_id: check_id.into(),
        expected_behavior: label.into(),
        passed,
        message,
    }
}

fn run_test(state: &ControlStationState, test: TestName, points: ConformancePoints) {
    let scenario_id = state.scenario_id.lock().unwrap().clone();
    let mut checks = {
        let profile = state.profile_index.read().unwrap();
        let received = state.latest_transmission_values.read().unwrap();
        let sent_ao = state.sent_ao_indices.read().unwrap();
        let sent_bo = state.sent_bo_indices.read().unwrap();
        evaluate_points(
            &profile,
            &received,
            &sent_ao,
            &sent_bo,
            &points,
            &scenario_id,
            test,
        )
    };
    checks.extend(database_readback_checks(state, &scenario_id, test));

    let comments = checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| SunSpecComment {
            uid: check.point.uid.clone(),
            index: format!("{}{}", check.point.point_type, check.point.index),
            value: String::new(),
            issue: format!("{}: {}", check.expected_behavior, check.message),
        })
        .collect::<Vec<_>>();
    for check in checks {
        log_point_check_result(check);
    }
    log_conformance_result(comments.is_empty(), comments);
}

/// Evaluate all point types against one snapshot; every outcome is reported.
fn evaluate_points(
    profile: &ProfileIndex,
    received: &HashMap<String, serde_json::Value>,
    sent_ao: &HashSet<u16>,
    sent_bo: &HashSet<u16>,
    points: &ConformancePoints,
    scenario_id: &str,
    test: TestName,
) -> Vec<PointCheckResult> {
    let identities = points
        .ai
        .iter()
        .map(|uid| ("AI", *uid as u16, format!("{uid:?}")))
        .chain(
            points
                .bi
                .iter()
                .map(|uid| ("BI", *uid as u16, format!("{uid:?}"))),
        )
        .chain(
            points
                .ao
                .iter()
                .map(|uid| ("AO", *uid as u16, format!("{uid:?}"))),
        )
        .chain(
            points
                .bo
                .iter()
                .map(|uid| ("BO", *uid as u16, format!("{uid:?}"))),
        );
    let mut checks = Vec::new();
    for (point_type, index, uid) in identities {
        let (definition, bounds, data_type, written) = match point_type {
            "AI" => (
                profile
                    .ai_points
                    .get(&index)
                    .map(|p| (p.name.clone(), p.iec_61850_uid.clone())),
                profile
                    .ai_points
                    .get(&index)
                    .map(|p| (p.minimum().0, p.maximum().0)),
                "analog_input",
                None,
            ),
            "BI" => (
                profile
                    .bi_points
                    .get(&index)
                    .map(|p| (p.name.clone(), p.iec_61850_uid.clone())),
                None,
                "binary_input",
                None,
            ),
            "AO" => (
                profile
                    .ao_points
                    .get(&index)
                    .map(|p| (p.name.clone(), p.iec_61850_uid.clone())),
                profile
                    .ao_points
                    .get(&index)
                    .map(|p| (p.minimum().0, p.maximum().0)),
                "analog_output",
                Some(sent_ao.contains(&index)),
            ),
            "BO" => (
                profile
                    .bo_points
                    .get(&index)
                    .map(|p| (p.name.clone(), p.iec_61850_uid.clone())),
                None,
                "binary_output",
                Some(sent_bo.contains(&index)),
            ),
            _ => unreachable!(),
        };
        let (name, uid) = definition
            .map(|(name, uid)| (Some(name), uid))
            .unwrap_or((None, uid));
        let point = CheckPoint {
            point_type: point_type.into(),
            index,
            uid,
            name,
        };
        let record = received.get(&format!("{data_type}:{index}"));
        let value = record.and_then(|record| record.get("value"));
        let mut add = |check_id: &str, label: &str, passed, message| {
            checks.push(check_result(
                scenario_id,
                test,
                &point,
                check_id,
                label,
                passed,
                message,
            ));
        };
        add(
            "profile_definition",
            "Defined in profile",
            point.name.is_some(),
            format!(
                "{point_type}{index} {} {} the loaded PICS profile.",
                point.uid,
                if point.name.is_some() {
                    "is defined in"
                } else {
                    "is missing from"
                }
            ),
        );

        match written {
            Some(passed) => add(
                "write_accepted",
                "Write accepted",
                passed,
                if passed {
                    "The outstation reported success for a write since the scenario reset.".into()
                } else {
                    "No successful write completed since the scenario reset.".into()
                },
            ),
            None => {
                let valid = if point_type == "AI" {
                    value.and_then(|v| v.as_f64()).is_some()
                } else {
                    value.and_then(|v| v.as_bool()).is_some()
                };
                add(
                    "value_received",
                    "Value received",
                    valid,
                    if valid {
                        format!(
                            "Received transmitted value {} at {} since the scenario reset.",
                            value.unwrap(),
                            record
                                .and_then(|r| r["ts"].as_str())
                                .unwrap_or("unknown receipt time")
                        )
                    } else {
                        "No value of the expected type received since the scenario reset.".into()
                    },
                );
            }
        }

        if point_type == "AI" || point_type == "AO" {
            let (passed, message) = match (bounds, value.and_then(|v| v.as_f64())) {
                (Some((min, max)), Some(value)) => (
                    value.is_finite() && value >= f64::from(min) && value <= f64::from(max),
                    format!(
                        "Received transmitted value {value}; inclusive transmitted bounds [{min}, {max}]."
                    ),
                ),
                (None, _) => (
                    false,
                    "Cannot check range: the point has no profile definition or bounds.".into(),
                ),
                (_, None) => (
                    false,
                    format!(
                        "Cannot check range: no numeric {} received since the scenario reset.",
                        if point_type == "AO" {
                            "output status value"
                        } else {
                            "value"
                        }
                    ),
                ),
            };
            add("value_in_range", "Value within range", passed, message);
        }
    }
    checks
}

/// The storage differs by mode, but every multiplexed value uses the same check.
fn database_readback_checks(
    state: &ControlStationState,
    scenario_id: &str,
    test: TestName,
) -> Vec<PointCheckResult> {
    let profile = state.profile.read().unwrap();
    match test {
        TestName::CURVE_001 | TestName::CURVE_002 | TestName::CURVE_003 => {
            let db = state.outstation_curve_database.read().unwrap();
            check_entries(
                scenario_id,
                test,
                "Curve",
                profile.ai.curves.iter().map(|p| p.iter_points()).collect(),
                |entry| db.received_values(entry),
            )
        }
        TestName::SCHED_001 => {
            let db = state.schedule_bc_database.read().unwrap();
            check_entries(
                scenario_id,
                test,
                "BC schedule",
                profile
                    .ai
                    .schedules_bc
                    .iter()
                    .map(|p| {
                        p.iter_points()
                            .into_iter()
                            .filter(|point| point.assoc_ao.is_some())
                            .collect()
                    })
                    .collect(),
                |entry| db.written_values(entry),
            )
        }
        TestName::SCHED_002 => {
            let db = state.schedule_database.read().unwrap();
            check_entries(
                scenario_id,
                test,
                "Schedule",
                profile
                    .ai
                    .schedules
                    .iter()
                    .map(|p| {
                        p.iter_points()
                            .into_iter()
                            .filter(|point| point.assoc_ao.is_some())
                            .collect()
                    })
                    .collect(),
                |entry| db.written_values(entry),
            )
        }
        _ => Vec::new(),
    }
}

fn check_entries(
    scenario_id: &str,
    test: TestName,
    kind: &str,
    entries: Vec<Vec<&AiPoint>>,
    mut read_entry: impl FnMut(u16) -> Option<Vec<PointValue>>,
) -> Vec<PointCheckResult> {
    let mut checks = Vec::new();
    for (entry_index, points) in entries.into_iter().enumerate() {
        let entry_number = entry_index as u16 + 1;
        let received = read_entry(entry_number);
        for definition in points {
            let point = CheckPoint {
                point_type: "AI".into(),
                index: definition.point_index,
                uid: definition.iec_61850_uid.clone(),
                name: Some(definition.name.clone()),
            };
            let actual = received.as_ref().and_then(|values| {
                values
                    .iter()
                    .find(|value| value.key() == PointIndex::Ai(definition.point_index))
            });
            let (passed, message) = match (AiValue::try_from(definition), actual) {
                (Ok(expected), Some(actual)) => (
                    *actual == PointValue::Ai(expected),
                    format!(
                        "{kind} {entry_number}: expected transmitted value {}; received {actual}.",
                        expected.value
                    ),
                ),
                (Err(error), _) => (
                    false,
                    format!("{kind} {entry_number}: cannot scale the profile value: {error}"),
                ),
                (_, None) => (
                    false,
                    format!(
                        "{kind} {entry_number}: {}.",
                        if received.is_none() {
                            "database entry unavailable"
                        } else {
                            "point readback not received"
                        }
                    ),
                ),
            };
            checks.push(check_result(
                scenario_id,
                test,
                &point,
                &format!("{kind}_{entry_number}_readback"),
                &format!("{kind} {entry_number}: Readback matches profile"),
                passed,
                message,
            ));
        }
    }
    checks
}

/// Run all conformance tests against the current control station state.
///
/// Each test is wrapped in a tracing span whose `test_name` field is captured
/// by the [`ConformanceTrackingLayer`][common::conformance_testing::logging::ConformanceTrackingLayer]
/// to associate logged events with the correct [`TestName`].
///
/// # Note on startup validity
///
/// Results are only meaningful after at least one complete integrity poll has returned.
/// Tests that check AI values will report "Not received" failures if called before
/// the outstation has responded to the first poll.
pub fn run_conformance_tests(state: &Arc<ControlStationState>) {
    tracing::info!("Running conformance tests.");
    macro_rules! run {
        ($test_name:ident, $fn:ident) => {{
            let _span =
                span!(Level::INFO, "", test_name = %TestName::$test_name).entered();
            run_test(state, TestName::$test_name, point_lists::$fn());
        }};
    }

    run!(MON_001, mon_001);
    run!(ALARM_001, alarm_001);
    run!(CONN_001, conn_001);
    run!(SERV_001, serv_001);
    run!(OP_001, op_001);
    run!(CURVE_001, curve_001);
    run!(CURVE_002, curve_002);
    run!(CURVE_003, curve_003);
    run!(VRT_001, vrt_001);
    run!(FRT_001, frt_001);
    run!(FW_001, fw_001);
    run!(DRCS_001, drcs_001);
    run!(DVW_001, dvw_001);
    run!(APL_001, apl_001);
    run!(CHG_001, chg_001);
    run!(CCM_001, ccm_001);
    run!(APR1_001, apr1_001);
    run!(APR2_001, apr2_001);
    run!(APR3_001, apr3_001);
    run!(AGC_001, agc_001);
    run!(APS_001, aps_001);
    run!(VW_001, vw_001);
    run!(FWC_001, fwc_001);
    run!(CVAR_001, cvar_001);
    run!(FPF_001, fpf_001);
    run!(VV_001, vv_001);
    run!(WV_001, wv_001);
    run!(PFC_001, pfc_001);
    run!(PSIG_001, psig_001);
    run!(SCHED_001, sched_001);
    run!(SCHED_002, sched_002);
}
