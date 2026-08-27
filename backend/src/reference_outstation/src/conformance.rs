//! Semantic conformance tests for the reference outstation.
//!
//! Each test checks that the required DNP3 points for a control mode have been interacted
//! with — specifically:
//! - **AI / BI points**: the point exists in the loaded PicsProfile (the outstation supports it).
//! - **AO / BO points**: the control station has written to the point at least once.
//!
//! Results are logged via [`common::conformance_testing::logging::log_conformance_result`] and
//! captured by the [`common::conformance_testing::logging::ConformanceTrackingLayer`] that is
//! wired into the tracing subscriber in `main.rs`.

use std::sync::Arc;

use common::conformance_testing::{
    logging::log_conformance_result,
    message_structure::{SunSpecComment, TestName},
};
use common::uids::{ai_uid::AiUid, ao_uid::AoUid, bi_uid::BiUid, bo_uid::BoUid};
use tracing::{Level, span};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a comment if the AI point at `uid` is **not present** in the profile.
fn validate_ai_present(state: &AppState, uid: AiUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if state
        .profile_index
        .read()
        .unwrap()
        .ai_points
        .contains_key(&index)
    {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AI{index}"),
        value: "Not present".into(),
        issue: format!("AI{index} is not defined in the loaded PicsProfile"),
    })
}

/// Returns a comment if the BI point at `uid` is **not present** in the profile.
fn validate_bi_present(state: &AppState, uid: BiUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if state
        .profile_index
        .read()
        .unwrap()
        .bi_points
        .contains_key(&index)
    {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("BI{index}"),
        value: "Not present".into(),
        issue: format!("BI{index} is not defined in the loaded PicsProfile"),
    })
}

/// Returns a comment if the AO point at `uid` has **not been written** by the control station.
fn validate_ao_written(state: &AppState, uid: AoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if state.received_ao_indices.read().unwrap().contains(&index) {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AO{index}"),
        value: "Not written".into(),
        issue: format!("AO{index} has not been written by the control station"),
    })
}

/// Returns a comment if the most recent AO command value for `uid` was out of the profile range.
fn validate_ao_in_range(state: &AppState, uid: AoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    let (value, min, max) = *state.ao_range_violations.read().unwrap().get(&index)?;
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("AO{index}"),
        value: format!("{value}"),
        issue: format!(
            "AO{index} command value {value} is out of the profile range [{min}, {max}]"
        ),
    })
}

/// Returns a comment if the BO point at `uid` has **not been written** by the control station.
fn validate_bo_written(state: &AppState, uid: BoUid) -> Option<SunSpecComment> {
    let index = uid as u16;
    if state.received_bo_indices.read().unwrap().contains(&index) {
        return None;
    }
    Some(SunSpecComment {
        uid: format!("{uid:?}"),
        index: format!("BO{index}"),
        value: "Not written".into(),
        issue: format!("BO{index} has not been written by the control station"),
    })
}

fn run_test(
    state: &AppState,
    ai_uids: &[AiUid],
    bi_uids: &[BiUid],
    ao_uids: &[AoUid],
    bo_uids: &[BoUid],
) {
    let mut comments = Vec::new();
    comments.extend(
        ai_uids
            .iter()
            .filter_map(|&uid| validate_ai_present(state, uid)),
    );
    comments.extend(
        bi_uids
            .iter()
            .filter_map(|&uid| validate_bi_present(state, uid)),
    );
    comments.extend(
        ao_uids
            .iter()
            .filter_map(|&uid| validate_ao_written(state, uid)),
    );
    comments.extend(
        bo_uids
            .iter()
            .filter_map(|&uid| validate_bo_written(state, uid)),
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

fn mon_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::mon_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn alarm_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::alarm_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn conn_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::conn_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn serv_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::serv_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn op_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::op_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn curve_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::curve_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn curve_002(state: &AppState) {
    let pts = common::conformance_testing::point_lists::curve_002();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn curve_003(state: &AppState) {
    let pts = common::conformance_testing::point_lists::curve_003();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn vrt_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::vrt_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn frt_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::frt_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn fw_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::fw_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn drcs_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::drcs_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn dvw_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::dvw_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apl_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::apl_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn chg_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::chg_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn ccm_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::ccm_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apr1_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::apr1_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apr2_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::apr2_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn apr3_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::apr3_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn agc_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::agc_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn aps_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::aps_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn vw_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::vw_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn fwc_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::fwc_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn cvar_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::cvar_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn fpf_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::fpf_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn vv_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::vv_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn wv_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::wv_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn pfc_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::pfc_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn psig_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::psig_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn sched_001(state: &AppState) {
    let pts = common::conformance_testing::point_lists::sched_001();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

fn sched_002(state: &AppState) {
    let pts = common::conformance_testing::point_lists::sched_002();
    run_test(state, pts.ai, pts.bi, pts.ao, pts.bo);
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

/// Run all conformance tests against the current outstation state.
///
/// Each test is wrapped in a tracing span whose `test_name` field is captured
/// by the [`ConformanceTrackingLayer`][common::conformance_testing::logging::ConformanceTrackingLayer]
/// to associate logged events with the correct [`TestName`].
pub fn run_conformance_tests(state: &Arc<AppState>) {
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
