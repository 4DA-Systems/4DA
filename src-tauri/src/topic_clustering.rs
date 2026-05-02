// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Post-scoring topic clustering for corroboration detection.
//!
//! After PASIFA scores items, this module clusters related items about the
//! same topic from different sources using embedding cosine similarity.
//! Clusters enable:
//! 1. Corroboration bonuses (+0.05 per extra source, max +0.15)
//! 2. "N sources" badges on briefing items
//! 3. Cluster-aware dedup (show lead item only, with alt sources)
//!
//! This is strictly better than the Jaccard-based fuzzy dedup in
//! `briefing_dedupe.rs` because it uses semantic similarity instead of
//! surface-level word overlap. The Jaccard dedup remains as a fast
//! pre-filter for exact/near-exact title matches; this module catches
//! semantically related items that share no title words.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::utils::cosine_similarity;

/// Cosine similarity threshold for clustering items as the same topic.
/// 0.80 is conservative — catches "React 19 released" from HN + Reddit
/// without merging "React 19 released" with "Vue 4 released".
const CLUSTER_SIMILARITY_THRESHOLD: f32 = 0.80;

/// Minimum score for an item to be considered for clustering.
/// Items below this are clear noise — don't waste cycles on them.
const CLUSTER_SCORE_FLOOR: f32 = 0.20;

/// Maximum corroboration bonus from multiple sources.
const MAX_CORROBORATION_BONUS: f32 = 0.15;

/// Per-source corroboration bonus increment.
const PER_SOURCE_BONUS: f32 = 0.05;

/// A cluster of items about the same topic from potentially different sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TopicCluster {
    /// ID of the highest-scoring item in the cluster (the representative).
    pub lead_item_id: i64,
    /// All item IDs in the cluster (including the lead).
    pub member_ids: Vec<i64>,
    /// Number of distinct source types in the cluster.
    pub source_count: usize,
    /// Bonus to apply to the lead item's score (0.0 if single source).
    pub corroboration_bonus: f32,
    /// Whether items in the cluster have conflicting sentiment.
    pub has_mixed_signals: bool,
}

/// Minimal item data needed for clustering. Constructed from briefing items
/// plus their embeddings loaded from the DB.
pub(crate) struct ClusterCandidate {
    pub id: i64,
    pub score: f32,
    pub source_type: String,
    pub embedding: Vec<f32>,
    pub title: String,
    /// Content type for future mixed-signal detection refinements.
    #[allow(dead_code)]
    pub content_type: Option<String>,
}

/// An alternative source for a clustered item — shown as "also reported by" in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AltSource {
    pub source_type: String,
    pub url: Option<String>,
    pub title: String,
}

/// Cluster items by embedding similarity.
///
/// Items with cosine similarity > 0.80 are considered the same topic.
/// Only clusters items scoring >= 0.20 (don't waste time on clear noise).
/// Uses a simple greedy single-pass algorithm: for each unassigned item,
/// find all unassigned items similar to it and form a cluster.
pub(crate) fn cluster_items(candidates: &[ClusterCandidate]) -> Vec<TopicCluster> {
    // Filter to items above noise floor with valid embeddings
    let viable: Vec<&ClusterCandidate> = candidates
        .iter()
        .filter(|c| {
            c.score >= CLUSTER_SCORE_FLOOR
                && !c.embedding.is_empty()
                && c.embedding.iter().any(|&v| v != 0.0)
        })
        .collect();

    if viable.is_empty() {
        return Vec::new();
    }

    let mut clusters: Vec<TopicCluster> = Vec::new();
    let mut assigned: HashSet<i64> = HashSet::new();

    for item in &viable {
        if assigned.contains(&item.id) {
            continue;
        }

        let mut cluster_members = vec![item.id];
        let mut cluster_sources: HashSet<String> = HashSet::new();
        cluster_sources.insert(item.source_type.clone());
        let mut best_id = item.id;
        let mut best_score = item.score;

        // Compare against all unassigned items
        for other in &viable {
            if other.id == item.id || assigned.contains(&other.id) {
                continue;
            }

            let sim = cosine_similarity(&item.embedding, &other.embedding);
            if sim > CLUSTER_SIMILARITY_THRESHOLD {
                cluster_members.push(other.id);
                cluster_sources.insert(other.source_type.clone());
                assigned.insert(other.id);
                if other.score > best_score {
                    best_id = other.id;
                    best_score = other.score;
                }
            }
        }

        assigned.insert(item.id);

        let source_count = cluster_sources.len();
        let corroboration_bonus = if source_count > 1 {
            ((source_count - 1) as f32 * PER_SOURCE_BONUS).min(MAX_CORROBORATION_BONUS)
        } else {
            0.0
        };

        let has_mixed = detect_mixed_signals(&cluster_members, candidates);

        clusters.push(TopicCluster {
            lead_item_id: best_id,
            member_ids: cluster_members,
            source_count,
            corroboration_bonus,
            has_mixed_signals: has_mixed,
        });
    }

    if clusters.iter().any(|c| c.source_count > 1) {
        let multi_source = clusters.iter().filter(|c| c.source_count > 1).count();
        info!(
            target: "4da::clustering",
            total_clusters = clusters.len(),
            multi_source_clusters = multi_source,
            "Topic clustering complete"
        );
    }

    clusters
}

