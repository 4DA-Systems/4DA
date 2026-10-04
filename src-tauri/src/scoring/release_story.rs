// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Release-story deduplication: one row per `<project> <version>` story.
//!
//! The same release reaches the feed from several sources with titles that
//! share too few words for the fuzzy (Jaccard >= 0.65) pass. Live 2026-10-04,
//! three of the Signal top-20 were one story: "Rust 1.99 is out, and for the
//! first time the corresponding cargo-semver-checks…" (Mastodon), "Announcing
//! Rust 1.99.0" (RSS) and "Rust 1.99.0 released" (Mastodon). A shared
//! `project + dotted version` pair is deterministic proof of one story, so the
//! highest-scoring copy survives and the rest become its `similar_titles`.
//!
//! Scope: editorial rows only. Registry and advisory rows (one row per package
//! and version, graded and superseded by their own lanes) are never merged.
//! A dotted version is required — "React 20 ships…" and "React 20 migration
//! guide" stay separate — and `1.99` and `1.99.0` are the same release.

use std::collections::HashMap;

use tracing::info;

use crate::SourceRelevance;

/// Sources whose rows are one-package-one-version by construction.
const STRUCTURED_SOURCES: &[&str] = &[
    "crates_io",
    "npm_registry",
    "npm",
    "pypi",
    "go_modules",
    "osv",
    "cve",
];

/// Words that sit before a version without naming a project.
const NOT_A_PROJECT: &[&str] = &[
    "v", "version", "release", "released", "releases", "update", "to", "from", "and", "of", "the",
    "in", "on", "at", "is", "for", "with", "rc", "beta", "alpha",
];

/// `(project, version)` for the first `<word> <major.minor[.patch]>` pair in
/// the title, lowercased, with a trailing `.0` patch dropped. `None` when the
/// title names no dotted version after a project word.
pub(crate) fn release_story_key(title: &str) -> Option<(String, String)> {
    let decoded = crate::decode_html_entities(title);
    let tokens: Vec<String> = decoded
        .split_whitespace()
        .map(|t| {
            t.chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-')
                .collect::<String>()
                .trim_matches(|c| c == '.' || c == '-')
                .to_lowercase()
        })
        .collect();
    for pair in tokens.windows(2) {
        let (word, ver) = (&pair[0], &pair[1]);
        let Some(version) = dotted_version(ver) else {
            continue;
        };
        if word.len() < 2
            || NOT_A_PROJECT.contains(&word.as_str())
            || !word.chars().next().is_some_and(char::is_alphabetic)
        {
            continue;
        }
        return Some((word.clone(), version));
    }
    None
}

/// `1.99`, `1.99.0`, `v2.12.1` → `1.99`, `1.99`, `2.12.1`. Two or three
/// numeric parts; anything else (dates, single numbers) is not a version.
fn dotted_version(token: &str) -> Option<String> {
    let t = token.strip_prefix('v').unwrap_or(token);
    let parts: Vec<&str> = t.split('.').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    if parts.len() == 3 && parts[2] == "0" {
        return Some(format!("{}.{}", parts[0], parts[1]));
    }
    Some(parts.join("."))
}

/// `1.99` → `[1, 99, 0]` so `1.99` and `1.99.0` compare equal and `1.100 > 1.99`.
fn version_parts(version: &str) -> Vec<u32> {
    let mut parts: Vec<u32> = version
        .split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();
    parts.resize(3, 0);
    parts
}

/// Release-announcement wording — the only kind of older-version row a newer
/// release supersedes.
fn is_release_announcement(title: &str) -> bool {
    let t = title.to_lowercase();
    [
        "announcing",
        "released",
        "release notes",
        "is out",
        "is here",
        "now available",
        "ships",
        "lands",
    ]
    .iter()
    .any(|w| t.contains(w))
}

