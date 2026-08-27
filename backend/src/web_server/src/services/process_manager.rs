//! Subprocess-based orchestration for outstation and control station binaries.
//!
//! - Outstation is spawned first with CLI args for transport, address, and profile
//! - Control station is spawned after the outstation is ready, with outstation address args
//! - Commands are sent to control station via stdin (`new_scenario {id} {profile_json}`)
//! - Conformance events are parsed from stdout (`MESA_CONFORMANCE_EVENT:{json}`)
//! - Events are forwarded through broadcast channels to SSE clients

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use common::conformance_testing::message_structure::TestName;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};

use crate::models::job::JobEvent;

const LOG_LEVEL: &str = "debug";

/// Normalize loopback address strings from the frontend.
///
/// The frontend may send "local", "localhost", or "127.0.0.1" as the IP address.
/// The outstation binary expects a valid IP address for `SocketAddr` parsing,
/// so we normalize all loopback variants to "127.0.0.1".
pub fn normalize_loopback(addr: &str) -> String {
    match addr.to_lowercase().as_str() {
        "local" | "localhost" => "127.0.0.1".to_string(),
        other => other.to_string(),
    }
}

/// Check whether an address string refers to the local machine.
///
/// Returns true for "local", "localhost", and "127.0.0.1" (case-insensitive).
pub fn is_loopback(addr: &str) -> bool {
    matches!(
        addr.trim().to_lowercase().as_str(),
        "local" | "localhost" | "127.0.0.1"
    )
}

/// Normalize a conformance event payload for the frontend.
///
/// The Rust binaries emit `"pass": true/false` but the frontend expects `"passed"`.
/// This function renames the field and injects `"event_type"` into the message body
/// so the frontend's type guards can identify the event.
pub fn normalize_conformance_payload(payload: &mut serde_json::Value) {
    if let Some(obj) = payload.as_object_mut() {
        if let Some(pass_val) = obj.remove("pass") {
            obj.insert("passed".to_string(), pass_val);
        }
        obj.entry("event_type".to_string())
            .or_insert_with(|| serde_json::Value::String("conformance_test_result".to_string()));
    }
}

/// Format a normalized conformance payload as a human-readable log line.
/// `normalize_conformance_payload` must be called before this so `"passed"` is present.
fn format_conformance_log(payload: &serde_json::Value, should_pass: Option<bool>) -> String {
    let test = payload.get("test").and_then(|v| v.as_str()).unwrap_or("?");
    let passed = payload
        .get("passed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let status = if passed { "PASS" } else { "FAIL" };
    let mut line = match should_pass {
        Some(expected) => {
            let result_mark = if passed == expected { "✓" } else { "✗" };
            if expected {
                format!("{result_mark} {test}: {status}")
            } else {
                format!("{result_mark} {test}: actual: {status} expected: FAIL")
            }
        }
        None => format!("{test}: {status}"),
    };
    if !passed {
        if let Some(first_issue) = payload
            .get("comments")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("issue"))
            .and_then(|v| v.as_str())
        {
            line = format!("{line} — {first_issue}");
        }
    }
    line
}

/// Terminate a child process, extract the exit code, and emit log + status events.
///
/// Records the final per-process status into `current_statuses` before
/// broadcasting so a subscriber that connects right after teardown still
/// sees the correct terminal status via replay.
pub async fn terminate_and_emit(
    child: &mut Child,
    process_name: &str,
    job_id: &str,
    event_tx: &broadcast::Sender<JobEvent>,
    current_statuses: &std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, String>>>,
) {
    use super::job_service::{emit_status_update, make_event};

    terminate_child(child, Duration::from_secs(5)).await;
    let exit_code = child.try_wait().ok().flatten().and_then(|s| s.code());
    let code_str = match exit_code {
        Some(c) => c.to_string(),
        None => "None".to_string(),
    };
    let _ = event_tx.send(make_event(
        "log",
        "test_runner",
        job_id,
        serde_json::json!({"message": format!("{process_name} exited with code {code_str}.")}),
    ));
    // None (signal kill) or 0 = stopped cleanly; non-zero = error
    let exit_status = match exit_code {
        Some(c) if c != 0 => "error",
        _ => "stopped",
    };
    emit_status_update(
        current_statuses,
        event_tx,
        job_id,
        process_name,
        exit_status,
    );
}

/// Strip ANSI escape sequences from a line of text.
fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1B' {
            // Skip the escape sequence: ESC [ ... final_byte
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // consume until we hit a letter in '@'..='~'
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() || ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

const CONFORMANCE_PREFIX: &str = "MESA_CONFORMANCE_EVENT:";

