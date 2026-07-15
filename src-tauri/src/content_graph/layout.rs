// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Two-phase, cluster-first layout (deterministic, no RNG).
//!
//! Phase 1 treats each cluster as a disc sized by member count: discs seed on
//! a circle (largest central), then a short force pass pulls discs with
//! inter-cluster edges together and separates overlapping discs.
//! Phase 2 places members inside their disc on a golden-angle (sunflower)
//! spiral — collision-free by construction — with high-degree members central.
//! Nodes outside every cluster sit on a peripheral orbit ring.
//!
//! Replaces single-phase Fruchterman-Reingold, which collapsed dense
//! near-clique clusters into an unreadable ball, left the canvas center
//! empty, and pinned sparse nodes to the frame border (the hard bounds clamp
//! rendered them as beads along the edge).

use std::collections::HashMap;

use super::types::{GraphCluster, GraphEdge, GraphNode};

/// Target spacing between neighboring member dots inside a cluster disc.
/// Sized for the readable label under each dot (~128px wide at zoom 1).
const MEMBER_SPACING: f32 = 95.0;
/// Minimum free gap between two cluster discs.
const CLUSTER_GAP: f32 = 130.0;
/// Extra clearance between the outermost cluster disc and the orbit ring.
const RING_MARGIN: f32 = 170.0;
/// Golden angle in radians — successive spiral points never align.
const GOLDEN_ANGLE: f32 = 2.399_963;
/// Phase-1 iterations; the cluster graph is tiny (rarely >15 discs).
const PHASE1_ITERATIONS: usize = 120;
/// Logical canvas center; the frontend fits the view, so overflow is fine.
const CENTER: (f32, f32) = (600.0, 500.0);

