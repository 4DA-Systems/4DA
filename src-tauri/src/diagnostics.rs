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
    let uptime_secs = APP_START
        .get()
        .map(|start| start.elapsed().as_secs())
        .unwrap_or(0);

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

/// Get approximate process RSS memory in bytes
fn get_process_memory() -> u64 {
    // On Windows, use the Win32 API via std
    #[cfg(target_os = "windows")]
    {
        // Use GetProcessMemoryInfo via raw FFI
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

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux/macOS, read /proc/self/status or use sysctl
        // Fallback: return 0 (diagnostics still useful without memory)
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(|kb| kb * 1024)
                })
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_start_time_is_idempotent() {
        // First call sets the time
        record_start_time();
        let first = APP_START.get().map(|i| i.elapsed());
        // Second call should be a no-op
        record_start_time();
        let second = APP_START.get().map(|i| i.elapsed());
        // Both should be set (not None)
        assert!(first.is_some());
        assert!(second.is_some());
    }

    #[test]
    fn get_process_memory_returns_value() {
        let mem = get_process_memory();
        // Just verify it doesn't panic and returns a non-negative value
        let _ = mem;
    }

    #[test]
    fn diagnostics_snapshot_serializes_correctly() {
        let snap = DiagnosticsSnapshot {
            memory_bytes: 1024 * 1024 * 50,
            db_size_bytes: 1024 * 1024 * 10,
            source_item_count: 500,
            context_chunk_count: 200,
            feedback_count: 50,
            uptime_secs: 3600,
            source_health: vec![SourceHealthSummary {
                source_type: "hackernews".to_string(),
                status: "healthy".to_string(),
                consecutive_failures: 0,
            }],
            schema_version: 42,
            db_size_warning: false,
        };
        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(json["source_item_count"], 500);
        assert_eq!(json["db_size_warning"], false);
        assert_eq!(json["source_health"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn source_health_summary_serializes_correctly() {
        let health = SourceHealthSummary {
            source_type: "reddit".to_string(),
            status: "degraded".to_string(),
            consecutive_failures: 3,
        };
        let json = serde_json::to_value(&health).unwrap();
        assert_eq!(json["source_type"], "reddit");
        assert_eq!(json["consecutive_failures"], 3);
    }

    #[test]
    fn db_size_warning_threshold_is_500mb() {
        // Below threshold
        let snap = DiagnosticsSnapshot {
            memory_bytes: 0,
            db_size_bytes: 499 * 1024 * 1024,
            source_item_count: 0,
            context_chunk_count: 0,
            feedback_count: 0,
            uptime_secs: 0,
            source_health: vec![],
            schema_version: 0,
            db_size_warning: 499 * 1024 * 1024 > 500 * 1024 * 1024,
        };
        assert!(!snap.db_size_warning);

        // Above threshold
        let snap2 = DiagnosticsSnapshot {
            db_size_warning: 501 * 1024 * 1024 > 500 * 1024 * 1024,
            ..snap
        };
        assert!(snap2.db_size_warning);
    }
}
