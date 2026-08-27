//! OutstationApplication, OutstationInformation, and ControlHandler implementations.

use std::sync::Arc;
use std::time::Duration;

use dnp3::app::Timestamp;
use dnp3::app::control::*;
use dnp3::app::measurement::AnalogInput as StepFuncAnalogInput;
use dnp3::app::measurement::AnalogOutputStatus;
use dnp3::app::measurement::BinaryInput;
use dnp3::app::measurement::BinaryOutputStatus;
use dnp3::app::measurement::Counter;
use dnp3::app::measurement::Flags;
use dnp3::app::measurement::{FrozenCounter, Time};
use dnp3::outstation::database::*;
use dnp3::outstation::*;

use crate::config::ControlBehavior;
use crate::state::AppState;
use common::profile::DatabaseEntry;
use common::profile::values::TransmissionI32;

/// Reference outstation application implementation.
pub(crate) struct RefOutstationApplication {
    accept_time_writes: bool,
    time_synced: bool,
}

impl RefOutstationApplication {
    /// Create a new application instance.
    pub fn new(accept_time_writes: bool) -> Self {
        Self {
            accept_time_writes,
            time_synced: false,
        }
    }
}

impl OutstationApplication for RefOutstationApplication {
    /// Handle a time write from the control station.
    ///
    /// **DNP3 spec: Group 50 – Variation 1 (Absolute Time)**
    /// - Request: FC 2 (write), Qualifier 07 (qty = 1)
    ///
    /// Accepts the write if `accept_time_writes` is configured, then clears the
    /// `NEED_TIME` IIN bit. Returns `NotSupported` when time sync is disabled.
    fn write_absolute_time(&mut self, time: Timestamp) -> Result<(), RequestError> {
        if self.accept_time_writes {
            tracing::info!("received time write: {}", time.raw_value());
            self.time_synced = true;
            Ok(())
        } else {
            Err(RequestError::NotSupported)
        }
    }

    fn get_application_iin(&self) -> ApplicationIin {
        ApplicationIin {
            need_time: !self.time_synced,
            ..Default::default()
        }
    }

    /// Handle a freeze-counter request from the control station.
    ///
    /// **DNP3 spec: Group 20 – Counter Objects (freeze variants)**
    /// - FC 7 (freeze), FC 8 (freeze, no ack), FC 9 (freeze and clear), FC 10 (freeze and clear, no ack)
    /// - Frozen values are reported in Group 21 – Frozen Counter Objects (Var 1: 32-bit with flag, Var 9: 32-bit without flag)
    ///
    /// This base implementation returns `NotSupported`.  The [`FreezeOutstationApplication`]
    /// wrapper overrides this to perform the actual counter-to-frozen-counter copy.
    fn freeze_counter(
        &mut self,
        indices: FreezeIndices,
        freeze_type: FreezeType,
        database: &mut DatabaseHandle,
    ) -> Result<(), RequestError> {
        let _ = (indices, freeze_type, database);
        Err(RequestError::NotSupported)
    }
}

/// Reference outstation information handler (no-op).
pub(crate) struct RefOutstationInformation;
impl OutstationInformation for RefOutstationInformation {}

/// Outstation application wrapper that handles freeze with configurable mappings.
pub(crate) struct FreezeOutstationApplication {
    inner: RefOutstationApplication,
    state: Arc<AppState>,
}

impl FreezeOutstationApplication {
    /// Create a new wrapper.
    pub fn new(accept_time_writes: bool, state: Arc<AppState>) -> Self {
        Self {
            inner: RefOutstationApplication::new(accept_time_writes),
            state,
        }
    }
}

impl OutstationApplication for FreezeOutstationApplication {
    fn write_absolute_time(&mut self, time: Timestamp) -> Result<(), RequestError> {
        self.inner.write_absolute_time(time)
    }

    fn get_application_iin(&self) -> ApplicationIin {
        self.inner.get_application_iin()
    }

