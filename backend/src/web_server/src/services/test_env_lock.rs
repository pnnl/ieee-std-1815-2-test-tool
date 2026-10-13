//! Shared test-only lock and restore-guard for env-var-mutating tests in
//! `job_service` and `process_manager`. `std::env::set_var`/`remove_var`
//! are unsafe under edition 2024 because they race any other thread
//! reading or writing the process environment, and cargo runs unit tests
//! in parallel by default. One lock, shared across both modules' test
//! suites, is what makes a SAFETY comment claiming "every test that
//! mutates env vars holds this lock" actually true.
//!
//! A test that mutates env vars holds `ENV_MUTEX` for its whole body, then
//! constructs a `ScopedEnvVar` per variable. `ScopedEnvVar` does not lock
//! `ENV_MUTEX` itself, so a test needing more than one variable does not
//! deadlock on its own lock; it restores each variable's previous value (or
//! absence) on drop, so a panicking assertion still leaves the environment
//! clean for the next test.

use tokio::sync::Mutex;

/// Process-wide async mutex serializing every test in this crate that
/// mutates an environment variable. Async (rather than `std::sync::Mutex`)
/// because `job_service`'s tests hold it across `.await` points while they
/// drive `JobService::create_job` through its background task.
pub(crate) static ENV_MUTEX: Mutex<()> = Mutex::const_new(());

/// RAII guard that sets or unsets an env var and restores the previous
/// value (or absence) on drop. The caller must hold `ENV_MUTEX` for the
/// guard's whole lifetime.
pub(crate) struct ScopedEnvVar {
    key: String,
    previous: Option<String>,
}

impl ScopedEnvVar {
    /// Set `key` to `value`. Caller must hold `ENV_MUTEX`.
    pub(crate) fn set(key: &str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: the caller holds ENV_MUTEX for this guard's whole
        // lifetime; every env-mutating test in this crate does the same.
        unsafe { std::env::set_var(key, value) };
        Self {
            key: key.to_string(),
            previous,
        }
    }

    /// Ensure `key` is unset. Caller must hold `ENV_MUTEX`.
    pub(crate) fn unset(key: &str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: see `set`.
        unsafe { std::env::remove_var(key) };
        Self {
            key: key.to_string(),
            previous,
        }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        // SAFETY: see `set`; the caller's ENV_MUTEX guard outlives this drop.
        unsafe {
            match self.previous.take() {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }
}
