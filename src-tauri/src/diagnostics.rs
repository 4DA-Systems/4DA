// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Diagnostics module for 4DA
//!
//! Provides runtime health information: memory usage, database size,
//! item counts, uptime, and source health summary.

use serde::Serialize;
use std::time::Instant;

use crate::db::Database;

/// Application start time — set once at startup
static APP_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Record the application start time (call once during setup)
pub fn record_start_time() {
    let _ = APP_START.set(Instant::now());
}

/// Diagnostics snapshot
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsSnapshot {
    /// Process memory in bytes (RSS approximation)
    pub memory_bytes: u64,
    /// Database file size in bytes
    pub db_size_bytes: u64,
    /// Number of source_items in database
    pub source_item_count: i64,
    /// Number of context_chunks in database
    pub context_chunk_count: i64,
    /// Number of feedback records
    pub feedback_count: i64,
    /// Application uptime in seconds
    pub uptime_secs: u64,
    /// Source health summary: (source_type, status)
    pub source_health: Vec<SourceHealthSummary>,
    /// Schema version
    pub schema_version: i64,
    /// Whether database size exceeds warning threshold (500MB)
    pub db_size_warning: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceHealthSummary {
    pub source_type: String,
    pub status: String,
    pub consecutive_failures: i64,
}

/// Collect a diagnostics snapshot
pub fn collect_diagnostics(db: &Database, db_path: &std::path::Path) -> DiagnosticsSnapshot {
    let stats = db.get_db_stats().unwrap_or_default();
    let schema_version = db.get_schema_version().unwrap_or(0);

    // Database file size
    let db_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    // Process memory (Windows: use GetProcessMemoryInfo or approximate)
    let memory_bytes = get_process_memory();

    // Uptime
    let uptime_secs = APP_START.get().map_or(0, |start| start.elapsed().as_secs());

    // Source health from database
    let source_health = db
        .get_source_health_summary()
        .unwrap_or_default()
        .into_iter()
        .map(|(source_type, status, failures)| SourceHealthSummary {
            source_type,
            status,
            consecutive_failures: failures,
        })
        .collect();

    let db_size_warning = db_size_bytes > 500 * 1024 * 1024; // 500MB

    if db_size_warning {
        tracing::warn!(
            target: "4da::diagnostics",
            size_mb = db_size_bytes / (1024 * 1024),
            "Database exceeds 500MB warning threshold"
        );
    }

    DiagnosticsSnapshot {
        memory_bytes,
        db_size_bytes,
        source_item_count: stats.source_items,
        context_chunk_count: stats.context_chunks,
        feedback_count: stats.feedback_count,
        uptime_secs,
        source_health,
        schema_version,
        db_size_warning,
    }
}

/// Log current process RSS at a named scoring/analysis stage. Used to bisect
/// the background-analysis native crash (Victauri findings #3): a large spike
/// across a stage points at OOM; a flat curve before death points at a native
/// fault (sqlite-vec / ONNX). Cheap — safe to call on the hot path.
pub(crate) fn log_rss(stage: &str) {
    let mb = get_process_memory() / (1024 * 1024);
    // Debug-level by default (enable with RUST_LOG=4da::rss=debug to bisect
    // memory). Only a genuinely dangerous level warns — above the normal
    // cross-encoder reranker peak (~1.7 GB on capable machines).
    tracing::debug!(target: "4da::rss", stage, rss_mb = mb, "scoring memory checkpoint");
    if mb > 2500 {
        tracing::warn!(target: "4da::rss", stage, rss_mb = mb, "HIGH RSS — possible OOM approaching");
    }
}

/// Get approximate process RSS memory in bytes
pub(crate) fn get_process_memory() -> u64 {
    #[cfg(target_os = "windows")]
    {
        get_process_memory_windows()
    }

    #[cfg(target_os = "macos")]
    {
        get_process_memory_macos()
    }

    #[cfg(target_os = "linux")]
    {
        get_process_memory_linux()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        0
    }
}

/// Windows: use GetProcessMemoryInfo via Win32 FFI
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn get_process_memory_windows() -> u64 {
    use std::mem::MaybeUninit;
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            ppsmemcounters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    unsafe {
        let mut pmc = MaybeUninit::<ProcessMemoryCounters>::zeroed();
        let size = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        (*pmc.as_mut_ptr()).cb = size;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), pmc.as_mut_ptr(), size) != 0 {
            return (*pmc.as_ptr()).working_set_size as u64;
        }
    }
    0
}

