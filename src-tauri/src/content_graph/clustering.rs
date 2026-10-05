// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Community detection (deterministic multi-level Louvain). Labels live in
//! labels.rs.
//!
//! Connected components were the previous mechanism and are exactly why the
//! live graph rendered one 23-node "theme" mixing tauri plugins, axum crates,
//! a YNAB client, and Kerberos bindings: components CHAIN — one promiscuous
//! hub welds every reachable node into a single cluster. Weighted modularity
//! optimization keeps densely-linked sub-themes together and cuts the chains
//! (live prototype 2026-07-19: the same edge set split into a pure 9-node
//! tauri theme, a pure axum theme, and honest dust).
//!
//! Determinism: nodes are visited in ascending item-id order, ties break
//! toward the smaller community id, and there is no RNG — the same corpus
//! always yields the same communities (every user gets a stable graph).

use std::collections::{HashMap, HashSet};

use super::types::{GraphCluster, GraphEdge, RawItem};

/// Max local-moving sweeps. Converges in single digits on ≤150 nodes; the cap
/// only guards against pathological oscillation.
const MAX_SWEEPS: usize = 30;

/// Max aggregation levels. Real corpora converge in 2-3; the cap is a guard.
const MAX_LEVELS: usize = 10;

/// One Louvain level: greedy modularity local moving over a weighted graph.
///
/// `adj[i]` lists (neighbor, weight) with no self entries; `self_w[i]` is the
/// node's internal weight (nonzero for aggregated super-nodes, counted twice
/// in the degree per the standard Louvain convention). Deterministic: nodes
/// visit in index order, candidate communities in ascending id, ties keep the
/// current community. Returns per-node community labels.
fn local_moving(adj: &[Vec<(usize, f32)>], self_w: &[f32]) -> Vec<usize> {
    let n = adj.len();
    let mut degree = vec![0.0f32; n];
    let mut m2 = 0.0f32;
    for i in 0..n {
        let mut k = 2.0 * self_w[i];
        for &(_, w) in &adj[i] {
            k += w;
        }
        degree[i] = k;
        m2 += k;
    }
    if m2 <= 0.0 {
        return (0..n).collect();
    }

    let mut community: Vec<usize> = (0..n).collect();
    let mut sigma_tot = degree.clone();
    for _ in 0..MAX_SWEEPS {
        let mut moved = false;
        for i in 0..n {
            if adj[i].is_empty() {
                continue;
            }
            let current = community[i];

            // Weight from i to each neighboring community.
            let mut links: HashMap<usize, f32> = HashMap::new();
            for &(j, w) in &adj[i] {
                *links.entry(community[j]).or_insert(0.0) += w;
            }

            // Score of i in community c = links(i→c) − k_i · Σ_tot(c \ i) / m2
            // (modularity gain with shared constant terms dropped).
            let sigma_own = sigma_tot[current] - degree[i];
            let stay = links.get(&current).copied().unwrap_or(0.0) - degree[i] * sigma_own / m2;

            let mut candidates: Vec<usize> = links.keys().copied().collect();
            candidates.sort_unstable(); // deterministic tie-break: smaller id
            let mut best = current;
            let mut best_score = stay;
            for c in candidates {
                if c == current {
                    continue;
                }
                let score = links[&c] - degree[i] * sigma_tot[c] / m2;
                if score > best_score + 1e-7 {
                    best = c;
                    best_score = score;
                }
            }

            if best != current {
                sigma_tot[current] -= degree[i];
                sigma_tot[best] += degree[i];
                community[i] = best;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    community
}

pub(super) fn compute_clusters(items: &[RawItem], edges: &[GraphEdge]) -> Vec<GraphCluster> {
    let n = items.len();
    if n == 0 || edges.is_empty() {
        return Vec::new();
    }

    // Deterministic node order: ascending item id.
    let mut order: Vec<i64> = items.iter().map(|i| i.id).collect();
    order.sort_unstable();
    let idx_of: HashMap<i64, usize> = order.iter().enumerate().map(|(i, &id)| (id, i)).collect();

    // Weighted adjacency (duplicates already merged upstream).
    let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    for edge in edges {
        let (Some(&a), Some(&b)) = (idx_of.get(&edge.source), idx_of.get(&edge.target)) else {
            continue;
        };
        if a == b {
            continue;
        }
        let w = edge.weight.max(0.0);
        adj[a].push((b, w));
        adj[b].push((a, w));
    }

    // Multi-level Louvain. A single local-moving pass under-merges: it gets
    // stuck wherever no SINGLE node move improves modularity but moving a
    // whole group would (live proof 2026-07-19: four AI-topic clusters held
    // four inter-cluster edges among themselves and stayed split — the map
    // rendered ~30 micro-themes instead of ~10 legible ones). Aggregating
    // communities into super-nodes and re-running local moves is the standard
    // Louvain fix and finds exactly those group merges.
    //
    // `leaf_members[s]` = leaf indexes represented by current super-node s;
    // `self_w[s]` = internal weight (intra-community edges folded so far).
    let mut cur_adj = adj;
    let mut self_w = vec![0.0f32; n];
    let mut leaf_members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    for _ in 0..MAX_LEVELS {
        let community = local_moving(&cur_adj, &self_w);

        // Group super-nodes by community, deterministic: communities ordered
        // by their smallest member super-node index (itself ordered by
        // smallest leaf id transitively).
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_of: HashMap<usize, usize> = HashMap::new();
        for (s, &c) in community.iter().enumerate() {
            match group_of.get(&c) {
                Some(&g) => groups[g].push(s),
                None => {
                    group_of.insert(c, groups.len());
                    groups.push(vec![s]);
                }
            }
        }
        if groups.len() == cur_adj.len() {
            break; // no merges this level — converged
        }

        // Aggregate into the next-level graph. Pair sums accumulate in the
        // deterministic order of the current adjacency (canonical upstream),
        // so f32 addition order is stable across runs.
        let g_of: Vec<usize> = {
            let mut v = vec![0usize; cur_adj.len()];
            for (g, members) in groups.iter().enumerate() {
                for &s in members {
                    v[s] = g;
                }
            }
            v
        };
        let mut new_self = vec![0.0f32; groups.len()];
        for (g, members) in groups.iter().enumerate() {
            for &s in members {
                new_self[g] += self_w[s];
            }
        }
        let mut pair_w: std::collections::BTreeMap<(usize, usize), f32> =
            std::collections::BTreeMap::new();
        for (s, neighbors) in cur_adj.iter().enumerate() {
            for &(t, w) in neighbors {
                if s >= t {
                    continue; // each undirected edge once
                }
                let (gs, gt) = (g_of[s], g_of[t]);
                if gs == gt {
                    new_self[gs] += w;
                } else {
                    let key = if gs < gt { (gs, gt) } else { (gt, gs) };
                    *pair_w.entry(key).or_insert(0.0) += w;
                }
            }
        }
        let mut new_adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); groups.len()];
        for (&(a, b), &w) in &pair_w {
            new_adj[a].push((b, w));
            new_adj[b].push((a, w));
        }
        let new_members: Vec<Vec<usize>> = groups
            .iter()
            .map(|g| {
                let mut m: Vec<usize> = g.iter().flat_map(|&s| leaf_members[s].clone()).collect();
                m.sort_unstable();
                m
            })
            .collect();

        cur_adj = new_adj;
        self_w = new_self;
        leaf_members = new_members;
    }

    // Materialize communities with ≥2 members.
    let mut members: HashMap<usize, Vec<i64>> = HashMap::new();
    for (s, leafs) in leaf_members.iter().enumerate() {
        members.insert(s, leafs.iter().map(|&i| order[i]).collect());
    }

    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();
    let mut clusters: Vec<GraphCluster> = members
        .into_values()
        .filter(|ids| ids.len() >= 2)
        .map(|mut node_ids| {
            node_ids.sort_unstable();
            let sources: HashSet<&str> = node_ids
                .iter()
                .filter_map(|id| item_map.get(id))
                .map(|i| i.source_type.as_str())
                .collect();
            GraphCluster {
                id: format!("cluster_{}", node_ids[0]),
                label: String::new(),
                node_ids,
                source_count: sources.len(),
                coherence: 0.0,
                stack_node_ids: Vec::new(),
            }
        })
        .collect();
    clusters.sort_by(|a, b| a.id.cmp(&b.id));
    clusters
}
