// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use super::{HealthIssue, HealthSeverity};
use std::path::Path;

// ============================================================================
// Linux checks
// ============================================================================

#[cfg(target_os = "linux")]
pub(super) fn check_cjk_fonts(issues: &mut Vec<HealthIssue>) {
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

#[cfg(target_os = "linux")]
pub(super) fn check_dbus_available(issues: &mut Vec<HealthIssue>) {
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

#[cfg(target_os = "linux")]
pub(super) fn check_display_server(issues: &mut Vec<HealthIssue>) {
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

// ============================================================================
// Windows checks
// ============================================================================

#[cfg(target_os = "windows")]
const MIN_WEBVIEW2_MAJOR: u32 = 120; // Chromium 120 = Jan 2024 — well below the evergreen floor as of 2026

#[cfg(target_os = "windows")]
pub(super) fn check_webview2_version(issues: &mut Vec<HealthIssue>) {
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
        tracing::info!(
            target: "4da::startup",
            webview2_version = %version,
            webview2_major = major,
            min_required = MIN_WEBVIEW2_MAJOR,
            "WebView2 runtime version is healthy"
        );
    }
}

#[cfg(target_os = "windows")]
fn read_registry_value(key: &str, value_name: &str) -> Option<String> {
    use std::process::Command;
    let mut cmd = Command::new("reg");
    cmd.args(["query", key, "/v", value_name]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(value_name) {
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

// ============================================================================
// macOS checks
// ============================================================================

#[cfg(target_os = "macos")]
pub(super) fn check_icloud_interference(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
    let data_str = data_dir.to_string_lossy();

    let icloud_indicators = [
        "Library/Mobile Documents",
        "CloudStorage",
        "iCloud Drive",
        ".icloud",
    ];

    let in_icloud = icloud_indicators
        .iter()
        .any(|pattern| data_str.contains(pattern));

    let has_icloud_marker = data_dir.join(".icloud").exists();

    if in_icloud || has_icloud_marker {
        issues.push(HealthIssue {
            component: "storage",
            severity: HealthSeverity::Error,
            message: "Data directory appears to be inside iCloud Drive. This can corrupt the database during sync. Move 4DA's data directory outside of iCloud-synced folders, or set FOURDA_DB_PATH to a non-synced location.".to_string(),
        });
    }
}

#[cfg(target_os = "macos")]
pub(super) fn check_macos_keychain(issues: &mut Vec<HealthIssue>) {
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

fn detect_onedrive_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();

    if lower.contains("\\onedrive\\")
        || lower.contains("\\onedrive -")
        || lower.contains("/onedrive/")
        || lower.contains("/onedrive -")
    {
        return true;
    }

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

fn detect_google_drive_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    lower.contains("\\google drive\\")
        || lower.contains("\\googledrive\\")
        || lower.contains("/google drive/")
        || lower.contains("/googledrive/")
        || lower.contains("/volumes/googledrive")
}

fn detect_dropbox_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    lower.contains("\\dropbox\\") || lower.contains("/dropbox/")
}

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

pub(super) fn check_cloud_sync_interference(data_dir: &Path, issues: &mut Vec<HealthIssue>) {
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
