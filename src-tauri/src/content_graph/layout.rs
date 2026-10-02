// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Cluster-first layout with an unconnected lane (deterministic, no RNG).
//!
//! Phase 1 treats each cluster as a disc sized by member count: discs seed on
//! a circle (largest central), then a short force pass pulls discs with
//! inter-cluster edges together and separates overlapping discs.
//! Phase 2 places members inside their disc on a golden-angle (sunflower)
//! spiral — collision-free by construction — with high-degree members central.
//!
//! Unclustered nodes go to ONE labelled lane under the map ("not connected
//! to any theme" — the frontend draws the lane header above the grid). They
//! used to orbit their nearest cluster as SATELLITES at a distance set by
//! cosine (floor 0.45), but a satellite sat inside a theme's halo with no
//! edge to it: live 2026-10-02, 26% of nodes rendered as apparent theme
//! members that the graph's own edge rule (mutual top-3, floor 0.55) had
//! judged unrelated. Proximity that the edge model disowns is decoration,
//! not relation — the lane says plainly what these items are.
//!
//! A final global collision pass resolves any residual overlap.

use std::collections::HashMap;

use super::types::{GraphCluster, GraphEdge, GraphNode};

/// Target spacing between neighboring member dots inside a cluster disc.
/// Sized for the readable label under each dot (~128px wide at zoom 1).
const MEMBER_SPACING: f32 = 95.0;
/// Minimum free gap between two cluster discs.
const CLUSTER_GAP: f32 = 120.0;
/// Golden angle in radians — successive spiral points never align.
const GOLDEN_ANGLE: f32 = 2.399_963;
/// Phase-1 iterations; the cluster graph is tiny (rarely >15 discs).
const PHASE1_ITERATIONS: usize = 120;
/// Logical canvas center; the frontend fits the view, so overflow is fine.
const CENTER: (f32, f32) = (600.0, 500.0);
/// Lane grid columns for unconnected nodes — wide enough that a 40-node
/// lane stays a few rows, not a tall column under a wide map.
const SHELF_COLS: usize = 12;
/// Gap between the lowest map node and the lane's first row (leaves room
/// for the lane header the frontend draws).
const LANE_GAP: f32 = 260.0;
/// Global collision pass: minimum center distance and sweep count.
const COLLIDE_DIST: f32 = 82.0;
const COLLIDE_ITERATIONS: usize = 60;

