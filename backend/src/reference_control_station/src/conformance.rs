//! Semantic conformance tests for the reference control station.
//!
//! Each test checks that the required DNP3 points for a control mode have been interacted
//! with — specifically:
//! - **AI / BI points**: the control station has received at least one value for the point.
//! - **AO / BO points**: the control station has sent at least one write to the point.
//!
//! Results are logged via [`common::conformance_testing::logging::log_conformance_result`] and
//! captured by the [`common::conformance_testing::logging::ConformanceTrackingLayer`] that is
//! wired into the tracing subscriber in `main.rs`.

use std::{collections::HashMap, sync::Arc};

use common::profile::{AiValue, PointIndex, PointValue};
use common::uids::{ai_uid::AiUid, ao_uid::AoUid, bi_uid::BiUid, bo_uid::BoUid};
use common::{
    conformance_testing::{
        logging::log_conformance_result,
        message_structure::{ConformanceCheck, SunSpecComment, TestName},
    },
    profile::validation::ValidationErrors,
};
use tracing::{Level, span};

use crate::state::{ControlStationState, RangeViolation};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reports whether the AI point at `uid` has been received from the outstation.
fn validate_ai_received(state: &ControlStationState, uid: AiUid) -> SunSpecComment {
    let index = uid as u16;
    let key = format!("analog_input:{index}");
    let passed = state
        .latest_transmission_values
        .read()
        .unwrap()
        .contains_key(&key);

    SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AI{index}"),
        check: ConformanceCheck::ReceivedFromOutstation,
        value: if passed {
            "Received".into()
        } else {
            "Not received".into()
        },
        message: format!(
            "AI{index} has {} from the outstation",
            if passed {
                "been received"
            } else {
                "not been received"
            }
        ),
        passed,
    }
}

/// Reports whether the BI point at `uid` has been received from the outstation.
fn validate_bi_received(state: &ControlStationState, uid: BiUid) -> SunSpecComment {
    let index = uid as u16;
    let key = format!("binary_input:{index}");
    let passed = state
        .latest_transmission_values
        .read()
        .unwrap()
        .contains_key(&key);
    SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("BI{index}"),
        check: ConformanceCheck::ReceivedFromOutstation,
        value: if passed {
            "Received".into()
        } else {
            "Not received".into()
        },
        message: format!(
            "BI{index} has {} from the outstation",
            if passed {
                "been received"
            } else {
                "not been received"
            }
        ),
        passed,
    }
}

/// Reports whether the AO point at `uid` has been sent to the outstation.
fn validate_ao_sent(state: &ControlStationState, uid: AoUid) -> SunSpecComment {
    let index = uid as u16;
    let passed = state.sent_ao_indices.read().unwrap().contains(&index);
    SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AO{index}"),
        check: ConformanceCheck::WrittenToOutstation,
        value: if passed {
            "Sent".into()
        } else {
            "Not sent".into()
        },
        message: format!(
            "AO{index} has {} to the outstation",
            if passed {
                "been written"
            } else {
                "not been written"
            }
        ),
        passed,
    }
}

/// Reports whether an AI range violation has been recorded for `uid`.
fn validate_ai_in_range(state: &ControlStationState, uid: AiUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if !state
        .profile_index
        .read()
        .unwrap()
        .ai_points
        .contains_key(&index)
        || !state
            .latest_transmission_values
            .read()
            .unwrap()
            .contains_key(&format!("analog_input:{index}"))
    {
        return None;
    }
    let violation = state
        .ai_range_violations
        .read()
        .unwrap()
        .get(&index)
        .copied();
    let passed = violation.is_none();
    let (value, message) = match violation {
        Some(RangeViolation { value, min, max }) => (
            value.to_string(),
            format!("AI{index} value {value} is out of the profile range [{min}, {max}]"),
        ),
        None => (
            "No violation recorded".into(),
            format!("AI{index} has no recorded range violation"),
        ),
    };
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AI{index}"),
        check: ConformanceCheck::WithinProfileRange,
        value,
        message,
        passed,
    })
}

