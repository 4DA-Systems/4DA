// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Content Graph — relationship visualization for surfaced intelligence.
//!
//! Assembles edges from four existing relationship systems:
//! - topic_clustering (cosine ≥ 0.77 semantic similarity)
//! - signal_chains (temporal causal links across days)
//! - concept_graph (topic co-occurrence)
//! - dedup (Jaccard ≥ 0.65 near-duplicates)
//!
//! Computes a deterministic force-directed layout in Rust and returns
//! positioned nodes + edges for the frontend to render without JS layout.

use std::collections::{HashMap, HashSet};

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use ts_rs::TS;

use crate::db::blob_to_embedding;
use crate::error::Result;
use crate::signal_chains::detect_chains;
use crate::utils::cosine_similarity;

const DEFAULT_DAYS: u32 = 7;
const DEFAULT_MAX_NODES: usize = 150;
const SIMILARITY_THRESHOLD: f32 = 0.77;
const LEXICAL_FALLBACK_THRESHOLD: f32 = 0.73;
const LEXICAL_OVERLAP_MIN: f32 = 0.60;
const MIN_EDGES_TO_APPEAR: usize = 1;
const LAYOUT_WIDTH: f32 = 1000.0;
const LAYOUT_HEIGHT: f32 = 800.0;
const LAYOUT_ITERATIONS: usize = 80;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ContentGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub clusters: Vec<GraphCluster>,
    pub meta: GraphMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphNode {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub relevance_score: f32,
    pub signal_type: Option<String>,
    pub signal_priority: Option<String>,
    pub created_at: String,
    pub primary_topic: Option<String>,
    pub cluster_id: Option<String>,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    pub edge_type: EdgeType,
    pub weight: f32,
    pub label: Option<String>,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Semantic,
    Chain,
    Concept,
    Convergence,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphCluster {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<i64>,
    pub source_count: usize,
    pub centroid_x: f32,
    pub centroid_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphMeta {
    pub total_items: usize,
    pub total_edges: usize,
    pub cluster_count: usize,
    pub time_window_days: u32,
    pub edge_threshold: String,
}

// ============================================================================
// Graph Construction
// ============================================================================

struct RawItem {
    id: i64,
    title: String,
    url: Option<String>,
    source_type: String,
    relevance_score: f32,
    created_at: String,
    embedding: Vec<f32>,
}

pub fn build_graph(
    conn: &rusqlite::Connection,
    days: u32,
    max_nodes: usize,
) -> Result<ContentGraph> {
    let items = load_scored_items(conn, days, max_nodes)?;
    if items.is_empty() {
        return Ok(ContentGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            meta: GraphMeta {
                total_items: 0,
                total_edges: 0,
                cluster_count: 0,
                time_window_days: days,
                edge_threshold: format!("cosine >= {SIMILARITY_THRESHOLD}"),
            },
        });
    }

    let mut edges = Vec::new();
    let id_set: HashSet<i64> = items.iter().map(|i| i.id).collect();

    compute_semantic_edges(&items, &mut edges);
    compute_chain_edges(conn, &id_set, &mut edges);
    merge_duplicate_edges(&mut edges);

    let mut clusters = compute_clusters(&items, &edges);
    assign_cluster_labels(&items, &mut clusters);

    let edge_count_per_node = count_edges_per_node(&edges);
    let mut nodes: Vec<GraphNode> = items
        .iter()
        .filter(|item| {
            edge_count_per_node.get(&item.id).copied().unwrap_or(0) >= MIN_EDGES_TO_APPEAR
        })
        .map(|item| {
            let cluster_id = clusters
                .iter()
                .find(|c| c.node_ids.contains(&item.id))
                .map(|c| c.id.clone());
            GraphNode {
                id: item.id,
                title: item.title.clone(),
                url: item.url.clone(),
                source_type: item.source_type.clone(),
                relevance_score: item.relevance_score,
                signal_type: None,
                signal_priority: None,
                created_at: item.created_at.clone(),
                primary_topic: None,
                cluster_id,
                x: 0.0,
                y: 0.0,
            }
        })
        .collect();

    let visible_ids: HashSet<i64> = nodes.iter().map(|n| n.id).collect();
    edges.retain(|e| visible_ids.contains(&e.source) && visible_ids.contains(&e.target));
    let clusters: Vec<GraphCluster> = clusters
        .into_iter()
        .map(|mut c| {
            c.node_ids.retain(|id| visible_ids.contains(id));
            c
        })
        .filter(|c| c.node_ids.len() >= 2)
        .collect();

    let mut clusters = clusters;
    compute_layout(&mut nodes, &edges, &mut clusters);

    let meta = GraphMeta {
        total_items: nodes.len(),
        total_edges: edges.len(),
        cluster_count: clusters.len(),
        time_window_days: days,
        edge_threshold: format!("cosine >= {SIMILARITY_THRESHOLD}"),
    };

    info!(
        target: "4da::content_graph",
        nodes = nodes.len(),
        edges = edges.len(),
        clusters = clusters.len(),
        "Content graph built"
    );

    Ok(ContentGraph {
        nodes,
        edges,
        clusters,
        meta,
    })
}

// ============================================================================
// Data Loading
// ============================================================================

fn load_scored_items(
    conn: &rusqlite::Connection,
    days: u32,
    max_nodes: usize,
) -> Result<Vec<RawItem>> {
    let mut stmt = conn.prepare(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score,
                si.created_at, si.embedding
         FROM source_items si
         WHERE si.relevance_score IS NOT NULL
           AND si.created_at >= datetime('now', ?1)
           AND si.embedding_status = 'complete'
         ORDER BY si.relevance_score DESC
         LIMIT ?2",
    )?;

    let days_param = format!("-{days} days");
    let rows = stmt.query_map(params![days_param, max_nodes as i64], |row| {
        let embedding_blob: Vec<u8> = row.get(6)?;
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, f64>(4)? as f32,
            row.get::<_, String>(5)?,
            embedding_blob,
        ))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, title, url, source_type, score, created_at, embedding_blob) = row?;
        let embedding = blob_to_embedding(&embedding_blob);
        if embedding.is_empty() || embedding.iter().all(|&v| v == 0.0) {
            continue;
        }
        items.push(RawItem {
            id,
            title,
            url,
            source_type,
            relevance_score: score,
            created_at,
            embedding,
        });
    }

    debug!(
        target: "4da::content_graph",
        loaded = items.len(),
        days,
        "Loaded scored items for graph"
    );

    Ok(items)
}

