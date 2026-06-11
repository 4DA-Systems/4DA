// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Startup health self-check for 4DA.
//!
//! Validates the app is in a good state when it launches.
//! All checks are fast (< 100ms total), offline, and never panic.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::{info, warn};

static STARTUP_HEALTH_CACHE: OnceLock<Vec<HealthIssue>> = OnceLock::new();

#[path = "startup_health_platform.rs"]
mod startup_health_platform;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HealthIssue {
    pub component: &'static str,
    pub severity: HealthSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum HealthSeverity {
    Warning,
    Error,
}

// ============================================================================
// Public API
// ============================================================================

/// Run all startup health checks and return any issues found.
///
/// Designed to be fast (< 100ms), offline (no network calls), and infallible.
pub(crate) fn run_startup_health_check() -> Vec<HealthIssue> {
    let mut issues = Vec::new();
    let data_dir = get_data_dir();

    check_database(&data_dir, &mut issues);
    check_settings(&data_dir, &mut issues);
    check_embedding_provider(&data_dir, &mut issues);
    check_source_adapters(&mut issues);
    check_disk_write(&data_dir, &mut issues);
    check_disk_space(&data_dir, &mut issues);
    check_database_size(&data_dir, &mut issues);
    check_embedding_coverage(&mut issues);

    #[cfg(target_os = "linux")]
    {
        startup_health_platform::check_cjk_fonts(&mut issues);
        startup_health_platform::check_dbus_available(&mut issues);
        startup_health_platform::check_display_server(&mut issues);
    }

    #[cfg(target_os = "windows")]
    {
        startup_health_platform::check_webview2_version(&mut issues);
    }

    #[cfg(target_os = "macos")]
    {
        startup_health_platform::check_icloud_interference(&data_dir, &mut issues);
        startup_health_platform::check_macos_keychain(&mut issues);
    }

    // Cross-platform keychain probe — log-only, not user-facing.
    check_keychain_functional();

    // Cross-platform cloud-sync detection (OneDrive/Google Drive/Dropbox).
    // iCloud is handled inside check_icloud_interference on macOS.
    startup_health_platform::check_cloud_sync_interference(&data_dir, &mut issues);

    // Log results
    if issues.is_empty() {
        info!(target: "4da::startup", "Health check passed: all systems nominal");
    } else {
        let errors = issues
            .iter()
            .filter(|i| i.severity == HealthSeverity::Error)
            .count();
        let warnings = issues
            .iter()
            .filter(|i| i.severity == HealthSeverity::Warning)
            .count();
        for issue in &issues {
            match issue.severity {
                HealthSeverity::Error => {
                    warn!(target: "4da::startup", component = issue.component, "HEALTH ERROR: {}", issue.message);
                }
                HealthSeverity::Warning => {
                    warn!(target: "4da::startup", component = issue.component, "HEALTH WARNING: {}", issue.message);
                }
            }
        }
        info!(target: "4da::startup", errors, warnings, "Health check complete with issues");
    }

    issues
}

// ============================================================================
// Individual Checks
// ============================================================================

/// Resolve the data directory via centralized RuntimePaths.
fn get_data_dir() -> PathBuf {
    crate::runtime_paths::RuntimePaths::get().data_dir.clone()
}

/// Check 1: Database file exists and is readable.
pub(crate) fn check_database(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    // First, surface any cold-boot recovery notice. `state.rs::get_database`
    // calls `recover_corrupt_db_if_needed` and stashes the result so we can
    // tell the user about it on the next health-check poll. The notice is
    // consumed (one-shot) so the banner shows exactly once per cold boot.
    if let Some(notice) = crate::db::migrations::take_db_recovery_notice() {
        match notice {
            crate::db::migrations::CorruptionRecovery::Healthy
            | crate::db::migrations::CorruptionRecovery::NoExistingDb => {
                // Healthy paths produce no banner.
            }
            crate::db::migrations::CorruptionRecovery::RestoredFromBackup { restored_from } => {
                issues.push(HealthIssue {
                    component: "database",
                    severity: HealthSeverity::Warning,
                    message: format!(
                        "Database was corrupt and was restored from a backup ({}). \
                         Some recent changes may be missing. \
                         Your previous database is preserved alongside the backup file for support.",
                        restored_from.display()
                    ),
                });
            }
            crate::db::migrations::CorruptionRecovery::QuarantinedNoBackup { quarantined_to } => {
                issues.push(HealthIssue {
                    component: "database",
                    severity: HealthSeverity::Error,
                    message: format!(
                        "Database was corrupt and no backup was available — a fresh database has been created. \
                         The corrupted file is preserved at {} so you can attach it to a support request.",
                        quarantined_to.display()
                    ),
                });
            }
            crate::db::migrations::CorruptionRecovery::RecoveryFailed { reason } => {
                issues.push(HealthIssue {
                    component: "database",
                    severity: HealthSeverity::Error,
                    message: format!(
                        "Database integrity check failed and preemptive recovery could not run: {reason}. \
                         The app may have fallen back to a fresh database. \
                         Check the data directory for *.db.corrupt files and contact support."
                    ),
                });
            }
        }
    }

    let db_path = data_dir.join("4da.db");
    if !db_path.exists() {
        // Not an error on first run — the DB will be created.
        issues.push(HealthIssue {
            component: "database",
            severity: HealthSeverity::Warning,
            message: format!(
                "Database not found at {}. Will be created on first use.",
                db_path.display()
            ),
        });
        return;
    }
    match std::fs::metadata(&db_path) {
        Ok(meta) => {
            if meta.len() == 0 {
                issues.push(HealthIssue {
                    component: "database",
                    severity: HealthSeverity::Warning,
                    message: "Database file exists but is empty".to_string(),
                });
            }
        }
        Err(e) => {
            issues.push(HealthIssue {
                component: "database",
                severity: HealthSeverity::Error,
                message: format!("Cannot read database file: {e}"),
            });
        }
    }
}

/// Check 2: Settings file parses without error.
pub(crate) fn check_settings(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    let settings_path = data_dir.join("settings.json");
    if !settings_path.exists() {
        // Not an error — first run uses defaults.
        issues.push(HealthIssue {
            component: "settings",
            severity: HealthSeverity::Warning,
            message: "No settings.json found. Using defaults (first run).".to_string(),
        });
        return;
    }
    match std::fs::read_to_string(&settings_path) {
        Ok(content) => {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
                issues.push(HealthIssue {
                    component: "settings",
                    severity: HealthSeverity::Error,
                    message: format!("settings.json is invalid JSON: {e}"),
                });
            }
        }
        Err(e) => {
            issues.push(HealthIssue {
                component: "settings",
                severity: HealthSeverity::Error,
                message: format!("Cannot read settings.json: {e}"),
            });
        }
    }
}