/// Reports whether an AO readback range violation has been recorded for `uid`.
fn validate_ao_in_range(state: &ControlStationState, uid: AoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if !state
        .profile_index
        .read()
        .unwrap()
        .ao_points
        .contains_key(&index)
        || !state
            .latest_transmission_values
            .read()
            .unwrap()
            .contains_key(&format!("analog_output:{index}"))
    {
        return None;
    }
    let violation = state
        .ao_range_violations
        .read()
        .unwrap()
        .get(&index)
        .copied();
    let passed = violation.is_none();
    let (value, message) = match violation {
        Some(RangeViolation { value, min, max }) => (
            value.to_string(),
            format!("AO{index} value {value} is out of the profile range [{min}, {max}]"),
        ),
        None => (
            "No violation recorded".into(),
            format!("AO{index} has no recorded range violation"),
        ),
    };
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AO{index}"),
        check: ConformanceCheck::WithinProfileRange,
        value,
        message,
        passed,
    })
}

/// Reports whether the BO point at `uid` has been sent to the outstation.
fn validate_bo_sent(state: &ControlStationState, uid: BoUid) -> SunSpecComment {
    let index = uid as u16;
    let passed = state.sent_bo_indices.read().unwrap().contains(&index);
    SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("BO{index}"),
        check: ConformanceCheck::WrittenToOutstation,
        value: if passed {
            "Sent".into()
        } else {
            "Not sent".into()
        },
        message: format!(
            "BO{index} has {} to the outstation",
            if passed {
                "been written"
            } else {
                "not been written"
            }
        ),
        passed,
    }
}

/// Validate curve readback state against the PICS profile.
///
/// For each curve entry N in the database:
/// - If no values have been received (empty written_values), reports that readback
///   was not received for that entry.
/// - Otherwise, compares each received value against the profile's expected value
///   and reports any mismatches.
fn validate_curves_from_outstation(state: &ControlStationState) -> Vec<SunSpecComment> {
    let profile_guard = state.profile.read().unwrap();
    let profile = &*profile_guard;
    let selector_uid = AiUid::Curve_DGSMn_InCrv;
    let db = state.outstation_curve_database.read().unwrap();
    let mut comments = Vec::new();

    for (curve_idx, curve) in profile.ai.curves.iter().enumerate() {
        let entry_number = (curve_idx as u16) + 1;

        match db.received_values(entry_number) {
            None => {
                comments.push(SunSpecComment {
                    uid: format!("{selector_uid:?}"),
                    index: format!("Curve {entry_number}"),
                    check: ConformanceCheck::CurveReadbackMatchesProfile,
                    value: "No entry".into(),
                    message: format!(
                        "Curve {entry_number} has no database entry (out of max range)"
                    ),
                    passed: false,
                });
            }
            Some(written) if written.is_empty() => {
                comments.push(SunSpecComment {
                    uid: format!("{selector_uid:?}"),
                    index: format!("Curve {entry_number}"),
                    check: ConformanceCheck::CurveReadbackMatchesProfile,
                    value: "No readback".into(),
                    message: format!("Curve {entry_number}: no readback received from outstation"),
                    passed: false,
                });
            }
            Some(written) => {
                let profile_values: Result<HashMap<PointIndex, PointValue>, ValidationErrors> =
                    curve
                        .iter_points()
                        .into_iter()
                        .map(|pt| {
                            let point = PointValue::Ai(AiValue::try_from(pt)?);
                            Ok((point.key(), point))
                        })
                        .collect();

                let profile_values = match profile_values {
                    Ok(v) => v,
                    Err(err) => {
                        comments.push(SunSpecComment {
                            uid: format!("{selector_uid:?}"),
                            index: format!("Curve {entry_number}"),
                            check: ConformanceCheck::CurveReadbackMatchesProfile,
                            value: String::new(),
                            message: format!("Curve {entry_number}: scaling error: {err}"),
                            passed: false,
                        });
                        continue;
                    }
                };

                for point_value in &written {
                    if let Some(expected) = profile_values.get(&point_value.key()) {
                        tracing::debug!(
                            "Curve {entry_number}: validating outstation readback {} = {}; expected profile value is {}",
                            point_value.key(),
                            point_value,
                            expected
                        );
                        if point_value != expected {
                            comments.push(SunSpecComment {
                                    uid: format!("{selector_uid:?}"),
                                    index: point_value.key().to_string(),
                                    check: ConformanceCheck::CurveReadbackMatchesProfile,
                                    value: point_value.to_string(),
                                    message: format!(
                                        "For a control station curve point, the control station wrote that value to the outstation, but the outstation read back a different value. Curve #{}, Point {}: expected {}, got {}",
                                        entry_number, point_value.key(), expected, point_value
                                    ),
                                    passed: false,
                                });
                        } else {
                            comments.push(SunSpecComment {
                                uid: format!("{selector_uid:?}"),
                                index: point_value.key().to_string(),
                                check: ConformanceCheck::CurveReadbackMatchesProfile,
                                value: point_value.to_string(),
                                message: format!("Curve {entry_number} {} readback matches profile value {expected}", point_value.key()),
                                passed: true,
                            });
                        }
                    }
                }
            }
        }
    }

    comments
}

