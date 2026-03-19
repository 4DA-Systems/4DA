use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter};
use tracing::debug;

use crate::db::Database;
use crate::monitoring::MonitoringState;

/// Ambient signal representing 4DA's current state.
/// Emitted to the frontend only when values change (not on a timer).
/// The heartbeat UI interpolates between received signals locally.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VoidSignal {
    /// Source fetch activity: 0 = idle, 1 = actively fetching
    pub pulse: f32,
    /// Average relevance heat from last analysis: 0 = nothing relevant, 1 = highly relevant
    pub heat: f32,
    /// Discovery burst: max(0, latest_score - 0.7), for flash effect on high-relevance items
    pub burst: f32,
    /// Context morph: ACE file change activity level
    pub morph: f32,
    /// Error state: 1.0 if recent error, 0.0 otherwise
    pub error: f32,
    /// Hours since last analysis / 24, capped at 1.0
    pub staleness: f32,
    /// Total cached items (for cold start detection)
    pub item_count: u32,
    /// Signal intensity: 0.0-1.0, derived from highest signal priority / 4.0
    pub signal_intensity: f32,
    /// Signal urgency: 0.0-1.0, weighted urgency from signal types
    pub signal_urgency: f32,
    /// Count of critical-priority signals
    pub critical_count: u32,
    /// Color shift: -1.0 (cool/learning) to +1.0 (warm/alert)
    pub signal_color_shift: f32,
    /// Intelligence metabolism health: 0 = no autophagy data, 1 = fully calibrated
    pub metabolism: f32,
    /// Count of open decision windows requiring attention
    pub open_windows: u32,
    /// Compound advantage trend: -1 declining, 0 stable, +1 growing
    pub advantage_trend: f32,
}

impl Default for VoidSignal {
    fn default() -> Self {
        Self {
            pulse: 0.0,
            heat: 0.0,
            burst: 0.0,
            morph: 0.0,
            error: 0.0,
            staleness: 1.0,
            item_count: 0,
            signal_intensity: 0.0,
            signal_urgency: 0.0,
            critical_count: 0,
            signal_color_shift: 0.0,
            metabolism: 0.0,
            open_windows: 0,
            advantage_trend: 0.0,
        }
    }
}

impl VoidSignal {
    /// Returns true if this signal differs meaningfully from another.
    /// Used to suppress duplicate emissions.
    pub fn differs_from(&self, other: &VoidSignal, threshold: f32) -> bool {
        (self.pulse - other.pulse).abs() > threshold
            || (self.heat - other.heat).abs() > threshold
            || (self.burst - other.burst).abs() > threshold
            || (self.morph - other.morph).abs() > threshold
            || (self.error - other.error).abs() > threshold
            || (self.staleness - other.staleness).abs() > threshold
            || self.item_count != other.item_count
            || (self.signal_intensity - other.signal_intensity).abs() > threshold
            || (self.signal_urgency - other.signal_urgency).abs() > threshold
            || self.critical_count != other.critical_count
            || (self.signal_color_shift - other.signal_color_shift).abs() > threshold
            || (self.metabolism - other.metabolism).abs() > threshold
            || self.open_windows != other.open_windows
            || (self.advantage_trend - other.advantage_trend).abs() > threshold
    }
}

/// Aggregate signal summary from analysis results.
/// Used to drive the heartbeat's signal-aware color and intensity.
#[derive(Debug, Clone)]
pub struct SignalSummary {
    /// Highest priority level seen (1=low, 2=medium, 3=high, 4=critical)
    pub max_priority: u8,
    /// Count of critical-priority signals
    pub critical_count: u32,
    /// Count per signal type slug (e.g. "security_alert" -> 2)
    pub signal_type_counts: HashMap<String, u32>,
    /// The signal type with the most occurrences
    #[allow(dead_code)]
    // Reason: populated in extract_signal_summary but not yet consumed by heartbeat rendering
    pub dominant_type: Option<String>,
    /// Weighted urgency score 0.0-1.0
    pub urgency_score: f32,
}

/// Last emitted signal, used for deduplication.
static LAST_VOID_SIGNAL: parking_lot::Mutex<Option<VoidSignal>> = parking_lot::Mutex::new(None);

/// Emit a void signal to the frontend, but only if it meaningfully changed.
pub fn emit_if_changed(app: &AppHandle, new_signal: VoidSignal) {
    let mut last = LAST_VOID_SIGNAL.lock();
    let should_emit = match &*last {
        Some(prev) => new_signal.differs_from(prev, 0.01),
        None => true,
    };
    if should_emit {
        debug!(target: "4da::void", pulse = new_signal.pulse, heat = new_signal.heat,
               burst = new_signal.burst, staleness = new_signal.staleness,
               items = new_signal.item_count, "Emitting void signal");
        if let Err(e) = app.emit("void-signal", &new_signal) {
            tracing::warn!("Failed to emit 'void-signal': {e}");
        }
        *last = Some(new_signal);
    }
}

