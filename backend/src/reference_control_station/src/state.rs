//! Shared control station application state.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

use common::profile::PicsProfile;
use common::profile::validation::Validated;
use common::profile::values::TransmissionI32;

use crate::config::AppConfig;
use crate::data_record::DataRecordWriter;

/// Maximum number of events to retain in memory.
const MAX_EVENT_HISTORY: usize = 10000;

/// Shared state for the control station application.
pub(crate) struct ControlStationState {
    /// JSONL data file writer.
    pub data_writer: Option<DataRecordWriter>,
    /// Latest values per point (key: "type:index"). A landing pad for the most recent AI/BI values received from the outstation.
    pub latest_transmission_values: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    /// Last observed IIN bits (iin1, iin2).
    pub last_iin: Arc<RwLock<Option<(u8, u8)>>>,
    /// Running configuration.
    pub config: Arc<AppConfig>,
    /// Recent events with timestamps for the events API.
    pub event_history: Arc<RwLock<VecDeque<serde_json::Value>>>,
    /// Loaded PICS profile.
    pub profile: Arc<RwLock<Arc<Validated<PicsProfile>>>>,
    /// Precomputed O(1) lookup index.
    pub profile_index: Arc<RwLock<Arc<common::profile::ProfileIndex>>>,
    /// Curve database.
    pub outstation_curve_database: Arc<RwLock<common::profile::CurveDatabase>>,
    /// Backward-compatible schedule database.
    pub schedule_bc_database: Arc<RwLock<common::profile::ScheduleBCDatabase>>,
    /// IEEE 1815.2 schedule database.
    pub schedule_database: Arc<RwLock<common::profile::ScheduleDatabase>>,
    /// AO indices that have been sent to the outstation at least once.
    pub sent_ao_indices: Arc<RwLock<HashSet<u16>>>,
    /// BO indices that have been sent to the outstation at least once.
    pub sent_bo_indices: Arc<RwLock<HashSet<u16>>>,
    /// Most recent out-of-range violation per AI point index: (detected_value, min, max).
    /// Overwritten on each occurrence so only the latest violation is retained.
    #[allow(clippy::type_complexity)]
    pub ai_range_violations:
        Arc<RwLock<HashMap<u16, (TransmissionI32, TransmissionI32, TransmissionI32)>>>,
    /// Most recent out-of-range violation per AO readback point index: (detected_value, min, max).
    #[allow(clippy::type_complexity)]
    pub ao_range_violations:
        Arc<RwLock<HashMap<u16, (TransmissionI32, TransmissionI32, TransmissionI32)>>>,
    /// Scenario ID shared with ConformanceTrackingLayer for tagging conformance results.
    pub scenario_id: Arc<Mutex<String>>,
}

impl ControlStationState {
    /// Record that an AO index was sent to the outstation (for conformance tracking).
    pub fn record_ao_write(&self, index: u16) {
        self.sent_ao_indices.write().unwrap().insert(index);
    }

    /// Record that a BO index was sent to the outstation (for conformance tracking).
    pub fn record_bo_write(&self, index: u16) {
        self.sent_bo_indices.write().unwrap().insert(index);
    }

    /// Add an event to the history buffer, evicting oldest if at capacity.
    pub fn push_event(&self, event: serde_json::Value) {
        let mut history = self.event_history.write().unwrap();
        if history.len() >= MAX_EVENT_HISTORY {
            history.pop_front();
        }
        history.push_back(event);
    }

    /// Record the most recent out-of-range violation for an AI point.
    /// Overwrites any previous violation for the same index.
    pub fn record_ai_range_violation(
        &self,
        index: u16,
        value: TransmissionI32,
        min: TransmissionI32,
        max: TransmissionI32,
    ) {
        self.ai_range_violations
            .write()
            .unwrap()
            .insert(index, (value, min, max));
    }

    /// Record the most recent out-of-range violation for an AO readback point.
    /// Overwrites any previous violation for the same index.
    pub fn record_ao_range_violation(
        &self,
        index: u16,
        value: TransmissionI32,
        min: TransmissionI32,
        max: TransmissionI32,
    ) {
        self.ao_range_violations
            .write()
            .unwrap()
            .insert(index, (value, min, max));
    }

    /// Reset all conformance tracking state between scenarios.
    ///
    /// Clears received AI/BI values, sent AO/BO indices, range violations, and
    /// event history so that the next scenario starts with a clean slate.
    pub fn reset_conformance_state(&self) {
        tracing::info!("Resetting conformance tracking state for new scenario");
        self.latest_transmission_values.write().unwrap().clear();
        self.sent_ao_indices.write().unwrap().clear();
        self.sent_bo_indices.write().unwrap().clear();
        self.ai_range_violations.write().unwrap().clear();
        self.ao_range_violations.write().unwrap().clear();
        self.event_history.write().unwrap().clear();
    }
}
