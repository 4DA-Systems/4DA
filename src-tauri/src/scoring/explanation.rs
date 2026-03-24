use super::ace_context::ACEContext;
use crate::{context_engine, scoring_config, RelevanceMatch};
use fourda_macros::score_component;

/// Generate a human-readable explanation for why an item was considered relevant.
/// Produces specific, actionable text naming the exact technologies/topics that matched.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_relevance_explanation(
    _title: &str,
    context_score: f32,
    interest_score: f32,
    matches: &[RelevanceMatch],
    ace_ctx: &ACEContext,
    item_topics: &[String],
    interests: &[context_engine::Interest],
    declared_tech: &[String],
    matched_skill_gaps: &[String],
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut used_topics: Vec<&str> = Vec::new();

    // 1. Declared tech stack matches (highest priority — user's explicit stack)
    let declared_hits: Vec<&str> = item_topics
        .iter()
        .filter_map(|t| {
            declared_tech
                .iter()
                .find(|tech| {
                    let tl = tech.to_lowercase();
                    *t == tl || t.contains(tl.as_str())
                })
                .map(std::string::String::as_str)
        })
        .collect();
    if !declared_hits.is_empty() {
        let names: Vec<&str> = declared_hits.iter().copied().take(3).collect();
        for &n in &names {
            used_topics.push(n);
        }
        parts.push(format!("Uses {} (your stack)", names.join(", ")));
    }

    // 1b. Detected-only tech matches (weaker signal — from auto-scan, not user's explicit stack)
    let detected_only_hits: Vec<&str> = item_topics
        .iter()
        .filter_map(|t| {
            ace_ctx
                .detected_tech
                .iter()
                .find(|tech| *tech == t || t.contains(tech.as_str()))
                .map(std::string::String::as_str)
        })
        .filter(|t| !used_topics.contains(t))
        .collect();
    if !detected_only_hits.is_empty() && declared_hits.is_empty() {
        // Only show detected tech if no declared tech matched (avoid confusing "python (detected)" next to "rust (your stack)")
        let names: Vec<&str> = detected_only_hits.iter().copied().take(2).collect();
        for &n in &names {
            used_topics.push(n);
        }
        parts.push(format!(
            "Related to {} (detected in project)",
            names.join(", ")
        ));
    }

    // 2. Active project topic matches
    let topic_hits: Vec<&str> = item_topics
        .iter()
        .filter_map(|t| {
            ace_ctx
                .active_topics
                .iter()
                .find(|at| *at == t || t.contains(at.as_str()))
                .map(std::string::String::as_str)
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
                    parts.push(format!("Matches your project context: \"{phrase}\""));
                }
            }
        }
    }

    // 4. Learned affinity (only if nothing else matched)
    if parts.is_empty() {
        for topic in item_topics {
            if let Some((score, _)) = ace_ctx.topic_affinities.get(topic.as_str()) {
                if *score > 0.3 {
                    parts.push(format!("You engage with {topic} content"));
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
                parts.push(format!("Similar to your code: \"{phrase}\""));
            }
        }
    }

    // 6. Skill gap annotation — surfaces the intelligence loop to the user
    if !matched_skill_gaps.is_empty() {
        let names: Vec<&str> = matched_skill_gaps
            .iter()
            .map(std::string::String::as_str)
            .take(3)
            .collect();
        parts.push(format!("Closes skill gap: {}", names.join(", ")));
    }

    // Return empty string instead of vague fallback — the frontend handles empty gracefully
    parts.join(" · ")
}

/// Extract a short meaningful phrase from matched context text
pub(crate) fn extract_short_phrase(matched_text: &str) -> String {
    let clean = matched_text.trim().trim_end_matches("...");
    let phrase = clean
        .find(['.', '\n'])
        .filter(|&pos| pos > 10)
        .map_or(&clean[..clean.len().min(80)], |pos| &clean[..pos])
        .trim();
    if phrase.len() < 10 {
        String::new()
    } else {
        phrase.to_string()
    }
}

