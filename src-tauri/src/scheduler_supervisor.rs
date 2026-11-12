// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Supervision for the GUI's background scheduler (audit O2).
//!
//! The scheduler (`monitoring::start_scheduler`) is one loop that awaits
//! about twenty jobs in turn. Before this module none of them had a time
//! limit or panic containment, the loop's task handle was dropped, and the
//! watchdog that names a stalled engine ran as a sun INSIDE that loop. One
//! hung child process (a `Get-CimInstance` that never returns) or one panic
//! ended background syncing for the rest of the session, silently — and the
//! watchdog stopped with it.
//!
//! Three pieces:
//! - [`run_job`] / [`run_blocking_job`]: every job runs under a time budget
//!   and panic containment. A job that overruns or panics is logged and
//!   recorded as that job's outcome, and the loop moves on to the next job.
//! - [`supervise`]: the loop itself runs contained; if it ever unwinds it is
//!   restarted after a pause instead of disappearing.
//! - [`spawn_watchdog`]: the engine watchdog, plus a check of the loop's own
//!   heartbeat, on an independent task, so a stalled loop is named.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tracing::{error, warn};

/// Unix seconds of the scheduler loop's last completed tick (0 = never).
static LAST_TICK_UNIX: AtomicU64 = AtomicU64::new(0);

/// The loop is "stalled" when no tick completed for this long. Ticks are a
/// minute apart and the longest job budget is [`LONG_JOB_BUDGET`], so this
/// only trips when something is stuck past its budget.
pub(crate) const STALL_AFTER: Duration = Duration::from_mins(20);

/// Budget for an ordinary maintenance job.
pub(crate) const JOB_BUDGET: Duration = Duration::from_mins(5);
/// Budget for the jobs that legitimately run long (the daily decay +
/// retention pass, the morning brief's summary on a local model).
pub(crate) const LONG_JOB_BUDGET: Duration = Duration::from_mins(15);

/// How the job ended, when it did not end normally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobFailure {
    TimedOut(Duration),
    Panicked,
}

impl std::fmt::Display for JobFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut(d) => write!(f, "timed out after {} s", d.as_secs()),
            Self::Panicked => write!(f, "panicked"),
        }
    }
}

/// Run an async job under `budget` with panic containment. `Err` when it
/// overran (the future is dropped at its next await) or panicked; the failure
/// is logged and written as the job's `scheduler_state` outcome.
pub(crate) async fn run_job<F>(
    name: &'static str,
    budget: Duration,
    fut: F,
) -> Result<F::Output, JobFailure>
where
    F: Future,
{
    let outcome = match tokio::time::timeout(budget, crate::task_guard::contain(name, fut)).await {
        Ok(Some(v)) => return Ok(v),
        Ok(None) => JobFailure::Panicked,
        Err(_) => JobFailure::TimedOut(budget),
    };
    error!(target: "4da::monitor", job = name, failure = %outcome, "Scheduler job did not finish — the loop continues");
    crate::scheduler_state::record_outcome(
        name,
        Some(&outcome.to_string()),
        u64::try_from(budget.as_millis()).unwrap_or(u64::MAX),
    );
    Err(outcome)
}

/// Run a synchronous job on the blocking pool under `budget`. A synchronous
/// body cannot be interrupted, so an overrun leaves it running on its pool
/// thread while the loop moves on; a panic is caught by the pool.
pub(crate) async fn run_blocking_job<T, F>(
    name: &'static str,
    budget: Duration,
    body: F,
) -> Result<T, JobFailure>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    // A real pool thread (`spawn_blocking`), not `ipc_blocking`'s
    // `block_in_place`: that runs the body on this task, where no timeout can
    // reach it.
    match run_job(name, budget, tokio::task::spawn_blocking(body)).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            error!(target: "4da::monitor", job = name, error = %e, "Scheduler job panicked on the blocking pool");
            crate::scheduler_state::record_outcome(name, Some("panicked"), 0);
            Err(JobFailure::Panicked)
        }
        Err(f) => Err(f),
    }
}

