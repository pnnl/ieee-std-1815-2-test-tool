//! Shared application state.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use common::profile::PicsProfile;
use common::profile::validation::Validated;
use common::profile::values::TransmissionI32;
use dnp3::outstation::OutstationHandle;
use tokio::time::Instant;

use crate::config::{AppConfig, ControlBehavior, RangeConfig};

/// Default execution duration for already_executing behavior (ms).
const DEFAULT_EXECUTION_DURATION_MS: u64 = 5000;

/// Shared application state accessible from simulation tasks and the control handler.
pub(crate) struct AppState {
    /// Outstation handle for database transactions.
    pub outstation: RwLock<Option<OutstationHandle>>,
    /// Binary control behaviors (index -> behavior).
    pub binary_controls: Arc<RwLock<HashMap<u16, ControlBehavior>>>,
    /// Analog control behaviors (index -> behavior).
    pub analog_controls: Arc<RwLock<HashMap<u16, ControlBehavior>>>,
    /// Freeze mappings (counter_index -> frozen_counter_index).
    pub freeze_mappings: Arc<RwLock<HashMap<u16, u16>>>,
    /// Running configuration snapshot.
    pub config: Arc<AppConfig>,
    /// Number of counters.
    pub counter_count: u16,
    /// Number of frozen counters.
    pub frozen_counter_count: u16,
    /// Default binary control behavior.
    pub default_binary_control: ControlBehavior,
    /// Default analog control behavior.
    pub default_analog_control: ControlBehavior,
    /// Binary control execution durations (index -> duration_ms).
    pub binary_execution_durations: HashMap<u16, u64>,
    /// Analog control execution durations (index -> duration_ms).
    pub analog_execution_durations: HashMap<u16, u64>,
    /// Analog output range bounds (index -> range config).
    pub analog_ranges: HashMap<u16, RangeConfig>,
    /// Binary control busy-until timestamps (index -> Instant when busy expires).
    pub binary_busy_until: Arc<RwLock<HashMap<u16, Instant>>>,
    /// Analog control busy-until timestamps (index -> Instant when busy expires).
    pub analog_busy_until: Arc<RwLock<HashMap<u16, Instant>>>,
    /// Loaded PICS profile.
    pub profile: Arc<RwLock<Arc<Validated<PicsProfile>>>>,
    /// Precomputed O(1) lookup index derived from the profile.
    pub profile_index: Arc<RwLock<Arc<common::profile::ProfileIndex>>>,
    /// AO indices that have been operated at least once by the control station.
    pub received_ao_indices: Arc<RwLock<HashSet<u16>>>,
    /// BO indices that have been operated at least once by the control station.
    pub received_bo_indices: Arc<RwLock<HashSet<u16>>>,
    /// Curve database.
    pub curve_database: Arc<RwLock<common::profile::CurveDatabase>>,
    /// Backward-compatible schedule database.
    pub schedule_bc_database: Arc<RwLock<common::profile::ScheduleBCDatabase>>,
    /// IEEE 1815.2 schedule database.
    pub schedule_database: Arc<RwLock<common::profile::ScheduleDatabase>>,
    /// Scenario ID shared with ConformanceTrackingLayer for tagging conformance results.
    pub scenario_id: Arc<Mutex<String>>,
    /// Most recent out-of-range violation per AO command point index: (detected_value, min, max).
    /// Overwritten on each occurrence so only the latest violation is retained.
    #[allow(clippy::type_complexity)]
    pub ao_range_violations:
        Arc<RwLock<HashMap<u16, (TransmissionI32, TransmissionI32, TransmissionI32)>>>,
}

impl AppState {
    /// Get the outstation handle (panics if not set yet).
    pub fn outstation(&self) -> OutstationHandle {
        self.outstation
            .read()
            .unwrap()
            .clone()
            .expect("outstation handle not set")
    }

    /// Set the outstation handle.
    pub fn set_outstation(&self, handle: OutstationHandle) {
        *self.outstation.write().unwrap() = Some(handle);
    }

    /// Get the behavior for a binary control index.
    /// Record that a BO index was operated (for conformance tracking).
    pub fn record_bo_write(&self, index: u16) {
        self.received_bo_indices.write().unwrap().insert(index);
    }

