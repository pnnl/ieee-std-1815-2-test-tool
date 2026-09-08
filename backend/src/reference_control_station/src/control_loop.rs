//! Profile-driven control loop for the reference control station.
//!
//! Periodically compares AI readback values reported by the outstation against
//! the expected values in the loaded PicsProfile and issues corrective AO writes
//! for any mismatches.  Multiplexed structures (curves and schedules) are written
//! unconditionally each tick because checking them would require manipulating the
//! selector, which would disrupt the outstation's readback state.
//!
//! BO points are driven by their associated BI state: if a profile BI is not `true`,
//! the control loop sends a LATCH_ON to the paired BO.
//!
//! Conformance tests are run at the end of each control-loop iteration so that
//! results always reflect the latest writes and reads.  A `Notify` allows the
//! scenario orchestrator to trigger an immediate cycle after a `new_scenario`
//! command instead of waiting for the next periodic tick.

use std::sync::Arc;
use std::time::Duration;

use common::profile::values::TransmissionI32;
use dnp3::app::control::{CommandStatus, Group12Var1, Group41Var1, OpType};
use dnp3::master::{
    AssociationHandle, Classes, CommandBuilder, CommandMode, CommandSupport, ReadRequest,
};
use tokio::sync::Notify;

use common::uids::ao_uid::AoUid;

use crate::conformance::run_conformance_tests;
use crate::state::ControlStationState;

/// Spawn the periodic profile-driven control loop as a background task.
///
/// The task runs every `interval_secs` seconds and:
/// 1. Syncs plain AO points against AI readbacks.
/// 2. Syncs BO points by sending LATCH_ON when the paired BI is not true.
/// 3. Writes all profile curves unconditionally.
/// 4. Writes all profile backward-compatible schedules unconditionally.
/// 5. Writes all profile IEEE 1815.2 schedules unconditionally.
/// 6. Runs conformance tests so results always follow a complete write cycle.
///
/// The first iteration runs immediately on startup without waiting for the interval.
/// Calling `notify.notify_one()` triggers an extra immediate iteration, which the
/// scenario orchestrator does after each `new_scenario` command.
/// The loop exits silently when the channel or association is dropped.
pub(crate) fn update_outstation_or_test_forever(
    state: Arc<ControlStationState>,
    association: AssociationHandle,
    interval_seconds: u64,
    notify: Arc<Notify>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            write_output_values_to_outstation(&state, &association).await;

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(interval_seconds)) => {
                    write_output_values_to_outstation(&state, &association).await;
                }
                _ = notify.notified() => {
                    tracing::info!("Received control loop notification; running immediate iteration");
                    state.reset_conformance_state();
                    write_output_values_to_outstation(&state, &association).await;
                    run_conformance_tests(&state);
                }
            }
        }
    })
}

/// Execute one full control-loop iteration.
async fn write_output_values_to_outstation(
    state: &ControlStationState,
    association: &AssociationHandle,
) {
    tracing::info!("Writing output values to outstation according to profile");
    write_all_ao_to_outstation(state, association).await;
    write_bo_to_outstation(state, association).await;
    write_curves_and_poll(state, association).await;
    write_schedules_bc(state, association).await;
    write_schedules(state, association).await;
}

// ---------------------------------------------------------------------------
// Plain AO sync
// ---------------------------------------------------------------------------

