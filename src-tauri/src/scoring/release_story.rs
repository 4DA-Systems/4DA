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

/// Merge editorial rows that announce the same `<project> <version>`.
/// Must run after `sort_results` (highest score first survives). Returns
/// the number of rows folded into a survivor.
pub(crate) fn release_story_dedup_results(results: &mut Vec<SourceRelevance>) -> usize {
    let mut first_by_key: HashMap<(String, String), usize> = HashMap::new();
    let mut folded: Vec<(usize, usize)> = Vec::new();
    for (idx, item) in results.iter().enumerate() {
        if item.excluded || STRUCTURED_SOURCES.contains(&item.source_type.as_str()) {
            continue;
        }
        let Some(key) = release_story_key(&item.title) else {
            continue;
        };
        match first_by_key.get(&key) {
            Some(&survivor) => folded.push((idx, survivor)),
            None => {
                first_by_key.insert(key, idx);
            }
        }
    }
    if folded.is_empty() {
        return 0;
    }
    for &(dup, survivor) in &folded {
        let title = results[dup].title.clone();
        results[survivor].similar_count += 1;
        results[survivor].similar_titles.push(title);
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
