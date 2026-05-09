// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use tracing::info;

use crate::{extract_topics, scoring_config, SourceRelevance};

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

    // Annotate representatives
    for (rep_idx, titles) in &rep_to_titles {
        results[*rep_idx].similar_count = titles.len() as u32;
        results[*rep_idx].similar_titles = titles.clone();
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

/// Compute serendipity candidates from items that failed the confirmation gate
/// but scored well on exactly 1 axis (partial relevance, different perspective)
pub(crate) fn compute_serendipity_candidates(
    results: &[SourceRelevance],
    budget_percent: u8,
) -> Vec<SourceRelevance> {
    // Budget: how many serendipity items to include
    let total_relevant = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let budget = ((total_relevant.max(5) * budget_percent as usize) / 100).clamp(1, 5);

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
