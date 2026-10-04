// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for the content graph pipeline — split from mod.rs so the pipeline
//! module stays inside the size gate while the test corpus keeps growing.

use super::*;
use types::RawItem;

fn raw(id: i64, title: &str, source_type: &str, score: f32, embedding: Vec<f32>) -> RawItem {
    RawItem {
        id,
        title: title.to_string(),
        url: None,
        source_type: source_type.to_string(),
        relevance_score: score,
        signal_type: None,
        signal_priority: None,
        matched_package: None,
        created_at: "2026-05-24".to_string(),
        curated: false,
        reserved: false,
        grounded: false,
        embedding,
    }
}

fn edge(source: i64, target: i64, weight: f32) -> GraphEdge {
    GraphEdge {
        source,
        target,
        edge_type: EdgeType::Semantic,
        weight,
        label: None,
        methods: vec!["semantic".to_string()],
    }
}

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
            story_count: 0,
            collapsed_items: 0,
            hidden_items: 0,
            window_candidates: 0,
            time_window_days: 7,
            edge_threshold: "mutual top-3 nearest neighbors".to_string(),
            mean_cluster_coherence: None,
            curated_items: 0,
            windows_differ: false,
        },
    };
    assert_eq!(graph.nodes.len(), 0);
    assert_eq!(graph.edges.len(), 0);
}