/// Find the binary path for a given binary name.
///
/// Search order:
/// 1. Environment variable (e.g. `REFERENCE_CONTROL_STATION_BIN`, `REFERENCE_OUTSTATION_BIN`)
/// 2. `./target/debug/{name}` (dev builds)
/// 3. `./target/release/{name}` (release builds)
/// 4. Fall back to just the name (assume it is in PATH, e.g. Docker `/usr/local/bin`)
pub fn find_binary(name: &str, env_var: &str) -> String {
    if let Ok(path) = std::env::var(env_var) {
        return path;
    }

    let debug_path = format!("./target/debug/{name}");
    if std::path::Path::new(&debug_path).exists() {
        return debug_path;
    }

    let release_path = format!("./target/release/{name}");
    if std::path::Path::new(&release_path).exists() {
        return release_path;
    }

    // Fall back to PATH lookup
    name.to_string()
}

/// Spawn a binary as a child process with stdout/stderr streaming.
///
/// `source_label` is the name used in log events (e.g. "outstation", "control_station").
/// Returns the child process and two task handles for stdout/stderr streaming.
#[allow(clippy::too_many_arguments)]
fn spawn_binary(
    mut cmd: Command,
    binary_name: &str,
    source_label: &str,
    job_id: &str,
    event_tx: broadcast::Sender<JobEvent>,
    cancel_token: CancellationToken,
    forward_logs: bool,
    expected_outcomes: Arc<HashMap<(String, TestName), bool>>,
) -> Result<(Child, JoinHandle<()>, JoinHandle<()>), String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to spawn {binary_name}: {e}"))?;

    let stdout = child.stdout.take().expect("stdout should be piped");
    let stderr = child.stderr.take().expect("stderr should be piped");

    let jid = job_id.to_string();
    let label = source_label.to_string();
    let tx = event_tx.clone();
    let cancel = cancel_token.clone();
    let stdout_handle = tokio::spawn(stream_output(
        jid.clone(),
        label.clone(),
        "stdout".to_string(),
        stdout,
        tx.clone(),
        cancel.clone(),
        false,
        forward_logs,
        expected_outcomes.clone(),
    ));

    let stderr_handle = tokio::spawn(stream_output(
        jid,
        label,
        "stderr".to_string(),
        stderr,
        tx,
        cancel,
        true,
        forward_logs,
        expected_outcomes,
    ));

    Ok((child, stdout_handle, stderr_handle))
}

/// Spawn the outstation as a child process.
///
/// Returns the child process handle and task handles that stream stdout/stderr.
///
/// `reference-outstation` is TCP-only and its CLI accepts `--local`,
/// `--profile`, and `--log-level`. The legacy `outstation` binary used to
/// take `--transport`; that flag is intentionally not forwarded here.
pub fn spawn_outstation(
    job_id: &str,
    local_addr: &str,
    profile_path: &str,
    event_tx: broadcast::Sender<JobEvent>,
    cancel_token: CancellationToken,
    forward_logs: bool,
    expected_outcomes: Arc<HashMap<(String, TestName), bool>>,
) -> Result<(Child, JoinHandle<()>, JoinHandle<()>), String> {
    let binary = find_binary("reference-outstation", "REFERENCE_OUTSTATION_BIN");
    info!("Spawning outstation binary: {binary} --local {local_addr} --profile {profile_path}");

    let mut cmd = Command::new(&binary);
    cmd.arg("--local")
        .arg(local_addr)
        .arg("--profile")
        .arg(profile_path)
        .arg("--log-level")
        .arg(LOG_LEVEL)
        .stdin(Stdio::null());

    spawn_binary(
        cmd,
        "reference-outstation",
        "outstation",
        job_id,
        event_tx,
        cancel_token,
        forward_logs,
        expected_outcomes,
    )
}

/// Spawn the control station as a child process.
///
/// Returns the child process handle and task handles for stdout/stderr streaming.
/// The caller can write to `child.stdin` to send commands.
#[allow(clippy::too_many_arguments)]
pub fn spawn_control_station(
    job_id: &str,
    outstation_ip: &str,
    outstation_port: u16,
    profile_path: &str,
    event_tx: broadcast::Sender<JobEvent>,
    cancel_token: CancellationToken,
    forward_logs: bool,
    expected_outcomes: Arc<HashMap<(String, TestName), bool>>,
) -> Result<(Child, JoinHandle<()>, JoinHandle<()>), String> {
    let binary = find_binary("reference-control-station", "REFERENCE_CONTROL_STATION_BIN");

    // Build the args vec first so the log line and the spawned command shape
    // are guaranteed to match. Without this, the log line drifts from the
    // actual subprocess invocation when `profile_path` is `None`: the
    // historical formulation printed `--profile ` with an empty trailing arg
    // even though no `--profile` flag was passed to the binary.
    let port_str = outstation_port.to_string();
    let mut args: Vec<&str> = vec![
        "--outstation-ip",
        outstation_ip,
        "--outstation-port",
        &port_str,
        "--log-level",
        LOG_LEVEL,
    ];
    args.push("--profile");
    args.push(profile_path);
    info!(
        "Spawning control-station binary: {binary} {args}",
        args = args.join(" ")
    );

    let mut cmd = Command::new(&binary);
    cmd.args(&args).stdin(Stdio::piped());

    spawn_binary(
        cmd,
        "reference-control-station",
        "control_station",
        job_id,
        event_tx,
        cancel_token,
        forward_logs,
        expected_outcomes,
    )
}

