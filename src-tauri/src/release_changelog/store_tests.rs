// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The archive cache: one row per (ecosystem, package, version), crates.io
//! names folded, changelog text round-tripped through gzip, refusals cached,
//! old rows pruned.

use super::*;

fn found(text: &str) -> ChangelogRecord {
    ChangelogRecord {
        status: RecordStatus::Found,
        file: Some("rten-0.27.0/CHANGELOG.md".into()),
        text: Some(text.into()),
        reason: None,
    }
}

#[test]
fn a_record_round_trips() {
    let db = crate::test_utils::test_db();
    let rec = found("## 0.27.0\n- Removed `x`\n\n## 0.26.0\n- Fixed y\n");
    assert!(!has(&db, Ecosystem::Crates, "rten", "0.27.0"));
    put(&db, Ecosystem::Crates, "rten", "0.27.0", &rec).unwrap();
    assert_eq!(
        get(&db, Ecosystem::Crates, "rten", "0.27.0").unwrap(),
        Some(rec)
    );
    assert!(has(&db, Ecosystem::Crates, "rten", "0.27.0"));
    // Another version, or the same name on npm, is a different archive.
    assert!(!has(&db, Ecosystem::Crates, "rten", "0.26.0"));
    assert!(!has(&db, Ecosystem::Npm, "rten", "0.27.0"));
}

#[test]
fn a_large_changelog_round_trips_whole() {
    // The cache keeps the text, never a capped parse: a first read and a
    // cached read must agree however long the changelog is.
    let db = crate::test_utils::test_db();
    let text = (0..3000)
        .map(|i| format!("## 1.{i}.0\n- Fixed thing {i}\n\n"))
        .collect::<Vec<_>>()
        .concat();
    put(&db, Ecosystem::Npm, "ai", "7.0.127", &found(&text)).unwrap();
    let got = get(&db, Ecosystem::Npm, "ai", "7.0.127").unwrap().unwrap();
    assert_eq!(got.text.as_deref(), Some(text.as_str()));
}

#[test]
fn crate_names_fold_dash_underscore_and_case() {
    let db = crate::test_utils::test_db();
    put(
        &db,
        Ecosystem::Crates,
        "Hyper_Util",
        "0.1.21",
        &found("## 0.1.21\n- x\n"),
    )
    .unwrap();
    assert!(has(&db, Ecosystem::Crates, "hyper-util", "0.1.21"));
}

#[test]
fn a_refusal_is_cached_with_its_reason() {
    let db = crate::test_utils::test_db();
    let rec = ChangelogRecord {
        status: RecordStatus::Refused,
        file: None,
        text: None,
        reason: Some("archive is 9360572 bytes compressed (cap 5242880)".into()),
    };
    put(&db, Ecosystem::Crates, "windows", "0.62.2", &rec).unwrap();
    assert_eq!(
        get(&db, Ecosystem::Crates, "windows", "0.62.2").unwrap(),
        Some(rec)
    );
}

#[test]
fn a_later_write_replaces_the_row() {
    let db = crate::test_utils::test_db();
    put(&db, Ecosystem::Npm, "stripe", "23.0.0", &found("old")).unwrap();
    let newer = found("## 23.0.0\n- new\n");
    put(&db, Ecosystem::Npm, "stripe", "23.0.0", &newer).unwrap();
    assert_eq!(
        get(&db, Ecosystem::Npm, "stripe", "23.0.0").unwrap(),
        Some(newer)
    );
}

#[test]
fn prune_drops_only_old_rows() {
    let db = crate::test_utils::test_db();
    put(&db, Ecosystem::Npm, "fresh", "1.0.0", &found("x")).unwrap();
    put(&db, Ecosystem::Npm, "stale", "1.0.0", &found("x")).unwrap();
    db.conn
        .lock()
        .execute(
            "UPDATE release_changelogs SET fetched_at = datetime('now', '-200 days') WHERE package = 'stale'",
            [],
        )
        .unwrap();
    assert_eq!(prune(&db).unwrap(), 1);
    assert!(has(&db, Ecosystem::Npm, "fresh", "1.0.0"));
    assert!(!has(&db, Ecosystem::Npm, "stale", "1.0.0"));
}

#[test]
fn ensure_table_is_idempotent() {
    let db = crate::test_utils::test_db();
    let conn = db.conn.lock();
    ensure_table(&conn).unwrap();
    ensure_table(&conn).unwrap();
}
