// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Machine contention: 4DA's background work steps aside while the user is
//! using the machine hard (a game, VR, a presentation).
//!
//! Measured 2026-11-03 (RTX 4080 SUPER, i9-9900K, Resident Evil 4 in VR):
//! 4DA's scheduled cycles kept running beside the game. The engine's embedding
//! batches pinned four CPU threads through Ollama, and the judge model took
//! 8.25 GB of the 16 GB card, pushing 1.2 GB of the game's textures into system
//! RAM. The game stuttered unplayably on every area load. Nothing in 4DA asked
//! whether the machine was busy: `presence` only gated notifications.
//!
//! Two guards, both automatic:
//! - **Cycles wait.** A *scheduled* cycle (GUI scheduler, `fourda-engine`
//!   once/daemon) is skipped while [`current`] reports contention; the next
//!   tick retries. A cycle the user starts is never deferred. Deferral is
//!   bounded by [`MAX_DEFER`]: past it the cycle runs anyway, so a marathon
//!   session cannot leave dependency intelligence stale indefinitely.
//! - **The engine runs below normal priority** ([`lower_own_priority`]), so
//!   work already in flight when a game starts never preempts it.
//!
//! The judge lanes add a third, finer guard: `local_judge` checks live VRAM
//! before every load and unloads a resident judge when a game needs the GPU.

use std::time::Duration;

use tracing::info;

use crate::presence::BusyReason;

/// GPU utilization (percent, `nvidia-smi utilization.gpu`) that marks the GPU
/// as busy with another app. An idle desktop, video playback (decoder engine)
/// and browsing sit far below it; a game or VR session sits near 100%.
pub(crate) const GPU_BUSY_PCT: u32 = 50;

/// The longest a scheduled cycle waits behind a busy machine. Past it the
/// cycle runs anyway (below normal priority, judge still yielding VRAM).
pub(crate) const MAX_DEFER: Duration = Duration::from_hours(4);

/// Samples taken for the GPU check, and the gap between them. One reading can
/// land on a spike (a browser tab compositing); a majority of three cannot.
const GPU_SAMPLES: usize = 3;
const GPU_SAMPLE_GAP: Duration = Duration::from_millis(400);

/// Why 4DA's heavy background work is waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Contention {
    /// A fullscreen app (exclusive or borderless) or a presentation owns the screen.
    Screen(BusyReason),
    /// Another app is driving the GPU. Catches VR and windowed games, which
    /// the OS fullscreen check cannot see (a VR game renders in a window).
    Gpu { utilization_pct: u32 },
}

impl Contention {
    /// Stable tag for logs and run receipts.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Screen(r) => r.as_str(),
            Self::Gpu { .. } => "gpu_busy",
        }
    }
}

/// Which OS presence states defer compute. Away and locked are the BEST time
/// for background work, and Do Not Disturb / Focus Assist are about
/// interruptions, not load, so only a busy screen counts.
pub(crate) fn screen_defers(reason: BusyReason) -> bool {
    matches!(
        reason,
        BusyReason::FullscreenApp | BusyReason::FullscreenWindow | BusyReason::Presentation
    )
}

/// Whether the GPU readings show another app driving it.
///
/// `samples` are utilization percents; a majority must reach [`GPU_BUSY_PCT`].
/// When Ollama has a model on the GPU, utilization may be 4DA's own judge, so
/// the reading that counts is VRAM held by everything else: 40% of the card
/// taken by other apps means a game or renderer is running. Measured
/// 2026-11-03 (16,376 MiB card): idle desktop beside the judge 1.9 GB (12%),
/// RE4 VR plus desktop 7.6 GB (47%).
pub(crate) fn gpu_contended(
    samples: &[u32],
    used_mb: u64,
    total_mb: u64,
    ollama_resident_mb: u64,
) -> bool {
    if ollama_resident_mb > 0 {
        let other = used_mb.saturating_sub(ollama_resident_mb);
        return total_mb > 0 && other * 5 >= total_mb * 2;
    }
    let hot = samples.iter().filter(|&&s| s >= GPU_BUSY_PCT).count();
    !samples.is_empty() && hot * 2 > samples.len()
}

/// Apply the [`MAX_DEFER`] bound. `last_fetch_age` is how long ago the sources
/// were last fetched; unknown means never, and a cycle that never ran is not
/// deferred.
pub(crate) fn bounded(
    contention: Option<Contention>,
    last_fetch_age: Option<Duration>,
) -> Option<Contention> {
    match last_fetch_age {
        Some(age) if age < MAX_DEFER => contention,
        _ => None,
    }
}

/// One `nvidia-smi` reading: `(utilization %, memory used MiB, total MiB)`.
pub(crate) fn parse_gpu_line(line: &str) -> Option<(u32, u64, u64)> {
    let mut parts = line.split(',').map(str::trim);
    let util = parts.next()?.parse().ok()?;
    let used = parts.next()?.parse().ok()?;
    let total = parts.next()?.parse().ok()?;
    Some((util, used, total))
}