/// Core write+flush logic for sending a command line to a `ChildStdin`.
async fn write_stdin_line(
    stdin: &mut ChildStdin,
    command: &str,
    label: &str,
) -> Result<(), String> {
    let line = format!("{command}\n");
    stdin.write_all(line.as_bytes()).await.map_err(|e| {
        warn!(error = %e, "{label}: write_all failed");
        format!("Failed to write to control station stdin: {e}")
    })?;
    stdin.flush().await.map_err(|e| {
        warn!(error = %e, "{label}: flush failed");
        format!("Failed to flush control station stdin: {e}")
    })?;
    Ok(())
}

/// Send a command to the control station via stdin.
///
/// Format: `{command}\n`
pub async fn send_stdin_command(child: &mut Child, command: &str) -> Result<(), String> {
    info!(command_len = command.len(), command_preview = %command.chars().take(80).collect::<String>(), "send_stdin_command: writing to child stdin");
    let stdin = child.stdin.as_mut().ok_or_else(|| {
        warn!("send_stdin_command: stdin handle is None (not piped or already taken)");
        "Control station stdin not available".to_string()
    })?;
    write_stdin_line(stdin, command, "send_stdin_command").await?;
    info!("send_stdin_command: command written and flushed successfully");
    Ok(())
}

/// Send a command via a shared stdin handle wrapped in `Arc<Mutex<ChildStdin>>`.
///
/// This allows both the orchestrator task and manual `send_scenario_command`
/// calls to write to the same control station stdin without ownership conflicts.
pub async fn send_stdin_command_shared(
    stdin: &Arc<Mutex<ChildStdin>>,
    command: &str,
) -> Result<(), String> {
    info!(command_len = command.len(), command_preview = %command.chars().take(80).collect::<String>(), "send_stdin_command_shared: writing to shared stdin");
    let mut stdin_guard = stdin.lock().await;
    write_stdin_line(&mut stdin_guard, command, "send_stdin_command_shared").await?;
    info!("send_stdin_command_shared: command written and flushed successfully");
    Ok(())
}

/// Result of waiting for a TCP port.
#[derive(Debug, PartialEq)]
pub enum PortWaitResult {
    /// Port is accepting connections.
    Ready,
    /// Timed out waiting for the port.
    Timeout,
    /// The child process exited before the port became available.
    ChildExited(Option<i32>),
}

/// Wait for a TCP port to become available.
pub async fn wait_for_tcp_port(host: &str, port: u16, timeout: Duration) -> PortWaitResult {
    wait_for_tcp_port_with_child(host, port, timeout, None).await
}

