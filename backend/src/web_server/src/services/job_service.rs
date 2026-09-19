use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::conformance_testing::message_structure::TestName;
use common::profile::validation::Validated;
use serde::de::Error;
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, Level, debug, error, info, span, warn};
use uuid::Uuid;

use common::profile::PicsProfile;

use crate::models::job::{
    CreateJobRequest, CreateJobResponse, DeviceUnderTest, JobEvent, JobStatus, JobStatusResponse,
    StopJobResponse,
};
use crate::models::scenario::Scenario;
use crate::services::process_manager;

const EVENT_CHANNEL_CAPACITY: usize = 256;
const DEFAULT_OUTSTATION_IP: &str = "127.0.0.1";
const DEFAULT_OUTSTATION_PORT: u16 = 20000;
const OUTSTATION_STARTUP_WAIT_SECS: u64 = 30;

/// Maximum number of `log` events buffered per job for late-subscriber replay.
/// Bounded to prevent unbounded memory growth if no SSE subscriber ever attaches.
/// 500 is enough to cover roughly two seconds of full-throughput outstation +
/// control-station logging during startup, which is the gap we are closing.
const LOG_BUFFER_CAPACITY: usize = 500;

/// Timeout for waiting for all conformance results per scenario (seconds).
const SCENARIO_TIMEOUT_SECS: u64 = 300;

/// Internal state for a running job.
struct JobState {
    status: JobStatus,
    cancel_token: CancellationToken,
    event_tx: broadcast::Sender<JobEvent>,
    /// Child process handles for subprocess-based execution
    outstation_child: Option<Child>,
    control_station_child: Option<Child>,
    /// Shared stdin handle for the control station child process.
    /// Wrapped in Arc<Mutex> so both the orchestrator task and manual
    /// send_scenario_command calls can write to it.
    control_station_stdin: Option<Arc<Mutex<ChildStdin>>>,
    /// Stdout/stderr streaming task handles
    io_handles: Vec<JoinHandle<()>>,
    /// Profile path on disk (for cleanup)
    profile_path: String,
    /// Latest emitted status for each process (e.g. "outstation" -> "running").
    /// Replayed verbatim to late-arriving SSE subscribers so they see the
    /// current state immediately on connect. `tokio::sync::broadcast` does
    /// not retain history, so without this snapshot a subscriber that
    /// attaches after `status_update` events have fired sees nothing until
    /// the next state change.
    current_statuses: Arc<StdRwLock<HashMap<String, String>>>,
    /// Bounded ring of recent `log` events (oldest first), capped at
    /// `LOG_BUFFER_CAPACITY`. Populated by a parasitic broadcast subscriber
    /// task spawned in `create_job` and replayed alongside `current_statuses`
    /// on `subscribe_events`. Same fix shape as `current_statuses` but for
    /// the log stream: a fresh job emits dozens of log lines from outstation
    /// and control_station before the frontend's EventSource attaches, and
    /// `tokio::sync::broadcast` does not retain history so those would
    /// otherwise be lost.
    log_buffer: Arc<StdRwLock<VecDeque<JobEvent>>>,
}

pub struct JobService {
    jobs: Arc<Mutex<HashMap<String, JobState>>>,
    data_dir: String,
    /// Pre-loaded scenario definitions, used by the orchestrator to look up
    /// expected tests per scenario.
    scenarios: Arc<Vec<Scenario>>,
    /// The full reference profile (loaded from `profiles/full.json`), used by
    /// the `Unsupported` scenario transform to compute the diff against the
    /// submitted profile. `None` when the service is created without a full
    /// profile (e.g. in unit tests that don't exercise the Unsupported scenario).
    full_profile: Option<Arc<Validated<PicsProfile>>>,
    /// What runs a station's cargo build. Always `RealCompileRunner` outside
    /// tests; see `CompileRunner`.
    compile_runner: Arc<dyn CompileRunner>,
}

impl JobService {
    pub fn new(data_dir: String) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            data_dir,
            scenarios: Arc::new(Vec::new()),
            full_profile: None,
            compile_runner: Arc::new(RealCompileRunner),
        }
    }

    /// Create a JobService with pre-loaded scenario definitions and the full reference profile.
    pub fn with_scenarios(
        data_dir: String,
        scenarios: Arc<Vec<Scenario>>,
        full_profile: Arc<Validated<PicsProfile>>,
    ) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            data_dir,
            scenarios,
            full_profile: Some(full_profile),
            compile_runner: Arc::new(RealCompileRunner),
        }
    }

    /// Test-only: substitute what runs a station's cargo build, so a test
    /// can assert how many builds a station actually started instead of
    /// inferring it from a `status_update` that a later status for the same
    /// process can overwrite before the test observes it.
    #[cfg(test)]
    fn with_compile_runner(mut self, compile_runner: Arc<dyn CompileRunner>) -> Self {
        self.compile_runner = compile_runner;
        self
    }

    pub async fn job_count(&self) -> usize {
        self.jobs.lock().await.len()
    }

    /// Create a new job that spawns outstation and/or control station as child processes.
    ///
    /// Returns the job_id immediately. Process management (compilation, spawning,
    /// port waiting, orchestration) runs as a background tokio task. The frontend
    /// connects to the SSE stream after receiving the job_id and sees events as
    /// processes come up.
    pub async fn create_job(&self, request: CreateJobRequest) -> Result<CreateJobResponse, String> {
        // Single-job enforcement: stop any active job before creating a new one
        {
            let active_job_ids: Vec<String> = {
                let jobs = self.jobs.lock().await;
                jobs.iter()
                    .filter(|(_, state)| {
                        matches!(state.status, JobStatus::Starting | JobStatus::Running)
                    })
                    .map(|(id, _)| id.clone())
                    .collect()
            };
            for old_job_id in active_job_ids {
                info!(old_job_id = %old_job_id, "create_job: stopping existing active job before creating new one");
                let _ = self.stop_job(&old_job_id).await;
            }
        }

        let job_id = Uuid::new_v4().simple().to_string();
        let cancel_token = CancellationToken::new();
        let (event_tx, _) = broadcast::channel::<JobEvent>(EVENT_CHANNEL_CAPACITY);

        info!(
            job_id = %job_id,
            scenario_count = request.scenario_ids.len(),
            scenario_ids = ?request.scenario_ids,
            use_ref_outstation = request.outstation_config.use_reference_outstation,
            use_ref_control_station = request.control_station_config.use_reference_control_station,
            "create_job called"
        );

        // Validate the profile can be parsed (fail fast before spawning background work)
        let parsed_profile: PicsProfile =
            serde_json::from_value(request.profile.clone()).map_err(|e| {
                info!(job_id = %job_id, error = %e, "create_job: profile parse failed");
                format!("Failed to parse profile: {e}")
            })?;
        let validated_profile = PicsProfile::into_validated(parsed_profile).map_err(|e| {
            info!(job_id = %job_id, error = %e, "create_job: profile validation failed");
            format!("Invalid profile: {e}")
        })?;

        // Conditionally adjust base profile bounds ONLY if a scenario requiring headroom is selected
        let mut request = request;
        let base_profile = scenario_generator::prepare_base_profile_for_scenarios(
            validated_profile,
            &request.scenario_ids,
        )
        .map_err(|e| {
            info!(job_id = %job_id, error = %e, "create_job: base profile preparation failed");
            format!("Failed to prepare base profile for scenarios: {e}")
        })?;
        request.profile = serde_json::to_value(&base_profile).unwrap();

        let profile_path = format!("/tmp/mesa-tool_profile_{}.json", job_id);
        // Write the profile to a temporary file for the outstation and control station to read.
        std::fs::write(&profile_path, serde_json::to_string(&base_profile).unwrap()).map_err(|e| {
            info!(job_id = %job_id, error = %e, "create_job: failed to write profile to temporary file");
            format!("Failed to write profile to temporary file: {e}")
        })?;
        info!(job_id = %job_id, "create_job: profile parsed successfully");

        // Store the job state immediately so SSE subscriptions work right away.
        // The background task will update the state as processes come up.
        let current_statuses = Arc::new(StdRwLock::new(HashMap::new()));
        let log_buffer = Arc::new(StdRwLock::new(VecDeque::with_capacity(LOG_BUFFER_CAPACITY)));

        // Spawn the parasitic log-buffering subscriber. It owns its own
        // broadcast::Receiver and writes every `log` event into `log_buffer`,
        // dropping the oldest entry when full. The task ends naturally when
        // `recv()` returns `Closed`, which happens once the last `Sender` is
        // dropped during job cleanup. No explicit shutdown is required.
        spawn_log_buffer_task(job_id.clone(), event_tx.subscribe(), log_buffer.clone());

        let state = JobState {
            status: JobStatus::Starting,
            cancel_token: cancel_token.clone(),
            event_tx: event_tx.clone(),
            outstation_child: None,
            control_station_child: None,
            control_station_stdin: None,
            io_handles: Vec::new(),
            profile_path,
            current_statuses: current_statuses.clone(),
            log_buffer: log_buffer.clone(),
        };

        let response = CreateJobResponse {
            job_id: job_id.clone(),
        };

        self.jobs.lock().await.insert(job_id.clone(), state);

        info!(
            job_id = %response.job_id,
            "create_job: job stored, returning job_id immediately. Background task will spawn processes."
        );

        // Spawn background task for all process management
        let jobs = self.jobs.clone();
        let scenarios = self.scenarios.clone();
        let job_id_bg = job_id.clone();
        let full_profile = self.full_profile.clone();
        let compile_runner = self.compile_runner.clone();

        tokio::spawn(async move {
            run_job_background(
                job_id_bg,
                request,
                cancel_token,
                event_tx,
                jobs,
                scenarios,
                current_statuses,
                full_profile,
                compile_runner,
            )
            .await;
        });

        Ok(response)
    }

    /// Stop a running job by killing child processes and awaiting IO task completion.
    pub async fn stop_job(&self, job_id: &str) -> Result<StopJobResponse, String> {
        let mut jobs = self.jobs.lock().await;
        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| format!("Job '{job_id}' not found"))?;

        // Signal cancellation (stops IO streaming tasks)
        state.cancel_token.cancel();
        state.status = JobStatus::Stopping;

        // Take child processes, shared stdin, and IO handles
        let mut outstation_child = state.outstation_child.take();
        let mut control_station_child = state.control_station_child.take();
        let _cs_stdin = state.control_station_stdin.take(); // drop shared stdin handle
        let io_handles: Vec<JoinHandle<()>> = state.io_handles.drain(..).collect();
        let event_tx = state.event_tx.clone();
        let current_statuses = state.current_statuses.clone();
        let profile_path = state.profile_path.clone();

        // Release lock before awaiting
        drop(jobs);

        // Emit stopping event after we have the statuses handle
        emit_status_update(
            &current_statuses,
            &event_tx,
            job_id,
            "test_runner",
            "stopping",
        );

        // Terminate child processes and emit per-process exit events
        if let Some(ref mut child) = control_station_child {
            info!("Terminating control station child process");
            process_manager::terminate_and_emit(
                child,
                "control_station",
                job_id,
                &event_tx,
                &current_statuses,
            )
            .await;
        }
        if let Some(ref mut child) = outstation_child {
            info!("Terminating outstation child process");
            process_manager::terminate_and_emit(
                child,
                "outstation",
                job_id,
                &event_tx,
                &current_statuses,
            )
            .await;
        }

        // Wait for IO tasks to finish
        for handle in io_handles {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // Clean up profile file
        let _ = std::fs::remove_file(&profile_path);

        // Update status to stopped
        {
            let mut jobs = self.jobs.lock().await;
            if let Some(state) = jobs.get_mut(job_id) {
                state.status = JobStatus::Stopped;
            }
        }
        emit_status_update(
            &current_statuses,
            &event_tx,
            job_id,
            "test_runner",
            "stopped",
        );

        Ok(StopJobResponse {
            message: "Job stopped".to_string(),
        })
    }

    /// Subscribe to the event broadcast channel for SSE streaming.
    ///
    /// Returns a tuple `(replay, receiver)` where `replay` is the catch-up
    /// snapshot delivered before the live stream starts. The replay contains:
    ///   1. The latest `status_update` event for each known process (so pills
    ///      reflect current state immediately on connect).
    ///   2. The bounded ring of recent `log` events in chronological order
    ///      (so the outstation/control_station panes show activity that
    ///      happened before the EventSource attached).
    ///
    /// Status events are emitted before log events; this is fine because the
    /// frontend renders each pane independently and status pills are state,
    /// not a position in a stream.
    ///
    /// `tokio::sync::broadcast` does not retain history, so without this
    /// replay a subscriber attaching mid-flight would miss the first
    /// 1-2 seconds of activity entirely (the bug this fix targets).
    pub async fn subscribe_events(
        &self,
        job_id: &str,
    ) -> Result<(Vec<JobEvent>, broadcast::Receiver<JobEvent>), String> {
        info!(job_id = %job_id, "subscribe_events: new SSE subscriber requested");
        let jobs = self.jobs.lock().await;
        let state = jobs.get(job_id).ok_or_else(|| {
            info!(job_id = %job_id, "subscribe_events: job not found");
            format!("Job '{job_id}' not found")
        })?;
        let receiver_count = state.event_tx.receiver_count();

        // Snapshot current per-process statuses for replay.
        let status_replay: Vec<JobEvent> = {
            let map = state
                .current_statuses
                .read()
                .expect("current_statuses lock poisoned");
            map.iter()
                .map(|(process, status)| make_status_update_event(job_id, process, status))
                .collect()
        };

        // Snapshot the bounded log ring (oldest first).
        let log_replay: Vec<JobEvent> = {
            let buf = state.log_buffer.read().expect("log_buffer lock poisoned");
            buf.iter().cloned().collect()
        };

        let mut replay = Vec::with_capacity(status_replay.len().saturating_add(log_replay.len()));
        replay.extend(status_replay);
        replay.extend(log_replay);

        info!(
            job_id = %job_id,
            existing_receivers = receiver_count,
            replay_events = replay.len(),
            "subscribe_events: subscribing to broadcast channel"
        );
        Ok((replay, state.event_tx.subscribe()))
    }

    /// Get current status for a job.
    pub async fn get_job_status(&self, job_id: &str) -> Result<JobStatusResponse, String> {
        let jobs = self.jobs.lock().await;
        let state = jobs
            .get(job_id)
            .ok_or_else(|| format!("Job '{job_id}' not found"))?;
        Ok(JobStatusResponse {
            job_id: job_id.to_string(),
            status: state.status.clone(),
        })
    }

    /// Get a reference to the data directory path.
    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }
}