/// For every AO in the profile whose associated AI is a base AI point with an expected
/// value, compare the current AI readback against that value and issue a corrective
/// write if they differ.
///
/// Curve and schedule AOs are intentionally skipped — they are managed by the dedicated
/// curve/schedule writers which set the mux selector before writing data points.
async fn write_all_ao_to_outstation(state: &ControlStationState, association: &AssociationHandle) {
    let profile_index = {
        let guard = state.profile_index.read().unwrap();
        Arc::clone(&guard)
    };

    // Collect (ao_index, expected_raw_value) pairs where the associated AI is a base
    // AI point (not a curve/schedule AI) and has a defined expected value.
    let writes: Vec<(u16, TransmissionI32)> = {
        let latest_input_values = state.latest_transmission_values.read().unwrap();

        profile_index
            .ao_points
            .keys()
            .filter_map(|&ao_index| {
                let ai_index = profile_index.ao_to_ai.get(&ao_index)?;

                // Skip AOs associated with curve/schedule AI points — those are
                // managed by write_curves / write_schedules_bc / write_schedules.
                if !profile_index.base_ai_indices.contains(ai_index) {
                    return None;
                }

                let ai_point = profile_index.ai_points.get(ai_index)?;
                let expected_transmission = match TransmissionI32::try_from_engineering(
                    ai_point.value(),
                    ai_point.multiplier(),
                    ai_point.offset,
                ) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(
                            "Skipping AO{ao_index} sync due to invalid engineering value for associated AI{ai_index}: {err}"
                        );
                        return None;
                    }
                };

                let key = format!("analog_input:{ai_index}");
                let needs_write = match latest_input_values.get(&key) {
                    Some(current) => {
                        TransmissionI32(current["value"].as_f64().unwrap_or_else(|| panic!("Invalid value for analog input: AI{ai_index}: {current:?}")) as i32) != expected_transmission
                    }
                    None => true, // not yet received — write the profile value
                };

                if needs_write {
                    Some((ao_index, expected_transmission))
                } else {
                    None
                }
            })
            .collect()
    };

    for (ao_index, value) in writes {
        write_ao(state, association, ao_index, value).await;
    }
}

// ---------------------------------------------------------------------------
// BO sync from BI state
// ---------------------------------------------------------------------------

/// For every BI in the profile that is currently `false` (or not yet received),
/// send a LATCH_ON command to its paired BO to drive the outstation to the expected state.
async fn write_bo_to_outstation(state: &ControlStationState, association: &AssociationHandle) {
    let profile_index = {
        let guard = state.profile_index.read().unwrap();
        Arc::clone(&guard)
    };

    let bo_writes: Vec<u16> = {
        let latest = state.latest_transmission_values.read().unwrap();

        profile_index
            .bi_points
            .keys()
            .filter_map(|&bi_index| {
                let bo_index = profile_index.bi_to_bo.get(&bi_index)?;

                let key = format!("binary_input:{bi_index}");
                let is_true = latest
                    .get(&key)
                    .and_then(|v| v["value"].as_bool())
                    .unwrap_or(false);

                if is_true { None } else { Some(*bo_index) }
            })
            .collect()
    };

    for bo_index in bo_writes {
        write_bo(state, association, bo_index).await;
    }
}

// ---------------------------------------------------------------------------
// Curve writer
// ---------------------------------------------------------------------------

