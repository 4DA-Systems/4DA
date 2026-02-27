// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use tauri::{AppHandle, Emitter};

use crate::{get_database, get_monitoring_state, void_engine, AnalysisStatus, SourceRelevance};

/// Emit a progress event to the frontend
pub(crate) fn emit_progress(
    app: &AppHandle,
    stage: &str,
    progress: f32,
    message: &str,
    processed: usize,
    total: usize,
) {
    let status = AnalysisStatus {
        stage: stage.to_string(),
        progress,
        message: message.to_string(),
        items_processed: processed,
        items_total: total,
    };
    let _ = app.emit("analysis-progress", &status);
}

// ============================================================================
// Void Engine Signal Helpers
// ============================================================================

/// Emit void signal: active source fetching
pub(crate) fn void_signal_fetching(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_fetching(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: cache fill complete
pub(crate) fn void_signal_cache_filled(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_cache_filled(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Extract a SignalSummary from analysis results.
fn extract_signal_summary(results: &[SourceRelevance]) -> Option<void_engine::SignalSummary> {
    let mut type_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut max_priority: u8 = 0;
    let mut critical_count: u32 = 0;

    for r in results {
        if let Some(ref st) = r.signal_type {
            *type_counts.entry(st.clone()).or_insert(0) += 1;
        }
        if let Some(ref sp) = r.signal_priority {
            let pval = match sp.as_str() {
                "critical" => 4u8,
                "high" => 3,
                "medium" => 2,
                "low" => 1,
                _ => 0,
            };
            if pval > max_priority {
                max_priority = pval;
            }
            if pval == 4 {
                critical_count += 1;
            }
        }
    }

    let total_signals: u32 = type_counts.values().sum();
    if total_signals == 0 {
        return None;
    }

    // Urgency: weighted sum / (total * max_weight)
    let weighted_sum: f32 = type_counts
        .iter()
        .map(|(slug, count)| {
            let weight = match slug.as_str() {
                "security_alert" => 4.0,
                "breaking_change" => 3.0,
                "tool_discovery" => 2.0,
                "tech_trend" => 2.0,
                "competitive_intel" => 2.0,
                "learning" => 1.0,
                _ => 1.0,
            };
            weight * (*count as f32)
        })
        .sum();
    let urgency = (weighted_sum / (total_signals as f32 * 4.0)).min(1.0);

    let dominant_type = type_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(s, _)| s.clone());

    Some(void_engine::SignalSummary {
        max_priority,
        critical_count,
        signal_type_counts: type_counts,
        dominant_type,
        urgency_score: urgency,
    })
}

/// Emit void signal: analysis complete with scores
pub(crate) fn void_signal_analysis_complete(app: &AppHandle, results: &[SourceRelevance]) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let top_scores: Vec<f32> = results
            .iter()
            .filter(|r| r.relevant)
            .map(|r| r.top_score)
            .collect();
        let summary = extract_signal_summary(results);
        let signal =
            void_engine::signal_after_analysis(db, monitoring, &top_scores, summary.as_ref());
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: notification fired
pub(crate) fn void_signal_notification(app: &AppHandle, is_critical: bool, count: usize) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_notification(db, monitoring, is_critical, count);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: source fetch progress
pub(crate) fn void_signal_fetch_progress(app: &AppHandle, completed: usize, total: usize) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_fetch_progress(db, monitoring, completed, total);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: ACE context changed
pub(crate) fn void_signal_context_change(app: &AppHandle, intensity: f32) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_context_change(db, monitoring, intensity);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit an achievement unlocked event to the frontend
pub fn emit_achievement_unlocked(
    app: &AppHandle,
    achievement: &crate::game_engine::AchievementUnlocked,
) {
    if let Err(e) = app.emit("achievement-unlocked", achievement) {
        tracing::debug!(target: "4da::events", error = %e, "Failed to emit achievement event");
    }
}

/// Emit void signal: error occurred
pub(crate) fn void_signal_error(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_error(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}
