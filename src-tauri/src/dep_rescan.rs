// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! When is the dependency inventory due for a rescan, and the GUI's
//! freshness-gated mid-session rescan.
//!
//! The headless engine and the GUI's scheduled cycle share the due-logic
//! here: rescan when never scanned, when a watched manifest/lockfile changed
//! since the last scan (an mtime walk, cheap when nothing changed), or when
//! the last scan is older than the freshness budget.
//!
//! Live 2026-10-24: the GUI scanned lockfiles once at startup; PR #1007 then
//! bumped tokio and zod in the local lockfiles, and the scheduled cycle never
//! re-read them, so hours later the brief told the user to fix advisories the
//! lockfiles had already cleared. [`rescan_if_due`] closes that gap.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::Emitter;
use tracing::{debug, info, warn};

use crate::inventory_freshness::RefreshGuard;

/// Re-scan budget for the ACE dependency scan, in hours. Env-overridable via
/// `FOURDA_DEP_SCAN_MAX_AGE_HOURS` (positive integer; blank/invalid falls back to the default).
/// Mirrors the OSV-sync freshness knob so a deployment can tune dep-scan cadence the same way.
fn dep_scan_max_age_hours() -> i64 {
    parse_dep_scan_max_age(std::env::var("FOURDA_DEP_SCAN_MAX_AGE_HOURS").ok())
}

/// Pure parser for [`dep_scan_max_age_hours`]: a positive integer hours value; any
/// missing/blank/non-numeric/non-positive input falls back to the 24h default.
fn parse_dep_scan_max_age(raw: Option<String>) -> i64 {
    const DEFAULT_DEP_SCAN_MAX_AGE_HOURS: i64 = 24;
    raw.and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|&h| h > 0)
        .unwrap_or(DEFAULT_DEP_SCAN_MAX_AGE_HOURS)
}

/// Directories never descended into when checking manifest freshness — large, churning, or
/// vendored trees that never hold a first-party manifest worth re-scanning for.
const FRESHNESS_SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "vendor",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".cargo",
    ".gradle",
];

/// Manifest + lockfile names that drive the dependency profile. Kept in sync with
/// `ace::scanner` ManifestType + the lockfile processors; `.csproj` is matched by extension.
const WATCHED_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "npm-shrinkwrap.json",
    "requirements.txt",
    "pyproject.toml",
    "poetry.lock",
    "uv.lock",
    "pdm.lock",
    "Pipfile",
    "Pipfile.lock",
    "go.mod",
    "go.sum",
    "Gemfile",
    "Gemfile.lock",
    "composer.json",
    "composer.lock",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "pubspec.yaml",
    "pubspec.lock",
];