/// Blocking: [`GPU_SAMPLES`] readings ~[`GPU_SAMPLE_GAP`] apart. `None` on a
/// non-NVIDIA GPU or a failed probe; the screen check still applies then.
fn sample_nvidia() -> Option<(Vec<u32>, u64, u64)> {
    let mut utils = Vec::with_capacity(GPU_SAMPLES);
    let mut last = None;
    for i in 0..GPU_SAMPLES {
        if i > 0 {
            std::thread::sleep(GPU_SAMPLE_GAP);
        }
        let out = crate::hardware_detect::run_quiet(
            "nvidia-smi",
            &[
                "--query-gpu=utilization.gpu,memory.used,memory.total",
                "--format=csv,noheader,nounits",
            ],
        )?;
        let (util, used, total) = parse_gpu_line(out.lines().next()?)?;
        utils.push(util);
        last = Some((used, total));
    }
    let (used, total) = last?;
    Some((utils, used, total))
}

/// VRAM (MiB) Ollama's loaded models hold, per `/api/ps`; 0 when unreachable.
async fn ollama_resident_mb() -> u64 {
    let Ok(client) = crate::http_client::outbound_builder()
        .timeout(Duration::from_secs(3))
        .build()
    else {
        return 0;
    };
    let url = format!("{}/api/ps", crate::local_judge::DEFAULT_OLLAMA_URL);
    let Ok(resp) = client.get(url).send().await else {
        return 0;
    };
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return 0;
    };
    body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["size_vram"].as_u64())
        .sum::<u64>()
        / (1024 * 1024)
}

/// How long ago the sources were last fetched (UTC `YYYY-MM-DD HH:MM:SS`).
fn last_fetch_age() -> Option<Duration> {
    let snap = crate::engine_runs::freshness_snapshot().ok()?;
    let naive =
        chrono::NaiveDateTime::parse_from_str(&snap.last_source_fetch?, "%Y-%m-%d %H:%M:%S")
            .ok()?;
    let secs = chrono::Utc::now()
        .signed_duration_since(naive.and_utc())
        .num_seconds();
    u64::try_from(secs).ok().map(Duration::from_secs)
}

/// What is contending for the machine right now, if anything (unbounded).
pub(crate) async fn current() -> Option<Contention> {
    if let Some(reason) = crate::presence::os_busy_reason().filter(|r| screen_defers(*r)) {
        return Some(Contention::Screen(reason));
    }
    let (samples, used, total) = tokio::task::spawn_blocking(sample_nvidia).await.ok()??;
    let resident = ollama_resident_mb().await;
    gpu_contended(&samples, used, total, resident).then(|| Contention::Gpu {
        utilization_pct: samples.iter().copied().max().unwrap_or(0),
    })
}

/// The gate for SCHEDULED heavy work: `Some(reason)` means skip this tick.
/// Logs the decision. Never call it for work the user started.
pub(crate) async fn defer_scheduled_cycle(trigger: &'static str) -> Option<Contention> {
    if std::env::var("FOURDA_IGNORE_CONTENTION").is_ok_and(|v| v == "1") {
        return None;
    }
    let found = current().await;
    let age = last_fetch_age();
    let deferred = bounded(found, age);
    match (found, deferred) {
        (Some(c), Some(_)) => info!(
            target: "4da::contention",
            trigger,
            reason = c.as_str(),
            last_fetch_min = age.map(|a| a.as_secs() / 60),
            "Machine busy (game, VR or presentation); scheduled cycle waits for the next tick"
        ),
        (Some(c), None) => info!(
            target: "4da::contention",
            trigger,
            reason = c.as_str(),
            "Machine busy, but the deferral bound is reached; running the cycle at background priority"
        ),
        _ => {}
    }
    deferred
}

/// Run this process below normal CPU priority, so engine work already in
/// flight when a game starts never preempts it. On Windows the children the
/// engine spawns (nvidia-smi, git, PowerShell probes) inherit the class.
/// `FOURDA_ENGINE_NORMAL_PRIORITY=1` opts out. Best-effort: a failure leaves
/// the default priority.
#[allow(unsafe_code)] // Intentional: one Win32 / libc call on the current process.
pub(crate) fn lower_own_priority() {
    if std::env::var("FOURDA_ENGINE_NORMAL_PRIORITY").is_ok_and(|v| v == "1") {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS,
        };
        // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no
        // closing; SetPriorityClass only reads it.
        let ok = unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) };
        info!(target: "4da::contention", ok = ok != 0, "Engine priority set to below normal");
    }
    #[cfg(unix)]
    {
        // SAFETY: setpriority on our own process (who = 0) has no memory effects.
        let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS as _, 0, 10) };
        info!(target: "4da::contention", ok = rc == 0, "Engine priority set to nice 10");
    }
}

#[cfg(test)]
#[path = "contention_tests.rs"]
mod tests;
