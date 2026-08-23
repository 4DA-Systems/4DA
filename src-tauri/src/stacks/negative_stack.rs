// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Negative Stack Model — infers what technologies the user does NOT use
//! and applies suppressive priors in scoring. Self-correcting: new deps promote,
//! positive interactions weaken, temporal decay prevents stale suppression.

// UTF-8 safety gate (see the `clippy::string_slice` note in Cargo.toml).
// Byte-slicing a `str` panics on any index that is not a char boundary, so the
// lint is denied here: every slice must carry an explicit char-boundary proof
// (`floor_char_boundary`, an offset from `find` of an ASCII needle, or one of
// the `utils::text` helpers) or an `#[allow]` that states why it is safe.
#![deny(clippy::string_slice)]

use std::collections::{HashMap, HashSet};

use crate::utils::has_word_boundary_match;

/// Prior applied when an item matches a suppressed technology BUT carries
/// comparative/migration framing whose target or subject is the user's own
/// stack. Mirrors the 0.85 comparative tier of
/// `competing_tech::compute_competing_penalty`: soften, don't kill —
/// "We migrated our Electron app to Tauri" is top-value validation content
/// for a Tauri user, not noise.
const MIGRATION_SOFTENED_PRIOR: f32 = 0.85;

/// Comparative/migration markers, mirroring the `comparative_markers` table in
/// `competing_tech::compute_competing_penalty` (competing_tech.rs), plus
/// "migrated": the title scan below is word-bounded, so "migrate" alone cannot
/// catch the past tense in exactly the titles this carve-out exists for
/// ("We migrated our Electron app to Tauri").
const COMPARATIVE_MARKERS: [&str; 15] = [
    "vs",
    "versus",
    "compared to",
    "comparison",
    "alternative",
    "alternatives",
    "migrate",
    "migrated",
    "migrating",
    "migration",
    "switching from",
    "moving from",
    "moved from",
    "switch to",
    "benchmark",
];

/// Probability modifier for technologies the user likely doesn't use.
/// 1.0 = neutral (no suppression), 0.05 = maximum suppression.
#[derive(Debug, Clone, Default)]
pub struct NegativeStackContext {
    pub priors: HashMap<String, f32>,
    /// Suppressed tech (lowercase) -> the user-owned competitor(s) whose
    /// presence produced its prior. Feeds the migration carve-out in
    /// [`lookup_prior_with_content`]: `"electron" -> ["tauri"]` means electron
    /// content that reads as a migration to tauri is softened, not killed.
    pub owned_competitors: HashMap<String, Vec<String>>,
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
    let mut owned_competitors: HashMap<String, Vec<String>> = HashMap::new();

    // Infer competing-absent technologies
    for &(tech, competitors) in competing_pairs {
        let tech_lower = tech.to_lowercase();

        // Check if user has this tech
        if user_direct_deps.contains(&tech_lower) {
            continue; // User has this tech — it's positive, skip
        }

        // The competitors of this tech that the user actually has
        let owners: Vec<String> = competitors
            .iter()
            .map(|comp| comp.to_lowercase())
            .filter(|comp| user_direct_deps.contains(comp))
            .collect();

        if !owners.is_empty() {
            // User has a competitor but NOT this tech -> strong negative.
            // Remember WHICH competitor(s), so the migration carve-out can
            // recognize the user's own stack as the migration target.
            priors.insert(tech_lower.clone(), 0.15);
            owned_competitors.insert(tech_lower, owners);
        }
    }

    NegativeStackContext {
        priors,
        owned_competitors,
    }
}

/// Topic-only lookup, kept for call sites without item text (e.g. blind-spot
/// filtering, which only has a title's extracted topics). Cannot see
/// comparative/migration framing, so the carve-out never fires here.
/// Delegates to [`lookup_prior_with_content`].
pub fn lookup_prior(ctx: &NegativeStackContext, topics: &[String]) -> f32 {
    lookup_prior_with_content(ctx, topics, "", "")
}

