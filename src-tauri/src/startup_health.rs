// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

    #[cfg(target_os = "windows")]
    {
        check_webview2_version(&mut issues);
    }

    #[cfg(target_os = "macos")]
    {
        check_icloud_interference(&data_dir, &mut issues);
        check_macos_keychain(&mut issues);
    }

    // Cross-platform cloud-sync detection (OneDrive/Google Drive/Dropbox).
    // iCloud is handled inside check_icloud_interference on macOS.
    check_cloud_sync_interference(&data_dir, &mut issues);

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
        // Key is empty in JSON — check the platform keychain (keys are migrated
        // there by SettingsManager and stripped from the on-disk JSON).
        let has_keychain_key = crate::settings::keystore::has_secret("llm_api_key")
            || match crate::settings::keystore::get_secret("llm_api_key") {
                Ok(Some(k)) => !k.is_empty(),
                _ => false,
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

/// Minimum WebView2 runtime version required by 4DA.
///
/// WebView2 ships its own update channel ("evergreen") via Edge, so most
/// Windows users are well above this floor. We pin a deliberately conservative
/// minimum that covers every web platform feature 4DA's frontend uses, plus
/// a 12-month safety margin against feature drift.
///
/// Update this constant when introducing a new web API that needs a newer
/// WebView2 build. Always raise — never lower.
#[cfg(target_os = "windows")]
const MIN_WEBVIEW2_MAJOR: u32 = 120; // Chromium 120 = Jan 2024 — well below the evergreen floor as of 2026

/// Windows-only: detect the installed WebView2 Runtime version and warn if
/// it's missing or older than the minimum 4DA supports.
///
/// Reads the version from the EdgeUpdate registry key (the canonical
/// Microsoft-documented location). Tries the four possible registry roots
/// in order: system-wide x86, system-wide x64, per-user x86, per-user x64.
///
/// This check is intentionally:
///   - Offline (registry only — no network)
///   - Fast (single `reg query` invocation, ~10ms)
///   - Soft-fail (warning, not error) — if WebView2 is missing entirely the
///     app would not have launched in the first place, so reaching this
///     code means *some* webview is present and the user can keep working
///     while we surface the upgrade hint
#[cfg(target_os = "windows")]
fn check_webview2_version(issues: &mut Vec<HealthIssue>) {
    // Stable WebView2 Runtime client GUID per Microsoft docs.
    const CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

    let candidates = [
        format!("HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT_GUID}"),
        format!("HKLM\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT_GUID}"),
        format!("HKCU\\Software\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT_GUID}"),
        format!("HKCU\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT_GUID}"),
    ];

    let mut found_version: Option<String> = None;
    for key in &candidates {
        if let Some(v) = read_registry_value(key, "pv") {
            if !v.is_empty() && v != "0.0.0.0" {
                found_version = Some(v);
                break;
            }
        }
    }

    let version = match found_version {
        Some(v) => v,
        None => {
            // No version found in any of the four canonical locations. The app
            // is running, so a webview is loaded somehow — but we can't verify
            // its version. Surface a soft warning so the user can investigate.
            issues.push(HealthIssue {
                component: "webview2",
                severity: HealthSeverity::Warning,
                message: "WebView2 Runtime version could not be determined from the registry. \
                          The app is running, but auto-update health may be unverifiable. \
                          Reinstall WebView2 from https://go.microsoft.com/fwlink/p/?LinkId=2124703 \
                          if you see UI rendering issues."
                    .to_string(),
            });
            return;
        }
    };

    // Parse the major version (first dotted segment). Version strings look like
    // "120.0.2210.144" or sometimes "120.0.0.0" — we only care about the major.
    let major = version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    if major == 0 {
        issues.push(HealthIssue {
            component: "webview2",
            severity: HealthSeverity::Warning,
            message: format!(
                "WebView2 Runtime reported an unparseable version '{version}'. \
                 The app is running but UI features may be limited. \
                 Update Microsoft Edge from Settings → About to refresh WebView2."
            ),
        });
        return;
    }

    if major < MIN_WEBVIEW2_MAJOR {
        issues.push(HealthIssue {
            component: "webview2",
            severity: HealthSeverity::Warning,
            message: format!(
                "WebView2 Runtime is version {version} (major {major}). \
                 4DA requires at least major {MIN_WEBVIEW2_MAJOR} for full UI support. \
                 Update Microsoft Edge — WebView2 evergreen updates ride along with Edge updates. \
                 Check Settings → Apps → Microsoft Edge WebView2 Runtime, then run Edge's \
                 'About' page to trigger an update. Some UI features may render incorrectly until updated."
            ),
        });
    } else {
        // Healthy — log only, no user-visible issue.
        tracing::info!(
            target: "4da::startup",
            webview2_version = %version,
            webview2_major = major,
            min_required = MIN_WEBVIEW2_MAJOR,
            "WebView2 runtime version is healthy"
        );
    }
}

/// Read a single REG_SZ value from the Windows registry by shelling out to
/// `reg query`. Returns `None` on any failure (missing key, missing value,
/// non-zero exit, parse error).
///
/// Why `reg.exe` instead of a registry crate: zero new dependencies, works on
/// every Windows build since XP, can never panic, fast enough for a one-shot
/// startup probe (~10ms). The output format is stable and parses cleanly.
#[cfg(target_os = "windows")]
fn read_registry_value(key: &str, value_name: &str) -> Option<String> {
    use std::process::Command;
    let output = Command::new("reg")
        .args(["query", key, "/v", value_name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output looks like:
    //     HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{...}
    //         pv    REG_SZ    120.0.2210.144
    // We split on whitespace and take the last token of the line that starts
    // with the value name.
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(value_name) {
            // Split on REG_SZ to get everything after the type marker.
            if let Some(after) = trimmed.split("REG_SZ").nth(1) {
                let v = after.trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
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

// ============================================================================
// Cross-platform cloud-sync detection (OneDrive, Google Drive, Dropbox)
// ============================================================================
//
// SQLite in WAL mode + any eventually-consistent cloud syncer = corruption.
// The syncer snapshots the main .db file while WAL still holds uncommitted
// pages, then uploads a torn copy. On restore the WAL no longer matches and
// the database is unrecoverable without a backup. Detection here is
// substring-based (paths, env vars) and intentionally conservative: we emit
// a Warning, not an Error, because some users deliberately place data in a
// synced folder and tolerate the risk. iCloud is handled separately because
// its path layout is macOS-specific and checked earlier.

/// Which cloud-sync provider a path appears to live inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloudSyncProvider {
    OneDrive,
    GoogleDrive,
    Dropbox,
}

impl CloudSyncProvider {
    fn display(self) -> &'static str {
        match self {
            Self::OneDrive => "OneDrive",
            Self::GoogleDrive => "Google Drive",
            Self::Dropbox => "Dropbox",
        }
    }
}

/// Return true if `path` appears to live inside a OneDrive-synced folder.
///
/// Detection order:
/// 1. Path substring match for `\OneDrive\` or `\OneDrive -` (enterprise tenants
///    expose folders like `OneDrive - Contoso`). Match is case-insensitive.
/// 2. Environment variable `OneDrive` / `OneDriveCommercial` / `OneDriveConsumer`,
///    if the data path starts with any of those (case-insensitive on Windows).
fn detect_onedrive_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();

    // Path substring check works on both Windows (\\OneDrive\\) and any OS where
    // a user has symlinked/mounted a OneDrive folder at a known name.
    if lower.contains("\\onedrive\\")
        || lower.contains("\\onedrive -")
        || lower.contains("/onedrive/")
        || lower.contains("/onedrive -")
    {
        return true;
    }

    // Environment variable check — OneDrive sets these on Windows. They point
    // to the actual sync root, which may be on a non-default drive.
    for var in ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"] {
        if let Ok(root) = std::env::var(var) {
            if root.is_empty() {
                continue;
            }
            let root_lower = root.to_lowercase();
            if !root_lower.is_empty() && lower.starts_with(&root_lower) {
                return true;
            }
        }
    }

    false
}

/// Return true if `path` appears to live inside a Google Drive synced folder.
fn detect_google_drive_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    lower.contains("\\google drive\\")
        || lower.contains("\\googledrive\\")
        || lower.contains("/google drive/")
        || lower.contains("/googledrive/")
        // Google Drive for Desktop (macOS) mounts under /Volumes/GoogleDrive
        || lower.contains("/volumes/googledrive")
}

/// Return true if `path` appears to live inside a Dropbox synced folder.
fn detect_dropbox_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    lower.contains("\\dropbox\\") || lower.contains("/dropbox/")
}

