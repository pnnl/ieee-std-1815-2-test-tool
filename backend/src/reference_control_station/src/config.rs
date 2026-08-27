//! Application configuration for the reference control station.

const DEFAULT_TIME_SYNC_PROCEDURE: &str = "lan";
const DEFAULT_MIN_DELAY_MS: u64 = 1000;
const DEFAULT_MAX_DELAY_MS: u64 = 30000;
const DEFAULT_MAX_COUNT: u16 = 100;

/// Top-level application config.
#[derive(Debug)]
pub(crate) struct AppConfig {
    /// DNP3 protocol settings.
    pub dnp3: Dnp3Config,
    /// Logging settings.
    pub logging: LoggingConfig,
    /// Startup sequence settings.
    pub startup: StartupConfig,
    /// Poll schedules.
    pub poll_schedules: Vec<PollSchedule>,
    /// Reconnect settings.
    pub reconnect: ReconnectConfig,
    /// Path to the DER profile JSON file. Empty means no profile.
    pub profile_path: String,
    /// In-memory database configuration.
    pub databases: DatabasesConfig,
    /// How often the profile-driven control loop runs (seconds).
    pub control_loop_interval_secs: u64,
    /// How often the conformance tests run (seconds).
    pub conformance_loop_interval_secs: u64,
}

/// DNP3 transport config.
#[derive(Debug)]
pub(crate) struct Dnp3Config {
    /// Control station DNP3 address.
    pub control_station_address: u16,
    /// Outstation DNP3 address.
    pub outstation_address: u16,
    /// Remote outstation address.
    pub remote_address: String,
}

/// Logging config.
#[derive(Debug)]
pub(crate) struct LoggingConfig {
    /// Log level.
    pub level: String,
    /// Optional JSONL output file for received data.
    pub data_file: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            data_file: String::new(),
        }
    }
}

/// Startup sequence settings.
#[derive(Debug)]
pub(crate) struct StartupConfig {
    /// Disable unsolicited on startup.
    pub disable_unsolicited: bool,
    /// Perform integrity poll on startup.
    pub integrity_poll: bool,
    /// Enable unsolicited on startup.
    pub enable_unsolicited: bool,
    /// Auto time sync on startup.
    pub auto_time_sync: bool,
    /// Time sync procedure.
    pub time_sync_procedure: String,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            disable_unsolicited: true,
            integrity_poll: true,
            enable_unsolicited: true,
            auto_time_sync: true,
            time_sync_procedure: DEFAULT_TIME_SYNC_PROCEDURE.to_string(),
        }
    }
}

/// A single poll schedule.
#[derive(Debug, Clone)]
pub(crate) struct PollSchedule {
    /// Classes to poll.
    pub classes: Vec<u8>,
    /// Interval in seconds.
    pub interval_secs: u64,
}

/// Reconnect settings.
#[derive(Debug)]
pub(crate) struct ReconnectConfig {
    /// Minimum delay in ms between reconnect attempts.
    pub min_delay_ms: u64,
    /// Maximum delay in ms between reconnect attempts (exponential back-off ceiling).
    pub max_delay_ms: u64,
    /// Initial reconnect delay in ms (used for the first attempt after disconnect).
    pub reconnect_delay_ms: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            min_delay_ms: DEFAULT_MIN_DELAY_MS,
            max_delay_ms: DEFAULT_MAX_DELAY_MS,
            reconnect_delay_ms: DEFAULT_MIN_DELAY_MS,
        }
    }
}

pub(crate) use common::profile::DatabasesConfig;