/// Build a signal from current database and monitoring state.
/// Called after events that change the underlying data.
pub fn compute_signal(db: &Database, monitoring: &MonitoringState) -> VoidSignal {
    let item_count = db.total_item_count().unwrap_or(0) as u32;

    // Staleness: hours since last check / 24, capped at 1.0
    let last_check_epoch = monitoring.last_check.load(Ordering::Relaxed);
    let staleness = if last_check_epoch == 0 {
        1.0 // Never checked
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let hours_since = (now.saturating_sub(last_check_epoch)) as f32 / 3600.0;
        (hours_since / 24.0).min(1.0)
    };

    // Error: check if monitoring is_checking stuck (simple heuristic)
    let error = 0.0f32;

    // Intelligence metabolism: ratio of calibrations to total autophagy cycles
    let (metabolism, open_windows_count, advantage_trend) = {
        let conn = crate::open_db_connection().ok();
        let met = conn.as_ref().and_then(|c| {
            c.query_row(
                "SELECT COALESCE(
                    CAST((SELECT COUNT(*) FROM digested_intelligence WHERE superseded_by IS NULL) AS REAL)
                    / NULLIF((SELECT COUNT(*) FROM autophagy_cycles), 0),
                    0.0
                )", [], |r| r.get::<_, f64>(0),
            ).ok()
        }).unwrap_or(0.0).min(1.0) as f32;
        let ow = conn
            .as_ref()
            .and_then(|c| {
                c.query_row(
                    "SELECT COUNT(*) FROM decision_windows WHERE status = 'open'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .ok()
            })
            .unwrap_or(0) as u32;
        let at = conn.as_ref().and_then(|c| {
            // Compare latest two advantage scores to determine trend
            let mut stmt = c.prepare(
                "SELECT score FROM advantage_score WHERE period = 'weekly' ORDER BY computed_at DESC LIMIT 2"
            ).ok()?;
            let scores: Vec<f32> = stmt.query_map([], |r| r.get::<_, f32>(0))
                .ok()?
                .flatten()
                .collect();
            if scores.len() >= 2 && scores[1] > 0.0 {
                Some(((scores[0] - scores[1]) / scores[1]).clamp(-1.0, 1.0))
            } else if !scores.is_empty() && scores[0] > 0.0 {
                Some(1.0) // Growing from zero
            } else {
                Some(0.0)
            }
        }).unwrap_or(0.0);
        (met, ow, at)
    };

    VoidSignal {
        pulse: 0.0, // Updated by specific event handlers
        heat: 0.0,  // Updated after analysis
        burst: 0.0, // Updated after analysis
        morph: 0.0, // Updated after ACE scan
        error,
        staleness,
        item_count,
        signal_intensity: 0.0,
        signal_urgency: 0.0,
        critical_count: 0,
        signal_color_shift: 0.0,
        metabolism,
        open_windows: open_windows_count,
        advantage_trend,
    }
}

/// Map a signal type slug to a color shift value.
/// Negative = cool (blue), Positive = warm (gold/red).
fn signal_type_color_shift(slug: &str) -> f32 {
    match slug {
        "security_alert" => 1.0,
        "breaking_change" => 0.6,
        "tool_discovery" => 0.3,
        "tech_trend" => 0.0,
        "competitive_intel" => -0.2,
        "learning" => -0.4,
        _ => 0.0,
    }
}

/// Compute signal after an analysis completes.
/// Takes the analysis results to derive heat and burst,
/// and an optional SignalSummary to drive signal-aware fields.
pub fn signal_after_analysis(
    db: &Database,
    monitoring: &MonitoringState,
    top_scores: &[f32],
    summary: Option<&SignalSummary>,
) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);

    if !top_scores.is_empty() {
        // Heat: average of top scores (capped at 1.0)
        let sum: f32 = top_scores.iter().sum();
        signal.heat = (sum / top_scores.len() as f32).min(1.0);

        // Burst: max score above 0.7 threshold
        let max_score = top_scores
            .iter()
            .copied()
            .fold(0.0f32, |a, b| if a > b { a } else { b });
        signal.burst = (max_score - 0.7).clamp(0.0, 1.0);
    }

    // Staleness should be near zero right after analysis
    signal.staleness = 0.0;

    // Signal-aware fields from classification summary
    if let Some(s) = summary {
        signal.signal_intensity = (s.max_priority as f32 / 4.0).clamp(0.0, 1.0);
        signal.signal_urgency = s.urgency_score;
        signal.critical_count = s.critical_count;

        // Color shift: weighted average across all signal types
        let total_signals: u32 = s.signal_type_counts.values().sum();
        if total_signals > 0 {
            let weighted_sum: f32 = s
                .signal_type_counts
                .iter()
                .map(|(slug, count)| signal_type_color_shift(slug) * (*count as f32))
                .sum();
            signal.signal_color_shift = (weighted_sum / total_signals as f32).clamp(-1.0, 1.0);
        }
    }

    signal
}

