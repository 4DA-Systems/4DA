// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Desktop-level intelligence briefing window for 4DA.
//!
//! A dedicated webview (separate from the signal notification window) that displays
//! the daily morning briefing with enriched data: synthesized intelligence,
//! knowledge gaps, and signal priorities.
//!
//! The briefing window is:
//! - 560×780 logical pixels, positioned bottom-right above the taskbar
//! - Transparent, borderless, always-on-top for 8 seconds then normal z-order
//! - Pre-created on app startup (hidden) for instant display
//! - Visible in taskbar, accessible via Alt+Tab
//! - Auto-dismisses after 5 minutes of no interaction
//! - Never steals focus on creation (focused: false)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewUrl};
use tracing::{info, warn};

use crate::monitoring_briefing::BriefingNotification;

/// Label used for the briefing window.
const WINDOW_LABEL: &str = "briefing";

/// Briefing window dimensions (logical pixels).
const WINDOW_WIDTH: u32 = 560;
const WINDOW_HEIGHT: u32 = 780;

/// Auto-dismiss after 5 minutes of no interaction.
/// Previous value of 60s was too short — users routinely missed the briefing
/// because it vanished before they noticed it.
const AUTO_DISMISS_MS: u64 = 300_000;

/// Duration (ms) the briefing stays always-on-top before dropping to normal z-order.
/// Must be long enough for async LLM synthesis to arrive (~5-10s) plus reading time.
/// The window never steals focus, so "always-on-top" just means visible — the user
/// can still click through to their IDE. JS hover/click handles actual dismissal.
const ALWAYS_ON_TOP_MS: u64 = 30_000;

// ============================================================================
// Auto-dismiss cancellation
// ============================================================================

static DISMISS_CANCEL: std::sync::LazyLock<parking_lot::Mutex<Option<Arc<AtomicBool>>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(None));

/// Whether the briefing window's JS listener is registered and ready.
static WINDOW_READY: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Window Lifecycle
// ============================================================================

/// Pre-create the briefing window in a hidden state.
///
/// Called once during app startup so the window is warm and can be shown
/// instantly when a morning briefing fires.
pub fn init_briefing_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let _window = tauri::webview::WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        WebviewUrl::App("briefing.html".into()),
    )
    .title("4DA — Daily Brief")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(false)
    .focused(false)
    .resizable(false)
    .visible(false)
    .inner_size(f64::from(WINDOW_WIDTH), f64::from(WINDOW_HEIGHT))
    .build()?;

    info!(target: "4da::briefing", "Briefing window pre-warmed");
    Ok(())
}

/// Mark the briefing window as ready (JS listener registered).
/// Called when the frontend emits `briefing-ready`.
pub fn mark_ready() {
    WINDOW_READY.store(true, Ordering::Relaxed);
}

