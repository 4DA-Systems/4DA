// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Boot context detection — adapt the cold-boot grace period to the actual
//! cause of this 4DA launch.
//!
//! Sovereign Cold Boot uses a fixed 90-second grace period as the safe
//! default, but the optimal grace differs by launch cause:
//!
//! | Boot context        | Why                                      | Grace |
//! |---------------------|------------------------------------------|-------|
//! | `ColdPowerOn`       | System just powered on, OS still loading | 90s   |
//! | `AutoStart`         | Launched at login, user is in their flow | 90s   |
//! | `UserLaunched`      | User clicked icon, expects responsiveness| 30s   |
//! | `ProcessRestart`    | 4DA was running minutes ago              | 0s    |
//!
//! The detection logic intentionally uses NO new crate dependencies:
//! - System uptime: direct platform syscalls (`GetTickCount64`,
//!   `sysctl(KERN_BOOTTIME)`, `/proc/uptime`)
//! - Autostart state: existing `tauri-plugin-autostart`
//! - Process recency: the existing `scheduler_state` table from Wave 1
//!
//! Detection runs ONCE at startup. The result is cached in a `OnceCell`
//! and exposed via `current_grace_secs()` which the scheduler reads
//! before applying its grace period.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use once_cell::sync::OnceCell;
use tracing::info;

/// What kind of cold boot is this?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootContext {
    /// System uptime is short (<5 min). The OS itself just finished booting,
    /// so the user is still settling in and we shouldn't compete for CPU.
    ColdPowerOn,
    /// 4DA was launched as part of OS login (autostart enabled and uptime
    /// is recent). Treat the same as cold power-on — the user wants a
    /// responsive desktop, not a CPU spike.
    AutoStart,
    /// User clicked the icon themselves on a system that has been running
    /// for a while. They expect immediate responsiveness from THIS click,
    /// so we tighten the grace period to 30s.
    UserLaunched,
    /// 4DA was running very recently (the persisted scheduler state has
    /// timestamps from <10 minutes ago). This is effectively a process
    /// restart — no grace needed, the persisted state already prevents
    /// the stampede.
    ProcessRestart,
}

impl BootContext {
    /// Grace period in seconds for this boot context.
    pub fn grace_secs(self) -> u64 {
        match self {
            BootContext::ColdPowerOn | BootContext::AutoStart => 90,
            BootContext::UserLaunched => 30,
            BootContext::ProcessRestart => 0,
        }
    }

    /// Human-readable label for logging.
    pub fn label(self) -> &'static str {
        match self {
            BootContext::ColdPowerOn => "cold-power-on",
            BootContext::AutoStart => "autostart",
            BootContext::UserLaunched => "user-launched",
            BootContext::ProcessRestart => "process-restart",
        }
    }
}

/// Cached grace period in seconds, set by `detect_and_cache`.
/// The scheduler reads this once during startup and uses it as its grace.
static CACHED_GRACE_SECS: AtomicU64 = AtomicU64::new(90);

/// Cached boot context, set by `detect_and_cache`. Exposed for diagnostics.
static CACHED_BOOT_CONTEXT: OnceCell<BootContext> = OnceCell::new();

/// Get the cached cold-boot grace period for this process.
/// Returns the default (90s) until `detect_and_cache` is called.
pub fn current_grace_secs() -> u64 {
    CACHED_GRACE_SECS.load(Ordering::Relaxed)
}

/// Get the cached boot context (if detection has run).
#[allow(dead_code)] // exposed for diagnostics + future telemetry
pub fn current_boot_context() -> Option<BootContext> {
    CACHED_BOOT_CONTEXT.get().copied()
}

/// Detect this process's boot context and cache the result.
/// Called once from `setup_app` BEFORE the scheduler starts ticking.
///
/// `autostart_enabled` should come from the `tauri-plugin-autostart` plugin
/// (`app.autolaunch().is_enabled()`). Pass `false` if the check fails — the
/// detector treats unknown autostart state as user-launched.
pub fn detect_and_cache(autostart_enabled: bool) -> BootContext {
    let uptime = system_uptime().unwrap_or(Duration::from_secs(u64::MAX));
    let uptime_secs = uptime.as_secs();

    // Check the persisted scheduler state for ANY recent job completion.
    // If 4DA was running within the last ~10 minutes, this is a process
    // restart (e.g. user closed and reopened, dev reload, crash recovery).
    // The persisted state already prevents the stampede; no extra grace needed.
    let last_run = most_recent_persisted_run();
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let recently_running = last_run > 0 && now_secs.saturating_sub(last_run) < 600;

    let context = if recently_running {
        BootContext::ProcessRestart
    } else if uptime_secs < 300 {
        // System uptime <5 min — definitely a cold power-on regardless of autostart.
        BootContext::ColdPowerOn
    } else if autostart_enabled && uptime_secs < 600 {
        // Autostart enabled + uptime <10 min: launched as part of login flow.
        // We give login flows the same generous grace as cold boot because
        // multiple apps are competing for CPU on the user's desktop.
        BootContext::AutoStart
    } else {
        // System has been running a while — user clicked the icon themselves.
        BootContext::UserLaunched
    };

    let grace = context.grace_secs();
    CACHED_GRACE_SECS.store(grace, Ordering::Relaxed);
    let _ = CACHED_BOOT_CONTEXT.set(context);

    info!(
        target: "4da::boot_context",
        context = %context.label(),
        grace_s = grace,
        uptime_s = uptime_secs,
        autostart = autostart_enabled,
        recently_running,
        "Boot context detected"
    );

    context
}

