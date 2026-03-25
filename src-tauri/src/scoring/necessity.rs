//! Necessity scoring — "what you'd regret missing"
//!
//! Computes a 0.0-1.0 necessity score from the intersection of:
//! - User's dependency graph (what they have)
//! - Content signals (what's happening)
//! - Temporal urgency (when it matters)
//! - Blind spot relevance (what they're missing)
//!
//! This is ADDITIVE to the existing PASIFA pipeline — it does not replace
//! any existing scoring. The necessity score is stored alongside other
//! breakdown fields for downstream consumers (UI, agent briefs, etc.).

use tracing::debug;

// ============================================================================
// Types
// ============================================================================

/// Inputs gathered from existing scoring signals.
/// All fields are populated from values already computed in the pipeline.
#[allow(dead_code)] // cvss_score reserved for future CVE enrichment
pub(crate) struct NecessityInputs {
    /// Does this item match a user dependency? (from dep_match_score)
    pub dep_match_score: f32,
    /// Matched dependency names (from matched_deps)
    pub matched_deps: Vec<String>,
    /// Content signal type (security_alert, breaking_change, etc.)
    pub signal_type: Option<String>,
    /// Content signal priority (critical, high, medium)
    pub signal_priority: Option<String>,
    /// CVE severity if applicable (CRITICAL, HIGH, MEDIUM, LOW)
    pub cve_severity: Option<String>,
    /// CVSS score if applicable (0.0-10.0)
    pub cvss_score: Option<f32>,
    /// Number of user projects affected by matched deps
    pub affected_project_count: usize,
    /// Skill gap boost from sovereign profile
    pub skill_gap_boost: f32,
    /// Decision window boost (active decision affected)
    pub window_boost: f32,
    /// Hours since content was published
    pub age_hours: f64,
    /// Content type classification
    pub content_type: Option<String>,
    /// Contradiction boost: how much this item overlaps with contradicted topics (0.0-1.0)
    pub contradiction_boost: f32,
}

/// Result of necessity computation
pub(crate) struct NecessityResult {
    /// 0.0-1.0 necessity score
    pub score: f32,
    /// One-line human-readable explanation
    pub reason: String,
    /// What category of necessity this falls into
    pub category: NecessityCategory,
    /// How urgently the developer should act
    pub urgency: Urgency,
}

/// Classification of why this content is necessary
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NecessityCategory {
    /// CVE affecting your deps
    SecurityVulnerability,
    /// Breaking change in your stack
    BreakingChange,
    /// Something you use is being deprecated
    DeprecationNotice,
    /// Important topic you're not tracking
    BlindSpot,
    /// Affects an active architectural decision
    DecisionRelevant,
    /// Major shift in your primary stack (reserved for ecosystem_shift_mult integration)
    #[allow(dead_code)]
    EcosystemShift,
    /// Not a necessity item
    None,
}

/// How urgently the developer should respond
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Urgency {
    /// Act today (critical CVE, imminent breaking change)
    Immediate,
    /// Address soon (high CVE, deprecation with timeline)
    ThisWeek,
    /// Good to know (trend shifts, blind spots)
    Awareness,
    /// No urgency
    None,
}

impl NecessityCategory {
    /// Stable string slug for serialization
    pub fn slug(&self) -> &'static str {
        match self {
            Self::SecurityVulnerability => "security_vulnerability",
            Self::BreakingChange => "breaking_change",
            Self::DeprecationNotice => "deprecation_notice",
            Self::BlindSpot => "blind_spot",
            Self::DecisionRelevant => "decision_relevant",
            Self::EcosystemShift => "ecosystem_shift",
            Self::None => "none",
        }
    }
}

