// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
    /// Primary project tech (same dir as CWD) -> 0.85, secondary -> 0.40.
    /// Used by semantic scoring instead of flat 0.6 for all detected tech.
    pub tech_weights: HashMap<String, f32>,
    /// Negative stack: Bayesian priors for technologies the user likely does NOT use.
    /// Built from competing-tech inference + anti-topics. Applied undampened in scoring.
    pub negative_stack: crate::stacks::negative_stack::NegativeStackContext,
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
            let conf = if t.confidence.is_finite() && t.confidence >= 0.0 && t.confidence <= 1.0 {
                t.confidence
            } else {
                warn!(target: "4da::scoring", topic = %t.topic, raw = t.confidence, "Invalid ACE confidence — clamping to 0.5");
                0.5
            };
            ctx.topic_confidence.insert(topic_lower, conf);
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
            let existing = ctx
                .tech_weights
                .get(&name_lower)
                .copied()
                .unwrap_or(0.0_f32);
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
            let conf = if a.confidence.is_finite() && a.confidence >= 0.0 && a.confidence <= 1.0 {
                a.confidence
            } else {
                warn!(target: "4da::scoring", topic = %a.topic, raw = a.confidence, "Invalid ACE anti-topic confidence — clamping to 0.5");
                0.5
            };
            ctx.anti_topic_confidence.insert(topic_lower, conf);
        }
    }

    // Get topic affinities - BOTH positive AND negative
    // PASIFA: Negative affinities are valuable learned signals with confidence
    if let Ok(affinities) = ace.get_topic_affinities() {
        for aff in affinities {
            // Include affinities with enough data, regardless of sign
            if aff.total_exposures >= 3 && aff.affinity_score.abs() > 0.1 {
                let conf = if aff.confidence.is_finite()
                    && aff.confidence >= 0.0
                    && aff.confidence <= 1.0
                {
                    aff.confidence
                } else {
                    warn!(target: "4da::scoring", topic = %aff.topic, raw = aff.confidence, "Invalid ACE affinity confidence — clamping to 0.5");
                    0.5
                };
                ctx.topic_affinities
                    .insert(aff.topic.to_lowercase(), (aff.affinity_score, conf));
            }
        }
    }

    // Merge session-aware work topics with graduated confidence.
    // Uses gap-based session detection: current session gets highest confidence,
    // previous same-day session gets moderate, yesterday gets low.
    if let Ok(work_topics) = ace.get_session_aware_work_topics() {
        for (topic, weight) in work_topics {
            if !ctx.active_topics.contains(&topic) {
                ctx.active_topics.push(topic.clone());
            }
            // Session-aware weights map to confidence:
            // weight 1.0 (current session) -> confidence 0.95
            // weight 0.5 (previous session) -> confidence 0.85
            // weight 0.2 (yesterday) -> confidence 0.79
            let work_confidence = 0.75 + weight * 0.20;
            let existing = ctx.topic_confidence.get(&topic).copied().unwrap_or(0.0);
            ctx.topic_confidence
                .insert(topic, existing.max(work_confidence));
        }
        debug!(target: "4da::ace", "Merged session-aware work topics into ACE context");
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

/// Check if item should be excluded by ACE anti-topics.
/// Uses word-boundary matching to prevent "test" blocking "testing" or "contest".
pub(crate) fn check_ace_exclusions(topics: &[String], ace_ctx: &ACEContext) -> Option<String> {
    // Both topics (from extract_topics) and anti_topics are already lowercase
    for topic in topics {
        let topic_words: std::collections::HashSet<&str> = topic.split_whitespace().collect();
        for anti_topic in &ace_ctx.anti_topics {
            let anti_words: Vec<&str> = anti_topic.split_whitespace().collect();
            // All words in the anti-topic must appear as whole words in the topic
            let all_match = anti_words.iter().all(|aw| topic_words.contains(aw));
            if all_match && !anti_words.is_empty() {
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
        // "crypto" as a whole word in the topic should match
        let topics = vec!["crypto trading".to_string()];
        let result = check_ace_exclusions(&topics, &ctx);
        assert!(result.is_some());
        assert!(result.unwrap().contains("crypto"));
    }

    #[test]
    fn test_check_ace_exclusions_no_substring_match() {
        // "crypto" should NOT match "cryptocurrency" (word boundary enforcement)
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("crypto".to_string());
        let topics = vec!["cryptocurrency".to_string()];
        let result = check_ace_exclusions(&topics, &ctx);
        assert!(
            result.is_none(),
            "Substring 'crypto' should not match 'cryptocurrency'"
        );
    }

    #[test]
    fn test_check_ace_exclusions_multi_word_anti_topic() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("machine learning".to_string());
        let topics = vec!["machine learning ops".to_string()];
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