/// Wait for a TCP port to become available, with optional early-exit detection.
///
/// If `child` is provided, each poll iteration checks whether the process has
/// already exited. This avoids waiting the full timeout when the binary crashes
/// immediately (e.g. bad CLI args, missing profile, invalid address).
pub async fn wait_for_tcp_port_with_child(
    host: &str,
    port: u16,
    timeout: Duration,
    mut child: Option<&mut Child>,
) -> PortWaitResult {
    let addr = format!("{host}:{port}");
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        // Check if the child process already exited
        if let Some(ref mut c) = child {
            match c.try_wait() {
                Ok(Some(status)) => {
                    warn!("Child process exited with {status} before port {addr} became available");
                    return PortWaitResult::ChildExited(status.code());
                }
                Ok(None) => {} // still running
                Err(e) => {
                    warn!("Failed to check child process status: {e}");
                }
            }
        }

        match tokio::time::timeout(
            Duration::from_millis(500),
            tokio::net::TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(stream)) => {
                drop(stream);
                return PortWaitResult::Ready;
            }
            _ => {
                if tokio::time::Instant::now() >= deadline {
                    return PortWaitResult::Timeout;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Gracefully terminate a child process. Sends SIGKILL if it does not exit within `timeout`.
pub async fn terminate_child(child: &mut Child, timeout: Duration) {
    // Try to kill (on Unix this sends SIGKILL, but we try graceful first if possible)
    let _ = child.start_kill();
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(_) => {}
        Err(_) => {
            // Force kill
            let _ = child.kill().await;
        }
    }
}

/// Route subprocess output through tracing. The web server can then apply a
/// separate filter to each log output.
fn subprocess_log_level(output_line: &str) -> Option<&str> {
    output_line
        .split_whitespace()
        .take(3)
        .find(|prefix_field| matches!(*prefix_field, "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE"))
}

fn log_subprocess_output(source: &str, stream_name: &str, output_line: &str) {
    let log_level = subprocess_log_level(output_line);
    let log_message = format_args!("[{source} {stream_name}] {output_line}");

    match log_level {
        Some("ERROR") => error!("{log_message}"),
        Some("WARN") => warn!("{log_message}"),
        Some("DEBUG") => debug!("{log_message}"),
        Some("TRACE") => trace!("{log_message}"),
        Some("INFO") => info!("{log_message}"),
        _ if stream_name == "stderr" => error!("{log_message}"),
        _ => info!("{log_message}"),
    }
}

/// Stream lines from a reader, parse conformance events, and forward to the broadcast channel.
#[allow(clippy::too_many_arguments)]
async fn stream_output<R: tokio::io::AsyncRead + Unpin>(
    job_id: String,
    source: String,
    label: String,
    reader: R,
    event_tx: broadcast::Sender<JobEvent>,
    cancel_token: CancellationToken,
    skip_conformance: bool,
    forward_logs: bool,
    expected_outcomes: Arc<HashMap<(String, TestName), bool>>,
) {
    info!(
        job_id = %job_id,
        source = %source,
        label = %label,
        skip_conformance = skip_conformance,
        forward_logs = forward_logs,
        "stream_output: starting reader task"
    );
    let mut line_count: u64 = 0;
    let mut lines = BufReader::new(reader).lines();
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!(
                    job_id = %job_id,
                    source = %source,
                    label = %label,
                    total_lines = line_count,
                    "stream_output: cancelled"
                );
                break;
            },
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        line_count += 1;
                        let clean = strip_ansi(&line);
                        if clean.is_empty() {
                            continue;
                        }

                        if !forward_logs {
                            continue;
                        }

                        log_subprocess_output(&source, &label, &clean);

                        let receiver_count = event_tx.receiver_count();
                        if line_count <= 5 || line_count.is_multiple_of(50) {
                            info!(
                                job_id = %job_id,
                                source = %source,
                                label = %label,
                                line_count = line_count,
                                receivers = receiver_count,
                                line_preview = %clean.chars().take(120).collect::<String>(),
                                "stream_output: received line from subprocess"
                            );
                        }

                        // Rate-limit: log on the first line and every 50th line
                        // thereafter. A noisy subprocess with no SSE clients
                        // attached would otherwise flood the log.
                        if receiver_count == 0 && (line_count == 1 || line_count.is_multiple_of(50)) {
                            warn!(
                                job_id = %job_id,
                                source = %source,
                                label = %label,
                                line_count = line_count,
                                "stream_output: no receivers on broadcast channel, events will be dropped"
                            );
                        }

                        // Check for conformance event prefix
                        if !skip_conformance {
                            if let Some(json_str) = clean.strip_prefix(CONFORMANCE_PREFIX) {
                                info!(
                                    job_id = %job_id,
                                    source = %source,
                                    "stream_output: found MESA_CONFORMANCE_EVENT, parsing JSON"
                                );

                                handle_conformance_event(json_str, &job_id, &source, &event_tx, &expected_outcomes);
                                continue;
                            }
                        }

                        // Skip internal conformance tracing lines (already captured above).
                        // The subprocess's fmt layer may or may not include the target
                        // ("conformance") depending on its configuration. When the
                        // target is hidden (.with_target(false)), the line looks like:
                        //   2026-04-10T... DEBUG logging.rs:33: event_json={...}
                        // When the target is shown:
                        //   2026-04-10T... DEBUG conformance: logging.rs:33: event_json={...}
                        // In either case, "event_json=" is the reliable marker.
                        if clean.contains("event_json=") {
                            continue;
                        }

                        // Forward non-debug subprocess output as a log event.
                        if subprocess_log_level(&clean) != Some("DEBUG") {
                            let event = super::job_service::make_event(
                                "log",
                                &source,
                                &job_id,
                                serde_json::json!({"message": format!("[{label}] {clean}")}),
                            );
                            let _ = event_tx.send(event);
                        }
                    }
                    Ok(None) => {
                        info!(
                            job_id = %job_id,
                            source = %source,
                            label = %label,
                            total_lines = line_count,
                            "stream_output: EOF reached"
                        );
                        break;
                    }
                    Err(e) => {
                        warn!("Error reading {source} {label}: {e}");
                        break;
                    }
                }
            }
        }
    }
}

