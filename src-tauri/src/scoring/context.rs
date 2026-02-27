use std::collections::HashMap;
use tracing::info;

use crate::db::Database;

use super::{get_ace_context, get_topic_embeddings, ScoringContext};

/// Build a ScoringContext by loading all needed state. Call once per analysis run.
pub(crate) async fn build_scoring_context(db: &Database) -> Result<ScoringContext, String> {
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;
    let feedback_interaction_count: i64 = db.query_feedback_count().unwrap_or(0);

    let context_engine = crate::get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    // User's explicit tech stack from onboarding (small, curated list)
    let declared_tech: Vec<String> = static_identity
        .tech_stack
        .iter()
        .map(|t| t.to_lowercase())
        .collect();

    let ace_ctx = get_ace_context();

    // Load recent work topics for intent-aware scoring (last 2h of git/file activity)
    let work_topics: Vec<String> = match crate::get_ace_engine() {
        Ok(ace) => ace
            .get_recent_work_topics(2)
            .unwrap_or_default()
            .into_iter()
            .map(|(topic, _weight)| topic)
            .collect(),
        Err(_) => vec![],
    };
    let has_active_work = !work_topics.is_empty();

    let topic_embeddings = get_topic_embeddings(&ace_ctx).await;

    // Load feedback-derived topic boosts (Phase 9: feedback learning loop)
    let feedback_boosts: HashMap<String, f64> = db
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

    // Build domain profile for graduated domain relevance scoring
    let (
        domain_profile,
        composed_stack,
        open_windows,
        calibration_deltas,
        topic_half_lives,
        sovereign_profile,
    ) = {
        let conn = crate::open_db_connection()?;
        let dp = crate::domain_profile::build_domain_profile(&conn);
        let cs = crate::stacks::load_composed_stack(&conn);
        // Intelligence metabolism: load decision windows and autophagy calibrations
        let ow = crate::decision_advantage::get_open_windows(&conn);
        let cd = crate::autophagy::load_calibration_deltas(&conn);
        let thl = crate::autophagy::load_topic_decay_profiles(&conn);
        // Unified profile (non-fatal if assembly fails)
        let sp = Some(crate::sovereign_developer_profile::assemble_profile(&conn));
        (dp, cs, ow, cd, thl, sp)
    };

    // Warm-start source preferences from stack profiles (only fills gaps)
    let mut source_quality = source_quality;
    if composed_stack.active {
        for (&source, &pref) in &composed_stack.source_preferences {
            source_quality.entry(source.to_string()).or_insert(pref);
        }
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
        "ACE context loaded for scoring"
    );

    if feedback_interaction_count < 10 {
        info!(target: "4da::scoring", feedback_count = feedback_interaction_count, "Bootstrap mode: relaxed 1-signal gate for new user");
    }

    Ok(ScoringContext {
        cached_context_count,
        interest_count: static_identity.interests.len(),
        interests: static_identity.interests,
        exclusions: static_identity.exclusions,
        ace_ctx,
        topic_embeddings,
        feedback_boosts,
        source_quality,
        declared_tech,
        domain_profile,
        work_topics,
        feedback_interaction_count,
        composed_stack,
        open_windows,
        calibration_deltas,
        topic_half_lives,
        sovereign_profile,
    })
}