/// macOS: use mach kernel task_info API for resident memory size
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn get_process_memory_macos() -> u64 {
    use std::mem::{size_of, MaybeUninit};

    // Mach kernel types and constants
    type MachPortT = u32;
    type KernReturnT = i32;
    type TaskFlavorT = u32;
    type MachMsgTypeNumberT = u32;

    const KERN_SUCCESS: KernReturnT = 0;
    const MACH_TASK_BASIC_INFO: TaskFlavorT = 20;

    // mach_task_basic_info struct layout (matching XNU headers)
    // Fields: virtual_size (u64), resident_size (u64), resident_size_max (u64),
    //         user_time (time_value_t = 2xi32), system_time (time_value_t = 2xi32),
    //         policy (i32), suspend_count (i32)
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time_secs: i32,
        user_time_usecs: i32,
        system_time_secs: i32,
        system_time_usecs: i32,
        policy: i32,
        suspend_count: i32,
    }

    extern "C" {
        fn mach_task_self() -> MachPortT;
        fn task_info(
            target_task: MachPortT,
            flavor: TaskFlavorT,
            task_info_out: *mut MachTaskBasicInfo,
            task_info_out_cnt: *mut MachMsgTypeNumberT,
        ) -> KernReturnT;
    }

    unsafe {
        let mut info = MaybeUninit::<MachTaskBasicInfo>::zeroed();
        // Count is in units of natural_t (u32), i.e. struct size / 4
        let mut count = (size_of::<MachTaskBasicInfo>() / size_of::<u32>()) as MachMsgTypeNumberT;

        let kr = task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            info.as_mut_ptr(),
            &mut count,
        );

        if kr == KERN_SUCCESS {
            return (*info.as_ptr()).resident_size;
        }
    }

    // Fallback: shell out to ps if mach API fails
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u64>()
                .ok()
                .map(|kb| kb * 1024)
        })
        .unwrap_or(0)
}

/// Parse VmRSS from /proc/self/status content, returning bytes.
/// Extracted as a standalone function so it can be tested on any platform.
#[cfg(any(target_os = "linux", test))]
fn parse_vmrss_from_status(content: &str) -> u64 {
    content
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        })
        .unwrap_or(0)
}

/// Linux: read /proc/self/status for VmRSS
#[cfg(target_os = "linux")]
fn get_process_memory_linux() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .map(|s| parse_vmrss_from_status(&s))
        .unwrap_or(0)
}

// ============================================================================
// Local diagnostic report export (replaces third-party crash reporting)
//
// Nothing here transmits anywhere. The report is assembled from the local
// diagnostics snapshot plus a tail of the on-device rotating log, scrubbed of
// usernames and secret-shaped tokens, and written to disk for the user to
// review and *choose* to attach to a bug report. This is the local-first
// replacement for the removed Sentry integration.
// ============================================================================

/// A diagnostic report the user can review and choose to share.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    /// The full, scrubbed, human-readable report text (safe to share).
    pub report: String,
    /// Absolute path the report was written to (for the UI only — NOT included
    /// in `report`, because the path contains the local username).
    pub saved_path: String,
}

/// Redact usernames in filesystem paths and secret-shaped tokens from text
/// before it goes into an exportable, user-shareable diagnostic report.
///
/// Defense-in-depth: most secrets are already redacted at their log site
/// (`sanitize_api_error` in `llm.rs`, `SensitiveString` Debug impls), but a
/// bundle a user might post publicly must not rely on that alone.
pub fn scrub(input: &str) -> String {
    redact_secret_tokens(&redact_user_paths(input))
}