#[test]
fn test_mutual_neighbors_create_edge() {
    let items = vec![
        raw(
            1,
            "Rust async runtime",
            "hackernews",
            0.8,
            vec![1.0, 0.0, 0.0],
        ),
        raw(
            2,
            "Rust async runtime update",
            "reddit",
            0.7,
            vec![1.0, 0.0, 0.0],
        ),
    ];

    let mut edge_list = Vec::new();
    edges::compute_semantic_edges(&items, &mut edge_list);

    assert_eq!(
        edge_list.len(),
        1,
        "identical embeddings are mutual nearest neighbors"
    );
    assert_eq!(edge_list[0].edge_type, EdgeType::Semantic);
    assert!((edge_list[0].weight - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_knn_floor_blocks_nonsense_mutual_pairs() {
    // Two orthogonal items are each other's ONLY neighbor — mutual by
    // construction — but similarity 0 sits under KNN_FLOOR, so no edge.
    let items = vec![
        raw(1, "Rust async", "hackernews", 0.8, vec![1.0, 0.0, 0.0]),
        raw(
            2,
            "Python web framework",
            "reddit",
            0.7,
            vec![0.0, 1.0, 0.0],
        ),
    ];

    let mut edge_list = Vec::new();
    edges::compute_semantic_edges(&items, &mut edge_list);

    assert!(
        edge_list.is_empty(),
        "sub-floor mutual neighbors must not connect"
    );
}

#[test]
fn test_knn_mutuality_required() {
    // Hub h is near a, b, c, d (its top-3 covers only 3 of them), while
    // e is far from everything except h. e ranks h first, but h's top-3
    // never includes e -> no h–e edge (this asymmetry-cut is what stops
    // one promiscuous hub from wiring the whole corpus together).
    let items = vec![
        raw(1, "hub", "hn", 0.9, vec![1.0, 0.0, 0.0]),
        raw(2, "a", "hn", 0.9, vec![0.99, 0.14, 0.0]),
        raw(3, "b", "hn", 0.9, vec![0.99, 0.0, 0.14]),
        raw(4, "c", "hn", 0.9, vec![0.99, 0.10, 0.10]),
        raw(5, "e", "hn", 0.9, vec![0.75, -0.66, 0.0]),
    ];

    let mut edge_list = Vec::new();
    edges::compute_semantic_edges(&items, &mut edge_list);

    assert!(
        !edge_list
            .iter()
            .any(|e| (e.source == 1 && e.target == 5) || (e.source == 5 && e.target == 1)),
        "one-sided nearest-neighbor pairs must not connect"
    );
}

#[test]
fn test_knn_edges_deterministic() {
    let build = || {
        let items: Vec<RawItem> = (0..20)
            .map(|i| {
                let angle = i as f32 * 0.3;
                raw(
                    i64::from(i),
                    "t",
                    "hn",
                    0.5,
                    vec![angle.cos(), angle.sin(), 0.2],
                )
            })
            .collect();
        let mut edge_list = Vec::new();
        edges::compute_semantic_edges(&items, &mut edge_list);
        edge_list
            .iter()
            .map(|e| (e.source, e.target))
            .collect::<Vec<_>>()
    };
    assert_eq!(build(), build());
}

#[test]
fn test_merge_duplicate_edges() {
    let mut edge_list = vec![
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

    edges::merge_duplicate_edges(&mut edge_list);

    assert_eq!(edge_list.len(), 1, "duplicate edges should merge");
    assert_eq!(edge_list[0].edge_type, EdgeType::Convergence);
    assert_eq!(edge_list[0].methods.len(), 2);
    assert!((edge_list[0].weight - 0.85).abs() < f32::EPSILON);
}

#[test]
fn test_cluster_formation() {
    let items = vec![
        raw(1, "A", "hn", 0.8, vec![1.0, 0.0]),
        raw(2, "B", "reddit", 0.7, vec![1.0, 0.0]),
        raw(3, "C", "github", 0.6, vec![0.0, 1.0]),
    ];

    let edge_list = vec![edge(1, 2, 0.9)];

    let clusters = clustering::compute_clusters(&items, &edge_list);
    assert_eq!(clusters.len(), 1, "connected items should form one cluster");
    assert_eq!(clusters[0].node_ids.len(), 2);
    assert_eq!(clusters[0].source_count, 2);
}

#[test]
fn test_cluster_labels_prefer_distinctive_terms() {
    // "release" appears in both clusters; the distinctive terms must win.
    let items = vec![
        raw(
            1,
            "React server components release",
            "hn",
            0.8,
            vec![1.0, 0.0],
        ),
        raw(
            2,
            "React server actions release",
            "reddit",
            0.7,
            vec![1.0, 0.0],
        ),
        raw(3, "Tokio scheduler release", "hn", 0.8, vec![0.0, 1.0]),
        raw(
            4,
            "Tokio runtime internals release",
            "reddit",
            0.7,
            vec![0.0, 1.0],
        ),
    ];
    let edge_list = vec![edge(1, 2, 0.9), edge(3, 4, 0.9)];

    let mut clusters = clustering::compute_clusters(&items, &edge_list);
    labels::assign_cluster_labels(&items, &mut clusters);

    for cluster in &clusters {
        let first = cluster.label.split(" · ").next().unwrap_or("");
        assert_ne!(
            first, "release",
            "shared term must not lead a label; got '{}'",
            cluster.label
        );
    }
}

#[test]
fn test_sparsify_keeps_backbone_connectivity() {
    // A 10-node clique: sparsify must keep every node reachable and cut
    // well below the full 45 edges.
    let mut edge_list = Vec::new();
    for i in 1..=10i64 {
        for j in (i + 1)..=10 {
            edge_list.push(edge(i, j, 0.8 + (i as f32) * 0.001));
        }
    }
    edges::sparsify_edges(&mut edge_list, 3);

    assert!(
        edge_list.len() < 45,
        "clique should shrink, kept {}",
        edge_list.len()
    );
    // Connectivity: union everything and confirm one component.
    let mut parent: std::collections::HashMap<i64, i64> = (1..=10).map(|i| (i, i)).collect();
    fn find(parent: &std::collections::HashMap<i64, i64>, mut x: i64) -> i64 {
        while parent[&x] != x {
            x = parent[&x];
        }
        x
    }
    for e in &edge_list {
        let (a, b) = (find(&parent, e.source), find(&parent, e.target));
        if a != b {
            parent.insert(a.max(b), a.min(b));
        }
    }
    let roots: std::collections::HashSet<i64> = (1..=10).map(|i| find(&parent, i)).collect();
    assert_eq!(roots.len(), 1, "sparsified graph must stay connected");
}

#[test]
fn test_sparsify_leaves_sparse_graphs_alone() {
    let mut edge_list = vec![edge(1, 2, 0.9), edge(2, 3, 0.8)];
    edges::sparsify_edges(&mut edge_list, 4);
    assert_eq!(edge_list.len(), 2);
}

#[test]
fn test_title_word_overlap_high() {
    // 0.5 = STORY_OVERLAP_MIN in story.rs, title_word_overlap's only consumer.
    let overlap = edges::title_word_overlap(
        "React 19 server components released",
        "React 19 server components update",
    );
    assert!(
        overlap > 0.5,
        "similar titles should overlap >0.5, got {overlap}"
    );
}

#[test]
fn test_title_word_overlap_low() {
    let overlap = edges::title_word_overlap("Rust async runtime", "Python web framework");
    assert!(overlap < 0.5);
}

#[test]
fn test_louvain_cuts_hub_chains() {
    // Two dense themes bridged by one promiscuous hub. Connected
    // components fused all 9 nodes into one cluster (the live 23-node
    // crates blob); modularity must keep the themes separate.
    let items: Vec<RawItem> = (1..=9)
        .map(|i| raw(i, "t", "src", 0.5, vec![1.0, 0.0]))
        .collect();
    let mut edge_list = Vec::new();
    // Theme A: 1-2-3-4 clique.
    for i in 1..=4i64 {
        for j in (i + 1)..=4 {
            edge_list.push(edge(i, j, 0.9));
        }
    }
    // Theme B: 5-6-7-8 clique.
    for i in 5..=8i64 {
        for j in (i + 1)..=8 {
            edge_list.push(edge(i, j, 0.9));
        }
    }
    // Hub 9 weakly touches both themes.
    edge_list.push(edge(9, 1, 0.6));
    edge_list.push(edge(9, 5, 0.6));

    let clusters = clustering::compute_clusters(&items, &edge_list);

    let of = |id: i64| {
        clusters
            .iter()
            .position(|c| c.node_ids.contains(&id))
            .expect("clustered")
    };
    assert_eq!(of(1), of(4), "theme A stays together");
    assert_eq!(of(5), of(8), "theme B stays together");
    assert_ne!(of(1), of(5), "hub must not weld the two themes");
}

/// Two dense themes joined by TWO strong bridges in a large-enough graph:
/// no SINGLE node move improves modularity (each node's internal links
/// dominate its bridge), but merging the two groups does — exactly the
/// case single-level local moving cannot find and the aggregation levels
/// exist for. Live analogue 2026-07-19: four AI-topic clusters holding
/// four inter-cluster edges among themselves, rendered split.
#[test]
fn test_multilevel_merges_bridged_themes() {
    let items: Vec<RawItem> = (1..=18)
        .map(|i| raw(i, "t", "src", 0.5, vec![1.0, 0.0]))
        .collect();
    let mut edge_list = Vec::new();
    // Six 3-cliques (0.9 internal).
    for base in [1i64, 4, 7, 10, 13, 16] {
        for a in base..base + 3 {
            for b in (a + 1)..base + 3 {
                edge_list.push(edge(a, b, 0.9));
            }
        }
    }
    // Cliques {1,2,3} and {4,5,6} joined by two strong bridges.
    edge_list.push(edge(1, 4, 0.8));
    edge_list.push(edge(2, 5, 0.8));

    let clusters = clustering::compute_clusters(&items, &edge_list);
    let of = |id: i64| {
        clusters
            .iter()
            .position(|c| c.node_ids.contains(&id))
            .expect("clustered")
    };
    assert_eq!(of(1), of(4), "double-bridged themes must merge");
    assert_ne!(of(1), of(7), "unbridged cliques stay separate");
    assert_eq!(clusters.len(), 5, "6 cliques minus one merge");
}

/// A term found in ONE member title is not a shared topic: a 2-member
/// cluster with disjoint titles must take the fallback label, not a
/// single-title word crowned by c-TF-IDF ("bun · claude · now", live).
#[test]
fn test_label_requires_two_title_hits() {
    let items = vec![
        raw(
            1,
            "Rust async runtime deep dive",
            "hackernews",
            0.8,
            vec![1.0, 0.0],
        ),
        raw(
            2,
            "Postgres tuning secrets",
            "hackernews",
            0.7,
            vec![1.0, 0.0],
        ),
    ];
    let edge_list = vec![edge(1, 2, 0.9)];

    let mut clusters = clustering::compute_clusters(&items, &edge_list);
    assert_eq!(clusters.len(), 1);
    labels::assign_cluster_labels(&items, &mut clusters);
    // No term appears in 2+ titles → no term label; the fallback names the
    // typical member (equal embeddings: the earliest), never "assorted".
    assert_eq!(
        clusters[0].label, "Rust async runtime deep dive",
        "no term appears in 2+ titles → representative-title label"
    );
}

/// The graph's chain floor must sit strictly between the ungrounded
/// confidence cap and the grounded band start (~0.43, see chain_policy):
/// exactly the dependency-grounded chains render. If the policy bands
/// move, this breaks loudly instead of silently letting keyword welds
/// back onto the map.
#[test]
fn chain_floor_sits_between_confidence_bands() {
    assert!(edges::CHAIN_MIN_CONFIDENCE > crate::signal_chains::UNGROUNDED_CONFIDENCE_CAP);
    assert!(edges::CHAIN_MIN_CONFIDENCE < 0.43);
}

#[test]
fn test_louvain_deterministic() {
    let build = || {
        let items: Vec<RawItem> = (1..=12)
            .map(|i| raw(i, "t", "src", 0.5, vec![1.0, 0.0]))
            .collect();
        let mut edge_list = Vec::new();
        for i in 1..=6i64 {
            for j in (i + 1)..=6 {
                edge_list.push(edge(i, j, 0.8));
            }
        }
        for i in 7..=12i64 {
            for j in (i + 1)..=12 {
                edge_list.push(edge(i, j, 0.8));
            }
        }
        edge_list.push(edge(3, 9, 0.55));
        clustering::compute_clusters(&items, &edge_list)
            .into_iter()
            .map(|c| c.node_ids)
            .collect::<Vec<_>>()
    };
    assert_eq!(build(), build());
}

#[test]
fn test_label_falls_back_to_source_digest_when_no_shared_topic() {
    // Six crates.io releases with NOTHING in common except the source
    // template — the live case whose label degenerated to
    // "crates · agentmail-rs · alpacars". "crates" is source boilerplate
    // (in 100% of titles) and no other term covers 30%, so the honest
    // digest label must win.
    let titles = [
        "crates.io: agentmail-rs v0.2.0",
        "crates.io: alpacars v0.3.0",
        "crates.io: krb5-gss v0.1.0",
        "crates.io: rust-ynab v0.5.6",
        "crates.io: oganesson-rs v0.2.1",
        "crates.io: nidus-sqs v1.0.12",
    ];
    let items: Vec<RawItem> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| raw(i as i64 + 1, t, "crates_io", 0.5, vec![1.0, 0.0]))
        .collect();
    let mut edge_list = Vec::new();
    for i in 1..=6i64 {
        for j in (i + 1)..=6 {
            edge_list.push(edge(i, j, 0.8));
        }
    }

    let mut clusters = clustering::compute_clusters(&items, &edge_list);
    assert_eq!(clusters.len(), 1);
    labels::assign_cluster_labels(&items, &mut clusters);

    assert_eq!(
        clusters[0].label, "crates.io releases",
        "template-only cohesion is stated as what it is — a registry group-by"
    );
}

#[test]
fn test_label_uses_real_topic_despite_source_boilerplate() {
    // Boilerplate-heavy source ("crates" in every title) plus a REAL
    // shared theme inside compound names — the live tauri sub-theme. The
    // item universe includes unrelated crates so "tauri" is a topic
    // (5 of 11 items), not source boilerplate. Sub-token extraction must
    // surface it and the label must not be the digest fallback.
    let titles = [
        "crates.io: tauri-browser v0.5.0",
        "crates.io: tauri-plugin-syncular v0.3.1",
        "crates.io: tauri-plugin-hdiff-update v0.3.0",
        "crates.io: tauri-plugin-thermal-printer v2.1.0",
        "crates.io: tauri-plugin-serialplugin v3.0.1",
        "crates.io: agentmail-rs v0.2.0",
        "crates.io: alpacars v0.3.0",
        "crates.io: krb5-gss v0.1.0",
        "crates.io: rust-ynab v0.5.6",
        "crates.io: oganesson-rs v0.2.1",
        "crates.io: nidus-sqs v1.0.12",
    ];
    let items: Vec<RawItem> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| raw(i as i64 + 1, t, "crates_io", 0.5, vec![1.0, 0.0]))
        .collect();
    // Only the five tauri crates form a cluster.
    let mut edge_list = Vec::new();
    for i in 1..=5i64 {
        for j in (i + 1)..=5 {
            edge_list.push(edge(i, j, 0.85));
        }
    }

    let mut clusters = clustering::compute_clusters(&items, &edge_list);
    assert_eq!(clusters.len(), 1);
    labels::assign_cluster_labels(&items, &mut clusters);

    assert!(
        clusters[0].label.split(" · ").any(|w| w == "tauri"),
        "the shared 'tauri' theme must be labelable; got '{}'",
        clusters[0].label
    );
    assert!(
        !clusters[0].label.split(" · ").any(|w| w == "crates"),
        "source boilerplate must not enter labels; got '{}'",
        clusters[0].label
    );
}

#[test]
fn test_extract_title_keywords() {
    let keywords = labels::extract_title_keywords("Show HN: A New Rust Web Framework");
    assert!(keywords.contains(&"rust".to_string()));
    assert!(keywords.contains(&"web".to_string()));
    assert!(keywords.contains(&"framework".to_string()));
    assert!(!keywords.contains(&"a".to_string()));
    assert!(!keywords.contains(&"hn".to_string()));
}

#[test]
fn test_extract_title_keywords_drops_numeric_noise() {
    // Two live label leaks: a mastodon status URL glued to text tokenized
    // as "116885294589687234Here" (2026-07-16), and short version/count
    // tokens crowning labels ("152", "8th", "191k", "160-post",
    // 2026-07-19). Digit-dominated tokens never label; real names with
    // incidental digits survive.
    let keywords = labels::extract_title_keywords(
        "RE: fosstodon 116885294589687234Here TypeScript 7.0 released v5-9 8th 191k sqlite3",
    );
    assert!(!keywords.iter().any(|k| k.contains("116885294589687234")));
    assert!(keywords.contains(&"typescript".to_string()));
    assert!(
        keywords.contains(&"sqlite3".to_string()),
        "incidental digit stays"
    );
    for noise in ["v5-9", "8th", "191k"] {
        assert!(
            !keywords.contains(&noise.to_string()),
            "digit-dominated token '{noise}' must not label"
        );
    }
}

#[test]
fn test_edge_count_per_node() {
    let edge_list = vec![edge(1, 2, 0.9), edge(1, 3, 0.8)];

    let counts = edges::count_edges_per_node(&edge_list);
    assert_eq!(counts[&1], 2);
    assert_eq!(counts[&2], 1);
    assert_eq!(counts[&3], 1);
}

/// End-to-end determinism against a real migrated schema. HashMap
/// iteration order changes per instance (random hash seed), and any
/// order-sensitive f32 accumulation downstream diverges builds — live
/// measure 2026-07-19: 104 of 139 node positions differed between two
/// same-corpus builds until edge order was canonicalized.
///
/// Scope honesty: exact-tie fixtures cannot reproduce the float leak
/// (summing EQUAL values is order-independent; the live divergence needs
/// corpus-scale near-ties in low-order bits), so this test guards the
/// structural pipeline determinism and the schema contract. The
/// float-order fix itself is proven by the live double-build acceptance
/// check (two `build_content_graph` calls must return identical output).
#[test]
fn test_build_graph_is_deterministic_end_to_end() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    // Four groups of 6 on a cone: member = 0.894·axis + 0.447·ortho with
    // orthonormal axes/orthos, so EVERY within-group pair sits at exactly
    // cos 0.80 — massive ties (the ordering-leak trigger) while staying
    // below the 0.92 story-collapse rule and above the kNN floor.
    let names = ["alpha", "beta", "gamma", "delta"];
    let mut id = 0i64;
    for (gi, name) in names.iter().enumerate() {
        for m in 0..6usize {
            id += 1;
            let mut emb = vec![0.0f32; 32];
            emb[gi] = 0.894_427;
            emb[4 + gi * 6 + m] = 0.447_214;
            let source = if m % 2 == 0 { "crates_io" } else { "mastodon" };
            let title = format!("crates.io: {name}-crate-{m} v0.{m}.0");
            let blob = crate::db::embedding_to_blob(&emb);
            conn.execute(
                "INSERT INTO source_items (id, source_type, source_id, title, content,
                        content_hash, embedding, embedding_status, relevance_score, created_at)
                     VALUES (?1, ?2, ?3, ?4, '', ?5, ?6, 'complete', ?7, datetime('now'))",
                rusqlite::params![
                    id,
                    source,
                    format!("crate-{name}-{m}"),
                    title,
                    format!("hash{id}"),
                    blob,
                    0.9 - (gi as f64) * 0.01, // relevance ties within groups
                ],
            )
            .expect("insert item");
        }
    }

    let sig = |g: &ContentGraph| {
        let mut nodes: Vec<String> = g
            .nodes
            .iter()
            .map(|n| format!("{}:{:.2}:{:.2}:{:?}", n.id, n.x, n.y, n.cluster_id))
            .collect();
        nodes.sort();
        let mut edges: Vec<String> = g
            .edges
            .iter()
            .map(|e| format!("{}-{}:{:?}", e.source, e.target, e.edge_type))
            .collect();
        edges.sort();
        let clusters: Vec<String> = g
            .clusters
            .iter()
            .map(|c| format!("{}:{}:{:?}", c.id, c.label, c.node_ids))
            .collect();
        (nodes, edges, clusters)
    };

    let a = build_graph(&conn, 7, 150).expect("first build");
    let b = build_graph(&conn, 7, 150).expect("second build");
    assert_eq!(a.nodes.len(), 24, "fixture loads fully");
    assert_eq!(sig(&a), sig(&b), "two same-corpus builds must be identical");
}

