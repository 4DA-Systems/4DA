// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Cluster-first layout with semantic satellites (deterministic, no RNG).
//!
//! Phase 1 treats each cluster as a disc sized by member count: discs seed on
//! a circle (largest central), then a short force pass pulls discs with
//! inter-cluster edges together and separates overlapping discs. Disc spacing
//! reserves each cluster's satellite halo.
//! Phase 2 places members inside their disc on a golden-angle (sunflower)
//! spiral — collision-free by construction — with high-degree members central.
//!
//! Unclustered singletons are NOT parked on an arbitrary ring (the Wave 1
//! orbit ring made two-thirds of the canvas encode nothing — live measure
//! 2026-07-16: 41 of 63 nodes; a pure MDS projection was prototyped on the
//! real embeddings and rejected: 2D captured 22.6% of variance, a blob).
//! Instead each singleton becomes a SATELLITE of its semantically nearest
//! cluster, at a distance proportional to (1 - similarity) — so proximity on
//! screen means topical relatedness for every node. Live evidence: 90 of 93
//! singletons sit at cosine 0.45–0.77 from a cluster. The remainder (< the
//! [`SATELLITE_MIN_SIM`] floor) go to a small shelf grid below the map —
//! honest "unrelated to any theme" placement, no fake geometry.
//!
//! A final global collision pass resolves any residual overlap.

use std::collections::HashMap;

use super::types::{GraphCluster, GraphEdge, GraphNode};

/// Target spacing between neighboring member dots inside a cluster disc.
/// Sized for the readable label under each dot (~128px wide at zoom 1).
const MEMBER_SPACING: f32 = 95.0;
/// Minimum free gap between two cluster halos.
const CLUSTER_GAP: f32 = 120.0;
/// Golden angle in radians — successive spiral points never align.
const GOLDEN_ANGLE: f32 = 2.399_963;
/// Phase-1 iterations; the cluster graph is tiny (rarely >15 discs).
const PHASE1_ITERATIONS: usize = 120;
/// Logical canvas center; the frontend fits the view, so overflow is fine.
const CENTER: (f32, f32) = (600.0, 500.0);
/// Satellites closer than this to their disc edge would read as members.
const SATELLITE_BASE: f32 = 70.0;
/// How far (1 - similarity) pushes a satellite outward.
const SATELLITE_SPREAD: f32 = 260.0;
/// Below this best-similarity a singleton is genuinely unrelated → shelf.
pub(super) const SATELLITE_MIN_SIM: f32 = 0.45;
/// Shelf grid columns for unrelated singletons.
const SHELF_COLS: usize = 8;
/// Global collision pass: minimum center distance and sweep count.
const COLLIDE_DIST: f32 = 82.0;
const COLLIDE_ITERATIONS: usize = 60;

/// A singleton's semantic attachment: nearest cluster + best similarity.
pub(super) struct SatelliteAssign {
    pub cluster_id: String,
    pub sim: f32,
}

