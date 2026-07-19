// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Temporal layout anchors (P2.11): persisted cluster positions so the map
//! stays spatially recognizable day-over-day.
//!
//! The layout is deterministic per build but recomputed globally, so a single
//! new item could rearrange the entire map — a daily-use surface where the
//! user's spatial memory resets every visit. Anchors fix that: after each
//! build the Tauri command persists every cluster's centroid plus its member
//! ITEM ids (not story-representative ids — representatives change when
//! relevance shifts). On the next build, clusters whose member sets overlap a
//! stored anchor seed their disc at the anchor position instead of a spiral
//! slot.
//!
//! Purity contract: `build_graph` only READS anchors — persisting is the
//! command wrapper's side effect. Two builds against the same DB state are
//! therefore still byte-identical (the e2e determinism test relies on this);
//! across builds, positions are approximately stable, not frozen — the force
//! pass still runs, so growth and new relations keep shaping the map.

use std::collections::{HashMap, HashSet};

use tracing::debug;

use super::types::ContentGraph;

/// Minimum member-set Jaccard overlap for a cluster to adopt an anchor.
/// Below this the cluster is a genuinely new theme and seeds on the spiral.
const ANCHOR_MIN_OVERLAP: f32 = 0.2;

/// Anchors older than this are yesterday's map of a corpus that no longer
/// exists; matching against them would pin new themes to stale geometry.
const ANCHOR_MAX_AGE_DAYS: u32 = 14;

pub(super) struct StoredAnchor {
    pub x: f32,
    pub y: f32,
    pub member_ids: HashSet<i64>,
}

/// Load stored anchors for this window. Read-tolerant: any failure yields an
/// empty set — the graph must never fail because layout memory is unreadable.
pub(super) fn load_anchors(conn: &rusqlite::Connection, window_days: u32) -> Vec<StoredAnchor> {
    let mut out = Vec::new();
    let Ok(mut stmt) = conn.prepare(
        "SELECT x, y, member_ids FROM graph_layout_anchors
         WHERE window_days = ?1
           AND updated_at >= datetime('now', ?2)
         ORDER BY cluster_key",
    ) else {
        return out;
    };
    let age_param = format!("-{ANCHOR_MAX_AGE_DAYS} days");
    let rows = stmt.query_map(rusqlite::params![window_days, age_param], |row| {
        Ok((
            row.get::<_, f64>(0)? as f32,
            row.get::<_, f64>(1)? as f32,
            row.get::<_, String>(2)?,
        ))
    });
    let Ok(rows) = rows else { return out };
    for row in rows.flatten() {
        let (x, y, ids_json) = row;
        let member_ids: HashSet<i64> = serde_json::from_str::<Vec<i64>>(&ids_json)
            .unwrap_or_default()
            .into_iter()
            .collect();
        if !member_ids.is_empty() {
            out.push(StoredAnchor { x, y, member_ids });
        }
    }
    out
}

/// Greedy anchor assignment: pairs ranked by Jaccard overlap (desc), ties
/// broken by cluster id then anchor index — deterministic. Each cluster and
/// each anchor is used at most once; pairs under [`ANCHOR_MIN_OVERLAP`] never
/// match. Returns cluster-id → seed position.
pub(super) fn match_anchors(
    cluster_members: &[(String, HashSet<i64>)],
    anchors: &[StoredAnchor],
) -> HashMap<String, (f32, f32)> {
    let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
    for (ci, (_, members)) in cluster_members.iter().enumerate() {
        for (ai, anchor) in anchors.iter().enumerate() {
            let inter = members.intersection(&anchor.member_ids).count();
            if inter == 0 {
                continue;
            }
            let union = members.len() + anchor.member_ids.len() - inter;
            let jaccard = inter as f32 / union.max(1) as f32;
            if jaccard >= ANCHOR_MIN_OVERLAP {
                pairs.push((jaccard, ci, ai));
            }
        }
    }
    pairs.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| cluster_members[a.1].0.cmp(&cluster_members[b.1].0))
            .then(a.2.cmp(&b.2))
    });

    let mut used_clusters: HashSet<usize> = HashSet::new();
    let mut used_anchors: HashSet<usize> = HashSet::new();
    let mut seeds = HashMap::new();
    for (_, ci, ai) in pairs {
        if used_clusters.contains(&ci) || used_anchors.contains(&ai) {
            continue;
        }
        used_clusters.insert(ci);
        used_anchors.insert(ai);
        seeds.insert(
            cluster_members[ci].0.clone(),
            (anchors[ai].x, anchors[ai].y),
        );
    }
    debug!(
        target: "4da::content_graph",
        matched = seeds.len(),
        clusters = cluster_members.len(),
        anchors = anchors.len(),
        "Layout anchors matched"
    );
    seeds
}

/// Persist the built graph's cluster geometry as next build's anchors.
/// Replaces this window's full anchor set (stale clusters age out with it).
/// Write-tolerant: failures log and return — layout memory is a nicety, the
/// graph result is already on its way to the frontend.
pub fn persist_layout_anchors(conn: &rusqlite::Connection, window_days: u32, graph: &ContentGraph) {
    let member_ids_of: HashMap<i64, &Vec<i64>> =
        graph.nodes.iter().map(|n| (n.id, &n.member_ids)).collect();

    let run = || -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM graph_layout_anchors WHERE window_days = ?1",
            rusqlite::params![window_days],
        )?;
        let mut stmt = conn.prepare(
            "INSERT INTO graph_layout_anchors (window_days, cluster_key, x, y, member_ids, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        )?;
        for cluster in &graph.clusters {
            let mut members: Vec<i64> = cluster
                .node_ids
                .iter()
                .flat_map(|id| {
                    member_ids_of
                        .get(id)
                        .map(|v| v.iter().copied())
                        .into_iter()
                        .flatten()
                })
                .collect();
            members.sort_unstable();
            let json = serde_json::to_string(&members).unwrap_or_else(|_| "[]".to_string());
            stmt.execute(rusqlite::params![
                window_days,
                cluster.id,
                f64::from(cluster.centroid_x),
                f64::from(cluster.centroid_y),
                json
            ])?;
        }
        Ok(())
    };
    if let Err(e) = run() {
        debug!(target: "4da::content_graph", error = %e, "Failed to persist layout anchors");
    }
}