/// Corpus parity (W4-5, Phase 95): the graph shows what the current
/// brain stands behind. Curated items load at any age in the window;
/// young not-yet-judged items load as an honest interim; judged-and-
/// rejected items and OLD never-judged items (stale-epoch scores — the
/// live war-news-at-0.94 class) never load.
#[test]
fn test_graph_corpus_selects_verdicts_first() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    let insert = |id: i64, title: &str, age: &str, verdict: Option<i64>, score: f64| {
        // Distinct (orthogonal) embeddings — near-dup story collapse
        // would otherwise fold the fixture behind one representative.
        let mut v = vec![0.0f32; 8];
        v[id as usize] = 1.0;
        let emb = crate::db::embedding_to_blob(&v);
        conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, title, content,
                    content_hash, embedding, embedding_status, relevance_score,
                    created_at, feed_relevant)
                 VALUES (?1, 'hackernews', ?2, ?3, '', ?4, ?5, 'complete', ?6,
                    datetime('now', ?7), ?8)",
            rusqlite::params![
                id,
                format!("hn{id}"),
                title,
                format!("h{id}"),
                emb,
                score,
                age,
                verdict
            ],
        )
        .expect("insert");
    };

    insert(1, "curated but old", "-5 days", Some(1), 0.6);
    insert(2, "curated and fresh", "-1 hours", Some(1), 0.7);
    insert(3, "young, not yet judged", "-1 hours", None, 0.95);
    insert(4, "old, never judged (stale epoch)", "-5 days", None, 0.94);
    insert(5, "judged and rejected", "-1 hours", Some(0), 0.93);

    let graph = build_graph(&conn, 7, 150).expect("build");
    let ids: Vec<i64> = graph.nodes.iter().map(|n| n.id).collect();

    assert!(ids.contains(&1), "old curated item belongs to the corpus");
    assert!(ids.contains(&2), "fresh curated item belongs to the corpus");
    assert!(
        ids.contains(&3),
        "young unjudged item loads as honest interim"
    );
    assert!(
        !ids.contains(&4),
        "old never-judged item (stale-epoch score) must not load"
    );
    assert!(!ids.contains(&5), "rejected item must not load");
    assert_eq!(graph.meta.curated_items, 2, "curated count is honest");
}

/// Inserts an item with a one-hot embedding on `dim` (orthogonal to all
/// other dims — no edges, no story collapse).
fn insert_singleton(
    conn: &rusqlite::Connection,
    id: i64,
    dims: usize,
    dim: usize,
    score: f64,
    age: &str,
    feed_relevant: Option<i64>,
) {
    let mut v = vec![0.0f32; dims];
    v[dim] = 1.0;
    let emb = crate::db::embedding_to_blob(&v);
    conn.execute(
        "INSERT INTO source_items (id, source_type, source_id, title, content,
                content_hash, embedding, embedding_status, relevance_score,
                created_at, feed_relevant)
             VALUES (?1, 'hackernews', ?2, ?3, '', ?4, ?5, 'complete', ?6,
                datetime('now', ?7), ?8)",
        rusqlite::params![
            id,
            format!("hn{id}"),
            format!("item {id}"),
            format!("h{id}"),
            emb,
            score,
            age,
            feed_relevant
        ],
    )
    .expect("insert");
}