/// Validate backward-compatible schedule readback state against the PICS profile.
fn validate_schedule_bc_readback(state: &ControlStationState) -> Vec<SunSpecComment> {
    let profile_guard = state.profile.read().unwrap();
    let profile = &*profile_guard;

    let selector_uid = format!("{:?}", AiUid::BC_Scheduling_BC_FSCC_Schd);
    let db = state.schedule_bc_database.read().unwrap();
    let mut comments = Vec::new();

    for (sched_idx, schedule) in profile.ai.schedules_bc.iter().enumerate() {
        let entry_number = (sched_idx as u16) + 1;

        match db.written_values(entry_number) {
            None => {
                comments.push(SunSpecComment {
                    uid: selector_uid.clone(),
                    index: format!("BC Schedule {entry_number}"),
                    check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                    value: "No entry".into(),
                    message: format!(
                        "BC Schedule {entry_number} has no database entry (out of max range)"
                    ),
                    passed: false,
                });
            }
            Some(written) if written.is_empty() => {
                comments.push(SunSpecComment {
                    uid: selector_uid.clone(),
                    index: format!("BC Schedule {entry_number}"),
                    check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                    value: "No readback".into(),
                    message: format!(
                        "BC Schedule {entry_number}: no readback received from outstation"
                    ),
                    passed: false,
                });
            }
            Some(written) => {
                let profile_values: std::collections::HashMap<PointIndex, PointValue> =
                    match schedule
                        .iter_points()
                        .into_iter()
                        .filter(|point| point.assoc_ao.is_some())
                        .map(|pt| -> Result<_, ValidationErrors> {
                            let point = PointValue::Ai(AiValue::try_from(pt)?);
                            Ok((point.key(), point))
                        })
                        .collect::<Result<_, _>>()
                    {
                        Ok(v) => v,
                        Err(err) => {
                            comments.push(SunSpecComment {
                                uid: selector_uid.clone(),
                                index: format!("BC Schedule {entry_number}"),
                                check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                                value: String::new(),
                                message: format!(
                                    "BC Schedule {entry_number}: scaling error: {err}"
                                ),
                                passed: false,
                            });
                            continue;
                        }
                    };

                for point_value in &written {
                    if let Some(expected) = profile_values.get(&point_value.key()) {
                        if point_value != expected {
                            comments.push(SunSpecComment {
                                    uid: selector_uid.clone(),
                                    index: point_value.key().to_string(),
                                    check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                                    value: point_value.to_string(),
                                    message: format!(
                                        "BC Schedule {entry_number} {} readback {} does not match profile value {}",
                                        point_value.key(), point_value, expected
                                    ),
                                    passed: false,
                                });
                        } else {
                            comments.push(SunSpecComment {
                                uid: selector_uid.clone(),
                                index: point_value.key().to_string(),
                                check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                                value: point_value.to_string(),
                                message: format!("BC Schedule {entry_number} {} readback matches profile value {expected}", point_value.key()),
                                passed: true,
                            });
                        }
                    }
                }
            }
        }
    }

    comments
}

