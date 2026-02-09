use once_cell::sync::OnceCell;
use tracing::{debug, warn};

use crate::ace;
use crate::context_engine;
use crate::{embed_texts, get_ace_engine, RelevanceMatch};

/// Compute interest score by comparing item embedding against interest embeddings
pub(crate) fn compute_interest_score(
    item_embedding: &[f32],
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    let mut max_score: f32 = 0.0;

    for interest in interests {
        if let Some(ref interest_embedding) = interest.embedding {
            let similarity = crate::cosine_similarity(item_embedding, interest_embedding);
            let weighted = similarity * interest.weight;
            max_score = max_score.max(weighted);
        }
    }

    max_score
}

/// Generate a human-readable explanation for why an item was considered relevant
/// Returns a concise explanation suitable for display in the UI
pub(crate) fn generate_relevance_explanation(
    _title: &str,
    context_score: f32,
    interest_score: f32,
    matches: &[RelevanceMatch],
    ace_ctx: &ACEContext,
    item_topics: &[String],
) -> String {
    let mut reasons: Vec<String> = Vec::new();

    // Check context matches (what you're working on)
    if context_score > 0.2 {
        if let Some(first_match) = matches.first() {
            // Extract just the filename from the path
            let file_name = first_match
                .source_file
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&first_match.source_file);
            reasons.push(format!("Matches your current work ({})", file_name));
        } else {
            reasons.push("Relates to your active projects".to_string());
        }
    }

    // Check interest matches (declared interests)
    if interest_score > 0.2 {
        reasons.push("Matches your declared interests".to_string());
    }

    // Check ACE tech stack matches
    for topic in item_topics {
        let topic_lower = topic.to_lowercase();
        if let Some(tech) = ace_ctx
            .detected_tech
            .iter()
            .find(|t| t.to_lowercase() == topic_lower)
        {
            reasons.push(format!("Your project uses {}", tech));
            break;
        }
    }

    // Check ACE active topics matches (recent activity)
    for topic in item_topics {
        let topic_lower = topic.to_lowercase();
        if let Some(active_topic) = ace_ctx
            .active_topics
            .iter()
            .find(|t| t.to_lowercase() == topic_lower || topic_lower.contains(&t.to_lowercase()))
        {
            reasons.push(format!("Matches your recent activity: {}", active_topic));
            break;
        }
    }

    // Check learned affinity matches
    for topic in item_topics {
        let topic_lower = topic.to_lowercase();
        if let Some((score, _conf)) = ace_ctx.topic_affinities.get(&topic_lower) {
            if *score > 0.3 {
                reasons.push(format!("You've shown interest in {}", topic));
                break;
            }
        }
    }

    // Deduplicate and format output
    if reasons.is_empty() {
        "Matches your overall profile".to_string()
    } else if reasons.len() == 1 {
        reasons[0].clone()
    } else {
        // Return first two reasons joined by semicolon
        format!("{}; {}", reasons[0], reasons[1])
    }
}

// ============================================================================
// ACE Context Integration
// ============================================================================

/// ACE-discovered context for relevance scoring
/// PASIFA: Full context including confidence scores for weighted scoring
#[derive(Debug, Default)]
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
        for t in topics.iter().filter(|t| t.weight >= 0.3) {
            let topic_lower = t.topic.to_lowercase();
            ctx.active_topics.push(topic_lower.clone());
            ctx.topic_confidence.insert(topic_lower, t.confidence);
        }
    }

    // Get detected tech
    if let Ok(tech) = ace.get_detected_tech() {
        ctx.detected_tech = tech.iter().map(|t| t.name.to_lowercase()).collect();
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

    ctx
}

/// Check if item should be excluded by ACE anti-topics
pub(crate) fn check_ace_exclusions(topics: &[String], ace_ctx: &ACEContext) -> Option<String> {
    for topic in topics {
        let topic_lower = topic.to_lowercase();
        for anti_topic in &ace_ctx.anti_topics {
            if topic_lower.contains(anti_topic) || anti_topic.contains(&topic_lower) {
                return Some(format!("ACE anti-topic: {}", anti_topic));
            }
        }
    }
    None
}

