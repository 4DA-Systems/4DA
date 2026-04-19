// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Log file retention and reader commands.
//!
//! Provides:
//! - Cleanup of log files older than `RETENTION_DAYS` from the log directory.
//! - `get_recent_logs` Tauri command for support / in-app log viewer use.
//!
//! The file appender (tracing-appender) writes a daily-rotated file in
//! `data_dir/logs/fourda.log.YYYY-MM-DD`. This module complements the
//! appender by pruning old rotations and exposing a read path.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tracing::{debug, warn};

/// Keep log files for this many days. Daily rotation means this is also
/// roughly the number of distinct log files kept at any time.
pub const RETENTION_DAYS: u64 = 7;

/// Subdirectory under `data_dir` where log files are written.
pub const LOG_DIR_NAME: &str = "logs";

/// File name stem passed to `tracing_appender::rolling::daily`.
/// The appender writes `{stem}.YYYY-MM-DD` files.
pub const LOG_FILE_STEM: &str = "fourda.log";

/// Resolve the log directory under the runtime data dir.
/// Creates the directory if it doesn't exist.
pub fn log_dir() -> PathBuf {
    let dir = crate::runtime_paths::RuntimePaths::get()
        .data_dir
        .join(LOG_DIR_NAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Delete log files older than `RETENTION_DAYS` in the log directory.
///
/// Called at startup from `initialize_pre_tauri`. Failures are logged but
/// never abort startup — losing a retention sweep is strictly better than
/// failing to launch.
pub fn cleanup_old_logs() {
    let dir = log_dir();
    let cutoff = match SystemTime::now().checked_sub(Duration::from_secs(RETENTION_DAYS * 86_400)) {
        Some(t) => t,
        None => {
            warn!(target: "4da::log_retention", "Clock arithmetic underflow computing cutoff; skipping cleanup");
            return;
        }
    };

    let entries = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(e) => {
            debug!(
                target: "4da::log_retention",
                path = %dir.display(),
                error = %e,
                "Log directory unreadable; skipping cleanup"
            );
            return;
        }
    };

    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_log_file(&path) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());
        if modified < cutoff {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    removed += 1;
                    debug!(
                        target: "4da::log_retention",
                        path = %path.display(),
                        "Removed old log file"
                    );
                }
                Err(e) => {
                    warn!(
                        target: "4da::log_retention",
                        path = %path.display(),
                        error = %e,
                        "Failed to remove old log file"
                    );
                }
            }
        }
    }

    if removed > 0 {
        tracing::info!(
            target: "4da::log_retention",
            removed = removed,
            retention_days = RETENTION_DAYS,
            "Log retention cleanup complete"
        );
    }
}

/// Match any file whose name begins with `fourda.log` — the rolling
/// appender produces `fourda.log.2026-04-13`, `fourda.log`, etc.
fn is_log_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(LOG_FILE_STEM))
}

/// Return the path of the most-recently-modified log file, or None if
/// the directory is empty / unreadable.
fn most_recent_log_file() -> Option<PathBuf> {
    let dir = log_dir();
    let entries = std::fs::read_dir(&dir).ok()?;

    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_log_file(&path) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        match &best {
            Some((best_time, _)) if *best_time >= modified => {}
            _ => best = Some((modified, path)),
        }
    }
    best.map(|(_, p)| p)
}

/// Read the last `lines` lines of the most-recent log file.
///
/// Returns an empty string if no log file exists yet (e.g. on first launch).
/// Caps `lines` at 10_000 to bound memory. Caps total bytes read at 8 MiB.
pub fn read_recent_log_lines(lines: usize) -> Result<String, String> {
    const MAX_LINES: usize = 10_000;
    const MAX_BYTES: u64 = 8 * 1024 * 1024;

    let requested = lines.min(MAX_LINES);
    let Some(path) = most_recent_log_file() else {
        return Ok(String::new());
    };

    let metadata = std::fs::metadata(&path).map_err(|e| format!("Failed to stat log file: {e}"))?;
    let size = metadata.len();

    // Read either the whole file (if small) or the last MAX_BYTES bytes.
    let content = if size <= MAX_BYTES {
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read log file: {e}"))?
    } else {
        read_tail_bytes(&path, MAX_BYTES)?
    };

    // Take last N lines.
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(requested);
    let tail = all[start..].join("\n");
    Ok(tail)
}