pub(super) fn compute_layout(
    nodes: &mut [GraphNode],
    edges: &[GraphEdge],
    clusters: &mut [GraphCluster],
) {
    if nodes.is_empty() {
        return;
    }

    let id_to_idx: HashMap<i64, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();

    // Cluster membership by node index; nodes outside every cluster orbit.
    let mut cluster_of: HashMap<usize, usize> = HashMap::new();
    for (ci, cluster) in clusters.iter().enumerate() {
        for id in &cluster.node_ids {
            if let Some(&idx) = id_to_idx.get(id) {
                cluster_of.insert(idx, ci);
            }
        }
    }

    let radii: Vec<f32> = clusters
        .iter()
        .map(|c| disc_radius(c.node_ids.len()))
        .collect();
    let centers = place_cluster_discs(clusters, &radii, edges, &id_to_idx, &cluster_of);

    let degree = node_degrees(nodes.len(), edges, &id_to_idx);

    // Phase 2: sunflower spiral inside each disc, hubs central.
    for (ci, cluster) in clusters.iter_mut().enumerate() {
        let mut member_idxs: Vec<usize> = cluster
            .node_ids
            .iter()
            .filter_map(|id| id_to_idx.get(id).copied())
            .collect();
        // Hubs (highest degree) get the innermost spiral slots. Tiebreak on
        // id for determinism.
        member_idxs.sort_by_key(|&idx| (std::cmp::Reverse(degree[idx]), nodes[idx].id));

        let (cx, cy) = centers[ci];
        let count = member_idxs.len();
        for (slot, &idx) in member_idxs.iter().enumerate() {
            let r = radii[ci] * ((slot as f32 + 0.5) / count as f32).sqrt();
            // Per-cluster phase offset so parallel clusters don't show the
            // same spiral arm orientation.
            let theta = slot as f32 * GOLDEN_ANGLE + ci as f32 * 0.7;
            nodes[idx].x = cx + r * theta.cos();
            nodes[idx].y = cy + r * theta.sin();
        }
        cluster.centroid_x = cx;
        cluster.centroid_y = cy;
    }

    // Orbit ring for everything unclustered, radius past the outermost disc.
    let ring_radius = clusters
        .iter()
        .enumerate()
        .map(|(ci, _)| {
            let (cx, cy) = centers[ci];
            ((cx - CENTER.0).powi(2) + (cy - CENTER.1).powi(2)).sqrt() + radii[ci]
        })
        .fold(0.0f32, f32::max)
        + RING_MARGIN;

    let mut orbit_idxs: Vec<usize> = (0..nodes.len())
        .filter(|idx| !cluster_of.contains_key(idx))
        .collect();
    if !orbit_idxs.is_empty() {
        // Group like sources adjacently on the ring, then by relevance.
        orbit_idxs.sort_by(|&a, &b| {
            nodes[a]
                .source_type
                .cmp(&nodes[b].source_type)
                .then(
                    nodes[b]
                        .relevance_score
                        .partial_cmp(&nodes[a].relevance_score)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(nodes[a].id.cmp(&nodes[b].id))
        });
        // A ring too small for its population overlaps labels; grow it so arc
        // spacing stays readable.
        let needed = orbit_idxs.len() as f32 * MEMBER_SPACING / (2.0 * std::f32::consts::PI);
        let radius = ring_radius.max(needed).max(220.0);
        let step = 2.0 * std::f32::consts::PI / orbit_idxs.len() as f32;
        for (slot, &idx) in orbit_idxs.iter().enumerate() {
            let theta = slot as f32 * step - std::f32::consts::FRAC_PI_2;
            nodes[idx].x = CENTER.0 + radius * theta.cos();
            nodes[idx].y = CENTER.1 + radius * theta.sin();
        }
    }
}

/// Disc radius that gives `n` sunflower points ~[`MEMBER_SPACING`] spacing.
fn disc_radius(n: usize) -> f32 {
    (MEMBER_SPACING * 0.62) * (n as f32).sqrt() + 30.0
}

fn node_degrees(n: usize, edges: &[GraphEdge], id_to_idx: &HashMap<i64, usize>) -> Vec<usize> {
    let mut degree = vec![0usize; n];
    for edge in edges {
        if let (Some(&a), Some(&b)) = (id_to_idx.get(&edge.source), id_to_idx.get(&edge.target)) {
            degree[a] += 1;
            degree[b] += 1;
        }
    }
    degree
}

/// Phase 1: place cluster discs. Seed on a circle (largest disc central),
/// then iterate attraction along aggregated inter-cluster edges + separation
/// of overlapping discs.
fn place_cluster_discs(
    clusters: &[GraphCluster],
    radii: &[f32],
    edges: &[GraphEdge],
    id_to_idx: &HashMap<i64, usize>,
    cluster_of: &HashMap<usize, usize>,
) -> Vec<(f32, f32)> {
    let k = clusters.len();
    if k == 0 {
        return Vec::new();
    }

    // Aggregate inter-cluster affinity: summed weight of edges whose
    // endpoints live in different clusters.
    let mut affinity: HashMap<(usize, usize), f32> = HashMap::new();
    for edge in edges {
        let (Some(&a), Some(&b)) = (id_to_idx.get(&edge.source), id_to_idx.get(&edge.target))
        else {
            continue;
        };
        let (Some(&ca), Some(&cb)) = (cluster_of.get(&a), cluster_of.get(&b)) else {
            continue;
        };
        if ca != cb {
            let key = if ca < cb { (ca, cb) } else { (cb, ca) };
            *affinity.entry(key).or_insert(0.0) += edge.weight;
        }
    }

    // Deterministic seed: size order, largest at the center, the rest on a
    // circle wide enough that no pair starts overlapping.
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| {
        radii[b]
            .partial_cmp(&radii[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let max_r = radii.iter().fold(0.0f32, |m, &r| m.max(r));
    let seed_radius = (2.0 * max_r + CLUSTER_GAP).max(300.0);
    let mut centers = vec![CENTER; k];
    for (slot, &ci) in order.iter().enumerate() {
        if slot == 0 {
            centers[ci] = CENTER;
        } else {
            let theta = (slot - 1) as f32 / (k - 1).max(1) as f32 * 2.0 * std::f32::consts::PI;
            centers[ci] = (
                CENTER.0 + seed_radius * theta.cos(),
                CENTER.1 + seed_radius * theta.sin(),
            );
        }
    }

    if k == 1 {
        return centers;
    }

    for _ in 0..PHASE1_ITERATIONS {
        let mut disp = vec![(0.0f32, 0.0f32); k];

        // Attraction: related discs drift toward touching distance.
        for (&(ca, cb), &w) in &affinity {
            let dx = centers[cb].0 - centers[ca].0;
            let dy = centers[cb].1 - centers[ca].1;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let ideal = radii[ca] + radii[cb] + CLUSTER_GAP;
            if dist > ideal {
                // Gentle, weight-scaled pull; capped so one heavy affinity
                // cannot slingshot a disc through another.
                let pull = ((dist - ideal) * 0.05 * w.min(4.0)).min(40.0);
                disp[ca].0 += dx / dist * pull;
                disp[ca].1 += dy / dist * pull;
                disp[cb].0 -= dx / dist * pull;
                disp[cb].1 -= dy / dist * pull;
            }
        }

        // Weak gravity keeps disconnected discs from drifting away.
        for ci in 0..k {
            disp[ci].0 += (CENTER.0 - centers[ci].0) * 0.01;
            disp[ci].1 += (CENTER.1 - centers[ci].1) * 0.01;
        }

        for ci in 0..k {
            centers[ci].0 += disp[ci].0;
            centers[ci].1 += disp[ci].1;
        }

        // Separation: resolve any disc overlap symmetrically.
        for a in 0..k {
            for b in (a + 1)..k {
                let dx = centers[b].0 - centers[a].0;
                let dy = centers[b].1 - centers[a].1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let min_dist = radii[a] + radii[b] + CLUSTER_GAP;
                if dist < min_dist {
                    let push = (min_dist - dist) / 2.0;
                    let (ux, uy) = if dist > 1.0 {
                        (dx / dist, dy / dist)
                    } else {
                        // Coincident centers: split along a deterministic axis.
                        (1.0, 0.0)
                    };
                    centers[a].0 -= ux * push;
                    centers[a].1 -= uy * push;
                    centers[b].0 += ux * push;
                    centers[b].1 += uy * push;
                }
            }
        }
    }

    centers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_graph::types::EdgeType;

    fn node(id: i64) -> GraphNode {
        GraphNode {
            id,
            title: format!("n{id}"),
            url: None,
            source_type: "hn".to_string(),
            relevance_score: 0.5,
            signal_type: None,
            signal_priority: None,
            created_at: String::new(),
            primary_topic: None,
            cluster_id: None,
            member_count: 1,
            member_titles: Vec::new(),
            member_ids: vec![id],
            x: 0.0,
            y: 0.0,
        }
    }

    fn cluster(id: &str, node_ids: Vec<i64>) -> GraphCluster {
        GraphCluster {
            id: id.to_string(),
            label: String::new(),
            node_ids,
            source_count: 1,
            centroid_x: 0.0,
            centroid_y: 0.0,
        }
    }

    fn edge(source: i64, target: i64, weight: f32) -> GraphEdge {
        GraphEdge {
            source,
            target,
            edge_type: EdgeType::Semantic,
            weight,
            label: None,
            methods: vec![],
        }
    }

    #[test]
    fn members_stay_inside_their_disc() {
        let mut nodes: Vec<GraphNode> = (1..=12).map(node).collect();
        let mut clusters = vec![cluster("a", (1..=12).collect())];
        let edges: Vec<GraphEdge> = (2..=12).map(|i| edge(1, i, 0.8)).collect();

        compute_layout(&mut nodes, &edges, &mut clusters);

        let r = disc_radius(12);
        let (cx, cy) = (clusters[0].centroid_x, clusters[0].centroid_y);
        for n in &nodes {
            let d = ((n.x - cx).powi(2) + (n.y - cy).powi(2)).sqrt();
            assert!(
                d <= r + 1.0,
                "member {} at distance {d} outside disc {r}",
                n.id
            );
        }
    }

    #[test]
    fn cluster_discs_never_overlap() {
        // Three clusters chained by affinity — attraction must not merge them.
        let mut nodes: Vec<GraphNode> = (1..=30).map(node).collect();
        let mut clusters = vec![
            cluster("a", (1..=10).collect()),
            cluster("b", (11..=20).collect()),
            cluster("c", (21..=30).collect()),
        ];
        let edges = vec![edge(1, 11, 0.9), edge(11, 21, 0.9), edge(1, 21, 0.9)];

        compute_layout(&mut nodes, &edges, &mut clusters);

        for a in 0..clusters.len() {
            for b in (a + 1)..clusters.len() {
                let dx = clusters[a].centroid_x - clusters[b].centroid_x;
                let dy = clusters[a].centroid_y - clusters[b].centroid_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let min = disc_radius(10) * 2.0; // gap consumed is fine; discs must not merge
                assert!(dist >= min, "discs {a},{b} at {dist} (min {min})");
            }
        }
    }

    #[test]
    fn unclustered_nodes_sit_on_orbit_ring_beyond_discs() {
        let mut nodes: Vec<GraphNode> = (1..=8).map(node).collect();
        let mut clusters = vec![cluster("a", vec![1, 2, 3, 4])];
        let edges = vec![edge(1, 2, 0.8), edge(3, 4, 0.8)];

        compute_layout(&mut nodes, &edges, &mut clusters);

        let (cx, cy) = (CENTER.0, CENTER.1);
        let disc_edge = {
            let dx = clusters[0].centroid_x - cx;
            let dy = clusters[0].centroid_y - cy;
            (dx * dx + dy * dy).sqrt() + disc_radius(4)
        };
        for n in nodes.iter().filter(|n| n.id > 4) {
            let d = ((n.x - cx).powi(2) + (n.y - cy).powi(2)).sqrt();
            assert!(
                d > disc_edge,
                "orbit node {} at {d} not beyond disc edge {disc_edge}",
                n.id
            );
        }
    }

    #[test]
    fn layout_is_deterministic() {
        let build = || {
            let mut nodes: Vec<GraphNode> = (1..=15).map(node).collect();
            let mut clusters = vec![
                cluster("a", (1..=6).collect()),
                cluster("b", (7..=12).collect()),
            ];
            let edges = vec![edge(1, 7, 0.9), edge(2, 8, 0.8)];
            compute_layout(&mut nodes, &edges, &mut clusters);
            nodes.iter().map(|n| (n.x, n.y)).collect::<Vec<_>>()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn all_positions_finite() {
        let mut nodes: Vec<GraphNode> = (1..=40).map(node).collect();
        let mut clusters = vec![
            cluster("a", (1..=26).collect()),
            cluster("b", (27..=28).collect()),
        ];
        let mut edges: Vec<GraphEdge> = Vec::new();
        for i in 1..=26 {
            for j in (i + 1)..=26 {
                edges.push(edge(i, j, 0.9));
            }
        }
        edges.push(edge(27, 28, 0.8));

        compute_layout(&mut nodes, &edges, &mut clusters);
        for n in &nodes {
            assert!(
                n.x.is_finite() && n.y.is_finite(),
                "node {} not finite",
                n.id
            );
        }
    }

    #[test]
    fn empty_graph_is_a_no_op() {
        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut clusters: Vec<GraphCluster> = Vec::new();
        compute_layout(&mut nodes, &[], &mut clusters);
    }
}