    /// Handle a freeze-counter request with configurable counter-to-frozen-counter mappings.
    ///
    /// **DNP3 spec: Group 20 – Counter Objects (freeze variants)**
    /// - FC 7 (freeze), FC 8 (freeze, no ack), FC 9 (freeze and clear), FC 10 (freeze and clear, no ack)
    /// - Frozen values are reported in Group 21 – Frozen Counter Objects
    ///   (Var 1: 32-bit with flag; Var 9: 32-bit without flag; response: FC 129, Qualifier 00 or 01)
    ///
    /// Copies each counter's current value to its mapped frozen-counter index.
    /// When `FreezeType::FreezeAndClear`, the source counter is reset to 0 after copying.
    fn freeze_counter(
        &mut self,
        indices: FreezeIndices,
        freeze_type: FreezeType,
        database: &mut DatabaseHandle,
    ) -> Result<(), RequestError> {
        let clear = matches!(freeze_type, FreezeType::FreezeAndClear);

        let range = match indices {
            FreezeIndices::All => 0..self.state.counter_count,
            FreezeIndices::Range(start, stop) => start..(stop + 1), // DNP3 range end is inclusive
        };

        database.transaction(|db| {
            for counter_idx in range.clone() {
                if let Some(counter_val) = <Database as Get<Counter>>::get(db, counter_idx) {
                    let frozen_idx = self.state.get_freeze_target(counter_idx);
                    if frozen_idx < self.state.frozen_counter_count {
                        let frozen = FrozenCounter::new(
                            counter_val.value,
                            Flags::ONLINE,
                            get_current_time(),
                        );
                        db.update(frozen_idx, &frozen, UpdateOptions::detect_event());

                        if clear {
                            let zeroed = Counter::new(0, Flags::ONLINE, get_current_time());
                            db.update(counter_idx, &zeroed, UpdateOptions::detect_event());
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

/// Control handler with configurable per-index behaviors.
pub(crate) struct RefControlHandler {
    state: Arc<AppState>,
}

impl RefControlHandler {
    /// Create a new control handler.
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Map a ControlBehavior to a CommandStatus for non-success cases.
    fn behavior_to_status(behavior: ControlBehavior) -> CommandStatus {
        match behavior {
            ControlBehavior::Success => CommandStatus::Success,
            ControlBehavior::NotSupported | ControlBehavior::Timeout => CommandStatus::NotSupported,
            ControlBehavior::Blocked => CommandStatus::Blocked,
            ControlBehavior::FormatError | ControlBehavior::ParameterError => {
                CommandStatus::FormatError
            }
            ControlBehavior::AutomationInhibit => CommandStatus::AutomationInhibit,
            ControlBehavior::AlreadyExecuting => CommandStatus::Success, // handled separately
        }
    }

    /// Determine the BinaryOutputStatus value based on the CROB operation type.
    fn crob_output_value(op_type: OpType, current_value: bool) -> bool {
        match op_type {
            OpType::LatchOn | OpType::PulseOn => true,
            OpType::LatchOff | OpType::PulseOff => false,
            OpType::Nul | OpType::Unknown(_) => current_value,
        }
    }

    /// Returns true if the operation is a pulse (which should revert after busy window).
    fn is_pulse(op_type: OpType) -> bool {
        matches!(op_type, OpType::PulseOn | OpType::PulseOff)
    }

    /// Apply BO → BI associated index feedback from the profile.
    fn apply_bo_feedback(&self, index: u16, new_value: bool, database: &mut DatabaseHandle) {
        let profile_index = self.state.profile_index.read().unwrap();
        let Some(bi_index) = profile_index.bo_to_bi.get(&index) else {
            return;
        };
        database.transaction(|db| {
            db.update(
                *bi_index,
                &BinaryInput::new(new_value, Flags::ONLINE, get_current_time()),
                UpdateOptions::detect_event(),
            );
        });
    }

    /// Apply AO → AI associated index feedback from the profile.
    /// Also handles database multiplexing: if the AI is a selector readback, loads
    /// the selected entry into the DNP3 DB; if it is a data field, persists the
    /// value back to the in-memory database for the currently active entry.
    fn apply_ao_feedback(&self, index: u16, value: TransmissionI32, database: &mut DatabaseHandle) {
        let profile_index = self.state.profile_index.read().unwrap();
        let Some(&ai_index) = profile_index.ao_to_ai.get(&index) else {
            tracing::debug!(
                "Outstation received AO{index} = {}; no associated AI readback point is configured",
                value.0
            );
            return;
        };
        drop(profile_index);
        tracing::debug!(
            "Outstation received AO{index} = {}; applying feedback to associated AI{ai_index}",
            value.0
        );
        database.transaction(|dnp_db| {
            dnp_db.update(
                ai_index,
                &StepFuncAnalogInput::new(value.0.into(), Flags::ONLINE, get_current_time()),
                UpdateOptions::detect_event(),
            );
        });
        self.handle_database_multiplexing(ai_index, value, database);
    }

    /// Multiplex the curve and schedule databases based on a selector write or data write.
    ///
    /// Called after the AI readback has already been updated in the DNP3 DB.
    /// - If `ai_index` is the selector readback for a database, loads the selected
    ///   entry's stored values into the DNP3 DB.
    /// - If `ai_index` is a data field for a database, persists the value back to the
    ///   in-memory entry for the currently active entry (as indicated by the selector AI).
    fn handle_database_multiplexing(
        &self,
        ai_index: u16,
        value: TransmissionI32,
        database: &mut DatabaseHandle,
    ) {
        self.handle_curve_mux(ai_index, value, database);
        self.handle_schedule_bc_mux(ai_index, value, database);
        self.handle_schedule_mux(ai_index, value, database);
    }

    /// Handle curve database multiplexing for a single AI update.
    fn handle_curve_mux(
        &self,
        ai_index: u16,
        value: TransmissionI32,
        database: &mut DatabaseHandle,
    ) {
        let db_arc = &self.state.curve_database;

        let db = db_arc.read().unwrap();
        let selector_ai = db.selector_ai_uid() as u16;

        if ai_index == selector_ai {
            let curve_number = value.0 as u16;
            tracing::debug!(
                "Outstation selector AI{ai_index} = {curve_number}; selecting curve {curve_number}"
            );
            drop(db);
            let curve_points = db_arc.write().unwrap().set_active_entry(curve_number);
            let Some(curve_points) = curve_points else {
                tracing::warn!(
                    "Curve selector write {} is out of range; ignoring",
                    curve_number,
                );
                return;
            };
            database.transaction(|dnp3_db| {
                for curve_point in &curve_points {
                    tracing::debug!(
                        "Outstation curve {curve_number}: publishing stored AI{} = {} after selector AI{ai_index} changed",
                        curve_point.index,
                        curve_point.value.0
                    );
                    dnp3_db.update(
                        curve_point.index,
                        &StepFuncAnalogInput::new(
                            curve_point.value.0.into(),
                            Flags::ONLINE,
                            get_current_time(),
                        ),
                        UpdateOptions::detect_event(),
                    );
                }
            });
        } else if db
            .template_entry()
            .is_some_and(|s| s.all_values().iter().any(|v| v.index == ai_index))
        {
            let curve_number = db.current_entry();
            tracing::debug!(
                "Outstation curve {curve_number}: storing AO feedback as AI{ai_index} = {}",
                value.0
            );
            drop(db);
            db_arc.write().unwrap().update_value(ai_index, value);
        }
    }

    /// Handle backward-compatible schedule database multiplexing for a single AI update.
    fn handle_schedule_bc_mux(
        &self,
        ai_index: u16,
        value: TransmissionI32,
        database: &mut DatabaseHandle,
    ) {
        let db_arc = &self.state.schedule_bc_database;

        let db = db_arc.read().unwrap();
        let selector_ai = db.selector_ai_uid() as u16;

        if ai_index == selector_ai {
            let sched_number = value;
            drop(db);
            let points = db_arc
                .write()
                .unwrap()
                .set_active_entry(sched_number.0 as u16);
            let Some(points) = points else {
                tracing::warn!(
                    "BC schedule selector write {} is out of range; ignoring",
                    sched_number,
                );
                return;
            };
            database.transaction(|dnp3_db| {
                for point in &points {
                    dnp3_db.update(
                        point.index,
                        &StepFuncAnalogInput::new(
                            point.value.0.into(),
                            Flags::ONLINE,
                            get_current_time(),
                        ),
                        UpdateOptions::detect_event(),
                    );
                }
            });
        } else if db
            .template_entry()
            .is_some_and(|s| s.all_values().iter().any(|v| v.index == ai_index))
        {
            drop(db);
            db_arc.write().unwrap().update_value(ai_index, value);
        }
    }

    /// Handle IEEE 1815.2 schedule database multiplexing for a single AI update.
    fn handle_schedule_mux(
        &self,
        ai_index: u16,
        value: TransmissionI32,
        database: &mut DatabaseHandle,
    ) {
        let db_arc = &self.state.schedule_database;

        let db = db_arc.read().unwrap();
        let selector_ai = db.selector_ai_uid() as u16;

        if ai_index == selector_ai {
            let sched_number = value.0 as u16;
            drop(db);
            let points = db_arc.write().unwrap().set_active_entry(sched_number);
            let Some(points) = points else {
                tracing::warn!(
                    "IEEE schedule selector write {} is out of range; ignoring",
                    sched_number,
                );
                return;
            };
            database.transaction(|dnp3_db| {
                for point in &points {
                    dnp3_db.update(
                        point.index,
                        &StepFuncAnalogInput::new(
                            point.value.0.into(),
                            Flags::ONLINE,
                            get_current_time(),
                        ),
                        UpdateOptions::detect_event(),
                    );
                }
            });
        } else if db
            .template_entry()
            .is_some_and(|s| s.all_values().iter().any(|v| v.index == ai_index))
        {
            drop(db);
            db_arc.write().unwrap().update_value(ai_index, value);
        }
    }

    fn handle_select_analog_output(&mut self, value: TransmissionI32, index: u16) -> CommandStatus {
        // Range check runs first
        if !self.state.check_analog_range(index, value) {
            return CommandStatus::OutOfRange;
        }

        let behavior = self.state.get_analog_behavior(index);

        // SELECT always succeeds for already_executing (allows SBO pre-staging)
        if behavior == ControlBehavior::AlreadyExecuting {
            return CommandStatus::Success;
        }

        Self::behavior_to_status(behavior)
    }

    fn handle_operate_analog_output(
        &mut self,
        value: TransmissionI32,
        index: u16,
        database: &mut DatabaseHandle,
    ) -> CommandStatus {
        self.state.record_ao_write(index);
        tracing::debug!("Outstation received Direct Operate AO{index} = {}", value.0);

        // Conformance: the written AO index must exist in the profile.
        if !self
            .state
            .profile_index
            .read()
            .unwrap()
            .ao_points
            .contains_key(&index)
        {
            tracing::warn!(
                "Conformance: AO{index} written by control station is not defined in the loaded PicsProfile"
            );
        }

        let behavior = self.state.get_analog_behavior(index);

        // Range check runs first (applies regardless of behavior). If bounds exist and the
        // value violates them, record the violation for conformance reporting before rejecting.
        if let Some((min, max)) = self.state.get_analog_range_bounds(index) {
            if value < min || value > max {
                self.state.record_ao_range_violation(index, value, min, max);
                return CommandStatus::OutOfRange;
            }
        } else if !self.state.check_analog_range(index, value) {
            return CommandStatus::OutOfRange;
        }

        match behavior {
            ControlBehavior::Success => {
                database.transaction(|db| {
                    db.update(
                        index,
                        &AnalogOutputStatus::new(value.0.into(), Flags::ONLINE, get_current_time()),
                        UpdateOptions::detect_event(),
                    );
                });

                // Apply AO → AI feedback + curve multiplexing
                self.apply_ao_feedback(index, value, database);

                CommandStatus::Success
            }
            ControlBehavior::AlreadyExecuting => {
                if self.state.is_analog_busy(index) {
                    return CommandStatus::AlreadyActive;
                }

                database.transaction(|db| {
                    db.update(
                        index,
                        &AnalogOutputStatus::new(value.0.into(), Flags::ONLINE, get_current_time()),
                        UpdateOptions::detect_event(),
                    );
                });

                // Apply AO → AI feedback + curve multiplexing
                self.apply_ao_feedback(index, value, database);

                // Start busy timer and clear after duration
                self.state.start_analog_busy(index);
                let duration_ms = self.state.get_analog_execution_duration_ms(index);
                let busy_map = self.state.analog_busy_until.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(duration_ms)).await;
                    busy_map.write().unwrap().remove(&index);
                });

                CommandStatus::Success
            }
            other => Self::behavior_to_status(other),
        }
    }
}

impl ControlHandler for RefControlHandler {}

/// **DNP3 spec: Group 12 – Variation 1 (Control Relay Output Block, CROB)**
/// - Select: FC 3, Qualifier 17 or 28 (indexed); response: FC 129, echo of request
/// - Operate: FC 4; Direct Operate: FC 5; Direct Operate, No Ack: FC 6
///
/// Supported `OpType` values: `LatchOn`, `LatchOff`, `PulseOn`, `PulseOff`.
/// Updates Group 10 Var 2 (BinaryOutputStatus with flags) on the database, then
/// propagates BO → BI associated-index feedback from the profile.
impl ControlSupport<Group12Var1> for RefControlHandler {
    fn select(
        &mut self,
        _control: Group12Var1,
        index: u16,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        let behavior = self.state.get_binary_behavior(index);
        // SELECT always succeeds for already_executing (allows SBO pre-staging)
        if behavior == ControlBehavior::AlreadyExecuting {
            return CommandStatus::Success;
        }
        Self::behavior_to_status(behavior)
    }

    fn operate(
        &mut self,
        control: Group12Var1,
        index: u16,
        _op_type: OperateType,
        database: &mut DatabaseHandle,
    ) -> CommandStatus {
        self.state.record_bo_write(index);

        // Conformance: the written BO index must exist in the profile.
        if !self
            .state
            .profile_index
            .read()
            .unwrap()
            .bo_points
            .contains_key(&index)
        {
            tracing::warn!(
                "Conformance: BO{index} written by control station is not defined in the loaded PicsProfile"
            );
        }

        let behavior = self.state.get_binary_behavior(index);

        match behavior {
            ControlBehavior::Success => {
                // Read current value, compute new value, update
                let current = database
                    .transaction(|db| <Database as Get<BinaryOutputStatus>>::get(db, index))
                    .map(|v| v.value)
                    .unwrap_or(false);
                let new_value = Self::crob_output_value(control.code.op_type, current);

                database.transaction(|db| {
                    db.update(
                        index,
                        &BinaryOutputStatus::new(new_value, Flags::ONLINE, get_current_time()),
                        UpdateOptions::detect_event(),
                    );
                });

                // Apply BO → BI associated index feedback
                self.apply_bo_feedback(index, new_value, database);

                CommandStatus::Success
            }
            ControlBehavior::AlreadyExecuting => {
                // Check if busy
                if self.state.is_binary_busy(index) {
                    return CommandStatus::AlreadyActive;
                }

                // Read current value before updating
                let revert_value = database
                    .transaction(|db| <Database as Get<BinaryOutputStatus>>::get(db, index))
                    .map(|v| v.value)
                    .unwrap_or(false);
                let new_value = Self::crob_output_value(control.code.op_type, revert_value);

                // Update the output point
                database.transaction(|db| {
                    db.update(
                        index,
                        &BinaryOutputStatus::new(new_value, Flags::ONLINE, get_current_time()),
                        UpdateOptions::detect_event(),
                    );
                });

                // Apply BO → BI associated index feedback
                self.apply_bo_feedback(index, new_value, database);

                // Start busy timer
                self.state.start_binary_busy(index);
                let duration_ms = self.state.get_binary_execution_duration_ms(index);

                // For pulse operations, spawn a task to revert and clear busy
                if Self::is_pulse(control.code.op_type) {
                    let outstation = self.state.outstation();
                    let busy_map = self.state.binary_busy_until.clone();
                    let profile_index = Arc::clone(&*self.state.profile_index.read().unwrap());
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(duration_ms)).await;
                        busy_map.write().unwrap().remove(&index);
                        outstation.transaction(|db| {
                            db.update(
                                index,
                                &BinaryOutputStatus::new(
                                    revert_value,
                                    Flags::ONLINE,
                                    get_current_time(),
                                ),
                                UpdateOptions::detect_event(),
                            );
                            // Revert BI feedback too
                            if let Some(&bi_index) = profile_index.bo_to_bi.get(&index) {
                                db.update(
                                    bi_index,
                                    &BinaryInput::new(
                                        revert_value,
                                        Flags::ONLINE,
                                        get_current_time(),
                                    ),
                                    UpdateOptions::detect_event(),
                                );
                            }
                        });
                    });
                } else {
                    // For latch operations, just clear busy after duration
                    let busy_map = self.state.binary_busy_until.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(duration_ms)).await;
                        busy_map.write().unwrap().remove(&index);
                    });
                }

                CommandStatus::Success
            }
            other => Self::behavior_to_status(other),
        }
    }
}