/// Look up the negative prior for an item based on its extracted topics.
/// Returns the minimum prior across all matching topics (most suppressive wins).
/// Returns 1.0 if no negative signal applies.
///
/// Two hardenings over the original topic-substring form (2026-08-23 audit):
///
/// - **Bounded-token matching (§3.6):** `topic.contains(neg_tech)` tripped the
///   ×0.15 "react" prior on a Vue user's "reactivity" topic. A suppressed key
///   now has to occur as a whole token — "react-router" still matches "react",
///   "reactivity" does not. The "vuejs"/"reactjs" alias family (which only the
///   substring pass used to catch) is kept via an explicit `js`-suffix strip.
/// - **Migration carve-out (item 18):** comparative/migration content whose
///   target or subject is the user's OWN stack is softened to
///   [`MIGRATION_SOFTENED_PRIOR`] instead of killed. See
///   [`migration_carveout_applies`].
pub fn lookup_prior_with_content(
    ctx: &NegativeStackContext,
    topics: &[String],
    title: &str,
    content: &str,
) -> f32 {
    if ctx.priors.is_empty() || topics.is_empty() {
        return 1.0;
    }

    let mut min_prior: f32 = 1.0;
    let mut matched_techs: Vec<&String> = Vec::new();

    for topic in topics {
        let topic_lower = topic.to_lowercase();

        // Direct match
        if let Some((tech, &prior)) = ctx.priors.get_key_value(&topic_lower) {
            min_prior = min_prior.min(prior);
            matched_techs.push(tech);
            continue;
        }

        // Alias family: "vuejs"/"reactjs"/"nodejs" name the same tech as the
        // bare key. The bounded-token pass below cannot catch these ("js"
        // glues on with no boundary), and they were the one legitimate catch
        // of the substring pass this replaces.
        if let Some(stripped) = topic_lower.strip_suffix("js") {
            if stripped.len() >= 3 {
                if let Some((tech, &prior)) = ctx.priors.get_key_value(stripped) {
                    min_prior = min_prior.min(prior);
                    matched_techs.push(tech);
                    continue;
                }
            }
        }

        // Bounded-token containment: "react-router" carries "react" as a
        // whole token; "reactivity" does not (audit §3.6 — the old substring
        // check suppressed a Vue user's reactivity deep-dive undamped).
        for (neg_tech, &prior) in &ctx.priors {
            if neg_tech.len() >= 3 && has_word_boundary_match(&topic_lower, neg_tech) {
                min_prior = min_prior.min(prior);
                matched_techs.push(neg_tech);
                break;
            }
        }
    }

    if min_prior < MIGRATION_SOFTENED_PRIOR
        && migration_carveout_applies(ctx, &matched_techs, title, content)
    {
        return MIGRATION_SOFTENED_PRIOR;
    }

    min_prior
}

