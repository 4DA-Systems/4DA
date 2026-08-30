// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tauri commands for the interruption gate — status, Do Not Disturb, and
//! quiet hours.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::error::Result;
use crate::presence::{self, queue};

/// Live gate status, for the settings panel and the "held" indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceStatus {
    /// Whether 4DA would deliver a notification right now.
    pub available: bool,
    /// Machine-readable reason it would not, if it would not.
    pub reason: Option<String>,
    /// Human-readable clause, e.g. "while you were in a fullscreen app".
    pub reason_text: Option<String>,
    /// How many surfaces are waiting for the user to become available.
    pub held_count: usize,
    /// Whether OS-level presence detection exists on this platform. When
    /// false, only quiet hours and Do Not Disturb are enforced.
    pub os_detection_supported: bool,
}

/// Read the current interruption state.
#[tauri::command]
pub async fn get_presence_status() -> Result<PresenceStatus> {
    let presence = presence::current();
    let reason = presence.busy_reason();
    Ok(PresenceStatus {
        available: presence.is_available(),
        reason: reason.map(|r| r.as_str().to_string()),
        reason_text: reason.map(|r| r.user_text().to_string()),
        held_count: queue::held_count(),
        os_detection_supported: cfg!(windows),
    })
}

/// The user's interruption preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptionConfigDto {
    /// Hold surfaces while fullscreen / presenting / Focus Assist is on.
    pub respect_focus: bool,
    /// Quiet hours start, "HH:MM", or null when unset.
    pub quiet_hours_start: Option<String>,
    /// Quiet hours end, "HH:MM", or null when unset.
    pub quiet_hours_end: Option<String>,
    /// Whether Do Not Disturb is currently in force.
    pub dnd_active: bool,
    /// When timed Do Not Disturb expires (RFC3339), if it is timed.
    pub dnd_until: Option<String>,
}

/// Read the interruption preferences.
#[tauri::command]
pub async fn get_interruption_config() -> Result<InterruptionConfigDto> {
    let settings = crate::get_settings_manager().lock();
    let monitoring = &settings.get().monitoring;
    let dto = InterruptionConfigDto {
        respect_focus: monitoring.respect_focus.unwrap_or(true),
        quiet_hours_start: monitoring.quiet_hours_start.clone(),
        quiet_hours_end: monitoring.quiet_hours_end.clone(),
        dnd_active: false, // filled in below, outside the lock
        dnd_until: monitoring.dnd_until.clone(),
    };
    drop(settings);

    Ok(InterruptionConfigDto {
        dnd_active: presence::is_do_not_disturb_on(),
        ..dto
    })
}

/// Turn "respect fullscreen and focus" on or off.
#[tauri::command]
pub async fn set_respect_focus(enabled: bool) -> Result<()> {
    {
        let mut settings = crate::get_settings_manager().lock();
        settings.get_mut().monitoring.respect_focus = Some(enabled);
        settings.save()?;
    }
    tracing::info!(target: "4da::presence", enabled, "respect_focus updated");
    Ok(())
}

/// Set or clear quiet hours.
///
/// Both ends must be valid `HH:MM` for quiet hours to take effect; passing
/// `None` for either clears the window. An invalid time is rejected rather
/// than silently coerced, so a typo can never mute 4DA indefinitely.
#[tauri::command]
pub async fn set_quiet_hours(start: Option<String>, end: Option<String>) -> Result<()> {
    for value in [start.as_deref(), end.as_deref()].into_iter().flatten() {
        if presence::parse_hhmm_to_minutes(value).is_none() {
            return Err(crate::error::FourDaError::Validation(format!(
                "Quiet hours must be HH:MM in 24-hour time, got {value:?}"
            )));
        }
    }

    {
        let mut settings = crate::get_settings_manager().lock();
        let monitoring = &mut settings.get_mut().monitoring;
        monitoring.quiet_hours_start = start.clone();
        monitoring.quiet_hours_end = end.clone();
        settings.save()?;
    }
    tracing::info!(target: "4da::presence", ?start, ?end, "Quiet hours updated");
    Ok(())
}

/// Turn Do Not Disturb on for `minutes`, or indefinitely when `minutes` is
/// null. Passing `enabled: false` turns it off and releases anything held.
#[tauri::command]
pub async fn set_do_not_disturb(
    app: AppHandle,
    enabled: bool,
    minutes: Option<u64>,
) -> Result<PresenceStatus> {
    if enabled {
        presence::set_do_not_disturb(minutes)?;
    } else {
        presence::clear_do_not_disturb()?;
        // Turning DND off is an explicit "I'm back" — release immediately
        // rather than making the user wait out the watcher's settle period.
        if presence::is_available() {
            queue::flush(&app);
        }
    }
    crate::monitoring::refresh_tray_menu(&app);
    get_presence_status().await
}

/// Deliver everything the gate is holding, right now.
///
/// Exposed so the UI can offer "show me what I missed" without waiting for the
/// resume watcher.
#[tauri::command]
pub async fn flush_held_notifications(app: AppHandle) -> Result<usize> {
    let count = queue::held_count();
    queue::flush(&app);
    Ok(count)
}

/// Discard everything held without showing it.
///
/// Called when the user opens 4DA themselves — they are looking at the Brief
/// tab now, so replaying a briefing popup at them would be redundant.
#[tauri::command]
pub async fn discard_held_notifications() -> Result<()> {
    queue::clear();
    Ok(())
}