/// Compute semantic ACE boost using embeddings
/// PASIFA: Uses vector similarity instead of keyword matching when embeddings available
pub(crate) fn compute_semantic_ace_boost(
    item_embedding: &[f32],
    ace_ctx: &ACEContext,
    topic_embeddings: &std::collections::HashMap<String, Vec<f32>>,
) -> Option<f32> {
    if topic_embeddings.is_empty() {
        return None; // Fall back to keyword matching
    }

    let mut max_similarity: f32 = 0.0;
    let mut weighted_sum: f32 = 0.0;
    let mut weight_total: f32 = 0.0;

    // Compute similarity with active topics
    for topic in &ace_ctx.active_topics {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = crate::cosine_similarity(item_embedding, topic_emb);
            let conf = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
            weighted_sum += sim * conf;
            weight_total += conf;
            max_similarity = max_similarity.max(sim);
        }
    }

    // Compute similarity with detected tech
    for tech in &ace_ctx.detected_tech {
        if let Some(tech_emb) = topic_embeddings.get(tech) {
            let sim = crate::cosine_similarity(item_embedding, tech_emb);
            weighted_sum += sim * 0.8; // Tech is strong signal
            weight_total += 0.8;
            max_similarity = max_similarity.max(sim);
        }
    }

    if weight_total == 0.0 {
        return None;
    }

    // Compute weighted average similarity
    let avg_similarity = weighted_sum / weight_total;

    // Apply learned affinities as multiplier with confidence weighting
    let mut affinity_mult: f32 = 1.0;
    for (topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = crate::cosine_similarity(item_embedding, topic_emb);
            if sim > 0.5 {
                // Item is similar to a topic we have affinity data for
                // Scale by both similarity and confidence
                affinity_mult += affinity * confidence * 0.3 * sim;
            }
        }
    }
    affinity_mult = affinity_mult.clamp(0.5, 1.5);

    // Convert similarity (0-1) to boost (-0.3 to 0.5) range
    // High similarity (>0.7) = positive boost
    // Low similarity (<0.3) = negative boost
    let base_boost = (avg_similarity - 0.5) * 1.0; // Center around 0.5

    Some((base_boost * affinity_mult).clamp(-0.3, 0.5))
}

