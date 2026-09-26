// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which parsed RSS entries leave the adapter each cycle.
//!
//! The adapter used to concatenate every feed's entries in HTTP-completion
//! order and truncate the lot to `max_items` (30). Already-stored entries took
//! those 30 slots, so the same few fast feeds filled the cap every cycle and
//! the rest never reached the database at all: on 2026-09-24, 58 of 72 enabled
//! feeds had never produced an item, and 57 of those 58 were live and
//! answering (Tauri, React, Vite, RustSec, Cloudflare, ...).
//!
//! The cap now limits NEW entries only, and each feed gets a fair turn:
//!
//! 1. Each feed contributes its newest [`MAX_ENTRIES_PER_FEED`] entries.
//! 2. Entries already in the database are passed through uncapped. The
//!    processor only touches them, and that touch is what keeps an entry that
//!    is still in its feed from ageing out of retention and returning as "new".
//! 3. A never-seen entry older than [`NEW_ENTRY_MAX_AGE_DAYS`] is skipped. It
//!    is a feed's back catalogue, not news, and without this the first cycles
//!    after a feed becomes reachable would ingest years of old posts.
//! 4. The remaining new entries are taken round-robin across feeds, newest
//!    first, up to the cap, so no single feed can use the whole budget.

use chrono::{DateTime, Duration, Utc};

use super::SourceItem;

/// Newest entries considered per feed per cycle. Bounds the touch work for
/// huge feeds (snyk.io and vercel.com each publish more than 1,600 entries).
pub(crate) const MAX_ENTRIES_PER_FEED: usize = 20;

/// A never-seen entry published longer ago than this is not ingested.
pub(crate) const NEW_ENTRY_MAX_AGE_DAYS: i64 = 30;

fn published(item: &SourceItem) -> Option<DateTime<Utc>> {
    item.metadata
        .as_ref()
        .and_then(|m| m.get("pub_date"))
        .and_then(crate::sources::freshness::parse_timestamp)
}

/// Select this cycle's items: capped, fairly interleaved new entries first,
/// then every already-known entry. `feeds` holds each feed's entries in
/// document order; `is_known` answers "is this source_id already stored?".
pub(crate) fn select_items(
    feeds: Vec<Vec<SourceItem>>,
    new_cap: usize,
    is_known: impl Fn(&str) -> bool,
    now: DateTime<Utc>,
) -> Vec<SourceItem> {
    let cutoff = now - Duration::days(NEW_ENTRY_MAX_AGE_DAYS);
    let mut seen = std::collections::HashSet::new();
    let mut known = Vec::new();
    let mut new_per_feed: Vec<std::collections::VecDeque<SourceItem>> = Vec::new();

    for mut entries in feeds {
        // Newest first; undated entries keep their document order after the dated ones.
        entries.sort_by_key(|item| std::cmp::Reverse(published(item)));
        let mut fresh = std::collections::VecDeque::new();
        for item in entries.into_iter().take(MAX_ENTRIES_PER_FEED) {
            if !seen.insert(item.source_id.clone()) {
                continue;
            }
            if is_known(&item.source_id) {
                known.push(item);
            } else if published(&item).is_none_or(|d| d >= cutoff) {
                fresh.push_back(item);
            }
        }
        new_per_feed.push(fresh);
    }

    let mut selected = Vec::new();
    while selected.len() < new_cap && new_per_feed.iter().any(|f| !f.is_empty()) {
        for feed in new_per_feed.iter_mut() {
            if selected.len() >= new_cap {
                break;
            }
            if let Some(item) = feed.pop_front() {
                selected.push(item);
            }
        }
    }

    selected.extend(known);
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, days_ago: Option<i64>, now: DateTime<Utc>) -> SourceItem {
        let date = days_ago.map(|d| (now - Duration::days(d)).to_rfc2822());
        SourceItem::new("rss", id, &format!("Post {id}"))
            .with_metadata(serde_json::json!({ "pub_date": date }))
    }

    fn ids(items: &[SourceItem]) -> Vec<&str> {
        items.iter().map(|i| i.source_id.as_str()).collect()
    }

    #[test]
    fn known_entries_do_not_consume_the_new_cap() {
        let now = Utc::now();
        // The first feed is all already-stored entries — the shape that used to
        // fill the whole cap and starve every other feed.
        let busy: Vec<_> = (0..40)
            .map(|i| entry(&format!("old{i}"), Some(1), now))
            .collect();
        let quiet = vec![entry("q1", Some(2), now), entry("q2", Some(3), now)];
        let out = select_items(vec![busy, quiet], 2, |id| id.starts_with("old"), now);
        assert_eq!(&ids(&out)[..2], ["q1", "q2"]);
        // Known entries still flow through (for the retention touch), bounded per feed.
        assert_eq!(out.len(), 2 + MAX_ENTRIES_PER_FEED);
    }

    #[test]
    fn new_entries_are_interleaved_across_feeds_newest_first() {
        let now = Utc::now();
        let a = vec![entry("a-old", Some(9), now), entry("a-new", Some(1), now)];
        let b = vec![entry("b-new", Some(2), now), entry("b-old", Some(8), now)];
        let c = vec![entry("c-new", Some(3), now)];
        let out = select_items(vec![a, b, c], 3, |_| false, now);
        assert_eq!(ids(&out), ["a-new", "b-new", "c-new"]);
    }

    #[test]
    fn back_catalogue_is_skipped_but_undated_entries_are_kept() {
        let now = Utc::now();
        let feed = vec![
            entry("ancient", Some(NEW_ENTRY_MAX_AGE_DAYS + 1), now),
            entry("recent", Some(NEW_ENTRY_MAX_AGE_DAYS - 1), now),
            entry("undated", None, now),
        ];
        let out = select_items(vec![feed], 30, |_| false, now);
        assert_eq!(ids(&out), ["recent", "undated"]);
    }

    #[test]
    fn an_old_entry_that_is_already_stored_is_still_passed_through() {
        let now = Utc::now();
        let feed = vec![entry("stored", Some(400), now)];
        let out = select_items(vec![feed], 30, |_| true, now);
        assert_eq!(ids(&out), ["stored"]);
    }

    #[test]
    fn an_entry_listed_by_two_feeds_is_emitted_once() {
        let now = Utc::now();
        let a = vec![entry("shared", Some(1), now)];
        let b = vec![entry("shared", Some(1), now), entry("b1", Some(1), now)];
        let out = select_items(vec![a, b], 30, |_| false, now);
        assert_eq!(ids(&out), ["shared", "b1"]);
    }
}