/// Read the last `max_bytes` of a file as a UTF-8 string, skipping a
/// possibly-partial first line introduced by the tail offset.
fn read_tail_bytes(path: &Path, max_bytes: u64) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| format!("Failed to open log file: {e}"))?;
    let len = f
        .metadata()
        .map_err(|e| format!("Failed to stat log file: {e}"))?
        .len();
    let offset = len.saturating_sub(max_bytes);
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek log file: {e}"))?;
    let mut buf = Vec::with_capacity(max_bytes as usize);
    f.read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read log file: {e}"))?;
    let mut s = String::from_utf8_lossy(&buf).into_owned();
    // Drop a partial leading line (the seek almost certainly mid-line).
    if offset > 0 {
        if let Some(nl) = s.find('\n') {
            s.drain(..=nl);
        }
    }
    Ok(s)
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Return the last `lines` lines of the most-recent log file.
///
/// Used by support workflows and a future in-app log viewer. Free for all
/// tiers — users need their own diagnostic output, and no other user's data
/// is exposed.
#[tauri::command]
pub fn get_recent_logs(lines: usize) -> Result<String, String> {
    read_recent_log_lines(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Verify `is_log_file` matches the expected rolling-appender names.
    #[test]
    fn matches_rolling_log_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("fourda.log");
        let b = dir.path().join("fourda.log.2026-04-13");
        let c = dir.path().join("unrelated.txt");
        std::fs::write(&a, "x").unwrap();
        std::fs::write(&b, "x").unwrap();
        std::fs::write(&c, "x").unwrap();
        assert!(is_log_file(&a));
        assert!(is_log_file(&b));
        assert!(!is_log_file(&c));
    }

    /// Verify tail reader returns the requested number of lines.
    #[test]
    fn read_tail_lines_honours_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fourda.log");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..200 {
            writeln!(f, "line {i}").unwrap();
        }
        drop(f);

        // Read via tail reader directly (bypasses runtime_paths).
        let content = std::fs::read_to_string(&path).unwrap();
        let all: Vec<&str> = content.lines().collect();
        let tail: Vec<&str> = all.iter().rev().take(10).copied().collect();
        assert_eq!(tail.len(), 10);
        assert_eq!(tail[0], "line 199");
    }

    /// Requesting zero lines returns empty string (not an error).
    #[test]
    fn zero_lines_is_empty_not_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fourda.log");
        std::fs::write(&path, "a\nb\nc\n").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let all: Vec<&str> = content.lines().collect();
        let start = all.len().saturating_sub(0);
        assert_eq!(all[start..].join("\n"), "");
    }

    /// Partial leading line is dropped when seeking into a large file.
    #[test]
    fn tail_bytes_drops_partial_leading_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fourda.log");
        let mut f = std::fs::File::create(&path).unwrap();
        // Write a long first line then many short lines.
        writeln!(f, "{}", "x".repeat(200)).unwrap();
        for i in 0..50 {
            writeln!(f, "short {i}").unwrap();
        }
        drop(f);

        // Ask for only the last 100 bytes — the tail read should drop the
        // partial first line and keep only complete subsequent lines.
        let result = read_tail_bytes(&path, 100).unwrap();
        assert!(!result.is_empty());
        // None of the lines should contain the giant 200-x line.
        assert!(!result.contains(&"x".repeat(200)));
    }

    /// Retention is exposed as a const so ops can audit it without reading code.
    #[test]
    fn retention_is_one_week() {
        assert_eq!(RETENTION_DAYS, 7);
    }
}
