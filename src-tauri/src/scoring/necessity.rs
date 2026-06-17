// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
// REMOVE BY 2026-08-01
#[allow(dead_code)] // Pipeline struct field — populated but not read in current scoring
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
    /// Which skill gaps were matched (dep names)
    pub matched_skill_gaps: Vec<String>,
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
    /// A new release / update of something in your stack (see try_stack_update_path)
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
        } else if let Some(result) = try_stack_update_path(inputs, has_dep_match, &dep_names) {
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

/// Map a CVSS base score to a severity bucket. Mirrors the thresholds used by
/// `extract_cvss_from_content` in the pipeline so the numeric fallback agrees with
/// the string severity when both are present.
fn severity_from_cvss(score: f32) -> String {
    if score >= 9.0 {
        "critical"
    } else if score >= 7.0 {
        "high"
    } else if score >= 4.0 {
        "medium"
    } else {
        "low"
    }
    .to_string()
}

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

    // Severity precedence: explicit CVE severity > classifier signal priority >
    // CVSS base score (numeric fallback) > "medium". The CVSS fallback closes bug J:
    // a real critical CVE that reaches this path with no priority signal (e.g. a
    // dev-only dependency that didn't trip the classifier) was scored "medium" 0.60
    // even when the advisory carried CVSS 9.8.
    let severity: String = inputs
        .cve_severity
        .as_deref()
        .or(inputs.signal_priority.as_deref())
        .map(str::to_string)
        .or_else(|| inputs.cvss_score.map(severity_from_cvss))
        .unwrap_or_else(|| "medium".to_string());

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
        .is_some_and(|s| s == "breaking_change")
        || inputs
            .content_type
            .as_deref()
            .is_some_and(|ct| ct == "breaking_change");

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

/// Stack-update path — a new release / platform update of something in the user's
/// actual dependency graph. This is genuinely actionable ("what you NEED"): review
/// the changelog, check for breaking changes, decide whether to upgrade. Before this
/// path existed, a release of your own dependency (e.g. `crates.io: axum v0.8.9`) fell
/// through to the generic blind-spot path and was recency-decayed into invisibility,
/// so a dev's own stack updates never surfaced above unrelated security/blind-spot noise.
///
/// Gated on a real dependency match, so it only fires for YOUR stack — never generic
/// topical content (that stays low, preserving the necessity-over-want doctrine).
fn try_stack_update_path(
    inputs: &NecessityInputs,
    has_dep_match: bool,
    dep_names: &str,
) -> Option<(f32, String, NecessityCategory, Urgency)> {
    let is_update = inputs
        .content_type
        .as_deref()
        .is_some_and(|ct| ct == "release_notes" || ct == "platform_update");

    if !is_update || !has_dep_match {
        return None;
    }

    // Scaled by match strength: a strong direct-dep match scores higher. Bounded
    // below the security/breaking tiers (a release is awareness-actionable, not urgent)
    // but well above a recency-decayed blind-spot, so your stack's releases surface.
    let score = (0.45 + inputs.dep_match_score * 0.20).min(0.65);
    Some((
        score,
        format!("New release in your stack: {dep_names}"),
        NecessityCategory::EcosystemShift,
        Urgency::Awareness,
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
    let reason = if inputs.matched_skill_gaps.is_empty() {
        "Covers a gap in your technology profile".to_string()
    } else {
        format!(
            "You use {} but haven't engaged with recent updates",
            inputs.matched_skill_gaps.join(", ")
        )
    };
    Some((
        score,
        reason,
        NecessityCategory::BlindSpot,
        Urgency::Awareness,
    ))
}

#[cfg(test)]
#[path = "necessity_tests.rs"]
mod tests;
