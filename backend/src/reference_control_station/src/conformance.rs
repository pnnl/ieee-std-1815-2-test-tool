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

use common::profile::values::TransmissionI32;
use common::uids::{ai_uid::AiUid, ao_uid::AoUid, bi_uid::BiUid, bo_uid::BoUid};
use common::{
    conformance_testing::{
        logging::log_conformance_result,
        message_structure::{SunSpecComment, TestName},
    },
    profile::validation::ValidationErrors,
};
use tracing::{Level, span};

use crate::state::ControlStationState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a comment if the AI point at `uid` has **not been received** from the outstation.
fn validate_ai_received(state: &ControlStationState, uid: AiUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    let key = format!("analog_input:{index}");
    if state
        .latest_transmission_values
        .read()
        .unwrap()
        .contains_key(&key)
    {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AI{index}"),
        value: "Not received".into(),
        issue: format!("AI{index} has not been received from the outstation"),
    })
}

/// Returns a comment if the BI point at `uid` has **not been received** from the outstation.
fn validate_bi_received(state: &ControlStationState, uid: BiUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    let key = format!("binary_input:{index}");
    if state
        .latest_transmission_values
        .read()
        .unwrap()
        .contains_key(&key)
    {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("BI{index}"),
        value: "Not received".into(),
        issue: format!("BI{index} has not been received from the outstation"),
    })
}

/// Returns a comment if the AO point at `uid` has **not been sent** to the outstation.
fn validate_ao_sent(state: &ControlStationState, uid: AoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if state.sent_ao_indices.read().unwrap().contains(&index) {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AO{index}"),
        value: "Not sent".into(),
        issue: format!("AO{index} has not been written to the outstation"),
    })
}

/// Returns a comment if the most recent AI readback value for `uid` was out of range.
fn validate_ai_in_range(state: &ControlStationState, uid: AiUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    let (value, min, max) = *state.ai_range_violations.read().unwrap().get(&index)?;
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AI{index}"),
        value: format!("{value}"),
        issue: format!("AI{index} value {value} is out of the profile range [{min}, {max}]"),
    })
}

/// Returns a comment if the most recent AO status readback value for `uid` was out of range.
fn validate_ao_in_range(state: &ControlStationState, uid: AoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    let (value, min, max) = *state.ao_range_violations.read().unwrap().get(&index)?;
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AO{index}"),
        value: format!("{value}"),
        issue: format!("AO{index} value {value} is out of the profile range [{min}, {max}]"),
    })
}