/// Temporal freshness multiplier for PASIFA scoring.
/// Recent items get a slight boost, older items decay gently.
/// Returns a multiplier in [0.80, 1.10] range (tightened to reduce freshness bias):
///   - Items < 3 hours old: 1.10x boost (very fresh)
///   - Items 3-12 hours old: 1.08x boost
///   - Items 12-24 hours old: 1.05x boost
///   - Items 24-72 hours old: 1.0x (neutral)
///   - Items 3-7 days old: 0.92x decay
///   - Items 1-4 weeks old: 0.85x decay
///   - Items > 1 month old: 0.80x floor
#[score_component(output_range = "0.8..=1.1")]
pub(crate) fn compute_temporal_freshness(created_at: &chrono::DateTime<chrono::Utc>) -> f32 {
    let age_hours = ((chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0).max(0.0);

    scoring_config::freshness_multiplier(age_hours)
}

/// Calculate confidence score based on available signals and confirmation count.
/// Returns a value between 0.0 and 1.0 indicating how confident we are in the scoring.
/// The confirmation_count directly scales confidence: more confirmed axes = higher confidence.
#[score_component(output_range = "0.0..=1.0")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_confidence(
    context_score: f32,
    interest_score: f32,
    _semantic_boost: f32,
    ace_ctx: &ACEContext,
    topics: &[String],
    cached_context_count: i64,
    interest_count: i64,
    confirmation_count: u8,
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
        return scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL; // Low confidence - no strong signals
    }

    // Combine signals: average with bonus for confirmation count
    let avg_confidence = confidence_signals.iter().sum::<f32>() / confidence_signals.len() as f32;

    // Confirmation count directly scales confidence:
    // 0 confirmed → -0.15, 1 confirmed → 0.0, 2 confirmed → +0.10, 3 → +0.15, 4 → +0.20
    let idx = (confirmation_count as usize).min(scoring_config::CONFIDENCE_BONUSES.len() - 1);
    let confirmation_bonus = scoring_config::CONFIDENCE_BONUSES[idx];

    (avg_confidence + confirmation_bonus).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_temporal_freshness_very_recent() {
        let now = chrono::Utc::now();
        let freshness = compute_temporal_freshness(&now);
        assert_eq!(freshness, 1.10, "Items just created should get max boost");
    }

    #[test]
    fn test_temporal_freshness_few_hours() {
        let four_hours_ago = chrono::Utc::now() - chrono::Duration::hours(4);
        let freshness = compute_temporal_freshness(&four_hours_ago);
        assert_eq!(freshness, 1.08, "Items 4h old should get 1.08x boost");
    }

    #[test]
    fn test_temporal_freshness_half_day() {
        let thirteen_hours_ago = chrono::Utc::now() - chrono::Duration::hours(13);
        let freshness = compute_temporal_freshness(&thirteen_hours_ago);
        assert_eq!(freshness, 1.05, "Items 13h old should get 1.05x boost");
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
        assert_eq!(freshness, 0.85, "Items 8+ days old should decay to 0.85");
    }

    // ====================================================================
    // extract_short_phrase additional tests
    // ====================================================================

    #[test]
    fn test_extract_short_phrase_with_period() {
        let phrase = extract_short_phrase(
            "Vector search is powerful. It enables fast nearest neighbor lookups.",
        );
        // Should stop at the first period
        assert!(phrase.contains("Vector search"));
        assert!(!phrase.contains("enables"));
    }

    #[test]
    fn test_extract_short_phrase_with_newline() {
        let phrase = extract_short_phrase(
            "Async runtime improvements\nThe new version includes better scheduling",
        );
        // Should stop at the newline
        assert!(phrase.contains("Async runtime"));
        assert!(!phrase.contains("new version"));
    }

    #[test]
    fn test_extract_short_phrase_with_ellipsis() {
        let phrase = extract_short_phrase("A long context about development practices...");
        assert!(!phrase.ends_with("..."));
    }

    #[test]
    fn test_extract_short_phrase_too_short_returns_empty() {
        assert!(extract_short_phrase("tiny").is_empty());
        assert!(extract_short_phrase("ab").is_empty());
        assert!(extract_short_phrase("").is_empty());
    }

    #[test]
    fn test_extract_short_phrase_exactly_ten_chars() {
        // 10 chars should be included
        let phrase = extract_short_phrase("abcdefghij");
        assert_eq!(phrase, "abcdefghij");
    }

    #[test]
    fn test_extract_short_phrase_nine_chars_empty() {
        // 9 chars should be too short
        let phrase = extract_short_phrase("abcdefghi");
        assert!(phrase.is_empty());
    }

    // ====================================================================
    // calculate_confidence tests
    // ====================================================================

    #[test]
    fn test_calculate_confidence_no_signals() {
        let ctx = ACEContext::default();
        let confidence = calculate_confidence(0.0, 0.0, 0.0, &ctx, &[], 0, 0, 0);
        assert_eq!(confidence, scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL);
    }

    #[test]
    fn test_calculate_confidence_context_only() {
        let ctx = ACEContext::default();
        let confidence = calculate_confidence(0.8, 0.0, 0.0, &ctx, &[], 10, 0, 1);
        assert!(confidence > scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL);
    }

    #[test]
    fn test_calculate_confidence_higher_confirmation_boosts() {
        let ctx = ACEContext::default();
        let conf_1 = calculate_confidence(0.5, 0.5, 0.0, &ctx, &[], 10, 5, 1);
        let conf_3 = calculate_confidence(0.5, 0.5, 0.0, &ctx, &[], 10, 5, 3);
        assert!(
            conf_3 > conf_1,
            "More confirmed signals should increase confidence: {} > {}",
            conf_3,
            conf_1
        );
    }

    #[test]
    fn test_calculate_confidence_clamped() {
        let ctx = ACEContext::default();
        let confidence = calculate_confidence(1.0, 1.0, 1.0, &ctx, &[], 100, 100, 5);
        assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
        assert!(confidence >= 0.0, "Confidence should not be negative");
    }

    #[test]
    fn test_calculate_confidence_with_topic_affinities() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9));
        let topics = vec!["rust".to_string()];
        let confidence = calculate_confidence(0.5, 0.0, 0.0, &ctx, &topics, 10, 0, 2);
        // Should be higher than without affinities since we have an additional signal
        let conf_no_aff =
            calculate_confidence(0.5, 0.0, 0.0, &ACEContext::default(), &topics, 10, 0, 2);
        assert!(
            confidence >= conf_no_aff,
            "Topic affinities should boost or maintain confidence"
        );
    }

    // ====================================================================
    // generate_relevance_explanation tests
    // ====================================================================

    #[test]
    fn test_generate_explanation_declared_tech() {
        let ace_ctx = ACEContext {
            detected_tech: vec!["rust".to_string()],
            ..Default::default()
        };
        let explanation = generate_relevance_explanation(
            "Rust Performance Tips",
            0.2,
            0.2,
            &[],
            &ace_ctx,
            &["rust".to_string()],
            &[],
            &["Rust".to_string()],
            &[],
        );
        assert!(
            explanation.contains("your stack"),
            "Should mention 'your stack': {}",
            explanation
        );
    }

    #[test]
    fn test_generate_explanation_skill_gap_annotation() {
        let ace_ctx = ACEContext::default();
        let explanation = generate_relevance_explanation(
            "Getting started with Tokio async runtime",
            0.1,
            0.1,
            &[],
            &ace_ctx,
            &["tokio".to_string()],
            &[],
            &[],
            &["tokio".to_string()],
        );
        assert!(
            explanation.contains("Closes skill gap: tokio"),
            "Should annotate skill gap: {}",
            explanation
        );
    }

    #[test]
    fn test_generate_explanation_skill_gap_with_stack() {
        let ace_ctx = ACEContext {
            detected_tech: vec!["rust".to_string()],
            ..Default::default()
        };
        let explanation = generate_relevance_explanation(
            "Tokio and Rust async patterns",
            0.2,
            0.2,
            &[],
            &ace_ctx,
            &["rust".to_string(), "tokio".to_string()],
            &[],
            &["Rust".to_string()],
            &["tokio".to_string()],
        );
        assert!(
            explanation.contains("your stack"),
            "Should still show stack match: {}",
            explanation
        );
        assert!(
            explanation.contains("Closes skill gap: tokio"),
            "Should also show skill gap: {}",
            explanation
        );
    }

    #[test]
    fn test_generate_explanation_empty_when_no_signals() {
        let ace_ctx = ACEContext::default();
        let explanation = generate_relevance_explanation(
            "Some Random Title",
            0.1,
            0.1,
            &[],
            &ace_ctx,
            &["random".to_string()],
            &[],
            &[],
            &[],
        );
        // With no tech, no interest, no affinity, no context, should be empty
        assert!(
            explanation.is_empty(),
            "Should be empty with no signals: '{}'",
            explanation
        );
    }
}