/// Find the most recent `last_run_unix` across all jobs in `scheduler_state`.
/// Returns 0 if the table is missing/empty or the DB is unreachable.
fn most_recent_persisted_run() -> u64 {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.query_row(
        "SELECT COALESCE(MAX(last_run_unix), 0) FROM scheduler_state",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|v| v.max(0) as u64)
    .unwrap_or(0)
}

// ============================================================================
// Platform-specific system uptime
// ============================================================================

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn system_uptime() -> Option<Duration> {
    // GetTickCount64 returns milliseconds since system boot.
    // Available on Windows Vista and later (we target Win 10+).
    // Pure read of a system clock value — no pointer dereference, no FFI
    // memory exchange. Cannot panic, cannot leak resources.
    extern "system" {
        fn GetTickCount64() -> u64;
    }
    let ms = unsafe { GetTickCount64() };
    Some(Duration::from_millis(ms))
}

#[cfg(target_os = "linux")]
fn system_uptime() -> Option<Duration> {
    // /proc/uptime contains "<uptime_seconds> <idle_seconds>" as floats.
    let contents = std::fs::read_to_string("/proc/uptime").ok()?;
    let first = contents.split_whitespace().next()?;
    let secs: f64 = first.parse().ok()?;
    Some(Duration::from_secs_f64(secs))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn system_uptime() -> Option<Duration> {
    // sysctl({CTL_KERN, KERN_BOOTTIME}) returns a `timeval` with the
    // wall-clock time of system boot. We compare against now() to get uptime.
    use std::mem;

    #[repr(C)]
    struct Timeval {
        tv_sec: i64,
        tv_usec: i64,
    }

    extern "C" {
        fn sysctl(
            name: *mut i32,
            namelen: u32,
            oldp: *mut std::ffi::c_void,
            oldlenp: *mut usize,
            newp: *mut std::ffi::c_void,
            newlen: usize,
        ) -> i32;
    }

    const CTL_KERN: i32 = 1;
    const KERN_BOOTTIME: i32 = 21;

    let mut mib = [CTL_KERN, KERN_BOOTTIME];
    let mut tv = Timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut size = mem::size_of::<Timeval>();

    let rc = unsafe {
        sysctl(
            mib.as_mut_ptr(),
            2,
            &mut tv as *mut _ as *mut std::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 || tv.tv_sec <= 0 {
        return None;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let uptime_secs = now.as_secs().saturating_sub(tv.tv_sec as u64);
    Some(Duration::from_secs(uptime_secs))
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn system_uptime() -> Option<Duration> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grace_secs_per_context() {
        assert_eq!(BootContext::ColdPowerOn.grace_secs(), 90);
        assert_eq!(BootContext::AutoStart.grace_secs(), 90);
        assert_eq!(BootContext::UserLaunched.grace_secs(), 30);
        assert_eq!(BootContext::ProcessRestart.grace_secs(), 0);
    }

    #[test]
    fn label_per_context() {
        assert_eq!(BootContext::ColdPowerOn.label(), "cold-power-on");
        assert_eq!(BootContext::AutoStart.label(), "autostart");
        assert_eq!(BootContext::UserLaunched.label(), "user-launched");
        assert_eq!(BootContext::ProcessRestart.label(), "process-restart");
    }

    #[test]
    fn system_uptime_is_reasonable_on_supported_platforms() {
        // The system has been up for at least a few seconds by the time this
        // test runs. We can't assert an exact value but we can sanity-check
        // that the result is non-zero on supported platforms.
        if cfg!(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "macos"
        )) {
            let uptime = system_uptime();
            assert!(
                uptime.is_some(),
                "uptime detection should work on this platform"
            );
            let secs = uptime.unwrap().as_secs();
            // CI machines have been up for at least one second by the time
            // this test runs. We don't bound from above because long-running
            // dev machines can have months of uptime.
            assert!(secs >= 1, "uptime should be at least 1 second, got {secs}");
        }
    }
}