/// Detect which (if any) cloud-sync provider the data directory sits inside.
/// Returns `None` when the path looks safely local.
fn detect_cloud_sync(data_dir: &Path) -> Option<CloudSyncProvider> {
    if detect_onedrive_path(data_dir) {
        Some(CloudSyncProvider::OneDrive)
    } else if detect_google_drive_path(data_dir) {
        Some(CloudSyncProvider::GoogleDrive)
    } else if detect_dropbox_path(data_dir) {
        Some(CloudSyncProvider::Dropbox)
    } else {
        None
    }
}

/// Push a health issue if the data directory is inside a known cloud-sync root.
///
/// Emitted as a Warning (not Error) because some users intentionally place
/// data there. The message gives them the remediation (move the folder, or
/// set `FOURDA_DB_PATH`).
fn check_cloud_sync_interference(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    let Some(provider) = detect_cloud_sync(data_dir) else {
        return;
    };
    let name = provider.display();
    issues.push(HealthIssue {
        component: "storage",
        severity: HealthSeverity::Warning,
        message: format!(
            "Data directory appears to be inside {name}. Cloud-sync services can corrupt \
             SQLite databases that use WAL mode by uploading torn snapshots. Move 4DA's \
             data directory outside the {name} sync root, or set the FOURDA_DB_PATH \
             environment variable to a local-only folder."
        ),
    });
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod cloud_sync_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_windows_onedrive_personal_path() {
        let p = PathBuf::from("C:\\Users\\alice\\OneDrive\\4DA\\data");
        assert!(detect_onedrive_path(&p));
        assert_eq!(detect_cloud_sync(&p), Some(CloudSyncProvider::OneDrive));
    }

    #[test]
    fn detects_windows_onedrive_enterprise_path() {
        let p = PathBuf::from("C:\\Users\\alice\\OneDrive - Contoso\\4DA\\data");
        assert!(detect_onedrive_path(&p));
        assert_eq!(detect_cloud_sync(&p), Some(CloudSyncProvider::OneDrive));
    }

    #[test]
    fn detects_google_drive_path() {
        let windows = PathBuf::from("C:\\Users\\alice\\Google Drive\\4DA");
        let mac_mount = PathBuf::from("/Volumes/GoogleDrive/My Drive/4DA");
        assert!(detect_google_drive_path(&windows));
        assert!(detect_google_drive_path(&mac_mount));
        assert_eq!(
            detect_cloud_sync(&windows),
            Some(CloudSyncProvider::GoogleDrive)
        );
    }

    #[test]
    fn detects_dropbox_path() {
        let p = PathBuf::from("C:\\Users\\alice\\Dropbox\\4DA");
        assert!(detect_dropbox_path(&p));
        assert_eq!(detect_cloud_sync(&p), Some(CloudSyncProvider::Dropbox));
    }

    #[test]
    fn safe_local_path_is_not_flagged() {
        let p = PathBuf::from("C:\\Users\\alice\\AppData\\Local\\4DA");
        assert!(!detect_onedrive_path(&p));
        assert!(!detect_google_drive_path(&p));
        assert!(!detect_dropbox_path(&p));
        assert_eq!(detect_cloud_sync(&p), None);

        let unix = PathBuf::from("/home/alice/.local/share/4DA");
        assert_eq!(detect_cloud_sync(&unix), None);
    }
}
