// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One chain per technology: topic spellings fold before grouping.
//!
//! Audit 2026-10-07 (noted pre-existing in #888): Preemption showed Next.js as
//! separate chains. The live snapshot held three at once — `next`,
//! `next.js` and `nextjs` signal chains, five events each — because
//! `extract_topics` mints every spelling it sees as its own topic: "Next.js 15"
//! yields both `next` (the word before the dot) and `next.js` (the phrase), and
//! a tagged post yields `nextjs`. Chains grouped on the raw string, so one
//! subject split three ways and filled three of the ten chain slots.
//!
//! Spellings now fold through the curated alias table
//! ([`crate::scoring::aliases::canonical_spelling`]: case, dots, hyphens,
//! spaces and the `.js` suffix), and anything the table does not know folds on
//! its separator-free form. The raw spellings are kept per group: dependency
//! grounding must still ask about the real package name (`next`, not
//! `nextjs`), and the chain is shown under the spelling its items use most.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::signal_chains_candidates::ChainCandidateItem;
use super::TopicChainItem;
use crate::scoring::aliases::{canonical_spelling, compact_spelling};

/// Every item about one technology, however each item spelled it.
pub(super) struct TopicGroup {
    /// The spelling the chain is shown and identified by.
    pub(super) display: String,
    /// The fold's canonical key (`nextjs` for next / next.js / nextjs).
    pub(super) key: String,
    /// Every raw spelling that folded in, sorted.
    pub(super) variants: Vec<String>,
    /// Each item once, even when it carried several spellings.
    pub(super) items: Vec<TopicChainItem>,
}

/// The grouping key for a raw topic.
pub(super) fn chain_topic_key(topic: &str) -> String {
    if let Some(canonical) = canonical_spelling(topic) {
        return canonical.to_string();
    }
    let compact = compact_spelling(topic);
    if compact.is_empty() {
        topic.trim().to_lowercase()
    } else {
        compact
    }
}

/// Group candidate items by canonical topic. Deterministic: groups come out
/// sorted by key, items in input order.
pub(super) fn group_topics(items: &[ChainCandidateItem]) -> Vec<TopicGroup> {
    struct Acc {
        // spelling -> distinct items that used it
        spellings: BTreeMap<String, BTreeSet<i64>>,
        seen: BTreeSet<i64>,
        items: Vec<TopicChainItem>,
    }
    let mut groups: BTreeMap<String, Acc> = BTreeMap::new();

    for (id, title, source_type, created_at, content, tags) in items {
        for topic in crate::extract_topics(title, content, tags) {
            let acc = groups
                .entry(chain_topic_key(&topic))
                .or_insert_with(|| Acc {
                    spellings: BTreeMap::new(),
                    seen: BTreeSet::new(),
                    items: Vec::new(),
                });
            acc.spellings.entry(topic).or_default().insert(*id);
            if acc.seen.insert(*id) {
                acc.items.push((
                    *id,
                    title.clone(),
                    source_type.clone(),
                    created_at.clone(),
                    content.clone(),
                ));
            }
        }
    }

    groups
        .into_iter()
        .map(|(key, acc)| TopicGroup {
            display: display_spelling(&acc.spellings),
            key,
            variants: acc.spellings.keys().cloned().collect(),
            items: acc.items,
        })
        .collect()
}

/// The spelling most items used; ties go to the longer (more specific:
/// `next.js` over `next`), then the alphabetically first.
fn display_spelling(spellings: &BTreeMap<String, BTreeSet<i64>>) -> String {
    let counts: HashMap<&String, usize> = spellings.iter().map(|(s, ids)| (s, ids.len())).collect();
    spellings
        .keys()
        .max_by(|a, b| {
            counts[a]
                .cmp(&counts[b])
                .then(a.len().cmp(&b.len()))
                .then(b.cmp(a))
        })
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, title: &str, tags: &[&str]) -> ChainCandidateItem {
        (
            id,
            title.to_string(),
            "hackernews".to_string(),
            format!("2026-10-0{id}T12:00:00Z"),
            String::new(),
            tags.iter().map(|t| (*t).to_string()).collect(),
        )
    }

    #[test]
    fn next_js_spellings_form_one_group() {
        let items = vec![
            item(1, "Next.js 15 ships partial prerendering", &[]),
            item(2, "Migrating a nextjs app router project", &[]),
            item(3, "Vercel posts next roadmap", &["nextjs"]),
        ];
        let groups = group_topics(&items);
        let next: Vec<&TopicGroup> = groups.iter().filter(|g| g.key == "nextjs").collect();
        assert_eq!(next.len(), 1, "one Next.js group");
        let next = next[0];
        assert_eq!(
            next.items.iter().map(|i| i.0).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "each item once, even when it carried two spellings"
        );
        assert!(next.variants.contains(&"next".to_string()));
        assert!(next.variants.contains(&"next.js".to_string()));
        assert!(next.variants.contains(&"nextjs".to_string()));
        assert!(
            groups
                .iter()
                .all(|g| !g.variants.contains(&"next".to_string()) || g.key == "nextjs"),
            "no stray `next` group"
        );
    }

    #[test]
    fn node_and_vue_spellings_fold_and_unknown_names_fold_on_separators() {
        assert_eq!(chain_topic_key("node.js"), chain_topic_key("nodejs"));
        assert_eq!(chain_topic_key("node"), chain_topic_key("Node.js"));
        assert_eq!(chain_topic_key("vue.js"), chain_topic_key("vue"));
        assert_eq!(
            chain_topic_key("react native"),
            chain_topic_key("react-native")
        );
        // Not in the alias table: still one subject across separators/case.
        assert_eq!(
            chain_topic_key("styled-components"),
            chain_topic_key("Styled Components")
        );
        // Different subjects stay apart.
        assert_ne!(chain_topic_key("container"), chain_topic_key("docker"));
        assert_ne!(chain_topic_key("react"), chain_topic_key("react native"));
    }

    #[test]
    fn display_prefers_the_spelling_items_use_most_then_the_specific_one() {
        let mut spellings = BTreeMap::new();
        spellings.insert("next".to_string(), BTreeSet::from([1, 2]));
        spellings.insert("next.js".to_string(), BTreeSet::from([1, 2]));
        spellings.insert("nextjs".to_string(), BTreeSet::from([3]));
        assert_eq!(display_spelling(&spellings), "next.js");
        spellings.insert("nextjs".to_string(), BTreeSet::from([3, 4, 5]));
        assert_eq!(display_spelling(&spellings), "nextjs");
    }
}
