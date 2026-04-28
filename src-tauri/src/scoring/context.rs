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

    // ── Synthesize implicit interests from ACE-discovered context ──
    let mut interests = static_identity.interests;
    synthesize_ace_interests(&mut interests, &ace_ctx, &topic_embeddings);

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
        "ACE context loaded for scoring"
    );

    // ── Apply learned topic affinities to keyword interest weights ──
    {
        let affinities: HashMap<String, f32> = match crate::get_ace_engine() {
            Ok(ace) => ace
                .get_topic_affinities_min(affinity_min_exposures)
                .unwrap_or_default()
                .into_iter()
                .filter(|a| a.confidence > 0.3)
                .map(|a| (a.topic.to_lowercase(), a.affinity_score))
                .collect(),
            Err(_) => HashMap::new(),
        };
        apply_affinity_adjustments(&mut interests, &affinities);
    }

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

/// Synthesize ACE-discovered context into keyword interests.
///
/// Bridges ACE intelligence into keyword scoring: detected tech, active topics,
/// and key dependencies become synthetic interests so the keyword engine matches
/// content against the user's actual stack — not just onboarding declarations.
pub(crate) fn synthesize_ace_interests(
    interests: &mut Vec<crate::context_engine::Interest>,
    ace_ctx: &super::ace_context::ACEContext,
    topic_embeddings: &HashMap<String, Vec<f32>>,
) {
    let mut existing: std::collections::HashSet<String> =
        interests.iter().map(|i| i.topic.to_lowercase()).collect();

    // Phase 1: Detected tech → keyword interests (weight from tech_weights)
    // Only promote display-worthy tech (languages, major frameworks, databases).
    // ORMs, build tools, and utility libraries detected by ACE must NOT become
    // synthesized interests — they pollute feeds with off-stack content.
    let mut tech_synth = 0usize;
    for tech_name in &ace_ctx.detected_tech {
        let lower = tech_name.to_lowercase();
        if existing.contains(&lower) {
            continue;
        }
        if !crate::domain_profile::is_display_worthy(&lower) {
            continue;
        }
        let weight = ace_ctx.tech_weights.get(tech_name).copied().unwrap_or(0.4);
        let embedding = topic_embeddings.get(tech_name).cloned();
        interests.push(crate::context_engine::Interest {
            id: None,
            topic: tech_name.clone(),
            weight,
            embedding,
            source: crate::context_engine::InterestSource::Inferred,
        });
        existing.insert(lower);
        tech_synth += 1;
    }
    if tech_synth > 0 {
        tracing::info!(target: "4da::scoring", tech_synth, "ACE auto-enrichment: synthesized interests from detected tech");
    }

    // Phase 2: Active topics → keyword interests (confidence-weighted)
    let mut topic_synth = 0usize;
    for topic_name in &ace_ctx.active_topics {
        if topic_synth >= 10 {
            break;
        }
        let conf = ace_ctx
            .topic_confidence
            .get(topic_name)
            .copied()
            .unwrap_or(0.0);
        if conf >= 0.65 && !existing.contains(&topic_name.to_lowercase()) {
            let embedding = topic_embeddings.get(topic_name).cloned();
            interests.push(crate::context_engine::Interest {
                id: None,
                topic: topic_name.clone(),
                weight: 0.5 + (conf - 0.65) * 0.7, // 0.65→0.50, 1.0→0.75
                embedding,
                source: crate::context_engine::InterestSource::Inferred,
            });
            existing.insert(topic_name.to_lowercase());
            topic_synth += 1;
        }
    }
    if topic_synth > 0 {
        tracing::info!(target: "4da::scoring", topic_synth, "ACE auto-enrichment: synthesized interests from active topics");
    }

    // Phase 3: Direct dependencies → low-weight keyword interests
    // No display-worthy filter here: if a dep is in the user's actual
    // package.json, content about it IS relevant. The Phase 1 filter
    // only blocks inferred/detected tech that isn't identity-defining.
    let mut dep_synth = 0usize;
    for (dep_name, dep_info) in &ace_ctx.dependency_info {
        if dep_synth >= 15 {
            break;
        }
        if !dep_info.is_direct || dep_info.is_dev {
            continue;
        }
        let lower = dep_name.to_lowercase();
        if existing.contains(&lower) || lower.len() < 3 {
            continue;
        }
        interests.push(crate::context_engine::Interest {
            id: None,
            topic: dep_name.clone(),
            weight: 0.3,
            embedding: None,
            source: crate::context_engine::InterestSource::Inferred,
        });
        existing.insert(lower);
        dep_synth += 1;
    }
    if dep_synth > 0 {
        tracing::info!(target: "4da::scoring", dep_synth, "ACE auto-enrichment: synthesized interests from direct dependencies");
    }
}