/// Check 3: If an LLM provider is configured, verify the API key is non-empty.
/// No network calls — just validates the config looks plausible.
/// Checks both the JSON file and the platform keychain (keys may have been
/// migrated from plaintext to keychain by SettingsManager).
pub(crate) fn check_embedding_provider(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    check_embedding_provider_inner(data_dir, issues, true)
}

/// Inner implementation with `check_keychain` toggle so tests can bypass the
/// real platform keychain (which may contain a live key on dev machines).
pub(crate) fn check_embedding_provider_inner(
    data_dir: &Path,
    issues: &mut Vec<HealthIssue>,
    check_keychain: bool,
) {
    let settings_path = data_dir.join("settings.json");
    if !settings_path.exists() {
        return; // No settings means no provider configured — that's fine.
    }
    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return, // Already reported in check_settings.
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return, // Already reported in check_settings.
    };

    let provider = parsed
        .get("llm")
        .and_then(|llm| llm.get("provider"))
        .and_then(|p| p.as_str())
        .unwrap_or("none");

    if provider == "none" || provider.is_empty() {
        // No provider configured — that's a valid state (embedding uses Ollama fallback).
        return;
    }

    // Ollama and local don't need an API key.
    if provider == "ollama" || provider == "local" {
        return;
    }

    let api_key = parsed
        .get("llm")
        .and_then(|llm| llm.get("api_key"))
        .and_then(|k| k.as_str())
        .unwrap_or("");

    if api_key.is_empty() {
        // Key is empty in JSON — check the platform keychain (keys are migrated
        // there by SettingsManager and stripped from the on-disk JSON).
        let has_keychain_key = if check_keychain {
            crate::settings::keystore::has_secret("llm_api_key")
                || match crate::settings::keystore::get_secret("llm_api_key") {
                    Ok(Some(k)) => !k.is_empty(),
                    _ => false,
                }
        } else {
            false
        };

        if !has_keychain_key {
            issues.push(HealthIssue {
                component: "embedding",
                severity: HealthSeverity::Warning,
                message: format!(
                    "LLM provider '{provider}' is configured but API key is empty. LLM features will not work."
                ),
            });
        }
    }
}

/// Check 4: At least one content source is configured/enabled.
fn check_source_adapters(issues: &mut Vec<HealthIssue>) {
    let registry = crate::get_source_registry();
    let count = {
        let reg = registry.lock();
        reg.count()
    };
    if count == 0 {
        issues.push(HealthIssue {
            component: "sources",
            severity: HealthSeverity::Error,
            message: "No content sources registered. The app will have no content to display."
                .to_string(),
        });
    }
}

