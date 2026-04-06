// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Startup health self-check for 4DA.
//!
//! Validates the app is in a good state when it launches.
//! All checks are fast (< 100ms total), offline, and never panic.

use serde::Serialize;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

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

    #[cfg(target_os = "linux")]
    {
        check_cjk_fonts(&mut issues);
        check_dbus_available(&mut issues);
        check_display_server(&mut issues);
    }

    #[cfg(target_os = "macos")]
    {
        check_icloud_interference(&data_dir, &mut issues);
        check_macos_keychain(&mut issues);
    }

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

/// Resolve the data directory (same logic as state.rs get_db_path).
fn get_data_dir() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base
}

/// Check 1: Database file exists and is readable.
pub(crate) fn check_database(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
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
pub(crate) fn check_embedding_provider(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
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

    // Ollama doesn't need an API key.
    if provider == "ollama" {
        return;
    }

    let api_key = parsed
        .get("llm")
        .and_then(|llm| llm.get("api_key"))
        .and_then(|k| k.as_str())
        .unwrap_or("");

    if api_key.is_empty() {
        issues.push(HealthIssue {
            component: "embedding",
            severity: HealthSeverity::Warning,
            message: format!(
                "LLM provider '{provider}' is configured but API key is empty. LLM features will not work."
            ),
        });
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
    if let Ok(meta) = std::fs::metadata(&db_path) {
        let size_mb = meta.len() / (1024 * 1024);
        if size_mb > 500 {
            issues.push(HealthIssue {
                component: "database",
                severity: HealthSeverity::Warning,
                message: format!(
                    "Database is {}MB. Consider running database optimization in Settings to reclaim space.",
                    size_mb
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

/// Check if CJK fonts are available (affects Chinese, Japanese, Korean users).
#[cfg(target_os = "linux")]
fn check_cjk_fonts(issues: &mut Vec<HealthIssue>) {
    // Check for common CJK font paths
    let cjk_font_dirs = [
        "/usr/share/fonts/opentype/noto",
        "/usr/share/fonts/google-noto-cjk",
        "/usr/share/fonts/noto-cjk",
        "/usr/share/fonts/truetype/noto",
    ];
    let has_cjk = cjk_font_dirs
        .iter()
        .any(|d| std::path::Path::new(d).exists());

    // Also check via fc-list if available
    let has_cjk = has_cjk
        || std::process::Command::new("fc-list")
            .args([":lang=ja"])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

    if !has_cjk {
        issues.push(HealthIssue {
            component: "fonts",
            severity: HealthSeverity::Warning,
            message: "CJK fonts not detected. Chinese, Japanese, and Korean text may not display correctly. Install noto-fonts-cjk (Arch), fonts-noto-cjk (Debian), or google-noto-sans-cjk-fonts (Fedora).".to_string(),
        });
    }
}

/// Check if D-Bus is available (required for notifications and keyring).
#[cfg(target_os = "linux")]
fn check_dbus_available(issues: &mut Vec<HealthIssue>) {
    let dbus_running = std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok()
        || std::path::Path::new("/run/dbus/system_bus_socket").exists();

    if !dbus_running {
        issues.push(HealthIssue {
            component: "dbus",
            severity: HealthSeverity::Warning,
            message: "D-Bus session bus not detected. Notifications, system tray, and secure credential storage may not work. This is expected in containers and WSL1.".to_string(),
        });
    }
}

/// Check display server type and warn about known issues.
#[cfg(target_os = "linux")]
fn check_display_server(issues: &mut Vec<HealthIssue>) {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();

    // GNOME + Wayland: tray won't work
    if session_type == "wayland" && desktop.contains("GNOME") {
        issues.push(HealthIssue {
            component: "display",
            severity: HealthSeverity::Warning,
            message: "GNOME on Wayland detected. System tray is not available natively. Install the AppIndicator extension for tray support, or use the main window instead.".to_string(),
        });
    }

    // Tiling WM users might need floating window hints
    let tiling_wms = ["I3", "SWAY", "HYPRLAND", "BSPWM", "DWM"];
    if tiling_wms.iter().any(|wm| desktop.contains(wm)) {
        issues.push(HealthIssue {
            component: "display",
            severity: HealthSeverity::Warning,
            message: format!(
                "{} detected. You may need to add a floating window rule for 4DA. Notification popups may appear tiled instead of floating.",
                desktop.split(':').next().unwrap_or(&desktop)
            ),
        });
    }
}

/// Check if the data directory is inside an iCloud Drive synced folder.
/// iCloud sync + SQLite WAL = database corruption risk.
#[cfg(target_os = "macos")]
fn check_icloud_interference(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    let data_str = data_dir.to_string_lossy();

    // Check common iCloud sync paths
    let icloud_indicators = [
        "Library/Mobile Documents",
        "CloudStorage",
        "iCloud Drive",
        ".icloud",
    ];

    let in_icloud = icloud_indicators
        .iter()
        .any(|pattern| data_str.contains(pattern));

    // Also check if the data directory has the .icloud file (synced marker)
    let has_icloud_marker = data_dir.join(".icloud").exists();

    if in_icloud || has_icloud_marker {
        issues.push(HealthIssue {
            component: "storage",
            severity: HealthSeverity::Error,
            message: "Data directory appears to be inside iCloud Drive. This can corrupt the database during sync. Move 4DA's data directory outside of iCloud-synced folders, or set FOURDA_DB_PATH to a non-synced location.".to_string(),
        });
    }
}

/// Check if macOS Keychain is accessible (affects API key storage).
#[cfg(target_os = "macos")]
fn check_macos_keychain(issues: &mut Vec<HealthIssue>) {
    // Try to detect if the keychain daemon is running
    // On macOS, securityd should always be running, but in some CI/container environments it may not be
    let securityd_running = std::process::Command::new("pgrep")
        .args(["-x", "securityd"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !securityd_running {
        issues.push(HealthIssue {
            component: "keychain",
            severity: HealthSeverity::Warning,
            message: "macOS Keychain service (securityd) not detected. API keys will be stored in plaintext. This is expected in CI/container environments.".to_string(),
        });
    }
}

// ============================================================================
// Diagnostic Report (for support)
// ============================================================================

/// Generate a comprehensive diagnostic report for troubleshooting.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DiagnosticReport {
    pub app_version: &'static str,
    pub platform: &'static str,
    pub arch: &'static str,
    pub data_dir: String,
    pub db_size_bytes: u64,
    pub settings_exists: bool,
    pub disk_available_mb: u64,
    pub health_issues: Vec<HealthIssue>,
}

pub(crate) fn generate_diagnostic_report() -> DiagnosticReport {
    let data_dir = get_data_dir();
    let issues = run_startup_health_check();
    let db_size = std::fs::metadata(data_dir.join("4da.db"))
        .map(|m| m.len())
        .unwrap_or(0);
    let disk_available = get_available_disk_space(&data_dir);

    DiagnosticReport {
        app_version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        data_dir: data_dir.display().to_string(),
        db_size_bytes: db_size,
        settings_exists: data_dir.join("settings.json").exists(),
        disk_available_mb: disk_available / (1024 * 1024),
        health_issues: issues,
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns startup health issues for the frontend to optionally display.
#[tauri::command]
pub(crate) fn get_startup_health() -> Vec<HealthIssue> {
    // Re-run checks so frontend gets a fresh snapshot.
    run_startup_health_check()
}

/// Returns a full diagnostic report for support/troubleshooting.
#[tauri::command]
pub(crate) fn get_diagnostic_report() -> DiagnosticReport {
    generate_diagnostic_report()
}
