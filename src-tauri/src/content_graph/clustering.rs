// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Community detection (deterministic multi-level Louvain) and cluster label
//! extraction.
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

/// A term must appear in at least this share of member titles for a c-TF-IDF
/// label to be honest; below it the cluster gets a source-digest label
/// instead of three words that describe almost none of its members.
const LABEL_COVERAGE_MIN: f32 = 0.30;

/// A token present in at least this share of ONE source's items is that
/// source's boilerplate ("crates" prefixes every crates.io title) — it names
/// the source, not a topic, so it never enters labels.
const BOILERPLATE_SHARE: f32 = 0.80;

/// Sources need at least this many items in the window for a stable
/// boilerplate estimate.
const BOILERPLATE_MIN_ITEMS: usize = 5;

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
                centroid_x: 0.0,
                centroid_y: 0.0,
            }
        })
        .collect();
    clusters.sort_by(|a, b| a.id.cmp(&b.id));
    clusters
}

/// Label clusters by c-TF-IDF: a term scores by how frequent it is INSIDE the
/// cluster, discounted by how many OTHER clusters also use it. Raw frequency
/// produced junk labels ("our · middleware · second", "axios · via ·
/// prototype") because advisory boilerplate and connective words dominate
/// counts; distinctiveness against the sibling clusters is what names a topic.
///
/// Two honesty guards (2026-07-19):
/// - Source boilerplate ("crates" leads every crates.io title, so it covers
///   100% of a registry cluster and c-TF-IDF crowns it) is excluded per
///   source before counting.
/// - If the best term covers fewer than [`LABEL_COVERAGE_MIN`] of member
///   titles, no real shared topic exists — the label would degenerate to
///   member names ("crates · agentmail-rs · alpacars", live). Fall back to
///   an honest source digest label instead.
pub(super) fn assign_cluster_labels(items: &[RawItem], clusters: &mut [GraphCluster]) {
    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();
    let boilerplate = source_boilerplate_terms(items);
    let empty: HashSet<String> = HashSet::new();

    let keywords_of = |id: i64| -> Vec<String> {
        item_map
            .get(&id)
            .map(|item| {
                let boiler = boilerplate.get(item.source_type.as_str()).unwrap_or(&empty);
                extract_title_keywords(&item.title)
                    .into_iter()
                    .filter(|w| !boiler.contains(w))
                    .collect()
            })
            .unwrap_or_default()
    };

    // Per-cluster term frequencies.
    let tfs: Vec<HashMap<String, usize>> = clusters
        .iter()
        .map(|cluster| {
            let mut tf: HashMap<String, usize> = HashMap::new();
            for &id in &cluster.node_ids {
                for word in keywords_of(id) {
                    *tf.entry(word).or_insert(0) += 1;
                }
            }
            tf
        })
        .collect();

    // Document frequency across clusters (a "document" = one cluster).
    let mut df: HashMap<&str, usize> = HashMap::new();
    for tf in &tfs {
        for term in tf.keys() {
            *df.entry(term.as_str()).or_insert(0) += 1;
        }
    }

    let n_clusters = clusters.len().max(1) as f32;
    for (cluster, tf) in clusters.iter_mut().zip(&tfs) {
        let mut scored: Vec<(&str, f32)> = tf
            .iter()
            .map(|(term, &count)| {
                let d = df.get(term.as_str()).copied().unwrap_or(1) as f32;
                let idf = (1.0 + n_clusters / d).ln();
                (term.as_str(), count as f32 * idf)
            })
            .collect();
        // Deterministic: score desc, then alphabetical.
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });

        // Coverage: share of members whose title carries the top term, and
        // the absolute hit count. A term found in ONE title is not a shared
        // topic no matter how distinctive c-TF-IDF finds it — on 2-member
        // clusters the old 30% floor let single-title words label the pair
        // ("bun · claude · now", live 2026-07-19), so the top term must
        // appear in at least two member titles.
        let (coverage, coverage_hits) = scored
            .first()
            .map(|(top, _)| {
                let hits = cluster
                    .node_ids
                    .iter()
                    .filter(|id| keywords_of(**id).iter().any(|w| w == top))
                    .count();
                (hits as f32 / cluster.node_ids.len().max(1) as f32, hits)
            })
            .unwrap_or((0.0, 0));

        cluster.label =
            if coverage >= LABEL_COVERAGE_MIN && coverage_hits >= 2 && !scored.is_empty() {
                scored
                    .iter()
                    .take(3)
                    .map(|(w, _)| *w)
                    .collect::<Vec<_>>()
                    .join(" · ")
            } else {
                digest_label(cluster, &item_map)
            };
    }
}

/// Honest fallback when no term describes the cluster: name what it IS — a
/// group of related items with no single shared topic word.
fn digest_label(cluster: &GraphCluster, item_map: &HashMap<i64, &RawItem>) -> String {
    let mut sources: Vec<&str> = cluster
        .node_ids
        .iter()
        .filter_map(|id| item_map.get(id))
        .map(|i| i.source_type.as_str())
        .collect();
    sources.sort_unstable();
    sources.dedup();
    match sources.as_slice() {
        [one] => format!("{} · assorted", source_display(one)),
        _ => "related items · assorted".to_string(),
    }
}