/// **DNP3 spec: Group 41 – Variation 1 (32-bit integer Analog Output)**
/// Not required by IEEE 1815.2 — returns `NotSupported` for all operations.
/// The required variation is Group 41 Var 2 (16-bit).
impl ControlSupport<Group41Var1> for RefControlHandler {
    fn select(
        &mut self,
        control: Group41Var1,
        index: u16,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        self.handle_select_analog_output(TransmissionI32(control.value), index)
    }

    fn operate(
        &mut self,
        control: Group41Var1,
        index: u16,
        _op_type: OperateType,
        database: &mut DatabaseHandle,
    ) -> CommandStatus {
        self.handle_operate_analog_output(TransmissionI32(control.value), index, database)
    }
}

/// **DNP3 spec: Group 41 – Variation 2 (16-bit integer Analog Output)**
/// - Select: FC 3, Qualifier 17 or 28 (indexed); response: FC 129, echo of request
/// - Operate: FC 4; Direct Operate: FC 5; Direct Operate, No Ack: FC 6
///
/// Range is validated against the profile min/max before the behavior check.
/// On success, updates Group 40 Var 2 (AnalogOutputStatus with flags) on the database,
/// then propagates AO → AI associated-index feedback and database multiplexing.
impl ControlSupport<Group41Var2> for RefControlHandler {
    fn select(
        &mut self,
        control: Group41Var2,
        index: u16,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        self.handle_select_analog_output(control.value.into(), index)
    }