    /// Record that an AO index was operated (for conformance tracking).
    pub fn record_ao_write(&self, index: u16) {
        self.received_ao_indices.write().unwrap().insert(index);
    }

    pub fn get_binary_behavior(&self, index: u16) -> ControlBehavior {
        self.binary_controls
            .read()
            .unwrap()
            .get(&index)
            .copied()
            .unwrap_or(self.default_binary_control)
    }

    /// Get the behavior for an analog control index.
    pub fn get_analog_behavior(&self, index: u16) -> ControlBehavior {
        self.analog_controls
            .read()
            .unwrap()
            .get(&index)
            .copied()
            .unwrap_or(self.default_analog_control)
    }

    /// Get the frozen counter index for a given counter index.
    pub fn get_freeze_target(&self, counter_index: u16) -> u16 {
        self.freeze_mappings
            .read()
            .unwrap()
            .get(&counter_index)
            .copied()
            .unwrap_or(counter_index)
    }

    /// Check if a binary control point is currently busy (already_executing).
    pub fn is_binary_busy(&self, index: u16) -> bool {
        self.binary_busy_until
            .read()
            .unwrap()
            .get(&index)
            .is_some_and(|&until| Instant::now() < until)
    }

    /// Check if an analog control point is currently busy (already_executing).
    pub fn is_analog_busy(&self, index: u16) -> bool {
        self.analog_busy_until
            .read()
            .unwrap()
            .get(&index)
            .is_some_and(|&until| Instant::now() < until)
    }

    /// Start a busy window for a binary control point.
    pub fn start_binary_busy(&self, index: u16) {
        let duration_ms = self
            .binary_execution_durations
            .get(&index)
            .copied()
            .unwrap_or(DEFAULT_EXECUTION_DURATION_MS);
        let until = Instant::now() + std::time::Duration::from_millis(duration_ms);
        self.binary_busy_until.write().unwrap().insert(index, until);
    }

    /// Start a busy window for an analog control point.
    pub fn start_analog_busy(&self, index: u16) {
        let duration_ms = self
            .analog_execution_durations
            .get(&index)
            .copied()
            .unwrap_or(DEFAULT_EXECUTION_DURATION_MS);
        let until = Instant::now() + std::time::Duration::from_millis(duration_ms);
        self.analog_busy_until.write().unwrap().insert(index, until);
    }

    /// Get the execution duration in ms for a binary control point.
    pub fn get_binary_execution_duration_ms(&self, index: u16) -> u64 {
        self.binary_execution_durations
            .get(&index)
            .copied()
            .unwrap_or(DEFAULT_EXECUTION_DURATION_MS)
    }

    /// Get the execution duration in ms for an analog control point.
    pub fn get_analog_execution_duration_ms(&self, index: u16) -> u64 {
        self.analog_execution_durations
            .get(&index)
            .copied()
            .unwrap_or(DEFAULT_EXECUTION_DURATION_MS)
    }

    /// Check analog output value against configured range bounds.
    /// Returns true if in range or no range configured.
    pub fn check_analog_range(&self, index: u16, value: TransmissionI32) -> bool {
        // Check profile-derived ranges first (covers all AO types: base, meter, inverter, battery).
        if let Some(ao_pt) = self.profile_index.read().unwrap().ao_points.get(&index) {
            return value >= ao_pt.minimum() && value <= ao_pt.maximum();
        }
        // Fall back to TOML ranges
        if let Some(range) = self.analog_ranges.get(&index) {
            value >= range.min && value <= range.max
        } else {
            true // no bounds configured = always in range
        }
    }

    /// Return the (min, max) bounds for an AO point, or None if no bounds are configured.
    pub fn get_analog_range_bounds(
        &self,
        index: u16,
    ) -> Option<(TransmissionI32, TransmissionI32)> {
        if let Some(ao_pt) = self.profile_index.read().unwrap().ao_points.get(&index) {
            return Some((ao_pt.minimum(), ao_pt.maximum()));
        }
        self.analog_ranges
            .get(&index)
            .map(|range| (range.min, range.max))
    }

    /// Record the most recent out-of-range violation for an AO command point.
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
}
