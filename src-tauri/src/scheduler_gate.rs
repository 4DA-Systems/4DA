// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Scheduler Gate — resource-aware throttling for background work.
//!
//! Probes host power state (AC/battery), battery percentage, CPU load,
//! and user idle time to derive a `GatePolicy` that the scheduler uses
//! to decide which jobs to run, at what intervals.

use std::sync::atomic::{AtomicU8, Ordering};
use tracing::{debug, info};

// ============================================================================
// Policy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GatePolicy {
    /// AC power, CPU headroom, user idle — run everything aggressively
    Aggressive = 0,
    /// AC power or battery > 80% — normal intervals
    Normal = 1,
    /// Battery < 80% OR CPU > 70% — double intervals, serialize jobs
    Throttled = 2,
    /// Battery < 20% OR user explicitly paused — critical jobs only
    Paused = 3,
}

impl GatePolicy {
    pub fn interval_multiplier(self) -> u64 {
        match self {
            Self::Aggressive => 1,
            Self::Normal => 1,
            Self::Throttled => 2,
            Self::Paused => 4,
        }
    }

    pub fn allows_job(self, priority: JobPriority) -> bool {
        match self {
            Self::Aggressive | Self::Normal => true,
            Self::Throttled => matches!(
                priority,
                JobPriority::Critical | JobPriority::High | JobPriority::Normal
            ),
            Self::Paused => matches!(priority, JobPriority::Critical),
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Aggressive,
            1 => Self::Normal,
            2 => Self::Throttled,
            3 => Self::Paused,
            _ => Self::Normal,
        }
    }
}

impl std::fmt::Display for GatePolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aggressive => write!(f, "aggressive"),
            Self::Normal => write!(f, "normal"),
            Self::Throttled => write!(f, "throttled"),
            Self::Paused => write!(f, "paused"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobPriority {
    Critical,
    High,
    Normal,
    Low,
}

// ============================================================================
// Host State Probing
// ============================================================================

#[derive(Debug, Clone)]
pub struct HostState {
    pub on_ac_power: bool,
    pub battery_percent: Option<u8>,
    pub cpu_load_percent: Option<f32>,
    pub user_idle_secs: Option<u64>,
}

impl HostState {
    pub fn probe() -> Self {
        let (on_ac, battery_pct) = probe_power_state();
        let cpu_load = probe_cpu_load();
        let idle_secs = probe_user_idle();

        Self {
            on_ac_power: on_ac,
            battery_percent: battery_pct,
            cpu_load_percent: cpu_load,
            user_idle_secs: idle_secs,
        }
    }

    pub fn derive_policy(&self) -> GatePolicy {
        // Paused conditions (most restrictive first)
        if let Some(pct) = self.battery_percent {
            if !self.on_ac_power && pct < 20 {
                return GatePolicy::Paused;
            }
        }

        // Throttled conditions
        if let Some(cpu) = self.cpu_load_percent {
            if cpu > 70.0 {
                return GatePolicy::Throttled;
            }
        }
        if let Some(pct) = self.battery_percent {
            if !self.on_ac_power && pct < 80 {
                return GatePolicy::Throttled;
            }
        }

        // Aggressive conditions (AC + low CPU + user idle)
        let cpu_low = self.cpu_load_percent.map_or(true, |c| c < 50.0);
        let user_idle = self.user_idle_secs.map_or(false, |s| s > 300);
        if self.on_ac_power && cpu_low && user_idle {
            return GatePolicy::Aggressive;
        }

        GatePolicy::Normal
    }
}

// ============================================================================
// Global Gate State
// ============================================================================

static CURRENT_POLICY: AtomicU8 = AtomicU8::new(1); // Normal

pub fn current_policy() -> GatePolicy {
    GatePolicy::from_u8(CURRENT_POLICY.load(Ordering::Relaxed))
}

pub fn update_policy() -> GatePolicy {
    let state = HostState::probe();
    let new_policy = state.derive_policy();
    let old_policy = GatePolicy::from_u8(CURRENT_POLICY.swap(new_policy as u8, Ordering::Relaxed));

    if old_policy != new_policy {
        info!(
            target: "4da::gate",
            from = %old_policy,
            to = %new_policy,
            on_ac = state.on_ac_power,
            battery = ?state.battery_percent,
            cpu = ?state.cpu_load_percent,
            idle_s = ?state.user_idle_secs,
            "Scheduler gate policy changed"
        );
    } else {
        debug!(
            target: "4da::gate",
            policy = %new_policy,
            "Gate policy unchanged"
        );
    }

    new_policy
}

pub fn should_run_job(priority: JobPriority) -> bool {
    current_policy().allows_job(priority)
}

pub fn effective_interval(base_interval_secs: u64) -> u64 {
    base_interval_secs * current_policy().interval_multiplier()
}

// ============================================================================
// Platform-Specific Probes (Windows)
// ============================================================================

#[cfg(windows)]
#[allow(unsafe_code)]
fn probe_power_state() -> (bool, Option<u8>) {
    use windows_sys::Win32::System::Power::GetSystemPowerStatus;
    use windows_sys::Win32::System::Power::SYSTEM_POWER_STATUS;

    let mut status = SYSTEM_POWER_STATUS {
        ACLineStatus: 0,
        BatteryFlag: 0,
        BatteryLifePercent: 0,
        SystemStatusFlag: 0,
        BatteryLifeTime: 0,
        BatteryFullLifeTime: 0,
    };

    let success = unsafe { GetSystemPowerStatus(&mut status) };
    if success == 0 {
        return (true, None); // assume AC on failure
    }

    let on_ac = status.ACLineStatus == 1;
    let battery_pct = if status.BatteryLifePercent <= 100 {
        Some(status.BatteryLifePercent)
    } else {
        None // 255 means unknown
    };

    (on_ac, battery_pct)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn probe_user_idle() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount64;

    #[repr(C)]
    struct LASTINPUTINFO {
        cb_size: u32,
        dw_time: u32,
    }

    extern "system" {
        fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
    }

    let mut info = LASTINPUTINFO {
        cb_size: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dw_time: 0,
    };

    let success = unsafe { GetLastInputInfo(&mut info) };
    if success == 0 {
        return None;
    }

    let tick_count = unsafe { GetTickCount64() };
    let idle_ms = tick_count.saturating_sub(info.dw_time as u64);
    Some(idle_ms / 1000)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn probe_cpu_load() -> Option<f32> {
    // Simple heuristic: use GetSystemTimes to compute CPU usage
    // over the interval between calls. On first call, return None.
    use std::sync::Mutex;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        fn as_u64(self) -> u64 {
            (self.high as u64) << 32 | self.low as u64
        }
    }

    extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    }

    static PREV: Mutex<Option<(u64, u64, u64)>> = Mutex::new(None);

    let mut idle = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();

    let success = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if success == 0 {
        return None;
    }

    let idle_t = idle.as_u64();
    let kernel_t = kernel.as_u64();
    let user_t = user.as_u64();

    let mut prev = PREV.lock().ok()?;
    let result = if let Some((prev_idle, prev_kernel, prev_user)) = *prev {
        let d_idle = idle_t.saturating_sub(prev_idle);
        let d_kernel = kernel_t.saturating_sub(prev_kernel);
        let d_user = user_t.saturating_sub(prev_user);
        let d_total = d_kernel + d_user;
        if d_total == 0 {
            Some(0.0)
        } else {
            // kernel time includes idle time on Windows
            let busy = d_total.saturating_sub(d_idle);
            Some((busy as f32 / d_total as f32) * 100.0)
        }
    } else {
        None
    };

    *prev = Some((idle_t, kernel_t, user_t));
    result
}

