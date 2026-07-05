// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::ace_context::ACEContext;
use super::aliases;
use super::context::is_low_quality_topic;
use crate::{context_engine, scoring_config, RelevanceMatch};
use fourda_macros::score_component;

/// Word-boundary-aware topic match: the item topic equals the active topic, or
/// contains it as a whole delimited segment. Prevents infix artifacts — e.g. the
/// active topic "os" must NOT match the item topic "macos" (one segment, not "os").
fn topic_word_match(item_topic: &str, active_topic: &str) -> bool {
    item_topic == active_topic
        || item_topic
            .split(|c: char| matches!(c, '-' | '.' | '/' | '_' | ' '))
            .any(|seg| seg == active_topic)
}

/// Dedup in place, first occurrence wins. Hit lists collect one entry per matching
/// item topic, so the same tech can appear twice ("react" + "react-dom" both hit
/// declared "react"), which previously rendered as "Uses react, react (your stack)".
fn dedup_preserve_order(hits: &mut Vec<&str>) {
    let mut seen: Vec<&str> = Vec::with_capacity(hits.len());
    hits.retain(|h| {
        if seen.contains(h) {
            false
        } else {
            seen.push(h);
            true
        }
    });
}

/// Generate a human-readable explanation for why an item was considered relevant.
/// Produces specific, actionable text naming the exact technologies/topics that matched.
/// Combines up to 2 primary reasons for multi-signal trust, plus optional annotations.
///
/// V1-pipeline legacy path. The V2 pipeline derives its explanation from the
/// ranked evidence chain instead (`explanation_chain::build_explanation_chain`
/// + `render_subtitle`) so the subtitle, chips, and expanded view all read one
/// source. Bare-count annotations ("N signals confirmed") are banned on every
/// path — a count is not evidence.
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
    //    Enriched with version info from dependency_info when available.
    let mut declared_hits: Vec<&str> = item_topics
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
    dedup_preserve_order(&mut declared_hits);
    if !declared_hits.is_empty() {
        let names: Vec<String> = declared_hits
            .iter()
            .copied()
            .take(3)
            .map(|n| {
                let nl = n.to_lowercase();
                if let Some(info) = ace_ctx.dependency_info.get(&nl) {
                    if let Some(ref v) = info.version {
                        return format!("{n} v{v}");
                    }
                }
                n.to_string()
            })
            .collect();
        for hit in &declared_hits {
            used_topics.push(hit);
        }
        parts.push(format!("Uses {} (your stack)", names.join(", ")));
    }

    // 1b. Detected-only tech matches (weaker signal — from auto-scan, not user's explicit stack)
    let mut detected_only_hits: Vec<&str> = item_topics
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
    dedup_preserve_order(&mut detected_only_hits);
    if !detected_only_hits.is_empty() {
        let names: Vec<String> = detected_only_hits
            .iter()
            .copied()
            .take(2)
            .map(|n| {
                if let Some(info) = ace_ctx.dependency_info.get(n) {
                    if let Some(ref v) = info.version {
                        return format!("{n} v{v}");
                    }
                }
                n.to_string()
            })
            .collect();
        for &n in &detected_only_hits {
            used_topics.push(n);
        }
        // Show project name(s) if available from ACE evidence
        let project_label = detected_only_hits
            .iter()
            .find_map(|&tech| {
                ace_ctx.tech_projects.get(tech).and_then(|projects| {
                    if projects.len() == 1 {
                        Some(projects[0].clone())
                    } else {
                        None
                    }
                })
            })
            .map_or_else(|| "detected in project".to_string(), |p| format!("in {p}"));
        parts.push(format!("Related to {} ({project_label})", names.join(", ")));
    }

    // 2. Active project topic matches — combine with tier 1 for multi-signal depth.
    // Gate by is_low_quality_topic (drops short / code-fragment noise like "os") and
    // require a word-boundary match (not an arbitrary infix like "macos" -> "os"), so
    // the reasoning only cites credible active topics — faithful to the same quality bar
    // synthesize_ace_interests applies before an active topic can influence the score.
    let mut topic_hits: Vec<&str> = item_topics
        .iter()
        .filter_map(|t| {
            ace_ctx
                .active_topics
                .iter()
                .find(|at| !is_low_quality_topic(at) && topic_word_match(t, at))
                .map(std::string::String::as_str)
        })
        .filter(|t| !used_topics.contains(t))
        .collect();
    dedup_preserve_order(&mut topic_hits);
    if !topic_hits.is_empty() && parts.len() < 2 {
        let names: Vec<&str> = topic_hits.iter().copied().take(2).collect();
        for &n in &names {
            used_topics.push(n);
        }
        parts.push(format!("Related to {} (active project)", names.join(", ")));
    }

    // 3. Declared interest matches — add as second reason if we only have one.
    // Word-boundary match (the full interest token occurring as a whole
    // delimited segment of the item topic) OR a curated alias-group match
    // ("reactjs" cites interest "react"), mirroring the scoring path. Raw
    // substring matching is NOT reintroduced — it claimed "Matches interest:
    // tower-http" on any item whose topic was merely "http".
    if interest_score > 0.15 && parts.len() < 2 {
        let mut interest_hits: Vec<&str> = item_topics
            .iter()
            .filter_map(|t| {
                interests
                    .iter()
                    .find(|i| {
                        let il = i.topic.to_lowercase();
                        topic_word_match(t, &il) || aliases::are_aliases(t, &il)
                    })
                    .map(|i| i.topic.as_str())
            })
            .filter(|t| {
                let tl = t.to_lowercase();
                !used_topics.iter().any(|u| *u == tl)
            })
            .collect();
        dedup_preserve_order(&mut interest_hits);
        if !interest_hits.is_empty() {
            let names: Vec<&str> = interest_hits.iter().copied().take(2).collect();
            parts.push(format!("Matches interest: {}", names.join(", ")));
        } else if parts.is_empty() {
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

    // 6. Skill gap annotation — surfaces the intelligence loop to the user.
    //    Dedup: skip techs already mentioned in the stack/detected section above
    //    to avoid "Uses react (your stack) · Closes skill gap: react" redundancy.
    if !matched_skill_gaps.is_empty() {
        let new_gaps: Vec<&str> = matched_skill_gaps
            .iter()
            .map(std::string::String::as_str)
            .filter(|g| {
                let gl = g.to_lowercase();
                !used_topics.iter().any(|u| u.to_lowercase() == gl)
            })
            .take(3)
            .collect();
        if !new_gaps.is_empty() {
            parts.push(format!("Closes skill gap: {}", new_gaps.join(", ")));
        } else if !parts.is_empty() {
            parts.push("has unread updates".to_string());
        }
    }

    parts.join(" · ")
}

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