/// Embed ACE topics for semantic matching
/// Uses database-persisted embeddings with in-memory cache fallback
/// Returns topic -> embedding map
pub(crate) fn get_topic_embeddings(
    ace_ctx: &ACEContext,
) -> std::collections::HashMap<String, Vec<f32>> {
    // Lazy static cache for topic embeddings
    use std::sync::Mutex;
    static TOPIC_EMBEDDING_CACHE: OnceCell<Mutex<std::collections::HashMap<String, Vec<f32>>>> =
        OnceCell::new();
    static DB_LOADED: OnceCell<Mutex<bool>> = OnceCell::new();

    let cache = TOPIC_EMBEDDING_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let db_loaded = DB_LOADED.get_or_init(|| Mutex::new(false));

    let Ok(mut cache_guard) = cache.lock() else {
        warn!(target: "4da::embeddings", "Topic cache lock poisoned, returning empty");
        return std::collections::HashMap::new();
    };
    let Ok(mut db_loaded_guard) = db_loaded.lock() else {
        warn!(target: "4da::embeddings", "DB loaded lock poisoned, returning empty");
        return std::collections::HashMap::new();
    };

    // First time: load persisted embeddings from database
    if !*db_loaded_guard {
        if let Ok(ace) = get_ace_engine() {
            if let Ok(db_embeddings) = ace::load_topic_embeddings(ace.get_conn()) {
                for (topic, embedding) in db_embeddings {
                    cache_guard.insert(topic, embedding);
                }
                debug!(
                    target: "4da::embeddings",
                    count = cache_guard.len(),
                    "Loaded topic embeddings from database"
                );
            }
        }
        *db_loaded_guard = true;
    }

    // Collect topics that need embedding
    let mut topics_to_embed: Vec<String> = Vec::new();

    for topic in &ace_ctx.active_topics {
        if !cache_guard.contains_key(topic) {
            topics_to_embed.push(topic.clone());
        }
    }

    for tech in &ace_ctx.detected_tech {
        if !cache_guard.contains_key(tech) {
            topics_to_embed.push(tech.clone());
        }
    }

    for topic in ace_ctx.topic_affinities.keys() {
        if !cache_guard.contains_key(topic) {
            topics_to_embed.push(topic.clone());
        }
    }

    // Generate embeddings for missing topics and persist to database
    if !topics_to_embed.is_empty() {
        // Limit batch size to avoid overwhelming
        let batch: Vec<String> = topics_to_embed.into_iter().take(50).collect();
        let batch_len = batch.len();

        if let Ok(embeddings) = embed_texts(&batch) {
            // Get ACE connection for persistence
            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());

            for (topic, embedding) in batch.into_iter().zip(embeddings.into_iter()) {
                // Persist to database if possible
                if let Some(ref conn) = ace_conn {
                    let _ = ace::store_topic_embedding(conn, &topic, &embedding);
                }
                cache_guard.insert(topic, embedding);
            }

            debug!(
                target: "4da::embeddings",
                generated = batch_len,
                "Generated and persisted new topic embeddings"
            );
        }
    }

    // Return a copy of relevant embeddings
    let mut result = std::collections::HashMap::new();
    for topic in &ace_ctx.active_topics {
        if let Some(emb) = cache_guard.get(topic) {
            result.insert(topic.clone(), emb.clone());
        }
    }
    for tech in &ace_ctx.detected_tech {
        if let Some(emb) = cache_guard.get(tech) {
            result.insert(tech.clone(), emb.clone());
        }
    }
    for topic in ace_ctx.topic_affinities.keys() {
        if let Some(emb) = cache_guard.get(topic) {
            result.insert(topic.clone(), emb.clone());
        }
    }

    result
}

/// Compute affinity multiplier from learned topic preferences
/// PASIFA: Applies learned affinities as multiplicative factors with confidence scaling
pub(crate) fn compute_affinity_multiplier(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    if ace_ctx.topic_affinities.is_empty() {
        return 1.0; // No learned preferences, neutral
    }

    let mut effect_sum: f32 = 0.0;
    let mut match_count: usize = 0;

    for topic in topics {
        let topic_lower = topic.to_lowercase();

        // Check direct match
        if let Some(&(affinity, confidence)) = ace_ctx.topic_affinities.get(&topic_lower) {
            // Effect = affinity * confidence (confidence scales the effect directly)
            effect_sum += affinity * confidence;
            match_count += 1;
            continue;
        }

        // Check partial matches
        for (aff_topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
            if topic_lower.contains(aff_topic) || aff_topic.contains(&topic_lower) {
                // Partial match: reduce effect by 0.7
                effect_sum += affinity * confidence * 0.7;
                match_count += 1;
                break;
            }
        }
    }

    if match_count == 0 {
        return 1.0; // No matches, neutral
    }

    // Average effect across matched topics, then convert to multiplier [0.3, 1.7]
    // This ensures confidence directly scales the effect:
    // High confidence (1.0) + high affinity (0.8) -> effect = 0.8 -> mult = 1.56
    // Low confidence (0.3) + high affinity (0.8) -> effect = 0.24 -> mult = 1.17
    let avg_effect = effect_sum / match_count as f32;
    (1.0 + avg_effect * 0.7).clamp(0.3, 1.7)
}

