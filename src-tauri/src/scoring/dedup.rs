// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use tracing::info;

use crate::{extract_topics, scoring_config, SourceRelevance};

/// Sort results: excluded items last, grounded tier first, then by score
/// descending within each tier.
///
/// Grounded-first is the plan's Phase-4 rule made binding at selection: the
/// measured truth (2026-06-21, re-confirmed 2026-07-05) is that scores cannot
/// separate signal from noise at the top — a machine-verifiable edge to the
/// user's stack (`ScoreBreakdown::strongly_grounded`, the same canonical
/// predicate the gate, pools, and Brief slate use) is the only axis a content
/// author can't fabricate. Ungrounded items still surface (deprioritized, the
/// pools UI dims them) — they just can't occupy the top slots ahead of
/// grounded evidence, which is what kept crate-name-collision noise
/// (`bevy-react-macros` at 0.90) above real on-stack releases.
pub(crate) fn sort_results(results: &mut [SourceRelevance]) {
    results.sort_by(|a, b| {
        if a.excluded != b.excluded {
            return if a.excluded {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        let a_grounded = is_strongly_grounded_result(a);
        let b_grounded = is_strongly_grounded_result(b);
        if a_grounded != b_grounded {
            return if a_grounded {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Canonical grounding verdict for a scored result: the breakdown's
/// `strongly_grounded` flag, false when no breakdown was persisted.
fn is_strongly_grounded_result(item: &SourceRelevance) -> bool {
    item.score_breakdown
        .as_ref()
        .is_some_and(|b| b.strongly_grounded)
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

/// Query parameters that identify the CAMPAIGN or the referrer, not the CONTENT.
///
/// Only these may be discarded. Both dedup passes used to throw the WHOLE query
/// string away, which meant every `youtube.com/watch?v=...` — where the query IS
/// the identity of the page — normalized to the single key
/// `https://youtube.com/watch`, so YouTube could contribute at most ONE item to
/// any batch. The same collapse hit every `?p=` / `?id=` / `?story_fbid=` style
/// permalink.
fn is_tracking_param(key: &str) -> bool {
    key.starts_with("utm_")
        || matches!(
            key,
            "ref" | "ref_src" | "fbclid" | "gclid" | "si" | "igshid"
        )
}

/// THE canonical URL identity for deduplication. Shared by BOTH passes: this
/// post-scoring one (`dedup_results`) and the pre-scoring one
/// (`analysis_rerank::analysis_dedup::dedup_stored_items`). They were two
/// independent copies of the same logic, which is how the same defect came to
/// sit in both; there is now one implementation and one place to fix.
///
/// Strips protocol variance, `www.`, trailing slash, fragment, and tracking
/// parameters — but PRESERVES content-bearing query parameters, sorted by key so
/// that parameter order cannot defeat dedup.
///
/// Keys are lowercased (servers treat them case-insensitively in practice);
/// VALUES keep their case, because they are identifiers — YouTube's
/// `v=dQw4w9WgXcQ` is a different video from `v=dqw4w9wgxcq`, and folding them
/// together would silently drop a real item.
pub(crate) fn normalize_result_url(url: &str) -> String {
    let url = url.trim();
    let without_fragment = url.split('#').next().unwrap_or(url);
    let (path_part, query_part) = match without_fragment.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (without_fragment, None),
    };

    let base = path_part
        .replace("http://", "https://")
        .replace("://www.", "://")
        .trim_end_matches('/')
        .to_lowercase();

    let Some(query) = query_part else {
        return base;
    };
    let mut params: Vec<(String, Option<&str>)> = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (key.to_lowercase(), Some(value)),
            None => (pair.to_lowercase(), None),
        })
        .filter(|(key, _)| !is_tracking_param(key))
        .collect();

    if params.is_empty() {
        return base;
    }

    params.sort_unstable();
    let normalized_query = params
        .iter()
        .map(|(key, value)| match value {
            Some(v) => format!("{key}={v}"),
            None => key.clone(),
        })
        .collect::<Vec<_>>()
        .join("&");

    format!("{base}?{normalized_query}")
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

/// Compute Jaccard similarity between two title strings based on word tokens.
/// Returns 0.0 (no overlap) to 1.0 (identical word sets).
/// Used to catch near-duplicate content that URL and exact-title dedup miss
/// (cross-posts, minor title variations, same content from different sources).
fn jaccard_word_similarity(a: &str, b: &str) -> f32 {
    let words_a: std::collections::HashSet<&str> =
        a.split_whitespace().filter(|w| w.len() >= 2).collect();
    let words_b: std::collections::HashSet<&str> =
        b.split_whitespace().filter(|w| w.len() >= 2).collect();

    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Fuzzy title deduplication: catches near-duplicates that URL/exact-title dedup miss.
/// Uses Jaccard word similarity on normalized titles. Items with >= 0.65 word overlap
/// are considered duplicates — the higher-scoring item survives.
/// This catches cross-posted content and minor title variations.
pub(crate) fn fuzzy_dedup_results(results: &mut Vec<SourceRelevance>) {
    if results.len() < 2 {
        return;
    }

    let initial = results.len();

    // Pre-compute normalized titles
    let normalized: Vec<String> = results
        .iter()
        .map(|item| normalize_result_title(&item.title))
        .collect();

    // Track which indices to remove (results are sorted desc, so i < j means i scored higher)
    let mut remove_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for i in 0..results.len() {
        if remove_indices.contains(&i) || results[i].excluded {
            continue;
        }
        for j in (i + 1)..results.len() {
            if remove_indices.contains(&j) || results[j].excluded {
                continue;
            }
            let similarity = jaccard_word_similarity(&normalized[i], &normalized[j]);
            if similarity >= 0.65 {
                // j scored lower (results sorted desc) — mark for removal
                remove_indices.insert(j);
            }
        }
    }

    if remove_indices.is_empty() {
        return;
    }

    // Annotate survivors with similar titles from their fuzzy duplicates
    for &removed_idx in &remove_indices {
        let removed_title = results[removed_idx].title.clone();
        for i in 0..results.len() {
            if remove_indices.contains(&i) || i == removed_idx {
                continue;
            }
            let sim = jaccard_word_similarity(&normalized[i], &normalized[removed_idx]);
            if sim >= 0.65 {
                results[i].similar_count += 1;
                results[i].similar_titles.push(removed_title);
                break;
            }
        }
    }

    // Remove fuzzy duplicates
    let mut idx = 0;
    results.retain(|_| {
        let keep = !remove_indices.contains(&idx);
        idx += 1;
        keep
    });

    let removed = initial - results.len();
    if removed > 0 {
        info!(target: "4da::scoring", removed = removed, kept = results.len(), "Fuzzy title deduplication");
    }
}

/// Topic-level deduplication: groups items sharing the same primary extracted topic.
/// Keeps the highest-scoring item as representative and annotates with similar count/titles.
/// Must be called after sort_results() so highest-scored items come first.
pub(crate) fn topic_dedup_results(results: &mut Vec<SourceRelevance>) {
    if results.len() < 2 {
        return;
    }

    let mut topic_to_representative: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut grouped_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

    // For each item, extract topics from title and find if it shares a primary topic with an earlier item
    for (i, item) in results.iter().enumerate() {
        if item.excluded || grouped_indices.contains(&i) {
            continue;
        }
        let topics = extract_topics(&item.title, "", &[]);
        for topic in &topics {
            // Skip short/stopword topics
            if topic.len() < 3 {
                continue;
            }
            if let Some(&rep_idx) = topic_to_representative.get(topic.as_str()) {
                if rep_idx != i {
                    // Only dedup if this item scores significantly lower than representative.
                    // Items within 0.10 of each other both survive (different perspectives).
                    let rep_score = results[rep_idx].top_score;
                    let this_score = results[i].top_score;
                    if rep_score - this_score > 0.10 {
                        grouped_indices.insert(i);
                        break;
                    }
                }
            } else {
                // First time seeing this topic — this item is the representative
                topic_to_representative.insert(topic.clone(), i);
            }
        }
    }

    if grouped_indices.is_empty() {
        return;
    }

    // Collect titles of grouped items and annotate representatives
    // Build a map: representative_index -> Vec<grouped_title>
    let mut rep_to_titles: std::collections::HashMap<usize, Vec<String>> =
        std::collections::HashMap::new();

    for &gi in &grouped_indices {
        let grouped_topics = extract_topics(&results[gi].title, "", &[]);
        for topic in &grouped_topics {
            if topic.len() < 3 {
                continue;
            }
            if let Some(&rep_idx) = topic_to_representative.get(topic.as_str()) {
                if rep_idx != gi {
                    rep_to_titles
                        .entry(rep_idx)
                        .or_default()
                        .push(results[gi].title.clone());
                    break;
                }
            }
        }
    }

    // Annotate representatives and apply corroboration boost
    for (rep_idx, titles) in &rep_to_titles {
        results[*rep_idx].similar_count = titles.len() as u32;
        results[*rep_idx].similar_titles = titles.clone();
        // Corroboration boost: items confirmed across multiple sources are more important.
        // +0.03 per grouped item, capped at +0.09 (3 corroborating items)
        let boost = (titles.len() as f32 * 0.03).min(0.09);
        results[*rep_idx].top_score = (results[*rep_idx].top_score + boost).min(1.0);
    }

    // Remove grouped items (retain only non-grouped)
    let mut idx = 0;
    results.retain(|_| {
        let keep = !grouped_indices.contains(&idx);
        idx += 1;
        keep
    });

    let total_grouped: usize = rep_to_titles.values().map(std::vec::Vec::len).sum();
    if total_grouped > 0 {
        info!(target: "4da::scoring", grouped = total_grouped, representatives = rep_to_titles.len(), "Topic-level deduplication");
    }
}

/// Extract the registrable domain from a URL string.
/// Strips scheme, path, query, fragment, port, and `www.` prefix.
fn extract_domain(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('/').next()?;
    let host = host.split('?').next().unwrap_or(host);
    let host = host.split('#').next().unwrap_or(host);
    // Strip port
    let host = host.split(':').next().unwrap_or(host);
    // Strip www.
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

/// Apply domain diversity decay: penalize items sharing the same URL domain.
/// Items are processed in score-descending order. The first item from each domain
/// keeps its full score. Subsequent items get exponentially decayed scores.
/// This prevents feed clustering around a single prolific blog or source.
pub(crate) fn apply_domain_diversity(results: &mut [SourceRelevance]) -> usize {
    let decay = scoring_config::DOMAIN_DIVERSITY_DECAY;
    let floor = scoring_config::DOMAIN_DIVERSITY_FLOOR;

    let mut domain_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut adjusted = 0usize;

    for item in results.iter_mut() {
        if item.excluded {
            continue;
        }
        let domain = match item.url.as_deref().and_then(extract_domain) {
            Some(d) => d,
            None => continue,
        };
        let position = domain_counts.entry(domain).or_insert(0);
        if *position > 0 {
            let multiplier = (1.0 - floor) * decay.powf(*position as f32) + floor;
            item.top_score *= multiplier;
            adjusted += 1;
        }
        *position += 1;
    }

    if adjusted > 0 {
        info!(target: "4da::scoring", adjusted = adjusted, "Domain diversity applied");
    }
    adjusted
}

/// Apply source-type diversity: when multiple items share the same source type
/// AND primary topic, subsequent items get decayed to prevent one source flooding
/// results with a trending topic (e.g., 4 HN items all about "WebAssembly").
pub(crate) fn apply_source_topic_diversity(results: &mut [SourceRelevance]) -> usize {
    let mut group_counts: std::collections::HashMap<(String, String), usize> =
        std::collections::HashMap::new();
    let mut adjusted = 0usize;

    for item in results.iter_mut() {
        if item.excluded {
            continue;
        }
        let topics = extract_topics(&item.title, "", &[]);
        let primary = match topics.first() {
            Some(t) => t.clone(),
            None => continue,
        };
        let key = (item.source_type.clone(), primary);
        let count = group_counts.entry(key).or_insert(0);
        // Allow 2 items from same source+topic before decaying
        if *count >= 2 {
            let penalty = 0.85_f32.powf((*count - 1) as f32);
            item.top_score *= penalty;
            adjusted += 1;
        }
        *count += 1;
    }

    if adjusted > 0 {
        info!(target: "4da::scoring", adjusted, "Source-topic diversity applied");
    }
    adjusted
}

/// Compute serendipity candidates from items that failed the confirmation gate
/// but scored well on exactly 1 axis (partial relevance, different perspective)
pub(crate) fn compute_serendipity_candidates(
    results: &[SourceRelevance],
    budget_percent: u8,
) -> Vec<SourceRelevance> {
    // Budget: how many serendipity items to include. Budget-true, rounding
    // DOWN, capped at 5. The previous formula seeded the count with
    // `total_relevant.max(5)` and then `.clamp(1, 5)` — forcing at least one
    // scorer-REJECTED item into the feed every cycle regardless of the
    // configured budget. On a small feed (~4 relevant/cycle) those forced
    // injections accumulated to 17.6% of the curated set against a
    // configured 8% (measured live 2026-08-11). A budget of 8% on 4
    // relevant items is 0 injections, and that is what it must produce.
    let total_relevant = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let budget = ((total_relevant * budget_percent as usize) / 100).min(5);
    if budget == 0 {
        return Vec::new();
    }

    // Find items that failed the gate but had some signal
    let mut candidates: Vec<SourceRelevance> = results
        .iter()
        .filter(|r| {
            !r.relevant
            && !r.excluded
            && r.top_score > scoring_config::SERENDIPITY_MIN_SCORE // Had some score
            && (r.context_score > scoring_config::SERENDIPITY_MIN_AXIS_SCORE || r.interest_score > scoring_config::SERENDIPITY_MIN_AXIS_SCORE) // Had at least 1 axis
        })
        .cloned()
        .collect();

    // Sort by top_score (highest partial scores first)
    candidates.sort_by(|a, b| {
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Mark as serendipity and make them "relevant" so they show up
    candidates
        .into_iter()
        .take(budget)
        .map(|mut item| {
            item.serendipity = true;
            item.relevant = true;
            item.explanation = Some(
                "Serendipity: outside your usual interests but may offer a fresh perspective"
                    .to_string(),
            );
            item
        })
        .collect()
}

#[cfg(test)]
#[path = "dedup_tests.rs"]
mod tests;