fn handle_conformance_event(
    json_str: &str,
    job_id: &str,
    source: &str,
    event_tx: &broadcast::Sender<JobEvent>,
    expected_outcomes: &HashMap<(String, TestName), bool>,
) {
    // Parse and forward as conformance_test_result event.
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(mut parsed) => {
            normalize_conformance_payload(&mut parsed);
            // Also emit a human-readable log entry so the
            // log panel shows SunSpecMessage content.
            let should_pass = parsed
                .get("scenario_id")
                .and_then(|value| value.as_str())
                .zip(parsed.get("test").and_then(|value| value.as_str()))
                .and_then(|(scenario_id, test_id)| {
                    expected_outcomes
                        .get(&(scenario_id.to_string(), TestName::try_from(test_id).ok()?))
                        .copied()
                });
            let log_msg = format_conformance_log(&parsed, should_pass);
            let event =
                super::job_service::make_event("conformance_test_result", source, job_id, parsed);
            let send_result = event_tx.send(event);
            debug!(
                job_id = %job_id,
                source = %source,
                sent_ok = send_result.is_ok(),
                "stream_output: forwarded conformance event"
            );
            let log_event = super::job_service::make_event(
                "log",
                "test_runner",
                job_id,
                serde_json::json!({"message": log_msg}),
            );
            let _ = event_tx.send(log_event);
        }
        Err(e) => {
            warn!("Failed to parse conformance event JSON: {e}, raw: {json_str}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    /// Process-wide mutex serializing tests that mutate environment
    /// variables. `std::env::set_var` / `remove_var` are unsafe under
    /// edition 2024 because they are not thread-safe; cargo runs unit
    /// tests in parallel by default, so we guard mutation here.
    static ENV_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    /// RAII guard that sets an env var on construction and restores the
    /// previous value (or removes it) on drop. Holds the env mutex for
    /// the duration so concurrent env-mutating tests serialize.
    struct ScopedTestEnvVar {
        key: String,
        previous: Option<String>,
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedTestEnvVar {
        /// Set `key` to `value` for the lifetime of the returned guard.
        fn set(key: &str, value: &str) -> Self {
            let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            let previous = std::env::var(key).ok();
            // SAFETY: serialized via ENV_MUTEX; no other test in this
            // process mutates env vars without acquiring the same lock.
            unsafe { std::env::set_var(key, value) };
            Self {
                key: key.to_string(),
                previous,
                _guard: guard,
            }
        }

        /// Ensure `key` is unset for the lifetime of the returned guard.
        fn unset(key: &str) -> Self {
            let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            let previous = std::env::var(key).ok();
            // SAFETY: serialized via ENV_MUTEX.
            unsafe { std::env::remove_var(key) };
            Self {
                key: key.to_string(),
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedTestEnvVar {
        fn drop(&mut self) {
            // SAFETY: we still hold ENV_MUTEX via _guard.
            unsafe {
                match self.previous.take() {
                    Some(v) => std::env::set_var(&self.key, v),
                    None => std::env::remove_var(&self.key),
                }
            }
        }
    }

    #[test]
    fn test_strip_ansi_no_escapes() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn test_strip_ansi_with_color_codes() {
        assert_eq!(strip_ansi("\x1B[31mred\x1B[0m"), "red");
    }

    #[test]
    fn test_strip_ansi_complex_sequence() {
        let input = "\x1B[1;32mgreen bold\x1B[0m normal";
        let result = strip_ansi(input);
        assert_eq!(result, "green bold normal");
    }

    #[test]
    fn test_strip_ansi_empty_string() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_strip_ansi_preserves_non_escape_brackets() {
        assert_eq!(strip_ansi("array[0]"), "array[0]");
    }

    #[test]
    fn test_find_binary_env_var() {
        let _env = ScopedTestEnvVar::set("TEST_BIN_PATH", "/custom/path/binary");
        let result = find_binary("test-binary", "TEST_BIN_PATH");
        assert_eq!(result, "/custom/path/binary");
    }

    #[test]
    fn test_find_binary_fallback_to_name() {
        // When no env var and no local files exist, should return the name itself
        let _env = ScopedTestEnvVar::unset("NONEXISTENT_BIN_VAR");
        let result = find_binary("nonexistent-binary-name", "NONEXISTENT_BIN_VAR");
        assert_eq!(result, "nonexistent-binary-name");
    }

    #[tokio::test]
    async fn test_wait_for_tcp_port_timeout() {
        // Port that nothing is listening on should time out
        let result = wait_for_tcp_port("127.0.0.1", 59999, Duration::from_millis(200)).await;
        assert_eq!(result, PortWaitResult::Timeout);
    }

    #[tokio::test]
    async fn test_wait_for_tcp_port_succeeds_when_listening() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let result = wait_for_tcp_port("127.0.0.1", port, Duration::from_secs(2)).await;
        assert_eq!(result, PortWaitResult::Ready);
    }

    #[tokio::test]
    async fn test_terminate_child_kills_process() {
        let mut child = Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("Failed to spawn sleep");
        terminate_child(&mut child, Duration::from_secs(2)).await;
        // Process should have exited
        let status = child.try_wait().expect("Failed to check process status");
        assert!(status.is_some());
    }

    #[tokio::test]
    async fn test_send_stdin_command_to_cat() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn cat");

        send_stdin_command(&mut child, "hello world")
            .await
            .expect("Failed to send command");

        // Close stdin so cat exits
        child.stdin.take();

        let output = child
            .wait_with_output()
            .await
            .expect("Failed to wait for cat");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "hello world");
    }

    #[tokio::test]
    async fn test_send_stdin_command_no_stdin_returns_error() {
        let mut child = Command::new("echo")
            .arg("test")
            .stdin(Stdio::null())
            .spawn()
            .expect("Failed to spawn echo");

        let result = send_stdin_command(&mut child, "hello").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("stdin not available"));

        let _ = child.wait().await;
    }

    // -- normalize_loopback tests --

    #[test]
    fn test_normalize_loopback_local() {
        assert_eq!(normalize_loopback("local"), "127.0.0.1");
    }

    #[test]
    fn test_normalize_loopback_localhost() {
        assert_eq!(normalize_loopback("localhost"), "127.0.0.1");
    }

    #[test]
    fn test_normalize_loopback_case_insensitive() {
        assert_eq!(normalize_loopback("LOCAL"), "127.0.0.1");
        assert_eq!(normalize_loopback("LocalHost"), "127.0.0.1");
    }

    #[test]
    fn test_normalize_loopback_ip_passthrough() {
        assert_eq!(normalize_loopback("127.0.0.1"), "127.0.0.1");
        assert_eq!(normalize_loopback("192.168.1.100"), "192.168.1.100");
        assert_eq!(normalize_loopback("10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn test_normalize_loopback_empty_string() {
        assert_eq!(normalize_loopback(""), "");
    }

    // -- is_loopback tests --

    #[test]
    fn test_is_loopback_local() {
        assert!(is_loopback("local"));
        assert!(is_loopback("LOCAL"));
    }

    #[test]
    fn test_is_loopback_localhost() {
        assert!(is_loopback("localhost"));
        assert!(is_loopback("LocalHost"));
    }

    #[test]
    fn test_is_loopback_127() {
        assert!(is_loopback("127.0.0.1"));
    }

    #[test]
    fn test_is_loopback_remote_ip() {
        assert!(!is_loopback("192.168.1.100"));
        assert!(!is_loopback("10.0.0.1"));
    }

    #[test]
    fn test_is_loopback_empty() {
        assert!(!is_loopback(""));
    }

    // -- normalize_conformance_payload tests --

    #[test]
    fn test_normalize_conformance_payload_renames_pass() {
        let mut payload = serde_json::json!({"pass": true, "test": "MON_001"});
        normalize_conformance_payload(&mut payload);
        assert_eq!(payload.get("passed").and_then(|v| v.as_bool()), Some(true));
        assert!(payload.get("pass").is_none());
    }

    #[test]
    fn test_normalize_conformance_payload_injects_event_type() {
        let mut payload = serde_json::json!({"pass": false});
        normalize_conformance_payload(&mut payload);
        assert_eq!(
            payload.get("event_type").and_then(|v| v.as_str()),
            Some("conformance_test_result")
        );
    }

    #[test]
    fn test_normalize_conformance_payload_preserves_existing_event_type() {
        let mut payload = serde_json::json!({"pass": true, "event_type": "custom"});
        normalize_conformance_payload(&mut payload);
        assert_eq!(
            payload.get("event_type").and_then(|v| v.as_str()),
            Some("custom")
        );
    }

    // -- format_conformance_log tests --

    #[test]
    fn test_format_conformance_log_expected_pass_achieved() {
        let payload = serde_json::json!({"test": "MON_001", "passed": true, "comments": []});
        assert_eq!(
            format_conformance_log(&payload, Some(true)),
            "✓ MON_001: PASS"
        );
    }

    #[test]
    fn test_format_conformance_log_expected_pass_not_achieved() {
        let payload = serde_json::json!({"test": "MON_001", "passed": false, "comments": []});
        assert_eq!(
            format_conformance_log(&payload, Some(true)),
            "✗ MON_001: FAIL"
        );
    }

    #[test]
    fn test_format_conformance_log_expected_fail_achieved() {
        let payload = serde_json::json!({"test": "CURVE_001", "passed": false, "comments": []});
        assert_eq!(
            format_conformance_log(&payload, Some(false)),
            "✓ CURVE_001: actual: FAIL expected: FAIL"
        );
    }

    #[test]
    fn test_format_conformance_log_expected_fail_not_achieved() {
        let payload = serde_json::json!({"test": "CURVE_001", "passed": true, "comments": []});
        assert_eq!(
            format_conformance_log(&payload, Some(false)),
            "✗ CURVE_001: actual: PASS expected: FAIL"
        );
    }

    // -- wait_for_tcp_port_with_child tests --

    #[tokio::test]
    async fn test_wait_for_tcp_port_child_exits_immediately() {
        // Spawn a process that exits immediately with an error
        let mut child = Command::new("false")
            .spawn()
            .expect("Failed to spawn false");

        // Give it a moment to exit
        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = wait_for_tcp_port_with_child(
            "127.0.0.1",
            59998,
            Duration::from_secs(5),
            Some(&mut child),
        )
        .await;

        assert_eq!(result, PortWaitResult::ChildExited(Some(1)));
    }

    #[tokio::test]
    async fn test_wait_for_tcp_port_no_child_timeout() {
        let result = wait_for_tcp_port("127.0.0.1", 59997, Duration::from_millis(200)).await;
        assert_eq!(result, PortWaitResult::Timeout);
    }

    #[tokio::test]
    async fn test_wait_for_tcp_port_no_child_ready() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let result = wait_for_tcp_port("127.0.0.1", port, Duration::from_secs(2)).await;
        assert_eq!(result, PortWaitResult::Ready);
    }

    #[tokio::test]
    async fn test_stream_output_parses_conformance_events() {
        let json_payload =
            r#"{"test":"MON_001","scenario_id":"sc1","timestamp":123,"pass":true,"comments":[]}"#;
        let input = format!("MESA_CONFORMANCE_EVENT:{json_payload}\nsome log line\n");
        let cursor = std::io::Cursor::new(input.into_bytes());

        let (tx, mut rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();
        let expected_outcomes = Arc::new(HashMap::from([(
            ("sc1".to_string(), TestName::MON_001),
            true,
        )]));

        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "control_station".to_string(),
            "stdout".to_string(),
            cursor,
            tx,
            cancel.clone(),
            false,
            true,
            expected_outcomes,
        ));

        // Wait for task to finish (input is finite)
        handle.await.unwrap();

        // Should have received three events: conformance result + its formatted log + the plain log line
        let e1 = rx.try_recv().expect("Should have conformance event");
        assert_eq!(e1.event_type, "conformance_test_result");
        assert_eq!(e1.source, "control_station");
        assert_eq!(e1.job_id, "job-1");

        // Verify the message payload matches the frontend's expected shape:
        // - "pass" renamed to "passed"
        // - "event_type" injected into message body
        let msg = &e1.message;
        assert_eq!(msg.get("passed").and_then(|v| v.as_bool()), Some(true));
        assert!(
            msg.get("pass").is_none(),
            "pass should be renamed to passed"
        );
        assert_eq!(
            msg.get("event_type").and_then(|v| v.as_str()),
            Some("conformance_test_result"),
        );
        assert_eq!(msg.get("test").and_then(|v| v.as_str()), Some("MON_001"));
        assert_eq!(msg.get("scenario_id").and_then(|v| v.as_str()), Some("sc1"));

        // The conformance event is also echoed to the log panel as a human-readable entry.
        let e2 = rx.try_recv().expect("Should have conformance log event");
        assert_eq!(e2.event_type, "log");
        assert_eq!(
            e2.message["message"].as_str(),
            Some("✓ MON_001: PASS"),
            "conformance log should show that the result matched the expectation"
        );

        let e3 = rx.try_recv().expect("Should have log event");
        assert_eq!(e3.event_type, "log");
        assert!(
            e3.message["message"]
                .as_str()
                .unwrap()
                .contains("some log line")
        );
    }

    #[tokio::test]
    async fn test_stream_output_skip_conformance() {
        let json_payload =
            r#"{"test":"MON_001","scenario_id":"sc1","timestamp":123,"pass":true,"comments":[]}"#;
        let input = format!("MESA_CONFORMANCE_EVENT:{json_payload}\nregular log\n");
        let cursor = std::io::Cursor::new(input.into_bytes());

        let (tx, mut rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();

        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "outstation".to_string(),
            "stderr".to_string(),
            cursor,
            tx,
            cancel.clone(),
            true, // skip conformance
            true,
            Arc::new(HashMap::new()),
        ));

        handle.await.unwrap();

        // Conformance line should be skipped entirely when skip_conformance is true
        // We should only get the conformance line forwarded as a regular log,
        // wait, actually skip_conformance means we skip parsing, so the line
        // is still there but not parsed as conformance. Let me re-check the logic...
        // Actually looking at the code: if skip_conformance is true, the conformance
        // prefix check is skipped entirely, so the line falls through to the log handler.
        let e1 = rx.try_recv().expect("Should have first event");
        // With skip_conformance=true, the conformance line is forwarded as a log
        assert_eq!(e1.event_type, "log");

        let e2 = rx.try_recv().expect("Should have second event");
        assert_eq!(e2.event_type, "log");
    }

    #[tokio::test]
    async fn test_stream_output_skips_empty_lines() {
        let input = "line one\n\n\nline two\n";
        let cursor = std::io::Cursor::new(input.as_bytes().to_vec());

        let (tx, mut rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();

        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "test".to_string(),
            "stdout".to_string(),
            cursor,
            tx,
            cancel,
            true,
            true,
            Arc::new(HashMap::new()),
        ));

        handle.await.unwrap();

        let e1 = rx.try_recv().expect("Should have first event");
        assert!(e1.message["message"].as_str().unwrap().contains("line one"));

        let e2 = rx.try_recv().expect("Should have second event");
        assert!(e2.message["message"].as_str().unwrap().contains("line two"));

        // No more events
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_stream_output_does_not_forward_debug_logs() {
        let input = concat!(
            "2026-08-10T12:00:00Z DEBUG Curve 1: received AI329\n",
            "2026-08-10T12:00:01Z INFO job completed\n",
        );
        let cursor = std::io::Cursor::new(input.as_bytes().to_vec());

        let (tx, mut rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();

        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "control_station".to_string(),
            "stderr".to_string(),
            cursor,
            tx,
            cancel,
            true,
            true,
            Arc::new(HashMap::new()),
        ));

        handle.await.unwrap();

        let event = rx.try_recv().expect("INFO log should be forwarded");
        assert!(
            event.message["message"]
                .as_str()
                .unwrap()
                .contains("job completed")
        );
        assert!(rx.try_recv().is_err(), "DEBUG log must not reach the UI");
    }

    #[tokio::test]
    async fn test_stream_output_cancellation() {
        // Use a command that produces output indefinitely
        let child = Command::new("yes")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn yes");

        let stdout = child.stdout.unwrap();
        let (tx, _rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();

        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "test".to_string(),
            "stdout".to_string(),
            stdout,
            tx,
            cancel_clone,
            true,
            true,
            Arc::new(HashMap::new()),
        ));

        // Cancel after a short delay
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();

        // Task should complete within a reasonable time
        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(
            result.is_ok(),
            "Stream task should have stopped after cancellation"
        );
    }

    #[tokio::test]
    async fn test_stream_output_filters_internal_conformance_tracing() {
        let input = "2024-01-01 conformance: something event_json=blah\nreal log line\n";
        let cursor = std::io::Cursor::new(input.as_bytes().to_vec());

        let (tx, mut rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();

        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "test".to_string(),
            "stdout".to_string(),
            cursor,
            tx,
            cancel,
            false,
            true,
            Arc::new(HashMap::new()),
        ));

        handle.await.unwrap();

        // Should only get the "real log line", the internal conformance trace should be filtered
        let e = rx.try_recv().expect("Should have log event");
        assert!(
            e.message["message"]
                .as_str()
                .unwrap()
                .contains("real log line")
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_stream_output_filters_conformance_tracing_without_target() {
        // When the subprocess uses .with_target(false), the fmt output does not
        // include the "conformance" target. The filter should still suppress
        // these lines based on the "event_json=" marker alone.
        let input = "2026-04-10T01:31:57.857Z DEBUG logging.rs:33: event_json={\"pass\":true,\"comments\":[]}\nreal log line\n";
        let cursor = std::io::Cursor::new(input.as_bytes().to_vec());

        let (tx, mut rx) = broadcast::channel::<JobEvent>(16);
        let cancel = CancellationToken::new();

        let handle = tokio::spawn(stream_output(
            "job-1".to_string(),
            "test".to_string(),
            "stdout".to_string(),
            cursor,
            tx,
            cancel,
            false,
            true,
            Arc::new(HashMap::new()),
        ));

        handle.await.unwrap();

        // Only the real log line should come through; the event_json tracing line
        // should be filtered even though it lacks " conformance: " in the output.
        let e = rx.try_recv().expect("Should have log event");
        assert!(
            e.message["message"]
                .as_str()
                .unwrap()
                .contains("real log line")
        );
        assert!(rx.try_recv().is_err());
    }
}
