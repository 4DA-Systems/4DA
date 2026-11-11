// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The engine cycle's Upgrade Plan snapshot refresh (step 3b): a plan that
//! could not be read is never written over the last good one (audit
//! 2026-11-11).

use super::refresh_plan_snapshot;
use crate::db::Database;
use crate::test_utils::test_db;

fn stored_plan(db: &Database) -> Option<String> {
    db.get_kv("upgrade_plan_snapshot").expect("kv read")
}

#[test]
fn a_failed_plan_read_keeps_the_last_good_snapshot() {
    let db = test_db();
    db.store_dependency("/proj/a", "lodash", Some("4.17.20"), "npm", false, None)
        .expect("dependency");
    db.upsert_osv_advisory(
        "GHSA-engine-0001",
        "Vulnerability in lodash",
        None,
        "lodash",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
        Some(r#"["4.17.21"]"#),
        Some("CVSS_V3"),
        Some(7.5),
        Some("https://osv.dev/GHSA-engine-0001"),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .expect("advisory");

    let steps = refresh_plan_snapshot(&db, Some(7)).expect("the plan builds");
    assert_eq!(steps, 1);
    let good = stored_plan(&db).expect("the built plan is persisted");
    assert!(good.contains("lodash") && good.contains(r#""engine_run_id":7"#));

    // The advisory mirror becomes unreadable (a damaged or half-migrated
    // database): the next cycle's refresh must fail, not write an empty plan.
    db.conn
        .lock()
        .execute_batch("DROP TABLE osv_advisories")
        .expect("drop the advisory mirror");
    refresh_plan_snapshot(&db, Some(8))
        .expect_err("a plan that could not be read is an error, not an empty plan");
    assert_eq!(
        stored_plan(&db).as_deref(),
        Some(good.as_str()),
        "the last good snapshot must survive a failed read, byte for byte"
    );
}

/// A plan that was built and is empty is still written: "evaluated, nothing
/// to do" stays distinguishable from "never computed".
#[test]
fn an_empty_plan_that_was_read_is_still_written() {
    let db = test_db();
    assert_eq!(refresh_plan_snapshot(&db, None).expect("an empty plan"), 0);
    let stored = stored_plan(&db).expect("the empty plan is persisted");
    assert!(stored.contains(r#""item_count":0"#), "{stored}");
}
