// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! A failed read is an error, never an empty result (audit 2026-11-11).
//!
//! The Preemption feed is a claim: an empty worklist beside a `ready` advisory
//! state renders "no known vulnerabilities". Each read below used to answer a
//! database it could not read with an empty list, which then became exactly
//! that claim — and, for the Upgrade Plan, replaced the last good snapshot
//! `4da plan` and the MCP server read.

use super::*;
use crate::db::Database;
use crate::test_utils::test_db;

/// One version-confirmed advisory: `lodash` 4.17.20, fixed in 4.17.21.
fn seed_one_finding(db: &Database) {
    db.store_dependency("/proj/a", "lodash", Some("4.17.20"), "npm", false, None)
        .expect("dependency");
    db.upsert_osv_advisory(
        "GHSA-read-0001",
        "Vulnerability in lodash",
        None,
        "lodash",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
        Some(r#"["4.17.21"]"#),
        Some("CVSS_V3"),
        Some(7.5),
        Some("https://osv.dev/GHSA-read-0001"),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .expect("advisory");
}

/// Make the advisory mirror unreadable, the way a damaged or half-migrated
/// database is: every matcher read now fails.
fn break_advisory_mirror(db: &Database) {
    db.conn
        .lock()
        .execute_batch("DROP TABLE osv_advisories")
        .expect("drop the advisory mirror");
}

fn stored_plan(db: &Database) -> Option<String> {
    db.get_kv("upgrade_plan_snapshot").expect("kv read")
}

#[test]
fn an_unreadable_advisory_mirror_is_an_error_not_zero_alerts() {
    let db = test_db();
    seed_one_finding(&db);
    let alerts = osv_alerts_from_db(&db).expect("a readable mirror");
    assert_eq!(alerts.len(), 1, "the finding is there while the read works");

    break_advisory_mirror(&db);
    let err =
        osv_alerts_from_db(&db).expect_err("an unreadable mirror must not read as no advisories");
    // The view maps this wording to its database error, not the generic one.
    assert!(
        err.to_string()
            .starts_with("Database error: could not read"),
        "got: {err}"
    );
}

/// The other half of the distinction: nothing to find is still `Ok`.
#[test]
fn an_empty_but_readable_mirror_is_ok_and_empty() {
    let db = test_db();
    let alerts = osv_alerts_from_db(&db).expect("an empty mirror is readable");
    assert!(alerts.is_empty());
}

#[test]
fn a_plan_that_cannot_be_read_fails_the_feed_and_keeps_the_stored_plan() {
    let db = test_db();
    seed_one_finding(&db);
    let mut items = Vec::new();
    append_upgrade_plan_from(&db, &mut items).expect("the plan builds while the read works");
    assert_eq!(items.len(), 1, "the plan step joins the feed");
    let good = stored_plan(&db).expect("the built plan is persisted");
    assert!(good.contains("lodash"), "the stored plan names the step");

    break_advisory_mirror(&db);
    let mut items = Vec::new();
    let err = append_upgrade_plan_from(&db, &mut items)
        .expect_err("a failed read must fail the feed, not append nothing");
    assert!(err.starts_with("Database error: could not read"), "{err}");
    assert!(items.is_empty());
    assert_eq!(
        stored_plan(&db).as_deref(),
        Some(good.as_str()),
        "the last good plan must survive a failed read, byte for byte"
    );
}

/// A plan that was built and is empty is still persisted: "evaluated, nothing
/// to do" stays distinguishable from "never computed".
#[test]
fn an_empty_plan_that_was_read_is_still_persisted() {
    let db = test_db();
    let mut items = Vec::new();
    append_upgrade_plan_from(&db, &mut items).expect("an empty plan builds");
    assert!(items.is_empty());
    let stored = stored_plan(&db).expect("the empty plan is persisted");
    assert!(stored.contains(r#""item_count":0"#), "{stored}");
}
