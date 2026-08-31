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

/// Per-evidence scoring weight for a detected technology, strongest path wins.
///
/// 0.85 primary project, 0.40 secondary, 0.10 support infrastructure (MCP
/// servers, editor extensions, tooling and scripts — present because the app
/// ships them, not because they are what the developer builds).
///
/// MAX across the evidence list, and the subproject test applies PER PATH. The
/// original form ran two `any()` passes over the whole list with the
/// subproject test first, so one support path anywhere pinned the language to
/// 0.10 regardless of how much primary evidence sat beside it. Measured live
/// at e0381216: `javascript` (47 evidence entries, 7 of them primary
/// manifests) and `typescript` (26 entries, 6 primary) were both held at 0.10
/// — below a secondary project's 0.40 — by four paths each under
/// `editors/vscode` and `mcp-4da-server`, in an application that is roughly
/// half TypeScript. A tech that lives ONLY in support infrastructure still
/// scores 0.10, which is what the penalty was written for.
fn tech_weight_from_evidence(evidence: &[String], primary_dirs: &[String]) -> f32 {
    evidence
        .iter()
        .map(|ev| {
            let ev_lower = ev.to_lowercase().replace('\\', "/");
            let is_subproject = ev_lower.contains("/mcp-")
                || ev_lower.contains("/editors/")
                || ev_lower.contains("/tools/")
                || ev_lower.contains("/scripts/");
            if is_subproject {
                0.10
            } else if primary_dirs.iter().any(|d| ev_lower.contains(d.as_str())) {
                0.85
            } else {
                0.40
            }
        })
        .fold(0.0_f32, f32::max)
}