/// Phase 96: an active snooze removes the item from the map; an expired
/// snooze does not (resurfacing needs no un-snooze step). This is the
/// end-to-end proof that Snooze stopped being a write-only button.
#[test]
fn test_snoozed_items_are_excluded_until_expiry() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    for (id, dim) in [(1i64, 0usize), (2, 1), (3, 2)] {
        insert_singleton(&conn, id, 8, dim, 0.9, "-1 hours", None);
    }
    conn.execute_batch(
        "INSERT INTO snoozed_items (source_item_id, snooze_until)
                 VALUES (2, datetime('now', '+3 days'));
             INSERT INTO snoozed_items (source_item_id, snooze_until)
                 VALUES (3, datetime('now', '-1 hours'));",
    )
    .expect("snooze rows");

    let graph = build_graph(&conn, 7, 150).expect("build");
    let ids: Vec<i64> = graph.nodes.iter().map(|n| n.id).collect();
    assert!(ids.contains(&1), "unsnoozed item shows");
    assert!(!ids.contains(&2), "actively snoozed item must not load");
    assert!(ids.contains(&3), "expired snooze resurfaces the item");
    assert_eq!(graph.meta.window_candidates, 2, "count matches selection");
}

/// Corpus parity at the visibility cap: a curated singleton can never
/// lose its map slot to not-yet-judged items, no matter how low it
/// scores (live 2026-07-19: both cap-hidden items were curated while
/// ~104 unjudged showed).
#[test]
fn test_curated_singletons_exempt_from_cap() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    let dims = SINGLETON_CAP + 4;
    // 42 high-scoring unjudged singletons (two beyond the cap)…
    for i in 0..(SINGLETON_CAP + 2) {
        insert_singleton(
            &conn,
            i as i64 + 1,
            dims,
            i,
            0.9 - i as f64 * 0.001,
            "-1 hours",
            None,
        );
    }
    // …and one bottom-scored curated item, older than the young horizon.
    let curated_id = (SINGLETON_CAP + 3) as i64;
    insert_singleton(
        &conn,
        curated_id,
        dims,
        SINGLETON_CAP + 2,
        0.1,
        "-5 days",
        Some(1),
    );

    let graph = build_graph(&conn, 7, 150).expect("build");
    let ids: Vec<i64> = graph.nodes.iter().map(|n| n.id).collect();
    assert!(
        ids.contains(&curated_id),
        "curated singleton must be visible regardless of the cap"
    );
    assert_eq!(
        graph.meta.hidden_items, 2,
        "only non-curated overflow is hidden"
    );
    assert_eq!(graph.nodes.len(), SINGLETON_CAP + 1, "cap + curated");
}

/// The node budget applies to STORIES (post-collapse), keeps the
/// highest-relevance ones, and the meta reports honest coverage.
#[test]
fn test_node_budget_truncates_post_collapse_with_honest_meta() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    for i in 0..8usize {
        insert_singleton(
            &conn,
            i as i64 + 1,
            8,
            i,
            0.9 - i as f64 * 0.05,
            "-1 hours",
            None,
        );
    }

    let graph = build_graph(&conn, 7, 5).expect("build");
    assert_eq!(graph.nodes.len(), 5, "budget respected post-collapse");
    let ids: Vec<i64> = graph.nodes.iter().map(|n| n.id).collect();
    for id in 1..=5i64 {
        assert!(ids.contains(&id), "top-relevance item {id} kept");
    }
    assert_eq!(graph.meta.window_candidates, 8, "full window counted");
    assert_eq!(graph.meta.hidden_items, 3, "truncated items disclosed");
}

/// P2.14: the curation ramp counts ITEMS, not stories — a story with one
/// curated member and one unjudged near-duplicate contributes exactly 1.
#[test]
fn test_curated_count_is_item_level() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    // Same one-hot embedding → the two items collapse into one story.
    insert_singleton(&conn, 1, 8, 0, 0.9, "-1 hours", Some(1));
    insert_singleton(&conn, 2, 8, 0, 0.8, "-1 hours", None);
    // Unrelated curated singleton.
    insert_singleton(&conn, 3, 8, 1, 0.7, "-1 hours", Some(1));

    let graph = build_graph(&conn, 7, 150).expect("build");
    assert_eq!(graph.nodes.len(), 2, "two items collapsed + one singleton");
    assert_eq!(
        graph.meta.curated_items, 2,
        "1 curated member in the story + 1 curated singleton; the unjudged near-dup must not launder in"
    );
}

/// P2.12: the top security items of the window hold reserved slots — a
/// relevance-first cap can no longer render a 130-advisory week as a map
/// with zero security nodes.
#[test]
fn test_security_quota_reserves_map_slots() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    let dims = 64;
    // 45 high-scoring unjudged discussion singletons…
    for i in 0..45usize {
        insert_singleton(
            &conn,
            i as i64 + 1,
            dims,
            i,
            0.9 - i as f64 * 0.001,
            "-1 hours",
            None,
        );
    }
    // …and three bottom-scored CVE advisories.
    for (j, id) in [(0usize, 100i64), (1, 101), (2, 102)] {
        let mut v = vec![0.0f32; dims];
        v[50 + j] = 1.0;
        let emb = crate::db::embedding_to_blob(&v);
        conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, title, content,
                content_hash, embedding, embedding_status, relevance_score, created_at)
             VALUES (?1, 'cve', ?2, ?3, '', ?4, ?5, 'complete', ?6, datetime('now', '-1 hours'))",
            rusqlite::params![
                id,
                format!("cve{id}"),
                format!("[CVE-2026-{id}] fixture advisory {id}"),
                format!("ch{id}"),
                emb,
                0.05 + j as f64 * 0.01
            ],
        )
        .expect("insert cve");
    }

    // Budget of 30 nodes: relevance-first would fill every slot with
    // discussion items; the quota must keep the advisories on the map.
    let graph = build_graph(&conn, 7, 30).expect("build");
    let ids: Vec<i64> = graph.nodes.iter().map(|n| n.id).collect();
    for id in [100i64, 101, 102] {
        assert!(
            ids.contains(&id),
            "reserved security item {id} must survive the cap"
        );
    }
    assert!(
        graph.nodes.iter().any(|n| n.category == "security"),
        "security category present on the map"
    );
}

/// P2.15: the window toggle is inert until curated verdicts age past the
/// shortest window — meta says so, and the UI hides the dead control.
#[test]
fn test_windows_differ_tracks_verdict_age() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    insert_singleton(&conn, 1, 8, 0, 0.9, "-1 hours", Some(1));
    let young_only = build_graph(&conn, 7, 150).expect("build");
    assert!(
        !young_only.meta.windows_differ,
        "all verdicts young: windows identical, toggle inert"
    );

    insert_singleton(&conn, 2, 8, 1, 0.8, "-10 days", Some(1));
    let with_old = build_graph(&conn, 7, 150).expect("build");
    assert!(
        with_old.meta.windows_differ,
        "a verdict older than 7d makes the windows genuinely different"
    );
}

/// P2.11: greedy anchor matching — best Jaccard wins, each anchor used once,
/// sub-floor overlap never matches.
#[test]
fn test_match_anchors_greedy_and_floored() {
    use std::collections::HashSet;

    let clusters = vec![
        ("cluster_a".to_string(), HashSet::from([1i64, 2, 3, 4])),
        ("cluster_b".to_string(), HashSet::from([5i64, 6, 7, 8])),
        ("cluster_c".to_string(), HashSet::from([100i64, 101])),
    ];
    let anchors = vec![
        super::anchors::StoredAnchor {
            x: 10.0,
            y: 10.0,
            member_ids: HashSet::from([1, 2, 3, 9]),
        },
        super::anchors::StoredAnchor {
            x: 20.0,
            y: 20.0,
            member_ids: HashSet::from([5, 6, 7, 8]),
        },
        super::anchors::StoredAnchor {
            x: 30.0,
            y: 30.0,
            member_ids: HashSet::from([200, 201]),
        },
    ];

    let seeds = super::anchors::match_anchors(&clusters, &anchors);
    assert_eq!(
        seeds.get("cluster_a"),
        Some(&(10.0, 10.0)),
        "overlap 3/5 matches"
    );
    assert_eq!(seeds.get("cluster_b"), Some(&(20.0, 20.0)), "exact match");
    assert!(
        !seeds.contains_key("cluster_c"),
        "zero overlap never matches"
    );
}

