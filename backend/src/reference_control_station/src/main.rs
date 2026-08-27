//! DNP3 Reference Control Station application.

#[allow(dead_code)]
mod config;
mod conformance;
mod control_loop;
mod data_handlers;
mod data_record;
mod state;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use common::conformance_testing::logging::ConformanceTrackingLayer;
use common::profile::PicsProfile;
use dnp3::app::ConnectStrategy;
use dnp3::link::{EndpointAddress, LinkErrorMode};
use dnp3::master::*;
use dnp3::tcp::*;
use tracing_subscriber::prelude::*;

use dnp3::app::{Listener, MaybeAsync};
use dnp3::tcp::ClientState;

use crate::config::{
    AppConfig, DatabasesConfig, Dnp3Config, LoggingConfig, ReconnectConfig, StartupConfig,
};
use crate::data_handlers::RefReadHandler;
use crate::state::ControlStationState;

/// Logs TCP client connection state changes so the test runner can see progress.
struct ClientStateListener;

impl Listener<ClientState> for ClientStateListener {
    fn update(&mut self, value: ClientState) -> MaybeAsync<()> {
        match value {
            ClientState::Connecting => tracing::info!("TCP: connecting to outstation..."),
            ClientState::Connected => tracing::info!("TCP: connected to outstation"),
            ClientState::WaitAfterFailedConnect(d) => {
                tracing::warn!(
                    "TCP: connection failed, retrying in {:.1}s",
                    d.as_secs_f32()
                )
            }
            ClientState::WaitAfterDisconnect(d) => {
                tracing::warn!("TCP: disconnected, reconnecting in {:.1}s", d.as_secs_f32())
            }
            ClientState::Disabled => tracing::info!("TCP: channel disabled"),
            ClientState::Shutdown => tracing::info!("TCP: channel shut down"),
        }
        MaybeAsync::ready(())
    }
}

/// MESA DNP3 Reference Control Station for IEEE 1815.2
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Outstation IP address.
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    outstation_ip: String,
    /// Outstation port.
    #[arg(long, default_value_t = 20000)]
    outstation_port: u16,
    /// DNP3 address of this control station (master).
    #[arg(long, default_value_t = 1)]
    control_station_address: u16,
    /// DNP3 address of the outstation.
    #[arg(long, default_value_t = 1024)]
    outstation_address: u16,
    /// PICS profile path.
    #[arg(long)]
    profile: String,
    /// Log level (error, warn, info, debug, trace).
    #[arg(long)]
    log_level: Option<String>,
    /// How often the profile-driven control loop runs (seconds).
    #[arg(long, default_value_t = 10)]
    control_loop_interval: u64,
    /// How often the conformance tests run (seconds).
    #[arg(long, default_value_t = 10)]
    conformance_loop_interval: u64,
}

/// AssociationHandler implementation.
#[derive(Copy, Clone)]
struct RefAssociationHandler;

impl AssociationHandler for RefAssociationHandler {}

/// AssociationInformation implementation.
#[derive(Copy, Clone)]
struct RefAssociationInformation;

