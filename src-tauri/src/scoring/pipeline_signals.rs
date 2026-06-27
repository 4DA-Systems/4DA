// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Signal classification, corroboration, and quality-composite helpers extracted
//! from the scoring pipeline.
//!
//! Keeps `pipeline.rs` under the 700-line file-size limit while co-locating
//! related scoring logic in one place.

use crate::db::Database;
use crate::scoring_config;
use crate::signals;
use crate::SourceRelevance;

use super::dependencies;
use super::ScoringContext;
use super::VersionDelta;

use super::pipeline::ScoringInput;

/// Build a real CorroborationContext from the database for a given item.
///
/// Queries:
/// 1. How many distinct source types have items about similar topics in the last 72 hours
/// 2. Whether any matched dependency confirms the signal
/// 3. Whether any open signal chain covers this topic and its current phase
pub(super) fn build_corroboration(
    db: &Database,
    topics: &[String],
    matched_deps: &[dependencies::DepMatch],
) -> signals::CorroborationContext {
    if topics.is_empty() {
        return signals::CorroborationContext::default();
    }

    // 1. Count distinct source types with items about the same topics in last 72 hours
    let source_count = {
        let conn = db.conn.lock();
        let topic_like_clauses: Vec<String> = topics
            .iter()
            .take(5) // Limit to top 5 topics for query performance
            .map(|t| {
                format!(
                    "LOWER(title) LIKE '%{}%'",
                    t.to_lowercase().replace('\'', "''")
                )
            })
            .collect();

        if topic_like_clauses.is_empty() {
            1_usize
        } else {
            let where_clause = topic_like_clauses.join(" OR ");
            let query = format!(
                "SELECT COUNT(DISTINCT source_type) FROM source_items \
                 WHERE created_at >= datetime('now', '-3 days') AND ({where_clause})"
            );
            conn.query_row(&query, [], |row| row.get::<_, i64>(0))
                .unwrap_or(1) as usize
        }
    };

    // 2. Dependency match — the single canonical grounding predicate. A bare
    //    non-dev hit is NOT enough; the classifier's Critical hard-gate trusts
    //    this flag, so it must mean the same "strongly grounded" as the
    //    evidence pool and the persisted link set (non-dev, confidence >= the
    //    strong floor, non-ambiguous name).
    let dependency_match = dependencies::is_strongly_grounded(matched_deps);

    // 3. Signal chain phase — detect if topics appear across multiple days
    //    (lightweight chain detection without the full detect_chains() machinery)
    let chain_phase = {
        let conn = db.conn.lock();
        let mut phase: Option<String> = None;
        for topic in topics.iter().take(3) {
            let topic_lower = topic.to_lowercase();
            // Count distinct days this topic has appeared in source items (last 7 days)
            let day_count: i64 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT DATE(created_at)) FROM source_items \
                     WHERE created_at >= datetime('now', '-7 days') AND LOWER(title) LIKE ?1",
                    rusqlite::params![format!("%{}%", topic_lower)],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if day_count >= 4 {
                phase = Some("peak".to_string());
                break;
            } else if day_count >= 3 {
                phase = Some("escalating".to_string());
                break;
            } else if day_count >= 2 && phase.is_none() {
                phase = Some("active".to_string());
            }
        }
        phase
    };

    signals::CorroborationContext {
        source_count,
        dependency_match,
        chain_phase,
    }
}

