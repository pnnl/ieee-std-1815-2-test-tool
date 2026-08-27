//! Application configuration for the reference outstation.

use std::collections::HashMap;

use common::profile::values::TransmissionI32;

const DEFAULT_CONTROL_BEHAVIOR: &str = "success";
const DEFAULT_MAX_COUNT: u16 = 100;

/// Top-level application config.
#[derive(Debug)]
pub(crate) struct AppConfig {
    /// DNP3 protocol settings.
    pub dnp3: Dnp3Config,
    /// Log level string (e.g. "info", "debug").
    pub log_level: String,
    /// Whether unsolicited responses are enabled.
    pub unsolicited_enabled: bool,
    /// Whether to accept time writes from the master.
    pub accept_time_writes: bool,
    /// Freeze mapping (counter_index -> frozen_counter_index).
    pub freeze_mapping: HashMap<u16, u16>,
    /// Control point behavior.
    pub controls: ControlsConfig,
    /// Path to the DER profile JSON file.
    pub profile_path: String,
    /// In-memory database configuration.
    pub databases: DatabasesConfig,
}

/// DNP3 transport config.
#[derive(Debug)]
pub(crate) struct Dnp3Config {
    /// Outstation DNP3 address.
    pub outstation_address: u16,
    /// Master DNP3 address.
    pub master_address: u16,
    /// TCP bind address.
    pub bind_address: String,
}

/// Control behavior configuration.
#[derive(Debug, Default)]
pub(crate) struct ControlsConfig {
    /// CROB control behaviors.
    pub binary: ControlTypeConfig,
    /// Analog output control behaviors.
    pub analog: ControlTypeConfig,
}

/// Per-type control behavior with default and per-index overrides.
#[derive(Debug)]
pub(crate) struct ControlTypeConfig {
    /// Default behavior for all indices.
    pub default: String,
    /// Execution duration in ms for already_executing behavior (index -> duration_ms).
    pub execution_duration: HashMap<u16, u64>,
    /// Analog output value range bounds (index -> {min, max}).
    pub range: HashMap<u16, RangeConfig>,
    /// Per-index overrides: index -> behavior string.
    pub overrides: HashMap<String, toml::Value>,
}

impl Default for ControlTypeConfig {
    fn default() -> Self {
        Self {
            default: DEFAULT_CONTROL_BEHAVIOR.to_string(),
            execution_duration: HashMap::new(),
            range: HashMap::new(),
            overrides: HashMap::new(),
        }
    }
}

/// Range bounds for analog output value checking.
#[derive(Debug, Clone)]
pub(crate) struct RangeConfig {
    /// Minimum allowed value.
    pub min: TransmissionI32,
    /// Maximum allowed value.
    pub max: TransmissionI32,
}

pub(crate) use common::profile::DatabasesConfig;

pub(crate) fn parse_control_behavior(s: &str) -> Result<ControlBehavior, String> {
    match s.to_lowercase().as_str() {
        "success" => Ok(ControlBehavior::Success),
        "not_supported" | "notsupported" => Ok(ControlBehavior::NotSupported),
        "timeout" => Ok(ControlBehavior::Timeout),
        "blocked" => Ok(ControlBehavior::Blocked),
        "format_error" | "formaterror" => Ok(ControlBehavior::FormatError),
        "parameter_error" | "parametererror" => Ok(ControlBehavior::ParameterError),
        "automation_inhibit" | "automationinhibit" => Ok(ControlBehavior::AutomationInhibit),
        "already_executing" | "alreadyexecuting" => Ok(ControlBehavior::AlreadyExecuting),
        _ => Err(format!("invalid control behavior: {s}")),
    }
}

/// Control point behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBehavior {
    Success,
    NotSupported,
    Timeout,
    Blocked,
    FormatError,
    ParameterError,
    AutomationInhibit,
    AlreadyExecuting,
}