/// Show the briefing window with enriched morning briefing data.
///
/// Positions bottom-right above the taskbar, emits the data payload,
/// shows always-on-top for 8 seconds (so the user notices it), then
/// drops to normal z-order. Auto-dismisses after 5 minutes.
pub fn show_briefing<R: Runtime>(app: &AppHandle<R>, briefing: &BriefingNotification) {
    // Cancel any existing dismiss timer.
    cancel_dismiss_timer();

    // Recovery: if the window's JS never loaded (dev server race condition on
    // startup), destroy the stale window so it gets recreated with a fresh load.
    // By the time a briefing fires, the dev server is guaranteed to be up.
    if !WINDOW_READY.load(Ordering::Relaxed) {
        if let Some(w) = app.get_webview_window(WINDOW_LABEL) {
            info!(target: "4da::briefing", "Briefing window JS never loaded — recreating");
            let _ = w.destroy();
        }
    }

    // Ensure the window exists.
    let window = if let Some(w) = app.get_webview_window(WINDOW_LABEL) {
        w
    } else {
        if let Err(e) = init_briefing_window(app) {
            warn!(target: "4da::briefing", error = %e, "Failed to create briefing window");
            return;
        }
        if let Some(w) = app.get_webview_window(WINDOW_LABEL) {
            w
        } else {
            warn!(target: "4da::briefing", "Briefing window missing after init");
            return;
        }
    };

    // Position bottom-right, above the taskbar — like a desktop widget.
    let positioned = (|| -> tauri::Result<()> {
        let monitor = window
            .primary_monitor()?
            .or_else(|| window.available_monitors().ok()?.into_iter().next());

        if let Some(monitor) = monitor {
            let scale = monitor.scale_factor();
            let screen = monitor.size();
            let monitor_pos = monitor.position();

            let win_w = (WINDOW_WIDTH as f64 * scale) as i32;
            let win_h = (WINDOW_HEIGHT as f64 * scale) as i32;
            let margin_right = (24.0 * scale) as i32;
            let margin_bottom = (80.0 * scale) as i32; // Above the taskbar

            let px = monitor_pos.x + (screen.width as i32 - win_w) - margin_right;
            let py = monitor_pos.y + (screen.height as i32 - win_h) - margin_bottom;

            window.set_position(PhysicalPosition::new(px, py))?;
        } else {
            window.center()?;
        }
        Ok(())
    })();

    if let Err(e) = positioned {
        warn!(target: "4da::briefing", error = %e, "Failed to position briefing window");
    }

    // Emit data to the briefing webview.
    // If the JS listener isn't registered yet, retry with exponential backoff
    // up to ~5 seconds total. This handles cold starts, slow disks, and first launch.
    if !WINDOW_READY.load(Ordering::Relaxed) {
        info!(target: "4da::briefing", "Window not ready, deferring briefing with retry loop");
        let app_deferred = app.clone();
        let data = briefing.clone();
        tauri::async_runtime::spawn(async move {
            let delays_ms: &[u64] = &[200, 400, 800, 1500, 2500];
            for (attempt, &delay) in delays_ms.iter().enumerate() {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                if WINDOW_READY.load(Ordering::Relaxed) {
                    info!(target: "4da::briefing", attempt = attempt + 1, "Window became ready");
                    break;
                }
            }
            // Emit regardless — even if not ready, the listener may be registered
            // but the ready event failed to propagate.
            if let Err(e) = app_deferred.emit_to(WINDOW_LABEL, "briefing-data", &data) {
                warn!(target: "4da::briefing", error = %e, "Deferred briefing emit failed after retries");
            } else {
                info!(target: "4da::briefing", "Briefing data emitted after deferred wait");
            }
        });
    } else if let Err(e) = app.emit_to(WINDOW_LABEL, "briefing-data", briefing) {
        warn!(target: "4da::briefing", error = %e, "Failed to emit briefing data");
    }

    // Re-raise to always-on-top for this showing (may have been dropped by
    // a previous show cycle or by the auto-drop timer).
    let _ = window.set_always_on_top(true);

    // Show the window without stealing focus — never interrupt the user.
    // The window is always-on-top so it appears above other windows, ensuring
    // the user actually sees it. After ALWAYS_ON_TOP_MS it drops to normal
    // z-order so it doesn't obstruct ongoing work.
    if let Err(e) = window.show() {
        warn!(target: "4da::briefing", error = %e, "Failed to show briefing window");
        return;
    }

    info!(
        target: "4da::briefing",
        items = briefing.items.len(),
        gaps = briefing.knowledge_gaps.len(),
        "Intelligence briefing shown (always-on-top for {}s, auto-dismiss in {}s)",
        ALWAYS_ON_TOP_MS / 1000,
        AUTO_DISMISS_MS / 1000,
    );

    // After ALWAYS_ON_TOP_MS, drop to normal z-order so the briefing doesn't
    // block the user's work. The window remains visible and in the taskbar.
    {
        let aot_window = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(ALWAYS_ON_TOP_MS)).await;
            if let Err(e) = aot_window.set_always_on_top(false) {
                tracing::debug!(target: "4da::briefing", error = %e, "Failed to drop always-on-top (non-fatal)");
            }
        });
    }

    // Start auto-dismiss timer.
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut guard = DISMISS_CANCEL.lock();
        *guard = Some(Arc::clone(&cancelled));
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(AUTO_DISMISS_MS)).await;
        if !cancelled.load(Ordering::Relaxed) {
            hide_briefing(&app_handle);
        }
    });
}