// ============================================================================
// Edge Computation
// ============================================================================

fn compute_semantic_edges(items: &[RawItem], edges: &mut Vec<GraphEdge>) {
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            let sim = cosine_similarity(&items[i].embedding, &items[j].embedding);
            let should_connect = sim >= SIMILARITY_THRESHOLD
                || (sim >= LEXICAL_FALLBACK_THRESHOLD
                    && title_word_overlap(&items[i].title, &items[j].title) >= LEXICAL_OVERLAP_MIN);

            if should_connect {
                edges.push(GraphEdge {
                    source: items[i].id,
                    target: items[j].id,
                    edge_type: EdgeType::Semantic,
                    weight: sim.clamp(0.0, 1.0),
                    label: Some(format!("similarity: {:.2}", sim)),
                    methods: vec!["semantic".to_string()],
                });
            }
        }
    }
}

fn compute_chain_edges(
    conn: &rusqlite::Connection,
    id_set: &HashSet<i64>,
    edges: &mut Vec<GraphEdge>,
) {
    let chains = match detect_chains(conn) {
        Ok(c) => c,
        Err(e) => {
            debug!(target: "4da::content_graph", error = %e, "Signal chain detection failed, skipping chain edges");
            return;
        }
    };

    for chain in &chains {
        let chain_item_ids: Vec<i64> = chain
            .links
            .iter()
            .map(|l| l.source_item_id)
            .filter(|id| id_set.contains(id))
            .collect();

        for window in chain_item_ids.windows(2) {
            edges.push(GraphEdge {
                source: window[0],
                target: window[1],
                edge_type: EdgeType::Chain,
                weight: (chain.confidence as f32).clamp(0.0, 1.0),
                label: Some(chain.chain_name.clone()),
                methods: vec!["signal_chain".to_string()],
            });
        }
    }
}

fn merge_duplicate_edges(edges: &mut Vec<GraphEdge>) {
    let mut merged: HashMap<(i64, i64), GraphEdge> = HashMap::new();

    for edge in edges.drain(..) {
        let key = if edge.source <= edge.target {
            (edge.source, edge.target)
        } else {
            (edge.target, edge.source)
        };

        merged
            .entry(key)
            .and_modify(|existing| {
                if edge.weight > existing.weight {
                    existing.weight = edge.weight;
                    existing.label = edge.label.clone();
                }
                for method in &edge.methods {
                    if !existing.methods.contains(method) {
                        existing.methods.push(method.clone());
                    }
                }
                if existing.edge_type != edge.edge_type {
                    existing.edge_type = EdgeType::Convergence;
                }
            })
            .or_insert(GraphEdge {
                source: key.0,
                target: key.1,
                ..edge
            });
    }

    *edges = merged.into_values().collect();
}

