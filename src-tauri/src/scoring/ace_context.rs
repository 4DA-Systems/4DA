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
    /// Maps detected tech → project directory basename(s) derived from manifest evidence.
    /// Example: "tauri" → ["4da"], "react" → ["4da", "my-app"]
    pub tech_projects: HashMap<String, Vec<String>>,
}

/// Structural labels that don't represent real user interests — they describe
/// code organization (directories, CI stages, commit prefixes) rather than
/// technologies the user cares about. Filtering these prevents the scoring
/// pipeline from boosting generic infrastructure content.
const STRUCTURAL_LABELS: &[&str] = &[
    "api",
    "backend",
    "frontend",
    "db",
    "database",
    "test",
    "tests",
    "testing",
    "ui",
    "config",
    "build",
    "deploy",
    "ci",
    "cd",
    "docs",
    "documentation",
    "refactor",
    "fix",
    "feature",
    "chore",
    "security",
    "update",
    "upgrade",
    "migration",
    "setup",
    "init",
];

/// Extract a project directory name from ACE evidence strings like "Found in /path/to/project/Cargo.toml".
/// Returns the parent directory basename (e.g., "my-app" from "/home/user/my-app/package.json").
fn extract_project_name_from_evidence(evidence: &str) -> Option<String> {
    let path_part = evidence.strip_prefix("Found in ").unwrap_or(evidence);
    let normalized = path_part.replace('\\', "/");
    let path = std::path::Path::new(normalized.as_str());
    let parent = path.parent()?;
    let name = parent.file_name()?.to_str()?;
    if name.is_empty() || name == "src" || name == "." {
        parent.parent()?.file_name()?.to_str().map(String::from)
    } else {
        Some(name.to_string())
    }
}

/// QUARANTINED (AD-029, v19) — always returns an EMPTY map.
///
/// Learned topic affinities were demoted after the 2026-08-11 poisoned-curve
/// incident: the capture layer mixed three incompatible strength scales and
/// self-poisoned (the 2026-07-13 doom loop drove the user's OWN stack to -1.0
/// affinity while `compute_affinity_multiplier` held x[0.3, 1.7] authority over
/// the score). The demotion was applied at the CALL SITES — `pipeline_v2.rs`
/// pins `affinity_mult` to 1.0, `semantic/boost.rs` dropped its affinity
/// scaling — but the LOADER kept populating this map, so any consumer that
/// reached for it bypassed the quarantine. `channel_render.rs` did exactly
/// that: it multiplied its match score by `compute_affinity_multiplier` and
/// PERSISTED the result via `upsert_channel_source_match`. That function
/// returns a neutral 1.0 only when the map is empty, and the loader guaranteed
/// it was not.
///
/// Killing it here makes the quarantine hold BY CONSTRUCTION: no call site can
/// opt back in, because the data never enters the scoring context. The capture
/// pipeline keeps writing `topic_affinities` (the Learned Preferences panel and
/// the engagement dashboard read the table directly), so nothing is lost that
/// re-enabling could not restore. Re-enable criteria live in AD-029; doing so
/// means deleting `ace_context_quarantines_topic_affinities`, not editing it.
/// (v20a: `compute_affinity_multiplier` and the other structurally-dead readers
/// of this map were deleted outright; this loader stays as the quarantine.)
fn load_topic_affinities(_ace: &crate::ace::ACE) -> HashMap<String, (f32, f32)> {
    HashMap::new()
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
            if STRUCTURAL_LABELS.contains(&topic_lower.as_str()) {
                continue;
            }
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

            // Subproject penalty: MCP servers, editors, tools are support infrastructure
            let is_subproject = t.evidence.iter().any(|ev| {
                let ev_lower = ev.to_lowercase().replace('\\', "/");
                ev_lower.contains("/mcp-")
                    || ev_lower.contains("/editors/")
                    || ev_lower.contains("/tools/")
                    || ev_lower.contains("/scripts/")
            });

            let weight: f32 = if is_subproject {
                0.10
            } else if is_primary {
                0.85
            } else {
                0.40
            };
            let existing = ctx
                .tech_weights
                .get(&name_lower)
                .copied()
                .unwrap_or(0.0_f32);
            ctx.tech_weights
                .insert(name_lower.clone(), weight.max(existing));

            // Extract project name(s) from evidence paths for project association
            let projects = ctx.tech_projects.entry(name_lower).or_default();
            for ev in &t.evidence {
                if let Some(project_name) = extract_project_name_from_evidence(ev) {
                    if !projects.contains(&project_name) {
                        projects.push(project_name);
                    }
                }
            }
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

    // Learned topic affinities are QUARANTINED — see `load_topic_affinities`.
    ctx.topic_affinities = load_topic_affinities(&ace);

    // Merge session-aware work topics with graduated confidence.
    // Uses gap-based session detection: current session gets highest confidence,
    // previous same-day session gets moderate, yesterday gets low.
    if let Ok(work_topics) = ace.get_session_aware_work_topics() {
        for (topic, weight) in work_topics {
            if STRUCTURAL_LABELS.contains(&topic.as_str()) {
                continue;
            }
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

    /// AD-029 quarantine guard. An ACE carrying STRONG, well-evidenced learned
    /// affinities must still hand the scoring context an EMPTY map — that is
    /// what guarantees no scoring consumer can ever see a learned affinity
    /// (the former reader, `compute_affinity_multiplier`, was deleted in v20a).
    ///
    /// This test fails the moment the loader starts populating the map again.
    /// Re-enabling affinities means DELETING this test as a deliberate act (and
    /// filing the ADR AD-029 asks for), not quietly editing around it.
    #[test]
    fn ace_context_quarantines_topic_affinities() {
        use crate::ace::create_test_ace;
        use crate::ace::BehaviorAction;

        let ace = create_test_ace();
        // 6 saves clears `get_topic_affinities`' min_exposures of 5 and pushes
        // |affinity_score| well past the loader's old 0.1 admission floor, so a
        // populating loader WOULD return a non-empty map here.
        for item_id in 1..=6 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Save,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            )
            .expect("record save");
        }

        // Precondition: the capture layer really did learn something — otherwise
        // the assertion below would pass vacuously.
        let learned = ace.get_topic_affinities().expect("read affinities");
        let rust = learned
            .iter()
            .find(|a| a.topic == "rust")
            .expect("capture layer learned a rust affinity");
        assert!(
            rust.affinity_score.abs() > 0.1 && rust.total_exposures >= 3,
            "fixture must satisfy the old loader's admission test, got {rust:?}"
        );

        assert!(
            load_topic_affinities(&ace).is_empty(),
            "learned affinities are quarantined (AD-029) and must not reach scoring"
        );
    }
}
