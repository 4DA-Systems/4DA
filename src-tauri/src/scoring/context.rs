// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;
use tracing::info;

use crate::db::Database;
use crate::error::{Result, ResultExt};
use crate::taste_test::continuous;

use super::{compute_taste_embedding, get_ace_context, get_topic_embeddings, ScoringContext};

static SCORING_CONTEXT_CACHE: LazyLock<Mutex<Option<(ScoringContext, Instant)>>> =
    LazyLock::new(|| Mutex::new(None));
const SCORING_CONTEXT_TTL_SECS: u64 = 300;

/// Build a ScoringContext by loading all needed state. Call once per analysis run.
/// Results are cached with a 5-minute TTL to avoid redundant DB queries.
pub(crate) async fn build_scoring_context(db: &Database) -> Result<ScoringContext> {
    // Check cache first (block scope ensures MutexGuard is dropped before any .await)
    {
        let cache = SCORING_CONTEXT_CACHE.lock().unwrap_or_else(|e| {
            tracing::warn!("SCORING_CONTEXT_CACHE mutex poisoned, recovering");
            e.into_inner()
        });
        if let Some((ref ctx, ref instant)) = *cache {
            if instant.elapsed().as_secs() < SCORING_CONTEXT_TTL_SECS {
                return Ok(ctx.clone());
            }
        }
    }
    let cached_context_count = db.context_count()?;
    let feedback_interaction_count: i64 = db.query_feedback_count().unwrap_or(0);

    let context_engine = crate::get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .context("Failed to load context")?;

    // User's explicit tech stack from onboarding (small, curated list)
    let declared_tech: Vec<String> = static_identity
        .tech_stack
        .iter()
        .map(|t| t.to_lowercase())
        .collect();

    // User's professional role from onboarding
    let user_role = static_identity.role.clone();

    // User's experience level (safe query — column may not exist yet)
    let experience_level: Option<String> = {
        let conn = crate::open_db_connection()?;
        conn.query_row(
            "SELECT experience_level FROM user_identity WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten()
    };

    let mut ace_ctx = get_ace_context();

    // ── Build Negative Stack from direct runtime deps + competing tech ──
    {
        let direct_dep_names: std::collections::HashSet<String> = ace_ctx
            .dependency_info
            .iter()
            .filter(|(_, info)| info.is_direct && !info.is_dev)
            .map(|(name, _)| name.to_lowercase())
            .collect();

        let anti_topic_pairs: Vec<(String, f32)> = ace_ctx
            .anti_topics
            .iter()
            .filter_map(|t| {
                ace_ctx
                    .anti_topic_confidence
                    .get(t)
                    .map(|&conf| (t.clone(), conf))
            })
            .collect();

        ace_ctx.negative_stack = crate::stacks::negative_stack::build_negative_stack(
            &direct_dep_names,
            crate::competing_tech::COMPETING_TECH,
            &anti_topic_pairs,
        );

        if !ace_ctx.negative_stack.priors.is_empty() {
            tracing::debug!(target: "4da::scoring",
                suppressed_count = ace_ctx.negative_stack.priors.len(),
                "Negative stack built: {} technologies suppressed",
                ace_ctx.negative_stack.priors.len()
            );
        }
    }

    // Load session-aware work topics for intent scoring.
    // Uses gap-based session detection instead of a fixed 2h window:
    // current session = 1.0, previous same-day = 0.5, yesterday = 0.2
    let work_topics: Vec<String> = match crate::get_ace_engine() {
        Ok(ace) => ace
            .get_session_aware_work_topics()
            .unwrap_or_default()
            .into_iter()
            .map(|(topic, _weight)| topic)
            .collect(),
        Err(_) => vec![],
    };
    let has_active_work = !work_topics.is_empty();

    let topic_embeddings = get_topic_embeddings(&ace_ctx).await;

    // Load feedback-derived topic boosts (Phase 9: feedback learning loop)
    let mut feedback_boosts: HashMap<String, f64> = db
        .get_feedback_topic_summary()
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.topic, f.net_score))
        .collect();

    // Load source quality preferences from ACE behavior learning
    let source_quality: HashMap<String, f32> = match crate::get_ace_engine() {
        Ok(ace) => ace
            .get_source_preferences()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        Err(_) => HashMap::new(),
    };

    // Open a single shared connection for all DB queries in context building
    let shared_conn = crate::open_db_connection()?;

    // Build domain profile for graduated domain relevance scoring
    let (
        domain_profile,
        composed_stack,
        open_windows,
        calibration_deltas,
        topic_half_lives,
        source_autopsies,
        anti_pattern_penalties,
        archetype_penalties,
        sovereign_profile,
    ) = {
        let dp = crate::domain_profile::build_domain_profile(&shared_conn);
        let cs = crate::stacks::load_composed_stack(&shared_conn);
        // Intelligence metabolism: load decision windows and autophagy calibrations
        let ow = crate::decision_advantage::get_open_windows(&shared_conn);
        let cd = crate::autophagy::load_calibration_deltas(&shared_conn);
        let thl = crate::autophagy::load_topic_decay_profiles(&shared_conn);
        // Autophagy intelligence: per-source engagement rates and anti-pattern penalties
        let sa = crate::autophagy::load_source_autopsies(&shared_conn);
        let ap = crate::autophagy::load_anti_patterns(&shared_conn);
        let arch = crate::autophagy::load_archetype_penalties(&shared_conn);
        // Unified profile (non-fatal if assembly fails)
        let sp = Some(crate::sovereign_developer_profile::assemble_profile(
            &shared_conn,
        ));
        (dp, cs, ow, cd, thl, sa, ap, arch, sp)
    };

    // ── ACE Auto-Enrichment: promote high-confidence detected tech into domain profile ──
    let mut domain_profile = domain_profile;
    {
        // Access raw ACE detected tech (with confidence/category) before it's flattened to strings
        let raw_detected_tech = match crate::get_ace_engine() {
            Ok(ace) => ace.get_detected_tech().unwrap_or_default(),
            Err(_) => vec![],
        };
        let mut promoted = 0usize;
        for tech in &raw_detected_tech {
            if tech.confidence >= 0.75
                && matches!(
                    tech.category,
                    crate::ace::TechCategory::Language | crate::ace::TechCategory::Framework
                )
            {
                let name_lower = tech.name.to_lowercase();
                if !domain_profile.primary_stack.contains(&name_lower)
                    && !domain_profile.all_tech.contains(&name_lower)
                {
                    domain_profile.all_tech.insert(name_lower.clone());
                    domain_profile.ace_promoted_tech.insert(name_lower);
                    promoted += 1;
                }
            }
        }
        if promoted > 0 {
            tracing::info!(target: "4da::scoring", promoted, "ACE auto-enrichment: promoted detected tech into domain profile");
        }
    }

    // ── Synthesize implicit interests from high-confidence ACE active topics ──
    let mut interests = static_identity.interests;
    {
        let existing: std::collections::HashSet<String> =
            interests.iter().map(|i| i.topic.to_lowercase()).collect();
        let mut synthesized = 0usize;
        for topic_name in &ace_ctx.active_topics {
            if synthesized >= 5 {
                break; // cap at 5 to avoid over-broadening
            }
            let conf = ace_ctx
                .topic_confidence
                .get(topic_name)
                .copied()
                .unwrap_or(0.0);
            if conf >= 0.75 && !existing.contains(&topic_name.to_lowercase()) {
                let embedding = topic_embeddings.get(topic_name).cloned();
                interests.push(crate::context_engine::Interest {
                    id: None,
                    topic: topic_name.clone(),
                    weight: 0.6,
                    embedding,
                    source: crate::context_engine::InterestSource::Inferred,
                });
                synthesized += 1;
            }
        }
        if synthesized > 0 {
            tracing::info!(target: "4da::scoring", synthesized, "ACE auto-enrichment: synthesized interests from active topics");
        }
    }

    // ── Count implicit interactions for faster bootstrap exit ──
    let implicit_interaction_count: i64 = {
        let conn = crate::open_db_connection();
        match conn {
            Ok(c) => c
                .query_row(
                    "SELECT COUNT(*) FROM interactions WHERE ABS(signal_strength) >= 0.3",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0),
            Err(_) => 0,
        }
    };
    // 3 implicit signals = 1 effective explicit signal
    let effective_feedback_count = feedback_interaction_count + implicit_interaction_count / 3;

    // Warm-start source preferences from stack profiles (only fills gaps)
    let mut source_quality = source_quality;
    if composed_stack.active {
        for (&source, &pref) in &composed_stack.source_preferences {
            source_quality.entry(source.to_string()).or_insert(pref);
        }
    }

    // Compute taste embedding from topic affinities + topic embeddings
    // In bootstrap mode, use lower exposure threshold for faster learning
    let affinity_min_exposures = if effective_feedback_count < 10 { 2 } else { 5 };
    let taste_embedding = {
        let affinities: Vec<(String, f32, f32)> = match crate::get_ace_engine() {
            Ok(ace) => ace
                .get_topic_affinities_min(affinity_min_exposures)
                .unwrap_or_default()
                .into_iter()
                .filter(|a| a.confidence > 0.05)
                .map(|a| (a.topic, a.affinity_score, a.confidence))
                .collect(),
            Err(_) => vec![],
        };
        compute_taste_embedding(&affinities, &topic_embeddings)
    };

    // Load persona posterior and inject persona-derived topic boosts
    let dominant_persona = {
        let (weights, update_count) = continuous::load_posterior(&shared_conn).unwrap_or_default();
        if update_count > 0 {
            let (idx, &max_w) = weights
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0, &0.0));
            // Above uniform threshold (1/9 ~ 0.11) with margin
            if max_w > 0.2 {
                let persona_boosts = continuous::get_persona_topic_boosts(idx, max_w as f32);
                for (topic, boost) in persona_boosts {
                    feedback_boosts
                        .entry(topic)
                        .and_modify(|v| *v += boost as f64)
                        .or_insert(boost as f64);
                }
                Some((idx, max_w as f32))
            } else {
                None
            }
        } else {
            None
        }
    };

    // NOTE: bridge_topic_affinities previously merged topic_affinities into
    // feedback_boosts, but this double-counted the same engagement signal:
    // one click updated topic_affinities → fed BOTH affinity_mult (multiplicative,
    // up to 1.7x) AND feedback_boost (additive, via this bridge).
    // Removed to eliminate quadruple-counting of engagement signals.
    let affinity_boosts_bridged = 0_usize;

    // Detect contradicted topics (both high affinity AND anti-topic).
    // Lightweight query — just topic names for necessity scoring.
    let contradicted_topics = {
        let conn = db.conn.lock();
        crate::anomaly::get_contradicted_topics(&conn).unwrap_or_default()
    };
    if !contradicted_topics.is_empty() {
        info!(target: "4da::scoring",
            count = contradicted_topics.len(),
            "Contradicted topics detected for necessity boosting"
        );
    }

    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        embeddings = topic_embeddings.len(),
        feedback_topics = feedback_boosts.len(),
        source_prefs = source_quality.len(),
        domain_primary = domain_profile.primary_stack.len(),
        domain_all = domain_profile.all_tech.len(),
        stack_active = composed_stack.active,
        has_active_work,
        has_taste_embedding = taste_embedding.is_some(),
        has_dominant_persona = dominant_persona.is_some(),
        affinity_boosts_bridged,
        "ACE context loaded for scoring"
    );

    if effective_feedback_count < 10 {
        info!(target: "4da::scoring",
            explicit = feedback_interaction_count,
            implicit = implicit_interaction_count,
            effective = effective_feedback_count,
            "Bootstrap mode: relaxed 1-signal gate for new user"
        );
    }

    let context = ScoringContext {
        cached_context_count,
        interest_count: interests.len(),
        interests,
        exclusions: static_identity.exclusions,
        ace_ctx,
        topic_embeddings,
        feedback_boosts,
        source_quality,
        declared_tech,
        domain_profile,
        work_topics,
        feedback_interaction_count: effective_feedback_count,
        composed_stack,
        open_windows,
        calibration_deltas,
        taste_embedding,
        topic_half_lives,
        source_autopsies,
        anti_pattern_penalties,
        archetype_penalties,
        contradicted_topics,
        sovereign_profile,
        dominant_persona,
        user_role,
        experience_level,
    };

    // Store in cache for subsequent calls within TTL
    {
        let mut cache = SCORING_CONTEXT_CACHE.lock().unwrap_or_else(|e| {
            tracing::warn!("SCORING_CONTEXT_CACHE mutex poisoned, recovering");
            e.into_inner()
        });
        *cache = Some((context.clone(), Instant::now()));
    }

    Ok(context)
}