fn count_edges_per_node(edges: &[GraphEdge]) -> HashMap<i64, usize> {
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for edge in edges {
        *counts.entry(edge.source).or_insert(0) += 1;
        *counts.entry(edge.target).or_insert(0) += 1;
    }
    counts
}

// ============================================================================
// Clustering
// ============================================================================

fn compute_clusters(items: &[RawItem], edges: &[GraphEdge]) -> Vec<GraphCluster> {
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    for edge in edges {
        if edge.weight >= SIMILARITY_THRESHOLD * 0.9 {
            adjacency.entry(edge.source).or_default().push(edge.target);
            adjacency.entry(edge.target).or_default().push(edge.source);
        }
    }

    let mut visited: HashSet<i64> = HashSet::new();
    let mut clusters = Vec::new();
    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();

    for item in items {
        if visited.contains(&item.id) {
            continue;
        }

        let mut component = Vec::new();
        let mut stack = vec![item.id];

        while let Some(node_id) = stack.pop() {
            if !visited.insert(node_id) {
                continue;
            }
            component.push(node_id);
            if let Some(neighbors) = adjacency.get(&node_id) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }

        if component.len() < 2 {
            continue;
        }

        let sources: HashSet<&str> = component
            .iter()
            .filter_map(|id| item_map.get(id))
            .map(|i| i.source_type.as_str())
            .collect();

        let cluster_id = format!("cluster_{}", component.iter().min().unwrap_or(&0));

        clusters.push(GraphCluster {
            id: cluster_id,
            label: String::new(),
            node_ids: component,
            source_count: sources.len(),
            centroid_x: 0.0,
            centroid_y: 0.0,
        });
    }

    clusters
}

fn assign_cluster_labels(items: &[RawItem], clusters: &mut [GraphCluster]) {
    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();

    for cluster in clusters.iter_mut() {
        let mut word_freq: HashMap<String, usize> = HashMap::new();
        for &id in &cluster.node_ids {
            if let Some(item) = item_map.get(&id) {
                for word in extract_title_keywords(&item.title) {
                    *word_freq.entry(word).or_insert(0) += 1;
                }
            }
        }

        let mut words: Vec<(String, usize)> = word_freq.into_iter().collect();
        words.sort_by(|a, b| b.1.cmp(&a.1));
        cluster.label = words
            .iter()
            .take(3)
            .map(|(w, _)| w.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
    }
}

fn extract_title_keywords(title: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "in", "of", "for", "to", "and", "is", "new", "on", "at", "by", "with",
        "from", "this", "that", "it", "its", "has", "have", "are", "was", "were", "been", "be",
        "do", "does", "did", "will", "would", "could", "should", "may", "can", "not", "no", "but",
        "or", "if", "how", "what", "when", "where", "who", "why", "which", "all", "each", "every",
        "both", "more", "most", "other", "some", "such", "than", "too", "very", "just", "about",
        "up", "out", "so", "show", "hn", "ask",
    ];

    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(String::from)
        .collect()
}

// ============================================================================
// Layout (Fruchterman-Reingold force-directed, deterministic)
// ============================================================================

