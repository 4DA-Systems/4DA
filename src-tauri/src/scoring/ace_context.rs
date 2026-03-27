use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

use super::dependencies::{load_dependency_intelligence, DepInfo};
use crate::get_ace_engine;

/// ACE-discovered context for relevance scoring
/// PASIFA: Full context including confidence scores for weighted scoring
#[derive(Debug, Default, Clone)]
pub(crate) struct ACEContext {
    /// Active topics detected from project manifests and git history
    pub active_topics: Vec<String>,
    /// Confidence scores for active topics (topic -> confidence 0.0-1.0)
    pub topic_confidence: std::collections::HashMap<String, f32>,
    /// Detected tech stack (languages, frameworks)
    pub detected_tech: Vec<String>,
    /// Anti-topics (topics user has consistently rejected)
    pub anti_topics: Vec<String>,
    /// Confidence scores for anti-topics (topic -> confidence 0.0-1.0)
    pub anti_topic_confidence: std::collections::HashMap<String, f32>,
    /// Topic affinities from behavior learning (topic -> (affinity_score, confidence))
    /// PASIFA: Now includes BOTH positive AND negative affinities with confidence
    pub topic_affinities: std::collections::HashMap<String, (f32, f32)>,
    /// Normalized dependency package names for O(1) lookup
    pub dependency_names: HashSet<String>,
    /// Dependency details: normalized_name -> info (version, language, search terms)
    pub dependency_info: HashMap<String, DepInfo>,
    /// Peak commit hours (0-23) from git analysis, sorted by frequency (most active first).
    /// Used to give a slight freshness boost to content published during active coding hours.
    pub peak_hours: Vec<u8>,
    /// Per-tech scoring weight based on project source.
    /// Primary project tech (same dir as CWD) → 0.85, secondary → 0.40.
    /// Used by semantic scoring instead of flat 0.6 for all detected tech.
    pub tech_weights: HashMap<String, f32>,
}

/// Fetch ACE-discovered context for relevance scoring
/// PASIFA: Now captures full context including confidence scores
pub(crate) fn get_ace_context() -> ACEContext {
    let ace = match get_ace_engine() {
        Ok(engine) => engine,
        Err(e) => {
            warn!(target: "4da::ace", error = %e, "ACE engine unavailable - using empty context");
            return ACEContext::default();
        }
    };

    let mut ctx = ACEContext::default();

    // Get active topics WITH confidence scores
    if let Ok(topics) = ace.get_active_topics() {
        for t in topics.iter().filter(|t| t.weight >= 0.55) {
            let topic_lower = t.topic.to_lowercase();
            ctx.active_topics.push(topic_lower.clone());
            ctx.topic_confidence.insert(topic_lower, t.confidence);
        }
    }

    // Get detected tech — filter to meaningful categories with decent confidence.
    // Exclude Platform (e.g. "windows", "macos", "linux") — developing ON a platform
    // doesn't mean the user is interested in content ABOUT that platform.
    if let Ok(tech) = ace.get_detected_tech() {
        // Determine primary project directory (CWD or first context_dir)
        let primary_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let filtered: Vec<_> = tech
            .iter()
            .filter(|t| {
                matches!(
                    t.category,
                    crate::ace::TechCategory::Language
                        | crate::ace::TechCategory::Framework
                        | crate::ace::TechCategory::Database
                ) && t.confidence >= 0.5
            })
            .take(20)
            .collect();

        for t in &filtered {
            let name_lower = t.name.to_lowercase();
            ctx.detected_tech.push(name_lower.clone());

            // Compute per-tech weight from evidence path (primary vs secondary project)
            let is_primary = t.evidence.iter().any(|ev| {
                let ev_lower = ev.to_lowercase().replace('\\', "/");
                let primary_normalized = primary_dir.replace('\\', "/");
                ev_lower.contains(&primary_normalized)
            });
            let weight: f32 = if is_primary { 0.85 } else { 0.40 };
            // If tech already has a weight (from another project), take the max (primary wins)
            let existing = ctx.tech_weights.get(&name_lower).copied().unwrap_or(0.0_f32);
            ctx.tech_weights.insert(name_lower, weight.max(existing));
        }
    }

    // Get anti-topics WITH confidence scores
    if let Ok(anti_topics) = ace.get_anti_topics(3) {
        for a in anti_topics
            .iter()
            .filter(|a| a.user_confirmed || a.confidence >= 0.5)
        {
            let topic_lower = a.topic.to_lowercase();
            ctx.anti_topics.push(topic_lower.clone());
            ctx.anti_topic_confidence.insert(topic_lower, a.confidence);
        }
    }

    // Get topic affinities - BOTH positive AND negative
    // PASIFA: Negative affinities are valuable learned signals with confidence
    if let Ok(affinities) = ace.get_topic_affinities() {
        for aff in affinities {
            // Include affinities with enough data, regardless of sign
            if aff.total_exposures >= 3 && aff.affinity_score.abs() > 0.1 {
                ctx.topic_affinities.insert(
                    aff.topic.to_lowercase(),
                    (aff.affinity_score, aff.confidence),
                );
            }
        }
    }

    // Merge recent work topics (last 2 hours) with high confidence.
    // These represent what the user is actively working on RIGHT NOW,
    // so they get elevated confidence to boost related content.
    if let Ok(work_topics) = ace.get_recent_work_topics(2) {
        for (topic, weight) in work_topics {
            if !ctx.active_topics.contains(&topic) {
                ctx.active_topics.push(topic.clone());
            }
            // Recent work topics get high confidence (0.85-0.95 scaled by recency)
            // weight ranges 0.5-1.0, maps to confidence 0.85-0.95
            let work_confidence = 0.85 + (weight - 0.5) * 0.2;
            let existing = ctx.topic_confidence.get(&topic).copied().unwrap_or(0.0);
            ctx.topic_confidence
                .insert(topic, existing.max(work_confidence));
        }
        debug!(target: "4da::ace", "Merged recent work topics into ACE context");
    }

    // Load dependency intelligence from project_dependencies table
    let (dep_names, dep_info) = load_dependency_intelligence();
    if !dep_names.is_empty() {
        debug!(target: "4da::ace",
            packages = dep_info.len(),
            search_terms = dep_names.len(),
            "Dependency intelligence loaded for scoring"
        );
    }
    ctx.dependency_names = dep_names;
    ctx.dependency_info = dep_info;

    // Load peak commit hours from ACE engine (populated during full scan)
    ctx.peak_hours = ace.peak_hours.clone();

    ctx
}