/// Construct the early-return `SourceRelevance` for excluded items.
///
/// Avoids duplicating the 30+-field struct literal in `score_item`.
pub(super) fn build_excluded_result(input: &ScoringInput, exclusion: String) -> SourceRelevance {
    SourceRelevance {
        id: input.id,
        title: input.title.to_string(),
        url: input.url.map(std::string::ToString::to_string),
        top_score: 0.0,
        matches: vec![],
        relevant: false,
        context_score: 0.0,
        interest_score: 0.0,
        excluded: true,
        excluded_by: Some(exclusion),
        source_type: input.source_type.to_string(),
        explanation: None,
        confidence: Some(0.0),
        score_breakdown: None,
        signal_type: None,
        signal_priority: None,
        signal_action: None,
        signal_triggers: None,
        signal_horizon: None,
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
        streets_engine: None,
        decision_window_match: None,
        decision_boost_applied: 0.0,
        created_at: input.created_at.map(chrono::DateTime::to_rfc3339),
        detected_lang: input.detected_lang.to_string(),
        is_critical_alert: false,
        applicability: None,
        advisory_id: None,
        primary_topic: None,
    }
}

/// Resolved signal fields returned by [`classify_item_signal`].
pub(super) struct SignalResult {
    pub sig_type: Option<String>,
    pub sig_priority: Option<String>,
    pub sig_action: Option<String>,
    pub sig_triggers: Option<Vec<String>>,
    pub sig_horizon: Option<String>,
}

impl SignalResult {
    pub fn none() -> Self {
        Self {
            sig_type: None,
            sig_priority: None,
            sig_action: None,
            sig_triggers: None,
            sig_horizon: None,
        }
    }
}