/// Merge editorial rows that announce the same `<project> <version>`, and
/// older release announcements of a project whose newer release is present.
/// Must run after `sort_results` (highest score first survives). Returns
/// the number of rows folded into a survivor.
///
/// Runs twice: in the analyzer's batch layer, and on the differential merge
/// (`merge_differential_results`, after `dedup_results` re-sorted it). The
/// second call is what folds a release story that arrives in a LATER batch
/// against rows already on display — live 2026-10-04 "Rust 1.99 is out…"
/// (one batch) and "Announcing Rust 1.98.0" (re-selected by a later one)
/// were both shown. Because a folded row may itself be an earlier survivor,
/// its own `similar_titles` move with it, and a title the survivor already
/// lists is not counted twice.
pub(crate) fn release_story_dedup_results(results: &mut Vec<SourceRelevance>) -> usize {
    // project -> [(index, version, version string)] in score order.
    let mut by_project: HashMap<String, Vec<(usize, Vec<u32>, String)>> = HashMap::new();
    for (idx, item) in results.iter().enumerate() {
        if item.excluded || STRUCTURED_SOURCES.contains(&item.source_type.as_str()) {
            continue;
        }
        let Some((project, version)) = release_story_key(&item.title) else {
            continue;
        };
        let parts = version_parts(&version);
        by_project
            .entry(project)
            .or_default()
            .push((idx, parts, version));
    }
    let mut folded: Vec<(usize, usize)> = Vec::new();
    for rows in by_project.values() {
        // 1. One row per (project, version): the first (highest-scored) copy
        //    survives, later copies fold into it.
        let mut version_survivor: HashMap<&Vec<u32>, usize> = HashMap::new();
        for (idx, parts, _) in rows {
            match version_survivor.get(parts) {
                Some(&survivor) => folded.push((*idx, survivor)),
                None => {
                    version_survivor.insert(parts, *idx);
                }
            }
        }
        // 2. The newest ANNOUNCED release supersedes older announcements:
        //    "Announcing Rust 1.98.0" folds under "Rust 1.99.0 released".
        //    Only announcements decide "newest" — a post that MENTIONS a
        //    future version ("on track to release in Rust 1.100") must never
        //    swallow the real 1.99 release (live 2026-10-04) — and only
        //    announcements fold, so a "TypeScript 5.9 decorators" tutorial
        //    stays.
        let announced = |idx: usize| is_release_announcement(&results[idx].title);
        let Some((newest_parts, newest_survivor)) = version_survivor
            .iter()
            .filter(|(_, &idx)| announced(idx))
            .max_by(|a, b| a.0.cmp(b.0))
            .map(|(p, &idx)| (*p, idx))
        else {
            continue;
        };
        for (parts, &idx) in &version_survivor {
            if *parts < newest_parts && announced(idx) {
                folded.push((idx, newest_survivor));
            }
        }
    }
    // A row folded into a survivor that itself folds is re-pointed to the
    // final survivor, so no title is attached to a dropped row.
    let final_of: HashMap<usize, usize> = folded.iter().copied().collect();
    for entry in &mut folded {
        let mut target = entry.1;
        while let Some(&next) = final_of.get(&target) {
            target = next;
        }
        entry.1 = target;
    }
    if folded.is_empty() {
        return 0;
    }
    for &(dup, survivor) in &folded {
        let mut carried = vec![results[dup].title.clone()];
        carried.extend(results[dup].similar_titles.iter().cloned());
        let target = &mut results[survivor];
        for title in carried {
            if title != target.title && !target.similar_titles.contains(&title) {
                target.similar_count += 1;
                target.similar_titles.push(title);
            }
        }
    }
    let drop: std::collections::HashSet<usize> = folded.iter().map(|(d, _)| *d).collect();
    let mut idx = 0;
    results.retain(|_| {
        let keep = !drop.contains(&idx);
        idx += 1;
        keep
    });
    info!(target: "4da::scoring", folded = drop.len(), kept = results.len(), "Release-story deduplication");
    drop.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_release_from_three_sources_is_one_key() {
        // Live 2026-10-04: one story, three top-20 rows.
        let a = release_story_key(
            "Rust 1.99 is out, and for the first time the corresponding cargo-semver-checks",
        );
        let b = release_story_key("Announcing Rust 1.99.0");
        let c = release_story_key("Rust🦀 1.99.0 released by @rust - highlights: extern \"C\"");
        assert_eq!(a, Some(("rust".into(), "1.99".into())));
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn different_versions_or_no_dotted_version_stay_apart() {
        assert_ne!(
            release_story_key("Announcing Rust 1.98.0"),
            release_story_key("Announcing Rust 1.99.0")
        );
        assert_eq!(
            release_story_key("React 20 ships with native signals"),
            None
        );
        assert_eq!(release_story_key("Why Hash Tables Collide"), None);
        assert_eq!(release_story_key("Released on 2026.10 schedule"), None);
        // A patch release is its own story.
        assert_ne!(
            release_story_key("Tauri 2.12 is here"),
            release_story_key("Tauri 2.12.1 fixes the updater")
        );
    }

    fn item(id: u64, title: &str, source: &str) -> SourceRelevance {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "url": null, "top_score": 0.9,
            "matches": [], "relevant": true, "source_type": source,
        }))
        .expect("SourceRelevance from JSON")
    }

    #[test]
    fn a_newer_release_supersedes_an_older_announcement_but_not_a_tutorial() {
        // Live 2026-10-04: "Announcing Rust 1.98.0" still showed after 1.99.
        let mut results = vec![
            item(1, "Announcing Rust 1.98.0", "rss"),
            item(2, "Rust 1.99.0 released", "mastodon"),
            item(3, "TypeScript 5.9 decorators in real codebases", "devto"),
            item(4, "Announcing TypeScript 7.0", "rss"),
            item(5, "This Week in Rust 668", "rss"),
        ];
        let folded = release_story_dedup_results(&mut results);
        let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert_eq!(
            folded, 1,
            "only the superseded Rust 1.98 announcement folds"
        );
        assert_eq!(ids, vec![2, 3, 4, 5]);
        assert!(results[0]
            .similar_titles
            .contains(&"Announcing Rust 1.98.0".to_string()));
    }

    #[test]
    fn a_future_version_mention_never_swallows_the_real_release() {
        // Live 2026-10-04: "on track to release in Rust 1.100" outranked the
        // 1.99 release in the first draft of this rule.
        let mut results = vec![
            item(1, "The `allocator_api` feature has been stabilized, on track to release in Rust 1.100", "reddit"),
            item(2, "Announcing Rust 1.99.0", "rss"),
            item(3, "Rust 1.98.0 is out", "reddit"),
            item(4, "Rust 1.99.0 released", "mastodon"),
        ];
        release_story_dedup_results(&mut results);
        let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            vec![1, 2],
            "1.99 survives; 1.98 and the 1.99 copy fold into it"
        );
        assert_eq!(results[1].similar_count, 2);
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(version_parts("1.100") > version_parts("1.99"));
        assert_eq!(version_parts("1.99"), version_parts("1.99.0"));
    }

    #[test]
    fn dedup_keeps_the_first_and_lists_the_rest() {
        let mut results = vec![
            item(
                1,
                "Rust 1.99 is out, and cargo-semver-checks ships with it",
                "mastodon",
            ),
            item(2, "Deser: Rethinking Rust Serialization", "hackernews"),
            item(3, "Announcing Rust 1.99.0", "rss"),
            item(4, "crates.io: tauri v2.12.1", "crates_io"),
            item(5, "Announcing Tauri 2.12.1", "rss"),
            item(6, "Rust 1.99.0 released", "mastodon"),
        ];
        let folded = release_story_dedup_results(&mut results);
        assert_eq!(folded, 2);
        let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            vec![1, 2, 4, 5],
            "registry row 4 never merges with editorial 5"
        );
        assert_eq!(results[0].similar_count, 2);
        assert!(results[0]
            .similar_titles
            .contains(&"Announcing Rust 1.99.0".to_string()));
    }
}
