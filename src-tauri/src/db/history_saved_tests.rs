// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Retention never deletes a saved item (audit 2026-11-11).
//!
//! The Saved list is `interactions JOIN source_items` on a `save` row, so a
//! prune that deleted the item removed it from the user's own list with no
//! trace. All three `source_items` prunes did: the two age prunes
//! (`cleanup_old_items`, `run_maintenance`) and the noise prune.

use crate::db::Database;
use crate::test_utils::{insert_test_item, test_db};

/// A database with the real `interactions` table (ACE owns its schema).
fn db_with_saves() -> Database {
    let db = test_db();
    crate::ace::db::migrate(&db.conn).expect("ACE schema");
    db
}

fn save(db: &Database, item_id: i64) {
    db.conn
        .lock()
        .execute(
            "INSERT INTO interactions (item_id, action_type, signal_strength)
             VALUES (?1, 'save', 1.0)",
            [item_id],
        )
        .expect("save row");
}

/// Last seen (and created) `days` ago, scored as confirmed noise by the
/// current pipeline: past every prune's age window, and below the noise bar.
fn age_as_noise(db: &Database, item_id: i64, days: i64) {
    db.conn
        .lock()
        .execute(
            &format!(
                "UPDATE source_items
                    SET last_seen = datetime('now', '-{days} days'),
                        created_at = datetime('now', '-{days} days'),
                        relevance_score = 0.01,
                        scored_pipeline_version = {}
                  WHERE id = ?1",
                crate::scoring::PIPELINE_VERSION
            ),
            [item_id],
        )
        .expect("age the item");
}

/// The Saved list, by the query `content_commands::get_saved_items` runs.
fn saved_titles(db: &Database) -> Vec<String> {
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT i.item_id, si.title
               FROM interactions i
               JOIN source_items si ON si.id = i.item_id
              WHERE i.action_type = 'save'
              ORDER BY si.title",
        )
        .expect("saved list");
    stmt.query_map([], |row| row.get::<_, String>(1))
        .expect("saved rows")
        .map(|title| title.expect("title"))
        .collect()
}

fn has_embedding(db: &Database, item_id: i64) -> bool {
    db.conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM source_vec WHERE rowid = ?1",
            [item_id],
            |row| row.get::<_, i64>(0),
        )
        .expect("embedding count")
        > 0
}

/// An old saved item beside an old unsaved one: `(saved, unsaved)`.
fn old_saved_and_unsaved(db: &Database) -> (i64, i64) {
    let saved = insert_test_item(db, "hackernews", "kept", "Saved article", "worth keeping");
    let unsaved = insert_test_item(db, "hackernews", "gone", "Old article", "nobody saved it");
    age_as_noise(db, saved, 400);
    age_as_noise(db, unsaved, 400);
    save(db, saved);
    (saved, unsaved)
}

#[test]
fn cleanup_old_items_keeps_a_saved_item_and_its_embedding() {
    let db = db_with_saves();
    let (saved, unsaved) = old_saved_and_unsaved(&db);

    assert_eq!(
        db.cleanup_old_items(30).unwrap(),
        1,
        "only the unsaved item"
    );
    assert_eq!(saved_titles(&db), vec!["Saved article"]);
    assert!(
        has_embedding(&db, saved),
        "the saved item keeps its embedding"
    );
    assert!(!has_embedding(&db, unsaved), "the pruned item's is swept");
    assert_eq!(db.total_item_count().unwrap(), 1);
}

#[test]
fn run_maintenance_keeps_a_saved_item_and_its_embedding() {
    let db = db_with_saves();
    let (saved, unsaved) = old_saved_and_unsaved(&db);

    let result = db.run_maintenance(30).unwrap();
    assert_eq!(result.deleted_items, 1, "only the unsaved item");
    assert_eq!(saved_titles(&db), vec!["Saved article"]);
    assert!(
        has_embedding(&db, saved),
        "the saved item keeps its embedding"
    );
    assert!(!has_embedding(&db, unsaved), "the pruned item's is swept");
}

/// A save outranks a score: the noise prune, its dry-run count and its sample
/// all leave the saved item out, so the three can still never disagree.
#[test]
fn the_noise_prune_never_counts_samples_or_deletes_a_saved_item() {
    let db = db_with_saves();
    let (_saved, unsaved) = old_saved_and_unsaved(&db);

    assert_eq!(db.count_prunable_noise(0.05, 30).unwrap(), 1);
    let sample = db.get_prunable_noise_sample(0.05, 30, 10).unwrap();
    assert_eq!(
        sample.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![unsaved]
    );
    assert_eq!(db.prune_noise(0.05, 30, 100).unwrap(), 1);
    assert_eq!(saved_titles(&db), vec!["Saved article"]);
    assert_eq!(db.prune_noise(0.05, 30, 100).unwrap(), 0);
}

/// The protection is the save, nothing more: once it is removed the item ages
/// out like any other, and other interactions never protected it.
#[test]
fn an_unsaved_item_ages_out_again() {
    let db = db_with_saves();
    let (saved, _unsaved) = old_saved_and_unsaved(&db);
    db.conn
        .lock()
        .execute(
            "INSERT INTO interactions (item_id, action_type) VALUES (?1, 'click')",
            [saved],
        )
        .unwrap();
    assert_eq!(db.cleanup_old_items(30).unwrap(), 1);

    db.conn
        .lock()
        .execute(
            "DELETE FROM interactions WHERE action_type = 'save' AND item_id = ?1",
            [saved],
        )
        .unwrap();
    assert_eq!(
        db.cleanup_old_items(30).unwrap(),
        1,
        "a click is not a save"
    );
    assert!(saved_titles(&db).is_empty());
    assert_eq!(db.total_item_count().unwrap(), 0);
}

/// A database ACE has not initialised has no `interactions` table: nothing
/// can have been saved, and every prune still runs.
#[test]
fn prunes_still_run_before_the_saves_table_exists() {
    let db = test_db();
    let old = insert_test_item(&db, "hackernews", "old", "Old article", "x");
    age_as_noise(&db, old, 400);
    assert_eq!(db.count_prunable_noise(0.05, 30).unwrap(), 1);
    assert_eq!(db.cleanup_old_items(30).unwrap(), 1);

    let old = insert_test_item(&db, "hackernews", "old2", "Old article 2", "x");
    age_as_noise(&db, old, 400);
    assert_eq!(db.run_maintenance(30).unwrap().deleted_items, 1);

    let old = insert_test_item(&db, "hackernews", "old3", "Old article 3", "x");
    age_as_noise(&db, old, 400);
    assert_eq!(db.prune_noise(0.05, 30, 100).unwrap(), 1);
}