fn compute_layout(nodes: &mut [GraphNode], edges: &[GraphEdge], clusters: &mut [GraphCluster]) {
    if nodes.is_empty() {
        return;
    }

    let n = nodes.len();
    let area = LAYOUT_WIDTH * LAYOUT_HEIGHT;
    let k = (area / n as f32).sqrt();
    let mut positions: Vec<(f32, f32)> = Vec::with_capacity(n);

    // Deterministic initial placement: grid with slight jitter from id hash
    let cols = (n as f32).sqrt().ceil() as usize;
    for (i, node) in nodes.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        let jitter_x = ((node.id * 7919) % 100) as f32 / 100.0 * 20.0 - 10.0;
        let jitter_y = ((node.id * 6271) % 100) as f32 / 100.0 * 20.0 - 10.0;
        let x = 50.0 + (col as f32 / cols as f32) * (LAYOUT_WIDTH - 100.0) + jitter_x;
        let y = 50.0 + (row as f32 / cols as f32) * (LAYOUT_HEIGHT - 100.0) + jitter_y;
        positions.push((x, y));
    }

    let id_to_idx: HashMap<i64, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();

    let mut temperature = LAYOUT_WIDTH / 4.0;
    let cooling = temperature / LAYOUT_ITERATIONS as f32;

    for _ in 0..LAYOUT_ITERATIONS {
        let mut displacements = vec![(0.0f32, 0.0f32); n];

        // Repulsive forces between all pairs
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = positions[i].0 - positions[j].0;
                let dy = positions[i].1 - positions[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let force = k * k / dist;
                let fx = dx / dist * force;
                let fy = dy / dist * force;
                displacements[i].0 += fx;
                displacements[i].1 += fy;
                displacements[j].0 -= fx;
                displacements[j].1 -= fy;
            }
        }

        // Attractive forces along edges
        for edge in edges {
            if let (Some(&i), Some(&j)) = (id_to_idx.get(&edge.source), id_to_idx.get(&edge.target))
            {
                let dx = positions[i].0 - positions[j].0;
                let dy = positions[i].1 - positions[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let force = dist * dist / k * edge.weight;
                let fx = dx / dist * force;
                let fy = dy / dist * force;
                displacements[i].0 -= fx;
                displacements[i].1 -= fy;
                displacements[j].0 += fx;
                displacements[j].1 += fy;
            }
        }

        // Apply displacements clamped by temperature
        for i in 0..n {
            let (dx, dy) = displacements[i];
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let clamped = dist.min(temperature);
            positions[i].0 += dx / dist * clamped;
            positions[i].1 += dy / dist * clamped;
            positions[i].0 = positions[i].0.clamp(20.0, LAYOUT_WIDTH - 20.0);
            positions[i].1 = positions[i].1.clamp(20.0, LAYOUT_HEIGHT - 20.0);
        }

        temperature -= cooling;
        if temperature < 1.0 {
            break;
        }
    }

    for (i, node) in nodes.iter_mut().enumerate() {
        node.x = positions[i].0;
        node.y = positions[i].1;
    }

    for cluster in clusters.iter_mut() {
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut count = 0;
        for &id in &cluster.node_ids {
            if let Some(&idx) = id_to_idx.get(&id) {
                cx += positions[idx].0;
                cy += positions[idx].1;
                count += 1;
            }
        }
        if count > 0 {
            cluster.centroid_x = cx / count as f32;
            cluster.centroid_y = cy / count as f32;
        }
    }
}