/// Validate IEEE 1815.2 schedule readback state against the PICS profile.
fn validate_schedule_readback(state: &ControlStationState) -> Vec<SunSpecComment> {
    let profile_guard = state.profile.read().unwrap();
    let profile = &*profile_guard;

    let selector_uid = format!("{:?}", AiUid::Scheduling_FSCC_Schd);
    let db = state.schedule_database.read().unwrap();
    let mut comments = Vec::new();

    for (sched_idx, schedule) in profile.ai.schedules.iter().enumerate() {
        let entry_number = (sched_idx as u16) + 1;

        match db.written_values(entry_number) {
            None => {
                comments.push(SunSpecComment {
                    uid: selector_uid.clone(),
                    index: format!("Schedule {entry_number}"),
                    check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                    value: "No entry".into(),
                    message: format!(
                        "Schedule {entry_number} has no database entry (out of max range)"
                    ),
                    passed: false,
                });
            }
            Some(written) if written.is_empty() => {
                comments.push(SunSpecComment {
                    uid: selector_uid.clone(),
                    index: format!("Schedule {entry_number}"),
                    check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                    value: "No readback".into(),
                    message: format!(
                        "Schedule {entry_number}: no readback received from outstation"
                    ),
                    passed: false,
                });
            }
            Some(written) => {
                let profile_values: std::collections::HashMap<PointIndex, PointValue> =
                    match schedule
                        .iter_points()
                        .into_iter()
                        .filter(|point| point.assoc_ao.is_some())
                        .map(|pt| -> Result<_, ValidationErrors> {
                            let point = PointValue::Ai(AiValue::try_from(pt)?);
                            Ok((point.key(), point))
                        })
                        .collect::<Result<_, _>>()
                    {
                        Ok(v) => v,
                        Err(err) => {
                            comments.push(SunSpecComment {
                                uid: selector_uid.clone(),
                                index: format!("Schedule {entry_number}"),
                                check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                                value: String::new(),
                                message: format!("Schedule {entry_number}: scaling error: {err}"),
                                passed: false,
                            });
                            continue;
                        }
                    };

                for point_value in &written {
                    if let Some(expected) = profile_values.get(&point_value.key()) {
                        if point_value != expected {
                            comments.push(SunSpecComment {
                                    uid: selector_uid.clone(),
                                    index: point_value.key().to_string(),
                                    check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                                    value: point_value.to_string(),
                                    message: format!(
                                        "Schedule {entry_number} {} readback {} does not match profile value {}",
                                        point_value.key(), point_value, expected
                                    ),
                                    passed: false,
                                });
                        } else {
                            comments.push(SunSpecComment {
                                uid: selector_uid.clone(),
                                index: point_value.key().to_string(),
                                check: ConformanceCheck::ScheduleReadbackMatchesProfile,
                                value: point_value.to_string(),
                                message: format!("Schedule {entry_number} {} readback matches profile value {expected}", point_value.key()),
                                passed: true,
                            });
                        }
                    }
                }
            }
        }
    }

    comments
}

fn run_test(
    state: &ControlStationState,
    ai_uids: &[AiUid],
    bi_uids: &[BiUid],
    ao_uids: &[AoUid],
    bo_uids: &[BoUid],
) {
    let mut comments = Vec::new();
    comments.extend(ai_uids.iter().map(|&uid| validate_ai_received(state, uid)));
    comments.extend(bi_uids.iter().map(|&uid| validate_bi_received(state, uid)));
    comments.extend(ao_uids.iter().map(|&uid| validate_ao_sent(state, uid)));
    comments.extend(bo_uids.iter().map(|&uid| validate_bo_sent(state, uid)));
    comments.extend(
        ai_uids
            .iter()
            .filter_map(|&uid| validate_ai_in_range(state, uid)),
    );
    comments.extend(
        ao_uids
            .iter()
            .filter_map(|&uid| validate_ao_in_range(state, uid)),
    );
    log_conformance_result(comments.iter().all(|comment| comment.passed), comments);
}

// ---------------------------------------------------------------------------
// Per-mode tests
// ---------------------------------------------------------------------------

fn mon_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::mon_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn alarm_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::alarm_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn conn_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::conn_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn serv_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::serv_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn op_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::op_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

/// Reference Indication Test CURVE-001
/// PDF download for reference: https://restservice.epri.com/publicdownload/000000003002016144/0/Product
fn curve_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::curve_001();
    let mut comments = Vec::new();
    comments.extend(pts.bi.iter().map(|&uid| validate_bi_received(state, uid)));
    comments.extend(pts.ao.iter().map(|&uid| validate_ao_sent(state, uid)));
    comments.extend(validate_curves_from_outstation(state));
    log_conformance_result(comments.iter().all(|comment| comment.passed), comments);
}