/// Returns a comment if the BO point at `uid` has **not been sent** to the outstation.
fn validate_bo_sent(state: &ControlStationState, uid: BoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if state.sent_bo_indices.read().unwrap().contains(&index) {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("BO{index}"),
        value: "Not sent".into(),
        issue: format!("BO{index} has not been written to the outstation"),
    })
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
                    value: "No entry".into(),
                    issue: format!("Curve {entry_number} has no database entry (out of max range)"),
                });
            }
            Some(written) if written.is_empty() => {
                comments.push(SunSpecComment {
                    uid: format!("{selector_uid:?}"),
                    index: format!("Curve {entry_number}"),
                    value: "No readback".into(),
                    issue: format!("Curve {entry_number}: no readback received from outstation"),
                });
            }
            Some(written) => {
                let profile_values: Result<HashMap<u16, TransmissionI32>, ValidationErrors> = curve
                    .iter_points()
                    .into_iter()
                    .map(|pt| {
                        Ok((
                            pt.point_index,
                            TransmissionI32::try_from_engineering(
                                pt.value(),
                                pt.multiplier(),
                                pt.offset,
                            )?,
                        ))
                    })
                    .collect();

                let profile_values = match profile_values {
                    Ok(v) => v,
                    Err(err) => {
                        comments.push(SunSpecComment {
                            uid: format!("{selector_uid:?}"),
                            index: format!("Curve {entry_number}"),
                            value: String::new(),
                            issue: format!("Curve {entry_number}: scaling error: {err}"),
                        });
                        continue;
                    }
                };

                for ai_value in &written {
                    if let Some(&expected) = profile_values.get(&ai_value.index) {
                        tracing::debug!(
                            "Curve {entry_number}: validating outstation readback AI{} = {}; expected profile value is {}",
                            ai_value.index,
                            ai_value.value.0,
                            expected.0
                        );
                        if ai_value.value != expected {
                            comments.push(SunSpecComment {
                                    uid: format!("{selector_uid:?}"),
                                    index: format!("Curve {entry_number} AI{}", ai_value.index),
                                    value: format!("{}", ai_value.value.0),
                                    issue: format!(
                                        "For a control station curve point, the control station wrote that value to the outstation, but the outstation read back a different value. Curve #{}, Point AI{}: expected {}, got {}",
                                        entry_number, ai_value.index, expected.0, ai_value.value.0
                                    ),
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
                    value: "No entry".into(),
                    issue: format!(
                        "BC Schedule {entry_number} has no database entry (out of max range)"
                    ),
                });
            }
            Some(written) if written.is_empty() => {
                comments.push(SunSpecComment {
                    uid: selector_uid.clone(),
                    index: format!("BC Schedule {entry_number}"),
                    value: "No readback".into(),
                    issue: format!(
                        "BC Schedule {entry_number}: no readback received from outstation"
                    ),
                });
            }
            Some(written) => {
                let profile_values: std::collections::HashMap<u16, TransmissionI32> = match schedule
                    .iter_points()
                    .into_iter()
                    .filter(|point| point.assoc_ao.is_some())
                    .map(|pt| -> Result<_, ValidationErrors> {
                        Ok((
                            pt.point_index,
                            TransmissionI32::try_from_engineering(
                                pt.value(),
                                pt.multiplier(),
                                pt.offset,
                            )?,
                        ))
                    })
                    .collect::<Result<_, _>>()
                {
                    Ok(v) => v,
                    Err(err) => {
                        comments.push(SunSpecComment {
                            uid: selector_uid.clone(),
                            index: format!("BC Schedule {entry_number}"),
                            value: String::new(),
                            issue: format!("BC Schedule {entry_number}: scaling error: {err}"),
                        });
                        continue;
                    }
                };

                for ai_value in &written {
                    if let Some(&expected) = profile_values.get(&ai_value.index) {
                        if ai_value.value != expected {
                            comments.push(SunSpecComment {
                                    uid: selector_uid.clone(),
                                    index: format!("BC Schedule {entry_number} AI{}", ai_value.index),
                                    value: format!("{}", ai_value.value.0),
                                    issue: format!(
                                        "BC Schedule {entry_number} AI{} readback {} does not match profile value {}",
                                        ai_value.index, ai_value.value.0, expected.0
                                    ),
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
                    value: "No entry".into(),
                    issue: format!(
                        "Schedule {entry_number} has no database entry (out of max range)"
                    ),
                });
            }
            Some(written) if written.is_empty() => {
                comments.push(SunSpecComment {
                    uid: selector_uid.clone(),
                    index: format!("Schedule {entry_number}"),
                    value: "No readback".into(),
                    issue: format!("Schedule {entry_number}: no readback received from outstation"),
                });
            }
            Some(written) => {
                let profile_values: std::collections::HashMap<u16, TransmissionI32> = match schedule
                    .iter_points()
                    .into_iter()
                    .filter(|point| point.assoc_ao.is_some())
                    .map(|pt| -> Result<_, ValidationErrors> {
                        Ok((
                            pt.point_index,
                            TransmissionI32::try_from_engineering(
                                pt.value(),
                                pt.multiplier(),
                                pt.offset,
                            )?,
                        ))
                    })
                    .collect::<Result<_, _>>()
                {
                    Ok(v) => v,
                    Err(err) => {
                        comments.push(SunSpecComment {
                            uid: selector_uid.clone(),
                            index: format!("Schedule {entry_number}"),
                            value: String::new(),
                            issue: format!("Schedule {entry_number}: scaling error: {err}"),
                        });
                        continue;
                    }
                };

                for ai_value in &written {
                    if let Some(&expected) = profile_values.get(&ai_value.index) {
                        if ai_value.value != expected {
                            comments.push(SunSpecComment {
                                    uid: selector_uid.clone(),
                                    index: format!("Schedule {entry_number} AI{}", ai_value.index),
                                    value: format!("{}", ai_value.value.0),
                                    issue: format!(
                                        "Schedule {entry_number} AI{} readback {} does not match profile value {}",
                                        ai_value.index, ai_value.value.0, expected.0
                                    ),
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
    comments.extend(
        ai_uids
            .iter()
            .filter_map(|&uid| validate_ai_received(state, uid)),
    );
    comments.extend(
        bi_uids
            .iter()
            .filter_map(|&uid| validate_bi_received(state, uid)),
    );
    comments.extend(
        ao_uids
            .iter()
            .filter_map(|&uid| validate_ao_sent(state, uid)),
    );
    comments.extend(
        bo_uids
            .iter()
            .filter_map(|&uid| validate_bo_sent(state, uid)),
    );
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
    log_conformance_result(comments.is_empty(), comments);
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
    comments.extend(
        pts.bi
            .iter()
            .filter_map(|&uid| validate_bi_received(state, uid)),
    );
    comments.extend(
        pts.ao
            .iter()
            .filter_map(|&uid| validate_ao_sent(state, uid)),
    );
    comments.extend(validate_curves_from_outstation(state));
    log_conformance_result(comments.is_empty(), comments);
}

fn curve_002(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::curve_002();
    let mut comments = Vec::new();
    comments.extend(
        pts.bi
            .iter()
            .filter_map(|&uid| validate_bi_received(state, uid)),
    );
    comments.extend(
        pts.ao
            .iter()
            .filter_map(|&uid| validate_ao_sent(state, uid)),
    );
    comments.extend(validate_curves_from_outstation(state));
    log_conformance_result(comments.is_empty(), comments);
}

fn curve_003(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::curve_003();
    let mut comments = Vec::new();
    comments.extend(
        pts.bi
            .iter()
            .filter_map(|&uid| validate_bi_received(state, uid)),
    );
    comments.extend(
        pts.ao
            .iter()
            .filter_map(|&uid| validate_ao_sent(state, uid)),
    );
    comments.extend(validate_curves_from_outstation(state));
    log_conformance_result(comments.is_empty(), comments);
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
    comments.extend(
        pts.ai
            .iter()
            .filter_map(|&uid| validate_ai_received(state, uid)),
    );
    comments.extend(
        pts.bi
            .iter()
            .filter_map(|&uid| validate_bi_received(state, uid)),
    );
    comments.extend(
        pts.ao
            .iter()
            .filter_map(|&uid| validate_ao_sent(state, uid)),
    );
    comments.extend(
        pts.bo
            .iter()
            .filter_map(|&uid| validate_bo_sent(state, uid)),
    );
    comments.extend(validate_schedule_bc_readback(state));
    log_conformance_result(comments.is_empty(), comments);
}

fn sched_002(state: &ControlStationState) {
    let pts = common::conformance_testing::point_lists::sched_002();
    let mut comments = Vec::new();
    comments.extend(
        pts.ai
            .iter()
            .filter_map(|&uid| validate_ai_received(state, uid)),
    );
    comments.extend(
        pts.bi
            .iter()
            .filter_map(|&uid| validate_bi_received(state, uid)),
    );
    comments.extend(
        pts.ao
            .iter()
            .filter_map(|&uid| validate_ao_sent(state, uid)),
    );
    comments.extend(
        pts.bo
            .iter()
            .filter_map(|&uid| validate_bo_sent(state, uid)),
    );
    comments.extend(validate_schedule_readback(state));
    log_conformance_result(comments.is_empty(), comments);
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