/// Hide the briefing window.
pub fn hide_briefing<R: Runtime>(app: &AppHandle<R>) {
    cancel_dismiss_timer();
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

/// Cancel any pending auto-dismiss timer.
fn cancel_dismiss_timer() {
    let mut guard = DISMISS_CANCEL.lock();
    if let Some(ref cancel) = *guard {
        cancel.store(true, Ordering::Relaxed);
    }
    *guard = None;
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Called from the briefing frontend when the user clicks a briefing item.
///
/// Records the interaction in ACE for engagement tracking, hides the briefing,
/// brings the main window to focus, and emits a navigation event.
#[tauri::command]
pub async fn briefing_item_clicked(app: AppHandle, item_id: Option<i64>) {
    // Record briefing click in ACE behavior system for engagement tracking
    if let Some(id) = item_id {
        if let Ok(ace) = crate::get_ace_engine() {
            let _ = ace.record_interaction(
                id,
                crate::ace::behavior::BehaviorAction::BriefingClick,
                vec![], // Topics will be extracted from the item by ACE
                "briefing".to_string(),
            );
        }
    }

    hide_briefing(&app);

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }

    let payload = serde_json::json!({ "item_id": item_id });
    if let Err(e) = app.emit("navigate-to-signals", payload) {
        warn!(target: "4da::briefing", error = %e, "Failed to emit navigate-to-signals");
    }

    info!(target: "4da::briefing", ?item_id, "Briefing item clicked — navigating to signals");
}

/// Called from the briefing frontend when the user opens a URL.
///
/// Opens the URL in the system browser via tauri-plugin-opener.
#[tauri::command]
pub async fn briefing_open_url(url: String) -> crate::error::Result<()> {
    crate::utils::validate_safe_url(&url)?;
    if let Err(e) = tauri_plugin_opener::open_url(&url, None::<&str>) {
        warn!(target: "4da::briefing", error = %e, url = %url, "Failed to open URL");
    } else {
        info!(target: "4da::briefing", url = %url, "Opened briefing URL in browser");
    }
    Ok(())
}

/// Manually trigger the morning briefing for testing.
///
/// Bypasses the once-per-day and time-window checks. Uses the full enrichment
/// pipeline (quality gate, dedupe, knowledge gaps, chains,
/// preemption alerts, blind spot score) and spawns async LLM synthesis —
/// identical output to the scheduled path.
#[tauri::command]
pub async fn trigger_morning_briefing(app: AppHandle) -> crate::error::Result<String> {
    let user_lang = crate::i18n::get_user_language();

    // Fetch raw items from analysis state or DB (same logic as check_morning_briefing)
    let raw_items: Vec<crate::monitoring_briefing::BriefingItem> = {
        let analysis_state = crate::get_analysis_state().lock();
        if let Some(ref results) = analysis_state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .filter(|r| r.detected_lang == user_lang)
                .filter(|r| r.top_score >= crate::monitoring_briefing::BRIEFING_SCORE_FLOOR)
                .take(25)
                .map(|r| crate::monitoring_briefing::BriefingItem {
                    title: r.title.clone(),
                    source_type: r.source_type.clone(),
                    score: r.top_score,
                    signal_type: r.signal_type.clone(),
                    url: r.url.clone(),
                    item_id: Some(r.id as i64),
                    signal_priority: r.signal_priority.clone(),
                    description: r.signal_action.clone(),
                    matched_deps: r.signal_triggers.clone().unwrap_or_default(),
                    content_type: r.score_breakdown.as_ref().and_then(|b| b.content_type.clone()),
                    corroboration_count: 0,
                    alt_sources: vec![],
                })
                .collect()
        } else if let Ok(db) = crate::get_database() {
            let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
            db.get_relevant_items_since(
                period_start,
                crate::monitoring_briefing::BRIEFING_SCORE_FLOOR.into(),
                25,
                &user_lang,
            )
            .ok()
            .map(|db_items| {
                db_items
                    .into_iter()
                    .map(|i| crate::monitoring_briefing::BriefingItem {
                        title: i.title,
                        source_type: i.source_type,
                        score: i.relevance_score.unwrap_or(0.0) as f32,
                        signal_type: None,
                        url: i.url,
                        item_id: Some(i.id),
                        signal_priority: None,
                        description: None,
                        matched_deps: vec![],
                        content_type: None,
                        corroboration_count: 0,
                        alt_sources: vec![],
                    })
                    .collect()
            })
            .unwrap_or_default()
        } else {
            vec![]
        }
    };

    if raw_items.is_empty() {
        return Ok("No items available for briefing — run an analysis first".into());
    }

    // Run full enrichment pipeline (skip novelty so manual trigger always shows content)
    let briefing = crate::monitoring_briefing::build_enriched_briefing(raw_items, &user_lang, true);

    let total = briefing.items.len();
    let gaps = briefing.knowledge_gaps.len();
    let chains = briefing.escalating_chains.len();
    let preemption = briefing.preemption_alerts.len();

    // Show briefing window + OS notification
    crate::monitoring_notifications::send_morning_briefing_notification(&app, &briefing);

    // Spawn async LLM synthesis (same as scheduler path)
    {
        let app_synth = app.clone();
        let briefing_synth = briefing.clone();
        tauri::async_runtime::spawn(async move {
            match crate::monitoring_briefing::synthesize_morning_briefing(&briefing_synth).await {
                Ok(synthesis) => {
                    info!(target: "4da::briefing", "Manual briefing synthesis ready");
                    let _ = app_synth.emit_to("briefing", "briefing-synthesis", &synthesis);
                }
                Err(e) => {
                    info!(target: "4da::briefing", reason = %e, "Manual synthesis skipped");
                }
            }
        });
    }

    info!(
        target: "4da::briefing",
        items = total,
        gaps = gaps,
        chains = chains,
        preemption = preemption,
        "Manual briefing triggered (full enrichment)"
    );

    Ok(format!(
        "Briefing triggered: {} items, {} gaps, {} chains, {} preemption",
        total, gaps, chains, preemption
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_constants() {
        assert_eq!(WINDOW_WIDTH, 560);
        assert_eq!(WINDOW_HEIGHT, 780);
        assert_eq!(AUTO_DISMISS_MS, 300_000);
        assert_eq!(ALWAYS_ON_TOP_MS, 30_000);
        assert_eq!(WINDOW_LABEL, "briefing");
    }

    #[test]
    fn test_cancel_dismiss_timer_no_panic() {
        // Should not panic when no timer is set
        cancel_dismiss_timer();
    }

    #[test]
    fn test_mark_ready() {
        WINDOW_READY.store(false, Ordering::Relaxed);
        assert!(!WINDOW_READY.load(Ordering::Relaxed));
        mark_ready();
        assert!(WINDOW_READY.load(Ordering::Relaxed));
        // Reset for other tests
        WINDOW_READY.store(false, Ordering::Relaxed);
    }
}