/// Write all curves defined in the profile to the outstation unconditionally.
///
/// For each curve (1-based), the function:
/// 1. Writes the curve-edit-selector AO with the curve number.
/// 2. Writes each curve data point (curve_type, number_of_points, x/y units, x/y values)
///    for which the profile provides a value.
async fn write_curves_and_poll(state: &ControlStationState, association: &AssociationHandle) {
    let (profile_index, profile) = {
        let pi_guard = state.profile_index.read().unwrap();
        let p_guard = state.profile.read().unwrap();
        (Arc::clone(&pi_guard), Arc::clone(&p_guard))
    };

    let selector_ao_index = AoUid::Curve_DGSMn_InCrv as u16;
    if !profile_index.ao_points.contains_key(&selector_ao_index) {
        tracing::debug!("No curve selector AO found in profile; skipping curve writes");
        return;
    }

    for (curve_number, curve) in profile.ai.curves.iter().enumerate() {
        let curve_number = (curve_number as i16) + 1;
        let selector_ai_index = common::uids::ai_uid::AiUid::Curve_DGSMn_InCrv as u16;

        // Select the curve to edit.
        tracing::debug!(
            "Curve {curve_number}: sending selector AO{selector_ao_index} = {curve_number}; expected selector readback is AI{selector_ai_index}"
        );
        write_ao(
            state,
            association,
            selector_ao_index,
            TransmissionI32(curve_number.into()),
        )
        .await;

        // Write each data point for this curve.
        for ai_point in curve.iter_points() {
            let transmission_value = match TransmissionI32::try_from_engineering(
                ai_point.value(),
                ai_point.multiplier(),
                ai_point.offset,
            ) {
                Ok(value) => value,
                Err(err) => {
                    tracing::warn!(
                        "Skipping write of curve AI point with invalid engineering value: {:?}, error: {err}",
                        ai_point
                    );
                    continue;
                }
            };
            if transmission_value.0 == i32::MIN {
                tracing::warn!(
                    "Control loop: skipping write of curve AI point with DNP3 'not applicable' value: {:?}",
                    ai_point
                );
                continue;
            }
            if let Some(&ao_index) = profile_index.ai_to_ao.get(&ai_point.point_index) {
                tracing::debug!(
                    "Curve {curve_number}: sending profile point AI{} = {} through associated AO{ao_index}",
                    ai_point.point_index,
                    transmission_value.0
                );
                write_ao(state, association, ao_index, transmission_value).await;
            }
        }

        // Awaiting the read before advancing to the next curve ensures the response
        // arrives while the selector is still at curve_number.
        state
            .outstation_curve_database
            .write()
            .unwrap()
            .set_active_entry(curve_number as u16);

        // arrives while the selector is still at curve_number. The selector AI in the
        // response determines which local readback entry receives the curve data.
        tracing::debug!(
            "Curve {curve_number}: starting class-0 scan for selector AI{selector_ai_index} and curve point readback"
        );
        match association
            .clone()
            .read(ReadRequest::class_scan(Classes::class0()))
            .await
        {
            Ok(()) => tracing::debug!(
                "Curve {curve_number}: class-0 scan completed for curve point readback"
            ),
            Err(err) => {
                tracing::warn!(
                    "Control loop: readback poll for curve {curve_number} failed: {err}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Schedule (backward-compatible) writer
// ---------------------------------------------------------------------------

/// Write all backward-compatible schedules defined in the profile unconditionally.
async fn write_schedules_bc(state: &ControlStationState, association: &AssociationHandle) {
    let (profile_index, profile) = {
        let pi_guard = state.profile_index.read().unwrap();
        let p_guard = state.profile.read().unwrap();
        (Arc::clone(&pi_guard), Arc::clone(&p_guard))
    };

    let selector_ao_index = AoUid::BC_Scheduling_BC_FSCC_Schd as u16;
    if !profile_index.ao_points.contains_key(&selector_ao_index) {
        tracing::debug!("No schedule-BC selector AO found in profile; skipping BC schedule writes");
        return;
    }

    for (schedule_number, schedule) in profile.ai.schedules_bc.iter().enumerate() {
        let schedule_number = TransmissionI32((schedule_number + 1) as i32);

        write_ao(state, association, selector_ao_index, schedule_number).await;

        for ai_point in schedule.iter_points() {
            if let Some(&ao_index) = profile_index.ai_to_ao.get(&ai_point.point_index) {
                let transmission_value = match TransmissionI32::try_from_engineering(
                    ai_point.value(),
                    ai_point.multiplier(),
                    ai_point.offset,
                ) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(
                            "Skipping write of schedule BC AI point with invalid engineering value: {:?}, error: {err}",
                            ai_point
                        );
                        continue;
                    }
                };
                write_ao(state, association, ao_index, transmission_value).await;
            }
        }

        state
            .schedule_bc_database
            .write()
            .unwrap()
            .set_active_entry(schedule_number.0 as u16);
        if let Err(err) = association
            .clone()
            .read(ReadRequest::class_scan(Classes::class0()))
            .await
        {
            tracing::warn!(
                "Control loop: readback poll for BC schedule {schedule_number} failed: {err}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Schedule (IEEE 1815.2) writer
// ---------------------------------------------------------------------------

/// Write all IEEE 1815.2 schedules defined in the profile unconditionally.
async fn write_schedules(state: &ControlStationState, association: &AssociationHandle) {
    let (profile_index, profile) = {
        let pi_guard = state.profile_index.read().unwrap();
        let p_guard = state.profile.read().unwrap();
        (Arc::clone(&pi_guard), Arc::clone(&p_guard))
    };

    let selector_ao_index = AoUid::Scheduling_FSCC_Schd as u16;
    if !profile_index.ao_points.contains_key(&selector_ao_index) {
        tracing::debug!("No schedule selector AO found in profile; skipping schedule writes");
        return;
    }

    for (schedule_number, schedule) in profile.ai.schedules.iter().enumerate() {
        let schedule_number = (schedule_number as i16) + 1;

        write_ao(
            state,
            association,
            selector_ao_index,
            schedule_number.into(),
        )
        .await;

        for ai_point in schedule.iter_points() {
            let transmission_value = match TransmissionI32::try_from_engineering(
                ai_point.value(),
                ai_point.multiplier(),
                ai_point.offset,
            ) {
                Ok(value) => value,
                Err(err) => {
                    tracing::warn!(
                        "Skipping write of schedule AI point with invalid engineering value: {:?}, error: {err}",
                        ai_point
                    );
                    continue;
                }
            };

            if transmission_value.0 == i32::MIN {
                tracing::warn!(
                    "Control loop: skipping write of schedule AI point with DNP3 'not applicable' value: {:?}",
                    ai_point
                );
                continue;
            }

            if let Some(&ao_index) = profile_index.ai_to_ao.get(&ai_point.point_index) {
                write_ao(state, association, ao_index, transmission_value).await;
            }
        }

        state
            .schedule_database
            .write()
            .unwrap()
            .set_active_entry(schedule_number as u16);
        if let Err(err) = association
            .clone()
            .read(ReadRequest::class_scan(Classes::class0()))
            .await
        {
            tracing::warn!(
                "Control loop: readback poll for schedule {schedule_number} failed: {err}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Issue a single `Group41Var1` DirectOperate write to `ao_index` with `value`.
///
/// Records the write on success.  Non-success responses and transport errors are
/// logged as warnings but do not abort the caller.
async fn write_ao(
    state: &ControlStationState,
    association: &AssociationHandle,
    ao_index: u16,
    value: TransmissionI32,
) {
    // This must be variation 1 to support 32-bit values, e.g. AO1
    let headers = <CommandBuilder as CommandSupport<Group41Var1>>::single_header_u16(
        Group41Var1 {
            value: value.0,
            status: CommandStatus::Success,
        },
        ao_index,
    );

    match association
        .clone()
        .operate(CommandMode::DirectOperate, headers)
        .await
    {
        Ok(()) => {
            state.record_ao_write(ao_index);
            tracing::debug!("Control loop: wrote AO{ao_index} = {value} to the outstation");
        }
        Err(err) => {
            tracing::error!("Control loop: AO{ao_index} write of value {value} failed: {err}");
        }
    }
}

/// Issue a LATCH_ON `Group12Var1` DirectOperate command to `bo_index`.
///
/// Records the write on success. Non-success responses and transport errors are
/// logged as warnings but do not abort the caller.
async fn write_bo(state: &ControlStationState, association: &AssociationHandle, bo_index: u16) {
    let headers = <CommandBuilder as CommandSupport<Group12Var1>>::single_header_u16(
        Group12Var1::from_op_type(OpType::LatchOn),
        bo_index,
    );

    match association
        .clone()
        .operate(CommandMode::DirectOperate, headers)
        .await
    {
        Ok(()) => {
            state.record_bo_write(bo_index);
            tracing::debug!("Control loop: sent LATCH_ON to BO{bo_index}");
        }
        Err(err) => {
            tracing::warn!("Control loop: BO{bo_index} LATCH_ON failed: {err}");
        }
    }
}
