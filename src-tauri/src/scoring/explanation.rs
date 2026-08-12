// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::ace_context::ACEContext;
use crate::scoring_config;
use fourda_macros::score_component;

/// Strip leading Markdown block markers (list bullets, blockquotes, ATX headings,
/// ordered-list numerals) and emphasis runs from a snippet so quoted phrases read
/// as prose rather than raw source. Conservative by design: it only removes block
/// markers at the start and paired `**`/`` ` `` emphasis, and trims leading `*`/`_`
/// — it never touches `_` or `*` that sit mid-token, so identifiers like
/// `anthropic_ai_sdk` survive intact.
fn strip_markdown_markers(s: &str) -> String {
    let mut t = s.trim_start();
    loop {
        let before = t;
        if let Some(r) = t.strip_prefix("> ") {
            t = r.trim_start();
            continue;
        }
        if let Some(r) = t.strip_prefix("- ") {
            t = r.trim_start();
            continue;
        }
        if let Some(r) = t.strip_prefix("* ") {
            t = r.trim_start();
            continue;
        }
        if let Some(r) = t.strip_prefix("+ ") {
            t = r.trim_start();
            continue;
        }
        if t.starts_with('#') {
            let after = t.trim_start_matches('#');
            if let Some(r) = after.strip_prefix(' ') {
                t = r.trim_start();
                continue;
            }
        }
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && t[digits..].starts_with(". ") {
            t = t[digits + 2..].trim_start();
            continue;
        }
        if t == before {
            break;
        }
    }
    // Remove paired bold/code emphasis runs (safe in prose), then trim any leading
    // inline emphasis left at the very start ("*Why", "_Note").
    t.replace("**", "")
        .replace('`', "")
        .trim_start_matches(['*', '_'])
        .to_string()
}

/// Extract a short meaningful phrase from matched context text.
/// Strips HTML tags first — matched text can contain raw markup from RSS/scraped content.
pub(crate) fn extract_short_phrase(matched_text: &str) -> String {
    let stripped = crate::utils::strip_html_tags(matched_text);
    let demarked = strip_markdown_markers(stripped.trim());
    let clean = demarked.trim().trim_end_matches("...");
    let phrase = clean
        .find(['.', '\n'])
        .filter(|&pos| pos > 10)
        .map_or(
            &clean[..clean.floor_char_boundary(clean.len().min(80))],
            |pos| &clean[..pos],
        )
        .trim();
    if phrase.len() < 10 {
        String::new()
    } else {
        phrase.split_whitespace().collect::<Vec<_>>().join(" ")
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
#[path = "explanation_tests.rs"]
mod tests;
