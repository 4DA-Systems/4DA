// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The stored plan is never fresher than the advisory data it was built from:
//! `expires_at` follows the oldest verified sync of the ecosystems the
//! inventory needs (`source_freshness`), not only the time the plan was
//! written. The engine rewrites the plan every cycle, so a plan stamped
//! `generated_at + 6 h` from a five-day-old mirror read as fresh to every
//! reader.

use chrono::{DateTime, Duration, Utc};

use super::{persist_upgrade_plan, plan_expiry, read_upgrade_plan_snapshot};
use crate::db::Database;
use crate::test_utils::test_db;

const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

fn at(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .expect("rfc3339")
        .with_timezone(&Utc)
}

fn sync_time(ago: Duration) -> String {
    (Utc::now() - ago).format(FORMAT).to_string()
}

fn max_age() -> Duration {
    Duration::hours(crate::osv::sync::osv_sync_max_age_hours())
}

/// Move an ecosystem's verified sync time, as an old mirror leaves it.
fn set_synced(db: &Database, ecosystem: &str, at: &str) {
    db.conn
        .lock()
        .execute(
            "UPDATE osv_sync_status SET last_synced_at = ?2 WHERE ecosystem = ?1",
            rusqlite::params![ecosystem, at],
        )
        .expect("status row");
}

/// An inventory that needs npm advisories, and the plan persisted over it.
fn npm_inventory() -> Database {
    let db = test_db();
    db.store_dependency("/proj/a", "lodash", Some("4.17.20"), "npm", false, None)
        .expect("dependency");
    db
}

fn persisted(db: &Database) -> super::super::types::UpgradePlanSnapshot {
    persist_upgrade_plan(db, &[], &[], 0, None);
    read_upgrade_plan_snapshot(db).expect("snapshot")
}

#[test]
fn fresh_advisory_data_keeps_the_sync_horizon() {
    let generated = Utc::now();
    let synced = generated - Duration::minutes(5);
    let text = synced.format(FORMAT).to_string();
    let expires = plan_expiry(generated, Some(&text), Duration::hours(6));
    assert!(expires > generated, "fresh data: the plan is fresh");
    assert!(expires <= generated + Duration::hours(6));
    // Due when the data it read falls due for a sync, to the second.
    assert_eq!(
        expires.timestamp(),
        (synced + Duration::hours(6)).timestamp()
    );
}

#[test]
fn five_day_old_advisory_data_is_expired_when_written() {
    let generated = Utc::now();
    let text = (generated - Duration::days(5)).format(FORMAT).to_string();
    let expires = plan_expiry(generated, Some(&text), Duration::hours(6));
    assert!(
        expires < generated,
        "a plan built from a five-day-old mirror is already expired: {expires} vs {generated}"
    );
}

#[test]
fn missing_or_unreadable_freshness_expires_at_generation() {
    let generated = Utc::now();
    assert_eq!(plan_expiry(generated, None, Duration::hours(6)), generated);
    assert_eq!(
        plan_expiry(generated, Some("yesterday"), Duration::hours(6)),
        generated
    );
}

#[test]
fn a_sync_time_ahead_of_the_clock_never_extends_the_horizon() {
    let generated = Utc::now();
    let text = (generated + Duration::days(1)).format(FORMAT).to_string();
    assert_eq!(
        plan_expiry(generated, Some(&text), Duration::hours(6)),
        generated + Duration::hours(6)
    );
}

#[test]
fn a_persisted_plan_over_a_fresh_mirror_is_fresh() {
    let db = npm_inventory();
    db.update_osv_sync_status("npm", 10, None).expect("sync");
    let snap = persisted(&db);
    assert!(snap.source_freshness.is_some());
    assert!(
        at(&snap.expires_at) > at(&snap.generated_at),
        "{} vs {}",
        snap.expires_at,
        snap.generated_at
    );
    assert!(at(&snap.expires_at) <= at(&snap.generated_at) + max_age());
}

#[test]
fn a_persisted_plan_over_a_five_day_old_mirror_is_already_expired() {
    let db = npm_inventory();
    db.update_osv_sync_status("npm", 10, None).expect("sync");
    let old = sync_time(Duration::days(5));
    set_synced(&db, "npm", &old);
    let snap = persisted(&db);
    assert_eq!(snap.source_freshness.as_deref(), Some(old.as_str()));
    assert!(
        at(&snap.expires_at) < at(&snap.generated_at),
        "expired when written: {} vs {}",
        snap.expires_at,
        snap.generated_at
    );
}

#[test]
fn a_persisted_plan_with_no_verified_advisory_data_is_already_expired() {
    // No sync ever: nothing dates the data the plan read.
    let db = npm_inventory();
    let snap = persisted(&db);
    assert!(snap.source_freshness.is_none());
    assert_eq!(at(&snap.expires_at), at(&snap.generated_at));

    // A failed first pass leaves an error and no time: still nothing to date.
    db.record_osv_sync_unverified("npm", None, "offline", None)
        .expect("status");
    let snap = persisted(&db);
    assert!(snap.source_freshness.is_none());
    assert_eq!(at(&snap.expires_at), at(&snap.generated_at));
}

#[test]
fn one_never_verified_ecosystem_leaves_no_floor() {
    let db = npm_inventory();
    db.store_dependency("/proj/a", "serde", Some("1.0.0"), "rust", false, None)
        .expect("dependency");
    db.update_osv_sync_status("npm", 10, None).expect("sync");
    let snap = persisted(&db);
    assert!(
        snap.source_freshness.is_none(),
        "fresh npm data does not date the crates.io half: {:?}",
        snap.source_freshness
    );
    assert_eq!(at(&snap.expires_at), at(&snap.generated_at));
}

#[test]
fn an_ecosystem_the_inventory_no_longer_needs_does_not_date_the_plan() {
    let db = npm_inventory();
    db.update_osv_sync_status("npm", 10, None).expect("sync");
    db.update_osv_sync_status("Go", 3, None).expect("sync");
    set_synced(&db, "Go", &sync_time(Duration::days(120)));
    let snap = persisted(&db);
    assert!(
        at(&snap.expires_at) > at(&snap.generated_at),
        "a months-old Go row with no Go dependency left: {:?}",
        snap.source_freshness
    );
}
