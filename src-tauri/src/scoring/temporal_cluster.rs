//! Temporal clustering: groups items about the same event or announcement.
//!
//! Runs after topic_dedup to catch remaining same-event duplicates that
//! single-keyword topic matching missed. Uses multi-word title overlap
//! (Jaccard similarity) to detect items covering the same news.
//!
//! Example: "TypeScript 6.0 Released" (HN) + "First Look at TypeScript 6.0" (Reddit)
//! → grouped into one cluster, highest-scoring kept as representative.

use tracing::info;

use crate::SourceRelevance;

/// Minimum Jaccard similarity of title words to consider items as same-event.
/// 0.35 catches "TypeScript 6.0 Released" vs "First Look at TypeScript 6.0"
/// while avoiding false positives like "Rust async patterns" vs "Rust error handling".
const SIMILARITY_THRESHOLD: f32 = 0.35;

/// Stop words excluded from Jaccard computation (too common to be meaningful).
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "it", "its", "this", "that", "are", "was", "were", "be", "been", "being", "have",
    "has", "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "can",
    "shall", "not", "no", "how", "what", "when", "where", "why", "who", "which", "all", "each",
    "every", "both", "few", "more", "most", "other", "some", "such", "than", "too", "very", "just",
    "about", "above", "after", "again", "also", "as", "into", "new", "now", "our", "out", "up",
    "your", "you", "we", "i",
];