/// Check 5: Data directory is writable (create + delete a temp file).
pub(crate) fn check_disk_write(data_dir: &PathBuf, issues: &mut Vec<HealthIssue>) {
    // Ensure the data directory exists first.
    if !data_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(data_dir) {
            issues.push(HealthIssue {
                component: "disk",
                severity: HealthSeverity::Error,
                message: format!("Cannot create data directory {}: {e}", data_dir.display()),
            });
            return;
        }
    }

    let probe = data_dir.join(".4da_health_probe");
    match std::fs::write(&probe, b"health") {
        Ok(()) => {
            // Clean up — failure here is not critical.
            let _ = std::fs::remove_file(&probe);
        }
        Err(e) => {
            issues.push(HealthIssue {
                component: "disk",
                severity: HealthSeverity::Error,
                message: format!("Data directory is not writable: {e}"),
            });
        }
    }
}

/// Check 6: Disk space — warn if less than 500MB, error if less than 100MB.
fn check_disk_space(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    let available = get_available_disk_space(data_dir);
    if available == 0 {
        return; // Could not determine — skip silently
    }
    let available_mb = available / (1024 * 1024);
    if available_mb < 100 {
        issues.push(HealthIssue {
            component: "disk",
            severity: HealthSeverity::Error,
            message: format!(
                "Critically low disk space: only {}MB available. 4DA needs space for its database and cache.",
                available_mb
            ),
        });
    } else if available_mb < 500 {
        issues.push(HealthIssue {
            component: "disk",
            severity: HealthSeverity::Warning,
            message: format!(
                "Low disk space: {}MB available. Consider freeing space to prevent issues.",
                available_mb
            ),
        });
    }
}

/// Get available disk space in bytes for the volume containing the given path.
/// Uses platform-specific APIs (Win32 on Windows, statvfs on Unix).
#[allow(unsafe_code)]
fn get_available_disk_space(path: &Path) -> u64 {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut free_bytes: u64 = 0;
        // SAFETY: Calling Win32 GetDiskFreeSpaceExW with valid aligned pointers
        let result = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_bytes as *mut u64,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result != 0 {
            free_bytes
        } else {
            0
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Use statvfs on Unix
        use std::ffi::CString;
        let path_cstr = match CString::new(path.to_string_lossy().as_bytes()) {
            Ok(c) => c,
            Err(_) => return 0,
        };
        // SAFETY: Calling POSIX statvfs with a valid null-terminated C string
        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(path_cstr.as_ptr(), &mut stat) == 0 {
                stat.f_bavail as u64 * stat.f_frsize as u64
            } else {
                0
            }
        }
    }
}

#[cfg(target_os = "windows")]
extern "system" {
    fn GetDiskFreeSpaceExW(
        lp_directory_name: *const u16,
        lp_free_bytes_available_to_caller: *mut u64,
        lp_total_number_of_bytes: *mut u64,
        lp_total_number_of_free_bytes: *mut u64,
    ) -> i32;
}

/// Check 7: Database size — warn if the database is getting large.
fn check_database_size(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    let db_path = data_dir.join("4da.db");
    let size_mb = std::fs::metadata(&db_path)
        .map(|m| m.len() / (1024 * 1024))
        .unwrap_or(0);

    // Only warn when there is genuinely RECLAIMABLE space: dead pages on the freelist
    // that VACUUM can return to the OS. A large database that is all live content is
    // healthy — 4DA's whole job is to read and embed the internet, so the file grows
    // with the corpus. Nagging about raw size is a false alarm, and telling the user to
    // "reclaim space" when the freelist is empty is simply wrong (VACUUM frees nothing).
    if let Ok(conn) = crate::open_db_connection() {
        let freelist: i64 = conn
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))
            .unwrap_or(0);
        let page_size: i64 = conn
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .unwrap_or(4096);
        let reclaimable_mb = freelist.saturating_mul(page_size) / (1024 * 1024);
        if reclaimable_mb >= 100 {
            issues.push(HealthIssue {
                component: "database",
                severity: HealthSeverity::Warning,
                message: format!(
                    "Database has {reclaimable_mb}MB of reclaimable free space ({size_mb}MB total). Run database optimization in Settings to compact it."
                ),
            });
        }
    }
    // WAL checkpoint: run immediately if large, don't bother the user about it.
    // Users should never see infrastructure maintenance warnings.
    let wal_path = data_dir.join("4da.db-wal");
    if let Ok(meta) = std::fs::metadata(&wal_path) {
        let size_mb = meta.len() / (1024 * 1024);
        if size_mb > 100 {
            tracing::info!(
                target: "4da::health",
                size_mb,
                "Large WAL file detected — running immediate checkpoint"
            );
            // TRUNCATE checkpoint — PASSIVE can't shrink a large WAL while readers are active
            if let Ok(conn) = crate::open_db_connection() {
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            }
        }
    }
}
/// Check 8: Zero-embedding coverage - semantic matching silently degrades when
/// the embedder is unavailable.
///
/// When no embedding provider is reachable (no Ollama, fastembed init failed,
/// no API key), items are stored with zero-vector embeddings and KNN context
/// scoring collapses - roughly half the scoring signal disappears while the
/// app looks perfectly healthy. Surface it honestly: if a meaningful share of
/// RECENTLY embedded items are zero vectors, tell the user and point at the
/// fix. Mirrors the database-size check's pattern (cheap SQL, warn-only,
/// never panics).
fn check_embedding_coverage(issues: &mut Vec<HealthIssue>) {
    if let Ok(conn) = crate::open_db_connection() {
        check_embedding_coverage_with_conn(&conn, issues);
    }
}