/// Apply learned topic affinities to keyword interest weights.
/// Positive affinity (+1.0) boosts weight by up to +0.2, negative (-1.0) reduces by up to -0.2.
pub(crate) fn apply_affinity_adjustments(
    interests: &mut [crate::context_engine::Interest],
    affinities: &HashMap<String, f32>,
) {
    if affinities.is_empty() {
        return;
    }
    let mut adjusted = 0usize;
    for interest in interests.iter_mut() {
        if let Some(&affinity) = affinities.get(&interest.topic.to_lowercase()) {
            let adjustment = affinity * 0.2;
            let new_weight = (interest.weight + adjustment).clamp(0.1, 1.0);
            if (new_weight - interest.weight).abs() > 0.01 {
                interest.weight = new_weight;
                adjusted += 1;
            }
        }
    }
    if adjusted > 0 {
        tracing::debug!(target: "4da::scoring", adjusted, "Applied topic affinity adjustments to keyword interest weights");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_engine::{Interest, InterestSource};
    use crate::scoring::ace_context::ACEContext;
    use crate::scoring::dependencies::DepInfo;

    fn make_interest(topic: &str, weight: f32) -> Interest {
        Interest {
            id: None,
            topic: topic.to_string(),
            weight,
            embedding: None,
            source: InterestSource::Explicit,
        }
    }

    fn make_dep(name: &str, direct: bool, dev: bool) -> DepInfo {
        DepInfo {
            package_name: name.to_string(),
            version: None,
            is_dev: dev,
            is_direct: direct,
            search_terms: vec![],
            ecosystem: "rust".to_string(),
        }
    }

    // ── Phase 1: Detected tech synthesis ──

    #[test]
    fn test_detected_tech_becomes_interests() {
        let mut interests = vec![make_interest("Rust", 1.0)];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["typescript".into(), "react".into()];
        ace.tech_weights.insert("typescript".into(), 0.85);
        ace.tech_weights.insert("react".into(), 0.40);

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 3);
        let ts = interests.iter().find(|i| i.topic == "typescript").unwrap();
        assert!(
            (ts.weight - 0.85).abs() < 0.01,
            "primary tech should get weight 0.85"
        );
        assert_eq!(ts.source, InterestSource::Inferred);

        let react = interests.iter().find(|i| i.topic == "react").unwrap();
        assert!(
            (react.weight - 0.40).abs() < 0.01,
            "secondary tech should get weight 0.40"
        );
    }

    #[test]
    fn test_detected_tech_no_duplicates() {
        let mut interests = vec![make_interest("Rust", 1.0)];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["rust".into()]; // same as existing (case-insensitive)

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 1, "should not duplicate existing interest");
    }

    #[test]
    fn test_detected_tech_default_weight() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["python".into()];
        // No tech_weights entry → should default to 0.4

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 1);
        assert!(
            (interests[0].weight - 0.4).abs() < 0.01,
            "no tech_weight → default 0.4"
        );
    }

    // ── Phase 2: Active topic synthesis ──

    #[test]
    fn test_active_topics_high_confidence() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.active_topics = vec!["docker".into(), "kubernetes".into()];
        ace.topic_confidence.insert("docker".into(), 0.90);
        ace.topic_confidence.insert("kubernetes".into(), 0.65);

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 2);
        let docker = interests.iter().find(|i| i.topic == "docker").unwrap();
        // weight = 0.5 + (0.90 - 0.65) * 0.7 = 0.5 + 0.175 = 0.675
        assert!(
            (docker.weight - 0.675).abs() < 0.01,
            "docker weight should be ~0.675, got {}",
            docker.weight
        );

        let k8s = interests.iter().find(|i| i.topic == "kubernetes").unwrap();
        // weight = 0.5 + (0.65 - 0.65) * 0.7 = 0.5
        assert!(
            (k8s.weight - 0.5).abs() < 0.01,
            "kubernetes weight should be 0.5, got {}",
            k8s.weight
        );
    }

    #[test]
    fn test_active_topics_below_threshold_ignored() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.active_topics = vec!["lowconf".into()];
        ace.topic_confidence.insert("lowconf".into(), 0.50); // below 0.65

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            0,
            "low-confidence topics should not be synthesized"
        );
    }

    #[test]
    fn test_active_topics_cap_at_10() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        for i in 0..15 {
            let name = format!("topic_{i}");
            ace.active_topics.push(name.clone());
            ace.topic_confidence.insert(name, 0.80);
        }

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 10, "should cap at 10 topic interests");
    }

    // ── Phase 3: Dependency synthesis ──

    #[test]
    fn test_direct_deps_become_interests() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("tokio".into(), make_dep("tokio", true, false));
        ace.dependency_info
            .insert("serde".into(), make_dep("serde", true, false));

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 2);
        for i in &interests {
            assert!((i.weight - 0.3).abs() < 0.01, "deps should have weight 0.3");
            assert_eq!(i.source, InterestSource::Inferred);
        }
    }

    #[test]
    fn test_dev_deps_excluded() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("insta".into(), make_dep("insta", true, true)); // dev dep

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 0, "dev deps should not be synthesized");
    }

    #[test]
    fn test_transitive_deps_excluded() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("syn".into(), make_dep("syn", false, false)); // transitive

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            0,
            "transitive deps should not be synthesized"
        );
    }

    #[test]
    fn test_short_dep_names_excluded() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("cc".into(), make_dep("cc", true, false)); // too short

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            0,
            "deps with name < 3 chars should be excluded"
        );
    }

    #[test]
    fn test_dep_cap_at_15() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        for i in 0..20 {
            let name = format!("package-{i:02}");
            ace.dependency_info
                .insert(name.clone(), make_dep(&name, true, false));
        }

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert!(
            interests.len() <= 15,
            "should cap at 15 dep interests, got {}",
            interests.len()
        );
    }

    // ── Cross-phase deduplication ──

    #[test]
    fn test_no_cross_phase_duplicates() {
        let mut interests = vec![make_interest("tokio", 1.0)];
        let mut ace = ACEContext::default();
        // tokio appears in all three phases
        ace.detected_tech = vec!["tokio".into()];
        ace.tech_weights.insert("tokio".into(), 0.85);
        ace.active_topics = vec!["tokio".into()];
        ace.topic_confidence.insert("tokio".into(), 0.90);
        ace.dependency_info
            .insert("tokio".into(), make_dep("tokio", true, false));

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(interests.len(), 1, "tokio should appear exactly once");
        assert!(
            (interests[0].weight - 1.0).abs() < 0.01,
            "original explicit weight preserved"
        );
    }

    // ── Affinity adjustments ──

    #[test]
    fn test_positive_affinity_boosts_weight() {
        let mut interests = vec![make_interest("Rust", 0.6)];
        let mut affinities = HashMap::new();
        affinities.insert("rust".to_string(), 0.8); // strong positive

        apply_affinity_adjustments(&mut interests, &affinities);

        // 0.6 + 0.8 * 0.2 = 0.6 + 0.16 = 0.76
        assert!(
            (interests[0].weight - 0.76).abs() < 0.01,
            "weight should be ~0.76, got {}",
            interests[0].weight
        );
    }

    #[test]
    fn test_negative_affinity_reduces_weight() {
        let mut interests = vec![make_interest("Java", 0.6)];
        let mut affinities = HashMap::new();
        affinities.insert("java".to_string(), -1.0); // strong negative

        apply_affinity_adjustments(&mut interests, &affinities);

        // 0.6 + (-1.0) * 0.2 = 0.6 - 0.2 = 0.4
        assert!(
            (interests[0].weight - 0.4).abs() < 0.01,
            "weight should be ~0.4, got {}",
            interests[0].weight
        );
    }

    #[test]
    fn test_affinity_clamps_to_minimum() {
        let mut interests = vec![make_interest("Cobol", 0.15)];
        let mut affinities = HashMap::new();
        affinities.insert("cobol".to_string(), -1.0);

        apply_affinity_adjustments(&mut interests, &affinities);

        // 0.15 - 0.2 = -0.05 → clamped to 0.1
        assert!(
            (interests[0].weight - 0.1).abs() < 0.01,
            "weight should clamp to 0.1, got {}",
            interests[0].weight
        );
    }

    #[test]
    fn test_affinity_clamps_to_maximum() {
        let mut interests = vec![make_interest("Rust", 0.95)];
        let mut affinities = HashMap::new();
        affinities.insert("rust".to_string(), 1.0);

        apply_affinity_adjustments(&mut interests, &affinities);

        // 0.95 + 0.2 = 1.15 → clamped to 1.0
        assert!(
            (interests[0].weight - 1.0).abs() < 0.01,
            "weight should clamp to 1.0, got {}",
            interests[0].weight
        );
    }

    #[test]
    fn test_no_affinity_leaves_weight_unchanged() {
        let mut interests = vec![make_interest("Rust", 0.7)];
        let affinities = HashMap::new(); // empty

        apply_affinity_adjustments(&mut interests, &affinities);

        assert!(
            (interests[0].weight - 0.7).abs() < 0.001,
            "weight should be unchanged"
        );
    }

    // ── End-to-end: ACE synthesis → keyword scoring ──

    #[test]
    fn test_e2e_synthesized_interest_improves_keyword_score() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        // Without ACE: only explicit "Rust" interest
        let explicit_only = vec![make_interest("Rust", 1.0)];
        let score_without = compute_keyword_interest_score(
            "New TypeScript 5.5 features for React developers",
            "TypeScript introduces new type guards and React Server Components support",
            &explicit_only,
        );

        // With ACE: "Rust" + synthesized "typescript" and "react"
        let mut with_ace = vec![make_interest("Rust", 1.0)];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["typescript".into(), "react".into()];
        ace.tech_weights.insert("typescript".into(), 0.85);
        ace.tech_weights.insert("react".into(), 0.40);
        synthesize_ace_interests(&mut with_ace, &ace, &HashMap::new());

        let score_with = compute_keyword_interest_score(
            "New TypeScript 5.5 features for React developers",
            "TypeScript introduces new type guards and React Server Components support",
            &with_ace,
        );

        assert!(
            score_with > score_without + 0.1,
            "ACE synthesis should significantly improve score for stack-relevant content: \
             without={score_without:.3}, with={score_with:.3}"
        );
        assert_eq!(
            score_without, 0.0,
            "explicit Rust interest should not match TypeScript content"
        );
        assert!(
            score_with > 0.5,
            "synthesized interests should produce a strong match, got {score_with:.3}"
        );
    }

    #[test]
    fn test_e2e_dep_synthesis_catches_dependency_content() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 1.0)];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("tokio".into(), make_dep("tokio", true, false));
        ace.dependency_info
            .insert("serde".into(), make_dep("serde", true, false));
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score = compute_keyword_interest_score(
            "Tokio 2.0 release brings major async runtime improvements",
            "The tokio async runtime for Rust gets a major update with new scheduler",
            &interests,
        );

        assert!(
            score > 0.2,
            "content about a direct dependency should score, got {score:.3}"
        );
    }

    #[test]
    fn test_e2e_affinity_changes_keyword_ranking() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 0.5), make_interest("Python", 0.5)];

        // User engages heavily with Rust, dismisses Python
        let mut affinities = HashMap::new();
        affinities.insert("rust".to_string(), 0.9);
        affinities.insert("python".to_string(), -0.8);
        apply_affinity_adjustments(&mut interests, &affinities);

        let rust_score = compute_keyword_interest_score(
            "Advanced Rust async patterns",
            "Exploring async Rust with tokio and futures",
            &interests,
        );
        let python_score = compute_keyword_interest_score(
            "Advanced Python async patterns",
            "Exploring async Python with asyncio and aiohttp",
            &interests,
        );

        assert!(
            rust_score > python_score,
            "Rust (positive affinity) should rank higher than Python (negative): \
             rust={rust_score:.3}, python={python_score:.3}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 1: False Positive Resistance
    // Common dependency names are also common English words. Synthesized dep
    // interests must NOT inflate scores for unrelated content.
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_common_dep_names_dont_match_unrelated_content() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![];
        let mut ace = ACEContext::default();
        for dep in ["log", "time", "rand", "base64", "regex"] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        // Content carefully chosen to avoid substrings of dep names
        // (e.g., "technology" contains "log", so we avoid it)
        let score_privacy = compute_keyword_interest_score(
            "New EU privacy rules announced for 2026",
            "The European Union has announced sweeping privacy reforms that will affect all companies operating in the bloc.",
            &interests,
        );
        let score_salary = compute_keyword_interest_score(
            "How to negotiate a better salary",
            "Career experts share their tips for getting a raise at your next performance review.",
            &interests,
        );

        assert_eq!(
            score_privacy, 0.0,
            "deps [log, time, rand, base64, regex] must not match privacy content \
             — users would see irrelevant policy articles; got {score_privacy:.3}"
        );
        assert_eq!(
            score_salary, 0.0,
            "deps [log, time, rand, base64, regex] must not match salary negotiation content \
             — users would see irrelevant career advice; got {score_salary:.3}"
        );
    }

    #[test]
    fn test_dep_log_doesnt_match_changelog() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("log".to_string(), make_dep("log", true, false));
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score = compute_keyword_interest_score(
            "Changelog: what's new in our March update",
            "We've updated our privacy log and compliance documentation with the latest regulatory requirements.",
            &interests,
        );

        // "log" appears in "privacy log" content — it's a genuine substring hit.
        // Weight 0.3 keeps the final score bounded, but it won't be zero.
        // The key invariant: even with a direct content hit, dep weight caps the damage.
        assert!(
            score < 0.25,
            "dep 'log' at weight 0.3 must keep changelog content scores low \
             — the low dep weight is the safety net against common-word hits; got {score:.3}"
        );
    }

    #[test]
    fn test_dep_time_doesnt_false_match() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("time".to_string(), make_dep("time", true, false));
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score = compute_keyword_interest_score(
            "Time management tips for remote developers",
            "Discover productivity strategies for managing your schedule while working from home.",
            &interests,
        );

        // "time" is a common word that appears in the title. The dep weight 0.3
        // keeps the final score low even with a title-level hit.
        assert!(
            score < 0.25,
            "dep 'time' at weight 0.3 matching 'time management' must stay low \
             — the article is about productivity, not the Rust time crate; got {score:.3}"
        );
    }

    #[test]
    fn test_generic_dep_names_filtered_by_length() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        for dep in ["cc", "ws", "fs"] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            0,
            "deps with names < 3 chars ('cc', 'ws', 'fs') must not be synthesized \
             — they are too ambiguous to use as keyword interests; got {} interests",
            interests.len()
        );
    }

    #[test]
    fn test_short_but_valid_deps_synthesize() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        for dep in ["log", "url", "syn"] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            3,
            "deps with names >= 3 chars ('log', 'url', 'syn') must be synthesized \
             — they are real crate names with enough specificity; got {} interests",
            interests.len()
        );
        for name in ["log", "url", "syn"] {
            assert!(
                interests.iter().any(|i| i.topic == name),
                "dep '{name}' should be present in synthesized interests"
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 2: Score Dilution Resistance
    // Many synthesized interests must not wash out explicit interest matches.
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_explicit_interest_dominates_synthesized() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        // Baseline: explicit Rust only
        let explicit_only = vec![make_interest("Rust", 1.0)];
        let score_explicit = compute_keyword_interest_score(
            "Advanced Rust async patterns",
            "Exploring async Rust with tokio and futures for high-performance systems",
            &explicit_only,
        );

        // Now add 10 synthesized tech + 15 deps alongside the explicit interest
        let mut interests = vec![make_interest("Rust", 1.0)];
        let mut ace = ACEContext::default();
        for tech in [
            "python", "java", "go", "csharp", "ruby", "swift", "kotlin", "scala", "haskell",
            "elixir",
        ] {
            ace.detected_tech.push(tech.to_string());
        }
        for dep in [
            "serde",
            "tokio",
            "axum",
            "sqlx",
            "tracing",
            "anyhow",
            "thiserror",
            "clap",
            "reqwest",
            "hyper",
            "tower",
            "bytes",
            "futures",
            "parking_lot",
            "dashmap",
        ] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score_with_noise = compute_keyword_interest_score(
            "Advanced Rust async patterns",
            "Exploring async Rust with tokio and futures for high-performance systems",
            &interests,
        );

        // compute_keyword_interest_score takes MAX across interests, not average.
        // The explicit Rust@1.0 interest should dominate regardless of the 25 others.
        assert!(
            (score_with_noise - score_explicit).abs() < 0.001,
            "explicit Rust@1.0 must dominate even with 25+ synthesized interests \
             — MAX-based scoring prevents dilution; explicit={score_explicit:.3}, \
             with_noise={score_with_noise:.3}"
        );
    }

    #[test]
    fn test_synthesized_interests_dont_dilute_specificity() {
        use crate::scoring::keywords::best_interest_specificity_weight;

        // 6+ interests triggers full specificity logic
        let interests = vec![
            make_interest("Rust", 1.0),
            make_interest("TypeScript", 0.8),
            make_interest("Docker", 0.7),
            make_interest("Security", 0.6),
            // These are broad terms (in BROAD_INTEREST_TERMS)
            Interest {
                id: None,
                topic: "web".to_string(),
                weight: 0.4,
                embedding: None,
                source: InterestSource::Inferred,
            },
            Interest {
                id: None,
                topic: "development".to_string(),
                weight: 0.3,
                embedding: None,
                source: InterestSource::Inferred,
            },
        ];

        let specificity = best_interest_specificity_weight(
            "New web development framework launches",
            "A new web framework promises to simplify development workflows for modern apps.",
            &interests,
        );

        // "web" and "development" are both BROAD_INTEREST_TERMS. With 6 interests,
        // full specificity logic applies (0.25 for broad terms).
        assert!(
            specificity <= 0.25,
            "broad synthesized topics ('web', 'development') with 6+ interests must \
             receive the broad penalty (0.25), not 1.0; got {specificity:.2}"
        );
    }

    #[test]
    fn test_max_synthesis_load_doesnt_break_scoring() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![
            make_interest("Rust", 1.0),
            make_interest("TypeScript", 0.8),
            make_interest("GraphQL", 0.7),
        ];

        let mut ace = ACEContext::default();
        // 5 tech
        for tech in ["python", "java", "go", "csharp", "ruby"] {
            ace.detected_tech.push(tech.to_string());
        }
        // 10 topics with high confidence
        for (i, topic) in [
            "docker",
            "kubernetes",
            "terraform",
            "ci-cd",
            "observability",
            "microservices",
            "grpc",
            "protobuf",
            "service-mesh",
            "istio",
        ]
        .iter()
        .enumerate()
        {
            ace.active_topics.push(topic.to_string());
            ace.topic_confidence
                .insert(topic.to_string(), 0.65 + (i as f32) * 0.03);
        }
        // 15 deps
        for dep in [
            "serde",
            "tokio",
            "axum",
            "sqlx",
            "tracing",
            "anyhow",
            "thiserror",
            "clap",
            "reqwest",
            "hyper",
            "tower",
            "bytes",
            "futures",
            "parking_lot",
            "dashmap",
        ] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        // Should have 3 explicit + up to 5 tech + 10 topics + 15 deps = up to 33
        assert!(
            interests.len() >= 20,
            "high synthesis load should produce many interests; got {}",
            interests.len()
        );

        let score = compute_keyword_interest_score(
            "Building a REST API with Rust and Axum",
            "Learn how to build production-ready APIs using Rust's Axum framework with async handlers.",
            &interests,
        );

        assert!(
            score >= 0.0 && score <= 1.0,
            "score must be in [0.0, 1.0] even with {0} interests; got {score:.3}",
            interests.len()
        );
    }

    #[test]
    fn test_synthesized_low_weight_cant_outscore_explicit() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 0.8)];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("tokio".to_string(), make_dep("tokio", true, false));
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let tokio_score = compute_keyword_interest_score(
            "Tokio 2.0 release",
            "The tokio async runtime gets a major update with new features and improvements.",
            &interests,
        );
        let rust_score = compute_keyword_interest_score(
            "Rust 2024 edition",
            "The Rust programming language announces the 2024 edition with major improvements.",
            &interests,
        );

        // tokio is a dep at 0.3, Rust is explicit at 0.8.
        // Even with a strong match, tokio's low weight caps its score.
        assert!(
            rust_score > tokio_score,
            "explicit Rust@0.8 must outscore synthesized tokio@0.3 \
             — weight hierarchy must hold; rust={rust_score:.3}, tokio={tokio_score:.3}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 3: Weight Hierarchy Correctness
    // tech_weight > topic_weight > dep_weight for the same content match
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_weight_hierarchy_tech_gt_topic_gt_dep() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let title = "Tokio async runtime improvements";
        let content = "The tokio library for Rust async programming receives performance improvements and new scheduling features.";

        // Tier 1: tech interest at 0.85 weight
        let tech_interests = vec![Interest {
            id: None,
            topic: "tokio".to_string(),
            weight: 0.85,
            embedding: None,
            source: InterestSource::Inferred,
        }];
        let tech_score = compute_keyword_interest_score(title, content, &tech_interests);

        // Tier 2: topic interest at ~0.60 weight
        let topic_interests = vec![Interest {
            id: None,
            topic: "tokio".to_string(),
            weight: 0.60,
            embedding: None,
            source: InterestSource::Inferred,
        }];
        let topic_score = compute_keyword_interest_score(title, content, &topic_interests);

        // Tier 3: dep interest at 0.30 weight
        let dep_interests = vec![Interest {
            id: None,
            topic: "tokio".to_string(),
            weight: 0.30,
            embedding: None,
            source: InterestSource::Inferred,
        }];
        let dep_score = compute_keyword_interest_score(title, content, &dep_interests);

        assert!(
            tech_score > topic_score,
            "tech weight (0.85) must produce higher score than topic weight (0.60) \
             for same content; tech={tech_score:.3}, topic={topic_score:.3}"
        );
        assert!(
            topic_score > dep_score,
            "topic weight (0.60) must produce higher score than dep weight (0.30) \
             for same content; topic={topic_score:.3}, dep={dep_score:.3}"
        );
    }

    #[test]
    fn test_affinity_adjusted_weight_ordering() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 0.5), make_interest("Python", 0.5)];

        // Positive affinity on Rust, negative on Python
        let mut affinities = HashMap::new();
        affinities.insert("rust".to_string(), 0.9); // +0.9 * 0.2 = +0.18 → 0.68
        affinities.insert("python".to_string(), -0.8); // -0.8 * 0.2 = -0.16 → 0.34
        apply_affinity_adjustments(&mut interests, &affinities);

        let rust_interest = interests.iter().find(|i| i.topic == "Rust").unwrap();
        let python_interest = interests.iter().find(|i| i.topic == "Python").unwrap();

        assert!(
            rust_interest.weight > python_interest.weight,
            "positive affinity must raise Rust above Python; \
             rust_w={:.3}, python_w={:.3}",
            rust_interest.weight,
            python_interest.weight
        );

        let rust_score = compute_keyword_interest_score(
            "Rust async patterns",
            "Exploring async Rust programming with tokio",
            &interests,
        );
        let python_score = compute_keyword_interest_score(
            "Python async patterns",
            "Exploring async Python programming with asyncio",
            &interests,
        );

        assert!(
            rust_score > python_score,
            "Rust content must outscore Python content after affinity adjustment; \
             rust={rust_score:.3}, python={python_score:.3}"
        );
    }

    #[test]
    fn test_phase_weight_formula_correctness() {
        // Phase 2 formula: weight = 0.5 + (conf - 0.65) * 0.7
        // Test at boundary values
        let mut ace = ACEContext::default();
        ace.active_topics = vec![
            "topic_a".to_string(),
            "topic_b".to_string(),
            "topic_c".to_string(),
        ];
        ace.topic_confidence.insert("topic_a".into(), 0.65);
        ace.topic_confidence.insert("topic_b".into(), 0.80);
        ace.topic_confidence.insert("topic_c".into(), 1.0);

        let mut interests = vec![];
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let a = interests.iter().find(|i| i.topic == "topic_a").unwrap();
        let b = interests.iter().find(|i| i.topic == "topic_b").unwrap();
        let c = interests.iter().find(|i| i.topic == "topic_c").unwrap();

        // conf=0.65 → 0.5 + (0.65-0.65)*0.7 = 0.50
        assert!(
            (a.weight - 0.50).abs() < 0.001,
            "conf=0.65 must yield weight 0.50; got {:.4}",
            a.weight
        );
        // conf=0.80 → 0.5 + (0.80-0.65)*0.7 = 0.5 + 0.105 = 0.605
        assert!(
            (b.weight - 0.605).abs() < 0.001,
            "conf=0.80 must yield weight 0.605; got {:.4}",
            b.weight
        );
        // conf=1.0 → 0.5 + (1.0-0.65)*0.7 = 0.5 + 0.245 = 0.745
        assert!(
            (c.weight - 0.745).abs() < 0.001,
            "conf=1.0 must yield weight 0.745; got {:.4}",
            c.weight
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 4: Negation + Synthesis Interaction
    // Negated terms in synthesized interests must reduce scores properly.
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_negated_synthesized_interest_reduces_score() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        // Synthesize "react" from detected tech
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.detected_tech.push("react".to_string());
        ace.tech_weights.insert("react".into(), 0.4);
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let positive_score = compute_keyword_interest_score(
            "Getting started with React",
            "React hooks tutorial for beginners building modern user interfaces.",
            &interests,
        );
        let negated_score = compute_keyword_interest_score(
            "Why we stopped using React",
            "We don't use react anymore, moving to Vue for better performance.",
            &interests,
        );

        assert!(
            positive_score > 0.0,
            "positive React content must score > 0 with synthesized react interest; \
             got {positive_score:.3}"
        );
        assert!(
            negated_score <= positive_score * 0.60,
            "negated React content ('stopped using React') must score at least 40% lower \
             than positive content — negation detection is a production safety net; \
             positive={positive_score:.3}, negated={negated_score:.3}"
        );
    }

    #[test]
    fn test_negation_doesnt_affect_other_interests() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 1.0)];
        let mut ace = ACEContext::default();
        ace.detected_tech.push("react".to_string());
        ace.tech_weights.insert("react".into(), 0.4);
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score = compute_keyword_interest_score(
            "Rust developers moving away from react",
            "Many Rust developers are migrating from react to other frontend solutions.",
            &interests,
        );

        // Rust should still match normally via the explicit interest at weight 1.0.
        // React is negated ("moving away from react") but that's a separate interest.
        assert!(
            score > 0.3,
            "explicit Rust@1.0 must still score well even when react is negated \
             in the same content — negation is per-interest, not global; got {score:.3}"
        );
    }

    #[test]
    fn test_migrating_from_pattern_is_negative() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.detected_tech.push("django".to_string());
        ace.tech_weights.insert("django".into(), 0.4);
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score = compute_keyword_interest_score(
            "Migrating from Django to FastAPI: a complete guide",
            "Learn how to migrate your application from django to fastapi step by step.",
            &interests,
        );

        // "migrating from django" triggers negation on content occurrence.
        // However title also contains "Django" which gets 0.80 base * weight 0.4.
        // Negation applies per-occurrence in content, but the title hit dominates.
        // The key invariant: negated content produces a LOWER score than non-negated.
        let non_negated_score = compute_keyword_interest_score(
            "Django best practices for production",
            "Learn django patterns for building scalable production web applications.",
            &interests,
        );
        assert!(
            score < non_negated_score,
            "content about migrating FROM django must score lower than positive django content \
             — 'migrating from' is a negation signal; negated={score:.3}, positive={non_negated_score:.3}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 5: Specificity + Synthesis Interaction
    // Broad synthesized topics must receive specificity penalties correctly.
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_broad_synthesized_topic_gets_specificity_penalty() {
        use crate::scoring::keywords::best_interest_specificity_weight;

        // "web" and "development" are both in BROAD_INTEREST_TERMS.
        // Need 6+ interests for full specificity logic.
        let interests = vec![
            make_interest("Rust", 1.0),
            make_interest("TypeScript", 0.8),
            make_interest("Docker", 0.7),
            make_interest("Kubernetes", 0.6),
            make_interest("GraphQL", 0.5),
            Interest {
                id: None,
                topic: "web development".to_string(),
                weight: 0.4,
                embedding: None,
                source: InterestSource::Inferred,
            },
        ];

        let specificity = best_interest_specificity_weight(
            "New web development trends for 2026",
            "The latest web development frameworks and tools shaping the industry.",
            &interests,
        );

        // "web development" contains "web" and "development", both broad.
        // With 6 interests, broad penalty = 0.25.
        assert!(
            specificity <= 0.25,
            "broad synthesized topic 'web development' with 6+ interests must get \
             specificity penalty (0.25); got {specificity:.2}"
        );
    }

    #[test]
    fn test_specific_synthesized_topic_no_penalty() {
        use crate::scoring::keywords::interest_specificity_weight;

        // "Tauri plugins" is multi-word and not in BROAD_INTEREST_TERMS
        let w = interest_specificity_weight("Tauri plugins");
        assert!(
            (w - 1.0).abs() < 0.001,
            "multi-word specific topic 'Tauri plugins' must get full specificity weight \
             (1.0), not a broad penalty; got {w:.2}"
        );
    }

    #[test]
    fn test_single_word_synthesized_gets_moderate_penalty() {
        use crate::scoring::keywords::interest_specificity_weight;

        // "kubernetes" is single-word and NOT in BROAD_INTEREST_TERMS
        let w = interest_specificity_weight("kubernetes");
        assert!(
            (w - 0.60).abs() < 0.001,
            "single non-broad word 'kubernetes' must get moderate specificity (0.60); \
             got {w:.2}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 6: Realistic Content Scenarios
    // End-to-end tests with real-world-like developer profiles.
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_realistic_rust_developer_profile() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 1.0), make_interest("TypeScript", 0.7)];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["react".into(), "tailwindcss".into()];
        ace.tech_weights.insert("react".into(), 0.4);
        ace.tech_weights.insert("tailwindcss".into(), 0.4);
        for dep in ["tokio", "serde", "axum", "sqlx"] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score_axum = compute_keyword_interest_score(
            "Axum 0.8 release with improved extractors",
            "The Axum web framework for Rust receives major updates to its extractor system.",
            &interests,
        );
        let score_react = compute_keyword_interest_score(
            "React Server Components deep dive",
            "Understanding React server components and their impact on modern web development.",
            &interests,
        );
        let score_rust_tokio = compute_keyword_interest_score(
            "Rust async patterns with Tokio",
            "Deep dive into async Rust programming with the tokio runtime and futures.",
            &interests,
        );
        let score_python = compute_keyword_interest_score(
            "Python Django tutorial for beginners",
            "Learn how to build web applications with Python and the Django framework.",
            &interests,
        );
        let score_css = compute_keyword_interest_score(
            "Understanding CSS Grid layouts",
            "A comprehensive guide to CSS Grid for responsive web design layouts.",
            &interests,
        );

        assert!(
            score_axum > 0.0,
            "axum dep should match axum content; got {score_axum:.3}"
        );
        assert!(
            score_react > 0.0,
            "detected react tech should match React content; got {score_react:.3}"
        );
        assert!(
            score_rust_tokio > score_react,
            "Rust+Tokio (explicit@1.0 + dep) must outscore React (tech@0.4); \
             rust_tokio={score_rust_tokio:.3}, react={score_react:.3}"
        );
        assert!(
            score_rust_tokio > score_axum,
            "Rust+Tokio (explicit@1.0) must outscore Axum (dep@0.3); \
             rust_tokio={score_rust_tokio:.3}, axum={score_axum:.3}"
        );
        assert_eq!(
            score_python, 0.0,
            "Python/Django must score 0.0 — not in profile; got {score_python:.3}"
        );
        assert_eq!(
            score_css, 0.0,
            "CSS Grid must score 0.0 — not in profile; got {score_css:.3}"
        );
    }

    #[test]
    fn test_realistic_frontend_developer_profile() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![
            make_interest("TypeScript", 1.0),
            make_interest("React", 0.9),
        ];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["nextjs".into(), "tailwindcss".into()];
        ace.tech_weights.insert("nextjs".into(), 0.4);
        ace.tech_weights.insert("tailwindcss".into(), 0.4);
        for dep in ["zod", "tanstack-query", "prisma"] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let score_nextjs = compute_keyword_interest_score(
            "Next.js 15 App Router migration guide",
            "Learn how to migrate your Next.js application to the new App Router architecture.",
            &interests,
        );
        // Avoid mentioning "TypeScript" in Zod content to isolate the dep match
        let score_zod = compute_keyword_interest_score(
            "Zod schema validation patterns",
            "Advanced patterns for validating data with Zod in your applications.",
            &interests,
        );
        let score_rust = compute_keyword_interest_score(
            "Rust ownership and borrowing explained",
            "Understanding Rust memory management through ownership and borrowing rules.",
            &interests,
        );

        assert!(
            score_nextjs > 0.0,
            "Next.js content must match for frontend dev profile; got {score_nextjs:.3}"
        );
        assert!(
            score_zod > 0.0,
            "Zod content must match via dep synthesis; got {score_zod:.3}"
        );
        assert_eq!(
            score_rust, 0.0,
            "Rust content must not match for pure frontend profile; got {score_rust:.3}"
        );
        assert!(
            score_nextjs > score_zod,
            "Next.js (tech@0.4) must outscore Zod (dep@0.3) when Zod content \
             doesn't cross-match explicit interests; \
             nextjs={score_nextjs:.3}, zod={score_zod:.3}"
        );
    }

    #[test]
    fn test_realistic_empty_ace_context_graceful() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 1.0), make_interest("TypeScript", 0.8)];
        let ace = ACEContext::default();
        let before_len = interests.len();
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            before_len,
            "empty ACEContext must not add any interests; \
             before={before_len}, after={}",
            interests.len()
        );

        let score = compute_keyword_interest_score(
            "Advanced Rust patterns",
            "Systems programming with Rust for high performance applications.",
            &interests,
        );
        assert!(
            score > 0.0,
            "scoring must still work normally with empty ACE context; got {score:.3}"
        );
    }

    #[test]
    fn test_realistic_ace_only_no_explicit() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["rust".into(), "python".into(), "docker".into()];
        ace.tech_weights.insert("rust".into(), 0.85);
        ace.tech_weights.insert("python".into(), 0.40);
        ace.tech_weights.insert("docker".into(), 0.40);
        for dep in ["tokio", "serde", "axum", "clap", "reqwest"] {
            ace.dependency_info
                .insert(dep.to_string(), make_dep(dep, true, false));
        }
        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert!(
            interests.len() >= 7,
            "ACE-only should produce at least 3 tech + 5 dep interests (minus any dupes); \
             got {}",
            interests.len()
        );

        let score_matching = compute_keyword_interest_score(
            "Rust and Tokio performance tuning",
            "Optimizing async Rust applications using tokio runtime configuration.",
            &interests,
        );
        let score_unrelated = compute_keyword_interest_score(
            "Gardening tips for spring",
            "Plant your vegetables early and water them regularly for best results.",
            &interests,
        );

        assert!(
            score_matching > 0.0,
            "ACE-synthesized interests must score matching content; got {score_matching:.3}"
        );
        assert_eq!(
            score_unrelated, 0.0,
            "ACE-synthesized interests must not match unrelated content; got {score_unrelated:.3}"
        );
    }

    #[test]
    fn test_affinity_reversal_changes_ranking() {
        use crate::scoring::keywords::compute_keyword_interest_score;

        let mut interests = vec![make_interest("Rust", 0.5), make_interest("Go", 0.5)];

        // Before affinity: both equal weight
        let rust_before = compute_keyword_interest_score(
            "Rust systems programming",
            "Build high-performance systems with Rust and its ownership model.",
            &interests,
        );
        let go_before = compute_keyword_interest_score(
            "Go systems programming",
            "Build high-performance systems with Go and its goroutine model.",
            &interests,
        );

        // After affinity: Rust boosted, Go suppressed
        let mut affinities = HashMap::new();
        affinities.insert("rust".to_string(), 1.0); // +0.2 → 0.7
        affinities.insert("go".to_string(), -1.0); // -0.2 → 0.3
        apply_affinity_adjustments(&mut interests, &affinities);

        let rust_after = compute_keyword_interest_score(
            "Rust systems programming",
            "Build high-performance systems with Rust and its ownership model.",
            &interests,
        );
        let go_after = compute_keyword_interest_score(
            "Go systems programming",
            "Build high-performance systems with Go and its goroutine model.",
            &interests,
        );

        assert!(
            rust_after > go_after,
            "after affinity reversal, Rust must outscore Go; \
             rust_after={rust_after:.3}, go_after={go_after:.3}"
        );
        assert!(
            rust_after > rust_before,
            "positive affinity must increase Rust score; \
             before={rust_before:.3}, after={rust_after:.3}"
        );
        assert!(
            go_after < go_before,
            "negative affinity must decrease Go score; \
             before={go_before:.3}, after={go_after:.3}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Group 7: Edge Cases & Robustness
    // The system must never panic or corrupt state under odd inputs.
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_synthesis_with_unicode_tech_names() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        // Use Phase 3 deps for unicode test — Phase 1 detected_tech applies
        // the display-worthy filter which would reject unknown names.
        ace.dependency_info
            .insert("café-lib".to_string(), make_dep("café-lib", true, false));
        ace.dependency_info.insert(
            "naïve-bayes".to_string(),
            make_dep("naïve-bayes", true, false),
        );

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            2,
            "unicode dep names must synthesize without panic; got {} interests",
            interests.len()
        );
        assert!(
            interests.iter().any(|i| i.topic == "café-lib"),
            "unicode dep name 'café-lib' must be preserved exactly"
        );
        assert!(
            interests.iter().any(|i| i.topic == "naïve-bayes"),
            "unicode dep name 'naïve-bayes' must be preserved exactly"
        );
    }

    #[test]
    fn test_empty_string_dep_name_handled() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        ace.dependency_info
            .insert("".to_string(), make_dep("", true, false));

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests.len(),
            0,
            "empty-string dep name must not synthesize (length 0 < 3); got {} interests",
            interests.len()
        );
    }

    #[test]
    fn test_duplicate_deps_across_phases() {
        let mut interests = vec![];
        let mut ace = ACEContext::default();
        // "rust" appears in both detected_tech AND dependency_info
        // (display-worthy, so Phase 1 accepts it)
        ace.detected_tech.push("rust".to_string());
        ace.tech_weights.insert("rust".into(), 0.85);
        ace.dependency_info
            .insert("rust".to_string(), make_dep("rust", true, false));

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        let rust_count = interests
            .iter()
            .filter(|i| i.topic.to_lowercase() == "rust")
            .count();
        assert_eq!(
            rust_count, 1,
            "rust appearing in both detected_tech and dependency_info must produce \
             exactly 1 interest (dedup by lowercase); got {rust_count}"
        );
        // Should keep the tech version (first phase wins) with weight 0.85
        let rust_i = interests.iter().find(|i| i.topic == "rust").unwrap();
        assert!(
            (rust_i.weight - 0.85).abs() < 0.01,
            "deduped rust must keep Phase 1 (tech) weight 0.85, not Phase 3 (dep) 0.3; \
             got {:.3}",
            rust_i.weight
        );
    }

    #[test]
    fn test_synthesis_preserves_existing_interest_order() {
        let mut interests = vec![
            make_interest("Alpha", 1.0),
            make_interest("Beta", 0.8),
            make_interest("Gamma", 0.6),
        ];
        let mut ace = ACEContext::default();
        ace.detected_tech = vec!["python".into(), "kotlin".into()];
        ace.tech_weights.insert("python".into(), 0.5);
        ace.tech_weights.insert("kotlin".into(), 0.5);

        synthesize_ace_interests(&mut interests, &ace, &HashMap::new());

        assert_eq!(
            interests[0].topic, "Alpha",
            "first explicit interest must remain at index 0 after synthesis"
        );
        assert_eq!(
            interests[1].topic, "Beta",
            "second explicit interest must remain at index 1 after synthesis"
        );
        assert_eq!(
            interests[2].topic, "Gamma",
            "third explicit interest must remain at index 2 after synthesis"
        );
        assert_eq!(
            interests.len(),
            5,
            "original 3 + 2 synthesized = 5 total interests; got {}",
            interests.len()
        );
    }
}
