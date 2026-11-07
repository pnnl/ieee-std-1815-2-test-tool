//! Shared control station application state.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use common::profile::PicsProfile;
use common::profile::validation::Validated;
use common::profile::values::TransmissionI32;

use crate::config::AppConfig;
use crate::data_record::DataRecordWriter;

/// An observed transmission value outside the profile's allowed range.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RangeViolation {
    pub value: TransmissionI32,
    pub min: TransmissionI32,
    pub max: TransmissionI32,
}

/// Shared state for the control station application.
pub(crate) struct ControlStationState {
    /// JSONL data file writer.
    pub data_writer: Option<DataRecordWriter>,
    /// Latest values per point (key: "type:index"). A landing pad for the most recent AI/BI values received from the outstation.
    pub latest_transmission_values: RwLock<HashMap<String, serde_json::Value>>,
    /// Running configuration.
    pub config: AppConfig,
    /// Loaded PICS profile.
    pub profile: RwLock<Arc<Validated<PicsProfile>>>,
    /// Precomputed O(1) lookup index.
    pub profile_index: RwLock<Arc<common::profile::ProfileIndex>>,
    /// Curve database.
    pub outstation_curve_database: RwLock<common::profile::CurveDatabase>,
    /// Backward-compatible schedule database.
    pub schedule_bc_database: RwLock<common::profile::ScheduleBCDatabase>,
    /// IEEE 1815.2 schedule database.
    pub schedule_database: RwLock<common::profile::ScheduleDatabase>,
    /// AO indices that have been sent to the outstation at least once.
    pub sent_ao_indices: RwLock<HashSet<u16>>,
    /// BO indices that have been sent to the outstation at least once.
    pub sent_bo_indices: RwLock<HashSet<u16>>,
    /// Most recent out-of-range violation per AI point index.
    /// Overwritten on each occurrence so only the latest violation is retained.
    pub ai_range_violations: RwLock<HashMap<u16, RangeViolation>>,
    /// Most recent out-of-range violation per AO readback point index.
    pub ao_range_violations: RwLock<HashMap<u16, RangeViolation>>,
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
            .insert(index, RangeViolation { value, min, max });
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
            .insert(index, RangeViolation { value, min, max });
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
    }
}