/// P2.11 end-to-end: stored anchors invert the two clusters' default
/// left-right arrangement, and persisting the built graph round-trips.
#[test]
fn test_layout_anchors_seed_and_roundtrip() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    // Two tight 4-item themes on a cone (within-group cosine 0.80 — clusters,
    // not stories), mirroring the determinism fixture's construction.
    let mut id = 0i64;
    for gi in 0..2usize {
        for m in 0..4usize {
            id += 1;
            let mut emb = vec![0.0f32; 16];
            emb[gi] = 0.894_427;
            emb[2 + gi * 4 + m] = 0.447_214;
            let blob = crate::db::embedding_to_blob(&emb);
            conn.execute(
                "INSERT INTO source_items (id, source_type, source_id, title, content,
                    content_hash, embedding, embedding_status, relevance_score, created_at)
                 VALUES (?1, 'hackernews', ?2, ?3, '', ?4, ?5, 'complete', ?6, datetime('now', '-1 hours'))",
                rusqlite::params![
                    id,
                    format!("hn{id}"),
                    format!("group{gi} item {m}"),
                    format!("h{id}"),
                    blob,
                    0.9 - gi as f64 * 0.01
                ],
            )
            .expect("insert");
        }
    }

    // Baseline (no anchors): record the natural x-order of the two clusters.
    let baseline = build_graph(&conn, 7, 150).expect("baseline build");
    assert_eq!(baseline.clusters.len(), 2, "fixture forms two clusters");
    let centroid_x = |g: &ContentGraph, members: &[i64]| -> f32 {
        g.clusters
            .iter()
            .find(|c| {
                members.iter().all(|m| {
                    g.nodes
                        .iter()
                        .any(|n| n.member_ids.contains(m) && c.node_ids.contains(&n.id))
                })
            })
            .map(|c| c.centroid_x)
            .expect("cluster for members")
    };
    let base_a = centroid_x(&baseline, &[1, 2, 3, 4]);
    let base_b = centroid_x(&baseline, &[5, 6, 7, 8]);

    // Anchors deliberately INVERT that arrangement with a wide margin.
    let (ax, bx) = if base_a <= base_b {
        (3000.0, -2000.0)
    } else {
        (-2000.0, 3000.0)
    };
    conn.execute(
        "INSERT INTO graph_layout_anchors (window_days, cluster_key, x, y, member_ids)
         VALUES (7, 'k_a', ?1, 500.0, '[1,2,3,4]'), (7, 'k_b', ?2, 500.0, '[5,6,7,8]')",
        rusqlite::params![ax, bx],
    )
    .expect("insert anchors");

    let anchored = build_graph(&conn, 7, 150).expect("anchored build");
    let anc_a = centroid_x(&anchored, &[1, 2, 3, 4]);
    let anc_b = centroid_x(&anchored, &[5, 6, 7, 8]);
    assert_eq!(
        (anc_a > anc_b),
        (ax > bx),
        "anchored arrangement must follow the anchors, not the spiral (a={anc_a}, b={anc_b})"
    );
    assert_ne!(
        (base_a > base_b),
        (anc_a > anc_b),
        "anchors genuinely inverted the baseline arrangement"
    );

    // Round-trip: persisting the anchored build and rebuilding keeps the
    // arrangement (approximate stability, not frozen geometry).
    super::anchors::persist_layout_anchors(&conn, 7, &anchored);
    let rebuilt = build_graph(&conn, 7, 150).expect("rebuild");
    let re_a = centroid_x(&rebuilt, &[1, 2, 3, 4]);
    let re_b = centroid_x(&rebuilt, &[5, 6, 7, 8]);
    assert_eq!(
        (re_a > re_b),
        (anc_a > anc_b),
        "arrangement survives persist and rebuild"
    );
}