/// Detect if items in a cluster have conflicting sentiment/content types.
///
/// Checks for opposing title sentiment words — e.g. one item says "improves"
/// while another says "regression". This flags clusters where the signal is
/// genuinely mixed, not just corroborated.
fn detect_mixed_signals(member_ids: &[i64], candidates: &[ClusterCandidate]) -> bool {
    if member_ids.len() < 2 {
        return false;
    }

    let members: Vec<&ClusterCandidate> = member_ids
        .iter()
        .filter_map(|id| candidates.iter().find(|c| c.id == *id))
        .collect();

    let positive_words = [
        "improves", "fixes", "faster", "better", "stable", "resolved", "upgrade",
    ];
    let negative_words = [
        "breaks",
        "regression",
        "slower",
        "leak",
        "vulnerability",
        "broken",
        "crash",
    ];

    let mut has_positive = false;
    let mut has_negative = false;

    for m in &members {
        let lower = m.title.to_lowercase();
        if positive_words.iter().any(|w| lower.contains(w)) {
            has_positive = true;
        }
        if negative_words.iter().any(|w| lower.contains(w)) {
            has_negative = true;
        }
    }

    has_positive && has_negative
}

/// Apply corroboration bonuses to scored items based on clustering.
/// Returns a map of item_id -> bonus to apply. Only the lead item of
/// each multi-source cluster receives the bonus.
pub(crate) fn compute_corroboration_bonuses(clusters: &[TopicCluster]) -> HashMap<i64, f32> {
    let mut bonuses = HashMap::new();
    for cluster in clusters {
        if cluster.corroboration_bonus > 0.0 {
            bonuses.insert(cluster.lead_item_id, cluster.corroboration_bonus);
        }
    }
    bonuses
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(id: i64, score: f32, source: &str, embedding: Vec<f32>) -> ClusterCandidate {
        ClusterCandidate {
            id,
            score,
            source_type: source.to_string(),
            embedding,
            title: format!("Item {id}"),
            content_type: None,
        }
    }

    fn make_candidate_with_title(
        id: i64,
        score: f32,
        source: &str,
        embedding: Vec<f32>,
        title: &str,
    ) -> ClusterCandidate {
        ClusterCandidate {
            id,
            score,
            source_type: source.to_string(),
            embedding,
            title: title.to_string(),
            content_type: None,
        }
    }

    #[test]
    fn test_cluster_same_topic() {
        // Two items with identical embeddings from different sources should cluster
        let emb = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            make_candidate(1, 0.80, "hackernews", emb.clone()),
            make_candidate(2, 0.70, "reddit", emb.clone()),
        ];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 1, "identical embeddings should form one cluster");
        assert_eq!(clusters[0].member_ids.len(), 2);
        assert_eq!(clusters[0].source_count, 2);
        assert_eq!(clusters[0].lead_item_id, 1); // higher score leads
    }

    #[test]
    fn test_no_cluster_different_topics() {
        // Two items with orthogonal embeddings should stay separate
        let candidates = vec![
            make_candidate(1, 0.80, "hackernews", vec![1.0, 0.0, 0.0]),
            make_candidate(2, 0.70, "reddit", vec![0.0, 1.0, 0.0]),
        ];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 2, "orthogonal embeddings should form separate clusters");
        assert!(clusters.iter().all(|c| c.member_ids.len() == 1));
    }

    #[test]
    fn test_corroboration_bonus_three_sources() {
        // 3-source cluster gives +0.10 bonus (2 extra sources * 0.05)
        let emb = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            make_candidate(1, 0.80, "hackernews", emb.clone()),
            make_candidate(2, 0.70, "reddit", emb.clone()),
            make_candidate(3, 0.60, "rss", emb.clone()),
        ];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].source_count, 3);
        let expected_bonus = 0.10; // (3-1) * 0.05
        assert!(
            (clusters[0].corroboration_bonus - expected_bonus).abs() < f32::EPSILON,
            "3-source cluster should give +0.10 bonus, got {}",
            clusters[0].corroboration_bonus,
        );

        let bonuses = compute_corroboration_bonuses(&clusters);
        assert_eq!(bonuses.len(), 1);
        assert!(
            (bonuses[&1] - expected_bonus).abs() < f32::EPSILON,
            "lead item should get the bonus"
        );
    }

    #[test]
    fn test_single_source_no_bonus() {
        // Cluster with 1 source (same source type) = 0.0 bonus
        let emb = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            make_candidate(1, 0.80, "hackernews", emb.clone()),
            make_candidate(2, 0.70, "hackernews", emb.clone()),
        ];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].source_count, 1); // same source type
        assert!(
            clusters[0].corroboration_bonus.abs() < f32::EPSILON,
            "single source should give 0.0 bonus"
        );

        let bonuses = compute_corroboration_bonuses(&clusters);
        assert!(bonuses.is_empty(), "no bonus entries for single-source clusters");
    }

    #[test]
    fn test_mixed_signal_detection() {
        // "X improves performance" vs "X causes regression" should be flagged
        let emb = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            make_candidate_with_title(
                1,
                0.80,
                "hackernews",
                emb.clone(),
                "React 19 improves rendering performance",
            ),
            make_candidate_with_title(
                2,
                0.70,
                "reddit",
                emb.clone(),
                "React 19 causes regression in hydration",
            ),
        ];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 1);
        assert!(
            clusters[0].has_mixed_signals,
            "opposing sentiment should trigger mixed signal flag"
        );
    }

    #[test]
    fn test_low_score_items_not_clustered() {
        // Items below 0.20 should be excluded from clustering entirely
        let emb = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            make_candidate(1, 0.10, "hackernews", emb.clone()),
            make_candidate(2, 0.05, "reddit", emb.clone()),
        ];

        let clusters = cluster_items(&candidates);
        assert!(clusters.is_empty(), "items below score floor should not cluster");
    }

    #[test]
    fn test_empty_embedding_excluded() {
        let candidates = vec![
            make_candidate(1, 0.80, "hackernews", vec![]),
            make_candidate(2, 0.70, "reddit", vec![0.0, 0.0, 0.0]),
        ];

        let clusters = cluster_items(&candidates);
        assert!(clusters.is_empty(), "empty/zero embeddings should be excluded");
    }

    #[test]
    fn test_corroboration_bonus_capped() {
        // 5-source cluster should cap at +0.15 (not 4 * 0.05 = 0.20)
        let emb = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            make_candidate(1, 0.90, "hackernews", emb.clone()),
            make_candidate(2, 0.80, "reddit", emb.clone()),
            make_candidate(3, 0.70, "rss", emb.clone()),
            make_candidate(4, 0.60, "github", emb.clone()),
            make_candidate(5, 0.50, "devto", emb.clone()),
        ];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].source_count, 5);
        assert!(
            (clusters[0].corroboration_bonus - MAX_CORROBORATION_BONUS).abs() < f32::EPSILON,
            "bonus should be capped at {MAX_CORROBORATION_BONUS}, got {}",
            clusters[0].corroboration_bonus,
        );
    }

    #[test]
    fn test_no_mixed_signal_single_item() {
        let candidates = vec![make_candidate_with_title(
            1,
            0.80,
            "hackernews",
            vec![1.0, 0.0, 0.0],
            "React 19 improves and breaks things",
        )];

        let clusters = cluster_items(&candidates);
        assert_eq!(clusters.len(), 1);
        assert!(
            !clusters[0].has_mixed_signals,
            "single-item cluster should not flag mixed signals"
        );
    }

    #[test]
    fn test_empty_candidates() {
        let clusters = cluster_items(&[]);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_compute_corroboration_bonuses_empty() {
        let bonuses = compute_corroboration_bonuses(&[]);
        assert!(bonuses.is_empty());
    }
}