impl Default for JobService {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Background task that handles all process management for a job.
///
/// Handles compilation (if needed), spawning outstation and control station,
/// port waiting, and scenario orchestration. Runs in the background while
/// `create_job` returns the job_id immediately.
#[allow(clippy::too_many_arguments)]
async fn run_job_background(
    job_id: String,
    request: CreateJobRequest,
    cancel_token: CancellationToken,
    event_tx: broadcast::Sender<JobEvent>,
    jobs: Arc<Mutex<HashMap<String, JobState>>>,
    scenarios: Arc<Vec<Scenario>>,
    current_statuses: Arc<StdRwLock<HashMap<String, String>>>,
    full_profile: Option<Arc<Validated<PicsProfile>>>,
    compile_runner: Arc<dyn CompileRunner>,
) {
    info!(job_id = %job_id, "run_job_background: starting process management");
    let profile_path = jobs
        .lock()
        .await
        .get(&job_id)
        .map(|state| state.profile_path.clone())
        .unwrap_or_else(|| {
            error!(job_id = %job_id, "run_job_background: profile_path not found in job state");
            String::new()
        });

    emit_status_update(
        &current_statuses,
        &event_tx,
        &job_id,
        "test_runner",
        "starting",
    );

    let raw_outstation_ip = request
        .outstation_config
        .ip_address
        .clone()
        .unwrap_or_else(|| DEFAULT_OUTSTATION_IP.to_string());
    let outstation_ip = process_manager::normalize_loopback(&raw_outstation_ip);
    let outstation_port = request
        .outstation_config
        .port
        .unwrap_or(DEFAULT_OUTSTATION_PORT);

    let use_ref_outstation = request.outstation_config.use_reference_outstation
        || process_manager::is_loopback(&raw_outstation_ip);
    let raw_cs_ip = request
        .control_station_config
        .ip_address
        .clone()
        .unwrap_or_default();
    let use_ref_control_station = request.control_station_config.use_reference_control_station
        || process_manager::is_loopback(&raw_cs_ip);

    let mut io_join_handles: Vec<JoinHandle<()>> = Vec::new();
    let mut outstation_child: Option<Child> = None;
    let mut control_station_child: Option<Child> = None;
    let mut cs_stdin: Option<Arc<Mutex<ChildStdin>>> = None;
    let expected_outcomes = Arc::new(
        scenarios
            .iter()
            .flat_map(|scenario| {
                scenario.expected_tests.iter().map(|expected_test| {
                    (
                        (scenario.id.clone(), expected_test.test_id),
                        expected_test.should_pass,
                    )
                })
            })
            .collect::<HashMap<_, _>>(),
    );

    // Classify both stations' *_BIN env vars before spawning either build:
    // a missing binary for one station must fail the job before a build for
    // the OTHER station starts, or the started build outlives the Failed
    // job (see fail_missing_station_bin).
    let outstation_check = if use_ref_outstation {
        Some(check_station_bin("REFERENCE_OUTSTATION_BIN"))
    } else {
        None
    };
    let control_station_check = if use_ref_control_station {
        Some(check_station_bin("REFERENCE_CONTROL_STATION_BIN"))
    } else {
        None
    };

    if let Some(StationBinCheck::Missing { path }) = &outstation_check {
        fail_missing_station_bin(
            &current_statuses,
            &event_tx,
            &jobs,
            &job_id,
            "outstation",
            "REFERENCE_OUTSTATION_BIN",
            path,
        )
        .await;
        return;
    }
    if let Some(StationBinCheck::Missing { path }) = &control_station_check {
        fail_missing_station_bin(
            &current_statuses,
            &event_tx,
            &jobs,
            &job_id,
            "control_station",
            "REFERENCE_CONTROL_STATION_BIN",
            path,
        )
        .await;
        return;
    }

    // process_manager::find_binary checks the same env var first (see its
    // doc comment), so when it is set to an existing file, that is the path
    // that will actually be spawned and a cargo build of it would be
    // redundant.
    let outstation_compile = match outstation_check.map(decide_station_compile) {
        Some(StationCompileDecision::Skip) => {
            info!(job_id = %job_id, "run_job_background: REFERENCE_OUTSTATION_BIN set, skipping cargo build");
            Some(CompileStep::AlreadyBuilt)
        }
        Some(StationCompileDecision::Build) => {
            emit_status_update(
                &current_statuses,
                &event_tx,
                &job_id,
                "outstation",
                "compiling",
            );
            info!(job_id = %job_id, "run_job_background: compiling outstation");
            let job_id = job_id.clone();
            let event_tx = event_tx.clone();
            let compile_runner = compile_runner.clone();
            Some(CompileStep::Building(tokio::spawn(async move {
                compile_runner
                    .compile(
                        &job_id,
                        "reference-outstation",
                        "reference-outstation",
                        &event_tx,
                    )
                    .await
            })))
        }
        Some(StationCompileDecision::Fail { .. }) => {
            unreachable!("Missing is filtered out and returned above")
        }
        None => None,
    };

    let control_station_compile = match control_station_check.map(decide_station_compile) {
        Some(StationCompileDecision::Skip) => {
            info!(job_id = %job_id, "run_job_background: REFERENCE_CONTROL_STATION_BIN set, skipping cargo build");
            Some(CompileStep::AlreadyBuilt)
        }
        Some(StationCompileDecision::Build) => {
            emit_status_update(
                &current_statuses,
                &event_tx,
                &job_id,
                "control_station",
                "compiling",
            );
            info!(job_id = %job_id, "run_job_background: compiling control station");
            let job_id = job_id.clone();
            let event_tx = event_tx.clone();
            let compile_runner = compile_runner.clone();
            Some(CompileStep::Building(tokio::spawn(async move {
                compile_runner
                    .compile(
                        &job_id,
                        "reference-control-station",
                        "reference-control-station",
                        &event_tx,
                    )
                    .await
            })))
        }
        Some(StationCompileDecision::Fail { .. }) => {
            unreachable!("Missing is filtered out and returned above")
        }
        None => None,
    };

    // Wait for the outstation compile, then spawn it. `AlreadyBuilt` means
    // REFERENCE_OUTSTATION_BIN named an existing file, so nothing was
    // compiled and there is nothing to await.
    if let Some(compile) = outstation_compile {
        let compile_ok = match compile {
            CompileStep::AlreadyBuilt => true,
            CompileStep::Building(handle) => handle.await.unwrap_or(false),
        };
        if !compile_ok {
            emit_status_update(&current_statuses, &event_tx, &job_id, "outstation", "error");
            update_job_status(&jobs, &job_id, JobStatus::Failed).await;
            return;
        }

        emit_status_update(
            &current_statuses,
            &event_tx,
            &job_id,
            "outstation",
            "starting",
        );

        let local_addr = format!("{outstation_ip}:{outstation_port}");
        let forward_outstation_logs = request.device_under_test != DeviceUnderTest::Outstation;
        match process_manager::spawn_outstation(
            &job_id,
            &local_addr,
            &profile_path,
            event_tx.clone(),
            cancel_token.clone(),
            forward_outstation_logs,
            expected_outcomes.clone(),
        ) {
            Ok((child, stdout_h, stderr_h)) => {
                emit_status_update(
                    &current_statuses,
                    &event_tx,
                    &job_id,
                    "outstation",
                    "running",
                );
                outstation_child = Some(child);
                io_join_handles.push(stdout_h);
                io_join_handles.push(stderr_h);
            }
            Err(e) => {
                let _ = event_tx.send(make_event(
                    "log",
                    "test_runner",
                    &job_id,
                    serde_json::json!({"message": format!("Failed to spawn outstation: {e}")}),
                ));
                update_job_status(&jobs, &job_id, JobStatus::Failed).await;
                return;
            }
        }
    }

    // Wait for the control-station compile, then spawn it. `AlreadyBuilt`
    // means REFERENCE_CONTROL_STATION_BIN named an existing file, so
    // nothing was compiled and there is nothing to await.
    if let Some(compile) = control_station_compile {
        let compile_ok = match compile {
            CompileStep::AlreadyBuilt => true,
            CompileStep::Building(handle) => handle.await.unwrap_or(false),
        };
        if !compile_ok {
            emit_status_update(
                &current_statuses,
                &event_tx,
                &job_id,
                "control_station",
                "error",
            );
            if let Some(mut child) = outstation_child.take() {
                process_manager::terminate_child(&mut child, Duration::from_secs(5)).await;
            }
            update_job_status(&jobs, &job_id, JobStatus::Failed).await;
            return;
        }

        if use_ref_outstation {
            let _ = event_tx.send(make_event("log", "test_runner", &job_id,
                serde_json::json!({"message": format!("Waiting for outstation at {outstation_ip}:{outstation_port}...")})));
            let port_result = process_manager::wait_for_tcp_port_with_child(
                &outstation_ip,
                outstation_port,
                Duration::from_secs(OUTSTATION_STARTUP_WAIT_SECS),
                outstation_child.as_mut(),
            )
            .await;
            match &port_result {
                process_manager::PortWaitResult::Ready => {
                    info!(job_id = %job_id, "run_job_background: outstation port is ready");
                }
                process_manager::PortWaitResult::ChildExited(code) => {
                    let msg = format!(
                        "Outstation exited with code {:?} before port was available.",
                        code
                    );
                    let _ = event_tx.send(make_event(
                        "log",
                        "test_runner",
                        &job_id,
                        serde_json::json!({"message": msg}),
                    ));
                    if let Some(mut child) = outstation_child.take() {
                        process_manager::terminate_child(&mut child, Duration::from_secs(5)).await;
                    }
                    update_job_status(&jobs, &job_id, JobStatus::Failed).await;
                    return;
                }
                process_manager::PortWaitResult::Timeout => {
                    let msg = format!(
                        "Timed out waiting for outstation at {outstation_ip}:{outstation_port}"
                    );
                    let _ = event_tx.send(make_event(
                        "log",
                        "test_runner",
                        &job_id,
                        serde_json::json!({"message": msg}),
                    ));
                    if let Some(mut child) = outstation_child.take() {
                        process_manager::terminate_child(&mut child, Duration::from_secs(5)).await;
                    }
                    update_job_status(&jobs, &job_id, JobStatus::Failed).await;
                    return;
                }
            }
        }

        emit_status_update(
            &current_statuses,
            &event_tx,
            &job_id,
            "control_station",
            "starting",
        );
        let forward_cs_logs = request.device_under_test != DeviceUnderTest::ControlStation;
        match process_manager::spawn_control_station(
            &job_id,
            &outstation_ip,
            outstation_port,
            profile_path.as_str(),
            event_tx.clone(),
            cancel_token.clone(),
            forward_cs_logs,
            expected_outcomes,
        ) {
            Ok((mut child, stdout_h, stderr_h)) => {
                emit_status_update(
                    &current_statuses,
                    &event_tx,
                    &job_id,
                    "control_station",
                    "running",
                );
                let child_stdin = child.stdin.take();
                control_station_child = Some(child);
                io_join_handles.push(stdout_h);
                io_join_handles.push(stderr_h);

                if let Some(stdin) = child_stdin {
                    cs_stdin = Some(Arc::new(Mutex::new(stdin)));
                }
            }
            Err(e) => {
                let _ = event_tx.send(make_event(
                    "log",
                    "test_runner",
                    &job_id,
                    serde_json::json!({"message": format!("Failed to spawn control station: {e}")}),
                ));
                if let Some(mut child) = outstation_child.take() {
                    process_manager::terminate_child(&mut child, Duration::from_secs(5)).await;
                }
                update_job_status(&jobs, &job_id, JobStatus::Failed).await;
                return;
            }
        }
    }

    // Update job state with spawned processes and transition to Running
    {
        let mut job_lock = jobs.lock().await;
        if let Some(state) = job_lock.get_mut(&job_id) {
            state.status = JobStatus::Running;
            state.outstation_child = outstation_child;
            state.control_station_child = control_station_child;
            state.control_station_stdin = cs_stdin.clone();
        }
    }

    emit_status_update(
        &current_statuses,
        &event_tx,
        &job_id,
        "test_runner",
        "running",
    );

    if !request.scenario_ids.is_empty() {
        if let Some(ref stdin_handle) = cs_stdin {
            let orchestrator_handle = run_all_scenarios_new_thread(
                job_id.clone(),
                request.scenario_ids.clone(),
                request.profile.clone(),
                stdin_handle.clone(),
                event_tx.clone(),
                cancel_token.clone(),
                jobs.clone(),
                scenarios.clone(),
                current_statuses.clone(),
                full_profile.clone(),
            );
            io_join_handles.push(orchestrator_handle);
        }
    }

    // Store IO handles in the job state
    {
        let mut job_lock = jobs.lock().await;
        if let Some(state) = job_lock.get_mut(&job_id) {
            state.io_handles = io_join_handles;
        }
    }
}

/// Update a job's status in the shared state map.
async fn update_job_status(
    jobs: &Arc<Mutex<HashMap<String, JobState>>>,
    job_id: &str,
    status: JobStatus,
) {
    let mut job_lock = jobs.lock().await;
    if let Some(state) = job_lock.get_mut(job_id) {
        state.status = status;
    }
}

/// Outcome of deciding whether a station's `*_BIN` env var lets us skip
/// `cargo build` for it.
///
/// Mirrors `process_manager::find_binary`'s env-var-first lookup order (see
/// its doc comment): that function returns the env var's value whenever the
/// var is set, regardless of whether a file exists there. So `Configured`
/// here, which requires the file to exist, always names the same path
/// `find_binary` will hand to the spawned process; and `Missing` catches the
/// one case `find_binary` does not check for itself, an env var pointing at
/// nothing.
#[derive(Debug, PartialEq, Eq)]
enum StationBinCheck {
    /// The env var is not set; compile as usual.
    NotConfigured,
    /// The env var names a file that exists; no build needed.
    Configured,
    /// The env var names a file that does not exist.
    Missing { path: String },
}

/// Classify a station's env var reading, without touching process
/// environment state. Kept separate from `check_station_bin` so the
/// decision logic is testable without mutating a process-global env var.
fn classify_station_bin(raw_value: Option<String>) -> StationBinCheck {
    match raw_value {
        Some(path) => {
            if std::path::Path::new(&path).is_file() {
                StationBinCheck::Configured
            } else {
                StationBinCheck::Missing { path }
            }
        }
        None => StationBinCheck::NotConfigured,
    }
}

/// Read `env_var` and classify it per `StationBinCheck`.
fn check_station_bin(env_var: &str) -> StationBinCheck {
    classify_station_bin(std::env::var(env_var).ok())
}

/// What `run_job_background` should do for a station, given its `*_BIN`
/// classification. Pure mapping from `StationBinCheck`, so the compile
/// decision is unit-testable without spawning `cargo` or touching the
/// process environment.
#[derive(Debug, PartialEq, Eq)]
enum StationCompileDecision {
    /// The env var names a file that does not exist; fail the job.
    Fail { path: String },
    /// The env var names an existing file; nothing to build.
    Skip,
    /// The env var is unset; run `cargo build` for the station.
    Build,
}

fn decide_station_compile(check: StationBinCheck) -> StationCompileDecision {
    match check {
        StationBinCheck::Missing { path } => StationCompileDecision::Fail { path },
        StationBinCheck::Configured => StationCompileDecision::Skip,
        StationBinCheck::NotConfigured => StationCompileDecision::Build,
    }
}

/// Fail a job because a `*_BIN` env var names a file that does not exist,
/// before any station has been spawned. Emits a `status_update` so the UI's
/// station pill leaves "starting" instead of hanging there with only a log
/// line as the signal, matching the compile-failure path's `<station> error`.
async fn fail_missing_station_bin(
    current_statuses: &Arc<StdRwLock<HashMap<String, String>>>,
    event_tx: &broadcast::Sender<JobEvent>,
    jobs: &Arc<Mutex<HashMap<String, JobState>>>,
    job_id: &str,
    station_label: &str,
    env_var: &str,
    path: &str,
) {
    let msg = format!("{env_var} names a file that does not exist: {path}");
    warn!(job_id = %job_id, env_var, path, "run_job_background: station bin missing");
    let _ = event_tx.send(make_event(
        "log",
        "test_runner",
        job_id,
        serde_json::json!({"message": msg}),
    ));
    emit_status_update(current_statuses, event_tx, job_id, station_label, "error");
    update_job_status(jobs, job_id, JobStatus::Failed).await;
}

/// Whether a station's compile step already finished (no build needed) or is
/// still running in the background as a spawned `cargo build`.
enum CompileStep {
    /// A `*_BIN` env var named an existing file; nothing was compiled.
    AlreadyBuilt,
    /// `cargo build` is running in a spawned task.
    Building(JoinHandle<bool>),
}

/// What actually builds a station's binary when `decide_station_compile`
/// says `Build`. Production always uses `RealCompileRunner`, whose `compile`
/// is `compile_cargo_package` unchanged. Tests substitute a runner that
/// records each invocation instead of spawning a subprocess, so "how many
/// builds did this station start" is a fact a test can assert directly
/// rather than infer from a `status_update` that a later status for the
/// same process silently overwrites in `current_statuses`.
trait CompileRunner: Send + Sync {
    fn compile<'a>(
        &'a self,
        job_id: &'a str,
        package_name: &'a str,
        display_name: &'a str,
        event_tx: &'a broadcast::Sender<JobEvent>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

struct RealCompileRunner;

impl CompileRunner for RealCompileRunner {
    fn compile<'a>(
        &'a self,
        job_id: &'a str,
        package_name: &'a str,
        display_name: &'a str,
        event_tx: &'a broadcast::Sender<JobEvent>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(compile_cargo_package(
            job_id,
            package_name,
            display_name,
            event_tx,
        ))
    }
}

