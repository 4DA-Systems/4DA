// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Story aggregation: collapse near-duplicate items behind one representative
//! BEFORE any edge computation.
//!
//! Raw pairwise similarity in a feed window is dominated by redundancy — an
//! OSV backfill lands dozens of advisories for one package in a single sync
//! (26 axios advisories, 2026-07-16) and every intra-batch pair clears the
//! semantic edge threshold, so the rendered graph was one dense clique plus
//! scraps (258 of 300 live edges were that clique). Collapsing members into a
//! story node removes the clique at the source and leaves edges that carry
//! actual structure.
//!
//! Two grouping signals, merged transitively via union-find:
//! - **Advisory key** (exact): security alerts for the same dependency, keyed
//!   on the dep_linker package — the same identity the feed's advisory
//!   stacking uses, so List and Graph collapse the same items.
//! - **Near-duplicate embeddings** (generic): cosine at or above
//!   [`STORY_COSINE`], or a slightly relaxed cosine backed by strong title
//!   overlap. Precision-first: a missed collapse is minor visual noise; a
//!   wrong collapse misrepresents structure.

use std::collections::HashMap;

use super::edges::title_word_overlap;
use super::types::{RawItem, StoryItem};
use crate::utils::cosine_similarity;

/// Cosine at/above which two items are the same story on embeddings alone.
const STORY_COSINE: f32 = 0.92;
/// Relaxed cosine floor when strong lexical overlap corroborates.
const STORY_COSINE_WITH_OVERLAP: f32 = 0.85;
/// Title-word Jaccard required to corroborate the relaxed cosine floor.
const STORY_OVERLAP_MIN: f32 = 0.50;
/// The feed's advisory-stacking gate; stories reuse it where persisted.
const SECURITY_SIGNAL: &str = "security_alert";
/// Sources that ONLY emit security advisories. `signal_type` is persisted
/// lazily (many advisory rows carry NULL — live check 2026-07-16: 198 of 210
/// window OSV items), so source identity is the reliable advisory gate.
const ADVISORY_SOURCES: &[&str] = &["osv", "cve"];

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut cur = i;
        while self.parent[cur] != root {
            let next = self.parent[cur];
            self.parent[cur] = root;
            cur = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            // Deterministic: smaller index wins as root.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[hi] = lo;
        }
    }
}

fn advisory_key(item: &RawItem) -> Option<String> {
    let is_advisory = item.signal_type.as_deref() == Some(SECURITY_SIGNAL)
        || ADVISORY_SOURCES.contains(&item.source_type.as_str());
    if !is_advisory {
        return None;
    }
    item.matched_package
        .as_deref()
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .or_else(|| advisory_title_subject(&item.title))
}

/// Package key from an advisory title, for advisory rows without a dep link.
/// Handles the two shapes 4DA generates/ingests: `[GHSA-…] Package: summary`
/// and `[GHSA-…] Package has/is/allows …`. Returns None unless the extracted
/// subject is short (1-2 tokens) — a long subject is a sentence, not a
/// package name, and a wrong merge misrepresents structure (precision-first).
fn advisory_title_subject(title: &str) -> Option<String> {
    let trimmed = title.trim();
    let rest = trimmed.strip_prefix('[')?;
    let is_advisory_id = rest.starts_with("GHSA-")
        || rest.starts_with("CVE-")
        || rest.starts_with("RUSTSEC-")
        || rest.starts_with("PYSEC-");
    if !is_advisory_id {
        return None;
    }
    let subject_full = rest.split_once("] ").map(|(_, s)| s)?;

    // Cut at the first verb/preposition marker (same family the preemption
    // matcher uses) to isolate the grammatical subject.
    const MARKERS: &[&str] = &[
        " has ",
        " is ",
        " are ",
        " allows ",
        " could ",
        " can ",
        " may ",
        " in ",
        ": ",
        " vulnerable",
        " affected",
        " exposes ",
        " — ",
        " - ",
    ];
    let cut = MARKERS
        .iter()
        .filter_map(|m| subject_full.find(m))
        .min()
        .unwrap_or(subject_full.len());
    let subject = subject_full[..cut].trim();

    let tokens: Vec<String> = subject
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| {
                c == '\'' || c == '\u{2019}' || c == ',' || c == '.' || c == '"'
            })
            .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() || tokens.len() > 2 {
        return None;
    }
    Some(tokens.join(" "))
}

