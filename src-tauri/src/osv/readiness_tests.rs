// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use chrono::NaiveDateTime;

use super::*;

fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-10-12 12:00:00", SYNC_TIME_FORMAT).unwrap()
}

fn row(ecosystem: &str, at: Option<&str>, error: Option<&str>) -> SyncStatus {
    SyncStatus {
        ecosystem: ecosystem.to_string(),
        last_synced_at: at.map(str::to_string),
        advisory_count: 0,
        error: error.map(str::to_string),
    }
}

fn req(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn no_inventory_when_no_ecosystem_is_required() {
    let r = classify(
        &[],
        &[row("npm", Some("2026-10-12 11:00:00"), None)],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::NoInventory);
    assert_eq!(r.ecosystems, 0);
}

/// Fresh-profile audit E2: "syncing" for 8.5 h while no sync ran. A never-
/// synced ecosystem is Syncing only while a sync actually runs.
#[test]
fn never_synced_is_not_started_until_a_sync_runs() {
    let rows = [row("npm", Some("2026-10-12 11:00:00"), None)];
    let idle = classify(&req(&["crates.io", "npm"]), &rows, now(), false);
    assert_eq!(idle.state, AdvisorySyncState::NotStarted);
    assert_eq!(idle.ecosystems, 2);
    assert!(!idle.in_flight);
    let running = classify(&req(&["crates.io", "npm"]), &rows, now(), true);
    assert_eq!(running.state, AdvisorySyncState::Syncing);
    assert!(running.in_flight);
}

#[test]
fn a_failure_being_retried_stays_failed_and_says_a_sync_runs() {
    let r = classify(
        &req(&["npm"]),
        &[row("npm", Some("2026-10-12 11:00:00"), Some("HTTP 503"))],
        now(),
        true,
    );
    assert_eq!(r.state, AdvisorySyncState::Failed);
    assert!(r.in_flight);
}

#[test]
fn failed_outranks_never_synced_and_carries_the_error() {
    let r = classify(
        &req(&["crates.io", "npm", "PyPI"]),
        &[row("npm", Some("2026-10-12 11:00:00"), Some("HTTP 503"))],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::Failed);
    assert_eq!(r.error.as_deref(), Some("npm: HTTP 503"));
}

#[test]
fn ready_when_every_ecosystem_synced_within_the_horizon_even_with_zero_advisories() {
    // A clean project: the sync stored nothing because nothing applies.
    let r = classify(
        &req(&["crates.io", "npm"]),
        &[
            row("crates.io", Some("2026-10-12 06:00:00"), None),
            row("npm", Some("2026-10-11 20:00:00"), None),
        ],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::Ready);
    assert_eq!(
        r.synced_at.as_deref(),
        Some("2026-10-11 20:00:00"),
        "the oldest sync is the floor"
    );
    assert!(r.error.is_none());
}

#[test]
fn stale_when_the_oldest_sync_is_past_the_trust_horizon() {
    let r = classify(
        &req(&["crates.io", "npm"]),
        &[
            row("crates.io", Some("2026-10-12 06:00:00"), None),
            row("npm", Some("2026-10-09 11:00:00"), None),
        ],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::Stale);
}

#[test]
fn ecosystem_names_match_case_insensitively_and_unrequired_rows_are_ignored() {
    let r = classify(
        &req(&["PyPI"]),
        &[
            row("pypi", Some("2026-10-12 10:00:00"), None),
            row("npm", None, Some("unrelated failure")),
        ],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::Ready);
}

#[test]
fn a_row_without_a_timestamp_is_not_a_sync() {
    let r = classify(&req(&["npm"]), &[row("npm", None, None)], now(), false);
    assert_eq!(r.state, AdvisorySyncState::NotStarted);
}

fn state(s: AdvisorySyncState) -> AdvisorySync {
    AdvisorySync {
        state: s,
        ecosystems: 1,
        synced_at: None,
        error: None,
        in_flight: false,
        skipped_temp: 0,
    }
}

#[test]
fn overlay_reports_matching_when_the_feed_predates_the_ready_data() {
    for before in [
        AdvisorySyncState::NotStarted,
        AdvisorySyncState::Syncing,
        AdvisorySyncState::Failed,
        AdvisorySyncState::Stale,
        AdvisorySyncState::NoInventory,
    ] {
        let out = overlay(Some(&state(before)), state(AdvisorySyncState::Ready));
        assert_eq!(out.state, AdvisorySyncState::Matching, "stamped {before:?}");
    }
}

#[test]
fn overlay_lets_a_live_problem_win_over_a_ready_stamp() {
    let out = overlay(
        Some(&state(AdvisorySyncState::Ready)),
        state(AdvisorySyncState::Failed),
    );
    assert_eq!(out.state, AdvisorySyncState::Failed);
}

#[test]
fn overlay_without_a_stamp_uses_the_live_state() {
    assert_eq!(
        overlay(None, state(AdvisorySyncState::Ready)).state,
        AdvisorySyncState::Ready
    );
    assert_eq!(
        overlay(
            Some(&state(AdvisorySyncState::Ready)),
            state(AdvisorySyncState::Ready)
        )
        .state,
        AdvisorySyncState::Ready
    );
}

#[test]
fn live_readiness_on_an_empty_database_has_nothing_to_check() {
    let db = crate::test_utils::test_db();
    assert_eq!(advisory_sync(&db).state, AdvisorySyncState::NoInventory);
}

#[test]
fn live_readiness_follows_the_sync_status_of_the_inventorys_ecosystems() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/project", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    let before = advisory_sync(&db);
    assert_eq!(
        before.state,
        AdvisorySyncState::NotStarted,
        "a lockfile read, no sync yet"
    );
    assert_eq!(before.ecosystems, 1);

    db.update_osv_sync_status("crates.io", 0, Some("network down"))
        .unwrap();
    assert_eq!(advisory_sync(&db).state, AdvisorySyncState::Failed);

    // Zero advisories stored is still a successful sync: nothing applies.
    db.update_osv_sync_status("crates.io", 0, None).unwrap();
    assert_eq!(advisory_sync(&db).state, AdvisorySyncState::Ready);
}

/// E6: when every project the walk found was a temp copy, the readiness says
/// so (the notice names the reason) instead of reading as missing coverage.
#[test]
fn no_inventory_carries_the_walks_temp_skips() {
    let db = crate::test_utils::test_db();
    db.set_kv(
        crate::ace::lockfile::report::REPORT_KV_KEY,
        r#"{"lockfiles_read":0,"skipped_temp_count":2}"#,
    )
    .unwrap();
    let r = advisory_sync(&db);
    assert_eq!(r.state, AdvisorySyncState::NoInventory);
    assert_eq!(r.skipped_temp, 2);
}