/// Run signal classification on a scored item, applying dependency-aware
/// priority escalation and score-aware priority caps.
///
/// Returns `SignalResult::none()` when classification is skipped or no signal
/// is detected.
pub(super) fn classify_item_signal(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    classifier: &signals::SignalClassifier,
    topics: &[String],
    matched_deps: &[dependencies::DepMatch],
    combined_score: f32,
) -> SignalResult {
    let corroboration = build_corroboration(db, topics, matched_deps);
    match classifier.classify(
        input.title,
        input.content,
        combined_score,
        &ctx.declared_tech,
        &ctx.ace_ctx.detected_tech,
        &corroboration,
    ) {
        Some(mut c) => {
            // Dependency-aware priority escalation:
            // Security + STRONG non-dev dep match (>= 0.40) → Critical
            // Breaking change + newer version → High
            // The 0.40 threshold requires the full package name OR 2+ subterms.
            if !matched_deps.is_empty() {
                let has_strong_dep = matched_deps.iter().any(|d| {
                    !d.is_dev
                        && d.confidence
                            >= scoring_config::SECURITY_DEP_VALIDATION_STRONG_DEP_THRESHOLD
                });
                if c.signal_type == signals::SignalType::SecurityAlert && has_strong_dep {
                    c.priority = signals::SignalPriority::Critical;
                    let best_dep = matched_deps
                        .iter()
                        .filter(|d| !d.is_dev)
                        .max_by(|a, b| {
                            a.confidence
                                .partial_cmp(&b.confidence)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap_or(&matched_deps[0]);
                    c.action = format!(
                        "Critical: Security issue affects your dependency {}",
                        best_dep.package_name
                    );
                } else if c.signal_type == signals::SignalType::BreakingChange
                    && matched_deps
                        .iter()
                        .any(|d| d.version_delta == VersionDelta::NewerMajor)
                    && c.priority < signals::SignalPriority::Alert
                {
                    c.priority = signals::SignalPriority::Alert;
                }
                // Add dep:package_name triggers
                for dep in matched_deps.iter().take(2) {
                    c.triggers.push(format!("dep:{}", dep.package_name));
                }
            }

            // Score-aware priority cap — low scores cannot produce HIGH priority
            if combined_score < scoring_config::LOW_SCORE_CAP
                && c.priority > signals::SignalPriority::Watch
            {
                c.priority = signals::SignalPriority::Watch;
            } else if (combined_score < scoring_config::MEDIUM_SCORE_CAP
                && c.priority > signals::SignalPriority::Advisory)
                || (combined_score > scoring_config::HIGH_SCORE_FLOOR
                    && c.priority < signals::SignalPriority::Advisory)
            {
                c.priority = signals::SignalPriority::Advisory;
            }
            SignalResult {
                sig_type: Some(c.signal_type.slug().to_string()),
                sig_priority: Some(c.priority.label().to_string()),
                sig_action: Some(c.action),
                sig_triggers: Some(c.triggers),
                sig_horizon: Some(c.horizon.label().to_string()),
            }
        }
        None => SignalResult::none(),
    }
}

/// Quality multipliers computed by [`compute_quality_composite`].
pub(super) struct QualityComposite {
    /// Dampened product of all quality multipliers.
    pub composite_mult: f32,
    /// Individual multipliers retained for breakdown reporting.
    pub competing_mult: f32,
    pub content_quality_mult: f32,
    pub content_dna_mult: f32,
    pub content_type: crate::content_dna::ContentType,
    pub novelty: crate::novelty::NoveltyScore,
    pub ecosystem_shift_mult: f32,
    pub stack_competing_mult: f32,
}

/// Adjust content DNA multiplier based on the user's self-declared experience level.
///
/// "learning" users benefit from tutorials and questions — these are useful signals,
/// not noise. "building" users get a mild lift. Default (None / "shipping" / "leading")
/// keeps the current calibration which is tuned for experienced developers.
fn adjust_dna_for_experience(
    content_type: &crate::content_dna::ContentType,
    base: f32,
    level: Option<&str>,
) -> f32 {
    use crate::content_dna::ContentType;
    match level {
        Some("learning") => match content_type {
            ContentType::Tutorial => base * 1.35, // 0.80→1.08: tutorials are helpful
            ContentType::Question => base * 1.30, // 0.65→0.85: questions resonate
            ContentType::HelpRequest => base * 1.25, // 0.50→0.63: may identify with
            ContentType::ShowAndTell => base * 1.15, // 0.85→0.98: inspiring projects
            ContentType::DeepDive => base * 0.90, // 1.15→1.04: can be overwhelming
            _ => base,
        },
        Some("building") => match content_type {
            ContentType::Tutorial => base * 1.10, // 0.80→0.88: occasionally useful
            ContentType::Question => base * 1.10, // 0.65→0.72: sometimes useful
            ContentType::ShowAndTell => base * 1.05, // 0.85→0.89: somewhat inspiring
            _ => base,
        },
        // "shipping", "leading", None → current calibration (experienced devs)
        _ => base,
    }
}

/// Compute all quality multipliers and combine them into a dampened composite.
///
/// Evaluates competing tech, content quality, content DNA, novelty, ecosystem
/// shift, and stack-aware competing penalty. Returns individual multipliers
/// (for breakdown) and the dampened composite product.
pub(super) fn compute_quality_composite(
    input: &ScoringInput,
    topics: &[String],
    ctx: &ScoringContext,
    domain_relevance: f32,
) -> QualityComposite {
    // Competing tech penalty: content primarily about alternatives gets demoted
    let competing_mult = crate::competing_tech::compute_competing_penalty(
        topics,
        input.title,
        input.content,
        &ctx.domain_profile.primary_stack,
    );

    // Content quality: penalize clickbait, boost authoritative technical content
    let content_quality =
        crate::content_quality::compute_content_quality(input.title, input.content, input.url);

    // Content DNA: source-type-aware utility multiplier
    let (content_type, content_dna_mult) = crate::content_dna::classify_content_for_source(
        input.title,
        input.content,
        input.source_type,
    );
    let content_dna_mult = if input.content.len() < 30 {
        content_dna_mult * 0.85
    } else {
        content_dna_mult
    };

    // Experience-level DNA adjustment: "learning" users benefit from tutorials,
    // "building" users get a mild lift, default keeps current calibration.
    let content_dna_mult = adjust_dna_for_experience(
        &content_type,
        content_dna_mult,
        ctx.experience_level.as_deref(),
    );

    // Novelty: penalize introductory content for known tech, boost releases
    let novelty = crate::novelty::compute_novelty(
        input.title,
        input.content,
        topics,
        &ctx.domain_profile.primary_stack,
        ctx.user_role.as_deref(),
        ctx.experience_level.as_deref(),
    );

    // Ecosystem shift detection from stack profiles
    let ecosystem_shift_mult =
        crate::stacks::scoring::detect_ecosystem_shift(topics, input.title, &ctx.composed_stack);

    // Stack-aware competing tech penalty: suppresses content about alternatives
    // when the user doesn't also mention their own tech (e.g., pure Go article for Rust user)
    let stack_competing_mult = crate::stacks::scoring::compute_competing_penalty(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    // Combine all quality multipliers as a SINGLE dampened composite.
    // Asymmetric dampening: penalties keep more teeth than boosts.
    let dampen = |m: f32| {
        if m < 1.0 {
            1.0 + (m - 1.0) * scoring_config::DAMPENING_PENALTY_STRENGTH
        } else {
            1.0 + (m - 1.0) * scoring_config::DAMPENING_BOOST_STRENGTH
        }
    };
    // Domain-aware content_dna dampening: "I built [YOUR TECH]" is valuable,
    // "I built [random thing]" is not. When domain_relevance == 1.0 (primary stack),
    // reduce content_dna penalty strength for primary stack items.
    let content_dna_dampened =
        if content_dna_mult < 1.0 && domain_relevance >= 1.0 && !ctx.domain_profile.is_empty() {
            1.0 + (content_dna_mult - 1.0) * scoring_config::DAMPENING_DOMAIN_AWARE_STRENGTH
        } else {
            dampen(content_dna_mult)
        };
    let composite_mult = dampen(competing_mult)
        * dampen(content_quality.multiplier)
        * content_dna_dampened
        * dampen(novelty.multiplier)
        * dampen(ecosystem_shift_mult)
        * dampen(stack_competing_mult);

    QualityComposite {
        composite_mult,
        competing_mult,
        content_quality_mult: content_quality.multiplier,
        content_dna_mult,
        content_type,
        novelty,
        ecosystem_shift_mult,
        stack_competing_mult,
    }
}

#[cfg(test)]
mod tests {
    use super::adjust_dna_for_experience;
    use super::build_corroboration;
    use super::dependencies::{DepMatch, VersionDelta};
    use crate::content_dna::ContentType;
    use crate::db::Database;
    use crate::test_utils::{insert_test_item, test_db};

    /// Build a `DepMatch` for grounding tests. A non-dev match with confidence
    /// >= `STRONG_GROUNDING_CONFIDENCE` (0.40) and a non-ambiguous name is
    /// "strongly grounded"; flip `is_dev` or drop the confidence to break it.
    fn dep(name: &str, confidence: f32, is_dev: bool) -> DepMatch {
        DepMatch {
            package_name: name.to_string(),
            confidence,
            version_delta: VersionDelta::Unknown,
            is_dev,
            is_direct: true,
            version: None,
            ecosystem: "rust".to_string(),
        }
    }

    /// Insert one item per entry in `day_offsets`, each titled with `topic`, and
    /// back-date its `created_at` by that many days. Drives the distinct-day
    /// chain-phase detection in `build_corroboration`.
    fn insert_topic_on_days(db: &Database, source_type: &str, topic: &str, day_offsets: &[i64]) {
        for (i, &days) in day_offsets.iter().enumerate() {
            let id = insert_test_item(
                db,
                source_type,
                &format!("{topic}-{i}"),
                &format!("{topic} update {i}"),
                "body",
            );
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET created_at = datetime('now', ?1) WHERE id = ?2",
                rusqlite::params![format!("-{days} days"), id],
            )
            .expect("backdate created_at");
        }
    }

    /// Float comparison helper — experience adjustments are exact products,
    /// but we compare with an epsilon to stay robust to f32 representation.
    fn approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < 1e-5,
            "expected {b}, got {a} (diff {})",
            (a - b).abs()
        );
    }

    const BASE: f32 = 0.80;

    // ---- "learning" experience level: tutorials/questions are signal, not noise ----

    #[test]
    fn learning_boosts_tutorials() {
        approx(
            adjust_dna_for_experience(&ContentType::Tutorial, BASE, Some("learning")),
            BASE * 1.35,
        );
    }

    #[test]
    fn learning_boosts_questions_and_help_requests() {
        approx(
            adjust_dna_for_experience(&ContentType::Question, BASE, Some("learning")),
            BASE * 1.30,
        );
        approx(
            adjust_dna_for_experience(&ContentType::HelpRequest, BASE, Some("learning")),
            BASE * 1.25,
        );
    }

    #[test]
    fn learning_mildly_boosts_show_and_tell() {
        approx(
            adjust_dna_for_experience(&ContentType::ShowAndTell, BASE, Some("learning")),
            BASE * 1.15,
        );
    }

    #[test]
    fn learning_dampens_deep_dives() {
        // Deep dives can overwhelm learners — the only sub-1.0 multiplier here.
        approx(
            adjust_dna_for_experience(&ContentType::DeepDive, BASE, Some("learning")),
            BASE * 0.90,
        );
    }

    #[test]
    fn learning_leaves_unlisted_types_unchanged() {
        // ReleaseNotes is not in the "learning" arm — must pass through untouched.
        approx(
            adjust_dna_for_experience(&ContentType::ReleaseNotes, BASE, Some("learning")),
            BASE,
        );
    }

    // ---- "building" experience level: a mild lift on a narrower set ----

    #[test]
    fn building_mildly_boosts_tutorials_and_questions() {
        approx(
            adjust_dna_for_experience(&ContentType::Tutorial, BASE, Some("building")),
            BASE * 1.10,
        );
        approx(
            adjust_dna_for_experience(&ContentType::Question, BASE, Some("building")),
            BASE * 1.10,
        );
    }

    #[test]
    fn building_mildly_boosts_show_and_tell() {
        approx(
            adjust_dna_for_experience(&ContentType::ShowAndTell, BASE, Some("building")),
            BASE * 1.05,
        );
    }

    #[test]
    fn building_leaves_deep_dives_and_help_requests_unchanged() {
        // DeepDive and HelpRequest are NOT in the "building" arm.
        approx(
            adjust_dna_for_experience(&ContentType::DeepDive, BASE, Some("building")),
            BASE,
        );
        approx(
            adjust_dna_for_experience(&ContentType::HelpRequest, BASE, Some("building")),
            BASE,
        );
    }

    // ---- experienced / unset levels: current calibration is preserved verbatim ----

    #[test]
    fn shipping_and_leading_preserve_base_calibration() {
        for level in ["shipping", "leading"] {
            for ct in [
                ContentType::Tutorial,
                ContentType::Question,
                ContentType::DeepDive,
                ContentType::ShowAndTell,
            ] {
                approx(adjust_dna_for_experience(&ct, BASE, Some(level)), BASE);
            }
        }
    }

    #[test]
    fn none_and_unknown_levels_preserve_base_calibration() {
        approx(
            adjust_dna_for_experience(&ContentType::Tutorial, BASE, None),
            BASE,
        );
        // An unrecognized level string falls through to the default arm.
        approx(
            adjust_dna_for_experience(&ContentType::Tutorial, BASE, Some("expert")),
            BASE,
        );
    }

    #[test]
    fn adjustment_scales_linearly_with_base() {
        // The multiplier is independent of the base magnitude.
        approx(
            adjust_dna_for_experience(&ContentType::Tutorial, 0.5, Some("learning")),
            0.5 * 1.35,
        );
        approx(
            adjust_dna_for_experience(&ContentType::Tutorial, 1.0, Some("learning")),
            1.35,
        );
    }

    // ---- build_corroboration: real corroboration context from the DB ----

    #[test]
    fn corroboration_empty_topics_is_default() {
        // No topics → no DB work, the canonical default context. The default
        // is deliberately *restrictive*: source_count = 1 (single-source gate
        // applies), not 0, so an un-topiced item is never treated as corroborated.
        let db = test_db();
        let c = build_corroboration(&db, &[], &[]);
        assert_eq!(c.source_count, 1);
        assert!(!c.dependency_match);
        assert!(c.chain_phase.is_none());
    }

    #[test]
    fn corroboration_counts_distinct_source_types() {
        // Three different source types all talking about "rust" → source_count 3.
        let db = test_db();
        insert_test_item(&db, "hackernews", "a", "Rust 2.0 released", "body");
        insert_test_item(&db, "reddit", "b", "Why Rust wins", "body");
        insert_test_item(&db, "github", "c", "rust-lang/rust news", "body");
        // An unrelated item must not inflate the count.
        insert_test_item(&db, "lobsters", "d", "Python tips", "body");

        let c = build_corroboration(&db, &["rust".to_string()], &[]);
        assert_eq!(
            c.source_count, 3,
            "three distinct source types mention rust"
        );
        // All inserted same-day → only one distinct day → no chain phase.
        assert!(c.chain_phase.is_none());
    }

    #[test]
    fn corroboration_source_count_zero_when_no_title_match() {
        let db = test_db();
        insert_test_item(&db, "hackernews", "a", "Python tips", "body");
        let c = build_corroboration(&db, &["nonexistent-topic".to_string()], &[]);
        assert_eq!(c.source_count, 0);
    }

    #[test]
    fn corroboration_dependency_match_true_for_strong_grounding() {
        let db = test_db();
        // Non-dev, confident, non-ambiguous package name → strongly grounded.
        let c = build_corroboration(&db, &["x".to_string()], &[dep("tokio", 0.95, false)]);
        assert!(c.dependency_match);
    }

    #[test]
    fn corroboration_dependency_match_false_for_dev_or_weak_dep() {
        let db = test_db();
        // Dev dependency is not a grounding edge even at high confidence.
        let c1 = build_corroboration(&db, &["x".to_string()], &[dep("tokio", 0.95, true)]);
        assert!(!c1.dependency_match);
        // Confidence below the 0.40 strong floor does not ground.
        let c2 = build_corroboration(&db, &["x".to_string()], &[dep("tokio", 0.30, false)]);
        assert!(!c2.dependency_match);
        // No deps at all.
        let c3 = build_corroboration(&db, &["x".to_string()], &[]);
        assert!(!c3.dependency_match);
    }

    #[test]
    fn corroboration_chain_phase_active_escalating_peak() {
        // 2 distinct days → "active".
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "kubernetes", &[0, 1]);
        assert_eq!(
            build_corroboration(&db, &["kubernetes".to_string()], &[]).chain_phase,
            Some("active".to_string())
        );

        // 3 distinct days → "escalating".
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "kubernetes", &[0, 1, 2]);
        assert_eq!(
            build_corroboration(&db, &["kubernetes".to_string()], &[]).chain_phase,
            Some("escalating".to_string())
        );

        // 4+ distinct days → "peak".
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "kubernetes", &[0, 1, 2, 3]);
        assert_eq!(
            build_corroboration(&db, &["kubernetes".to_string()], &[]).chain_phase,
            Some("peak".to_string())
        );
    }

    #[test]
    fn corroboration_chain_phase_ignores_items_outside_7_day_window() {
        // Two appearances, but one is 10 days old → only one in-window day → no chain.
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "graphql", &[0, 10]);
        assert!(build_corroboration(&db, &["graphql".to_string()], &[])
            .chain_phase
            .is_none());
    }
}