impl AssociationInformation for RefAssociationInformation {}

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
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                    log_filter,
                )),
        )
        .with(conformance_layer)
        .init();

    let app_config = AppConfig {
        dnp3: Dnp3Config {
            control_station_address: args.control_station_address,
            outstation_address: args.outstation_address,
            remote_address: format!("{}:{}", args.outstation_ip, args.outstation_port),
        },
        logging: LoggingConfig::default(),
        startup: StartupConfig::default(),
        poll_schedules: Vec::new(),
        reconnect: ReconnectConfig::default(),
        profile_path: args.profile.clone(),
        databases: DatabasesConfig::default(),
        control_loop_interval_secs: args.control_loop_interval,
        conformance_loop_interval_secs: args.conformance_loop_interval,
    };

    let control_station_address = EndpointAddress::try_new(app_config.dnp3.control_station_address)
        .map_err(|e| format!("invalid control station address: {e}"))?;
    let outstation_address = EndpointAddress::try_new(app_config.dnp3.outstation_address)
        .map_err(|e| format!("invalid outstation address: {e}"))?;

    let remote_address = app_config.dnp3.remote_address.clone();

    // Load profile
    let profile = {
        tracing::info!("Loading DER profile from '{}'", app_config.profile_path);
        let content = std::fs::read_to_string(&app_config.profile_path)
            .map_err(|e| format!("failed to read profile '{}': {e}", app_config.profile_path))?;
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

    // Open data file if configured
    let data_writer = data_record::open_data_file(&app_config.logging.data_file);

    // Build curve/schedule databases from profile with configured capacity
    let mut app_config = app_config;
    app_config.databases = common::profile::DatabasesConfig::from_profile(&profile);

    let max_curves = app_config.databases.max_curves;
    let max_schedules_bc = app_config.databases.max_schedules_bc;
    let max_schedules = app_config.databases.max_schedules;
    let outstation_curve_database = Arc::new(std::sync::RwLock::new(
        common::profile::CurveDatabase::from_profile(
            &profile,
            max_curves,
            common::uids::ai_uid::AiUid::Curve_DGSMn_InCrv,
        ),
    ));
    let schedule_bc_database = Arc::new(std::sync::RwLock::new(
        common::profile::ScheduleBCDatabase::from_profile(
            &profile,
            max_schedules_bc,
            common::uids::ai_uid::AiUid::BC_Scheduling_BC_FSCC_Schd,
        ),
    ));
    let schedule_database = Arc::new(std::sync::RwLock::new(
        common::profile::ScheduleDatabase::from_profile(
            &profile,
            max_schedules,
            common::uids::ai_uid::AiUid::Scheduling_FSCC_Schd,
        ),
    ));
    let profile_index = Arc::new(common::profile::ProfileIndex::from_profile(&profile));

    let config = Arc::new(app_config);

    // Build shared state
    let control_station_state = Arc::new(ControlStationState {
        data_writer,
        latest_transmission_values: Arc::new(std::sync::RwLock::new(HashMap::new())),
        last_iin: Arc::new(std::sync::RwLock::new(None)),
        config: config.clone(),
        event_history: Arc::new(std::sync::RwLock::new(std::collections::VecDeque::new())),
        profile: Arc::new(std::sync::RwLock::new(profile)),
        profile_index: Arc::new(std::sync::RwLock::new(profile_index)),
        outstation_curve_database,
        schedule_bc_database,
        schedule_database,
        sent_ao_indices: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
        sent_bo_indices: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
        ai_range_violations: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        ao_range_violations: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        scenario_id: Arc::clone(&scenario_id),
    });

    // Build association config from TOML startup settings
    let disable_unsol = if config.startup.disable_unsolicited {
        EventClasses::all()
    } else {
        EventClasses::none()
    };

    let enable_unsol = if config.startup.enable_unsolicited {
        EventClasses::all()
    } else {
        EventClasses::none()
    };

    let integrity_classes = if config.startup.integrity_poll {
        Classes::all()
    } else {
        Classes::none()
    };

    let mut assoc_config = AssociationConfig::new(
        disable_unsol,
        enable_unsol,
        integrity_classes,
        EventClasses::none(),
    );

    // Configure auto time sync
    if config.startup.auto_time_sync {
        let procedure = match config.startup.time_sync_procedure.as_str() {
            "non_lan" => TimeSyncProcedure::NonLan,
            "lan" => TimeSyncProcedure::Lan,
            _ => return Err("invalid time_sync_procedure: must be 'lan' or 'non_lan'".into()),
        };
        assoc_config.auto_time_sync = Some(procedure);
    }

    assoc_config.keep_alive_timeout = Some(Duration::from_secs(60));

    // Create connect strategy from reconnect config
    let connect_strategy = ConnectStrategy::new(
        Duration::from_millis(config.reconnect.min_delay_ms),
        Duration::from_millis(config.reconnect.max_delay_ms),
        Duration::from_millis(config.reconnect.reconnect_delay_ms),
    );

    // Create control station channel config
    let channel_config = MasterChannelConfig::new(control_station_address);

    // Create TCP client channel
    let mut channel = spawn_master_tcp_client(
        LinkErrorMode::Close,
        channel_config,
        EndpointList::new(remote_address.clone(), &[]),
        connect_strategy,
        Box::new(ClientStateListener),
    );

    // Add association
    let mut association = channel
        .add_association(
            outstation_address,
            assoc_config,
            RefReadHandler::boxed(control_station_state.clone()),
            Box::new(RefAssociationHandler),
            Box::new(RefAssociationInformation),
        )
        .await?;

    // Add configured polls
    let mut poll_handles = Vec::new();
    for schedule in &config.poll_schedules {
        let classes = build_classes(&schedule.classes);
        let poll = association
            .add_poll(
                ReadRequest::class_scan(classes),
                Duration::from_secs(schedule.interval_secs),
            )
            .await?;
        poll_handles.push(poll);
    }

    // Enable communications
    channel.enable().await?;

    tracing::info!(
        "Control station configured: address={}, outstation={}, remote={}",
        config.dnp3.control_station_address,
        config.dnp3.outstation_address,
        remote_address
    );

    // Notify used by the stdin handler to trigger an immediate control-loop cycle
    // after each `new_scenario` command instead of waiting for the next timer tick.
    let new_scenario_notify = Arc::new(tokio::sync::Notify::new());

    // Spawn profile-driven control loop (also runs conformance tests after each cycle).
    let interval = config.control_loop_interval_secs;
    tracing::info!("Starting profile-driven control loop (interval={interval}s)");
    let control_loop_handle = Some(control_loop::update_outstation_or_test_forever(
        control_station_state.clone(),
        association.clone(),
        interval,
        Arc::clone(&new_scenario_notify),
    ));

    // Spawn stdin reader task that handles `new_scenario` commands from the test runner.
    // Format: new_scenario <scenario_id> <profile_json>
    let stdin_state = Arc::clone(&control_station_state);
    let stdin_notify = Arc::clone(&new_scenario_notify);
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

            let parsed_profile: PicsProfile = match serde_json::from_str(profile_json_str) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(
                        "Failed to parse profile JSON for new_scenario '{}': {e}",
                        new_scenario_id
                    );
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

            // Rebuild in-memory databases from new profile
            let db_config = &stdin_state.config.databases;
            *stdin_state.outstation_curve_database.write().unwrap() =
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

            // Update scenario_id (used by ConformanceTrackingLayer to tag results)
            *stdin_state.scenario_id.lock().unwrap() = new_scenario_id.to_string();

            tracing::info!("Scenario '{new_scenario_id}' loaded successfully");

            // Wake the control loop so it writes and runs conformance tests immediately
            // rather than waiting for the next periodic tick.
            stdin_notify.notify_one();
        }
    });

    // Run until interrupted.
    // Note: these must be kept alive (not dropped) so the background control station task keeps running.
    // `let _ = x` in Rust drops immediately; `let _guards = x` keeps alive until end of scope.
    let _guards = (association, poll_handles, channel, control_loop_handle);
    std::future::pending::<()>().await;

    Ok(())
}

/// Build a Classes value from a list of class numbers.
fn build_classes(class_list: &[u8]) -> Classes {
    let has_0 = class_list.contains(&0);
    let has_1 = class_list.contains(&1);
    let has_2 = class_list.contains(&2);
    let has_3 = class_list.contains(&3);

    let events = EventClasses::new(has_1, has_2, has_3);
    Classes::new(has_0, events)
}
