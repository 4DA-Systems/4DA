// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Background-refresh OS task management.
//!
//! Installs (or removes) an operating-system scheduled task that runs the shipped binary in headless
//! mode (`fourda --engine-once`) on an interval, so the SQLite database stays fresh for the MCP
//! server even when the GUI is never opened. The scheduled task itself is the source of truth for
//! "is background refresh enabled" — there is no mirrored setting to drift out of sync.
//!
//! Windows is implemented via `schtasks`. macOS (launchd) and Linux (systemd-user timers) return an
//! honest "not yet supported" error rather than pretending to succeed — the doctrine is never to
//! claim an action the system can't stand behind.

use std::path::Path;

use serde::Serialize;

/// The scheduled-task name, shared by install/uninstall/status so they refer to the same task.
pub(crate) const TASK_NAME: &str = "4DA Background Refresh";

/// Default refresh cadence when the caller does not specify one.
pub(crate) const DEFAULT_INTERVAL_MINUTES: u64 = 30;

/// Reported state of the background-refresh task. Serialized to the frontend by the Tauri commands.
#[derive(Debug, Clone, Serialize)]
pub struct SchedulerStatus {
    /// Whether the scheduled task currently exists.
    pub installed: bool,
    /// Whether this platform supports background-refresh scheduling at all.
    pub supported: bool,
    /// `windows` | `macos` | `linux` | other.
    pub platform: String,
    /// The scheduled-task name.
    pub task_name: String,
    /// Configured interval in minutes, if it could be determined.
    pub interval_minutes: Option<u64>,
    /// Human-readable detail (the task command, or why it's unsupported).
    pub detail: String,
}

impl SchedulerStatus {
    /// Used only on platforms without a scheduler implementation (macOS/Linux); on Windows the
    /// `schtasks` path never constructs an "unsupported" status, so cfg it out there to stay warning-clean.
    #[cfg(not(target_os = "windows"))]
    fn unsupported(platform: &str) -> Self {
        Self {
            installed: false,
            supported: false,
            platform: platform.to_string(),
            task_name: TASK_NAME.to_string(),
            interval_minutes: None,
            detail: format!(
                "Background-refresh scheduling is not yet implemented on {platform}. \
                 Run `fourda --engine-once` from a {platform} scheduler (cron/launchd/systemd) manually."
            ),
        }
    }
}

// ============================================================================
// Public API (platform-dispatched)
// ============================================================================

/// Install/replace the background-refresh task to run `<exe> <headless_arg>` every `interval_minutes`.
pub fn install(
    interval_minutes: u64,
    exe: &Path,
    headless_arg: &str,
) -> Result<SchedulerStatus, String> {
    let interval = interval_minutes.max(1);
    #[cfg(target_os = "windows")]
    {
        windows::install(interval, exe, headless_arg)?;
        Ok(status())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (interval, exe, headless_arg);
        Err(status().detail)
    }
}

/// Remove the background-refresh task. Succeeds (idempotently) if it does not exist.
pub fn uninstall() -> Result<SchedulerStatus, String> {
    #[cfg(target_os = "windows")]
    {
        windows::uninstall()?;
        Ok(status())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(status().detail)
    }
}

/// Report whether the task exists and, best-effort, its interval.
pub fn status() -> SchedulerStatus {
    #[cfg(target_os = "windows")]
    {
        windows::status()
    }
    #[cfg(target_os = "macos")]
    {
        SchedulerStatus::unsupported("macos")
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        SchedulerStatus::unsupported("linux")
    }
}

// ============================================================================
// Tauri commands — the settings-UI background-refresh toggle (MonitoringSection.tsx)
// ============================================================================

/// Enable background refresh: install a scheduled task running the shipped binary `--engine-once`
/// every `interval_minutes` (default 30). Returns the resulting status.
#[tauri::command]
pub fn install_background_refresh(
    interval_minutes: Option<u64>,
) -> Result<SchedulerStatus, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not resolve the 4DA executable path: {e}"))?;
    install(
        interval_minutes.unwrap_or(DEFAULT_INTERVAL_MINUTES),
        &exe,
        "--engine-once",
    )
}

/// Disable background refresh: remove the scheduled task.
#[tauri::command]
pub fn uninstall_background_refresh() -> Result<SchedulerStatus, String> {
    uninstall()
}