/// Compute anti-topic penalty as a multiplicative factor
/// PASIFA: Items matching anti-topics get reduced score based on confidence
pub(crate) fn compute_anti_penalty(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    if ace_ctx.anti_topics.is_empty() {
        return 0.0; // No anti-topics, no penalty
    }

    let mut total_penalty: f32 = 0.0;

    for topic in topics {
        let topic_lower = topic.to_lowercase();

        for anti_topic in &ace_ctx.anti_topics {
            if topic_lower.contains(anti_topic) || anti_topic.contains(&topic_lower) {
                // Get confidence for this anti-topic (default 0.5)
                let confidence = ace_ctx
                    .anti_topic_confidence
                    .get(anti_topic)
                    .copied()
                    .unwrap_or(0.5);

                // Scale penalty by confidence: higher confidence = stronger penalty
                // Max penalty per match is 0.3
                total_penalty += 0.3 * confidence;
                break; // Only one penalty per topic
            }
        }
    }

    // Cap total penalty at 0.7 (never fully zero out)
    total_penalty.min(0.7)
}

/// Unified relevance scoring using multiplicative formula
/// PASIFA: semantic_sim * affinity_multiplier * (1.0 - anti_penalty)
pub(crate) fn compute_unified_relevance(
    base_score: f32,
    topics: &[String],
    ace_ctx: &ACEContext,
) -> f32 {
    let affinity_mult = compute_affinity_multiplier(topics, ace_ctx);
    let anti_penalty = compute_anti_penalty(topics, ace_ctx);

    // Apply multiplicative formula
    let unified_score = base_score * affinity_mult * (1.0 - anti_penalty);

    // Clamp to valid range
    unified_score.clamp(0.0, 1.0)
}

