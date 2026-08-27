//! DNP3 Reference Outstation application.

#[allow(dead_code)]
mod config;
mod conformance;
mod database;
mod outstation;
mod state;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use clap::Parser;
use common::conformance_testing::logging::ConformanceTrackingLayer;
use common::profile::PicsProfile;
use dnp3::app::{Listener, MaybeAsync};
use dnp3::link::{EndpointAddress, LinkErrorMode};
use dnp3::outstation::*;
use dnp3::tcp::*;
use tracing_subscriber::prelude::*;

use crate::config::{
    AppConfig, ControlsConfig, DatabasesConfig, Dnp3Config, parse_control_behavior,
};
use crate::database::{build_event_buffer_config, init_database, reinit_database};
use crate::outstation::{FreezeOutstationApplication, RefControlHandler, RefOutstationInformation};
use crate::state::AppState;

/// Logs DNP3 master connection state so the test runner can see when a control station connects.
struct ConnectionStateListener;

impl Listener<ConnectionState> for ConnectionStateListener {
    fn update(&mut self, value: ConnectionState) -> MaybeAsync<()> {
        match value {
            ConnectionState::Connected => tracing::info!("DNP3: control station connected"),
            ConnectionState::Disconnected => tracing::info!("DNP3: control station disconnected"),
        }
        MaybeAsync::ready(())
    }
}