/// P2.17: cross-process determinism. Same-process double builds share one
/// HashMap seed; every canonical-ordering fix claims independence from it,
/// and this is the test that would catch a regression: the same on-disk
/// corpus built in two separate processes (fresh random hash seeds) must
/// produce identical output. Child mode is selected via env var.
#[test]
fn test_build_graph_deterministic_across_processes() {
    // ---- child: build against the given DB and print a signature ----
    if let Ok(db_path) = std::env::var("FOURDA_GRAPH_DETERMINISM_DB") {
        let conn = rusqlite::Connection::open(&db_path).expect("child open");
        let graph = build_graph(&conn, 7, 150).expect("child build");
        let mut sig = String::new();
        for n in &graph.nodes {
            sig.push_str(&format!(
                "{}:{:.4}:{:.4}:{:?};",
                n.id, n.x, n.y, n.cluster_id
            ));
        }
        for e in &graph.edges {
            sig.push_str(&format!("{}-{}:{:?};", e.source, e.target, e.edge_type));
        }
        for c in &graph.clusters {
            sig.push_str(&format!("{}:{}:{:?};", c.id, c.label, c.node_ids));
        }
        println!("GRAPH_SIG_LEN:{} GRAPH_SIG:{}", sig.len(), sig);
        return;
    }

    // ---- parent: corpus-scale near-tie fixture on disk ----
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("determinism.db");
    {
        let conn = rusqlite::Connection::open(&db_path).expect("create db");
        conn.execute_batch(
            "CREATE TABLE source_items (
                 id INTEGER PRIMARY KEY, source_type TEXT NOT NULL, source_id TEXT,
                 title TEXT NOT NULL, url TEXT, content TEXT, content_hash TEXT,
                 embedding BLOB, embedding_status TEXT, relevance_score REAL,
                 created_at TEXT NOT NULL, signal_type TEXT, signal_priority TEXT,
                 feed_relevant INTEGER, feed_verdict_at TEXT
             );
             CREATE TABLE source_item_dependencies (
                 id INTEGER PRIMARY KEY, source_item_id INTEGER NOT NULL,
                 package_name TEXT NOT NULL, confidence REAL NOT NULL
             );
             CREATE TABLE snoozed_items (
                 source_item_id INTEGER PRIMARY KEY, snooze_until TEXT NOT NULL,
                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE scoring_explanations (
                 source_item_id INTEGER PRIMARY KEY, pipeline_version INTEGER NOT NULL,
                 breakdown TEXT NOT NULL, scored_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE graph_layout_anchors (
                 window_days INTEGER NOT NULL, cluster_key TEXT NOT NULL,
                 x REAL NOT NULL, y REAL NOT NULL, member_ids TEXT NOT NULL,
                 updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                 PRIMARY KEY (window_days, cluster_key)
             );
             CREATE TABLE user_dependencies (
                 id INTEGER PRIMARY KEY, package_name TEXT NOT NULL
             );
             CREATE TABLE project_dependencies (
                 id INTEGER PRIMARY KEY, package_name TEXT NOT NULL
             );",
        )
        .expect("schema");

        // Four 6-member cone groups: every within-group pair at exactly
        // cos 0.80 — massive float ties, the ordering-leak trigger.
        let names = ["alpha", "beta", "gamma", "delta"];
        let mut id = 0i64;
        for (gi, name) in names.iter().enumerate() {
            for m in 0..6usize {
                id += 1;
                let mut emb = vec![0.0f32; 32];
                emb[gi] = 0.894_427;
                emb[4 + gi * 6 + m] = 0.447_214;
                let blob = crate::db::embedding_to_blob(&emb);
                conn.execute(
                    "INSERT INTO source_items (id, source_type, source_id, title, content,
                        content_hash, embedding, embedding_status, relevance_score, created_at)
                     VALUES (?1, ?2, ?3, ?4, '', ?5, ?6, 'complete', ?7, datetime('now', '-1 hours'))",
                    rusqlite::params![
                        id,
                        if m % 2 == 0 { "crates_io" } else { "mastodon" },
                        format!("crate-{name}-{m}"),
                        format!("crates.io: {name}-crate-{m} v0.{m}.0"),
                        format!("hash{id}"),
                        blob,
                        0.9 - (gi as f64) * 0.01,
                    ],
                )
                .expect("insert item");
            }
        }
    }

    let exe = std::env::current_exe().expect("test exe");
    let run_child = || -> String {
        let out = std::process::Command::new(&exe)
            .args([
                "--exact",
                "content_graph::tests::test_build_graph_deterministic_across_processes",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(
                "FOURDA_GRAPH_DETERMINISM_DB",
                db_path.to_string_lossy().to_string(),
            )
            .output()
            .expect("spawn child");
        assert!(
            out.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout
            .lines()
            .find(|l| l.contains("GRAPH_SIG_LEN:"))
            .expect("child printed signature")
            .to_string()
    };

    let first = run_child();
    let second = run_child();
    assert!(first.contains("GRAPH_SIG:"), "signature captured");
    assert_eq!(
        first, second,
        "two fresh processes must build identical graphs"
    );
}

// ── Signal-quality remediation (2026-07-21 audit) ───────────────────────────

#[test]
fn test_story_representative_prefers_modal_title() {
    // Live shape: a 10-member story where 7 members carried the clean title
    // was fronted by a hashtag-scarred copy that happened to score highest.
    let shared = vec![1.0f32, 0.0, 0.0, 0.0];
    let items = vec![
        raw(
            1,
            "Claude Code uses Bun written in Rust now rust",
            "mastodon",
            0.99,
            shared.clone(),
        ),
        raw(
            2,
            "Claude Code uses Bun written in Rust now",
            "hackernews",
            0.90,
            shared.clone(),
        ),
        raw(
            3,
            "Claude Code uses Bun written in Rust now",
            "reddit",
            0.89,
            shared.clone(),
        ),
    ];
    let stories = story::collapse_stories(items);
    assert_eq!(stories.len(), 1, "near-duplicates collapse into one story");
    let s = &stories[0];
    assert_eq!(s.member_count, 3);
    assert_eq!(
        s.item.title, "Claude Code uses Bun written in Rust now",
        "the modal title fronts the story, not the highest-scored mangle"
    );
    assert_eq!(
        s.item.id, 2,
        "representative is the earliest load-order member with the modal title"
    );
    assert!(
        (s.item.relevance_score - 0.99).abs() < 1e-6,
        "story relevance stays the member max"
    );
}

#[test]
fn test_cluster_label_terms_all_need_two_hits() {
    // "rust · embedded · first" (live): the top term cleared the coverage
    // floor and single-title riders padded the label. Every term must hit
    // at least two member titles.
    let e = |d: usize| {
        let mut v = vec![0.0f32; 4];
        v[d] = 1.0;
        v
    };
    let items = vec![
        raw(1, "Rust embedded runtime ships", "hackernews", 0.9, e(0)),
        raw(2, "Rust embedded kernel patch", "reddit", 0.8, e(1)),
        raw(3, "Rust embedded board support", "lemmy", 0.7, e(2)),
        raw(4, "Rust first steps guide", "devto", 0.6, e(3)),
    ];
    let mut clusters = vec![GraphCluster {
        id: "cluster_1".to_string(),
        label: String::new(),
        node_ids: vec![1, 2, 3, 4],
        source_count: 4,
        coherence: 0.0,
        centroid_x: 0.0,
        centroid_y: 0.0,
    }];
    labels::assign_cluster_labels(&items, &mut clusters);
    let label = &clusters[0].label;
    assert!(label.contains("rust"), "shared term labels: {label}");
    assert!(label.contains("embedded"), "3-hit term labels: {label}");
    assert!(
        !label.contains("first") && !label.contains("steps") && !label.contains("guide"),
        "single-title riders must not decorate the label: {label}"
    );
}

/// Inserts an unjudged/curated singleton with an arbitrary source_type
/// (one-hot embedding — orthogonal, so no edges and no story collapse).
fn insert_singleton_src(
    conn: &rusqlite::Connection,
    id: i64,
    dims: usize,
    dim: usize,
    source_type: &str,
    score: f64,
    feed_relevant: Option<i64>,
) {
    let mut v = vec![0.0f32; dims];
    v[dim] = 1.0;
    let emb = crate::db::embedding_to_blob(&v);
    conn.execute(
        "INSERT INTO source_items (id, source_type, source_id, title, content,
                content_hash, embedding, embedding_status, relevance_score,
                created_at, feed_relevant)
             VALUES (?1, ?2, ?3, ?4, '', ?5, ?6, 'complete', ?7,
                datetime('now', '-1 hours'), ?8)",
        rusqlite::params![
            id,
            source_type,
            format!("s{id}"),
            format!("item {id}"),
            format!("h{id}"),
            emb,
            score,
            feed_relevant
        ],
    )
    .expect("insert");
}

#[test]
fn test_source_cap_limits_unjudged_flood_but_exempts_curated() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    // 10 unjudged crates releases outscore everything (the live firehose),
    // plus 4 each of three other sources, plus 2 CURATED crates at low score.
    let dims = 24;
    let mut next = 0usize;
    let mut id = 0i64;
    let mut add = |conn: &rusqlite::Connection, src: &str, score: f64, verdict: Option<i64>| {
        id += 1;
        next += 1;
        insert_singleton_src(conn, id, dims, next - 1, src, score, verdict);
        id
    };
    let mut crates_unjudged = Vec::new();
    for _ in 0..10 {
        crates_unjudged.push(add(&conn, "crates_io", 0.9, None));
    }
    for _ in 0..4 {
        add(&conn, "hackernews", 0.5, None);
    }
    for _ in 0..4 {
        add(&conn, "reddit", 0.45, None);
    }
    for _ in 0..4 {
        add(&conn, "devto", 0.42, None);
    }
    let curated_a = add(&conn, "crates_io", 0.30, Some(1));
    let curated_b = add(&conn, "crates_io", 0.29, Some(1));

    // max_nodes 12 → per-source cap = ceil(12 * 0.25) = 3 unjudged slots.
    let graph = build_graph(&conn, 7, 12).expect("build");
    let ids: Vec<i64> = graph.nodes.iter().map(|n| n.id).collect();

    let unjudged_crates_shown = crates_unjudged.iter().filter(|i| ids.contains(i)).count();
    assert_eq!(
        unjudged_crates_shown, 3,
        "unjudged flood capped at 25% of the budget: {ids:?}"
    );
    assert!(
        ids.contains(&curated_a) && ids.contains(&curated_b),
        "curated stories are exempt from the source cap (corpus parity)"
    );
    assert!(
        graph.meta.hidden_items >= 7,
        "capped stories count into hidden_items (got {})",
        graph.meta.hidden_items
    );
}

// ── Graph truth (adversarial audit 2026-10-02, section 3) ──────────────────

/// Labels one cluster made of `titles` (all members of one cluster, among
/// `others` that form no cluster but share the item universe — c-TF-IDF and
/// boilerplate are corpus-relative).
fn label_for(titles: &[(&str, &str)], others: &[(&str, &str)]) -> String {
    let mut items: Vec<RawItem> = Vec::new();
    let dims = titles.len() + others.len() + 1;
    for (i, (title, src)) in titles.iter().chain(others.iter()).enumerate() {
        let mut v = vec![0.0f32; dims];
        v[0] = 1.0;
        v[i + 1] = 0.3;
        items.push(raw(i as i64 + 1, title, src, 0.5, v));
    }
    let mut clusters = vec![GraphCluster {
        id: "cluster_1".to_string(),
        label: String::new(),
        node_ids: (1..=titles.len() as i64).collect(),
        source_count: 1,
        coherence: 0.0,
        centroid_x: 0.0,
        centroid_y: 0.0,
    }];
    labels::assign_cluster_labels(&items, &mut clusters);
    clusters.remove(0).label
}

fn assert_never_assorted(label: &str) {
    assert!(!label.is_empty(), "label must not be empty");
    assert!(
        !label.contains("assorted"),
        "'assorted' names nothing: {label}"
    );
}

#[test]
fn test_live_rust_concurrency_trio_is_labelled_by_its_shared_term() {
    // Live 2026-10-02: "related items · assorted". "rust" is in two of three
    // titles, but the old code checked coverage only on the raw c-TF-IDF
    // winner (a one-title word) and fell through to the digest.
    let label = label_for(
        &[
            (
                "Compio: A thread-per-core async Rust runtime with IOCP/io_uring/polling",
                "hackernews",
            ),
            (
                "Idiomatic Concurrency in Rust: Stop Writing Accidental Microservices",
                "devto",
            ),
            ("Mixing rayon and tokio for fun and (hair) loss", "lobsters"),
        ],
        &[],
    );
    assert_never_assorted(&label);
    assert_eq!(label, "rust");
}

#[test]
fn test_live_sql_injection_pair_is_labelled_sql() {
    let label = label_for(
        &[
            (
                "Everyone Greps for SQL Injection. Nobody Greps for the Other Eight.",
                "devto",
            ),
            (
                "Compile Time Prevention of SQL-Injections in Rust",
                "lobsters",
            ),
        ],
        &[],
    );
    assert_never_assorted(&label);
    assert!(
        label.split(" · ").any(|t| t == "sql"),
        "the shared topic is sql: {label}"
    );
}

#[test]
fn test_live_registry_pair_is_named_as_releases() {
    // Live: "npm · assorted" for @react-pdf/renderer + @upstash/ratelimit.
    let label = label_for(
        &[
            ("npm: @react-pdf/renderer v4.9.0", "npm_registry"),
            ("npm: @upstash/ratelimit v2.2.0", "npm_registry"),
        ],
        &[],
    );
    assert_eq!(label, "npm releases");
}

#[test]
fn test_no_shared_term_falls_back_to_representative_title() {
    // Live: "devto · assorted" and "hacker news · assorted" — pairs whose
    // titles share no content word. The label is the typical member's title,
    // shortened: concrete words the user can check against the dots.
    let titles = [
        (
            "You Are Lying to Your Compiler. Use \"satisfies\" Instead of \"as\".",
            "devto",
        ),
        (
            "TypeScript `using` in Real Codebases: Database Connections, File Handles, and Async Cleanup",
            "devto",
        ),
    ];
    let label = label_for(&titles, &[]);
    assert_never_assorted(&label);
    assert!(
        label.chars().count() <= 37,
        "title labels stay header-sized: {label}"
    );
    let head = label.trim_end_matches('…');
    assert!(
        titles.iter().any(|(t, _)| t.starts_with(head)),
        "fallback label must be a member's own words: {label}"
    );

    let label = label_for(
        &[
            (
                "Replace Your OpenAI, Anthropic or Google API Call with Middleware",
                "hackernews",
            ),
            ("Programmatic Tool Calling", "hackernews"),
        ],
        &[],
    );
    assert_never_assorted(&label);
}

#[test]
fn test_shared_dependency_names_a_cluster_without_a_shared_term() {
    let mut items = vec![
        raw(
            1,
            "Webhook retries that silently drop events",
            "devto",
            0.5,
            vec![1.0, 0.1],
        ),
        raw(
            2,
            "Cancelled customers who keep access",
            "devto",
            0.5,
            vec![1.0, 0.2],
        ),
    ];
    items[0].matched_package = Some("stripe".into());
    items[1].matched_package = Some("Stripe".into());
    let mut clusters = vec![GraphCluster {
        id: "cluster_1".to_string(),
        label: String::new(),
        node_ids: vec![1, 2],
        source_count: 1,
        coherence: 0.0,
        centroid_x: 0.0,
        centroid_y: 0.0,
    }];
    labels::assign_cluster_labels(&items, &mut clusters);
    assert_eq!(clusters[0].label, "stripe");
}

#[test]
fn test_idiom_filler_never_labels() {
    // Live 2026-10-03: an authorization cluster labelled "pass".
    let kw = labels::extract_title_keywords("An async job is not a free pass on authorization");
    assert!(!kw.contains(&"pass".to_string()), "{kw:?}");
    assert!(!kw.contains(&"free".to_string()), "{kw:?}");
    assert!(kw.contains(&"authorization".to_string()), "{kw:?}");
}

#[test]
fn test_generic_words_never_label_but_nextjs_does() {
    // Live: "tests · never · next" and "concurrent · building".
    let kw = labels::extract_title_keywords(
        "Our most valuable tests never break: building the next thing",
    );
    for generic in ["never", "next", "building"] {
        assert!(
            !kw.contains(&generic.to_string()),
            "{generic} leaked: {kw:?}"
        );
    }
    assert!(kw.contains(&"tests".to_string()));

    let kw = labels::extract_title_keywords("Next.js's after() Function Is Easy to Misuse");
    assert!(kw.contains(&"nextjs".to_string()), "{kw:?}");
    let kw = labels::extract_title_keywords("React.js ~New feature in React 19.3~");
    assert!(kw.contains(&"reactjs".to_string()) && kw.contains(&"react".to_string()));

    let label = label_for(
        &[
            (
                "Calling revalidateTag on an Already-Uncached Fetch Does Nothing, and Next.js Won't Tell You",
                "devto",
            ),
            (
                "That JSON error quotes what the server actually sent - and V8 cuts it at exactly",
                "devto",
            ),
            ("Next.js's after() Function Is Easy to Misuse in Both Directions", "devto"),
        ],
        &[],
    );
    assert_eq!(label, "next.js", "shown in its usual spelling");

    let label = label_for(
        &[
            (
                "Concurrent Writes in Practice: Building High-Throughput Apps with Turso's MVCC",
                "rss",
            ),
            (
                "maillon: a concurrent intrusive list for building faster synchronization primitives",
                "reddit",
            ),
        ],
        &[],
    );
    assert_eq!(label, "concurrent");
}

#[test]
fn test_label_does_not_repeat_a_compound_and_its_part() {
    let label = label_for(
        &[
            ("npm: @tauri-apps/api v2.12.0", "hackernews"),
            ("@tauri-apps/plugin-opener v2.7.0", "hackernews"),
            ("tauri apps everywhere", "hackernews"),
        ],
        &[],
    );
    let terms: Vec<&str> = label.split(" · ").collect();
    let has_compound = terms.contains(&"tauri-apps");
    let has_part = terms.contains(&"tauri") || terms.contains(&"apps");
    assert!(!(has_compound && has_part), "duplicate naming: {label}");
}

#[test]
fn test_live_mixed_api_cluster_leads_with_its_covering_term() {
    // Live 2026-10-02 (11 members): the most distinctive 3-title term
    // outranked the 5-title "api", failed the coverage floor, and the
    // cluster fell back to a registry title ("ai v7.0.126").
    let label = label_for(
        &[
            (
                "AI Gateway now supports TypeSafe clients and an HTTP API for Jev",
                "rss",
            ),
            ("npm: @tauri-apps/api v2.12.0", "npm_registry"),
            ("Trying GPT-6 Sol with AI SDK Evaluation API", "hackernews"),
            (
                "Jeb: Turn any OpenAI API into a decision model",
                "hackernews",
            ),
            ("npm: openai v7.25.0", "npm_registry"),
            ("npm: @tauri-apps/plugin-opener v2.7.0", "npm_registry"),
            ("npm: @tauri-apps/plugin-updater v2.13.1", "npm_registry"),
            ("What Is OpenAI's Decisions API?", "hackernews"),
            ("npm: @ai-sdk/openai v4.0.83", "npm_registry"),
            ("AI Gateway adds Browserbase Search and Fetch tools", "rss"),
            ("npm: ai v7.0.126", "npm_registry"),
        ],
        &[],
    );
    assert_never_assorted(&label);
    assert!(
        label.split(" · ").next() == Some("api") || label.split(" · ").next() == Some("openai"),
        "a term covering >= 30% of members leads: {label}"
    );
    assert!(!label.contains("v7.0.126"), "{label}");
}

#[test]
fn test_label_does_not_repeat_a_word_and_its_plural() {
    let label = label_for(
        &[
            ("Agent memory for long tasks", "devto"),
            ("Agents that verify their own work", "devto"),
            ("Agent sandboxes in practice", "hackernews"),
            ("Why agents fail in production", "devto"),
        ],
        &[],
    );
    let terms: Vec<&str> = label.split(" · ").collect();
    assert!(
        !(terms.contains(&"agent") && terms.contains(&"agents")),
        "plural duplicate: {label}"
    );
}

#[test]
fn test_short_title_strips_prefixes_and_cuts_on_words() {
    assert_eq!(
        labels::short_title("crates.io: tauri-plugin-opener v2.7.0"),
        "tauri-plugin-opener v2.7.0"
    );
    assert_eq!(
        labels::short_title("[GHSA-xxxx-yyyy] Axios: header injection"),
        "Axios: header injection"
    );
    let cut = labels::short_title(
        "Self Healing Execution Graphs - How to Catch Cascading Agent Failures Before They Spread",
    );
    assert!(cut.ends_with('…') && cut.chars().count() <= 29, "{cut}");
    // Live 2026-10-04: "PHILBIN: THE SAFEST (AND FASTEST)…" — a name heads it.
    assert_eq!(
        labels::short_title("Philbin: The safest (and fastest) AEGIS library"),
        "Philbin"
    );
}

/// Live 2026-10-04: three clusters on one map were all labelled "RUST".
/// Every cluster in a duplicate group gains its own next distinctive term,
/// and no two clusters ever share a label.
#[test]
fn test_duplicate_cluster_labels_are_disambiguated() {
    let groups: [&[&str]; 3] = [
        &[
            "Deser: Rethinking Rust Serialization",
            "Gluon: an embeddable language written in Rust",
        ],
        &[
            "Google Rewrites Critical C Dependencies to Rust Using Fuzzing",
            "Porting a 1996 Pascal Calculator to Rust",
        ],
        &[
            "Is trait-based dependency injection possible in Rust?",
            "Rust's derive often implies inline",
        ],
    ];
    let mut items: Vec<RawItem> = Vec::new();
    let mut clusters: Vec<GraphCluster> = Vec::new();
    let mut id = 0i64;
    for (gi, titles) in groups.iter().enumerate() {
        let mut node_ids = Vec::new();
        for t in titles.iter() {
            id += 1;
            let mut v = vec![0.0f32; 8];
            v[gi] = 1.0;
            v[3 + (id as usize % 5)] = 0.2;
            items.push(raw(id, t, "hackernews", 0.5, v));
            node_ids.push(id);
        }
        clusters.push(GraphCluster {
            id: format!("cluster_{gi}"),
            label: String::new(),
            node_ids,
            source_count: 1,
            coherence: 0.0,
            centroid_x: 0.0,
            centroid_y: 0.0,
        });
    }
    // The rest of the window (in no cluster): keeps "rust" from reading as
    // hackernews boilerplate (>=80% of one source's titles).
    for t in [
        "Postgres row-level security, measured",
        "SQLite backup API",
        "UUID v7 index performance",
        "WSL containers are generally available",
    ] {
        id += 1;
        items.push(raw(id, t, "hackernews", 0.5, vec![0.0; 8]));
    }
    labels::assign_cluster_labels(&items, &mut clusters);
    let labels: Vec<String> = clusters.iter().map(|c| c.label.to_lowercase()).collect();
    let unique: HashSet<&String> = labels.iter().collect();
    assert_eq!(unique.len(), labels.len(), "labels collide: {labels:?}");
    for l in &labels {
        assert!(
            l.starts_with("rust · "),
            "the shared theme leads, the distinction follows: {labels:?}"
        );
    }
    // Deterministic across runs.
    let mut again = clusters.clone();
    for c in &mut again {
        c.label.clear();
    }
    labels::assign_cluster_labels(&items, &mut again);
    assert_eq!(
        clusters.iter().map(|c| &c.label).collect::<Vec<_>>(),
        again.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
}

/// "Touches your stack" = the scoring pipeline's persisted grounding verdict
/// (Signal's predicate), not "any dependency link at any confidence".
#[test]
fn test_affects_you_follows_the_persisted_grounding_verdict() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();
    for (id, dim) in [(1i64, 0usize), (2, 1), (3, 2), (4, 3), (5, 4)] {
        insert_singleton(
            &conn,
            id,
            8,
            dim,
            0.9 - id as f64 * 0.01,
            "-1 hours",
            Some(1),
        );
    }
    // Every item carries a dep link — the OLD rule lit all five.
    for (id, pkg, conf) in [
        (1i64, "openai", 0.5),
        (2, "tokio", 0.95),
        (3, "hono", 0.9),
        (4, "react", 0.5),
        (5, "axum", 0.95),
    ] {
        conn.execute(
            "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem,
                 match_type, confidence) VALUES (?1, ?2, 'npm', 'title_heuristic', ?3)",
            rusqlite::params![id, pkg, conf],
        )
        .expect("dep link");
    }
    let breakdown = |id: i64, json: &str| {
        conn.execute(
            "INSERT INTO scoring_explanations (source_item_id, pipeline_version, breakdown)
             VALUES (?1, 1, ?2)",
            rusqlite::params![id, json],
        )
        .expect("breakdown");
    };
    // 2026-10-04 dependency-event claim: an editorial item is "your stack"
    // only when grounded AND about an event happening to the dependency.
    insert_singleton(&conn, 6, 8, 5, 0.83, "-1 hours", Some(1));
    insert_singleton(&conn, 7, 8, 6, 0.82, "-1 hours", Some(1));
    insert_singleton_src(&conn, 8, 8, 7, "crates_io", 0.81, Some(1));
    breakdown(1, r#"{"breakdown":{"strongly_grounded":false}}"#);
    breakdown(
        2,
        r#"{"breakdown":{"strongly_grounded":true,"dependency_event":true}}"#,
    );
    // A grounded tutorial: names the package, nothing happens to it.
    breakdown(
        6,
        r#"{"breakdown":{"strongly_grounded":true,"dependency_event":false}}"#,
    );
    // Stored before the claim existed: editorial → no claim; registry →
    // the old grounding-only rule.
    breakdown(7, r#"{"breakdown":{"strongly_grounded":true}}"#);
    breakdown(8, r#"{"breakdown":{"strongly_grounded":true}}"#);
    // Grounded by name but the installed version is confirmed past the fix.
    breakdown(
        3,
        r#"{"breakdown":{"strongly_grounded":true,"is_version_affected":false}}"#,
    );
    // Version-confirmed affected advisory.
    breakdown(
        4,
        r#"{"breakdown":{"strongly_grounded":false,"is_version_affected":true}}"#,
    );
    // 5: no breakdown persisted → no verdict, no claim.

    let graph = build_graph(&conn, 7, 150).expect("build");
    let affects = |id: i64| {
        graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.affects_you)
            .expect("node on map")
    };
    assert!(
        !affects(1),
        "a link the pipeline judged ungrounded must not glow"
    );
    assert!(affects(2), "strongly grounded → touches your stack");
    assert!(
        !affects(3),
        "confirmed not-affected never occupies the stack pool"
    );
    assert!(
        affects(4),
        "version-confirmed affected advisory grounds the item"
    );
    assert!(!affects(5), "no persisted verdict → no claim");
    assert!(
        !affects(6),
        "a grounded tutorial with no dependency event does not touch your stack"
    );
    assert!(
        !affects(7),
        "an editorial breakdown stored before the event claim makes no claim"
    );
    assert!(
        affects(8),
        "a grounded registry row stored before the event claim keeps the old rule"
    );
}

/// The registry list spelled into `GROUNDED_SQL` must be every registry
/// source plus the advisory sources — the same classes the scorer treats as
/// registry rows (`dep_linker::REGISTRY_SOURCE_TYPES`, osv/cve).
#[test]
fn test_grounded_sql_registry_list_matches_the_scorer() {
    let sql = super::loading::GROUNDED_SQL;
    for st in crate::dep_linker::REGISTRY_SOURCE_TYPES
        .iter()
        .chain(["osv", "cve"].iter())
    {
        assert!(
            sql.contains(&format!("'{st}'")),
            "GROUNDED_SQL is missing registry source {st}"
        );
    }
}

/// Phase profile of a real build — `#[ignore]`d; run on demand against a
/// corpus (opened READ-ONLY, so it is safe on the live DB while the app runs):
///
/// ```text
/// FOURDA_GRAPH_PROFILE_DB=D:/4DA/data/4da.db \
///   cargo test --lib -- --ignored content_graph::tests::profile_build_graph_phases --nocapture
/// ```
#[test]
#[ignore = "needs FOURDA_GRAPH_PROFILE_DB pointing at a real corpus"]
fn profile_build_graph_phases() {
    let Ok(path) = std::env::var("FOURDA_GRAPH_PROFILE_DB") else {
        return;
    };
    let conn = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("open corpus read-only");
    for run in 0..2 {
        let started = std::time::Instant::now();
        let (graph, phases) = build_graph_timed(&conn, 7, 150).expect("build");
        println!(
            "run {run}: total {} ms, nodes {}, edges {}, clusters {}, phases {phases:?}",
            started.elapsed().as_millis(),
            graph.nodes.len(),
            graph.edges.len(),
            graph.clusters.len()
        );
        for c in &graph.clusters {
            println!("  cluster {}: {}", c.node_ids.len(), c.label);
        }
        println!(
            "  touches-your-stack nodes: {}",
            graph.nodes.iter().filter(|n| n.affects_you).count()
        );
    }
}