/// Replace the path segment immediately following a known user-dir marker
/// (`C:\Users\name`, `/Users/name`, `/home/name`) with `<user>`.
fn redact_user_paths(input: &str) -> String {
    input
        .split('\n')
        .map(redact_user_paths_in_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_user_paths_in_line(line: &str) -> String {
    const MARKERS: &[&str] = &["\\Users\\", "/Users/", "/home/"];
    let mut out = line.to_string();
    for marker in MARKERS {
        let mut search_from = 0;
        while let Some(rel) = out[search_from..].find(marker) {
            let seg_start = search_from + rel + marker.len();
            let rest = &out[seg_start..];
            let seg_end_rel = rest
                .find(|c: char| {
                    c == '/' || c == '\\' || c.is_whitespace() || c == '"' || c == '\'' || c == ':'
                })
                .unwrap_or(rest.len());
            let seg = &rest[..seg_end_rel];
            if seg.is_empty() || seg == "<user>" {
                // Already redacted or empty — advance past it to avoid looping.
                search_from = seg_start + seg_end_rel;
                continue;
            }
            let suffix = out[seg_start + seg.len()..].to_string();
            out = format!("{}<user>{suffix}", &out[..seg_start]);
            search_from = seg_start + "<user>".len();
        }
    }
    out
}

/// Replace whitespace-delimited tokens that match a known secret shape with
/// `<redacted>` (preserving any `key=` label). Conservative on purpose so
/// debugging identifiers (commit hashes, item IDs) survive.
fn redact_secret_tokens(input: &str) -> String {
    input
        .split('\n')
        .map(|line| {
            line.split(' ')
                .map(redact_token)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_token(token: &str) -> String {
    // `key=value` form: inspect the value (the secret usually lives there) and
    // redact while preserving the label.
    if let Some(eq) = token.find('=') {
        let value = &token[eq + 1..];
        let core = value.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if looks_like_secret(core) {
            return format!("{}=<redacted>", &token[..eq]);
        }
    }
    // Bare token: test the alphanumeric core (strip surrounding punctuation).
    let core = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if looks_like_secret(core) {
        return "<redacted>".to_string();
    }
    token.to_string()
}

fn looks_like_secret(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("sk-")
        || lower.starts_with("pk_")
        || lower.starts_with("4da-")
        || lower.starts_with("bearer")
        || lower.starts_with("ghp_")
        || is_keygen_key(s)
}

/// Keygen license format: `BE####-######-…` (uppercase hex groups, hyphens).
fn is_keygen_key(s: &str) -> bool {
    s.len() >= 14
        && s.starts_with("BE")
        && s.contains('-')
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Read the newest rotating log file's last `max_lines` lines from the log dir.
fn read_recent_log_tail(max_lines: usize) -> String {
    let dir = crate::log_retention::log_dir();
    let newest = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(crate::log_retention::LOG_FILE_STEM))
        })
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

    let Some(path) = newest else {
        return "(no log file found)".to_string();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(max_lines);
            lines[start..].join("\n")
        }
        Err(e) => format!("(could not read log file: {e})"),
    }
}

fn format_source_health(health: &[SourceHealthSummary]) -> String {
    if health.is_empty() {
        return "  (none)".to_string();
    }
    health
        .iter()
        .map(|h| {
            format!(
                "  {} — {} ({} consecutive failures)",
                h.source_type, h.status, h.consecutive_failures
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Number of trailing log lines included in an exported report.
const REPORT_LOG_LINES: usize = 400;

/// Build a scrubbed diagnostic report and write it to
/// `<data_dir>/diagnostics/4da-diagnostics-<unix>.txt`. NOTHING is transmitted.
pub fn export_diagnostic_report(
    snapshot: &DiagnosticsSnapshot,
) -> crate::error::Result<DiagnosticReport> {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let body = format!(
        "4DA Diagnostic Report\n\
         =====================\n\
         Generated (unix): {unix}\n\
         App version: {ver}\n\
         OS / arch: {os} / {arch}\n\
         Uptime (s): {uptime}\n\
         \n\
         -- Database --\n\
         Schema version: {schema}\n\
         DB size (bytes): {dbsize}{warn}\n\
         Source items: {items}\n\
         Context chunks: {chunks}\n\
         Feedback records: {fb}\n\
         Process memory (bytes): {mem}\n\
         \n\
         -- Source health --\n\
         {health}\n\
         \n\
         -- Recent log (last {n} lines, scrubbed) --\n\
         {log}\n\
         \n\
         (Nothing in this report was transmitted. It was written to disk for you\n\
         to review and attach to a bug report if you choose.)\n",
        ver = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        uptime = snapshot.uptime_secs,
        schema = snapshot.schema_version,
        dbsize = snapshot.db_size_bytes,
        warn = if snapshot.db_size_warning {
            " (WARNING: exceeds 500MB)"
        } else {
            ""
        },
        items = snapshot.source_item_count,
        chunks = snapshot.context_chunk_count,
        fb = snapshot.feedback_count,
        mem = snapshot.memory_bytes,
        health = format_source_health(&snapshot.source_health),
        n = REPORT_LOG_LINES,
        log = read_recent_log_tail(REPORT_LOG_LINES),
    );

    // Scrub the entire report (the log tail is the main risk surface).
    let report = scrub(&body);

    let dir = crate::runtime_paths::RuntimePaths::get()
        .data_dir
        .join("diagnostics");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("4da-diagnostics-{unix}.txt"));

    match std::fs::write(&path, &report) {
        Ok(()) => Ok(DiagnosticReport {
            report,
            saved_path: path.display().to_string(),
        }),
        Err(e) => {
            tracing::warn!(target: "4da::diagnostics", error = %e, "Failed to write diagnostic report file");
            // Still hand back the text even if the disk write failed.
            Ok(DiagnosticReport {
                report,
                saved_path: String::new(),
            })
        }
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