fn source_display(source_type: &str) -> &str {
    match source_type {
        "crates_io" => "crates.io",
        "npm_registry" => "npm",
        "go_modules" => "go modules",
        "hackernews" => "hacker news",
        "papers_with_code" => "papers with code",
        other => other,
    }
}

/// Tokens appearing in ≥[`BOILERPLATE_SHARE`] of one source's items are that
/// source's template vocabulary, not topics.
fn source_boilerplate_terms(items: &[RawItem]) -> HashMap<&str, HashSet<String>> {
    let mut by_source: HashMap<&str, Vec<&RawItem>> = HashMap::new();
    for item in items {
        by_source
            .entry(item.source_type.as_str())
            .or_default()
            .push(item);
    }

    let mut out: HashMap<&str, HashSet<String>> = HashMap::new();
    for (source, members) in by_source {
        if members.len() < BOILERPLATE_MIN_ITEMS {
            continue;
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        for item in &members {
            let uniq: HashSet<String> = extract_title_keywords(&item.title).into_iter().collect();
            for w in uniq {
                *counts.entry(w).or_insert(0) += 1;
            }
        }
        let floor = (members.len() as f32 * BOILERPLATE_SHARE).ceil() as usize;
        let terms: HashSet<String> = counts
            .into_iter()
            .filter(|(_, c)| *c >= floor)
            .map(|(w, _)| w)
            .collect();
        if !terms.is_empty() {
            out.insert(source, terms);
        }
    }
    out
}

pub(super) fn extract_title_keywords(title: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a",
        "an",
        "the",
        "in",
        "of",
        "for",
        "to",
        "and",
        "is",
        "new",
        "on",
        "at",
        "by",
        "with",
        "from",
        "this",
        "that",
        "it",
        "its",
        "has",
        "have",
        "are",
        "was",
        "were",
        "been",
        "be",
        "do",
        "does",
        "did",
        "will",
        "would",
        "could",
        "should",
        "may",
        "can",
        "not",
        "no",
        "but",
        "or",
        "if",
        "how",
        "what",
        "when",
        "where",
        "who",
        "why",
        "which",
        "all",
        "each",
        "every",
        "both",
        "more",
        "most",
        "other",
        "some",
        "such",
        "than",
        "too",
        "very",
        "just",
        "about",
        "up",
        "out",
        "so",
        "show",
        "hn",
        "ask",
        "via",
        "our",
        "you",
        "your",
        "using",
        "into",
        "http",
        "https",
        "www",
        "com",
        // Generic verbs/adverbs and announcement boilerplate: maximally
        // "distinctive" to c-TF-IDF on small clusters yet topically empty —
        // live label leaks 2026-07-19: "bun · claude · NOW", "accelerated ·
        // bytecode · COME", "rewrite · GOING · rust-to-zig", "RELEASED ·
        // AHEAD · ahead-of-time", "ANNOUNCING · llvm · rust". Tech names
        // ("next", "go", "rust") stay labelable.
        "now",
        "one",
        "two",
        "three",
        "come",
        "comes",
        "coming",
        "going",
        "goes",
        "gets",
        "get",
        "got",
        "make",
        "makes",
        "made",
        "take",
        "takes",
        "like",
        "want",
        "wants",
        "really",
        "also",
        "even",
        "well",
        "say",
        "says",
        "said",
        "still",
        "back",
        "ahead",
        "today",
        "yesterday",
        "here",
        "there",
        "thing",
        "things",
        "released",
        "releases",
        "release",
        "announcing",
        "announced",
        "introducing",
        "available",
        "update",
        "updates",
        "updated",
    ];

    let keep = |w: &str| w.len() >= 3 && !STOPWORDS.contains(&w) && !is_numeric_noise(w);

    let mut out: Vec<String> = Vec::new();
    for token in title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| keep(w))
    {
        out.push(token.to_string());
        // Compound package names hide their shared theme inside one token:
        // "tauri-plugin-syncular" and "tauri-browser" share no whole token,
        // so a cluster of tauri crates could not be labeled "tauri". Emit
        // the sub-words too so shared prefixes become labelable.
        if token.contains(['-', '_']) {
            for sub in token.split(['-', '_']).filter(|s| keep(s)) {
                out.push(sub.to_string());
            }
        }
    }
    out
}

/// Digit-led or digit-dominated tokens name nothing a human scans a map by.
///
/// Two live leak classes (2026-07-16 and 2026-07-19): long digit runs are
/// ids/timestamps/URL fragments ("116885294589687234here"), and short
/// version/count/date tokens leak into labels as junk ("152", "8th", "191k",
/// "2026-07-14", "160-post") — maximally "distinctive" to c-TF-IDF yet
/// meaningless. Noise = starts with a digit (counts, ordinals, dates,
/// versions) OR carries at least as many digits as letters. Real names with
/// incidental digits ("typescript", "sqlite3", "react19") survive.
fn is_numeric_noise(token: &str) -> bool {
    if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    let digits = token.chars().filter(char::is_ascii_digit).count();
    let alphas = token.chars().filter(char::is_ascii_alphabetic).count();
    digits >= alphas.max(1)
}