/// Inner implementation taking an explicit connection so tests can run
/// against an in-memory database.
///
/// Sampling: the most recent 300 items (PK-descending - index-backed, no full
/// table scan) filtered to the last 24 hours. Fires only when the sample is
/// meaningful (>= 20 items) AND more than half are zero vectors, so first-run
/// and low-traffic installs never see a false alarm.
pub(crate) fn check_embedding_coverage_with_conn(
    conn: &rusqlite::Connection,
    issues: &mut Vec<HealthIssue>,
) {
    const MIN_SAMPLE: i64 = 20;
    let row = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN embedding = zeroblob(length(embedding)) THEN 1 ELSE 0 END), 0)
         FROM (
             SELECT embedding, created_at FROM source_items
             WHERE embedding IS NOT NULL AND length(embedding) > 0
             ORDER BY id DESC LIMIT 300
         )
         WHERE created_at > datetime('now', '-24 hours')",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    );
    let (total, zeros) = match row {
        Ok(v) => v,
        Err(_) => return, // Table missing or unreadable - other checks report that.
    };
    if total >= MIN_SAMPLE && zeros * 2 > total {
        let pct = zeros.saturating_mul(100) / total.max(1);
        issues.push(HealthIssue {
            component: "embedding",
            severity: HealthSeverity::Warning,
            message: format!(
                "Semantic matching is degraded: {pct}% of items fetched in the last 24 hours \
                 could not be embedded because no local embedding model was available. \
                 Scoring is falling back to keyword matching. Start Ollama or check the \
                 AI provider in Settings to restore full semantic scoring."
            ),
        });
    }
}

// ============================================================================
// Cross-platform keychain functional probe
// ============================================================================

/// Write-read-delete probe on the platform keychain.
///
/// Catches the scenario where the `keyring` crate reports write success but
/// the credential silently drops (observed on some Windows machines). When
/// the probe fails, API keys fall back to plaintext in settings.json — which
/// is standard for desktop apps (VS Code, Chrome, etc. all do this). The
/// result is logged for diagnostics but never shown to the user — plaintext
/// local storage on a single-user machine is not a degraded security posture.
fn check_keychain_functional() {
    let probe_key = "4da_health_probe";
    let probe_val = "probe-ok";

    let stored = crate::settings::keystore::store_secret(probe_key, probe_val);
    let functional = match stored {
        Ok(true) => crate::settings::keystore::verify_round_trip(probe_key, probe_val),
        _ => false,
    };
    let _ = crate::settings::keystore::delete_secret(probe_key);

    if functional {
        info!(
            target: "4da::startup",
            "Keychain probe OK — credentials stored in OS credential manager"
        );
    } else {
        info!(
            target: "4da::startup",
            "Keychain unavailable — credentials stored locally in settings.json (standard for desktop apps)"
        );
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns startup health issues for the frontend to optionally display.
///
/// Cached per process lifetime — the disk probe, keychain round-trip, and
/// platform checks only run once. HMR reloads in dev mode can call this
/// dozens of times; without the cache each call writes a probe file to
/// `data/`, which can trigger Vite's file watcher and create an infinite
/// reload loop.
#[tauri::command]
pub(crate) fn get_startup_health() -> Vec<HealthIssue> {
    STARTUP_HEALTH_CACHE
        .get_or_init(|| {
            let mut issues = run_startup_health_check();

            // Filter out false-positive "API key is empty" when the in-memory settings
            // (hydrated from keychain on startup) DO have the key. The disk-based check
            // reads settings.json directly, which has keys stripped after keychain
            // migration. Use lock() instead of try_lock() — this is a non-hot path
            // called once on mount, and try_lock() was silently failing during startup
            // when another thread held the mutex, letting the false-positive through.
            {
                let mut guard = crate::get_settings_manager().lock();
                guard.ensure_keys_hydrated();
                let has_key = !guard.get().llm.api_key.is_empty();
                if has_key {
                    issues.retain(|i| {
                        !(i.component == "embedding" && i.message.contains("API key is empty"))
                    });
                }
            }

            issues
        })
        .clone()
}