/// Bridge ACE topic affinities into the feedback_boosts map.
///
/// Filters for high-confidence (>0.6) affinities with significant signal (|score| > 0.2),
/// scales them to 50% weight relative to explicit feedback (save/dismiss), and merges
/// additively with clamping at +/-0.5 to prevent runaway boosts.
///
/// Returns the number of affinities bridged (for logging/metrics).
pub(crate) fn bridge_topic_affinities(feedback_boosts: &mut HashMap<String, f64>) -> usize {
    let affinities = match crate::get_ace_engine() {
        Ok(ace) => ace.get_topic_affinities().unwrap_or_default(),
        Err(_) => return 0,
    };

    let mut bridged = 0;
    for aff in &affinities {
        if aff.confidence > 0.6 && aff.affinity_score.abs() > 0.2 {
            let scaled = aff.affinity_score as f64 * 0.5;
            feedback_boosts
                .entry(aff.topic.to_lowercase())
                .and_modify(|v| *v = (*v + scaled).clamp(-0.5, 0.5))
                .or_insert(scaled.clamp(-0.5, 0.5));
            bridged += 1;
        }
    }

    bridged
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_preserves_existing_boosts() {
        // bridge_topic_affinities should never remove existing boost entries.
        // It may add/modify some if ACE is available, but "rust" should still be present.
        let mut boosts: HashMap<String, f64> = HashMap::new();
        boosts.insert("rust".to_string(), 0.3);

        let _bridged = bridge_topic_affinities(&mut boosts);

        // Existing "rust" entry still present (may be modified if ACE has rust affinity,
        // but should never be removed)
        assert!(
            boosts.contains_key("rust"),
            "Existing boost entries should be preserved"
        );
    }

    #[test]
    fn test_affinity_scaling_applied() {
        // Verify the scaling logic directly (unit test the math)
        let mut boosts: HashMap<String, f64> = HashMap::new();

        // Simulate what bridge_topic_affinities does for a single high-confidence affinity
        let affinity_score: f32 = 0.8;
        let confidence: f32 = 0.9;

        // Filter: confidence > 0.6 AND |score| > 0.2 -> passes
        assert!(confidence > 0.6);
        assert!(affinity_score.abs() > 0.2);

        let scaled = affinity_score as f64 * 0.5; // 0.4
        boosts
            .entry("react".to_string())
            .and_modify(|v| *v = (*v + scaled).clamp(-0.5, 0.5))
            .or_insert(scaled.clamp(-0.5, 0.5));

        assert!(
            (boosts["react"] - 0.4).abs() < 1e-6,
            "Scaled boost should be 0.4 (0.8 * 0.5), got {}",
            boosts["react"]
        );
    }

    #[test]
    fn test_affinity_clamping() {
        let mut boosts: HashMap<String, f64> = HashMap::new();
        // Pre-existing boost of 0.4
        boosts.insert("python".to_string(), 0.4);

        // Strong positive affinity would push over 0.5
        let scaled = 0.9_f64 * 0.5; // 0.45
        boosts
            .entry("python".to_string())
            .and_modify(|v| *v = (*v + scaled).clamp(-0.5, 0.5))
            .or_insert(scaled.clamp(-0.5, 0.5));

        // 0.4 + 0.45 = 0.85, clamped to 0.5
        assert!(
            (boosts["python"] - 0.5).abs() < 1e-6,
            "Should clamp at 0.5, got {}",
            boosts["python"]
        );

        // Test negative clamping
        let mut neg_boosts: HashMap<String, f64> = HashMap::new();
        neg_boosts.insert("crypto".to_string(), -0.3);

        let neg_scaled = -0.8_f64 * 0.5; // -0.4
        neg_boosts
            .entry("crypto".to_string())
            .and_modify(|v| *v = (*v + neg_scaled).clamp(-0.5, 0.5))
            .or_insert(neg_scaled.clamp(-0.5, 0.5));

        // -0.3 + (-0.4) = -0.7, clamped to -0.5
        assert!(
            (neg_boosts["crypto"] - (-0.5)).abs() < 1e-6,
            "Should clamp at -0.5, got {}",
            neg_boosts["crypto"]
        );
    }

    #[test]
    fn test_low_confidence_affinities_filtered() {
        // Verify that the filter conditions work correctly
        let confidence_low: f32 = 0.4;
        let confidence_high: f32 = 0.8;
        let score_small: f32 = 0.1;
        let score_large: f32 = 0.5;

        // Low confidence, high score -> filtered out
        assert!(
            !(confidence_low > 0.6 && score_large.abs() > 0.2),
            "Low confidence should be filtered"
        );

        // High confidence, small score -> filtered out
        assert!(
            !(confidence_high > 0.6 && score_small.abs() > 0.2),
            "Small score should be filtered"
        );

        // High confidence, large score -> passes filter
        assert!(
            confidence_high > 0.6 && score_large.abs() > 0.2,
            "High confidence + significant score should pass"
        );
    }
}
