use once_cell::sync::OnceCell;
use tracing::{debug, info, warn};

use crate::ace;
use crate::context_engine;
use crate::db::Database;
use crate::signals;
use crate::{
    check_exclusions, embed_texts, extract_topics, get_ace_engine, get_context_engine,
    get_relevance_threshold, RelevanceMatch, ScoreBreakdown, SourceRelevance,
};

/// Calibrate a raw similarity score (typically compressed in [0.3-0.6]) into
/// a spread distribution using a sigmoid stretch. Centers at 0.48 (empirical
/// midpoint for text-embedding-3-small L2 distances) and scales to use the
/// full [0.05-0.95] range. This fixes the "everything scores 45-50%" problem.
fn calibrate_score(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    if raw >= 1.0 {
        return 1.0;
    }
    // Sigmoid stretch: 1 / (1 + exp((center - raw) * scale))
    // center=0.48, scale=12 maps the typical [0.40-0.56] band to [0.15-0.85]
    // (scale=20 was too aggressive, compressing near edges)
    1.0 / (1.0 + ((0.48 - raw) * 12.0).exp())
}

/// Compute interest score by comparing item embedding against interest embeddings
pub(crate) fn compute_interest_score(
    item_embedding: &[f32],
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return 0.0; // Zero-norm embedding can't produce meaningful similarity
    }
    let mut max_score: f32 = 0.0;

    for interest in interests {
        if let Some(ref interest_embedding) = interest.embedding {
            let similarity =
                crate::cosine_similarity_with_norm(item_embedding, item_norm, interest_embedding);
            let weighted = similarity * interest.weight;
            max_score = max_score.max(weighted);
        }
    }

    max_score
}

/// Short tech keywords that are valid despite being <3 chars.
/// These are common abbreviations that users declare as interests.
const SHORT_TECH_KEYWORDS: &[&str] = &[
    "ai", "ml", "go", "r", "c", "ui", "ux", "db", "os", "ci", "cd", "qa", "js", "ts", "py", "rx",
    "vm", "k8", "tf", "gcp", "aws", "api", "cli", "css", "sql", "llm", "nlp", "cv",
];

/// Keyword-based interest matching: boosts items that literally contain declared interest terms.
/// Complements semantic matching which can miss exact keyword matches.
fn compute_keyword_interest_score(
    title: &str,
    content: &str,
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    let title_lower = title.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content.to_lowercase());
    let mut max_score: f32 = 0.0;

    for interest in interests {
        let interest_lower = interest.topic.to_lowercase();
        let terms: Vec<&str> = interest_lower.split_whitespace().collect();
        if terms.is_empty() {
            continue;
        }

        let mut hits = 0.0_f32;
        let mut counted_terms = 0_usize;
        for term in &terms {
            // Skip generic short words, but allow known tech abbreviations
            if term.len() < 2 {
                continue;
            }
            if term.len() < 3 && !SHORT_TECH_KEYWORDS.contains(term) {
                continue;
            }
            counted_terms += 1;

            // For very short terms (1-2 chars), require word boundary match to avoid false positives
            // e.g. "go" shouldn't match "google", "algorithm"
            let matched_title = if term.len() <= 2 {
                has_word_boundary_match(&title_lower, term)
            } else {
                title_lower.contains(term)
            };
            let matched_content = if !matched_title && term.len() <= 2 {
                has_word_boundary_match(&text_lower, term)
            } else if !matched_title {
                text_lower.contains(term)
            } else {
                false
            };

            if matched_title {
                hits += 1.5; // title match = 1.5x weight
            } else if matched_content {
                hits += 1.0;
            }
        }

        let divisor = counted_terms.max(1) as f32;
        let score = (hits / divisor).min(1.0) * interest.weight;
        max_score = max_score.max(score);
    }

    max_score
}