/// Temporal freshness multiplier for PASIFA scoring.
/// Recent items get a slight boost, older items decay gently.
/// Returns a multiplier in [0.85, 1.15] range:
///   - Items < 2 hours old: 1.15x boost (very fresh)
///   - Items 2-6 hours old: 1.10x boost
///   - Items 6-12 hours old: 1.05x boost
///   - Items 12-24 hours old: 1.0x (neutral)
///   - Items 24-36 hours old: 0.95x decay
///   - Items 36-48 hours old: 0.90x decay
///   - Items > 48 hours old: 0.85x floor
pub(crate) fn compute_temporal_freshness(created_at: &chrono::DateTime<chrono::Utc>) -> f32 {
    let age_hours = (chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0;

    if age_hours < 2.0 {
        1.15
    } else if age_hours < 6.0 {
        1.10
    } else if age_hours < 12.0 {
        1.05
    } else if age_hours < 24.0 {
        1.0
    } else if age_hours < 36.0 {
        0.95
    } else if age_hours < 48.0 {
        0.90
    } else {
        0.85
    }
}

/// Calculate confidence score based on available signals
/// Returns a value between 0.0 and 1.0 indicating how confident we are in the scoring
pub(crate) fn calculate_confidence(
    context_score: f32,
    interest_score: f32,
    _semantic_boost: f32,
    ace_ctx: &ACEContext,
    topics: &[String],
    cached_context_count: i64,
    interest_count: i64,
) -> f32 {
    let mut confidence_signals: Vec<f32> = Vec::new();

    // Context signal confidence (higher score = more confident match)
    if cached_context_count > 0 {
        confidence_signals.push(context_score.clamp(0.0, 1.0));
    }

    // Interest signal confidence
    if interest_count > 0 {
        confidence_signals.push(interest_score.clamp(0.0, 1.0));
    }

    // ACE topic confidence (average of matched topic confidences)
    let mut topic_confidences: Vec<f32> = Vec::new();
    for topic in topics {
        let topic_lower = topic.to_lowercase();

        // Check active topic confidences
        if let Some(&conf) = ace_ctx.topic_confidence.get(&topic_lower) {
            topic_confidences.push(conf);
        }

        // Check affinity confidences
        if let Some(&(_affinity, conf)) = ace_ctx.topic_affinities.get(&topic_lower) {
            topic_confidences.push(conf);
        }
    }

    if !topic_confidences.is_empty() {
        let avg_topic_conf = topic_confidences.iter().sum::<f32>() / topic_confidences.len() as f32;
        confidence_signals.push(avg_topic_conf);
    }

    // If we have multiple signals, they reinforce each other
    if confidence_signals.is_empty() {
        return 0.4; // Low confidence - no strong signals
    }

    // Combine signals: average with bonus for multiple signals
    let avg_confidence = confidence_signals.iter().sum::<f32>() / confidence_signals.len() as f32;
    let signal_count_bonus = (confidence_signals.len() as f32 * 0.1).min(0.2);

    (avg_confidence + signal_count_bonus).clamp(0.0, 1.0)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test ACEContext creation and defaults
    #[test]
    fn test_ace_context_default() {
        let ctx = ACEContext::default();
        assert!(ctx.active_topics.is_empty());
        assert!(ctx.detected_tech.is_empty());
        assert!(ctx.anti_topics.is_empty());
        assert!(ctx.topic_affinities.is_empty());
    }

    // Test affinity multiplier with empty context
    #[test]
    fn test_affinity_multiplier_empty_context() {
        let ctx = ACEContext::default();
        let topics = vec!["rust".to_string(), "tauri".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);
        assert_eq!(mult, 1.0, "Empty context should return neutral multiplier");
    }

    // Test affinity multiplier with positive affinity
    #[test]
    fn test_affinity_multiplier_positive() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9)); // High affinity, high confidence

        let topics = vec!["rust".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        // 0.8 * 0.9 = 0.72 weighted affinity
        // 1.0 + 0.72 * 0.7 = 1.504
        assert!(mult > 1.0, "Positive affinity should boost multiplier");
        assert!(mult <= 1.7, "Multiplier should be capped at 1.7");
    }

    // Test affinity multiplier with negative affinity
    #[test]
    fn test_affinity_multiplier_negative() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities
            .insert("crypto".to_string(), (-0.9, 0.8)); // Strong dislike, high confidence

        let topics = vec!["crypto".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        assert!(mult < 1.0, "Negative affinity should reduce multiplier");
        assert!(mult >= 0.3, "Multiplier should be capped at 0.3");
    }

    // Test anti-penalty computation
    #[test]
    fn test_anti_penalty_empty_context() {
        let ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let penalty = compute_anti_penalty(&topics, &ctx);
        assert_eq!(penalty, 0.0, "Empty context should return zero penalty");
    }

    // Test anti-penalty with matching anti-topic
    #[test]
    fn test_anti_penalty_with_match() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("spam".to_string());
        ctx.anti_topic_confidence.insert("spam".to_string(), 0.8);

        let topics = vec!["spam".to_string()];
        let penalty = compute_anti_penalty(&topics, &ctx);

        // 0.3 * 0.8 = 0.24
        assert!(penalty > 0.0, "Matching anti-topic should produce penalty");
        assert!(penalty <= 0.7, "Penalty should be capped at 0.7");
    }

    // Test unified relevance scoring
    #[test]
    fn test_unified_relevance_neutral() {
        let ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // With neutral context: 0.5 * 1.0 * (1.0 - 0.0) = 0.5
        assert_eq!(score, 0.5, "Neutral context should preserve base score");
    }

    // Test unified relevance with positive affinity
    #[test]
    fn test_unified_relevance_positive_affinity() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 1.0));

        let topics = vec!["rust".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // Base 0.5 * multiplier > 1.0 * (1.0 - 0.0)
        assert!(score > 0.5, "Positive affinity should boost score");
    }

    // Test unified relevance with anti-topic
    #[test]
    fn test_unified_relevance_anti_topic() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("spam".to_string());
        ctx.anti_topic_confidence.insert("spam".to_string(), 1.0);

        let topics = vec!["spam".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // Base 0.5 * 1.0 * (1.0 - penalty)
        assert!(score < 0.5, "Anti-topic should reduce score");
    }

    // Test confidence weighting effect
    #[test]
    fn test_confidence_weighting() {
        let mut ctx_high_conf = ACEContext::default();
        ctx_high_conf
            .topic_affinities
            .insert("rust".to_string(), (0.8, 1.0));

        let mut ctx_low_conf = ACEContext::default();
        ctx_low_conf
            .topic_affinities
            .insert("rust".to_string(), (0.8, 0.3));

        let topics = vec!["rust".to_string()];

        let score_high = compute_unified_relevance(0.5, &topics, &ctx_high_conf);
        let score_low = compute_unified_relevance(0.5, &topics, &ctx_low_conf);

        assert!(
            score_high > score_low,
            "Higher confidence should produce stronger effect"
        );
    }

    // Test score clamping
    #[test]
    fn test_score_clamping() {
        let mut ctx = ACEContext::default();
        // Extreme positive affinity
        ctx.topic_affinities.insert("rust".to_string(), (1.0, 1.0));

        let topics = vec!["rust".to_string()];
        let score = compute_unified_relevance(1.0, &topics, &ctx);

        assert!(score <= 1.0, "Score should be clamped to 1.0");

        // Extreme negative
        let mut ctx_neg = ACEContext::default();
        ctx_neg
            .topic_affinities
            .insert("spam".to_string(), (-1.0, 1.0));
        ctx_neg.anti_topics.push("spam".to_string());
        ctx_neg
            .anti_topic_confidence
            .insert("spam".to_string(), 1.0);

        let score_neg = compute_unified_relevance(0.5, &["spam".to_string()], &ctx_neg);
        assert!(score_neg >= 0.0, "Score should be clamped to 0.0");
    }

    // Test partial topic matching
    #[test]
    fn test_partial_topic_match() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9));

        // "rustlang" should partially match "rust"
        let topics = vec!["rustlang".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        assert!(mult > 1.0, "Partial match should still produce boost");
    }

    // Test temporal freshness computation
    #[test]
    fn test_temporal_freshness_very_recent() {
        let now = chrono::Utc::now();
        let freshness = compute_temporal_freshness(&now);
        assert_eq!(freshness, 1.15, "Items just created should get max boost");
    }

    #[test]
    fn test_temporal_freshness_few_hours() {
        let three_hours_ago = chrono::Utc::now() - chrono::Duration::hours(3);
        let freshness = compute_temporal_freshness(&three_hours_ago);
        assert_eq!(freshness, 1.10, "Items 3h old should get 1.10x boost");
    }

    #[test]
    fn test_temporal_freshness_half_day() {
        let nine_hours_ago = chrono::Utc::now() - chrono::Duration::hours(9);
        let freshness = compute_temporal_freshness(&nine_hours_ago);
        assert_eq!(freshness, 1.05, "Items 9h old should get 1.05x boost");
    }

    #[test]
    fn test_temporal_freshness_one_day() {
        let eighteen_hours_ago = chrono::Utc::now() - chrono::Duration::hours(18);
        let freshness = compute_temporal_freshness(&eighteen_hours_ago);
        assert_eq!(freshness, 1.0, "Items 18h old should be neutral");
    }

    #[test]
    fn test_temporal_freshness_old() {
        let two_days_ago = chrono::Utc::now() - chrono::Duration::hours(40);
        let freshness = compute_temporal_freshness(&two_days_ago);
        assert_eq!(freshness, 0.90, "Items 40h old should decay to 0.90");
    }

    #[test]
    fn test_temporal_freshness_very_old() {
        let old = chrono::Utc::now() - chrono::Duration::hours(72);
        let freshness = compute_temporal_freshness(&old);
        assert_eq!(freshness, 0.85, "Items 72h old should hit floor at 0.85");
    }
}
