// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Host-reachability tests for the dependency tables (2026-09-07 audit).
//!
//! Split out of `tests.rs` to keep that file under the Rust size ceiling —
//! the `*_tests.rs` convention this directory already uses.

use crate::test_utils::test_db;

#[test]
fn lockfile_only_crates_are_marked_and_unmarked_by_the_next_scan() {
    // 2026-09-07: `quinn-proto` was a HIGH "version-confirmed" advisory in
    // D:\4DA — a crate in Cargo.lock that no feature this tree enables ever
    // compiles. It is a TRANSITIVE, so it exists only in user_dependencies,
    // where nothing could record a platform verdict before Phase 122.
    let db = test_db();
    db.store_transitive_dependency("/app", "quinn-proto", Some("0.11.8"), "rust", false)
        .unwrap();
    db.store_transitive_dependency("/app", "serde", Some("1.0.219"), "rust", false)
        .unwrap();

    let marked = db
        .mark_lockfile_only_crates("/app", "rust", &["quinn-proto".to_string()])
        .unwrap();
    assert_eq!(marked, 1);

    let inactive = db.platform_inactive_packages();
    assert!(inactive.contains("quinn-proto"), "the phantom is flagged");
    assert!(
        !inactive.contains("serde"),
        "a crate the host DOES build is untouched"
    );

    // Self-correcting: enable the feature (or switch platform) and the next
    // scan reports an empty inactive list, which must CLEAR the marker rather
    // than leave the crate suppressed forever.
    let marked = db.mark_lockfile_only_crates("/app", "rust", &[]).unwrap();
    assert_eq!(marked, 0);
    assert!(
        !db.platform_inactive_packages().contains("quinn-proto"),
        "a crate that becomes reachable goes active again"
    );
}

#[test]
fn an_unknown_host_resolution_marks_nothing() {
    // `cargo_resolve` returns an empty list when cargo could not answer.
    // Empty must mean "no verdict", never "everything is unreachable" — the
    // latter would bury every advisory in the tree.
    let db = test_db();
    db.store_transitive_dependency("/app", "openssl", Some("0.10.68"), "rust", false)
        .unwrap();
    assert_eq!(
        db.mark_lockfile_only_crates("/app", "rust", &[]).unwrap(),
        0
    );
    assert!(db.platform_inactive_packages().is_empty());
}

#[test]
fn the_lockfile_only_marker_is_scoped_to_its_own_project_and_ecosystem() {
    // Two projects can resolve the same crate differently (different features,
    // different targets). Marking one must never suppress it in the other.
    let db = test_db();
    db.store_transitive_dependency("/app", "quinn-proto", Some("0.11.8"), "rust", false)
        .unwrap();
    db.store_transitive_dependency("/other", "quinn-proto", Some("0.11.8"), "rust", false)
        .unwrap();

    db.mark_lockfile_only_crates("/app", "rust", &["quinn-proto".to_string()])
        .unwrap();
    assert!(
        !db.platform_inactive_packages().contains("quinn-proto"),
        "still built in /other -> still fully urgent everywhere"
    );

    db.mark_lockfile_only_crates("/other", "rust", &["quinn-proto".to_string()])
        .unwrap();
    assert!(
        db.platform_inactive_packages().contains("quinn-proto"),
        "unreachable in EVERY project -> now de-prioritised"
    );
}
