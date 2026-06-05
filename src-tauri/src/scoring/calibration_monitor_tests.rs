// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the per-developer calibration monitor.

use super::compute_calibration_snapshot;
use crate::test_utils::{insert_test_item, test_db};

/// Set a scored item's relevance + optional high-stakes markers.
fn set_score(
    db: &crate::db::Database,
    id: i64,
    score: f64,
    content_type: Option<&str>,
    cve: Option<&str>,
) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items SET relevance_score=?2, content_type=?3, cve_ids=?4 WHERE id=?1",
        rusqlite::params![id, score, content_type, cve],
    )
    .unwrap();
}

#[test]
fn cold_start_no_feedback_is_safe() {
    let db = test_db();
    let snap = compute_calibration_snapshot(&db, 0.4).unwrap();
    assert_eq!(snap.feedback_count, 0);
    assert!(!snap.has_sufficient_feedback);
    assert_eq!(snap.precision_miss_rate, 0.0);
    assert_eq!(snap.recall_miss_rate, 0.0);
    assert_eq!(snap.discrimination, 0.0);
    // No feedback → calibration is honestly unassessable (silent at cold start).
    assert_eq!(snap.health(), None);
}

#[test]
fn precision_and_recall_misses_are_detected_from_feedback() {
    let db = test_db();

    // Engaged (relevant=1) but scored as noise → RECALL miss.
    let a = insert_test_item(&db, "hackernews", "a", "engaged but buried", "x");
    set_score(&db, a, 0.02, None, None);
    db.record_feedback(a, true).unwrap();
    // Engaged and scored high → correct.
    let b = insert_test_item(&db, "hackernews", "b", "engaged and surfaced", "x");
    set_score(&db, b, 0.70, None, None);
    db.record_feedback(b, true).unwrap();

    // Dismissed (relevant=0) but scored high → PRECISION miss.
    let c = insert_test_item(&db, "reddit", "c", "surfaced but unwanted", "x");
    set_score(&db, c, 0.80, None, None);
    db.record_feedback(c, false).unwrap();
    // Dismissed and scored low → correct.
    let d = insert_test_item(&db, "reddit", "d", "correctly buried", "x");
    set_score(&db, d, 0.03, None, None);
    db.record_feedback(d, false).unwrap();

    let snap = compute_calibration_snapshot(&db, 0.4).unwrap();
    assert_eq!(snap.feedback_count, 4);
    assert!(
        (snap.recall_miss_rate - 0.5).abs() < 1e-6,
        "1 of 2 engaged scored low"
    );
    assert!(
        (snap.precision_miss_rate - 0.5).abs() < 1e-6,
        "1 of 2 dismissed scored high"
    );
}

#[test]
fn discrimination_is_positive_when_scorer_separates_well() {
    let db = test_db();
    for (i, score) in [0.80_f64, 0.70, 0.65].iter().enumerate() {
        let id = insert_test_item(&db, "hackernews", &format!("eng{i}"), "engaged", "x");
        set_score(&db, id, *score, None, None);
        db.record_feedback(id, true).unwrap();
    }
    for (i, score) in [0.05_f64, 0.10, 0.08].iter().enumerate() {
        let id = insert_test_item(&db, "reddit", &format!("dis{i}"), "dismissed", "x");
        set_score(&db, id, *score, None, None);
        db.record_feedback(id, false).unwrap();
    }
    let snap = compute_calibration_snapshot(&db, 0.4).unwrap();
    // engaged avg ~0.717, dismissed avg ~0.077 → discrimination ~0.64, clearly positive.
    assert!(snap.discrimination > 0.5, "got {}", snap.discrimination);
    assert_eq!(snap.recall_miss_rate, 0.0);
    assert_eq!(snap.precision_miss_rate, 0.0);
}

#[test]
fn sufficient_feedback_threshold_gates_trust_and_health() {
    let db = test_db();
    // 12 clean feedback events (engaged high, dismissed low) → measurable + healthy.
    for i in 0..6 {
        let e = insert_test_item(&db, "hackernews", &format!("e{i}"), "engaged", "x");
        set_score(&db, e, 0.7, None, None);
        db.record_feedback(e, true).unwrap();
        let d = insert_test_item(&db, "reddit", &format!("d{i}"), "dismissed", "x");
        set_score(&db, d, 0.05, None, None);
        db.record_feedback(d, false).unwrap();
    }
    let snap = compute_calibration_snapshot(&db, 0.4).unwrap();
    assert_eq!(snap.feedback_count, 12);
    assert!(
        snap.has_sufficient_feedback,
        "12 >= MIN_FEEDBACK_FOR_METRICS"
    );
    // Clean separation → health near 1.0 and now Some(_).
    let health = snap.health().expect("measurable with 12 feedback");
    assert!(health > 0.99, "got {health}");
}