/// Current background-refresh status (installed? interval? platform support?).
#[tauri::command]
pub fn background_refresh_status() -> SchedulerStatus {
    status()
}

// ============================================================================
// Windows implementation (schtasks)
// ============================================================================

#[cfg(target_os = "windows")]
mod windows {
    use super::{SchedulerStatus, TASK_NAME};
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    /// Spawn schtasks with no console window. fourda.exe is a GUI-subsystem binary, so a
    /// console-subsystem child spawned without this flag makes Windows allocate a fresh console —
    /// a black window flashes on the user's desktop every time we install/uninstall/query the task.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// `schtasks /Create` — `/TR` carries the quoted exe + headless arg; `/SC MINUTE /MO N` repeats
    /// every N minutes; `/F` overwrites an existing task. Runs in the user context when logged on
    /// (no admin, no stored password), which is correct for refreshing the user's local database.
    pub(super) fn install(
        interval_minutes: u64,
        exe: &Path,
        headless_arg: &str,
    ) -> Result<(), String> {
        // `"<exe>" <arg>` as a single /TR value; std's Windows quoting escapes the inner quotes so
        // schtasks re-parses the exe path correctly even under "C:\Program Files\...".
        let task_run = format!("\"{}\" {}", exe.display(), headless_arg);
        let output = Command::new("schtasks")
            .args([
                "/Create",
                "/F",
                "/TN",
                TASK_NAME,
                "/TR",
                &task_run,
                "/SC",
                "MINUTE",
                "/MO",
                &interval_minutes.to_string(),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to run schtasks: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "schtasks /Create failed (exit {:?}): {}{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim(),
            ))
        }
    }

    /// `schtasks /Delete /F`. Treats "task does not exist" as success (idempotent disable).
    pub(super) fn uninstall() -> Result<(), String> {
        let output = Command::new("schtasks")
            .args(["/Delete", "/F", "/TN", TASK_NAME])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to run schtasks: {e}"))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        // ERROR: The specified task name ... does not exist — already disabled, treat as success.
        if stderr.contains("does not exist") || stderr.contains("cannot find") {
            return Ok(());
        }
        Err(format!(
            "schtasks /Delete failed (exit {:?}): {}",
            output.status.code(),
            stderr.trim(),
        ))
    }

    /// `schtasks /Query /V /FO LIST` — exit 0 means the task exists; parse the repeat interval and
    /// task-to-run from the verbose listing where available (field names are locale-sensitive, so
    /// parsing is best-effort and never fatal).
    pub(super) fn status() -> SchedulerStatus {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME, "/V", "/FO", "LIST"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                SchedulerStatus {
                    installed: true,
                    supported: true,
                    platform: "windows".to_string(),
                    task_name: TASK_NAME.to_string(),
                    interval_minutes: parse_repeat_minutes(&text),
                    detail: parse_field(&text, "Task To Run")
                        .unwrap_or_else(|| "Task installed.".to_string()),
                }
            }
            _ => SchedulerStatus {
                installed: false,
                supported: true,
                platform: "windows".to_string(),
                task_name: TASK_NAME.to_string(),
                interval_minutes: None,
                detail: "No background-refresh task is installed.".to_string(),
            },
        }
    }

    /// Extract the value of a `Field:  value` line from `schtasks /FO LIST` output.
    fn parse_field(text: &str, field: &str) -> Option<String> {
        text.lines()
            .find(|l| l.trim_start().starts_with(field))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    /// Best-effort parse of the "Repeat: Every:" interval (e.g. "0 Hour(s), 30 Minute(s)") to minutes.
    fn parse_repeat_minutes(text: &str) -> Option<u64> {
        let line = text.lines().find(|l| {
            l.contains("Repeat: Every:") || l.trim_start().starts_with("Repeat: Every")
        })?;
        let mut minutes = 0u64;
        let lower = line.to_lowercase();
        for (idx, tok) in lower.split_whitespace().enumerate() {
            if let Ok(n) = tok.parse::<u64>() {
                let rest = lower.split_whitespace().nth(idx + 1).unwrap_or("");
                if rest.starts_with("hour") {
                    minutes += n * 60;
                } else if rest.starts_with("minute") {
                    minutes += n;
                }
            }
        }
        (minutes > 0).then_some(minutes)
    }
}