fn curve_002(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::curve_002();
    let mut comments = Vec::new();
    comments.extend(pts.bi.iter().map(|&uid| validate_bi_received(state, uid)));
    comments.extend(pts.ao.iter().map(|&uid| validate_ao_sent(state, uid)));
    comments.extend(validate_curves_from_outstation(state));
    log_conformance_result(comments.iter().all(|comment| comment.passed), comments);
}

fn curve_003(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::curve_003();
    let mut comments = Vec::new();
    comments.extend(pts.bi.iter().map(|&uid| validate_bi_received(state, uid)));
    comments.extend(pts.ao.iter().map(|&uid| validate_ao_sent(state, uid)));
    comments.extend(validate_curves_from_outstation(state));
    log_conformance_result(comments.iter().all(|comment| comment.passed), comments);
}

fn vrt_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::vrt_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn frt_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::frt_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn fw_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::fw_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn drcs_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::drcs_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn dvw_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::dvw_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apl_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::apl_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn chg_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::chg_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn ccm_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::ccm_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apr1_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::apr1_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apr2_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::apr2_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apr3_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::apr3_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn agc_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::agc_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn aps_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::aps_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn vw_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::vw_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn fwc_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::fwc_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn cvar_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::cvar_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn fpf_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::fpf_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn vv_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::vv_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn wv_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::wv_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn pfc_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::pfc_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn psig_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::psig_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn sched_001(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::sched_001();
    let mut comments = Vec::new();
    comments.extend(pts.ai.iter().map(|&uid| validate_ai_received(state, uid)));
    comments.extend(pts.bi.iter().map(|&uid| validate_bi_received(state, uid)));
    comments.extend(pts.ao.iter().map(|&uid| validate_ao_sent(state, uid)));
    comments.extend(pts.bo.iter().map(|&uid| validate_bo_sent(state, uid)));
    comments.extend(validate_schedule_bc_readback(state));
    log_conformance_result(comments.iter().all(|comment| comment.passed), comments);
}