impl Urgency {
    /// Stable string label for serialization
    pub fn label(&self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::ThisWeek => "this_week",
            Self::Awareness => "awareness",
            Self::None => "none",
        }
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Maximum multi-project amplification factor
const MAX_PROJECT_AMPLIFICATION: f32 = 1.5;
/// Recency decay half-life in hours (1 week)
const RECENCY_HALF_LIFE_HOURS: f64 = 168.0;
/// Minimum recency multiplier (floor at 50%)
const RECENCY_FLOOR: f64 = 0.5;

// ============================================================================
// Core computation
// ============================================================================

/// Compute necessity score from pre-existing pipeline signals.
///
/// The score answers: "How much would this developer regret not knowing about this?"
///
/// Scoring paths (evaluated in priority order, first match wins):
/// 1. Security CVE with/without dep match
/// 2. Breaking change with/without dep match
/// 3. Deprecation notice with dep match
/// 4. Contradiction resolution (content touches a topic with conflicting signals)
/// 5. Decision-relevant content
/// 6. Blind spot / skill gap content
///
/// Post-processing:
/// - Multi-project amplification (more projects affected = higher score)
/// - Recency decay for non-security items (1-week half-life)
/// - Final clamp to 0.0-1.0
pub(crate) fn compute_necessity(inputs: &NecessityInputs) -> NecessityResult {
    let has_dep_match = inputs.dep_match_score > 0.0;
    let dep_names = if inputs.matched_deps.is_empty() {
        "your stack".to_string()
    } else {
        inputs.matched_deps.join(", ")
    };

    // Evaluate paths in priority order
    let (mut score, reason, category, urgency) =
        if let Some(result) = try_security_path(inputs, has_dep_match, &dep_names) {
            result
        } else if let Some(result) = try_breaking_change_path(inputs, has_dep_match, &dep_names) {
            result
        } else if let Some(result) = try_deprecation_path(inputs, has_dep_match, &dep_names) {
            result
        } else if let Some(result) = try_contradiction_path(inputs) {
            result
        } else if let Some(result) = try_decision_relevant_path(inputs) {
            result
        } else if let Some(result) = try_blind_spot_path(inputs) {
            result
        } else {
            (0.0, String::new(), NecessityCategory::None, Urgency::None)
        };

    // Multi-project amplification
    if inputs.affected_project_count > 1 {
        let amplification = (1.0 + (inputs.affected_project_count as f32 - 1.0) * 0.1)
            .min(MAX_PROJECT_AMPLIFICATION);
        let prev = score;
        score *= amplification;
        debug!(
            target: "4da::necessity",
            projects = inputs.affected_project_count,
            amplification,
            "Multi-project amplification: {:.3} -> {:.3}",
            prev, score
        );
    }

    // Recency decay for non-security items
    if category != NecessityCategory::SecurityVulnerability && inputs.age_hours > 0.0 {
        let decay = (RECENCY_FLOOR as f32)
            .max((1.0 - inputs.age_hours as f32 / RECENCY_HALF_LIFE_HOURS as f32).max(0.0));
        let prev = score;
        score *= decay;
        debug!(
            target: "4da::necessity",
            age_hours = inputs.age_hours,
            decay,
            "Recency decay: {:.3} -> {:.3}",
            prev, score
        );
    }

    // Final clamp
    score = score.clamp(0.0, 1.0);

    debug!(
        target: "4da::necessity",
        score,
        category = category.slug(),
        urgency = urgency.label(),
        reason = %reason,
        "Necessity computed"
    );

    NecessityResult {
        score,
        reason,
        category,
        urgency,
    }
}

// ============================================================================
// Path evaluators
// ============================================================================

/// Security CVE path (highest necessity).
fn try_security_path(
    inputs: &NecessityInputs,
    has_dep_match: bool,
    dep_names: &str,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    let is_security = inputs
        .signal_type
        .as_deref()
        .is_some_and(|s| s == "security_alert")
        || inputs.cve_severity.is_some()
        || inputs
            .content_type
            .as_deref()
            .is_some_and(|ct| ct == "security_advisory");

    if !is_security {
        return None;
    }

    let severity = inputs
        .cve_severity
        .as_deref()
        .or(inputs.signal_priority.as_deref())
        .unwrap_or("medium");

    if has_dep_match {
        let (score, urgency) = match severity.to_lowercase().as_str() {
            "critical" => (0.95, Urgency::Immediate),
            "high" => (0.85, Urgency::ThisWeek),
            "medium" => (0.60, Urgency::Awareness),
            _ => (0.50, Urgency::Awareness),
        };
        Some((
            score,
            format!("Security vulnerability affects {dep_names}"),
            NecessityCategory::SecurityVulnerability,
            urgency,
        ))
    } else {
        // General security awareness without dep match
        let score = match severity.to_lowercase().as_str() {
            "critical" => 0.40,
            "high" => 0.30,
            _ => 0.20,
        };
        Some((
            score,
            "Security advisory in your ecosystem".to_string(),
            NecessityCategory::SecurityVulnerability,
            Urgency::Awareness,
        ))
    }
}

/// Breaking change path.
fn try_breaking_change_path(
    inputs: &NecessityInputs,
    has_dep_match: bool,
    dep_names: &str,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    let is_breaking = inputs
        .signal_type
        .as_deref()
        .is_some_and(|s| s == "breaking_change");

    if !is_breaking {
        return None;
    }

    if has_dep_match {
        Some((
            0.80,
            format!("Breaking change affects {dep_names}"),
            NecessityCategory::BreakingChange,
            Urgency::ThisWeek,
        ))
    } else {
        Some((
            0.25,
            "Breaking change in related technology".to_string(),
            NecessityCategory::BreakingChange,
            Urgency::Awareness,
        ))
    }
}

/// Deprecation path — requires dep match for meaningful score.
fn try_deprecation_path(
    inputs: &NecessityInputs,
    has_dep_match: bool,
    dep_names: &str,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    // Detect deprecation from content type or signal type
    let is_deprecation = inputs
        .content_type
        .as_deref()
        .is_some_and(|ct| ct.contains("deprecat"))
        || inputs
            .signal_type
            .as_deref()
            .is_some_and(|s| s.contains("deprecat"));

    if !is_deprecation || !has_dep_match {
        return None;
    }

    Some((
        0.65,
        format!("Deprecation notice affects {dep_names}"),
        NecessityCategory::DeprecationNotice,
        Urgency::ThisWeek,
    ))
}

/// Contradiction resolution path — content touches a topic the user has conflicting signals about.
///
/// When the user both likes and dislikes a topic (high affinity AND anti-topic), content
/// that could resolve the confusion is moderately necessary. Scored between deprecation
/// and decision-relevant because resolving confusion has lasting value.
fn try_contradiction_path(
    inputs: &NecessityInputs,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    if inputs.contradiction_boost <= 0.0 {
        return None;
    }

    // Base score 0.45, boosted by contradiction strength (max 0.70)
    let score = (0.45 + inputs.contradiction_boost * 0.25).min(0.70);
    Some((
        score,
        "Touches a topic with conflicting signals in your profile".to_string(),
        NecessityCategory::BlindSpot, // Contradictions are a form of blind spot
        Urgency::Awareness,
    ))
}

/// Decision-relevant path — content affects an active architectural decision.
fn try_decision_relevant_path(
    inputs: &NecessityInputs,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    if inputs.window_boost <= 0.10 {
        return None;
    }

    let score = 0.50 + inputs.window_boost;
    Some((
        score,
        "Relevant to an active architectural decision".to_string(),
        NecessityCategory::DecisionRelevant,
        Urgency::Awareness,
    ))
}

/// Blind spot / skill gap path — important topic user isn't tracking.
fn try_blind_spot_path(
    inputs: &NecessityInputs,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    if inputs.skill_gap_boost <= 0.10 {
        return None;
    }

    let score = 0.40 + inputs.skill_gap_boost;
    Some((
        score,
        "Covers a skill gap in your technology profile".to_string(),
        NecessityCategory::BlindSpot,
        Urgency::Awareness,
    ))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create default inputs with everything zeroed/empty.
    fn default_inputs() -> NecessityInputs {
        NecessityInputs {
            dep_match_score: 0.0,
            matched_deps: vec![],
            signal_type: None,
            signal_priority: None,
            cve_severity: None,
            cvss_score: None,
            affected_project_count: 0,
            skill_gap_boost: 0.0,
            window_boost: 0.0,
            age_hours: 0.0,
            content_type: None,
            contradiction_boost: 0.0,
        }
    }

    #[test]
    fn test_critical_cve_with_dep_match() {
        let inputs = NecessityInputs {
            dep_match_score: 0.7,
            matched_deps: vec!["lodash".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("CRITICAL".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.90,
            "Critical CVE + dep match should score > 0.90, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::SecurityVulnerability);
        assert_eq!(result.urgency, Urgency::Immediate);
        assert!(result.reason.contains("lodash"));
    }

    #[test]
    fn test_high_cve_without_dep_match() {
        let inputs = NecessityInputs {
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("HIGH".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score < 0.40,
            "High CVE without dep match should score < 0.40, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::SecurityVulnerability);
        assert_eq!(result.urgency, Urgency::Awareness);
    }

    #[test]
    fn test_high_cve_with_dep_match() {
        let inputs = NecessityInputs {
            dep_match_score: 0.5,
            matched_deps: vec!["serde".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("HIGH".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.80,
            "High CVE + dep match should score > 0.80, got {}",
            result.score
        );
        assert_eq!(result.urgency, Urgency::ThisWeek);
    }

    #[test]
    fn test_breaking_change_with_dep_match() {
        let inputs = NecessityInputs {
            dep_match_score: 0.6,
            matched_deps: vec!["react".to_string()],
            signal_type: Some("breaking_change".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.70,
            "Breaking change + dep match should score > 0.70, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::BreakingChange);
        assert_eq!(result.urgency, Urgency::ThisWeek);
    }

    #[test]
    fn test_breaking_change_without_dep_match() {
        let inputs = NecessityInputs {
            signal_type: Some("breaking_change".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score < 0.30,
            "Breaking change without dep match should score < 0.30, got {}",
            result.score
        );
    }

    #[test]
    fn test_blind_spot_boost() {
        let inputs = NecessityInputs {
            skill_gap_boost: 0.15,
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.40,
            "Blind spot with skill_gap 0.15 should score > 0.40, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::BlindSpot);
        assert_eq!(result.urgency, Urgency::Awareness);
    }

    #[test]
    fn test_decision_relevant() {
        let inputs = NecessityInputs {
            window_boost: 0.18,
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.60,
            "Decision-relevant with window_boost 0.18 should score > 0.60, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::DecisionRelevant);
    }

    #[test]
    fn test_multi_project_amplification() {
        let inputs = NecessityInputs {
            dep_match_score: 0.6,
            matched_deps: vec!["tokio".to_string()],
            signal_type: Some("breaking_change".to_string()),
            affected_project_count: 4,
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        // Base score 0.80 * amplification (1.0 + 3*0.1 = 1.3) = 1.04, clamped to 1.0
        assert!(
            result.score > 0.80,
            "Multi-project should amplify score above base 0.80, got {}",
            result.score
        );
    }

    #[test]
    fn test_recency_decay_non_security() {
        // Breaking change that is 5 days old
        let inputs = NecessityInputs {
            dep_match_score: 0.6,
            matched_deps: vec!["react".to_string()],
            signal_type: Some("breaking_change".to_string()),
            age_hours: 120.0, // 5 days
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        // Base 0.80 * decay max(0.5, 1.0 - 120/168) = 0.80 * 0.286 -> but floor at 0.5
        // So 0.80 * 0.5 = 0.40 approximately
        assert!(
            result.score < 0.80,
            "5-day-old breaking change should decay below 0.80, got {}",
            result.score
        );
        assert!(
            result.score >= 0.30,
            "Should not decay too aggressively, got {}",
            result.score
        );
    }

    #[test]
    fn test_security_no_recency_decay() {
        // Critical security item that is 5 days old — should NOT decay
        let inputs = NecessityInputs {
            dep_match_score: 0.7,
            matched_deps: vec!["lodash".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("CRITICAL".to_string()),
            age_hours: 120.0, // 5 days
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.90,
            "Security items should not decay with age, got {}",
            result.score
        );
    }

    #[test]
    fn test_no_necessity_item() {
        let inputs = default_inputs();
        let result = compute_necessity(&inputs);
        assert!(
            result.score < 0.01,
            "No-signal item should score near 0.0, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::None);
        assert_eq!(result.urgency, Urgency::None);
    }

    #[test]
    fn test_medium_cve_with_dep_match() {
        let inputs = NecessityInputs {
            dep_match_score: 0.4,
            matched_deps: vec!["express".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("MEDIUM".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score >= 0.55 && result.score <= 0.65,
            "Medium CVE + dep match should be ~0.60, got {}",
            result.score
        );
        assert_eq!(result.urgency, Urgency::Awareness);
    }

    #[test]
    fn test_multi_project_capped_amplification() {
        // 10 projects affected — amplification capped at 1.5x
        let inputs = NecessityInputs {
            dep_match_score: 0.6,
            matched_deps: vec!["tokio".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("HIGH".to_string()),
            affected_project_count: 10,
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        // Base 0.85 * 1.5 = 1.275, clamped to 1.0
        assert_eq!(
            result.score, 1.0,
            "10-project amplification on high CVE should cap at 1.0"
        );
    }

    #[test]
    fn test_skill_gap_too_low_no_match() {
        let inputs = NecessityInputs {
            skill_gap_boost: 0.05, // below 0.10 threshold
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score < 0.01,
            "Skill gap below threshold should not trigger, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::None);
    }

    #[test]
    fn test_window_boost_too_low_no_match() {
        let inputs = NecessityInputs {
            window_boost: 0.08, // below 0.10 threshold
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score < 0.01,
            "Window boost below threshold should not trigger, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::None);
    }

    #[test]
    fn test_deprecation_with_dep_match() {
        let inputs = NecessityInputs {
            dep_match_score: 0.5,
            matched_deps: vec!["moment".to_string()],
            signal_type: Some("deprecation".to_string()),
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.60,
            "Deprecation + dep match should score > 0.60, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::DeprecationNotice);
        assert_eq!(result.urgency, Urgency::ThisWeek);
    }

    #[test]
    fn test_security_takes_priority_over_breaking_change() {
        // Item classified as both security AND breaking — security path should win
        let inputs = NecessityInputs {
            dep_match_score: 0.5,
            matched_deps: vec!["openssl".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("CRITICAL".to_string()),
            window_boost: 0.15,    // also decision relevant
            skill_gap_boost: 0.15, // also blind spot
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert_eq!(
            result.category,
            NecessityCategory::SecurityVulnerability,
            "Security should take priority"
        );
        assert!(result.score > 0.90);
    }

    #[test]
    fn test_contradiction_boost_triggers() {
        let inputs = NecessityInputs {
            contradiction_boost: 0.5, // single topic match
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score > 0.40,
            "Contradiction with 0.5 boost should score > 0.40, got {}",
            result.score
        );
        assert_eq!(result.category, NecessityCategory::BlindSpot);
        assert_eq!(result.urgency, Urgency::Awareness);
        assert!(result.reason.contains("conflicting signals"));
    }

    #[test]
    fn test_contradiction_strong_boost() {
        let inputs = NecessityInputs {
            contradiction_boost: 1.0, // multiple topic matches
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score >= 0.65,
            "Strong contradiction should score >= 0.65, got {}",
            result.score
        );
    }

    #[test]
    fn test_contradiction_no_boost() {
        let inputs = NecessityInputs {
            contradiction_boost: 0.0,
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert!(
            result.score < 0.01,
            "No contradiction boost should not trigger, got {}",
            result.score
        );
    }

    #[test]
    fn test_security_takes_priority_over_contradiction() {
        let inputs = NecessityInputs {
            dep_match_score: 0.5,
            matched_deps: vec!["openssl".to_string()],
            signal_type: Some("security_alert".to_string()),
            cve_severity: Some("CRITICAL".to_string()),
            contradiction_boost: 1.0, // also has contradiction
            ..default_inputs()
        };
        let result = compute_necessity(&inputs);
        assert_eq!(
            result.category,
            NecessityCategory::SecurityVulnerability,
            "Security should take priority over contradiction"
        );
        assert!(result.score > 0.90);
    }
}