/// Check if a short term appears as a whole word (bounded by non-alphanumeric chars)
fn has_word_boundary_match(text: &str, term: &str) -> bool {
    for (i, _) in text.match_indices(term) {
        let before_ok = i == 0 || !text.as_bytes()[i - 1].is_ascii_alphanumeric();
        let after_pos = i + term.len();
        let after_ok =
            after_pos >= text.len() || !text.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Generate a human-readable explanation for why an item was considered relevant.
/// Produces specific, actionable text naming the exact technologies/topics that matched.
pub(crate) fn generate_relevance_explanation(
    _title: &str,
    context_score: f32,
    interest_score: f32,
    matches: &[RelevanceMatch],
    ace_ctx: &ACEContext,
    item_topics: &[String],
    interests: &[context_engine::Interest],
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut used_topics: Vec<&str> = Vec::new();

    // 1. Tech stack matches (most specific signal)
    let tech_hits: Vec<&str> = item_topics
        .iter()
        .filter_map(|t| {
            ace_ctx
                .detected_tech
                .iter()
                .find(|tech| *tech == t || t.contains(tech.as_str()))
                .map(|s| s.as_str())
        })
        .collect();
    if !tech_hits.is_empty() {
        let names: Vec<&str> = tech_hits.iter().copied().take(3).collect();
        for &n in &names {
            used_topics.push(n);
        }
        parts.push(format!("Uses {} (your stack)", names.join(", ")));
    }

    // 2. Active project topic matches
    let topic_hits: Vec<&str> = item_topics
        .iter()
        .filter_map(|t| {
            ace_ctx
                .active_topics
                .iter()
                .find(|at| *at == t || t.contains(at.as_str()))
                .map(|s| s.as_str())
        })
        .filter(|t| !used_topics.contains(t))
        .collect();
    if !topic_hits.is_empty() {
        let names: Vec<&str> = topic_hits.iter().copied().take(2).collect();
        for &n in &names {
            used_topics.push(n);
        }
        parts.push(format!("Related to {} (active project)", names.join(", ")));
    }

    // 3. Declared interest matches (name the specific interest)
    if interest_score > 0.15 {
        let interest_hits: Vec<&str> = item_topics
            .iter()
            .filter_map(|t| {
                interests
                    .iter()
                    .find(|i| {
                        let il = i.topic.to_lowercase();
                        *t == il || t.contains(il.as_str()) || il.contains(t.as_str())
                    })
                    .map(|i| i.topic.as_str())
            })
            .filter(|t| {
                let tl = t.to_lowercase();
                !used_topics.iter().any(|u| *u == tl)
            })
            .collect();
        if !interest_hits.is_empty() {
            let names: Vec<&str> = interest_hits.iter().copied().take(2).collect();
            parts.push(format!("Matches interest: {}", names.join(", ")));
        } else if parts.is_empty() {
            // Interest score is high but no topic-level match — use context match
            if let Some(m) = matches.first().filter(|_| context_score > 0.2) {
                let phrase = extract_short_phrase(&m.matched_text);
                if !phrase.is_empty() {
                    parts.push(format!("Matches your project context: \"{}\"", phrase));
                }
            }
        }
    }

    // 4. Learned affinity (only if nothing else matched)
    if parts.is_empty() {
        for topic in item_topics {
            if let Some((score, _)) = ace_ctx.topic_affinities.get(topic.as_str()) {
                if *score > 0.3 {
                    parts.push(format!("You engage with {} content", topic));
                    break;
                }
            }
        }
    }

    // 5. Strong context match fallback
    if parts.is_empty() && context_score > 0.3 {
        if let Some(m) = matches.first() {
            let phrase = extract_short_phrase(&m.matched_text);
            if !phrase.is_empty() {
                parts.push(format!("Similar to your code: \"{}\"", phrase));
            }
        }
    }

    // Return empty string instead of vague fallback — the frontend handles empty gracefully
    parts.join(" · ")
}

/// Extract a short meaningful phrase from matched context text
fn extract_short_phrase(matched_text: &str) -> String {
    let clean = matched_text.trim().trim_end_matches("...");
    let phrase = clean
        .find(['.', '\n'])
        .filter(|&pos| pos > 10)
        .map(|pos| &clean[..pos])
        .unwrap_or(&clean[..clean.len().min(80)])
        .trim();
    if phrase.len() < 10 {
        String::new()
    } else {
        phrase.to_string()
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

    // Merge recent work topics (last 2 hours) with high confidence.
    // These represent what the user is actively working on RIGHT NOW,
    // so they get elevated confidence to boost related content.
    if let Ok(work_topics) = ace.get_recent_work_topics(2) {
        for (topic, weight) in work_topics {
            if !ctx.active_topics.contains(&topic) {
                ctx.active_topics.push(topic.clone());
            }
            // Recent work topics get high confidence (0.85-0.95 scaled by recency)
            // weight ranges 0.5-1.0, maps to confidence 0.85-0.95
            let work_confidence = 0.85 + (weight - 0.5) * 0.2;
            let existing = ctx.topic_confidence.get(&topic).copied().unwrap_or(0.0);
            ctx.topic_confidence
                .insert(topic, existing.max(work_confidence));
        }
        debug!(target: "4da::ace", "Merged recent work topics into ACE context");
    }

    ctx
}

/// Check if item should be excluded by ACE anti-topics
pub(crate) fn check_ace_exclusions(topics: &[String], ace_ctx: &ACEContext) -> Option<String> {
    // Both topics (from extract_topics) and anti_topics are already lowercase
    for topic in topics {
        for anti_topic in &ace_ctx.anti_topics {
            if topic.contains(anti_topic.as_str()) || anti_topic.contains(topic.as_str()) {
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

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return None; // Zero-norm embedding can't produce meaningful similarity
    }

    let mut max_similarity: f32 = 0.0;
    let mut weighted_sum: f32 = 0.0;
    let mut weight_total: f32 = 0.0;

    // Compute similarity with active topics
    for topic in &ace_ctx.active_topics {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, topic_emb);
            let conf = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
            weighted_sum += sim * conf;
            weight_total += conf;
            max_similarity = max_similarity.max(sim);
        }
    }

    // Compute similarity with detected tech
    for tech in &ace_ctx.detected_tech {
        if let Some(tech_emb) = topic_embeddings.get(tech) {
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, tech_emb);
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
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, topic_emb);
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
pub(crate) async fn get_topic_embeddings(
    ace_ctx: &ACEContext,
) -> std::collections::HashMap<String, Vec<f32>> {
    // Lazy static cache for topic embeddings
    use std::sync::Mutex;
    static TOPIC_EMBEDDING_CACHE: OnceCell<Mutex<std::collections::HashMap<String, Vec<f32>>>> =
        OnceCell::new();
    static DB_LOADED: OnceCell<Mutex<bool>> = OnceCell::new();

    let cache = TOPIC_EMBEDDING_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let db_loaded = DB_LOADED.get_or_init(|| Mutex::new(false));

    // Phase 1 (sync): Load DB cache + collect topics needing embedding
    // All MutexGuard usage is scoped here so they drop before any .await
    let topics_to_embed: Vec<String> = {
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
        let mut needed: Vec<String> = Vec::new();
        for topic in &ace_ctx.active_topics {
            if !cache_guard.contains_key(topic) {
                needed.push(topic.clone());
            }
        }
        for tech in &ace_ctx.detected_tech {
            if !cache_guard.contains_key(tech) {
                needed.push(tech.clone());
            }
        }
        for topic in ace_ctx.topic_affinities.keys() {
            if !cache_guard.contains_key(topic) {
                needed.push(topic.clone());
            }
        }

        needed
    }; // MutexGuards dropped here - safe to .await below

    // Phase 2 (async): Generate embeddings for missing topics
    if !topics_to_embed.is_empty() {
        let batch: Vec<String> = topics_to_embed.into_iter().take(50).collect();
        let batch_len = batch.len();

        if let Ok(embeddings) = embed_texts(&batch).await {
            // Phase 3 (sync): Store results back into cache
            let Ok(mut cache_guard) = cache.lock() else {
                warn!(target: "4da::embeddings", "Topic cache lock poisoned after embed, returning empty");
                return std::collections::HashMap::new();
            };

            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());
            for (topic, embedding) in batch.into_iter().zip(embeddings.into_iter()) {
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

    // Phase 4 (sync): Build result from cache
    let Ok(cache_guard) = cache.lock() else {
        warn!(target: "4da::embeddings", "Topic cache lock poisoned building result, returning empty");
        return std::collections::HashMap::new();
    };

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

    // Both topics (from extract_topics) and affinity keys are already lowercase
    for topic in topics {
        // Check direct match
        if let Some(&(affinity, confidence)) = ace_ctx.topic_affinities.get(topic.as_str()) {
            effect_sum += affinity * confidence;
            match_count += 1;
            continue;
        }

        // Check partial matches
        for (aff_topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
            if topic.contains(aff_topic.as_str()) || aff_topic.contains(topic.as_str()) {
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

    // Both topics and anti_topics are already lowercase
    for topic in topics {
        for anti_topic in &ace_ctx.anti_topics {
            if topic.contains(anti_topic.as_str()) || anti_topic.contains(topic.as_str()) {
                let confidence = ace_ctx
                    .anti_topic_confidence
                    .get(anti_topic)
                    .copied()
                    .unwrap_or(0.5);
                total_penalty += 0.3 * confidence;
                break;
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
    let age_hours = ((chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0).max(0.0);

    if age_hours < 3.0 {
        1.20 // Breaking/fresh: strong boost
    } else if age_hours < 12.0 {
        1.15
    } else if age_hours < 24.0 {
        1.10 // Today: still fresh
    } else if age_hours < 72.0 {
        1.0 // 1-3 days: neutral
    } else if age_hours < 168.0 {
        0.92 // 3-7 days: mild decay
    } else if age_hours < 720.0 {
        0.82 // 1-4 weeks: noticeable decay
    } else {
        0.70 // >1 month: significant decay
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
    // Topics and ace_ctx keys are already lowercase
    for topic in topics {
        if let Some(&conf) = ace_ctx.topic_confidence.get(topic.as_str()) {
            topic_confidences.push(conf);
        }
        if let Some(&(_affinity, conf)) = ace_ctx.topic_affinities.get(topic.as_str()) {
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
// Unified Scoring Pipeline
// ============================================================================

/// Input data for scoring a single item
pub(crate) struct ScoringInput<'a> {
    pub id: u64,
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub content: &'a str,
    pub source_type: &'a str,
    pub embedding: &'a [f32],
    pub created_at: Option<&'a chrono::DateTime<chrono::Utc>>,
}

/// Pre-loaded context for scoring (computed once per analysis run)
pub(crate) struct ScoringContext {
    pub cached_context_count: i64,
    pub interest_count: usize,
    pub interests: Vec<context_engine::Interest>,
    pub exclusions: Vec<String>,
    pub ace_ctx: ACEContext,
    pub topic_embeddings: std::collections::HashMap<String, Vec<f32>>,
    /// Feedback-derived topic boosts: topic -> net_score (-1.0 to 1.0)
    pub feedback_boosts: std::collections::HashMap<String, f64>,
    /// Source quality scores from learned preferences: source_type -> score (-1.0 to 1.0)
    pub source_quality: std::collections::HashMap<String, f32>,
}

/// Options controlling which scoring stages are applied
pub(crate) struct ScoringOptions {
    pub apply_freshness: bool,
    pub apply_signals: bool,
}

/// Build a ScoringContext by loading all needed state. Call once per analysis run.
pub(crate) async fn build_scoring_context(db: &Database) -> Result<ScoringContext, String> {
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let ace_ctx = get_ace_context();

    // Check if user has recent file activity (active work window)
    let has_active_work = match get_ace_engine() {
        Ok(ace) => ace.get_recent_work_topics(2).is_ok_and(|t| !t.is_empty()),
        Err(_) => false,
    };

    let topic_embeddings = get_topic_embeddings(&ace_ctx).await;

    // Load feedback-derived topic boosts (Phase 9: feedback learning loop)
    let feedback_boosts: std::collections::HashMap<String, f64> = db
        .get_feedback_topic_summary()
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.topic, f.net_score))
        .collect();

    // Load source quality preferences from ACE behavior learning
    let source_quality: std::collections::HashMap<String, f32> = match get_ace_engine() {
        Ok(ace) => ace
            .get_source_preferences()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        Err(_) => std::collections::HashMap::new(),
    };

    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        embeddings = topic_embeddings.len(),
        feedback_topics = feedback_boosts.len(),
        source_prefs = source_quality.len(),
        has_active_work,
        "ACE context loaded for scoring"
    );

    Ok(ScoringContext {
        cached_context_count,
        interest_count: static_identity.interests.len(),
        interests: static_identity.interests,
        exclusions: static_identity.exclusions,
        ace_ctx,
        topic_embeddings,
        feedback_boosts,
        source_quality,
    })
}

/// Score a single item through the full PASIFA pipeline.
/// Returns SourceRelevance with all fields populated.
pub(crate) fn score_item(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    options: &ScoringOptions,
    classifier: Option<&signals::SignalClassifier>,
) -> SourceRelevance {
    let topics = extract_topics(input.title, input.content);

    // Check exclusions
    let excluded_by = check_exclusions(&topics, &ctx.exclusions)
        .or_else(|| check_ace_exclusions(&topics, &ctx.ace_ctx));

    if let Some(exclusion) = excluded_by {
        return SourceRelevance {
            id: input.id,
            title: input.title.to_string(),
            url: input.url.map(|s| s.to_string()),
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
        };
    }

    // KNN context search
    let matches: Vec<RelevanceMatch> =
        if ctx.cached_context_count > 0 && !input.embedding.is_empty() {
            db.find_similar_contexts(input.embedding, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|result| {
                    let similarity = 1.0 / (1.0 + result.distance);
                    let matched_text = if result.text.len() > 100 {
                        let truncated: String = result.text.chars().take(100).collect();
                        format!("{}...", truncated)
                    } else {
                        result.text
                    };
                    RelevanceMatch {
                        source_file: result.source_file,
                        matched_text,
                        similarity,
                    }
                })
                .collect()
        } else {
            vec![]
        };

    // Raw scores from embedding similarity (compressed in ~0.40-0.56 range)
    let raw_context = matches.first().map(|m| m.similarity).unwrap_or(0.0);
    let raw_interest = compute_interest_score(input.embedding, &ctx.interests);

    // Calibrate: stretch compressed similarity scores to use full [0.05-0.95] range
    let context_score = calibrate_score(raw_context);
    let interest_score = calibrate_score(raw_interest);

    // Keyword interest matching: boosts items containing declared interest terms
    let keyword_score = compute_keyword_interest_score(input.title, input.content, &ctx.interests);

    // Semantic boost with keyword fallback
    let semantic_boost =
        compute_semantic_ace_boost(input.embedding, &ctx.ace_ctx, &ctx.topic_embeddings)
            .unwrap_or_else(|| compute_keyword_ace_boost(&topics, &ctx.ace_ctx));

    // Base score weighted by available data — smooth interpolation avoids cliff effects
    let base_score = if ctx.cached_context_count > 0 && ctx.interest_count > 0 {
        // Smoothly shift weight toward context as context_score increases
        // context_score=0.0 → ctx_w=0.15, context_score=1.0 → ctx_w=0.55
        let ctx_w = (0.15 + context_score * 0.40).clamp(0.15, 0.55);
        let remaining = 1.0 - ctx_w;
        let int_w = remaining * 0.55; // interests get ~55% of remainder
        let kw_w = remaining * 0.45; // keywords get ~45% of remainder
        (context_score * ctx_w + interest_score * int_w + keyword_score * kw_w + semantic_boost)
            .min(1.0)
    } else if ctx.interest_count > 0 {
        (interest_score * 0.45 + keyword_score * 0.35 + semantic_boost * 1.2).min(1.0)
    } else if ctx.cached_context_count > 0 {
        (context_score + semantic_boost).min(1.0)
    } else {
        (semantic_boost * 2.0).min(1.0)
    };

    // Optional freshness
    let freshness = if options.apply_freshness {
        if let Some(created_at) = input.created_at {
            compute_temporal_freshness(created_at)
        } else {
            1.0
        }
    } else {
        1.0
    };
    let base_score = (base_score * freshness).clamp(0.0, 1.0);

    // Source quality boost from learned preferences (capped +/-10%)
    let source_quality_boost = ctx
        .source_quality
        .get(input.source_type)
        .copied()
        .map(|score| (score * 0.10).clamp(-0.10, 0.10))
        .unwrap_or(0.0);
    let base_score = (base_score + source_quality_boost).clamp(0.0, 1.0);

    // Feedback learning boost (Phase 9): apply feedback-derived topic multiplier
    let feedback_boost = if !ctx.feedback_boosts.is_empty() {
        let mut boost_sum: f64 = 0.0;
        let mut match_count = 0;
        for topic in &topics {
            if let Some(&net_score) = ctx.feedback_boosts.get(topic.as_str()) {
                boost_sum += net_score;
                match_count += 1;
            }
        }
        if match_count > 0 {
            // Scale: net_score ranges from -1.0 to 1.0
            // Apply as +-15% boost per matching topic, capped at +-20%
            ((boost_sum / match_count as f64) * 0.15).clamp(-0.20, 0.20) as f32
        } else {
            0.0
        }
    } else {
        0.0
    };
    let base_score = (base_score + feedback_boost).clamp(0.0, 1.0);

    // Unified scoring
    let combined_score = compute_unified_relevance(base_score, &topics, &ctx.ace_ctx);
    let relevant = combined_score >= get_relevance_threshold();

    let affinity_mult = compute_affinity_multiplier(&topics, &ctx.ace_ctx);
    let anti_penalty = compute_anti_penalty(&topics, &ctx.ace_ctx);

    // Explanation
    let explanation = if relevant || combined_score >= 0.3 {
        Some(generate_relevance_explanation(
            input.title,
            context_score,
            interest_score,
            &matches,
            &ctx.ace_ctx,
            &topics,
            &ctx.interests,
        ))
    } else {
        None
    };

    // Confidence
    let confidence = calculate_confidence(
        context_score,
        interest_score,
        semantic_boost,
        &ctx.ace_ctx,
        &topics,
        ctx.cached_context_count,
        ctx.interest_count as i64,
    );

    let mut confidence_by_signal = std::collections::HashMap::new();
    if ctx.cached_context_count > 0 {
        confidence_by_signal.insert("context".to_string(), context_score);
    }
    if ctx.interest_count > 0 {
        confidence_by_signal.insert("interest".to_string(), interest_score);
    }
    if semantic_boost > 0.0 {
        confidence_by_signal.insert("ace_boost".to_string(), semantic_boost);
    }

    let score_breakdown = ScoreBreakdown {
        context_score,
        interest_score,
        keyword_score,
        ace_boost: semantic_boost,
        affinity_mult,
        anti_penalty,
        freshness_mult: freshness,
        feedback_boost,
        source_quality_boost,
        confidence_by_signal,
    };

    // Optional signal classification — only classify items that pass the relevance threshold
    let (sig_type, sig_priority, sig_action, sig_triggers) = if options.apply_signals && relevant {
        if let Some(clf) = classifier {
            match clf.classify(
                input.title,
                input.content,
                combined_score,
                &ctx.ace_ctx.detected_tech,
            ) {
                Some(c) => (
                    Some(c.signal_type.slug().to_string()),
                    Some(c.priority.label().to_string()),
                    Some(c.action),
                    Some(c.triggers),
                ),
                None => (None, None, None, None),
            }
        } else {
            (None, None, None, None)
        }
    } else {
        (None, None, None, None)
    };

    SourceRelevance {
        id: input.id,
        title: crate::decode_html_entities(input.title),
        url: input.url.map(|s| s.to_string()),
        top_score: combined_score,
        matches,
        relevant,
        context_score,
        interest_score,
        excluded: false,
        excluded_by: None,
        source_type: input.source_type.to_string(),
        explanation,
        confidence: Some(confidence),
        score_breakdown: Some(score_breakdown),
        signal_type: sig_type,
        signal_priority: sig_priority,
        signal_action: sig_action,
        signal_triggers: sig_triggers,
    }
}

/// Keyword-based ACE boost fallback when embeddings unavailable
/// Both topics (from extract_topics) and ace_ctx fields are already lowercase
fn compute_keyword_ace_boost(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    let mut boost: f32 = 0.0;
    for topic in topics {
        for active in &ace_ctx.active_topics {
            if topic.contains(active.as_str()) || active.contains(topic.as_str()) {
                boost += 0.15 * ace_ctx.topic_confidence.get(active).copied().unwrap_or(0.5);
                break;
            }
        }
        for tech in &ace_ctx.detected_tech {
            if topic.contains(tech.as_str()) || tech.contains(topic.as_str()) {
                boost += 0.12;
                break;
            }
        }
    }
    boost.clamp(0.0, 0.3)
}

/// Sort results: excluded items last, then by score descending
pub(crate) fn sort_results(results: &mut [SourceRelevance]) {
    results.sort_by(|a, b| {
        if a.excluded && !b.excluded {
            return std::cmp::Ordering::Greater;
        }
        if !a.excluded && b.excluded {
            return std::cmp::Ordering::Less;
        }
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Deduplicate scored results by URL and normalized title.
/// Keeps the highest-scoring item when duplicates are found.
pub(crate) fn dedup_results(results: &mut Vec<SourceRelevance>) {
    let initial = results.len();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Sort by score desc first so we keep the highest-scoring version
    results.sort_by(|a, b| {
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results.retain(|item| {
        // URL-based dedup
        if let Some(ref url) = item.url {
            let normalized = normalize_result_url(url);
            if !normalized.is_empty() && !seen_urls.insert(normalized) {
                return false;
            }
        }
        // Title-based dedup (strip punctuation, normalize whitespace)
        let title_key = normalize_result_title(&item.title);
        if !title_key.is_empty() && !seen_titles.insert(title_key) {
            return false;
        }
        true
    });

    let removed = initial - results.len();
    if removed > 0 {
        info!(target: "4da::scoring", removed = removed, kept = results.len(), "Post-scoring deduplication");
    }
}

fn normalize_result_url(url: &str) -> String {
    url.trim()
        .split('#')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url)
        .replace("http://", "https://")
        .replace("://www.", "://")
        .trim_end_matches('/')
        .to_lowercase()
}

fn normalize_result_title(title: &str) -> String {
    let decoded = crate::decode_html_entities(title);
    decoded
        .trim()
        .trim_start_matches("Show HN:")
        .trim_start_matches("Ask HN:")
        .trim_start_matches("Tell HN:")
        .trim_start_matches("Launch HN:")
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
        assert_eq!(freshness, 1.20, "Items just created should get max boost");
    }

    #[test]
    fn test_temporal_freshness_few_hours() {
        let four_hours_ago = chrono::Utc::now() - chrono::Duration::hours(4);
        let freshness = compute_temporal_freshness(&four_hours_ago);
        assert_eq!(freshness, 1.15, "Items 4h old should get 1.15x boost");
    }

    #[test]
    fn test_temporal_freshness_half_day() {
        let thirteen_hours_ago = chrono::Utc::now() - chrono::Duration::hours(13);
        let freshness = compute_temporal_freshness(&thirteen_hours_ago);
        assert_eq!(freshness, 1.10, "Items 13h old should get 1.10x boost");
    }

    #[test]
    fn test_temporal_freshness_one_day() {
        let thirty_hours_ago = chrono::Utc::now() - chrono::Duration::hours(30);
        let freshness = compute_temporal_freshness(&thirty_hours_ago);
        assert_eq!(freshness, 1.0, "Items 30h old should be neutral");
    }

    #[test]
    fn test_temporal_freshness_old() {
        let four_days_ago = chrono::Utc::now() - chrono::Duration::hours(96);
        let freshness = compute_temporal_freshness(&four_days_ago);
        assert_eq!(freshness, 0.92, "Items 4 days old should decay to 0.92");
    }

    #[test]
    fn test_temporal_freshness_very_old() {
        let old = chrono::Utc::now() - chrono::Duration::hours(200);
        let freshness = compute_temporal_freshness(&old);
        assert_eq!(freshness, 0.82, "Items 8+ days old should decay to 0.82");
    }

    // Test source quality boost: positive score
    #[test]
    fn test_source_quality_positive_boost() {
        let score = 0.5_f32;
        let source_score = 0.8_f32;
        let boost = (source_score * 0.10).clamp(-0.10, 0.10);
        let result = (score + boost).clamp(0.0, 1.0);
        assert!(
            (result - 0.58).abs() < 0.01,
            "Positive source should boost by up to 10%: got {}",
            result
        );
    }

    // Test source quality boost: negative reduction
    #[test]
    fn test_source_quality_negative_reduction() {
        let score = 0.5_f32;
        let source_score = -0.6_f32;
        let boost = (source_score * 0.10).clamp(-0.10, 0.10);
        let result = (score + boost).clamp(0.0, 1.0);
        assert!(
            (result - 0.44).abs() < 0.01,
            "Negative source should reduce by up to 10%: got {}",
            result
        );
    }

    // Test source quality boost: unknown source returns 0
    #[test]
    fn test_source_quality_unknown_neutral() {
        let source_quality: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        let boost = source_quality
            .get("unknown_source")
            .copied()
            .map(|score| (score * 0.10).clamp(-0.10, 0.10))
            .unwrap_or(0.0);
        assert_eq!(boost, 0.0, "Unknown source should be neutral");
    }

    // Test source quality boost: cap enforcement
    #[test]
    fn test_source_quality_cap_enforcement() {
        // Even with extreme source score, boost capped at +/-10%
        let extreme_positive = (2.0_f32 * 0.10).clamp(-0.10, 0.10);
        assert_eq!(extreme_positive, 0.10, "Positive boost should cap at 0.10");

        let extreme_negative = (-2.0_f32 * 0.10).clamp(-0.10, 0.10);
        assert_eq!(
            extreme_negative, -0.10,
            "Negative boost should cap at -0.10"
        );
    }

    #[test]
    fn test_extract_short_phrase_long_text() {
        let phrase = extract_short_phrase(
            "Vector search implementation using sqlite-vss for fast KNN queries",
        );
        assert!(phrase.contains("Vector search"));
        assert!(!phrase.is_empty());
    }

    #[test]
    fn test_extract_short_phrase_short_text() {
        let phrase = extract_short_phrase("short");
        assert!(phrase.is_empty()); // Too short to be useful
    }
}