/// Record that the loop finished a tick.
pub(crate) fn mark_tick() {
    LAST_TICK_UNIX.store(now_unix(), Ordering::Relaxed);
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Pure stall decision: `Some(seconds since the last tick)` when stalled.
/// A loop that never ticked yet (boot grace) is not stalled.
pub(crate) fn stalled_for(
    last_tick_unix: u64,
    now_unix: u64,
    stall_after: Duration,
) -> Option<u64> {
    if last_tick_unix == 0 {
        return None;
    }
    let age = now_unix.saturating_sub(last_tick_unix);
    (age > stall_after.as_secs()).then_some(age)
}

/// Run `make_loop()` forever: when the loop unwinds (or, impossibly, returns)
/// it is logged and started again after `pause`. The scheduler loop never
/// returns on its own, so every restart is a recovered failure.
pub(crate) async fn supervise<F, Fut>(name: &'static str, pause: Duration, mut make_loop: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    let mut restarts: u64 = 0;
    loop {
        let panicked = crate::task_guard::contain(name, make_loop())
            .await
            .is_none();
        restarts += 1;
        error!(
            target: "4da::monitor",
            task = name,
            panicked,
            restarts,
            "Scheduler loop ended — restarting it"
        );
        tokio::time::sleep(pause).await;
    }
}

/// The watchdog's cadence.
const WATCHDOG_EVERY: Duration = Duration::from_mins(15);

/// Start the watchdog on its own task, outside the loop it watches: the
/// engine-staleness check (`suns::engine_watchdog`) and the loop heartbeat.
pub(crate) fn spawn_watchdog() {
    tauri::async_runtime::spawn(async {
        let mut every = tokio::time::interval(WATCHDOG_EVERY);
        loop {
            every.tick().await;
            if let Some(age) = stalled_for(
                LAST_TICK_UNIX.load(Ordering::Relaxed),
                now_unix(),
                STALL_AFTER,
            ) {
                let msg = format!(
                    "Background scheduler stalled: no tick for {} minutes. Syncing and \
                     maintenance are paused; restart 4DA if this persists.",
                    age / 60
                );
                warn!(target: "4da::monitor", age_secs = age, "Background scheduler stalled");
                crate::suns::store_sun_alert("scheduler_watchdog", "scheduler_stalled", &msg);
            }
            if let Err(f) = run_blocking_job(
                "engine_watchdog",
                JOB_BUDGET,
                crate::suns::engine_watchdog::execute,
            )
            .await
            {
                warn!(target: "4da::monitor", failure = %f, "Engine watchdog did not finish");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    /// A job that never finishes no longer holds the loop: it is cut at its
    /// budget and the next job runs (audit O2).
    #[tokio::test]
    async fn a_hung_job_times_out_and_the_next_job_runs() {
        let hung = run_job(
            "test_hung",
            Duration::from_millis(50),
            std::future::pending::<()>(),
        )
        .await;
        assert_eq!(hung, Err(JobFailure::TimedOut(Duration::from_millis(50))));
        let next = run_job("test_next", Duration::from_secs(5), async { 7 }).await;
        assert_eq!(next, Ok(7));
    }

    #[tokio::test]
    async fn a_panicking_job_is_contained() {
        let r: Result<(), _> = run_job("test_panic", Duration::from_secs(5), async {
            tokio::task::yield_now().await;
            panic!("job boom")
        })
        .await;
        assert_eq!(r, Err(JobFailure::Panicked));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_blocking_job_overrun_frees_the_loop() {
        let started = std::time::Instant::now();
        let r = run_blocking_job("test_block", Duration::from_millis(100), || {
            std::thread::sleep(Duration::from_secs(2));
        })
        .await;
        assert_eq!(r, Err(JobFailure::TimedOut(Duration::from_millis(100))));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the loop did not wait for the body"
        );
        let p: Result<(), _> = run_blocking_job("test_block_panic", Duration::from_secs(5), || {
            panic!("pool boom")
        })
        .await;
        assert_eq!(p, Err(JobFailure::Panicked));
    }

    /// A loop that unwinds is started again.
    #[tokio::test]
    async fn a_panicking_loop_is_restarted() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let supervised = supervise("test_loop", Duration::from_millis(1), move || {
            let counter = Arc::clone(&counter);
            async move {
                assert!(counter.fetch_add(1, Ordering::SeqCst) >= 2, "loop boom");
                std::future::pending::<()>().await;
            }
        });
        // Generous: the process panic hook writes each panic to disk with a
        // blocking fsync, which a loaded machine can make slow.
        let _ = tokio::time::timeout(Duration::from_secs(10), supervised).await;
        assert_eq!(
            runs.load(Ordering::SeqCst),
            3,
            "two panics, two restarts, third run lives"
        );
    }

    #[test]
    fn stall_is_named_only_past_the_threshold() {
        let t = Duration::from_mins(20);
        assert_eq!(stalled_for(0, 10_000, t), None, "never ticked: boot grace");
        assert_eq!(stalled_for(10_000, 10_600, t), None);
        assert_eq!(stalled_for(10_000, 11_201, t), Some(1201));
    }
}