/// MESA DNP3 Reference Outstation for IEEE 1815.2
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Local TCP bind address.
    #[arg(long, default_value_t = String::from("0.0.0.0:20000"))]
    local: String,
    /// DNP3 address of this outstation.
    #[arg(long, default_value_t = 1024)]
    outstation_address: u16,
    /// DNP3 address of the master (control station).
    #[arg(long, default_value_t = 1)]
    master_address: u16,
    /// PICS profile path.
    #[arg(long)]
    profile: String,
    /// Log level (error, warn, info, debug, trace).
    #[arg(long)]
    log_level: Option<String>,
    /// How often the conformance tests run (seconds).
    #[arg(long, default_value_t = 10)]
    conformance_loop_interval: u64,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    let log_level = args.log_level.as_deref().unwrap_or("info");
    let log_filter = match log_level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        _ => tracing::Level::ERROR,
    };

    let scenario_id = Arc::new(Mutex::new(String::new()));
    let conformance_messages = Arc::new(Mutex::new(Vec::new()));
    let conformance_layer =
        ConformanceTrackingLayer::new(conformance_messages, Arc::clone(&scenario_id));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                    log_filter,
                )),
        )
        .with(conformance_layer)
        .init();

    let app_config = AppConfig {
        dnp3: Dnp3Config {
            outstation_address: args.outstation_address,
            master_address: args.master_address,
            bind_address: args.local.clone(),
        },
        log_level: log_level.to_string(),
        unsolicited_enabled: true,
        accept_time_writes: true,
        freeze_mapping: HashMap::new(),
        controls: ControlsConfig::default(),
        profile_path: args.profile.clone(),
        databases: DatabasesConfig::default(),
    };

    let bind_address: std::net::SocketAddr = app_config.dnp3.bind_address.parse().map_err(|e| {
        format!(
            "invalid bind address '{}': {e}",
            app_config.dnp3.bind_address
        )
    })?;

    let outstation_address = EndpointAddress::try_new(app_config.dnp3.outstation_address)
        .map_err(|e| format!("invalid outstation address: {e}"))?;
    let master_address = EndpointAddress::try_new(app_config.dnp3.master_address)
        .map_err(|e| format!("invalid master address: {e}"))?;

    // Load profile (required)
    let profile = {
        let path = &app_config.profile_path;
        tracing::info!("Loading DER profile from '{path}'");
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read profile '{path}': {e}"))?;
        let p = PicsProfile::into_validated(
            serde_json::from_str(&content).map_err(|e| format!("failed to parse profile: {e}"))?,
        )?;
        tracing::info!(
            "Profile loaded: {} BI, {} BO, {} AI, {} AO base points",
            p.bi.points.len(),
            p.bo.points.len(),
            p.ai.points.len(),
            p.ao.points.len(),
        );
        Arc::new(p)
    };

    // Build event buffer config from profile
    let event_buffer_config = build_event_buffer_config(&profile);

    let mut outstation_config =
        OutstationConfig::new(outstation_address, master_address, event_buffer_config);

    // Configure unsolicited support
    if !app_config.unsolicited_enabled {
        outstation_config.features.unsolicited = Feature::Disabled;
    }

    // Parse control behaviors from config
    let mut binary_controls = HashMap::new();
    let default_binary_control = parse_control_behavior(&app_config.controls.binary.default)
        .expect("invalid default binary control behavior");
    for (key, val) in &app_config.controls.binary.overrides {
        if let Ok(idx) = key.parse::<u16>() {
            if let Some(s) = val.as_str() {
                let behavior =
                    parse_control_behavior(s).expect("invalid binary control override behavior");
                binary_controls.insert(idx, behavior);
            }
        }
    }

    let mut analog_controls = HashMap::new();
    let default_analog_control = parse_control_behavior(&app_config.controls.analog.default)
        .expect("invalid default analog control behavior");
    for (key, val) in &app_config.controls.analog.overrides {
        if let Ok(idx) = key.parse::<u16>() {
            if let Some(s) = val.as_str() {
                let behavior =
                    parse_control_behavior(s).expect("invalid analog control override behavior");
                analog_controls.insert(idx, behavior);
            }
        }
    }

    let binary_execution_durations = app_config.controls.binary.execution_duration.clone();
    let analog_execution_durations = app_config.controls.analog.execution_duration.clone();
    let analog_ranges = app_config.controls.analog.range.clone();

    let freeze_mappings = app_config.freeze_mapping.clone();

    // Build curve/schedule databases from profile with configured capacity
    let mut app_config = app_config;
    app_config.databases = common::profile::DatabasesConfig::from_profile(&profile);

    let max_curves = app_config.databases.max_curves;
    let max_schedules_bc = app_config.databases.max_schedules_bc;
    let max_schedules = app_config.databases.max_schedules;
    let curve_database = Arc::new(RwLock::new(common::profile::CurveDatabase::from_profile(
        &profile,
        max_curves,
        common::uids::ai_uid::AiUid::Curve_DGSMn_InCrv,
    )));
    let schedule_bc_database = Arc::new(RwLock::new(
        common::profile::ScheduleBCDatabase::from_profile(
            &profile,
            max_schedules_bc,
            common::uids::ai_uid::AiUid::BC_Scheduling_BC_FSCC_Schd,
        ),
    ));
    let schedule_database = Arc::new(RwLock::new(
        common::profile::ScheduleDatabase::from_profile(
            &profile,
            max_schedules,
            common::uids::ai_uid::AiUid::Scheduling_FSCC_Schd,
        ),
    ));
    let profile_index = Arc::new(RwLock::new(Arc::new(
        common::profile::ProfileIndex::from_profile(&profile),
    )));

    let config = Arc::new(app_config);
    let counter_count = profile
        .ctr
        .iter()
        .map(|c| c.point_index)
        .max()
        .map_or(0, |m| m + 1);
    let frozen_counter_count = profile
        .ctr
        .iter()
        .filter(|c| c.frozen_counter_exists)
        .map(|c| c.point_index)
        .max()
        .map_or(0, |m| m + 1);

    // Build shared state (outstation handle set after creation)
    let state = Arc::new(AppState {
        outstation: RwLock::new(None),
        binary_controls: Arc::new(RwLock::new(binary_controls)),
        analog_controls: Arc::new(RwLock::new(analog_controls)),
        freeze_mappings: Arc::new(RwLock::new(freeze_mappings)),
        config: config.clone(),
        counter_count,
        frozen_counter_count,
        default_binary_control,
        default_analog_control,
        binary_execution_durations,
        analog_execution_durations,
        analog_ranges,
        binary_busy_until: Arc::new(RwLock::new(HashMap::new())),
        analog_busy_until: Arc::new(RwLock::new(HashMap::new())),
        profile: Arc::new(RwLock::new(profile.clone())),
        profile_index,
        received_ao_indices: Arc::new(RwLock::new(HashSet::new())),
        received_bo_indices: Arc::new(RwLock::new(HashSet::new())),
        curve_database,
        schedule_bc_database,
        schedule_database,
        ao_range_violations: Arc::new(RwLock::new(HashMap::new())),
        scenario_id: Arc::clone(&scenario_id),
    });

    // Create TCP server and outstation
    let mut server = Server::new_tcp_server(LinkErrorMode::Close, bind_address);

    let outstation = server.add_outstation(
        outstation_config,
        Box::new(FreezeOutstationApplication::new(
            config.accept_time_writes,
            state.clone(),
        )),
        Box::new(RefOutstationInformation),
        Box::new(RefControlHandler::new(state.clone())),
        Box::new(ConnectionStateListener),
        AddressFilter::Any,
    )?;

    // Set the outstation handle in shared state
    state.set_outstation(outstation.clone());

    // Initialize the database from profile
    init_database(&outstation, &profile);

    tracing::info!(
        "Outstation configured: address={}, master={}, bind={}",
        config.dnp3.outstation_address,
        config.dnp3.master_address,
        config.dnp3.bind_address
    );

    // Start the TCP server
    let _server_handle = server.bind().await?;
    tracing::info!("TCP server listening on {}", config.dnp3.bind_address);

    // Spawn a periodic conformance check task.
    let conformance_interval = args.conformance_loop_interval;
    tracing::info!("Starting conformance test loop (interval={conformance_interval}s)");
    let conformance_state = Arc::clone(&state);
    tokio::spawn(async move {
        let interval = Duration::from_secs(conformance_interval);
        loop {
            tokio::time::sleep(interval).await;
            crate::conformance::run_conformance_tests(&conformance_state);
        }
    });

    // Spawn a stdin reader task that handles `new_scenario` commands from the test runner.
    // Format: new_scenario <scenario_id> <profile_json>
    let stdin_state = Arc::clone(&state);
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let stdin = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(stdin).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            const NEW_SCENARIO_PREFIX: &str = "new_scenario ";
            let Some(rest) = line.strip_prefix(NEW_SCENARIO_PREFIX) else {
                tracing::warn!("Unknown stdin command: {line}");
                continue;
            };

            let Some(space_idx) = rest.find(' ') else {
                tracing::warn!("new_scenario command missing profile JSON");
                continue;
            };

            let new_scenario_id = &rest[..space_idx];
            let profile_json_str = &rest[space_idx + 1..];

            let parsed_profile: common::profile::PicsProfile =
                match serde_json::from_str(profile_json_str) {
                    Ok(p) => p,
                    Err(err) => {
                        tracing::error!("Failed to parse profile JSON for new_scenario: {err}");
                        continue;
                    }
                };
            let new_profile = match parsed_profile.into_validated() {
                Ok(p) => p,
                Err(err) => {
                    tracing::error!(
                        "Profile validation failed for test scenario '{new_scenario_id}': {err}"
                    );
                    continue;
                }
            };

            let new_profile = Arc::new(new_profile);
            let new_profile_index =
                Arc::new(common::profile::ProfileIndex::from_profile(&new_profile));

            tracing::info!("Loading new scenario '{new_scenario_id}'");

            // Re-initialize the DNP3 database
            let old_profile = Arc::clone(&*stdin_state.profile.read().unwrap());
            let outstation_handle = stdin_state.outstation();
            reinit_database(&outstation_handle, &old_profile, &new_profile);

            // Rebuild in-memory databases from new profile
            let db_config = &stdin_state.config.databases;
            *stdin_state.curve_database.write().unwrap() =
                common::profile::CurveDatabase::from_profile(
                    &new_profile,
                    db_config.max_curves,
                    common::uids::ai_uid::AiUid::Curve_DGSMn_InCrv,
                );
            *stdin_state.schedule_bc_database.write().unwrap() =
                common::profile::ScheduleBCDatabase::from_profile(
                    &new_profile,
                    db_config.max_schedules_bc,
                    common::uids::ai_uid::AiUid::BC_Scheduling_BC_FSCC_Schd,
                );
            *stdin_state.schedule_database.write().unwrap() =
                common::profile::ScheduleDatabase::from_profile(
                    &new_profile,
                    db_config.max_schedules,
                    common::uids::ai_uid::AiUid::Scheduling_FSCC_Schd,
                );

            // Swap profile and profile_index
            *stdin_state.profile.write().unwrap() = Arc::clone(&new_profile);
            *stdin_state.profile_index.write().unwrap() = new_profile_index;

            // Reset conformance tracking state for the new scenario
            stdin_state.received_ao_indices.write().unwrap().clear();
            stdin_state.received_bo_indices.write().unwrap().clear();
            stdin_state.ao_range_violations.write().unwrap().clear();

            // Update scenario_id (used by ConformanceTrackingLayer to tag results)
            *stdin_state.scenario_id.lock().unwrap() = new_scenario_id.to_string();

            tracing::info!("Scenario '{new_scenario_id}' loaded successfully");
        }
    });

    // Run until interrupted
    std::future::pending::<()>().await;

    Ok(())
}