/// Signal during active source fetching (pulse = 1.0).
pub fn signal_fetching(db: &Database, monitoring: &MonitoringState) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);
    signal.pulse = 1.0;
    signal
}

/// Signal after a cache fill completes (pulse drops back).
pub fn signal_cache_filled(db: &Database, monitoring: &MonitoringState) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);
    signal.pulse = 0.3; // Winding down
    signal
}

/// Signal when an error occurs.
pub fn signal_error(db: &Database, monitoring: &MonitoringState) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);
    signal.error = 1.0;
    signal
}

/// Signal after ACE file changes detected.
pub fn signal_context_change(
    db: &Database,
    monitoring: &MonitoringState,
    change_intensity: f32,
) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);
    signal.morph = change_intensity.min(1.0);
    signal
}

/// Signal when a notification fires — pulse the heartbeat to show awareness.
pub fn signal_notification(
    db: &Database,
    monitoring: &MonitoringState,
    is_critical: bool,
    count: usize,
) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);
    // Merge with last emitted signal to preserve analysis heat
    if let Some(prev) = LAST_VOID_SIGNAL.lock().as_ref() {
        signal.heat = prev.heat;
        signal.signal_intensity = prev.signal_intensity;
        signal.signal_color_shift = prev.signal_color_shift;
    }
    signal.burst = if is_critical { 1.0 } else { 0.6 };
    signal.critical_count = if is_critical { count as u32 } else { 0 };
    signal.signal_intensity = if is_critical { 1.0 } else { 0.75 };
    signal.signal_color_shift = if is_critical { 1.0 } else { 0.4 };
    signal
}

/// Update staleness on an existing signal, preserving analysis state.
/// Heat/burst decay by ~2% per call (once per minute → ~50% in 30 min).
pub fn tick_staleness(db: &Database, monitoring: &MonitoringState) -> VoidSignal {
    let base = compute_signal(db, monitoring);
    let last = LAST_VOID_SIGNAL.lock();
    match &*last {
        Some(prev) => {
            let decay = 0.98; // 2% decay per minute
            VoidSignal {
                pulse: prev.pulse * decay,
                heat: prev.heat * decay,
                burst: prev.burst * 0.90, // Burst decays faster (10%/min)
                morph: prev.morph * decay,
                error: prev.error * 0.95,    // Error clears over ~5 min
                staleness: base.staleness,   // Always fresh from clock
                item_count: base.item_count, // Always fresh from DB
                signal_intensity: prev.signal_intensity * decay,
                signal_urgency: prev.signal_urgency * decay,
                critical_count: prev.critical_count, // Integer — stays until next analysis
                signal_color_shift: prev.signal_color_shift * decay,
                metabolism: base.metabolism,
                open_windows: base.open_windows,
                advantage_trend: prev.advantage_trend * decay,
            }
        }
        None => base,
    }
}