/// Collapse `items` into stories. Output order follows the input order of
/// each story's representative (input is relevance-sorted, so stories stay
/// relevance-sorted). Deterministic throughout.
pub(super) fn collapse_stories(items: Vec<RawItem>) -> Vec<StoryItem> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }

    let mut uf = UnionFind::new(n);

    // Signal 1: exact advisory-package key.
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        if let Some(key) = advisory_key(item) {
            match by_key.get(&key) {
                Some(&first) => uf.union(first, i),
                None => {
                    by_key.insert(key, i);
                }
            }
        }
    }

    // Signal 2: near-duplicate embeddings (optionally corroborated by titles).
    for i in 0..n {
        for j in (i + 1)..n {
            let sim = cosine_similarity(&items[i].embedding, &items[j].embedding);
            let near_dup = sim >= STORY_COSINE
                || (sim >= STORY_COSINE_WITH_OVERLAP
                    && title_word_overlap(&items[i].title, &items[j].title) >= STORY_OVERLAP_MIN);
            if near_dup {
                uf.union(i, j);
            }
        }
    }

    // Materialize groups in input order (input is relevance-sorted).
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of_root: HashMap<usize, usize> = HashMap::new();
    for i in 0..n {
        let root = uf.find(i);
        match group_of_root.get(&root) {
            Some(&g) => groups[g].push(i),
            None => {
                group_of_root.insert(root, groups.len());
                groups.push(vec![i]);
            }
        }
    }

    let dim = items[0].embedding.len();
    groups
        .into_iter()
        .map(|member_idxs| build_story(&items, &member_idxs, dim))
        .collect()
}

fn build_story(items: &[RawItem], member_idxs: &[usize], dim: usize) -> StoryItem {
    // Representative: highest relevance, ties to the earliest-loaded (input is
    // already relevance-sorted, so the first index wins both).
    let rep_idx = member_idxs[0];
    let rep = &items[rep_idx];

    if member_idxs.len() == 1 {
        return StoryItem {
            item: clone_raw(rep),
            member_ids: vec![rep.id],
            member_count: 1,
            affects_you: rep.matched_package.is_some(),
        };
    }

    // Story embedding: normalized member centroid, so story-level semantic
    // edges compare "what this story is about", not one arbitrary member.
    let mut centroid = vec![0.0f32; dim];
    for &idx in member_idxs {
        for (c, v) in centroid.iter_mut().zip(&items[idx].embedding) {
            *c += v;
        }
    }
    let norm = centroid.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for c in &mut centroid {
            *c /= norm;
        }
    }

    let relevance = member_idxs
        .iter()
        .map(|&idx| items[idx].relevance_score)
        .fold(f32::MIN, f32::max);

    // Highest urgency present anywhere in the story fronts it: a story with
    // one critical member IS critical.
    let priority_rank = |p: Option<&str>| match p {
        Some("critical") => 0,
        Some("alert") => 1,
        Some(_) => 2,
        None => 3,
    };
    let signal_priority = member_idxs
        .iter()
        .filter_map(|&idx| items[idx].signal_priority.clone())
        .min_by_key(|p| priority_rank(Some(p)));

    let member_ids: Vec<i64> = member_idxs.iter().map(|&idx| items[idx].id).collect();

    let affects_you = member_idxs
        .iter()
        .any(|&idx| items[idx].matched_package.is_some());

    let curated = member_idxs.iter().any(|&idx| items[idx].curated);

    StoryItem {
        item: RawItem {
            id: rep.id,
            title: rep.title.clone(),
            url: rep.url.clone(),
            source_type: rep.source_type.clone(),
            relevance_score: relevance,
            signal_type: rep.signal_type.clone(),
            signal_priority,
            matched_package: rep.matched_package.clone(),
            created_at: rep.created_at.clone(),
            curated,
            embedding: centroid,
        },
        member_ids,
        member_count: member_idxs.len(),
        affects_you,
    }
}