/// True if any watched manifest/lockfile under `dir` (bounded depth, skipping vendored/hidden
/// trees) was modified strictly after `since_unix_secs` — i.e. a dependency was added/removed
/// or a version bumped since the last scan, so the dep profile is stale.
fn manifest_changed_since(dir: &Path, since_unix_secs: i64, depth: u8) -> bool {
    if depth > 6 {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let name = entry.file_name();
        let n = name.to_string_lossy();
        if ft.is_dir() {
            if FRESHNESS_SKIP_DIRS.contains(&n.as_ref()) || n.starts_with('.') {
                continue;
            }
            if manifest_changed_since(&entry.path(), since_unix_secs, depth + 1) {
                return true;
            }
        } else if ft.is_file()
            && (WATCHED_MANIFESTS.contains(&n.as_ref()) || n.ends_with(".csproj"))
        {
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                if let Ok(d) = modified.duration_since(std::time::UNIX_EPOCH) {
                    if d.as_secs() as i64 > since_unix_secs {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Decide whether the ACE dependency scan is due. `last_scan` is the most recent scan time
/// (`detected_projects.updated_at`, a sqlite `datetime('now')` UTC string) or `None` if never
/// scanned. Re-scan when: never scanned; a watched manifest/lockfile changed since the last
/// scan (prompt pickup of a dep add/bump on a running deployment); or the last scan is older
/// than the freshness budget (safety net for lockfile/transitive drift the mtime walk misses).
fn dep_scan_due(dirs: &[PathBuf], last_scan: Option<&str>) -> bool {
    let Some(last) = last_scan else {
        return true;
    };
    let Ok(naive) = chrono::NaiveDateTime::parse_from_str(last, "%Y-%m-%d %H:%M:%S") else {
        return true; // unparseable timestamp — re-scan rather than wedge on a stale profile
    };
    let now = chrono::Utc::now().naive_utc();
    if (now - naive).num_hours() >= dep_scan_max_age_hours() {
        return true;
    }
    let since = naive.and_utc().timestamp();
    dirs.iter().any(|d| manifest_changed_since(d, since, 0))
}

/// The configured context directories to hand `ace_full_scan`, when the dependency scan is
/// DUE (see [`dep_scan_due`]); `None` when nothing is configured or nothing changed.
/// Synchronous filesystem walk: async callers on a shared runtime run it on a blocking thread.
pub(crate) fn due_scan_paths() -> Option<Vec<String>> {
    let dirs = crate::get_context_dirs();
    if dirs.is_empty() {
        return None;
    }
    let last_scan = crate::get_database()
        .ok()
        .and_then(|db| db.last_ace_scan_time().ok().flatten());
    if !dep_scan_due(&dirs, last_scan.as_deref()) {
        return None;
    }
    Some(
        dirs.iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    )
}

/// What a gated rescan did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RescanOutcome {
    /// Another scan (startup, or an earlier rescan) is in flight: nothing ran.
    Skipped,
    /// The scan ran to its end (success or logged failure).
    Finished,
    /// The scan outlived `wait`; it keeps running and keeps the scan slot.
    StillRunning,
}

/// Run `scan` under a refresh guard from `begin`, waiting at most `wait` for it.
/// The guard moves into the spawned scan task, so the gate reopens when the scan ends,
/// panics, or hits the gate's deadline — and the scan slot stays held until the scan
/// itself ends, so no second scan starts beside a slow one.
async fn run_gated_rescan<Fut>(
    begin: impl FnOnce() -> Option<RefreshGuard>,
    scan: Fut,
    wait: Duration,
) -> RescanOutcome
where
    Fut: Future<Output = ()> + Send + 'static,
{
    let Some(guard) = begin() else {
        return RescanOutcome::Skipped;
    };
    let task = tauri::async_runtime::spawn(async move {
        let _guard = guard;
        scan.await;
    });
    match tokio::time::timeout(wait, task).await {
        Ok(_) => RescanOutcome::Finished,
        Err(_) => RescanOutcome::StillRunning,
    }
}

/// The GUI scheduled cycle's dependency rescan: re-read the lockfiles when they changed
/// since the last scan (or the scan is older than its budget), with dependency facts
/// gated meanwhile. Cheap when nothing changed (one mtime walk, off the async runtime).
/// Skips while the startup scan or another rescan runs. On completion the Preemption feed
/// is rebuilt against the new inventory and the UI is told, like the startup path.
pub(crate) async fn rescan_if_due(handle: &tauri::AppHandle) -> RescanOutcome {
    if crate::inventory_freshness::scan_in_flight() {
        debug!(target: "4da::inventory", "Dependency rescan check skipped: a scan is in flight");
        return RescanOutcome::Skipped;
    }
    let due = tauri::async_runtime::spawn_blocking(due_scan_paths)
        .await
        .ok()
        .flatten();
    let Some(paths) = due else {
        return RescanOutcome::Finished;
    };
    let emit_handle = handle.clone();
    let begin = move || {
        crate::inventory_freshness::try_begin_session_refresh(move || {
            crate::preemption::refresh_preemption_cache_after_cycle();
            let _ = emit_handle.emit(crate::inventory_freshness::INVENTORY_FRESH_EVENT, ());
        })
    };
    let scan = async move {
        info!(target: "4da::inventory", dirs = paths.len(), "Manifests or lockfiles changed since the last scan: rescanning dependencies");
        match crate::ace_commands::ace_full_scan(paths).await {
            Ok(_) => info!(target: "4da::inventory", "Dependency rescan complete"),
            Err(e) => {
                warn!(target: "4da::inventory", error = %e, "Dependency rescan failed: the stored inventory stands");
            }
        }
    };
    let outcome = run_gated_rescan(
        begin,
        scan,
        crate::inventory_freshness::STARTUP_SCAN_DEADLINE,
    )
    .await;
    if outcome == RescanOutcome::StillRunning {
        warn!(target: "4da::inventory", "Dependency rescan still running past its deadline: the cycle continues on the stored inventory");
    }
    outcome
}

#[cfg(test)]
#[path = "dep_rescan_tests.rs"]
mod tests;
