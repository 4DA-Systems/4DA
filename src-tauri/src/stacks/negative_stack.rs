// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Negative Stack Model — infers what technologies the user does NOT use
//! and applies suppressive priors in scoring. Self-correcting: new deps promote,
//! positive interactions weaken, temporal decay prevents stale suppression.

use std::collections::{HashMap, HashSet};

/// Probability modifier for technologies the user likely doesn't use.
/// 1.0 = neutral (no suppression), 0.05 = maximum suppression.
#[derive(Debug, Clone, Default)]
pub struct NegativeStackContext {
    pub priors: HashMap<String, f32>,
}

/// Build the negative stack from user's dependencies and competing tech knowledge.
///
/// Logic:
/// - For each tech in COMPETING_TECH: if user HAS a competitor but NOT this tech -> prior = 0.15
/// - Everything else -> 1.0 (neutral)
///
/// Bounded to direct deps only — transitive deps don't create negative inferences.
///
/// v19.1 (AD-029/AD-030): the auto-detected anti-topic input was REMOVED.
/// It injected 0.30 suppression priors from `anti_topics` rows whose
/// confidence was pure dismissal count (rejection_count/10 — five
/// dismissals auto-banned a topic to ×0.30 composite authority), the last
/// behavioral scoring path left after the v19 demotion. Explicit topic
/// suppression is user-authored `exclusions`, which hard-filter upstream.
pub fn build_negative_stack<S: std::hash::BuildHasher>(
    user_direct_deps: &HashSet<String, S>,
    competing_pairs: &[(&str, &[&str])],
) -> NegativeStackContext {
    let mut priors = HashMap::new();

    // Infer competing-absent technologies
    for &(tech, competitors) in competing_pairs {
        let tech_lower = tech.to_lowercase();

        // Check if user has this tech
        if user_direct_deps.contains(&tech_lower) {
            continue; // User has this tech — it's positive, skip
        }

        // Check if user has any competitor of this tech
        let has_competitor = competitors
            .iter()
            .any(|comp| user_direct_deps.contains(&comp.to_lowercase()));

        if has_competitor {
            // User has a competitor but NOT this tech -> strong negative
            priors.insert(tech_lower, 0.15);
        }
    }

    NegativeStackContext { priors }
}

/// Look up the negative prior for an item based on its extracted topics.
/// Returns the minimum prior across all matching topics (most suppressive wins).
/// Returns 1.0 if no negative signal applies.
pub fn lookup_prior(ctx: &NegativeStackContext, topics: &[String]) -> f32 {
    if ctx.priors.is_empty() || topics.is_empty() {
        return 1.0;
    }

    let mut min_prior: f32 = 1.0;

    for topic in topics {
        let topic_lower = topic.to_lowercase();

        // Direct match
        if let Some(&prior) = ctx.priors.get(&topic_lower) {
            min_prior = min_prior.min(prior);
            continue;
        }

        // Check if topic contains a negative-stack key (e.g., "vue-router" contains "vue")
        for (neg_tech, &prior) in &ctx.priors {
            if topic_lower.contains(neg_tech.as_str()) && neg_tech.len() >= 3 {
                min_prior = min_prior.min(prior);
                break;
            }
        }
    }

    min_prior
}

#[cfg(test)]
mod tests {
    use super::*;

    fn competing_pairs() -> Vec<(&'static str, &'static [&'static str])> {
        vec![
            ("react", &["vue", "angular", "svelte"][..]),
            ("vue", &["react", "angular", "svelte"][..]),
            ("angular", &["react", "vue", "svelte"][..]),
            ("tauri", &["electron", "nwjs"][..]),
            ("electron", &["tauri", "nwjs"][..]),
            ("django", &["express", "axum", "rails"][..]),
            ("express", &["django", "axum", "rails"][..]),
            ("axum", &["django", "express", "rails"][..]),
        ]
    }

    #[test]
    fn test_competing_absent_suppressed() {
        let mut deps = HashSet::new();
        deps.insert("react".to_string());
        deps.insert("tauri".to_string());

        let ctx = build_negative_stack(&deps, &competing_pairs());

        // Vue should be suppressed (competing with react)
        assert!(ctx.priors.get("vue").copied().unwrap_or(1.0) < 0.20);
        // Electron should be suppressed (competing with tauri)
        assert!(ctx.priors.get("electron").copied().unwrap_or(1.0) < 0.20);
        // React should NOT be suppressed (user has it)
        assert!(ctx.priors.get("react").is_none());
        // Django should NOT be suppressed (no competitor detected)
        assert!(ctx.priors.get("django").is_none());
    }

    #[test]
    fn test_monorepo_both_positive() {
        let mut deps = HashSet::new();
        deps.insert("react".to_string());
        deps.insert("vue".to_string()); // Monorepo with both

        let ctx = build_negative_stack(&deps, &competing_pairs());

        // Neither should be suppressed
        assert!(ctx.priors.get("react").is_none());
        assert!(ctx.priors.get("vue").is_none());
    }

    #[test]
    fn anti_topics_no_longer_feed_the_negative_stack() {
        // v19.1 (AD-029/AD-030): dismissal-derived anti-topics were the last
        // behavioral scoring path — a 0.30 suppression prior from rejection
        // counts alone. The builder no longer accepts them; only
        // competing-tech inference from the actual dependency graph
        // produces priors.
        let deps = HashSet::new();
        let ctx = build_negative_stack(&deps, &competing_pairs());
        assert!(
            ctx.priors.is_empty(),
            "no deps and no competing evidence must mean no suppression priors"
        );
    }

    #[test]
    fn test_lookup_prior_direct_and_contains() {
        let mut deps = HashSet::new();
        deps.insert("react".to_string());

        let ctx = build_negative_stack(&deps, &competing_pairs());

        // Direct match
        let topics = vec!["vue".to_string(), "frontend".to_string()];
        let prior = lookup_prior(&ctx, &topics);
        assert!(
            prior < 0.20,
            "Vue topic should be heavily suppressed, got {prior}"
        );

        // Contains match: "vue-router" contains "vue"
        let topics2 = vec!["vue-router".to_string()];
        let prior2 = lookup_prior(&ctx, &topics2);
        assert!(
            prior2 < 0.20,
            "vue-router should be suppressed via vue, got {prior2}"
        );
    }

    #[test]
    fn test_lookup_neutral_and_empty() {
        let mut deps = HashSet::new();
        deps.insert("react".to_string());

        let ctx = build_negative_stack(&deps, &competing_pairs());

        // Neutral topics
        let topics = vec!["rust".to_string(), "performance".to_string()];
        let prior = lookup_prior(&ctx, &topics);
        assert!(
            (prior - 1.0).abs() < f32::EPSILON,
            "Neutral topics should have prior 1.0"
        );

        // Empty deps -> no suppression at all
        let empty_ctx = build_negative_stack(&HashSet::new(), &competing_pairs());
        assert!(empty_ctx.priors.is_empty());
    }
}
