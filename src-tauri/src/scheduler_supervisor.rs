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

use tracing::{debug, error, info, warn};

/// Unix seconds when the scheduler last made progress: a tick began or a job
/// finished (0 = never).
static LAST_PROGRESS_UNIX: AtomicU64 = AtomicU64::new(0);

/// The loop counts as stalled when it made no progress for this long. The
/// longest job budget is [`LONG_JOB_BUDGET`] and every finished job counts
/// as progress, so a busy tick of several long jobs never trips it.
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
    /// The previous run of this blocking job is still on its pool thread
    /// (it overran its budget); this run was skipped rather than queued.
    StillRunning,
}

impl std::fmt::Display for JobFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut(d) => write!(f, "timed out after {} s", d.as_secs()),
            Self::Panicked => write!(f, "panicked"),
            Self::StillRunning => write!(f, "skipped: the previous run is still going"),
        }
    }
}

/// Run an async job under `budget` with panic containment. `Err` when it
/// overran (the future is dropped at its next await) or panicked; the failure
/// is logged and written as the job's `scheduler_state` outcome. Either way
/// the job has ended, which counts as scheduler progress.
pub(crate) async fn run_job<F>(
    name: &'static str,
    budget: Duration,
    fut: F,
) -> Result<F::Output, JobFailure>
where
    F: Future,
{
    let ended = tokio::time::timeout(budget, crate::task_guard::contain(name, fut)).await;
    mark_progress();
    let outcome = match ended {
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

/// Names of the blocking jobs whose body is still running on a pool thread.
fn in_flight() -> &'static parking_lot::Mutex<std::collections::HashSet<&'static str>> {
    static IN_FLIGHT: std::sync::OnceLock<
        parking_lot::Mutex<std::collections::HashSet<&'static str>>,
    > = std::sync::OnceLock::new();
    IN_FLIGHT.get_or_init(Default::default)
}

/// Clears a blocking job's in-flight mark when its body returns or unwinds.
struct InFlight(&'static str);

impl Drop for InFlight {
    fn drop(&mut self) {
        in_flight().lock().remove(self.0);
    }
}

/// Run a synchronous job on the blocking pool under `budget`. A synchronous
/// body cannot be interrupted, so an overrun leaves it running on its pool
/// thread while the loop moves on; a panic is caught by the pool. While a
/// body is still running, later runs of the same job are skipped
/// ([`JobFailure::StillRunning`]) instead of each taking another pool thread.
pub(crate) async fn run_blocking_job<T, F>(
    name: &'static str,
    budget: Duration,
    body: F,
) -> Result<T, JobFailure>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    if !in_flight().lock().insert(name) {
        debug!(target: "4da::monitor", job = name, "Previous run still going — skipped");
        return Err(JobFailure::StillRunning);
    }
    let guard = InFlight(name);
    // A real pool thread (`spawn_blocking`), not `ipc_blocking`'s
    // `block_in_place`: that runs the body on this task, where no timeout can
    // reach it.
    let task = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        body()
    });
    match run_job(name, budget, task).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            error!(target: "4da::monitor", job = name, error = %e, "Scheduler job panicked on the blocking pool");
            crate::scheduler_state::record_outcome(name, Some("panicked"), 0);
            Err(JobFailure::Panicked)
        }
        Err(f) => Err(f),
    }
}

/// Record that the scheduler made progress (a tick began, a job ended).
pub(crate) fn mark_progress() {
    LAST_PROGRESS_UNIX.store(now_unix(), Ordering::Relaxed);
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Pure stall test: `Some(seconds since the last progress)` when stalled.
/// A loop that never made progress yet (boot grace) is not stalled.
pub(crate) fn stalled_for(
    last_progress_unix: u64,
    now_unix: u64,
    stall_after: Duration,
) -> Option<u64> {
    if last_progress_unix == 0 {
        return None;
    }
    let age = now_unix.saturating_sub(last_progress_unix);
    (age > stall_after.as_secs()).then_some(age)
}

/// The shortest gap between the two readings that confirm a stall: longer
/// than one scheduler tick, so a loop that is alive has ticked in between.
const CONFIRM_GAP: Duration = Duration::from_mins(2);

/// What the watchdog concludes from one reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StallVerdict {
    /// Progress is recent. `resolve` when an alert raised earlier must now be
    /// cleared.
    Healthy { resolve: bool },
    /// The progress stamp is old, but nothing confirms it yet. After a sleep
    /// the wall clock jumps and the watchdog and the loop wake together, so
    /// the first stale reading is always this.
    Suspect,
    /// The same stale stamp was seen again at least [`CONFIRM_GAP`] later:
    /// the loop really made no progress. Carries the age in seconds.
    Stalled(u64),
}

/// The watchdog's memory between readings.
#[derive(Debug, Default)]
pub(crate) struct StallWatch {
    /// The stale stamp seen last, and the monotonic second it was seen at.
    suspect: Option<(u64, u64)>,
    raised: bool,
}

impl StallWatch {
    /// A watch for a new process: an alert left open by an earlier run is
    /// cleared on the first healthy reading.
    pub(crate) fn new() -> Self {
        Self {
            suspect: None,
            raised: true,
        }
    }