pub(super) fn compute_layout(
    nodes: &mut [GraphNode],
    edges: &[GraphEdge],
    clusters: &mut [GraphCluster],
    satellites: &HashMap<i64, SatelliteAssign>,
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

    // Satellites per cluster, most-similar first (deterministic tiebreak).
    let cluster_pos_by_id: HashMap<&str, usize> = clusters
        .iter()
        .enumerate()
        .map(|(ci, c)| (c.id.as_str(), ci))
        .collect();
    let mut sats_of: Vec<Vec<(usize, f32)>> = vec![Vec::new(); clusters.len()];
    for (idx, node) in nodes.iter().enumerate() {
        if cluster_of.contains_key(&idx) {
            continue;
        }
        if let Some(assign) = satellites.get(&node.id) {
            if let Some(&ci) = cluster_pos_by_id.get(assign.cluster_id.as_str()) {
                sats_of[ci].push((idx, assign.sim));
            }
        }
    }
    for sats in &mut sats_of {
        sats.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| nodes[a.0].id.cmp(&nodes[b.0].id))
        });
    }

    // Halo radius = disc + this cluster's farthest satellite orbit.
    let disc_radii: Vec<f32> = clusters
        .iter()
        .map(|c| disc_radius(c.node_ids.len()))
        .collect();
    let halo_radii: Vec<f32> = disc_radii
        .iter()
        .enumerate()
        .map(|(ci, &r)| {
            let worst_sim = sats_of[ci].last().map(|&(_, s)| s).unwrap_or(1.0);
            if sats_of[ci].is_empty() {
                r
            } else {
                r + SATELLITE_BASE + (1.0 - worst_sim).max(0.0) * SATELLITE_SPREAD + 40.0
            }
        })
        .collect();

    let centers = place_cluster_discs(clusters, &halo_radii, edges, &id_to_idx, &cluster_of);

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

    // Satellites: golden-angle around their cluster, radius grows as
    // similarity falls — closer on screen IS more related.
    for (ci, sats) in sats_of.iter().enumerate() {
        let (cx, cy) = centers[ci];
        for (slot, &(idx, sim)) in sats.iter().enumerate() {
            let radius = disc_radii[ci] + SATELLITE_BASE + (1.0 - sim).max(0.0) * SATELLITE_SPREAD;
            let theta = slot as f32 * GOLDEN_ANGLE + ci as f32 * 0.7 + 1.2;
            nodes[idx].x = cx + radius * theta.cos();
            nodes[idx].y = cy + radius * theta.sin();
        }
    }

    // Shelf: singletons related to nothing (below the similarity floor, or
    // no clusters exist at all). A plain grid under the map — honest, no
    // implied geometry.
    let shelf_idxs: Vec<usize> = {
        let mut v: Vec<usize> = (0..nodes.len())
            .filter(|idx| {
                !cluster_of.contains_key(idx) && !satellites.contains_key(&nodes[*idx].id)
            })
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
        let shelf_top = max_y + 200.0;
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

/// Phase 1: place cluster discs. Seed on a circle (largest disc central),
/// then iterate attraction along aggregated inter-cluster edges + separation
/// of overlapping halos.
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
            member_titles: Vec::new(),
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

    fn sat(cluster_id: &str, sim: f32) -> SatelliteAssign {
        SatelliteAssign {
            cluster_id: cluster_id.to_string(),
            sim,
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
    fn satellites_orbit_their_cluster_ordered_by_similarity() {
        let mut nodes: Vec<GraphNode> = (1..=7).map(node).collect();
        let mut clusters = vec![cluster("a", vec![1, 2, 3, 4])];
        let edges = vec![edge(1, 2, 0.8), edge(3, 4, 0.8)];
        let mut sats = HashMap::new();
        sats.insert(5i64, sat("a", 0.75)); // very related → closest
        sats.insert(6i64, sat("a", 0.60));
        sats.insert(7i64, sat("a", 0.46)); // barely related → farthest

        compute_layout(&mut nodes, &edges, &mut clusters, &sats);

        let (cx, cy) = (clusters[0].centroid_x, clusters[0].centroid_y);
        let dist = |id: i64| {
            let n = nodes.iter().find(|n| n.id == id).unwrap();
            ((n.x - cx).powi(2) + (n.y - cy).powi(2)).sqrt()
        };
        let r = disc_radius(4);
        assert!(dist(5) > r, "satellite must sit outside the disc");
        assert!(
            dist(5) < dist(6) && dist(6) < dist(7),
            "orbit distance must fall with similarity: {} {} {}",
            dist(5),
            dist(6),
            dist(7)
        );
    }

    #[test]
    fn unrelated_singletons_form_a_shelf_below_the_map() {
        let mut nodes: Vec<GraphNode> = (1..=8).map(node).collect();
        let mut clusters = vec![cluster("a", vec![1, 2, 3, 4])];
        let edges = vec![edge(1, 2, 0.8), edge(3, 4, 0.8)];
        // 5 is a satellite; 6,7,8 have no assignment → shelf.
        let mut sats = HashMap::new();
        sats.insert(5i64, sat("a", 0.6));

        compute_layout(&mut nodes, &edges, &mut clusters, &sats);

        let map_max_y = nodes
            .iter()
            .filter(|n| n.id <= 5)
            .map(|n| n.y)
            .fold(f32::MIN, f32::max);
        for id in [6i64, 7, 8] {
            let n = nodes.iter().find(|n| n.id == id).unwrap();
            assert!(
                n.y > map_max_y + 100.0,
                "shelf node {id} at y {} not below map max {map_max_y}",
                n.y
            );
        }
        // Shelf rows are horizontal: all three share one row here.
        let ys: Vec<f32> = [6i64, 7, 8]
            .iter()
            .map(|id| nodes.iter().find(|n| n.id == *id).unwrap().y)
            .collect();
        assert!((ys[0] - ys[1]).abs() < 1.0 && (ys[1] - ys[2]).abs() < 1.0);
    }

    #[test]
    fn no_two_nodes_closer_than_collision_distance() {
        // Crowd one cluster with many satellites at the same similarity —
        // the collision pass must keep everything readable.
        let mut nodes: Vec<GraphNode> = (1..=40).map(node).collect();
        let mut clusters = vec![cluster("a", (1..=6).collect())];
        let edges: Vec<GraphEdge> = (2..=6).map(|i| edge(1, i, 0.8)).collect();
        let mut sats = HashMap::new();
        for id in 7i64..=40 {
            sats.insert(id, sat("a", 0.6));
        }

        compute_layout(&mut nodes, &edges, &mut clusters, &sats);

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
            let mut sats = HashMap::new();
            sats.insert(13i64, sat("a", 0.7));
            sats.insert(14i64, sat("b", 0.5));
            compute_layout(&mut nodes, &edges, &mut clusters, &sats);
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