fn sched_002(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::sched_002();
    let mut comments = Vec::new();
    comments.extend(pts.ai.iter().map(|&uid| validate_ai_received(state, uid)));
    comments.extend(pts.bi.iter().map(|&uid| validate_bi_received(state, uid)));
    comments.extend(pts.ao.iter().map(|&uid| validate_ao_sent(state, uid)));
    comments.extend(pts.bo.iter().map(|&uid| validate_bo_sent(state, uid)));
    comments.extend(validate_schedule_readback(state));
    log_conformance_result(comments.iter().all(|comment| comment.passed), comments);
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

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
            $fn(state);
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex, RwLock};

    use common::conformance_testing::message_structure::TestEvent;
    use common::profile::values::TransmissionI32;
    use common::profile::{
        CurveDatabase, DatabasesConfig, PicsProfile, ProfileIndex, ScheduleBCDatabase,
        ScheduleDatabase,
    };
    use tracing::{
        Event, Subscriber,
        field::{Field, Visit},
    };
    use tracing_subscriber::{Layer, layer::Context, prelude::*};

    use super::*;

    struct EventCapture(Arc<Mutex<Vec<TestEvent>>>);

    impl<S: Subscriber> Layer<S> for EventCapture {
        fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
            struct EventVisitor<'a>(&'a Mutex<Vec<TestEvent>>);

            impl Visit for EventVisitor<'_> {
                fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                    if field.name() == "event_json" {
                        self.0.lock().expect("lock captured events").push(
                            serde_json::from_str(&format!("{value:?}"))
                                .expect("parse conformance event"),
                        );
                    }
                }
            }

            event.record(&mut EventVisitor(&self.0));
        }
    }

    fn test_state() -> ControlStationState {
        let profile = Arc::new(PicsProfile::load_full_profile());
        let config = DatabasesConfig::from_profile(&profile);
        ControlStationState {
            latest_transmission_values: RwLock::new(HashMap::new()),
            profile_index: RwLock::new(Arc::new(ProfileIndex::from_profile(&profile))),
            outstation_curve_database: RwLock::new(CurveDatabase::from_profile(
                &profile,
                config.max_curves,
                AiUid::Curve_DGSMn_InCrv,
            )),
            schedule_bc_database: RwLock::new(ScheduleBCDatabase::from_profile(
                &profile,
                config.max_schedules_bc,
                AiUid::BC_Scheduling_BC_FSCC_Schd,
            )),
            schedule_database: RwLock::new(ScheduleDatabase::from_profile(
                &profile,
                config.max_schedules,
                AiUid::Scheduling_FSCC_Schd,
            )),
            profile: RwLock::new(profile),
            sent_ao_indices: RwLock::new(HashSet::new()),
            sent_bo_indices: RwLock::new(HashSet::new()),
            ai_range_violations: RwLock::new(HashMap::new()),
            ao_range_violations: RwLock::new(HashMap::new()),
            scenario_id: Arc::new(Mutex::new(String::new())),
        }
    }

    #[test]
    fn point_checks_report_successes_and_failures() {
        let state = test_state();
        let ai = AiUid::Connect_DCTE3_WinTms;
        let bi = BiUid::Monitoring_CSWI_Pos_1;
        let ao = AoUid::Connect_DCTE3_WinTms;
        let bo = BoUid::Monitoring_CSWI_Pos;

        assert!(!validate_ai_received(&state, ai).passed);
        assert!(!validate_bi_received(&state, bi).passed);
        assert!(!validate_ao_sent(&state, ao).passed);
        assert!(!validate_bo_sent(&state, bo).passed);
        assert!(validate_ai_in_range(&state, ai).is_none());
        assert!(validate_ao_in_range(&state, ao).is_none());

        let initial_events = Arc::new(Mutex::new(Vec::new()));
        let subscriber =
            tracing_subscriber::registry().with(EventCapture(Arc::clone(&initial_events)));
        tracing::subscriber::with_default(subscriber, || {
            run_test(&state, &[ai], &[bi], &[ao], &[bo]);
        });
        let initial_events = initial_events.lock().expect("lock captured events");
        assert_eq!(initial_events.len(), 1);
        assert!(!initial_events[0].pass);
        assert_eq!(initial_events[0].comments.len(), 4);
        assert!(
            initial_events[0]
                .comments
                .iter()
                .all(|comment| comment.check != ConformanceCheck::WithinProfileRange)
        );

        state
            .latest_transmission_values
            .write()
            .expect("lock received values")
            .insert(
                format!("analog_input:{}", ai as u16),
                serde_json::Value::Null,
            );
        state
            .latest_transmission_values
            .write()
            .expect("lock received values")
            .insert(
                format!("binary_input:{}", bi as u16),
                serde_json::Value::Null,
            );
        state.record_ao_write(ao as u16);
        state.record_bo_write(bo as u16);
        assert!(validate_ao_in_range(&state, ao).is_none());
        state
            .latest_transmission_values
            .write()
            .expect("lock received values")
            .insert(
                format!("analog_output:{}", ao as u16),
                serde_json::Value::Null,
            );
        assert!(validate_ai_received(&state, ai).passed);
        assert!(validate_bi_received(&state, bi).passed);
        assert!(validate_ao_sent(&state, ao).passed);
        assert!(validate_bo_sent(&state, bo).passed);

        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(EventCapture(Arc::clone(&events)));
        tracing::subscriber::with_default(subscriber, || {
            run_test(&state, &[ai], &[bi], &[ao], &[bo]);
            state.record_ai_range_violation(
                ai as u16,
                TransmissionI32(11),
                TransmissionI32(0),
                TransmissionI32(10),
            );
            state.record_ao_range_violation(
                ao as u16,
                TransmissionI32(12),
                TransmissionI32(0),
                TransmissionI32(10),
            );
            run_test(&state, &[ai], &[bi], &[ao], &[bo]);
        });
        let events = events.lock().expect("lock captured events");
        assert_eq!(events.len(), 2);
        assert!(events[0].pass);
        assert_eq!(events[0].comments.len(), 6);
        assert!(events[0].comments.iter().all(|comment| comment.passed));
        assert_eq!(
            events[0]
                .comments
                .iter()
                .map(|comment| comment.check)
                .collect::<Vec<_>>(),
            vec![
                ConformanceCheck::ReceivedFromOutstation,
                ConformanceCheck::ReceivedFromOutstation,
                ConformanceCheck::WrittenToOutstation,
                ConformanceCheck::WrittenToOutstation,
                ConformanceCheck::WithinProfileRange,
                ConformanceCheck::WithinProfileRange,
            ]
        );
        let serialized =
            serde_json::to_value(&events[0].comments[0]).expect("serialize conformance comment");
        assert_eq!(serialized["check"], "received_from_outstation");
        assert_eq!(serialized["passed"], true);
        assert!(!events[1].pass);
        assert_eq!(
            events[1]
                .comments
                .iter()
                .filter(|comment| !comment.passed)
                .count(),
            2
        );
        assert_eq!(events[1].comments[4].value, "11");
        assert_eq!(events[1].comments[5].value, "12");
    }

    #[test]
    fn range_checks_require_points_in_the_profile() {
        let state = test_state();
        let ai = AiUid::Connect_DCTE3_WinTms;
        let ao = AoUid::Connect_DCTE3_WinTms;
        let profile = state.profile.read().expect("lock profile");
        let mut profile_index = ProfileIndex::from_profile(&profile);
        profile_index.ai_points.remove(&(ai as u16));
        profile_index.ao_points.remove(&(ao as u16));
        *state.profile_index.write().expect("lock profile index") = Arc::new(profile_index);
        drop(profile);

        let mut received = state
            .latest_transmission_values
            .write()
            .expect("lock received values");
        received.insert(
            format!("analog_input:{}", ai as u16),
            serde_json::Value::Null,
        );
        received.insert(
            format!("analog_output:{}", ao as u16),
            serde_json::Value::Null,
        );
        drop(received);

        assert!(validate_ai_in_range(&state, ai).is_none());
        assert!(validate_ao_in_range(&state, ao).is_none());
    }

    #[test]
    fn readback_checks_report_matches_and_mismatches() {
        let state = test_state();
        let profile = state.profile.read().expect("lock profile");
        let curve = AiValue::try_from(&profile.ai.curves[0].curve_type).expect("valid curve value");
        let schedule_bc = AiValue::try_from(
            profile.ai.schedules_bc[0]
                .iter_points()
                .into_iter()
                .find(|point| point.assoc_ao.is_some())
                .expect("BC schedule has a writable point"),
        )
        .expect("valid BC schedule value");
        let schedule = AiValue::try_from(
            profile.ai.schedules[0]
                .iter_points()
                .into_iter()
                .find(|point| point.assoc_ao.is_some())
                .expect("schedule has a writable point"),
        )
        .expect("valid schedule value");
        drop(profile);

        state
            .outstation_curve_database
            .write()
            .expect("lock curve database")
            .update_value(curve.index, curve.value);
        state
            .schedule_bc_database
            .write()
            .expect("lock BC schedule database")
            .update_value(schedule_bc.index, schedule_bc.value);
        state
            .schedule_database
            .write()
            .expect("lock schedule database")
            .update_value(schedule.index, schedule.value);

        for (comments, index, entry) in [
            (
                validate_curves_from_outstation(&state),
                format!("AI{}", curve.index),
                "Curve 1",
            ),
            (
                validate_schedule_bc_readback(&state),
                format!("AI{}", schedule_bc.index),
                "BC Schedule 1",
            ),
            (
                validate_schedule_readback(&state),
                format!("AI{}", schedule.index),
                "Schedule 1",
            ),
        ] {
            assert!(
                comments.iter().any(|comment| comment.index == index
                    && comment.passed
                    && comment.message.contains(entry)),
                "missing passing comment for {index}"
            );
        }

        state
            .outstation_curve_database
            .write()
            .expect("lock curve database")
            .update_value(curve.index, TransmissionI32(curve.value.0.wrapping_add(1)));
        state
            .schedule_bc_database
            .write()
            .expect("lock BC schedule database")
            .update_value(
                schedule_bc.index,
                TransmissionI32(schedule_bc.value.0.wrapping_add(1)),
            );
        state
            .schedule_database
            .write()
            .expect("lock schedule database")
            .update_value(
                schedule.index,
                TransmissionI32(schedule.value.0.wrapping_add(1)),
            );

        for (comments, index, entry) in [
            (
                validate_curves_from_outstation(&state),
                format!("AI{}", curve.index),
                "Curve #1",
            ),
            (
                validate_schedule_bc_readback(&state),
                format!("AI{}", schedule_bc.index),
                "BC Schedule 1",
            ),
            (
                validate_schedule_readback(&state),
                format!("AI{}", schedule.index),
                "Schedule 1",
            ),
        ] {
            assert!(
                comments.iter().any(|comment| comment.index == index
                    && !comment.passed
                    && comment.message.contains(entry)),
                "missing failing comment for {index}"
            );
        }
    }
}