// ============================================================================
// Platform-Specific Probes (non-Windows fallback)
// ============================================================================

#[cfg(not(windows))]
fn probe_power_state() -> (bool, Option<u8>) {
    (true, None) // assume AC, unknown battery
}

#[cfg(not(windows))]
fn probe_user_idle() -> Option<u64> {
    None
}

#[cfg(not(windows))]
fn probe_cpu_load() -> Option<f32> {
    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_from_ac_low_cpu_idle() {
        let state = HostState {
            on_ac_power: true,
            battery_percent: Some(100),
            cpu_load_percent: Some(20.0),
            user_idle_secs: Some(600),
        };
        assert_eq!(state.derive_policy(), GatePolicy::Aggressive);
    }

    #[test]
    fn test_policy_from_ac_no_idle() {
        let state = HostState {
            on_ac_power: true,
            battery_percent: Some(100),
            cpu_load_percent: Some(30.0),
            user_idle_secs: Some(10),
        };
        assert_eq!(state.derive_policy(), GatePolicy::Normal);
    }

    #[test]
    fn test_policy_battery_low() {
        let state = HostState {
            on_ac_power: false,
            battery_percent: Some(15),
            cpu_load_percent: Some(30.0),
            user_idle_secs: Some(600),
        };
        assert_eq!(state.derive_policy(), GatePolicy::Paused);
    }

    #[test]
    fn test_policy_battery_medium() {
        let state = HostState {
            on_ac_power: false,
            battery_percent: Some(60),
            cpu_load_percent: Some(30.0),
            user_idle_secs: Some(10),
        };
        assert_eq!(state.derive_policy(), GatePolicy::Throttled);
    }

    #[test]
    fn test_policy_high_cpu() {
        let state = HostState {
            on_ac_power: true,
            battery_percent: Some(100),
            cpu_load_percent: Some(85.0),
            user_idle_secs: Some(600),
        };
        assert_eq!(state.derive_policy(), GatePolicy::Throttled);
    }

    #[test]
    fn test_interval_multiplier() {
        assert_eq!(GatePolicy::Aggressive.interval_multiplier(), 1);
        assert_eq!(GatePolicy::Normal.interval_multiplier(), 1);
        assert_eq!(GatePolicy::Throttled.interval_multiplier(), 2);
        assert_eq!(GatePolicy::Paused.interval_multiplier(), 4);
    }

    #[test]
    fn test_job_allowed() {
        assert!(GatePolicy::Paused.allows_job(JobPriority::Critical));
        assert!(!GatePolicy::Paused.allows_job(JobPriority::Normal));
        assert!(!GatePolicy::Paused.allows_job(JobPriority::Low));
        assert!(GatePolicy::Throttled.allows_job(JobPriority::Normal));
        assert!(!GatePolicy::Throttled.allows_job(JobPriority::Low));
        assert!(GatePolicy::Normal.allows_job(JobPriority::Low));
    }

    #[test]
    fn test_desktop_no_battery_defaults_normal() {
        let state = HostState {
            on_ac_power: true,
            battery_percent: None,
            cpu_load_percent: None,
            user_idle_secs: None,
        };
        assert_eq!(state.derive_policy(), GatePolicy::Normal);
    }
}