    fn operate(
        &mut self,
        control: Group41Var2,
        index: u16,
        _op_type: OperateType,
        database: &mut DatabaseHandle,
    ) -> CommandStatus {
        self.handle_operate_analog_output(control.value.into(), index, database)
    }
}

/// **DNP3 spec: Group 41 – Variation 3 (32-bit float Analog Output)**
/// Not required by IEEE 1815.2 — returns `NotSupported` for all operations.
/// The required variation is Group 41 Var 2 (16-bit).
impl ControlSupport<Group41Var3> for RefControlHandler {
    fn select(
        &mut self,
        _control: Group41Var3,
        _index: u16,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }

    fn operate(
        &mut self,
        _control: Group41Var3,
        _index: u16,
        _op_type: OperateType,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }
}

/// **DNP3 spec: Group 41 – Variation 4 (64-bit float Analog Output)**
/// Not required by IEEE 1815.2 — returns `NotSupported` for all operations.
/// The required variation is Group 41 Var 2 (16-bit).
impl ControlSupport<Group41Var4> for RefControlHandler {
    fn select(
        &mut self,
        _control: Group41Var4,
        _index: u16,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }

    fn operate(
        &mut self,
        _control: Group41Var4,
        _index: u16,
        _op_type: OperateType,
        _database: &mut DatabaseHandle,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }
}

/// Get the current time as a synchronized DNP3 time.
pub(crate) fn get_current_time() -> Time {
    let epoch_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    Time::Synchronized(Timestamp::new(epoch_time.as_millis() as u64))
}