/// Attempt to compile a cargo package by name.
///
/// Emits "compiling" log events and returns true if compilation succeeded.
async fn compile_cargo_package(
    job_id: &str,
    package_name: &str,
    display_name: &str,
    event_tx: &broadcast::Sender<JobEvent>,
) -> bool {
    let _ = event_tx.send(make_event(
        "log",
        "test_runner",
        job_id,
        serde_json::json!({"message": format!("Compiling {display_name}...")}),
    ));

    // Redirect stdout/stderr to null. Piping without reading would fill the
    // OS pipe buffer on a noisy build and hang the child indefinitely. We
    // currently don't surface build output; if we want it later, switch to
    // .output() which reads the pipes to completion.
    let result = tokio::process::Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg(package_name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .map(|output| output.status);

    match result {
        Ok(status) if status.success() => {
            let _ = event_tx.send(make_event(
                "log",
                "test_runner",
                job_id,
                serde_json::json!({"message": format!("{display_name} compiled successfully.")}),
            ));
            true
        }
        Ok(status) => {
            let _ = event_tx.send(make_event("log", "test_runner", job_id,
                serde_json::json!({"message": format!("Compilation of {display_name} failed with code {:?}.", status.code())})));
            false
        }
        Err(e) => {
            let _ = event_tx.send(make_event(
                "log",
                "test_runner",
                job_id,
                serde_json::json!({"message": format!("Failed to compile {display_name}: {e}")}),
            ));
            false
        }
    }
}

/// Spawn a tokio task that iterates through scenarios sequentially,
/// Apply the scenario-specific profile transform.
///
/// Deserializes the profile JSON into a `PicsProfile`, applies the transform for the
/// given scenario, and serializes the result back to a `serde_json::Value`.
///
/// Returns `Err` with a human-readable message on any of the three failure modes:
/// unknown scenario ID, deserialization failure, or transform failure.
fn apply_scenario_transform(
    scenario_id: &str,
    profile: serde_json::Value,
    full_profile: Option<&Validated<PicsProfile>>,
) -> Result<serde_json::Value, String> {
    let sid = scenario_id
        .parse::<scenario_generator::ScenarioId>()
        .map_err(|e| format!("Unknown scenario id: {e}"))?;

    let pics_profile: PicsProfile = serde_json::from_value(profile)
        .map_err(|e| format!("Failed to deserialize profile for transform: {e}"))?;
    let pics_profile = PicsProfile::into_validated(pics_profile)
        .map_err(|e| format!("Failed to normalize profile: {e}"))?;

    // full_profile is only used by the Unsupported scenario. For all other scenarios
    // it is passed through but never read, so we fall back to the submitted profile
    // itself as a harmless stand-in when full_profile has not been loaded.
    let fallback;
    let full = match full_profile {
        Some(fp) => fp,
        None => {
            if sid == scenario_generator::ScenarioId::Unsupported {
                return Err(
                    "Unsupported scenario requires the full reference profile to be loaded"
                        .to_string(),
                );
            }
            fallback = pics_profile.clone();
            &fallback
        }
    };

    let updated = scenario_generator::update_profile(&sid, pics_profile, full)?;

    serde_json::to_value(updated)
        .map_err(|e| format!("Failed to serialize transformed profile: {e}"))
}

/// Run the scenarios for a job in the sequence in `scenario_ids`.
///
/// Send each scenario to the control station through standard input. Before the
/// next scenario starts, wait for all expected conformance test results.
#[allow(clippy::too_many_arguments)]
fn run_all_scenarios_new_thread(
    job_id: String,
    scenario_ids: Vec<String>,
    profile: serde_json::Value,
    cs_stdin: Arc<Mutex<ChildStdin>>,
    event_tx: broadcast::Sender<JobEvent>,
    cancel_token: CancellationToken,
    jobs: Arc<Mutex<HashMap<String, JobState>>>,
    scenarios: Arc<Vec<Scenario>>,
    current_statuses: Arc<StdRwLock<HashMap<String, String>>>,
    full_profile: Option<Arc<Validated<PicsProfile>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(job_id = %job_id, scenario_count = scenario_ids.len(), "orchestrator: starting scenario loop");

        for scenario_id in &scenario_ids {
            let scenario_span = span!(Level::INFO, "scenario_loop", scenario_id = %scenario_id);
            // TODO: Move the logic for each scenario to a separate function.
            if cancel_token.is_cancelled() {
                info!(job_id = %job_id, "orchestrator: cancelled, stopping scenario loop");
                return;
            }

            // Look up scenario metadata to find expected tests
            let scenario_meta = scenarios.iter().find(|s| &s.id == scenario_id);
            let scenario_name = scenario_meta
                .map(|s| s.name.clone())
                .unwrap_or_else(|| scenario_id.clone());
            let expected_test_ids: Vec<TestName> = scenario_meta
                .map(|s| s.expected_tests.iter().map(|t| t.test_id).collect())
                .unwrap_or_default();

            info!(
                job_id = %job_id,
                scenario_id = %scenario_id,
                scenario_name = %scenario_name,
                expected_test_count = expected_test_ids.len(),
                "orchestrator: starting scenario"
            );

            // Emit "starting scenario" event
            let _ = event_tx.send(make_event(
                "scenario_status",
                "test_runner",
                &job_id,
                serde_json::json!({
                    "event_type": "scenario_status",
                    "status": "starting",
                    "scenario_id": scenario_id,
                    "scenario_name": scenario_name,
                }),
            ));

            // Apply scenario-specific profile transform.
            // Note: this round-trips through PicsProfile (JSON -> struct -> transform -> JSON).
            // Any JSON fields not modelled in PicsProfile will be silently dropped.
            let transformed_profile = match apply_scenario_transform(
                scenario_id,
                profile.clone(),
                full_profile.as_deref(),
            ) {
                Ok(v) => v,
                Err(e) => {
                    error!(job_id = %job_id, scenario_id = %scenario_id, error = %e, "orchestrator: profile transform failed, skipping scenario");
                    let _ = event_tx.send(make_event(
                        "scenario_status",
                        "test_runner",
                        &job_id,
                        serde_json::json!({
                            "status": "error",
                            "scenario_id": scenario_id,
                            "scenario_name": scenario_name,
                            "message": e,
                        }),
                    ));
                    continue;
                }
            };
            let profile_json = match serde_json::to_string(&transformed_profile) {
                Ok(json) => json,
                Err(e) => {
                    warn!(job_id = %job_id, scenario_id = %scenario_id, error = %e, "orchestrator: failed to serialize transformed profile");
                    let _ = event_tx.send(make_event(
                        "scenario_status",
                        "test_runner",
                        &job_id,
                        serde_json::json!({
                            "event_type": "scenario_status",
                            "status": "error",
                            "scenario_id": scenario_id,
                            "scenario_name": scenario_name,
                            "message": format!("Failed to serialize profile: {e}"),
                        }),
                    ));
                    continue;
                }
            };

            // Send the new_scenario command via stdin
            let command = format!("new_scenario {scenario_id} {profile_json}");
            if let Err(e) = process_manager::send_stdin_command_shared(&cs_stdin, &command)
                .instrument(scenario_span.clone())
                .await
            {
                warn!(job_id = %job_id, scenario_id = %scenario_id, error = %e, "orchestrator: failed to send scenario command");
                let _ = event_tx.send(make_event(
                    "scenario_status",
                    "test_runner",
                    &job_id,
                    serde_json::json!({
                        "event_type": "scenario_status",
                        "status": "error",
                        "scenario_id": scenario_id,
                        "scenario_name": scenario_name,
                        "message": format!("Failed to send scenario command: {e}"),
                    }),
                ));
                continue;
            }

            info!(job_id = %job_id, scenario_id = %scenario_id, "orchestrator: scenario command sent, waiting for results");

            // Wait for conformance results from the broadcast channel
            if expected_test_ids.is_empty() {
                // No expected tests for this scenario. The control station processes
                // the command synchronously, so give it a brief moment to finish.
                info!(job_id = %job_id, scenario_id = %scenario_id, "orchestrator: no expected tests, using brief delay");
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!(job_id = %job_id, scenario_id = %scenario_id, "orchestrator: cancelled during scenario delay");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(2))
                        .instrument(scenario_span.clone()) => {}
                }
            } else {
                // Collect conformance results until each expected test has a result.
                let mut rx = event_tx.subscribe();
                let mut test_results: HashMap<TestName, Option<bool>> = expected_test_ids
                    .iter()
                    .map(|test_id| (*test_id, None))
                    .collect();

                let mut scenario_cancelled = false;
                let wait_result = tokio::time::timeout(
                    Duration::from_secs(SCENARIO_TIMEOUT_SECS),
                    async {
                        loop {
                            if test_results.values().all(|test_result| test_result.is_some()) {
                                break;
                            }

                            tokio::select! {
                                _ = cancel_token.cancelled() => {
                                    info!(job_id = %job_id, scenario_id = %scenario_id, "orchestrator: cancelled while waiting for results");
                                    scenario_cancelled = true;
                                    return;
                                }
                                event_result = rx.recv() => {
                                    match event_result {
                                        Ok(event) => {
                                            if event.event_type == "conformance_test_result" {
                                                let message = &event.message;
                                                let event_scenario_id = message.get("scenario_id")
                                                    .and_then(|value| value.as_str())
                                                    .unwrap_or("");
                                                if event_scenario_id != scenario_id.as_str() {
                                                    continue;
                                                }
                                                let test_id: TestName = match message.get("test")
                                                    .and_then(|value| value.as_str())
                                                    .and_then(|s| TestName::try_from(s).ok())
                                                    .ok_or_else(|| serde_json::Error::custom("Failed to parse test name")) {
                                                    Ok(test_id) => test_id,
                                                    Err(e) => {
                                                        warn!(job_id = %job_id, scenario_id = %scenario_id, error = %e, "orchestrator: failed to parse test name from conformance result");
                                                        continue;
                                                    }
                                                };

                                                let test_passed = message.get("passed")
                                                    .and_then(|value| value.as_bool())
                                                    .unwrap_or(false);

                                                if let Some(test_result) = test_results.get_mut(&test_id) {
                                                    if test_result.is_none() {
                                                        *test_result = Some(test_passed);
                                                        let resolved_test_count = test_results
                                                            .values()
                                                            .filter(|test_result| test_result.is_some())
                                                            .count();
                                                        debug!(
                                                            job_id = %job_id,
                                                            scenario_id = %scenario_id,
                                                            test = %test_id,
                                                            pass = test_passed,
                                                            resolved = resolved_test_count,
                                                            total = test_results.len(),
                                                            "orchestrator: conformance result received"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        Err(broadcast::error::RecvError::Lagged(missed_message_count)) => {
                                            warn!(job_id = %job_id, lagged = missed_message_count, "orchestrator: broadcast receiver lagged");
                                        }
                                        Err(broadcast::error::RecvError::Closed) => {
                                            warn!(job_id = %job_id, "orchestrator: broadcast channel closed");
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    },
                )
                .instrument(scenario_span.clone())
                .await;

                if scenario_cancelled {
                    return;
                }

                if wait_result.is_err() {
                    let unresolved_test_ids: Vec<&TestName> = test_results
                        .iter()
                        .filter(|(_, test_result)| test_result.is_none())
                        .map(|(test_id, _)| test_id)
                        .collect();
                    warn!(
                        job_id = %job_id,
                        scenario_id = %scenario_id,
                        unresolved_count = unresolved_test_ids.len(),
                        "orchestrator: timed out waiting for test results"
                    );
                    let _ = event_tx.send(make_event(
                        "scenario_status",
                        "test_runner",
                        &job_id,
                        serde_json::json!({
                            "event_type": "scenario_status",
                            "status": "timeout",
                            "scenario_id": scenario_id,
                            "scenario_name": scenario_name,
                            "message": format!("Did not receive {} test result(s) before the timeout", unresolved_test_ids.len()),
                        }),
                    ));
                }
            }

            if cancel_token.is_cancelled() {
                info!(job_id = %job_id, scenario_id = %scenario_id, "orchestrator: cancelled before completing scenario");
                return;
            }

            // Emit "scenario complete" event
            let _ = event_tx.send(make_event(
                "scenario_status",
                "test_runner",
                &job_id,
                serde_json::json!({
                    "event_type": "scenario_status",
                    "status": "complete",
                    "scenario_id": scenario_id,
                    "scenario_name": scenario_name,
                }),
            ));

            info!(job_id = %job_id, scenario_id = %scenario_id, "orchestrator: scenario complete");
        }

        // All scenarios done
        let _ = event_tx.send(make_event(
            "scenario_status",
            "test_runner",
            &job_id,
            serde_json::json!({
                "event_type": "scenario_status",
                "status": "all_complete",
                "message": "All scenarios complete",
            }),
        ));

        info!(job_id = %job_id, "orchestrator: all scenarios complete, cleaning up child processes");

        // Take child processes and clean up.
        // IMPORTANT: Do NOT cancel the token here. Cancelling before sending
        // the "finished" event creates a race where stream_output tasks exit
        // before the final events can be delivered. Instead, terminate the
        // child processes directly. Their stdout/stderr will EOF, which
        // naturally ends the stream_output tasks.
        let mut job_lock = jobs.lock().await;
        if let Some(state) = job_lock.get_mut(&job_id) {
            // Take child processes out of the state for termination
            let mut outstation_child = state.outstation_child.take();
            let mut control_station_child = state.control_station_child.take();
            let _cs_stdin = state.control_station_stdin.take(); // drop stdin handle
            let io_handles: Vec<JoinHandle<()>> = state.io_handles.drain(..).collect();
            let profile_path = state.profile_path.clone();

            // Release lock before awaiting async termination
            drop(job_lock);

            // Terminate child processes and emit per-process exit events
            if let Some(ref mut child) = control_station_child {
                info!(job_id = %job_id, "orchestrator: terminating control station child process");
                process_manager::terminate_and_emit(
                    child,
                    "control_station",
                    &job_id,
                    &event_tx,
                    &current_statuses,
                )
                .await;
            }
            if let Some(ref mut child) = outstation_child {
                info!(job_id = %job_id, "orchestrator: terminating outstation child process");
                process_manager::terminate_and_emit(
                    child,
                    "outstation",
                    &job_id,
                    &event_tx,
                    &current_statuses,
                )
                .await;
            }

            // Now cancel the token so stream_output tasks that haven't
            // already exited via EOF will stop promptly.
            cancel_token.cancel();

            // Wait for IO tasks to finish
            for handle in io_handles {
                match tokio::time::timeout(Duration::from_secs(5), handle).await {
                    Ok(_) => {}
                    Err(_) => {
                        warn!(job_id = %job_id, "orchestrator: IO task did not finish within timeout");
                    }
                }
            }

            // Clean up profile temp file
            let _ = std::fs::remove_file(&profile_path);
            info!(job_id = %job_id, "orchestrator: cleaned up temp profile file");

            // Update status to completed and send the "finished" event
            // BEFORE dropping the sender.
            {
                let mut job_lock = jobs.lock().await;
                if let Some(state) = job_lock.get_mut(&job_id) {
                    state.status = JobStatus::Finished;
                }
            }
            emit_status_update(
                &current_statuses,
                &event_tx,
                &job_id,
                "test_runner",
                "finished",
            );

            info!(job_id = %job_id, "orchestrator: finished event sent, waiting briefly before closing channel");

            // Give the SSE client time to receive the "finished" event
            // before we drop the broadcast sender.
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Remove the job from the HashMap. This drops the last
            // broadcast::Sender held in JobState, which closes the
            // BroadcastStream and causes the SSE connection to end.
            // The orchestrator's own `event_tx` clone is also about to
            // be dropped when this function returns.
            {
                let mut job_lock = jobs.lock().await;
                job_lock.remove(&job_id);
            }

            // Drop our clone of event_tx explicitly (it will drop at end
            // of scope anyway, but being explicit about the intent).
            drop(event_tx);

            info!(job_id = %job_id, "orchestrator: cleanup complete, job removed from state map");
        } else {
            info!(job_id = %job_id, "orchestrator: job not found in state map during cleanup (may have been stopped externally)");
        }
    })
}

/// Record the latest status for `process` and broadcast a `status_update` event.
///
/// The bookkeeping happens BEFORE the broadcast so a subscriber that calls
/// `subscribe_events` between the write and the send still sees the new
/// status via replay. Using a `std::sync::RwLock` (not the async one) keeps
/// the critical section small and lets us avoid holding any lock across the
/// broadcast send.
pub(crate) fn emit_status_update(
    current_statuses: &Arc<StdRwLock<HashMap<String, String>>>,
    event_tx: &broadcast::Sender<JobEvent>,
    job_id: &str,
    process: &str,
    status: &str,
) {
    {
        let mut map = current_statuses
            .write()
            .expect("current_statuses lock poisoned");
        map.insert(process.to_string(), status.to_string());
    }
    let _ = event_tx.send(make_status_update_event(job_id, process, status));
}

/// Build a `status_update` `JobEvent` with the canonical payload shape.
///
/// Centralizing the shape here keeps `emit_status_update` (live broadcast) and
/// `subscribe_events` (replay snapshot reconstruction from `current_statuses`)
/// in lockstep: any change to the event keys, source, or payload format only
/// needs to happen in one place.
fn make_status_update_event(job_id: &str, process: &str, status: &str) -> JobEvent {
    make_event(
        "status_update",
        "test_runner",
        job_id,
        serde_json::json!({"status": status, "process": process}),
    )
}

/// Spawn a parasitic broadcast subscriber that captures `log` events into a
/// bounded ring buffer for late-subscriber replay.
///
/// Using a parasitic subscriber rather than threading an `emit_log` helper
/// through every emission site keeps the diff tight: a dozen call sites
/// already invoke `event_tx.send(make_event("log", ...))` directly, and we do
/// not want to touch them. The downside is one extra `broadcast::Receiver`
/// per job that has to keep up with traffic; with a 256-slot channel and a
/// 500-slot ring, lag is unlikely except under sustained pathological log
/// rates. On lag we just skip the missed events; the next live event picked
/// up by an SSE subscriber will be correct.
///
/// The task ends when `recv()` returns `Closed`, which happens once the
/// last `Sender` is dropped during job cleanup. At that point the `Arc`
/// holding the `log_buffer` is dropped along with `JobState`.
fn spawn_log_buffer_task(
    job_id: String,
    mut rx: broadcast::Receiver<JobEvent>,
    log_buffer: Arc<StdRwLock<VecDeque<JobEvent>>>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if event.event_type != "log" {
                        continue;
                    }
                    let mut buf = log_buffer.write().expect("log_buffer lock poisoned");
                    if buf.len() >= LOG_BUFFER_CAPACITY {
                        buf.pop_front();
                    }
                    buf.push_back(event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(
                        job_id = %job_id,
                        lagged = n,
                        "log_buffer_task: broadcast receiver lagged, dropped log events will not be replayable"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!(job_id = %job_id, "log_buffer_task: broadcast channel closed, exiting");
                    return;
                }
            }
        }
    });
}

/// Helper to construct a JobEvent with the current timestamp.
/// Public within the crate so process_manager can use it to create events.
pub(crate) fn make_event(
    event_type: &str,
    source: &str, // TODO: make this an enum
    job_id: &str,
    message: serde_json::Value,
) -> JobEvent {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    // Inject event_type into the message payload so the frontend's type guards
    // (isStatusMessage, isConformanceTestResult, etc.) can check message.event_type.
    let message = if let serde_json::Value::Object(mut map) = message {
        map.entry("event_type".to_string())
            .or_insert_with(|| serde_json::Value::String(event_type.to_string()));
        serde_json::Value::Object(map)
    } else {
        message
    };
    JobEvent {
        event_type: event_type.to_string(),
        source: source.to_string(),
        job_id: job_id.to_string(),
        message,
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::job::DeviceUnderTest;
    use crate::services::test_env_lock;
    use crate::test_helpers::test_service;

    // -- Constructor tests --

    #[tokio::test]
    async fn test_new_service_has_no_jobs() {
        let service = test_service();
        assert_eq!(service.job_count().await, 0);
    }

    #[tokio::test]
    async fn test_new_service_stores_data_dir() {
        let service = JobService::new("/some/path".to_string());
        assert_eq!(service.data_dir(), "/some/path");
    }

    #[tokio::test]
    async fn test_default_service_has_empty_data_dir() {
        let service = JobService::default();
        assert_eq!(service.data_dir(), "");
    }

    // -- stop_job error paths --

    #[tokio::test]
    async fn test_stop_nonexistent_job_returns_error() {
        let service = test_service();
        let result = service.stop_job("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // -- subscribe_events error paths --

    #[tokio::test]
    async fn test_subscribe_nonexistent_job_returns_error() {
        let service = test_service();
        let result = service.subscribe_events("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // -- get_job_status error paths --

    #[tokio::test]
    async fn test_get_status_nonexistent_job_returns_error() {
        let service = test_service();
        let result = service.get_job_status("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // -- create_job with invalid profile --

    #[tokio::test]
    async fn test_create_job_invalid_profile_returns_error() {
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: None,
                port: None,
            },
            profile: serde_json::json!("not a valid profile"),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };
        let result = service.create_job(request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse profile"));
    }

    // -- create_job with no spawns (external DUT) --

    #[tokio::test]
    async fn test_create_job_no_reference_spawns_nothing() {
        // When both use_reference flags are false, no tasks are spawned
        // but the job is created with Running status
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: Some("10.0.0.1".to_string()),
                port: Some(20000),
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: Some("10.0.0.2".to_string()),
                port: Some(20001),
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let result = service.create_job(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.job_id.is_empty());
        assert_eq!(service.job_count().await, 1);
    }

    #[tokio::test]
    async fn test_create_job_generates_unique_ids() {
        let service = test_service();
        let make_request = || CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: None,
                port: None,
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let r1 = service.create_job(make_request()).await.unwrap();
        let r2 = service.create_job(make_request()).await.unwrap();
        assert_ne!(r1.job_id, r2.job_id);
        // First job was stopped by single-job enforcement but remains in the map
        assert_eq!(service.job_count().await, 2);
        // First job should now be stopped
        let status = service.get_job_status(&r1.job_id).await.unwrap();
        assert_eq!(status.status, JobStatus::Stopped);
    }

    #[tokio::test]
    async fn test_get_status_after_create() {
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: None,
                port: None,
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let created = service.create_job(request).await.unwrap();
        let status = service.get_job_status(&created.job_id).await.unwrap();
        // Job starts in Starting status since background task manages lifecycle
        assert!(status.status == JobStatus::Starting || status.status == JobStatus::Running);
        assert_eq!(status.job_id, created.job_id);
    }

    #[tokio::test]
    async fn test_subscribe_events_after_create() {
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: None,
                port: None,
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let created = service.create_job(request).await.unwrap();
        let result = service.subscribe_events(&created.job_id).await;
        assert!(result.is_ok());
    }

    // -- Late subscriber sees replayed status_update events --

    #[tokio::test]
    async fn test_subscribe_events_replays_current_statuses() {
        // Background task may emit status_update events before a subscriber
        // attaches. The replay vec returned by subscribe_events must include
        // those statuses so a late subscriber sees the right state on connect.
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: None,
                port: None,
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let created = service.create_job(request).await.unwrap();

        // Give the background task a moment to emit the early status_update events.
        // With no reference processes, run_job_background just emits
        // test_runner starting/running and returns.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (replay, _rx) = service.subscribe_events(&created.job_id).await.unwrap();

        // Should have at least the test_runner status replayed.
        let test_runner_status = replay.iter().find_map(|e| {
            if e.event_type == "status_update" {
                let process = e.message.get("process").and_then(|v| v.as_str())?;
                if process == "test_runner" {
                    return e
                        .message
                        .get("status")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
            None
        });

        assert!(
            test_runner_status.is_some(),
            "expected test_runner status_update in replay, got: {:?}",
            replay
        );
    }

    // -- Late subscriber sees replayed log events --

    #[tokio::test]
    async fn test_subscribe_events_replays_log_buffer() {
        // The parasitic log-buffer task captures `log` events from the
        // broadcast channel. A late subscriber must see those buffered logs
        // in the replay vec, in chronological order, so it does not miss
        // activity that happened before the EventSource attached.
        //
        // Use non-loopback IPs so `run_job_background` does not flip on the
        // reference processes (which would emit their own log events and
        // confuse the count assertion).
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: Some("10.0.0.1".to_string()),
                port: Some(20000),
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: Some("10.0.0.2".to_string()),
                port: Some(20001),
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let created = service.create_job(request).await.unwrap();

        // Let the background task settle so it does not produce any further
        // log events after we start counting. With non-loopback IPs there
        // should be no log events from the background task at all.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Inject a few synthetic log events directly via the broadcast sender.
        // The parasitic task should pick them up and store them in the ring.
        {
            let jobs = service.jobs.lock().await;
            let state = jobs
                .get(&created.job_id)
                .expect("job should exist after create");
            for i in 0..5 {
                let _ = state.event_tx.send(make_event(
                    "log",
                    "outstation",
                    &created.job_id,
                    serde_json::json!({"message": format!("line {i}")}),
                ));
            }
            // Also send a non-log event; it must NOT be buffered.
            let _ = state.event_tx.send(make_event(
                "scenario_status",
                "test_runner",
                &created.job_id,
                serde_json::json!({"event_type": "scenario_status", "status": "starting"}),
            ));
        }

        // Yield so the parasitic task can drain the channel into the ring.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let (replay, _rx) = service.subscribe_events(&created.job_id).await.unwrap();

        let log_events: Vec<&JobEvent> = replay.iter().filter(|e| e.event_type == "log").collect();
        assert_eq!(
            log_events.len(),
            5,
            "expected 5 log events in replay, got: {replay:?}"
        );
        for (i, ev) in log_events.iter().enumerate() {
            let msg = ev.message.get("message").and_then(|v| v.as_str()).unwrap();
            assert_eq!(msg, format!("line {i}"));
        }

        // No scenario_status events should appear in replay (they are not
        // buffered and there is no live snapshot for them).
        assert!(
            !replay.iter().any(|e| e.event_type == "scenario_status"),
            "scenario_status must not be replayed, got: {replay:?}"
        );
    }

    #[tokio::test]
    async fn test_log_buffer_drops_oldest_when_full() {
        // Push more than LOG_BUFFER_CAPACITY events. The ring must cap at
        // capacity and the earliest events must be evicted, not the latest.
        // Non-loopback IPs keep the reference processes off so their own log
        // events do not pollute the ring.
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: Some("10.0.0.1".to_string()),
                port: Some(20000),
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: Some("10.0.0.2".to_string()),
                port: Some(20001),
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let created = service.create_job(request).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Use a smaller batch than the broadcast channel capacity (256) per
        // burst, with a yield between bursts, so the parasitic subscriber
        // never lags. The total exceeds LOG_BUFFER_CAPACITY (500), so the
        // ring must drop oldest entries.
        let total = LOG_BUFFER_CAPACITY + 100;
        for chunk_start in (0..total).step_by(128) {
            {
                let jobs = service.jobs.lock().await;
                let state = jobs.get(&created.job_id).expect("job exists");
                let end = (chunk_start + 128).min(total);
                for i in chunk_start..end {
                    let _ = state.event_tx.send(make_event(
                        "log",
                        "outstation",
                        &created.job_id,
                        serde_json::json!({"seq": i}),
                    ));
                }
            }
            // Yield so the parasitic task drains the chunk before we send more.
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Final drain.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let (replay, _rx) = service.subscribe_events(&created.job_id).await.unwrap();
        let log_events: Vec<&JobEvent> = replay.iter().filter(|e| e.event_type == "log").collect();
        assert_eq!(
            log_events.len(),
            LOG_BUFFER_CAPACITY,
            "ring must cap at LOG_BUFFER_CAPACITY"
        );

        // Earliest sequence in the ring must be `total - LOG_BUFFER_CAPACITY`,
        // i.e. oldest entries were dropped, latest were kept.
        let first_seq = log_events[0]
            .message
            .get("seq")
            .and_then(|v| v.as_u64())
            .unwrap();
        let last_seq = log_events
            .last()
            .unwrap()
            .message
            .get("seq")
            .and_then(|v| v.as_u64())
            .unwrap();
        assert_eq!(first_seq, (total - LOG_BUFFER_CAPACITY) as u64);
        assert_eq!(last_seq, (total - 1) as u64);
    }

    #[tokio::test]
    async fn test_stop_job_no_reference_processes() {
        let service = test_service();
        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: false,
                ip_address: None,
                port: None,
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let created = service.create_job(request).await.unwrap();
        let stopped = service.stop_job(&created.job_id).await.unwrap();
        assert_eq!(stopped.message, "Job stopped");

        let status = service.get_job_status(&created.job_id).await.unwrap();
        assert_eq!(status.status, JobStatus::Stopped);
    }

    // -- make_event helper tests --

    #[test]
    fn test_make_event_populates_fields() {
        let event = make_event(
            "log",
            "test_runner",
            "job-123",
            serde_json::json!({"text": "test"}),
        );
        assert_eq!(event.event_type, "log");
        assert_eq!(event.source, "test_runner");
        assert_eq!(event.job_id, "job-123");
        assert!(event.timestamp > 0.0);
    }

    #[test]
    fn test_make_event_timestamp_is_recent() {
        let event = make_event("log", "test_runner", "j", serde_json::json!(null));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        // Should be within 2 seconds
        assert!(event.timestamp <= now);
        assert!(event.timestamp >= now - 2.0);
    }

    // -- Integration tests that spawn real tasks (ignored by default, need real ports) --

    #[tokio::test]
    #[ignore = "spawns real TCP server, needs available port"]
    async fn test_create_job_with_reference_outstation() {
        let data_dir = format!("{}/../../data", env!("CARGO_MANIFEST_DIR"));
        let service = JobService::new(data_dir);

        let profile_path = format!(
            "{}/../../data/template/profile.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let profile_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&profile_path).expect("profile.json must exist"),
        )
        .unwrap();

        let request = CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: true,
                ip_address: Some("127.0.0.1".to_string()),
                port: Some(20099), // use a non-standard port to avoid conflicts
            },
            profile: profile_json,
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        };

        let response = service.create_job(request).await.unwrap();
        // Give the outstation a moment to bind
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Check status
        let status = service.get_job_status(&response.job_id).await.unwrap();
        assert_eq!(status.status, JobStatus::Running);

        // Stop it
        let stopped = service.stop_job(&response.job_id).await.unwrap();
        assert_eq!(stopped.message, "Job stopped");
    }

    // -- StationBinCheck / classify_station_bin: pure decision logic --

    #[test]
    fn test_classify_station_bin_unset_is_not_configured() {
        assert_eq!(classify_station_bin(None), StationBinCheck::NotConfigured);
    }

    #[test]
    fn test_classify_station_bin_existing_file_is_configured() {
        // Any file guaranteed to exist works; this file itself qualifies.
        let existing = format!("{}/src/services/job_service.rs", env!("CARGO_MANIFEST_DIR"));
        assert!(
            std::path::Path::new(&existing).is_file(),
            "test fixture path must exist: {existing}"
        );
        assert_eq!(
            classify_station_bin(Some(existing)),
            StationBinCheck::Configured
        );
    }

    #[test]
    fn test_classify_station_bin_missing_file_is_missing() {
        let missing = format!(
            "/definitely-missing-{}/no-such-binary",
            Uuid::new_v4().simple()
        );
        assert_eq!(
            classify_station_bin(Some(missing.clone())),
            StationBinCheck::Missing { path: missing }
        );
    }

    // -- StationCompileDecision / decide_station_compile: pure decision
    // logic, the seam that lets the compile decision be asserted without
    // ever spawning cargo --

    #[test]
    fn test_decide_station_compile_unset_builds() {
        assert_eq!(
            decide_station_compile(StationBinCheck::NotConfigured),
            StationCompileDecision::Build
        );
    }

    #[test]
    fn test_decide_station_compile_configured_skips() {
        assert_eq!(
            decide_station_compile(StationBinCheck::Configured),
            StationCompileDecision::Skip
        );
    }

    #[test]
    fn test_decide_station_compile_missing_fails() {
        assert_eq!(
            decide_station_compile(StationBinCheck::Missing {
                path: "/no/such/file".to_string()
            }),
            StationCompileDecision::Fail {
                path: "/no/such/file".to_string()
            }
        );
    }

    // -- check_station_bin / run_job_background: env-var-driven behavior --
    //
    // These tests exercise #50's acceptance criteria through the public
    // create_job API. They mutate process-wide env vars, so they hold
    // test_env_lock::ENV_MUTEX for their whole body and use ScopedEnvVar to
    // restore state even on panic. That lock is shared with
    // process_manager's tests (see test_env_lock), so the SAFETY comments on
    // both modules' set_var/remove_var calls are true.

    async fn wait_for_status(service: &JobService, job_id: &str, target: JobStatus) -> JobStatus {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let status = service.get_job_status(job_id).await.unwrap().status;
            if status == target || tokio::time::Instant::now() >= deadline {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Whether `replay` contains a `status_update` event for `process` with
    /// the given `status`: a structural signal, so a later rewording of a
    /// log message cannot silently satisfy an absence check that relies on
    /// it (unlike matching on a log string's text).
    fn saw_status_update(replay: &[JobEvent], process: &str, status: &str) -> bool {
        replay.iter().any(|event| {
            event.event_type == "status_update"
                && event.message.get("process").and_then(|p| p.as_str()) == Some(process)
                && event.message.get("status").and_then(|s| s.as_str()) == Some(status)
        })
    }

    /// Test double for `CompileRunner`: records each invocation per package
    /// name and returns success without spawning `cargo`, so a test can
    /// assert exactly how many builds a station started.
    #[derive(Clone, Default)]
    struct CountingCompileRunner {
        counts: Arc<std::sync::Mutex<HashMap<String, u32>>>,
    }

    impl CountingCompileRunner {
        fn count(&self, package_name: &str) -> u32 {
            *self
                .counts
                .lock()
                .expect("counts lock poisoned")
                .get(package_name)
                .unwrap_or(&0)
        }
    }

    impl CompileRunner for CountingCompileRunner {
        fn compile<'a>(
            &'a self,
            _job_id: &'a str,
            package_name: &'a str,
            _display_name: &'a str,
            _event_tx: &'a broadcast::Sender<JobEvent>,
        ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
            Box::pin(async move {
                let mut counts = self.counts.lock().expect("counts lock poisoned");
                *counts.entry(package_name.to_string()).or_insert(0) += 1;
                true
            })
        }
    }

    /// Poll `recorder` for up to 5 seconds and return whatever count it
    /// last observed for `package_name`. The compile invocation runs inside
    /// the job's spawned background task, so the count only becomes visible
    /// after that task is scheduled; a fixed sleep would either race it or
    /// waste time, so poll instead, as `wait_for_status` already does for
    /// job status.
    async fn wait_for_compile_count(recorder: &CountingCompileRunner, package_name: &str) -> u32 {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let count = recorder.count(package_name);
            if count > 0 || tokio::time::Instant::now() >= deadline {
                return count;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Collect every event broadcast for a job from the moment `rx` was
    /// subscribed, until a `status_update` for `terminal_process` reaches
    /// `terminal_status`, or 5 seconds pass. Unlike `subscribe_events`'s
    /// replay snapshot, which keeps only the latest status per process, the
    /// live receiver delivers every event once, so a transient status (for
    /// example "compiling" on the way to "running") is still visible here
    /// even after `current_statuses` has moved past it. The caller must
    /// subscribe `rx` immediately after `create_job` returns, before any
    /// other `.await`, so no event fires before the subscription exists.
    async fn drain_live_events(
        rx: &mut broadcast::Receiver<JobEvent>,
        terminal_process: &str,
        terminal_status: &str,
    ) -> Vec<JobEvent> {
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return events;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(event)) => {
                    let is_terminal = event.event_type == "status_update"
                        && event.message.get("process").and_then(|p| p.as_str())
                            == Some(terminal_process)
                        && event.message.get("status").and_then(|s| s.as_str())
                            == Some(terminal_status);
                    events.push(event);
                    if is_terminal {
                        return events;
                    }
                }
                _ => return events,
            }
        }
    }

    /// A binary guaranteed to exist and to exit almost immediately on every
    /// platform: this test binary itself, given CLI args it does not
    /// understand. `/bin/true` does not exist on Windows, and this suite
    /// runs there too (issue #57).
    fn portable_station_binary() -> String {
        std::env::current_exe()
            .expect("current_exe must resolve inside a test binary")
            .to_str()
            .expect("test binary path must be UTF-8")
            .to_string()
    }

    fn outstation_only_request(port: u16) -> CreateJobRequest {
        CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: false,
                ip_address: None,
                port: None,
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: true,
                ip_address: Some("127.0.0.1".to_string()),
                port: Some(port),
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        }
    }

    fn control_station_only_request(port: u16) -> CreateJobRequest {
        CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: true,
                ip_address: Some("127.0.0.1".to_string()),
                port: Some(port),
            },
            outstation_config: crate::models::job::OutstationConfig {
                // A non-loopback address, not None: run_job_background
                // treats a loopback outstation address as "use reference"
                // even with use_reference_outstation false, and the default
                // outstation IP (None resolves to) is loopback.
                use_reference_outstation: false,
                ip_address: Some("10.0.0.2".to_string()),
                port: Some(20001),
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::ControlStation,
        }
    }

    fn both_stations_request(outstation_port: u16, control_station_port: u16) -> CreateJobRequest {
        CreateJobRequest {
            control_station_config: crate::models::job::ControlStationConfig {
                use_reference_control_station: true,
                ip_address: Some("127.0.0.1".to_string()),
                port: Some(control_station_port),
            },
            outstation_config: crate::models::job::OutstationConfig {
                use_reference_outstation: true,
                ip_address: Some("127.0.0.1".to_string()),
                port: Some(outstation_port),
            },
            profile: serde_json::to_value(PicsProfile::load_full_profile()).unwrap(),
            scenario_ids: vec![],
            device_under_test: DeviceUnderTest::Outstation,
        }
    }

    #[tokio::test]
    #[ignore = "invokes a real `cargo build -p reference-outstation` subprocess"]
    async fn test_create_job_unset_bin_still_compiles_via_cargo() {
        // Acceptance criterion 2: with the *_BIN vars unset, the compile
        // decision is unchanged, so a status_update to "compiling" is still
        // emitted for the outstation. Ignored by default because, like the
        // pre-existing test_create_job_with_reference_outstation, it drives
        // a real subprocess (here, a cargo build) rather than a fake; the
        // decision itself (build vs. skip vs. fail) is covered without
        // cargo by the decide_station_compile tests above.
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let _env_guard = test_env_lock::ScopedEnvVar::unset("REFERENCE_OUTSTATION_BIN");

        let service = test_service();
        let response = service
            .create_job(outstation_only_request(20197))
            .await
            .unwrap();

        // Poll for the "compiling" status_update rather than a fixed sleep:
        // it is emitted synchronously before the cargo subprocess starts.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut saw_compiling = false;
        while tokio::time::Instant::now() < deadline {
            let (replay, _rx) = service.subscribe_events(&response.job_id).await.unwrap();
            saw_compiling = saw_status_update(&replay, "outstation", "compiling");
            if saw_compiling {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            saw_compiling,
            "expected an outstation compiling status_update when REFERENCE_OUTSTATION_BIN is unset"
        );

        // Cargo is now building in the background; stop the job rather than
        // waiting for it to finish or bind a port.
        let _ = service.stop_job(&response.job_id).await;
    }

    // -- item 1 seam: the number of builds a station started is asserted
    // directly through the recorder, in the default suite, without a real
    // cargo build. Each kills a wiring mutant that swaps decide_station_compile's
    // Build arm for AlreadyBuilt: the pure decision tests above cannot see
    // it because they test the mapping, not this call site. --

    #[tokio::test]
    async fn test_create_job_unset_outstation_bin_starts_one_build() {
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let _env_guard = test_env_lock::ScopedEnvVar::unset("REFERENCE_OUTSTATION_BIN");

        let recorder = CountingCompileRunner::default();
        let service = test_service().with_compile_runner(Arc::new(recorder.clone()));
        let response = service
            .create_job(outstation_only_request(20182))
            .await
            .unwrap();

        let count = wait_for_compile_count(&recorder, "reference-outstation").await;
        assert_eq!(
            count, 1,
            "an unset REFERENCE_OUTSTATION_BIN must start exactly one outstation build"
        );

        let _ = service.stop_job(&response.job_id).await;
    }

    #[tokio::test]
    async fn test_create_job_unset_control_station_bin_starts_one_build() {
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let _env_guard = test_env_lock::ScopedEnvVar::unset("REFERENCE_CONTROL_STATION_BIN");

        let recorder = CountingCompileRunner::default();
        let service = test_service().with_compile_runner(Arc::new(recorder.clone()));
        let response = service
            .create_job(control_station_only_request(20181))
            .await
            .unwrap();

        let count = wait_for_compile_count(&recorder, "reference-control-station").await;
        assert_eq!(
            count, 1,
            "an unset REFERENCE_CONTROL_STATION_BIN must start exactly one control station build"
        );

        let _ = service.stop_job(&response.job_id).await;
    }

    #[tokio::test]
    async fn test_create_job_missing_outstation_bin_fails_before_spawn() {
        // Acceptance criterion 3: a *_BIN naming a missing file fails the
        // job with a message naming the variable and the path.
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let missing_path = format!(
            "{}/definitely-missing-{}/no-such-binary",
            std::env::temp_dir().display(),
            Uuid::new_v4().simple()
        );
        assert!(
            !std::path::Path::new(&missing_path).exists(),
            "test fixture path must not exist: {missing_path}"
        );
        let _env_guard =
            test_env_lock::ScopedEnvVar::set("REFERENCE_OUTSTATION_BIN", &missing_path);

        let service = test_service();
        let response = service
            .create_job(outstation_only_request(20196))
            .await
            .unwrap();

        let status = wait_for_status(&service, &response.job_id, JobStatus::Failed).await;
        assert_eq!(status, JobStatus::Failed);

        let (replay, _rx) = service.subscribe_events(&response.job_id).await.unwrap();
        let saw_message = replay.iter().any(|event| {
            event
                .message
                .get("message")
                .and_then(|m| m.as_str())
                .map(|m| m.contains("REFERENCE_OUTSTATION_BIN") && m.contains(&missing_path))
                .unwrap_or(false)
        });
        assert!(
            saw_message,
            "expected a log event naming REFERENCE_OUTSTATION_BIN and the missing path"
        );
        assert!(
            saw_status_update(&replay, "outstation", "error"),
            "expected an outstation error status_update so the UI pill leaves starting"
        );
    }

    #[tokio::test]
    async fn test_create_job_configured_outstation_bin_skips_cargo_build() {
        // Acceptance criterion 1: a *_BIN naming an existing file does not
        // invoke cargo. Asserted two ways: the compile runner recorded zero
        // invocations (catches a wiring bug that always builds regardless of
        // the decision), and the live event stream never carried an
        // outstation "compiling" status (catches a wiring bug that emits the
        // status without building; `current_statuses` only keeps the latest
        // value per process, so a replay snapshot cannot see this once the
        // station has moved on to "running").
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let binary = portable_station_binary();
        let _env_guard = test_env_lock::ScopedEnvVar::set("REFERENCE_OUTSTATION_BIN", &binary);

        let recorder = CountingCompileRunner::default();
        let service = test_service().with_compile_runner(Arc::new(recorder.clone()));
        let response = service
            .create_job(outstation_only_request(20195))
            .await
            .unwrap();

        // Subscribe before any other await so the live receiver cannot miss
        // an event the background task fires before this test resumes.
        let (_replay, mut rx) = service.subscribe_events(&response.job_id).await.unwrap();
        let events = drain_live_events(&mut rx, "outstation", "running").await;
        assert!(
            !saw_status_update(&events, "outstation", "compiling"),
            "cargo build should have been skipped when REFERENCE_OUTSTATION_BIN names an existing file"
        );
        assert_eq!(
            recorder.count("reference-outstation"),
            0,
            "no build should have started for a configured outstation"
        );

        let status = wait_for_status(&service, &response.job_id, JobStatus::Running).await;
        assert_eq!(status, JobStatus::Running);

        let stopped = service.stop_job(&response.job_id).await.unwrap();
        assert_eq!(stopped.message, "Job stopped");
    }

    // -- control-station coverage: T1, mutations of the outstation branch
    // must not also pass the control-station branch, and vice versa --

    #[tokio::test]
    async fn test_create_job_missing_control_station_bin_fails_before_spawn() {
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let missing_path = format!(
            "{}/definitely-missing-{}/no-such-binary",
            std::env::temp_dir().display(),
            Uuid::new_v4().simple()
        );
        assert!(
            !std::path::Path::new(&missing_path).exists(),
            "test fixture path must not exist: {missing_path}"
        );
        let _env_guard =
            test_env_lock::ScopedEnvVar::set("REFERENCE_CONTROL_STATION_BIN", &missing_path);

        let service = test_service();
        let response = service
            .create_job(control_station_only_request(20194))
            .await
            .unwrap();

        let status = wait_for_status(&service, &response.job_id, JobStatus::Failed).await;
        assert_eq!(status, JobStatus::Failed);

        let (replay, _rx) = service.subscribe_events(&response.job_id).await.unwrap();
        let saw_message = replay.iter().any(|event| {
            event
                .message
                .get("message")
                .and_then(|m| m.as_str())
                .map(|m| m.contains("REFERENCE_CONTROL_STATION_BIN") && m.contains(&missing_path))
                .unwrap_or(false)
        });
        assert!(
            saw_message,
            "expected a log event naming REFERENCE_CONTROL_STATION_BIN and the missing path"
        );
        assert!(
            saw_status_update(&replay, "control_station", "error"),
            "expected a control_station error status_update so the UI pill leaves starting"
        );
    }

    #[tokio::test]
    async fn test_create_job_configured_control_station_bin_skips_cargo_build() {
        // Same two-way assertion as the outstation equivalent above: a
        // recorded build count and a live-stream check, neither of which a
        // "keep only the latest status" replay snapshot can provide.
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let binary = portable_station_binary();
        let _env_guard = test_env_lock::ScopedEnvVar::set("REFERENCE_CONTROL_STATION_BIN", &binary);

        let recorder = CountingCompileRunner::default();
        let service = test_service().with_compile_runner(Arc::new(recorder.clone()));
        let response = service
            .create_job(control_station_only_request(20193))
            .await
            .unwrap();

        let (_replay, mut rx) = service.subscribe_events(&response.job_id).await.unwrap();
        let events = drain_live_events(&mut rx, "control_station", "running").await;
        assert!(
            !saw_status_update(&events, "control_station", "compiling"),
            "cargo build should have been skipped when REFERENCE_CONTROL_STATION_BIN names an existing file"
        );
        assert_eq!(
            recorder.count("reference-control-station"),
            0,
            "no build should have started for a configured control station"
        );

        let status = wait_for_status(&service, &response.job_id, JobStatus::Running).await;
        assert_eq!(status, JobStatus::Running);

        let stopped = service.stop_job(&response.job_id).await.unwrap();
        assert_eq!(stopped.message, "Job stopped");
    }

    #[tokio::test]
    async fn test_create_job_both_stations_configured_skips_both_cargo_builds() {
        // Both variables are set here, so a branch that reads the other
        // station's variable still skips; that mutation is caught by the
        // control-station-only and outstation-only tests above, not this
        // one. What this test adds over those is running both branches in
        // the same job, so a bug that only appears when both stations are
        // configured together still has a test to catch it.
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let binary = portable_station_binary();
        let _outstation_guard =
            test_env_lock::ScopedEnvVar::set("REFERENCE_OUTSTATION_BIN", &binary);
        let _control_station_guard =
            test_env_lock::ScopedEnvVar::set("REFERENCE_CONTROL_STATION_BIN", &binary);

        let service = test_service();
        let response = service
            .create_job(both_stations_request(20192, 20191))
            .await
            .unwrap();

        // The fake station binary (this test binary, given unrecognized
        // args) exits almost immediately without binding a port, so the
        // control-station branch's outstation-readiness wait fails the job
        // rather than reaching Running; that is expected here and orthogonal
        // to what this test asserts, which is that neither build was
        // spawned.
        let status = wait_for_status(&service, &response.job_id, JobStatus::Failed).await;
        assert_eq!(status, JobStatus::Failed);

        let (replay, _rx) = service.subscribe_events(&response.job_id).await.unwrap();
        assert!(
            !saw_status_update(&replay, "outstation", "compiling"),
            "outstation cargo build should have been skipped"
        );
        assert!(
            !saw_status_update(&replay, "control_station", "compiling"),
            "control_station cargo build should have been skipped"
        );
    }

    // -- item 1 regression: classify both stations before spawning either
    // build, so a missing binary for one station cannot leave a cargo build
    // for the other running past the job's Failed transition --

    #[tokio::test]
    async fn test_create_job_mixed_missing_control_station_fails_before_outstation_build_spawns() {
        // Both stations are classified before either build is spawned (see
        // the module comment above run_job_background's checks), so a
        // Missing control station must leave the compile runner untouched
        // for both packages, including the outstation, whose *_BIN is
        // unset and would otherwise build.
        let _lock = test_env_lock::ENV_MUTEX.lock().await;
        let _outstation_guard = test_env_lock::ScopedEnvVar::unset("REFERENCE_OUTSTATION_BIN");
        let missing_path = format!(
            "{}/definitely-missing-{}/no-such-binary",
            std::env::temp_dir().display(),
            Uuid::new_v4().simple()
        );
        let _control_station_guard =
            test_env_lock::ScopedEnvVar::set("REFERENCE_CONTROL_STATION_BIN", &missing_path);

        let recorder = CountingCompileRunner::default();
        let service = test_service().with_compile_runner(Arc::new(recorder.clone()));
        let response = service
            .create_job(both_stations_request(20190, 20189))
            .await
            .unwrap();

        let status = wait_for_status(&service, &response.job_id, JobStatus::Failed).await;
        assert_eq!(status, JobStatus::Failed);

        assert_eq!(
            recorder.count("reference-outstation"),
            0,
            "the outstation cargo build must not start once the control station's *_BIN is Missing"
        );
        assert_eq!(
            recorder.count("reference-control-station"),
            0,
            "a Missing *_BIN must fail the job before its own build starts"
        );

        let (replay, _rx) = service.subscribe_events(&response.job_id).await.unwrap();
        let saw_message = replay.iter().any(|event| {
            event
                .message
                .get("message")
                .and_then(|m| m.as_str())
                .map(|m| m.contains("REFERENCE_CONTROL_STATION_BIN") && m.contains(&missing_path))
                .unwrap_or(false)
        });
        assert!(
            saw_message,
            "expected a log event naming REFERENCE_CONTROL_STATION_BIN and the missing path"
        );
    }
}