pub(super) fn clone_raw(item: &RawItem) -> RawItem {
    RawItem {
        id: item.id,
        title: item.title.clone(),
        url: item.url.clone(),
        source_type: item.source_type.clone(),
        relevance_score: item.relevance_score,
        signal_type: item.signal_type.clone(),
        signal_priority: item.signal_priority.clone(),
        matched_package: item.matched_package.clone(),
        created_at: item.created_at.clone(),
        curated: item.curated,
        embedding: item.embedding.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, title: &str, embedding: Vec<f32>) -> RawItem {
        RawItem {
            id,
            title: title.to_string(),
            url: None,
            source_type: "hackernews".to_string(),
            relevance_score: 0.9,
            signal_type: None,
            signal_priority: None,
            matched_package: None,
            created_at: String::new(),
            curated: false,
            embedding,
        }
    }

    #[test]
    fn advisory_key_groups_same_package_alerts() {
        let mut a = item(1, "[GHSA-1] Axios has an SSRF", vec![1.0, 0.0]);
        a.signal_type = Some("security_alert".into());
        a.matched_package = Some("axios".into());
        // Orthogonal embedding — the exact key must group it anyway.
        let mut b = item(2, "[GHSA-2] Axios CRLF injection", vec![0.0, 1.0]);
        b.signal_type = Some("security_alert".into());
        b.matched_package = Some("Axios ".into()); // normalization: trim + lowercase

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(stories.len(), 1, "same-package advisories form one story");
        assert_eq!(stories[0].member_count, 2);
        assert_eq!(stories[0].member_ids, vec![1, 2]);
    }

    #[test]
    fn non_security_items_never_group_on_package() {
        let mut a = item(1, "axios 2.0 released", vec![1.0, 0.0]);
        a.matched_package = Some("axios".into());
        let mut b = item(2, "why we dropped axios", vec![0.0, 1.0]);
        b.matched_package = Some("axios".into());

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(
            stories.len(),
            2,
            "a release and an opinion piece about one package are distinct stories"
        );
    }

    #[test]
    fn osv_advisories_group_by_package_without_persisted_signal_type() {
        // Live failure 2026-07-16: 198/210 window OSV rows carry NULL
        // signal_type, so gating on it split the axios storm. Source identity
        // must be enough.
        let mut a = item(1, "[GHSA-1] Axios has an SSRF", vec![1.0, 0.0]);
        a.source_type = "osv".into();
        a.matched_package = Some("axios".into());
        let mut b = item(2, "[GHSA-2] whatever unrelated words", vec![0.0, 1.0]);
        b.source_type = "osv".into();
        b.matched_package = Some("axios".into());

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].member_count, 2);
    }

    #[test]
    fn advisory_title_subject_extracts_short_package_subjects() {
        assert_eq!(
            advisory_title_subject("[GHSA-3p68-rc4w-qgx5] Axios has a NO_PROXY Bypass"),
            Some("axios".to_string())
        );
        assert_eq!(
            advisory_title_subject("[GHSA-6chq-wfr3-2hj9] Axios: Header Injection"),
            Some("axios".to_string())
        );
        // A sentence-length subject is not a package name.
        assert_eq!(
            advisory_title_subject(
                "[GHSA-777c-7fjr-54vf] Allocation of Resources Without Limits or Throttling in Axios"
            ),
            None
        );
        // Not an advisory-shaped title at all.
        assert_eq!(advisory_title_subject("Axios 2.0 released"), None);
    }

    #[test]
    fn osv_advisories_without_dep_link_group_via_title_subject() {
        let mut a = item(1, "[GHSA-1] Axios has an SSRF", vec![1.0, 0.0]);
        a.source_type = "osv".into();
        let mut b = item(2, "[GHSA-2] Axios: CRLF Injection", vec![0.0, 1.0]);
        b.source_type = "osv".into();

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(
            stories.len(),
            1,
            "same-package advisories must group on the title-subject fallback"
        );
    }

    #[test]
    fn near_duplicate_embeddings_group() {
        let a = item(1, "React 19 released", vec![1.0, 0.0]);
        let b = item(2, "React 19 released today", vec![0.999, 0.0447]); // cos ~0.999
        let c = item(3, "Postgres tuning guide", vec![0.0, 1.0]);

        let stories = collapse_stories(vec![a, b, c]);
        assert_eq!(stories.len(), 2);
        let story = stories.iter().find(|s| s.member_count == 2).unwrap();
        assert_eq!(story.member_ids, vec![1, 2]);
    }

    #[test]
    fn moderate_similarity_without_overlap_stays_separate() {
        // cos ~0.89 — above the relaxed floor but below STORY_COSINE, and the
        // titles share nothing: must NOT collapse (precision-first).
        let a = item(1, "Rust async runtime deep dive", vec![1.0, 0.0]);
        let b = item(2, "Tokio internals explained", vec![0.89, 0.456]);

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(stories.len(), 2);
    }

    #[test]
    fn representative_is_highest_relevance_and_story_takes_max_score() {
        let mut a = item(1, "story A", vec![1.0, 0.0]);
        a.relevance_score = 0.95;
        let mut b = item(2, "story A again", vec![1.0, 0.0]);
        b.relevance_score = 0.6;

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].item.id, 1);
        assert!((stories[0].item.relevance_score - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn story_priority_is_most_urgent_member() {
        let mut a = item(1, "adv one", vec![1.0, 0.0]);
        a.signal_type = Some("security_alert".into());
        a.matched_package = Some("axios".into());
        a.signal_priority = Some("advisory".into());
        let mut b = item(2, "adv two", vec![1.0, 0.0]);
        b.signal_type = Some("security_alert".into());
        b.matched_package = Some("axios".into());
        b.signal_priority = Some("critical".into());

        let stories = collapse_stories(vec![a, b]);
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].item.signal_priority.as_deref(), Some("critical"));
    }

    #[test]
    fn centroid_embedding_is_normalized() {
        let a = item(1, "same thing", vec![1.0, 0.0]);
        let b = item(2, "same thing too", vec![1.0, 0.0]);
        let stories = collapse_stories(vec![a, b]);
        let emb = &stories[0].item.embedding;
        let norm: f32 = emb.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "centroid must be unit-norm, got {norm}"
        );
    }

    #[test]
    fn empty_input_yields_no_stories() {
        assert!(collapse_stories(Vec::new()).is_empty());
    }
}