    /// One reading: `last_progress` and `now` are wall-clock unix seconds,
    /// `mono_now` a monotonic seconds counter (the confirm gap is measured on
    /// it, so a wall-clock step cannot fake one).
    pub(crate) fn observe(&mut self, last_progress: u64, now: u64, mono_now: u64) -> StallVerdict {
        let Some(age) = stalled_for(last_progress, now, STALL_AFTER) else {
            self.suspect = None;
            let resolve = std::mem::take(&mut self.raised);
            return StallVerdict::Healthy { resolve };
        };
        match self.suspect {
            Some((stamp, seen_at))
                if stamp == last_progress
                    && mono_now.saturating_sub(seen_at) >= CONFIRM_GAP.as_secs() =>
            {
                self.raised = true;
                StallVerdict::Stalled(age)
            }
            Some((stamp, _)) if stamp == last_progress => StallVerdict::Suspect,
            _ => {
                self.suspect = Some((last_progress, mono_now));
                StallVerdict::Suspect
            }
        }
    }
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
        let started = std::time::Instant::now();
        let mut watch = StallWatch::new();
        let mut every = tokio::time::interval(WATCHDOG_EVERY);
        // After a sleep, one late reading, not a burst of back-to-back ones.
        every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            every.tick().await;
            let verdict = watch.observe(
                LAST_PROGRESS_UNIX.load(Ordering::Relaxed),
                now_unix(),
                started.elapsed().as_secs(),
            );
            match verdict {
                StallVerdict::Stalled(age) => {
                    let msg = format!(
                        "Background scheduler stalled: no job finished for {} minutes. Syncing \
                         and maintenance are paused; restart 4DA if this persists.",
                        age / 60
                    );
                    warn!(target: "4da::monitor", age_secs = age, "Background scheduler stalled");
                    crate::suns::store_sun_alert("scheduler_watchdog", "scheduler_stalled", &msg);
                }
                StallVerdict::Healthy { resolve: true } => {
                    info!(target: "4da::monitor", "Background scheduler running again — stall alert resolved");
                    crate::suns::resolve_sun_alerts("scheduler_watchdog", "scheduler_stalled");
                }
                StallVerdict::Healthy { resolve: false } | StallVerdict::Suspect => {}
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

    /// A blocking job whose body overran is not started again until that
    /// body ends: no pile of blocked pool threads (mini-autophagy, the mini
    /// digest and the suns tick all run every minute).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_overrunning_blocking_job_is_skipped_until_it_ends() {
        let slow = || std::thread::sleep(Duration::from_millis(600));
        let first = run_blocking_job("test_inflight", Duration::from_millis(50), slow).await;
        assert_eq!(first, Err(JobFailure::TimedOut(Duration::from_millis(50))));
        let second = run_blocking_job("test_inflight", JOB_BUDGET, || 1).await;
        assert_eq!(
            second,
            Err(JobFailure::StillRunning),
            "the first body is still running"
        );
        tokio::time::sleep(Duration::from_millis(900)).await;
        let third = run_blocking_job("test_inflight", JOB_BUDGET, || 2).await;
        assert_eq!(third, Ok(2), "runs again once the first body ended");
    }

    /// Every finished job counts as progress, so a tick of several long jobs
    /// within budget never reads as a stall.
    #[tokio::test]
    async fn a_finished_job_is_progress() {
        LAST_PROGRESS_UNIX.store(1, Ordering::Relaxed);
        let _ = run_job("test_progress", JOB_BUDGET, async { 1 }).await;
        assert!(LAST_PROGRESS_UNIX.load(Ordering::Relaxed) > 1);
    }

    /// After a sleep the wall clock jumps and the watchdog wakes with the
    /// loop: one stale reading is only a suspicion. The loop ticks, the next
    /// reading is healthy, and no alert is raised.
    #[test]
    fn a_wake_from_sleep_is_not_a_stall() {
        let mut watch = StallWatch::default();
        let stamp = 1_000;
        let after_sleep = stamp + 8 * 3_600;
        assert_eq!(
            watch.observe(stamp, after_sleep, 100),
            StallVerdict::Suspect
        );
        // The loop ticked right after waking; the next reading is healthy.
        assert_eq!(
            watch.observe(after_sleep + 60, after_sleep + 900, 1_000),
            StallVerdict::Healthy { resolve: false }
        );
    }

    /// A real stall: the same stale stamp seen twice, far enough apart. The
    /// alert is resolved when progress resumes.
    #[test]
    fn a_confirmed_stall_raises_and_progress_resolves_it() {
        let mut watch = StallWatch::default();
        let stamp = 1_000;
        assert_eq!(
            watch.observe(stamp, stamp + 1_300, 100),
            StallVerdict::Suspect
        );
        assert_eq!(
            watch.observe(stamp, stamp + 1_310, 110),
            StallVerdict::Suspect,
            "a second reading seconds later (a burst after wake) confirms nothing"
        );
        assert_eq!(
            watch.observe(stamp, stamp + 2_200, 1_000),
            StallVerdict::Stalled(2_200)
        );
        assert_eq!(
            watch.observe(stamp + 2_250, stamp + 2_300, 1_100),
            StallVerdict::Healthy { resolve: true }
        );
        assert_eq!(
            watch.observe(stamp + 2_350, stamp + 2_400, 1_200),
            StallVerdict::Healthy { resolve: false }
        );
    }

    /// A new process clears an alert an earlier run left open.
    #[test]
    fn a_new_watch_resolves_a_leftover_alert_once() {
        let mut watch = StallWatch::new();
        assert_eq!(
            watch.observe(1_000, 1_010, 0),
            StallVerdict::Healthy { resolve: true }
        );
        assert_eq!(
            watch.observe(1_000, 1_020, 10),
            StallVerdict::Healthy { resolve: false }
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
