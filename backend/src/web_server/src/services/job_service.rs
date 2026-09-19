use std::collections::{HashMap, VecDeque};
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
}

impl JobService {
    pub fn new(data_dir: String) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            data_dir,
            scenarios: Arc::new(Vec::new()),
            full_profile: None,
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
        }
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

        // std::env::temp_dir() resolves the OS temp directory (unlike the
        // hardcoded "/tmp" this replaces, which does not exist on Windows).
        let profile_path = std::env::temp_dir()
            .join(format!("mesa-tool_profile_{}.json", job_id))
            .to_string_lossy()
            .to_string();
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

    // Compile the outstation and control station in parallel.
    let outstation_compile = if use_ref_outstation {
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
        Some(tokio::spawn(async move {
            compile_cargo_package(
                &job_id,
                "reference-outstation",
                "reference-outstation",
                &event_tx,
            )
            .await
        }))
    } else {
        None
    };

    let control_station_compile = if use_ref_control_station {
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
        Some(tokio::spawn(async move {
            compile_cargo_package(
                &job_id,
                "reference-control-station",
                "reference-control-station",
                &event_tx,
            )
            .await
        }))
    } else {
        None
    };

    // Wait for the outstation compile, then spawn it.
    if let Some(handle) = outstation_compile {
        let compile_ok = handle.await.unwrap_or(false);
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

    // Wait for the control-station compile, then spawn it.
    if let Some(handle) = control_station_compile {
        let compile_ok = handle.await.unwrap_or(false);
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

    // -- profile file lives under the OS temp directory (#51) --

    /// Process-wide mutex serializing tests in this module that redirect
    /// `TMPDIR`. `std::env::set_var` / `remove_var` are unsafe under edition
    /// 2024 because they are not thread-safe, and cargo runs unit tests in
    /// parallel by default. Same pattern as
    /// `process_manager::tests::ENV_MUTEX`.
    static TMPDIR_MUTEX: std::sync::LazyLock<std::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

    /// RAII guard that points `TMPDIR` at a fresh directory for its
    /// lifetime, then restores the previous value on drop. Needed because on
    /// Linux `std::env::temp_dir()` already resolves to `/tmp`, so without
    /// this redirect a `starts_with(temp_dir())` assertion would pass
    /// against both the old hardcoded path and the fix, proving nothing.
    ///
    /// The directory is leaked (`TempDir::keep`) rather than removed on
    /// drop: `TMPDIR` is process-global, so a concurrent test can still be
    /// using the redirected path after this guard restores `TMPDIR`, and
    /// removing the directory here would fail that other test instead.
    struct ScopedTmpDir {
        dir: std::path::PathBuf,
        previous: Option<String>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ScopedTmpDir {
        fn new() -> Self {
            let guard = TMPDIR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            let dir = tempfile::tempdir().expect("create scratch temp dir").keep();
            let previous = std::env::var("TMPDIR").ok();
            // SAFETY: serialized via TMPDIR_MUTEX; no other test in this
            // module mutates TMPDIR without acquiring the same lock.
            unsafe { std::env::set_var("TMPDIR", &dir) };
            Self {
                dir,
                previous,
                _guard: guard,
            }
        }

        fn path(&self) -> &std::path::Path {
            &self.dir
        }
    }

    impl Drop for ScopedTmpDir {
        fn drop(&mut self) {
            // SAFETY: we still hold TMPDIR_MUTEX via _guard.
            unsafe {
                match self.previous.take() {
                    Some(v) => std::env::set_var("TMPDIR", v),
                    None => std::env::remove_var("TMPDIR"),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_profile_path_is_under_temp_dir_and_removed_on_stop() {
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

        // Redirect TMPDIR only around create_job, the one call that reads
        // std::env::temp_dir(). Dropping the guard immediately afterward
        // shrinks the window in which a concurrently running test could
        // observe the redirected value.
        let (created, expected_temp_dir) = {
            let scoped_tmp = ScopedTmpDir::new();
            let expected_temp_dir = std::env::temp_dir();
            assert_eq!(
                expected_temp_dir,
                scoped_tmp.path(),
                "std::env::temp_dir() should resolve to the redirected TMPDIR"
            );
            let created = service.create_job(request).await.unwrap();
            (created, expected_temp_dir)
        };

        let profile_path = {
            let jobs = service.jobs.lock().await;
            jobs.get(&created.job_id)
                .expect("job should exist after create")
                .profile_path
                .clone()
        };

        assert!(
            std::path::Path::new(&profile_path).starts_with(&expected_temp_dir),
            "profile_path {profile_path} is not under {}",
            expected_temp_dir.display()
        );
        assert!(
            std::path::Path::new(&profile_path).exists(),
            "profile file should exist right after create_job wrote it"
        );

        service.stop_job(&created.job_id).await.unwrap();

        assert!(
            !std::path::Path::new(&profile_path).exists(),
            "profile file should be removed once the job is stopped"
        );
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
}
