// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Force-directed layout: Fruchterman-Reingold algorithm (deterministic).

use std::collections::HashMap;

use super::types::{GraphCluster, GraphEdge, GraphNode};
use super::{LAYOUT_HEIGHT, LAYOUT_ITERATIONS, LAYOUT_WIDTH};

pub(super) fn compute_layout(
    nodes: &mut [GraphNode],
    edges: &[GraphEdge],
    clusters: &mut [GraphCluster],
) {
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