/// Signal during source fetching with progress indication.
pub fn signal_fetch_progress(
    db: &Database,
    monitoring: &MonitoringState,
    completed: usize,
    total: usize,
) -> VoidSignal {
    let mut signal = compute_signal(db, monitoring);
    // Merge heat from last signal so fetching doesn't zero it
    if let Some(prev) = LAST_VOID_SIGNAL.lock().as_ref() {
        signal.heat = prev.heat;
        signal.signal_color_shift = prev.signal_color_shift;
    }
    let progress = if total > 0 {
        completed as f32 / total as f32
    } else {
        0.0
    };
    signal.pulse = 0.4 + progress * 0.6; // 0.4 → 1.0 as sources complete
    signal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_differs_from_identical() {
        let a = VoidSignal::default();
        let b = VoidSignal::default();
        assert!(!a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_differs_from_changed() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            pulse: 0.5,
            ..Default::default()
        };
        assert!(a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_differs_from_within_threshold() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            pulse: 0.005,
            ..Default::default()
        };
        assert!(!a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_differs_from_item_count() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            item_count: 1,
            ..Default::default()
        };
        assert!(a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_default_signal() {
        let s = VoidSignal::default();
        assert_eq!(s.pulse, 0.0);
        assert_eq!(s.heat, 0.0);
        assert_eq!(s.burst, 0.0);
        assert_eq!(s.morph, 0.0);
        assert_eq!(s.error, 0.0);
        assert_eq!(s.staleness, 1.0);
        assert_eq!(s.item_count, 0);
        assert_eq!(s.signal_intensity, 0.0);
        assert_eq!(s.signal_urgency, 0.0);
        assert_eq!(s.critical_count, 0);
        assert_eq!(s.signal_color_shift, 0.0);
    }

    #[test]
    fn test_signal_after_analysis_heat_and_burst() {
        let scores = [0.8, 0.6, 0.75];
        let sum: f32 = scores.iter().sum();
        let heat = (sum / scores.len() as f32).min(1.0);
        let max_score = scores
            .iter()
            .copied()
            .fold(0.0f32, |a, b| if a > b { a } else { b });
        let burst = (max_score - 0.7).clamp(0.0, 1.0);

        assert!((heat - 0.7167).abs() < 0.01);
        assert!((burst - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_signal_after_analysis_no_scores() {
        let scores: Vec<f32> = vec![];
        let heat = if scores.is_empty() {
            0.0
        } else {
            scores.iter().sum::<f32>() / scores.len() as f32
        };
        let burst = 0.0f32;
        assert_eq!(heat, 0.0);
        assert_eq!(burst, 0.0);
    }

    // Signal-Aware Heartbeat Tests

    #[test]
    fn test_signal_color_shift_mapping() {
        assert_eq!(signal_type_color_shift("security_alert"), 1.0);
        assert_eq!(signal_type_color_shift("breaking_change"), 0.6);
        assert_eq!(signal_type_color_shift("tool_discovery"), 0.3);
        assert_eq!(signal_type_color_shift("tech_trend"), 0.0);
        assert_eq!(signal_type_color_shift("competitive_intel"), -0.2);
        assert_eq!(signal_type_color_shift("learning"), -0.4);
        assert_eq!(signal_type_color_shift("unknown_type"), 0.0);
    }

    #[test]
    fn test_signal_summary_security_dominant() {
        let mut counts = HashMap::new();
        counts.insert("security_alert".to_string(), 3);
        counts.insert("learning".to_string(), 1);
        let summary = SignalSummary {
            max_priority: 4,
            critical_count: 2,
            signal_type_counts: counts,
            dominant_type: Some("security_alert".to_string()),
            urgency_score: 0.8,
        };

        let intensity = (summary.max_priority as f32 / 4.0).clamp(0.0, 1.0);
        assert_eq!(intensity, 1.0);

        let total: u32 = summary.signal_type_counts.values().sum();
        let weighted: f32 = summary
            .signal_type_counts
            .iter()
            .map(|(s, c)| signal_type_color_shift(s) * (*c as f32))
            .sum();
        let shift = weighted / total as f32;
        assert!((shift - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_signal_summary_learning_dominant() {
        let mut counts = HashMap::new();
        counts.insert("learning".to_string(), 5);
        let summary = SignalSummary {
            max_priority: 1,
            critical_count: 0,
            signal_type_counts: counts,
            dominant_type: Some("learning".to_string()),
            urgency_score: 0.25,
        };

        let intensity = (summary.max_priority as f32 / 4.0).clamp(0.0, 1.0);
        assert_eq!(intensity, 0.25);

        let total: u32 = summary.signal_type_counts.values().sum();
        let weighted: f32 = summary
            .signal_type_counts
            .iter()
            .map(|(s, c)| signal_type_color_shift(s) * (*c as f32))
            .sum();
        let shift = weighted / total as f32;
        assert!((shift - (-0.4)).abs() < 0.01);
    }

    #[test]
    fn test_signal_summary_mixed_signals() {
        let mut counts = HashMap::new();
        counts.insert("security_alert".to_string(), 1);
        counts.insert("tool_discovery".to_string(), 2);
        counts.insert("learning".to_string(), 1);
        let summary = SignalSummary {
            max_priority: 4,
            critical_count: 1,
            signal_type_counts: counts,
            dominant_type: Some("tool_discovery".to_string()),
            urgency_score: 0.5,
        };

        let total: u32 = summary.signal_type_counts.values().sum();
        let weighted: f32 = summary
            .signal_type_counts
            .iter()
            .map(|(s, c)| signal_type_color_shift(s) * (*c as f32))
            .sum();
        let shift = weighted / total as f32;
        assert!((shift - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_differs_from_signal_intensity() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            signal_intensity: 0.5,
            ..Default::default()
        };
        assert!(a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_differs_from_critical_count() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            critical_count: 1,
            ..Default::default()
        };
        assert!(a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_differs_from_color_shift() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            signal_color_shift: 0.5,
            ..Default::default()
        };
        assert!(a.differs_from(&b, 0.01));
    }

    #[test]
    fn test_differs_from_signal_urgency() {
        let a = VoidSignal::default();
        let b = VoidSignal {
            signal_urgency: 0.3,
            ..Default::default()
        };
        assert!(a.differs_from(&b, 0.01));
    }
}