fn title_word_overlap(a: &str, b: &str) -> f32 {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "in", "of", "for", "to", "and", "is", "new",
    ];

    let set_a: HashSet<String> = a
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOPWORDS.contains(w))
        .map(String::from)
        .collect();
    let set_b: HashSet<String> = b
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOPWORDS.contains(w))
        .map(String::from)
        .collect();

    if set_a.is_empty() && set_b.is_empty() {
        return 0.0;
    }

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub fn build_content_graph(days: Option<u32>, max_nodes: Option<usize>) -> Result<ContentGraph> {
    let conn = crate::open_db_connection()?;
    let d = days.unwrap_or(DEFAULT_DAYS);
    let m = max_nodes.unwrap_or(DEFAULT_MAX_NODES);
    build_graph(&conn, d, m)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = ContentGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            meta: GraphMeta {
                total_items: 0,
                total_edges: 0,
                cluster_count: 0,
                time_window_days: 7,
                edge_threshold: "cosine >= 0.77".to_string(),
            },
        };
        assert_eq!(graph.nodes.len(), 0);
        assert_eq!(graph.edges.len(), 0);
    }

    #[test]
    fn test_semantic_edge_above_threshold() {
        let items = vec![
            RawItem {
                id: 1,
                title: "Rust async runtime".to_string(),
                url: None,
                source_type: "hackernews".to_string(),
                relevance_score: 0.8,
                created_at: "2026-05-24".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
            RawItem {
                id: 2,
                title: "Rust async runtime update".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                created_at: "2026-05-24".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
        ];

        let mut edges = Vec::new();
        compute_semantic_edges(&items, &mut edges);

        assert_eq!(edges.len(), 1, "identical embeddings should create an edge");
        assert_eq!(edges[0].edge_type, EdgeType::Semantic);
        assert!((edges[0].weight - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_no_edge_below_threshold() {
        let items = vec![
            RawItem {
                id: 1,
                title: "Rust async".to_string(),
                url: None,
                source_type: "hackernews".to_string(),
                relevance_score: 0.8,
                created_at: "2026-05-24".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
            RawItem {
                id: 2,
                title: "Python web framework".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                created_at: "2026-05-24".to_string(),
                embedding: vec![0.0, 1.0, 0.0],
            },
        ];

        let mut edges = Vec::new();
        compute_semantic_edges(&items, &mut edges);

        assert!(
            edges.is_empty(),
            "orthogonal embeddings should create no edge"
        );
    }

    #[test]
    fn test_merge_duplicate_edges() {
        let mut edges = vec![
            GraphEdge {
                source: 1,
                target: 2,
                edge_type: EdgeType::Semantic,
                weight: 0.85,
                label: Some("similarity: 0.85".to_string()),
                methods: vec!["semantic".to_string()],
            },
            GraphEdge {
                source: 1,
                target: 2,
                edge_type: EdgeType::Chain,
                weight: 0.70,
                label: Some("chain: tokio".to_string()),
                methods: vec!["signal_chain".to_string()],
            },
        ];

        merge_duplicate_edges(&mut edges);

        assert_eq!(edges.len(), 1, "duplicate edges should merge");
        assert_eq!(edges[0].edge_type, EdgeType::Convergence);
        assert_eq!(edges[0].methods.len(), 2);
        assert!((edges[0].weight - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cluster_formation() {
        let items = vec![
            RawItem {
                id: 1,
                title: "A".to_string(),
                url: None,
                source_type: "hn".to_string(),
                relevance_score: 0.8,
                created_at: "".to_string(),
                embedding: vec![1.0, 0.0],
            },
            RawItem {
                id: 2,
                title: "B".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                created_at: "".to_string(),
                embedding: vec![1.0, 0.0],
            },
            RawItem {
                id: 3,
                title: "C".to_string(),
                url: None,
                source_type: "github".to_string(),
                relevance_score: 0.6,
                created_at: "".to_string(),
                embedding: vec![0.0, 1.0],
            },
        ];

        let edges = vec![GraphEdge {
            source: 1,
            target: 2,
            edge_type: EdgeType::Semantic,
            weight: 0.9,
            label: None,
            methods: vec!["semantic".to_string()],
        }];

        let clusters = compute_clusters(&items, &edges);
        assert_eq!(clusters.len(), 1, "connected items should form one cluster");
        assert_eq!(clusters[0].node_ids.len(), 2);
        assert_eq!(clusters[0].source_count, 2);
    }

    #[test]
    fn test_layout_positions_in_bounds() {
        let mut nodes = vec![
            GraphNode {
                id: 1,
                title: "A".to_string(),
                url: None,
                source_type: "hn".to_string(),
                relevance_score: 0.8,
                signal_type: None,
                signal_priority: None,
                created_at: "".to_string(),
                primary_topic: None,
                cluster_id: None,
                x: 0.0,
                y: 0.0,
            },
            GraphNode {
                id: 2,
                title: "B".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                signal_type: None,
                signal_priority: None,
                created_at: "".to_string(),
                primary_topic: None,
                cluster_id: None,
                x: 0.0,
                y: 0.0,
            },
        ];

        let edges = vec![GraphEdge {
            source: 1,
            target: 2,
            edge_type: EdgeType::Semantic,
            weight: 0.9,
            label: None,
            methods: vec![],
        }];

        compute_layout(&mut nodes, &edges, &mut []);

        for node in &nodes {
            assert!(
                node.x >= 0.0 && node.x <= LAYOUT_WIDTH,
                "x out of bounds: {}",
                node.x
            );
            assert!(
                node.y >= 0.0 && node.y <= LAYOUT_HEIGHT,
                "y out of bounds: {}",
                node.y
            );
        }
    }

    #[test]
    fn test_title_word_overlap_high() {
        let overlap = title_word_overlap(
            "React 19 server components released",
            "React 19 server components update",
        );
        assert!(
            overlap > LEXICAL_OVERLAP_MIN,
            "similar titles should overlap >{LEXICAL_OVERLAP_MIN}, got {overlap}"
        );
    }

    #[test]
    fn test_title_word_overlap_low() {
        let overlap = title_word_overlap("Rust async runtime", "Python web framework");
        assert!(overlap < LEXICAL_OVERLAP_MIN);
    }

    #[test]
    fn test_extract_title_keywords() {
        let keywords = extract_title_keywords("Show HN: A New Rust Web Framework");
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"web".to_string()));
        assert!(keywords.contains(&"framework".to_string()));
        assert!(!keywords.contains(&"a".to_string()));
        assert!(!keywords.contains(&"hn".to_string()));
    }

    #[test]
    fn test_edge_count_per_node() {
        let edges = vec![
            GraphEdge {
                source: 1,
                target: 2,
                edge_type: EdgeType::Semantic,
                weight: 0.9,
                label: None,
                methods: vec![],
            },
            GraphEdge {
                source: 1,
                target: 3,
                edge_type: EdgeType::Chain,
                weight: 0.8,
                label: None,
                methods: vec![],
            },
        ];

        let counts = count_edges_per_node(&edges);
        assert_eq!(counts[&1], 2);
        assert_eq!(counts[&2], 1);
        assert_eq!(counts[&3], 1);
    }
}