/// Item 18 carve-out: comparative/migration content whose target or subject is
/// the user's OWN stack must not be killed by a competitor's ×0.15 prior.
///
/// Mirrors the mechanism of `competing_tech::compute_competing_penalty`
/// (competing_tech.rs): markers are scanned word-bounded in the title and by
/// substring in the first 500 chars of content; the user's own tech is scanned
/// word-bounded in the title and the first 2000 chars of content. Both legs
/// must hold, and the user tech consulted is pair-precise — only the owned
/// competitor(s) of a prior that actually MATCHED count, so "Migrating from
/// Vue to Angular" stays suppressed for a React user: markers alone never
/// soften.
fn migration_carveout_applies(
    ctx: &NegativeStackContext,
    matched_techs: &[&String],
    title: &str,
    content: &str,
) -> bool {
    if title.is_empty() && content.is_empty() {
        return false;
    }

    let title_lower = title.to_lowercase();
    // Limit content scan to first 2000 chars, as competing_tech does.
    // SAFE: `floor_char_boundary` returns a char boundary by definition.
    #[allow(clippy::string_slice)]
    let content_lower = content[..content.floor_char_boundary(2000)].to_lowercase();

    let markers_present = COMPARATIVE_MARKERS.iter().any(|m| {
        // SAFE: `floor_char_boundary` returns a char boundary by definition.
        #[allow(clippy::string_slice)]
        let head = &content_lower[..content_lower.floor_char_boundary(500)];
        has_word_boundary_match(&title_lower, m) || head.contains(m)
    });
    if !markers_present {
        return false;
    }

    // The user's own stack must be the migration target or subject: an owned
    // competitor of a matched prior appears in the title or early content.
    matched_techs.iter().any(|tech| {
        ctx.owned_competitors
            .get(tech.as_str())
            .is_some_and(|owners| {
                owners.iter().any(|owner| {
                    has_word_boundary_match(&title_lower, owner)
                        || has_word_boundary_match(&content_lower, owner)
                })
            })
    })
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
        // The suppression owners are recorded for the migration carve-out
        assert_eq!(
            ctx.owned_competitors.get("electron"),
            Some(&vec!["tauri".to_string()])
        );
        assert_eq!(
            ctx.owned_competitors.get("vue"),
            Some(&vec!["react".to_string()])
        );
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
        assert!(ctx.owned_competitors.is_empty());
    }

    #[test]
    fn test_lookup_prior_direct_and_bounded_token() {
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

        // Bounded-token match: "vue-router" carries "vue" as a whole token
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

    // --- §3.6: bounded-token matching, not substring ---

    #[test]
    fn substring_topic_no_longer_trips_prior() {
        // Vue user: "react" carries a ×0.15 prior. The old substring pass
        // matched "react" INSIDE "reactivity" — a Vue reactivity deep-dive
        // died undamped.
        let mut deps = HashSet::new();
        deps.insert("vue".to_string());
        let ctx = build_negative_stack(&deps, &competing_pairs());
        assert!(ctx.priors.get("react").copied().unwrap_or(1.0) < 0.20);

        let prior = lookup_prior(&ctx, &["reactivity".to_string()]);
        assert!(
            (prior - 1.0).abs() < f32::EPSILON,
            "'reactivity' must not trip the 'react' prior, got {prior}"
        );

        // Bounded token still matches: react-router IS react content.
        let prior2 = lookup_prior(&ctx, &["react-router".to_string()]);
        assert!(
            prior2 < 0.20,
            "react-router must still be suppressed via react, got {prior2}"
        );

        // Alias family: "reactjs" names the same tech as "react".
        let prior3 = lookup_prior(&ctx, &["reactjs".to_string()]);
        assert!(
            prior3 < 0.20,
            "reactjs must still be suppressed via react, got {prior3}"
        );
    }

    // --- Item 18: migration/comparison carve-out ---

    #[test]
    fn migration_to_user_stack_softened_not_killed() {
        let mut deps = HashSet::new();
        deps.insert("tauri".to_string());
        let ctx = build_negative_stack(&deps, &competing_pairs());

        let topics = vec!["electron".to_string(), "desktop".to_string()];
        let prior =
            lookup_prior_with_content(&ctx, &topics, "We migrated our Electron app to Tauri", "");
        assert!(
            (prior - MIGRATION_SOFTENED_PRIOR).abs() < 0.01,
            "migration-to-user-stack content must soften to 0.85, got {prior}"
        );
    }

    #[test]
    fn migration_marker_and_owner_in_content_body_softened() {
        let mut deps = HashSet::new();
        deps.insert("tauri".to_string());
        let ctx = build_negative_stack(&deps, &competing_pairs());

        let topics = vec!["electron".to_string()];
        let prior = lookup_prior_with_content(
            &ctx,
            &topics,
            "Our desktop rewrite, one year later",
            "After migrating from Electron to Tauri, our installer shrank by 90%.",
        );
        assert!(
            (prior - MIGRATION_SOFTENED_PRIOR).abs() < 0.01,
            "markers + owner in content body must soften to 0.85, got {prior}"
        );
    }

    #[test]
    fn plain_competitor_promo_still_suppressed() {
        let mut deps = HashSet::new();
        deps.insert("tauri".to_string());
        let ctx = build_negative_stack(&deps, &competing_pairs());

        let topics = vec!["electron".to_string()];
        let prior = lookup_prior_with_content(
            &ctx,
            &topics,
            "Electron 30 Released with Performance Improvements",
            "Electron 30 ships a new renderer and faster startup.",
        );
        assert!(
            prior < 0.20,
            "plain competitor promo must stay at the full prior, got {prior}"
        );
    }

    #[test]
    fn migration_between_foreign_techs_still_suppressed() {
        // React user: vue AND angular carry priors. Markers are present, but
        // the user's own stack is neither target nor subject — no carve-out.
        let mut deps = HashSet::new();
        deps.insert("react".to_string());
        let ctx = build_negative_stack(&deps, &competing_pairs());

        let topics = vec!["vue".to_string(), "angular".to_string()];
        let prior = lookup_prior_with_content(
            &ctx,
            &topics,
            "Migrating from Vue to Angular",
            "A practical guide to moving a large app from Vue to Angular.",
        );
        assert!(
            prior < 0.20,
            "markers alone (no owned tech present) must not soften, got {prior}"
        );
    }

    #[test]
    fn topic_only_lookup_cannot_see_migration_framing() {
        // The topic-only entry point (blind-spot filtering) has no item text,
        // so the carve-out never fires there — the kill stands.
        let mut deps = HashSet::new();
        deps.insert("tauri".to_string());
        let ctx = build_negative_stack(&deps, &competing_pairs());

        let prior = lookup_prior(&ctx, &["electron".to_string()]);
        assert!(
            prior < 0.20,
            "topic-only lookup keeps the prior, got {prior}"
        );
    }
}
