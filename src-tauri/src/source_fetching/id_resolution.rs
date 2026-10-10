// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Give every deep-scan item its durable `source_items.id`.
//!
//! The deep-scan fetcher builds `GenericSourceItem`s BEFORE they are stored,
//! so it stamped each one with a 64-bit hash of `source_type:source_id` as a
//! placeholder id — and nothing ever swapped it for the row id. Every
//! downstream write keyed on `SourceRelevance::id` then addressed a row that
//! does not exist: score/rank UPDATEs silently matched nothing, and the
//! `item_necessity` INSERT (FK to `source_items.id`) failed with
//! "FOREIGN KEY constraint failed" (fresh-profile E2E 2026-10-10). The
//! in-memory results the UI acts on carried the same phantom ids.

use tracing::warn;

use crate::db::Database;
use crate::GenericSourceItem;

/// Replace each item's placeholder id with its `source_items.id`, looked up
/// by `(source_type, source_id)` after the fetch stored it. Returns how many
/// items have no row (a failed upsert); those keep the placeholder, and every
/// FK-bound write skips them.
pub(crate) fn resolve_stored_ids(
    db: &Database,
    items: &mut [(GenericSourceItem, Vec<f32>)],
) -> usize {
    if items.is_empty() {
        return 0;
    }
    let keys: Vec<(&str, &str)> = items
        .iter()
        .map(|(item, _)| (item.source_type.as_str(), item.source_id.as_str()))
        .collect();
    let row_ids = match db.source_item_row_ids(&keys) {
        Ok(ids) => ids,
        Err(e) => {
            warn!(target: "4da::sources", error = %e, "Could not resolve stored item ids");
            return items.len();
        }
    };
    let mut unresolved = 0;
    for ((item, _), row_id) in items.iter_mut().zip(row_ids) {
        match row_id.and_then(|id| u64::try_from(id).ok()) {
            Some(id) => item.id = id,
            None => unresolved += 1,
        }
    }
    if unresolved > 0 {
        warn!(
            target: "4da::sources",
            unresolved,
            "Fetched items with no stored row — their scores cannot be persisted"
        );
    }
    unresolved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(source_type: &str, source_id: &str, placeholder: u64) -> (GenericSourceItem, Vec<f32>) {
        (
            GenericSourceItem {
                id: placeholder,
                source_id: source_id.to_string(),
                source_type: source_type.to_string(),
                title: format!("title {source_id}"),
                url: None,
                content: String::new(),
                feed_origin: None,
                tags: None,
                published_at: None,
            },
            vec![],
        )
    }

    #[test]
    fn placeholder_ids_become_row_ids_so_necessity_persists() {
        let db = crate::test_utils::test_db();
        let emb = crate::test_utils::seed_embedding("x");
        let stored_id = db
            .upsert_source_item("hackernews", "hn-1", None, "title hn-1", "", &emb)
            .expect("store item");

        let mut items = vec![
            item("hackernews", "hn-1", 0xDEAD_BEEF_CAFE_F00D),
            item("hackernews", "never-stored", 42),
        ];
        assert_eq!(resolve_stored_ids(&db, &mut items), 1);
        assert_eq!(items[0].0.id, stored_id as u64);
        assert_eq!(items[1].0.id, 42, "an unstored item keeps its placeholder");

        // The write that failed live: necessity keyed on the resolved id lands.
        db.persist_necessity_scores(&[(items[0].0.id, 0.6, None, None, None)])
            .expect("FK satisfied by the resolved id");
    }

    /// One orphan (an item pruned between scoring and persist, or a stray
    /// placeholder) must not fail the FK and roll back everyone else's row.
    #[test]
    fn necessity_batch_skips_orphans_instead_of_failing_whole() {
        let db = crate::test_utils::test_db();
        let emb = crate::test_utils::seed_embedding("y");
        let stored_id = db
            .upsert_source_item("lobsters", "l-1", None, "title l-1", "", &emb)
            .expect("store item") as u64;

        db.persist_necessity_scores(&[
            (0xDEAD_BEEF_CAFE_F00D, 0.9, None, None, None),
            (987_654_321, 0.8, None, None, None),
            (stored_id, 0.7, Some("reason".into()), None, None),
        ])
        .expect("orphans are skipped, not fatal");

        let conn = db.conn.lock();
        let rows: Vec<i64> = conn
            .prepare("SELECT source_item_id FROM item_necessity")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, vec![stored_id as i64]);
    }
}