/// Cluster items about the same event using title word overlap.
///
/// Augments existing `similar_count` / `similar_titles` fields set by topic_dedup.
/// Appends source type to grouped titles for cross-source visibility.
pub(crate) fn temporal_cluster_results(results: &mut Vec<SourceRelevance>) {
    if results.len() < 2 {
        return;
    }

    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

    // Extract normalized word sets for each item
    let word_sets: Vec<std::collections::HashSet<String>> = results
        .iter()
        .map(|r| title_words(&r.title, &stop_set))
        .collect();

    // Union-Find for transitive clustering
    let n = results.len();
    let mut parent: Vec<usize> = (0..n).collect();

    // Compare all pairs — O(n^2) but n is typically <200 after dedup
    for i in 0..n {
        if results[i].excluded {
            continue;
        }
        for j in (i + 1)..n {
            if results[j].excluded {
                continue;
            }
            // Skip if already in the same cluster
            if find(&mut parent, i) == find(&mut parent, j) {
                continue;
            }
            let sim = jaccard(&word_sets[i], &word_sets[j]);
            if sim >= SIMILARITY_THRESHOLD {
                union(&mut parent, i, j);
            }
        }
    }

    // Group indices by cluster root
    let mut clusters: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        clusters.entry(root).or_default().push(i);
    }

    // Filter to clusters with 2+ members
    let multi_clusters: Vec<Vec<usize>> = clusters
        .into_values()
        .filter(|members| members.len() >= 2)
        .collect();

    if multi_clusters.is_empty() {
        return;
    }

    // For each cluster: pick the highest-scoring representative, annotate, mark rest for removal
    let mut remove_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for members in &multi_clusters {
        // Representative = highest score (results are pre-sorted by score desc,
        // so the first member in index order with lowest index is highest-scored)
        let rep_idx = match members.iter().min_by(|&&a, &&b| {
            results[b]
                .top_score
                .partial_cmp(&results[a].top_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            Some(&idx) => idx,
            None => continue,
        };

        let mut extra_titles = Vec::new();
        for &idx in members {
            if idx != rep_idx {
                // Format: "Title (source_type)" for cross-source visibility
                let source = &results[idx].source_type;
                extra_titles.push(format!("{} ({})", results[idx].title, source));
                remove_indices.insert(idx);
            }
        }

        // Augment representative's existing similar_count/similar_titles
        // (topic_dedup may have already set these)
        results[rep_idx].similar_count += extra_titles.len() as u32;
        results[rep_idx].similar_titles.extend(extra_titles);
    }

    // Remove clustered items
    let total_removed = remove_indices.len();
    let mut idx = 0;
    results.retain(|_| {
        let keep = !remove_indices.contains(&idx);
        idx += 1;
        keep
    });

    if total_removed > 0 {
        info!(
            target: "4da::scoring",
            removed = total_removed,
            clusters = multi_clusters.len(),
            "Temporal clustering"
        );
    }
}

/// Extract meaningful words from a title (lowercased, stop words removed, min 2 chars).
fn title_words(
    title: &str,
    stop_words: &std::collections::HashSet<&str>,
) -> std::collections::HashSet<String> {
    let decoded = crate::decode_html_entities(title);
    decoded
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '+')
        .filter(|w| w.len() >= 2 && !stop_words.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Jaccard similarity: |A ∩ B| / |A ∪ B|
fn jaccard(a: &std::collections::HashSet<String>, b: &std::collections::HashSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union_size = a.union(b).count();
    intersection as f32 / union_size as f32
}

// ── Union-Find ──────────────────────────────────────────────────────────

fn find(parent: &mut [usize], i: usize) -> usize {
    if parent[i] != i {
        parent[i] = find(parent, parent[i]);
    }
    parent[i]
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent[rb] = ra;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(title: &str, source: &str, score: f32) -> SourceRelevance {
        SourceRelevance {
            id: 0,
            title: title.to_string(),
            url: None,
            top_score: score,
            matches: vec![],
            relevant: true,
            context_score: 0.0,
            interest_score: 0.0,
            excluded: false,
            excluded_by: None,
            source_type: source.to_string(),
            explanation: None,
            confidence: None,
            score_breakdown: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
        }
    }

    #[test]
    fn test_same_event_different_sources_clustered() {
        let mut items = vec![
            make_item(
                "TypeScript 6.0 Released with Pattern Matching",
                "hackernews",
                0.85,
            ),
            make_item(
                "TypeScript 6.0 Beta: First Look at Pattern Matching",
                "reddit",
                0.70,
            ),
            make_item(
                "Introducing TypeScript 6.0 and Pattern Matching",
                "devto",
                0.60,
            ),
            make_item(
                "Rust async runtime comparison: tokio vs smol",
                "hackernews",
                0.75,
            ),
        ];

        temporal_cluster_results(&mut items);

        // TS 6.0 articles should be clustered, Rust article should survive
        assert_eq!(
            items.len(),
            2,
            "Should have 2 items: TS cluster rep + Rust article"
        );
        // Highest-scored TS article (0.85) is the representative
        assert_eq!(items[0].top_score, 0.85);
        assert_eq!(items[0].similar_count, 2);
        assert_eq!(items[0].similar_titles.len(), 2);
        // Rust article untouched
        assert!(items[1].title.contains("Rust"));
    }

    #[test]
    fn test_unrelated_items_not_clustered() {
        let mut items = vec![
            make_item(
                "New CSS container queries support in Chrome",
                "hackernews",
                0.80,
            ),
            make_item("Rust memory safety guarantees explained", "reddit", 0.75),
            make_item("PostgreSQL 17 performance improvements", "hackernews", 0.70),
        ];

        temporal_cluster_results(&mut items);

        assert_eq!(items.len(), 3, "Unrelated items should not be clustered");
        assert!(items.iter().all(|i| i.similar_count == 0));
    }

    #[test]
    fn test_empty_and_single_item() {
        let mut empty: Vec<SourceRelevance> = vec![];
        temporal_cluster_results(&mut empty);
        assert!(empty.is_empty());

        let mut single = vec![make_item("Solo article", "hackernews", 0.80)];
        temporal_cluster_results(&mut single);
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn test_excluded_items_skipped() {
        let mut items = vec![make_item("TypeScript 6.0 Released", "hackernews", 0.85), {
            let mut item = make_item("TypeScript 6.0 Overview", "reddit", 0.70);
            item.excluded = true;
            item
        }];

        temporal_cluster_results(&mut items);

        // Excluded item should not participate in clustering
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_augments_existing_topic_dedup() {
        let mut items = vec![
            {
                let mut item =
                    make_item("Tauri 3.0 Desktop Framework Released", "hackernews", 0.90);
                // Simulate topic_dedup already grouped 1 item
                item.similar_count = 1;
                item.similar_titles = vec!["Tauri 3.0 Framework (arxiv)".to_string()];
                item
            },
            make_item(
                "Tauri 3.0 Released: Desktop Apps with Web Tech",
                "reddit",
                0.65,
            ),
        ];

        temporal_cluster_results(&mut items);

        assert_eq!(items.len(), 1);
        // Should have augmented: 1 (from topic_dedup) + 1 (from temporal) = 2
        assert_eq!(items[0].similar_count, 2);
        assert_eq!(items[0].similar_titles.len(), 2);
    }

    #[test]
    fn test_jaccard_similarity() {
        let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

        let a = title_words("TypeScript 6.0 Released with Pattern Matching", &stop_set);
        let b = title_words("TypeScript 6.0: First Look at Pattern Matching", &stop_set);
        let sim = jaccard(&a, &b);
        assert!(sim >= 0.35, "Same-event titles should be similar: {sim}");

        let c = title_words("Rust memory safety explained", &stop_set);
        let d = title_words("PostgreSQL query optimization tips", &stop_set);
        let sim2 = jaccard(&c, &d);
        assert!(
            sim2 < 0.35,
            "Unrelated titles should not be similar: {sim2}"
        );
    }

    #[test]
    fn test_transitive_clustering() {
        // A is similar to B, B is similar to C → all three should cluster
        let mut items = vec![
            make_item(
                "React 20 release candidate announced today",
                "hackernews",
                0.90,
            ),
            make_item("React 20 release candidate: what's changed", "reddit", 0.80),
            make_item("What's changed in React 20 release", "devto", 0.70),
        ];

        temporal_cluster_results(&mut items);

        assert_eq!(items.len(), 1, "Transitive cluster should merge all three");
        assert_eq!(items[0].similar_count, 2);
    }
}