/// The user's own project roots, lowercased and slash-normalised for substring
/// matching against ACE evidence paths.
///
/// Sourced from `context_dirs` — the directories the user actually nominated —
/// NOT from the current working directory.
///
/// CWD was the wrong source in every deployment that matters. In an installed
/// build it is the install directory, so no evidence path can contain it and
/// every technology collapses to the 0.40 default, silently discarding the
/// primary/secondary distinction this weight exists to draw. Worse, the
/// background refresh is registered with `schtasks /Create` and no working
/// directory (see `engine_scheduler.rs`), so Task Scheduler hands it
/// `%windir%\system32` — verified on this machine: the task XML has no
/// `<WorkingDirectory>` element. That unattended run is the one that produces
/// the user's feed. Even in dev it was wrong: cwd is `src-tauri/` while the
/// evidence lives at the repo root.
///
/// All configured roots count, not just the first — a user with three project
/// directories is the author of all three.
pub(crate) fn primary_project_dirs() -> Vec<String> {
    #[allow(unused_mut)]
    let mut dirs: Vec<String> = crate::get_context_dirs()
        .iter()
        .map(|p| p.to_string_lossy().to_lowercase().replace('\\', "/"))
        .filter(|s| !s.is_empty())
        .collect();

    // Dev convenience only, and only when nothing is configured: a debug run
    // before onboarding has no context_dirs yet. Never in a release build,
    // where cwd is the install directory.
    #[cfg(debug_assertions)]
    if dirs.is_empty() {
        if let Ok(cwd) = std::env::current_dir() {
            let s = cwd.to_string_lossy().to_lowercase().replace('\\', "/");
            if !s.is_empty() {
                dirs.push(s);
            }
        }
    }

    dirs
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
        // The user's nominated project roots. NOT cwd — see primary_project_dirs.
        let primary_dirs = primary_project_dirs();

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

            // Weight is computed PER EVIDENCE PATH and the strongest wins.
            //
            // This used to be two `any()` passes over the whole evidence list
            // with the subproject test evaluated FIRST, so a single support
            // path anywhere in the list pinned the language to 0.10 no matter
            // how much primary-project evidence sat beside it. Measured live at
            // e0381216: `javascript` had 47 evidence entries — SEVEN of them
            // primary-project manifests — and was pinned to 0.10 by four paths
            // under `editors/vscode`; `typescript` had 26 entries, six of them
            // primary, pinned by four under `mcp-4da-server`. Both are primary
            // languages of this application, weighted below a secondary
            // project's 0.40, in an app that is roughly half TypeScript.
            //
            // A language present in BOTH a primary manifest and a subproject
            // keeps the primary weight; a language that lives ONLY in support
            // infrastructure still scores 0.10, which is the behaviour the
            // penalty was written for.
            let weight = tech_weight_from_evidence(&t.evidence, &primary_dirs);
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
    }
    /// The single configured project root these cases were written against.
    fn roots() -> Vec<String> {
        vec!["d:/4da".to_string()]
    }

    fn ev(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_string()).collect()
    }

    /// 2026-08-26 audit, A7. Four support paths out of 47 must not outvote
    /// seven primary-project manifests.
    #[test]
    fn primary_evidence_survives_a_subproject_path() {
        let evidence = ev(&[
            "Found in D:/4DA/editors/vscode/4da/package.json",
            "Found in D:/4DA/package.json",
            "Found in D:/4DA/site/package.json",
        ]);
        assert_eq!(tech_weight_from_evidence(&evidence, &roots()), 0.85);
    }

    /// The penalty still does its job when support infrastructure is ALL there is.
    #[test]
    fn support_only_tech_keeps_the_subproject_penalty() {
        let evidence = ev(&[
            "Found in D:/4DA/mcp-4da-server/package.json",
            "Found in D:/4DA/editors/vscode/4da/package.json",
        ]);
        assert_eq!(tech_weight_from_evidence(&evidence, &roots()), 0.10);
    }

    #[test]
    fn tech_outside_the_primary_project_is_secondary() {
        let evidence = ev(&["Found in C:/Users/x/Documents/other-app/package.json"]);
        assert_eq!(tech_weight_from_evidence(&evidence, &roots()), 0.40);
    }

    #[test]
    fn empty_evidence_scores_zero() {
        assert_eq!(tech_weight_from_evidence(&[], &roots()), 0.0);
    }

    /// THE regression. With no configured roots — which is what an installed
    /// build and the `schtasks` background refresh both produced, because the
    /// old code read cwd and cwd is the install dir or `system32` — every
    /// technology collapsed to the 0.40 default and the primary/secondary
    /// distinction this weight exists to draw disappeared silently.
    #[test]
    fn no_configured_roots_cannot_promote_anything_to_primary() {
        let evidence = ev(&["Found in D:/4DA/package.json"]);
        assert_eq!(tech_weight_from_evidence(&evidence, &[]), 0.40);
        // ...and the subproject penalty must still apply, so the absence of
        // roots degrades one axis rather than flattening all of them.
        let support = ev(&["Found in D:/4DA/mcp-4da-server/package.json"]);
        assert_eq!(tech_weight_from_evidence(&support, &[]), 0.10);
    }

    /// A user with several project directories is the author of all of them.
    /// The old signature took ONE root, so the second and third project's code
    /// scored as somebody else's.
    #[test]
    fn every_configured_root_counts_as_primary() {
        let many = vec![
            "d:/4da".to_string(),
            "d:/runyourempire/victauri".to_string(),
            "c:/work/thing".to_string(),
        ];
        for path in [
            "Found in D:/4DA/package.json",
            "Found in D:/runyourempire/victauri/Cargo.toml",
            "Found in C:/work/thing/go.mod",
        ] {
            assert_eq!(
                tech_weight_from_evidence(&ev(&[path]), &many),
                0.85,
                "{path} should be primary"
            );
        }
        // Something outside every configured root is still secondary.
        assert_eq!(
            tech_weight_from_evidence(&ev(&["Found in C:/elsewhere/app/package.json"]), &many),
            0.40
        );
    }

    #[test]
    fn test_ace_context_dependency_names_default() {
        let ctx = ACEContext::default();
        assert!(ctx.dependency_names.is_empty());
        assert!(ctx.dependency_info.is_empty());
    }
}