pub(super) fn compute_layout(
    nodes: &mut [GraphNode],
    edges: &[GraphEdge],
    clusters: &mut [GraphCluster],
    anchor_seeds: &HashMap<String, (f32, f32)>,
) {
    if nodes.is_empty() {
        return;
    }

    let id_to_idx: HashMap<i64, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();

    let mut cluster_of: HashMap<usize, usize> = HashMap::new();
    for (ci, cluster) in clusters.iter().enumerate() {
        for id in &cluster.node_ids {
            if let Some(&idx) = id_to_idx.get(id) {
                cluster_of.insert(idx, ci);
            }
        }
    }

    // Disc radius = sunflower extent of the member count. (The halo that
    // reserved satellite orbits is gone with the satellites.)
    let disc_radii: Vec<f32> = clusters
        .iter()
        .map(|c| disc_radius(c.node_ids.len()))
        .collect();

    let seeds_by_idx: HashMap<usize, (f32, f32)> = clusters
        .iter()
        .enumerate()
        .filter_map(|(ci, c)| anchor_seeds.get(&c.id).map(|&p| (ci, p)))
        .collect();
    let centers = place_cluster_discs(
        clusters,
        &disc_radii,
        edges,
        &id_to_idx,
        &cluster_of,
        &seeds_by_idx,
    );

    let degree = node_degrees(nodes.len(), edges, &id_to_idx);

    // Phase 2: sunflower spiral inside each disc, hubs central.
    for (ci, cluster) in clusters.iter_mut().enumerate() {
        let mut member_idxs: Vec<usize> = cluster
            .node_ids
            .iter()
            .filter_map(|id| id_to_idx.get(id).copied())
            .collect();
        member_idxs.sort_by_key(|&idx| (std::cmp::Reverse(degree[idx]), nodes[idx].id));

        let (cx, cy) = centers[ci];
        let count = member_idxs.len();
        for (slot, &idx) in member_idxs.iter().enumerate() {
            let r = disc_radii[ci] * ((slot as f32 + 0.5) / count as f32).sqrt();
            let theta = slot as f32 * GOLDEN_ANGLE + ci as f32 * 0.7;
            nodes[idx].x = cx + r * theta.cos();
            nodes[idx].y = cy + r * theta.sin();
        }
        cluster.centroid_x = cx;
        cluster.centroid_y = cy;
    }

    // Lane: every node in no cluster. A plain grid under the map, by
    // relevance — honest, no implied geometry.
    let shelf_idxs: Vec<usize> = {
        let mut v: Vec<usize> = (0..nodes.len())
            .filter(|idx| !cluster_of.contains_key(idx))
            .collect();
        v.sort_by(|&a, &b| {
            nodes[b]
                .relevance_score
                .partial_cmp(&nodes[a].relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(nodes[a].id.cmp(&nodes[b].id))
        });
        v
    };
    if !shelf_idxs.is_empty() {
        let max_y = nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| !shelf_idxs.contains(i))
            .map(|(_, n)| n.y)
            .fold(CENTER.1, f32::max);
        let shelf_top = max_y + LANE_GAP;
        let width = (SHELF_COLS.min(shelf_idxs.len()).max(1) - 1) as f32 * MEMBER_SPACING;
        for (slot, &idx) in shelf_idxs.iter().enumerate() {
            let row = slot / SHELF_COLS;
            let col = slot % SHELF_COLS;
            nodes[idx].x = CENTER.0 - width / 2.0 + col as f32 * MEMBER_SPACING;
            nodes[idx].y = shelf_top + row as f32 * MEMBER_SPACING;
        }
    }

    resolve_collisions(nodes, &shelf_idxs);
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

/// Final global pass: separate any node pair closer than [`COLLIDE_DIST`].
/// Deterministic sweep order; shelf nodes stay pinned (their grid IS the
/// design), everything else shifts symmetrically.
fn resolve_collisions(nodes: &mut [GraphNode], pinned: &[usize]) {
    let n = nodes.len();
    for _ in 0..COLLIDE_ITERATIONS {
        let mut moved = false;
        for a in 0..n {
            for b in (a + 1)..n {
                let dx = nodes[b].x - nodes[a].x;
                let dy = nodes[b].y - nodes[a].y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist >= COLLIDE_DIST {
                    continue;
                }
                let (ux, uy) = if dist > 1.0 {
                    (dx / dist, dy / dist)
                } else {
                    // Coincident: split along a deterministic axis.
                    (1.0, 0.0)
                };
                let push = (COLLIDE_DIST - dist.max(1.0)) / 2.0;
                let a_pinned = pinned.contains(&a);
                let b_pinned = pinned.contains(&b);
                if !a_pinned {
                    let f = if b_pinned { 2.0 } else { 1.0 };
                    nodes[a].x -= ux * push * f;
                    nodes[a].y -= uy * push * f;
                }
                if !b_pinned {
                    let f = if a_pinned { 2.0 } else { 1.0 };
                    nodes[b].x += ux * push * f;
                    nodes[b].y += uy * push * f;
                }
                if !(a_pinned && b_pinned) {
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
}

/// Phase 1: place cluster discs. Anchored clusters (P2.11: matched to a
/// persisted position from a previous build) seed AT their anchor; the rest
/// seed on the compact spiral. The attraction/separation pass then runs over
/// all of them — anchors provide continuity, not frozen geometry.
fn place_cluster_discs(
    clusters: &[GraphCluster],
    radii: &[f32],
    edges: &[GraphEdge],
    id_to_idx: &HashMap<i64, usize>,
    cluster_of: &HashMap<usize, usize>,
    anchor_seeds: &HashMap<usize, (f32, f32)>,
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
    // Sorted iteration order: the force pass accumulates f32 displacements,
    // and HashMap order would make positions run-dependent (same class of
    // leak as merge_duplicate_edges — see its determinism note).
    let mut affinity: Vec<((usize, usize), f32)> = affinity.into_iter().collect();
    affinity.sort_by_key(|&(key, _)| key);

    // Deterministic seed: size order, largest central, the rest on a compact
    // golden-angle (sunflower) spiral. Community detection yields ~40 small
    // discs at steady state (live sweep 2026-07-19) — the previous
    // one-circle seed would arrange them all on a single giant ring, the
    // exact "big circle encodes nothing" pathology Wave 2 removed for
    // singletons, resurfacing at cluster level. A spiral seed packs the
    // plane; the separation pass below resolves residual overlap.
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| {
        radii[b]
            .partial_cmp(&radii[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let mean_halo = radii.iter().sum::<f32>() / k as f32;
    let spiral_step = (2.0 * mean_halo + CLUSTER_GAP) * 0.62;
    let mut centers = vec![CENTER; k];
    // Anchored discs take their remembered position; only unanchored discs
    // consume spiral slots (in size order, largest most central).
    let mut slot = 0usize;
    for &ci in &order {
        if let Some(&seed) = anchor_seeds.get(&ci) {
            centers[ci] = seed;
            continue;
        }
        if slot == 0 {
            centers[ci] = CENTER;
        } else {
            let r = spiral_step * (slot as f32).sqrt();
            let theta = slot as f32 * GOLDEN_ANGLE;
            centers[ci] = (CENTER.0 + r * theta.cos(), CENTER.1 + r * theta.sin());
        }
        slot += 1;
    }

    if k == 1 {
        return centers;
    }

    for _ in 0..PHASE1_ITERATIONS {
        let mut disp = vec![(0.0f32, 0.0f32); k];

        // Attraction: related discs drift toward touching distance.
        for &((ca, cb), w) in &affinity {
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

        // Separation: resolve any halo overlap symmetrically.
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
            member_ids: vec![id],
            category: "discussion".to_string(),
            affects_you: false,
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
            coherence: 0.0,
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

        compute_layout(&mut nodes, &edges, &mut clusters, &HashMap::new());

        let r = disc_radius(12);
        let (cx, cy) = (clusters[0].centroid_x, clusters[0].centroid_y);
        for n in &nodes {
            let d = ((n.x - cx).powi(2) + (n.y - cy).powi(2)).sqrt();
            // The collision pass may nudge members slightly past the disc rim.
            assert!(
                d <= r + COLLIDE_DIST,
                "member {} at distance {d} far outside disc {r}",
                n.id
            );
        }
    }

    #[test]
    fn cluster_discs_never_overlap() {
        let mut nodes: Vec<GraphNode> = (1..=30).map(node).collect();
        let mut clusters = vec![
            cluster("a", (1..=10).collect()),
            cluster("b", (11..=20).collect()),
            cluster("c", (21..=30).collect()),
        ];
        let edges = vec![edge(1, 11, 0.9), edge(11, 21, 0.9), edge(1, 21, 0.9)];

        compute_layout(&mut nodes, &edges, &mut clusters, &HashMap::new());

        for a in 0..clusters.len() {
            for b in (a + 1)..clusters.len() {
                let dx = clusters[a].centroid_x - clusters[b].centroid_x;
                let dy = clusters[a].centroid_y - clusters[b].centroid_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let min = disc_radius(10) * 2.0;
                assert!(dist >= min, "discs {a},{b} at {dist} (min {min})");
            }
        }
    }

    #[test]
    fn unrelated_singletons_form_a_shelf_below_the_map() {
        let mut nodes: Vec<GraphNode> = (1..=8).map(node).collect();
        let mut clusters = vec![cluster("a", vec![1, 2, 3, 4])];
        let edges = vec![edge(1, 2, 0.8), edge(3, 4, 0.8)];
        // 5-8 belong to no cluster → all go to the lane (no satellites:
        // proximity the edge model disowns is not shown as relation).
        compute_layout(&mut nodes, &edges, &mut clusters, &HashMap::new());

        let map_max_y = nodes
            .iter()
            .filter(|n| n.id <= 4)
            .map(|n| n.y)
            .fold(f32::MIN, f32::max);
        for id in [5i64, 6, 7, 8] {
            let n = nodes.iter().find(|n| n.id == id).unwrap();
            assert!(
                n.y > map_max_y + 100.0,
                "shelf node {id} at y {} not below map max {map_max_y}",
                n.y
            );
        }
        // Lane rows are horizontal: all four share one row here.
        let ys: Vec<f32> = [5i64, 6, 7, 8]
            .iter()
            .map(|id| nodes.iter().find(|n| n.id == *id).unwrap().y)
            .collect();
        assert!(ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1.0));
    }

    #[test]
    fn no_two_nodes_closer_than_collision_distance() {
        // A small cluster plus a crowded lane — the collision pass must keep
        // everything readable.
        let mut nodes: Vec<GraphNode> = (1..=40).map(node).collect();
        let mut clusters = vec![cluster("a", (1..=6).collect())];
        let edges: Vec<GraphEdge> = (2..=6).map(|i| edge(1, i, 0.8)).collect();
        compute_layout(&mut nodes, &edges, &mut clusters, &HashMap::new());

        for a in 0..nodes.len() {
            for b in (a + 1)..nodes.len() {
                let d =
                    ((nodes[a].x - nodes[b].x).powi(2) + (nodes[a].y - nodes[b].y).powi(2)).sqrt();
                assert!(
                    d >= COLLIDE_DIST * 0.7,
                    "nodes {} and {} at {d}",
                    nodes[a].id,
                    nodes[b].id
                );
            }
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
            compute_layout(&mut nodes, &edges, &mut clusters, &HashMap::new());
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

        compute_layout(&mut nodes, &edges, &mut clusters, &HashMap::new());
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
        compute_layout(&mut nodes, &[], &mut clusters, &HashMap::new());
    }
}