/// Check if item should be excluded by ACE anti-topics
pub(crate) fn check_ace_exclusions(topics: &[String], ace_ctx: &ACEContext) -> Option<String> {
    // Both topics (from extract_topics) and anti_topics are already lowercase
    for topic in topics {
        for anti_topic in &ace_ctx.anti_topics {
            if topic.contains(anti_topic.as_str()) || anti_topic.contains(topic.as_str()) {
                return Some(format!("ACE anti-topic: {anti_topic}"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ace_context_default() {
        let ctx = ACEContext::default();
        assert!(ctx.active_topics.is_empty());
        assert!(ctx.detected_tech.is_empty());
        assert!(ctx.anti_topics.is_empty());
        assert!(ctx.topic_affinities.is_empty());
    }

    #[test]
    fn test_check_ace_exclusions_no_anti_topics() {
        let ctx = ACEContext::default();
        let topics = vec!["rust".to_string(), "tauri".to_string()];
        assert!(check_ace_exclusions(&topics, &ctx).is_none());
    }

    #[test]
    fn test_check_ace_exclusions_match() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("crypto".to_string());
        let topics = vec!["cryptocurrency".to_string()];
        let result = check_ace_exclusions(&topics, &ctx);
        assert!(result.is_some());
        assert!(result.unwrap().contains("crypto"));
    }

    #[test]
    fn test_check_ace_exclusions_reverse_match() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("cryptocurrency".to_string());
        let topics = vec!["crypto".to_string()];
        let result = check_ace_exclusions(&topics, &ctx);
        assert!(result.is_some());
    }

    #[test]
    fn test_check_ace_exclusions_no_match() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("crypto".to_string());
        let topics = vec!["rust".to_string(), "tauri".to_string()];
        assert!(check_ace_exclusions(&topics, &ctx).is_none());
    }

    #[test]
    fn test_check_ace_exclusions_empty_topics() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("crypto".to_string());
        let topics: Vec<String> = vec![];
        assert!(check_ace_exclusions(&topics, &ctx).is_none());
    }

    #[test]
    fn test_check_ace_exclusions_multiple_anti_topics() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("crypto".to_string());
        ctx.anti_topics.push("nft".to_string());
        let topics = vec!["nft".to_string()];
        let result = check_ace_exclusions(&topics, &ctx);
        assert!(result.is_some());
    }

    #[test]
    fn test_ace_context_dependency_names_default() {
        let ctx = ACEContext::default();
        assert!(ctx.dependency_names.is_empty());
        assert!(ctx.dependency_info.is_empty());
    }

    #[test]
    fn test_ace_context_anti_topic_confidence_default() {
        let ctx = ACEContext::default();
        assert!(ctx.anti_topic_confidence.is_empty());
    }
}
