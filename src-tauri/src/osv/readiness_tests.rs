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
        not_covered: None,
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

/// Audit A2/E4 (2026-10-12): a Maven-only project read as "no dependencies"
/// and a mixed one as fully covered. The walk's unsupported files reach the
/// readiness as `not_covered`, whatever the sync state.
#[test]
fn readiness_names_the_files_4da_does_not_read() {
    let db = crate::test_utils::test_db();
    db.set_kv(
        crate::ace::lockfile::report::REPORT_KV_KEY,
        r#"{"lockfiles_read":1,"unsupported_count":3,"unsupported":[
            {"path":"C:/p/spring-petclinic/pom.xml","reason":"Maven (installed versions not resolved)"},
            {"path":"C:/p/api/build.gradle.kts","reason":"Gradle (installed versions not resolved)"},
            {"path":"C:/p/svc/packages.lock.json","reason":"NuGet lockfile"}],
           "failed_count":1,"failed":[
            {"path":"C:/p/web/package-lock.json","reason":"invalid JSON"}]}"#,
    )
    .unwrap();
    db.store_dependency("/project", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    db.update_osv_sync_status("crates.io", 0, None).unwrap();

    let r = advisory_sync(&db);
    assert_eq!(
        r.state,
        AdvisorySyncState::Ready,
        "the read part is checked"
    );
    let nc = r.not_covered.expect("unsupported files are named");
    assert_eq!(nc.count, 3);
    let ecosystems: Vec<&str> = nc.files.iter().map(|f| f.ecosystem.as_str()).collect();
    assert_eq!(ecosystems, vec!["Maven", "Gradle", "NuGet lockfile"]);
    assert!(nc.files[0].path.ends_with("pom.xml"));
    // A lockfile that failed to read is not covered either: an all-clear
    // would rest on whatever it pinned last time.
    assert_eq!(nc.unreadable_count, 1);
    assert_eq!(nc.unreadable[0].reason, "invalid JSON");
}

#[test]
fn readiness_has_no_not_covered_when_every_file_was_read() {
    let db = crate::test_utils::test_db();
    db.set_kv(
        crate::ace::lockfile::report::REPORT_KV_KEY,
        r#"{"lockfiles_read":2,"unsupported_count":0,"unsupported":[]}"#,
    )
    .unwrap();
    assert_eq!(advisory_sync(&db).not_covered, None);
    // And an older binary's snapshot without the field still deserializes.
    let old: AdvisorySync =
        serde_json::from_str(r#"{"state":"ready","ecosystems":1,"synced_at":null,"error":null}"#)
            .unwrap();
    assert_eq!(old.not_covered, None);
}

const MAVEN_REPORT: &str = r#"{"lockfiles_read":0,"unsupported_count":1,"unsupported":[
    {"path":"C:/p/spring-petclinic/pom.xml","reason":"Maven (installed versions not resolved)"}]}"#;

/// W5a: the manifest scanner registers a pom.xml's dependencies with no
/// version, and a versionless dependency is never matched (#974). They made
/// "Maven" a required ecosystem, its sync succeeded, and readiness said Ready
/// for a project nothing was checked in. Only a VERSIONED row makes an
/// ecosystem required: a Maven-only project has no inventory, and the
/// not-covered list names its pom.xml.
#[test]
fn a_maven_only_project_has_nothing_checked_and_names_its_pom() {
    let db = crate::test_utils::test_db();
    db.set_kv(crate::ace::lockfile::report::REPORT_KV_KEY, MAVEN_REPORT)
        .unwrap();
    db.store_dependency(
        "/spring-petclinic",
        "org.springframework.boot:spring-boot-starter-web",
        None,
        "java",
        false,
        None,
    )
    .unwrap();
    db.update_osv_sync_status("Maven", 0, None).unwrap();

    let r = advisory_sync(&db);
    assert_eq!(
        r.state,
        AdvisorySyncState::NoInventory,
        "nothing was checked"
    );
    assert_eq!(r.ecosystems, 0);
    let nc = r.not_covered.expect("the pom.xml is named");
    assert_eq!(nc.files[0].ecosystem, "Maven");
    assert!(nc.files[0].path.ends_with("pom.xml"));
}

/// The read part of a mixed machine is still Ready, and the Maven part is
/// named, not counted as covered.
#[test]
fn npm_and_maven_read_ready_for_npm_and_name_the_maven_files() {
    let db = crate::test_utils::test_db();
    db.set_kv(crate::ace::lockfile::report::REPORT_KV_KEY, MAVEN_REPORT)
        .unwrap();
    db.store_dependency("/web", "minimist", Some("1.2.5"), "javascript", false, None)
        .unwrap();
    db.store_dependency(
        "/spring-petclinic",
        "org.springframework.boot:spring-boot-starter-web",
        None,
        "java",
        false,
        None,
    )
    .unwrap();
    db.update_osv_sync_status("npm", 0, None).unwrap();

    let r = advisory_sync(&db);
    assert_eq!(r.state, AdvisorySyncState::Ready);
    assert_eq!(r.ecosystems, 1, "only npm is required: {r:?}");
    let nc = r.not_covered.expect("the pom.xml is named");
    assert_eq!(nc.files[0].ecosystem, "Maven");
}

/// #987 review S3 (cold start): the manifest scan registers dependencies with
/// no version before the first lockfile walk reports. That is "not read yet":
/// never "nothing checkable" to a first-run user.
#[test]
fn before_the_first_lockfile_walk_versionless_rows_are_not_started() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/web", "minimist", None, "javascript", false, None)
        .unwrap();
    let r = advisory_sync(&db);
    assert_eq!(r.state, AdvisorySyncState::NotStarted, "no walk report yet");
    // Once the walk has reported, the same rows are what was read: nothing
    // versioned, so nothing to check.
    db.set_kv(
        crate::ace::lockfile::report::REPORT_KV_KEY,
        r#"{"lockfiles_read":0}"#,
    )
    .unwrap();
    assert_eq!(advisory_sync(&db).state, AdvisorySyncState::NoInventory);
}

// --- Per-project coverage (W0-D, 2026-11-11) ------------------------------
// Coverage was judged per ECOSYSTEM: one pinned project made "PyPI" ready,
// and everything the walk saw but did not check (fixture lockfiles, ranged
// requirements, manifests with no lockfile, a sibling project's versionless
// rows) vanished from what Preemption showed beside its all-clear.

/// What the walk saw and did not check reaches `not_covered`, with counts and
/// names, while the checked part stays Ready.
#[test]
fn what_the_walk_did_not_check_is_named_beside_a_ready_check() {
    let db = crate::test_utils::test_db();
    db.set_kv(
        crate::ace::lockfile::report::REPORT_KV_KEY,
        r#"{"lockfiles_read":2,"unsupported_count":0,"failed_count":0,
           "skipped_lockfile_count":2,"skipped_lockfiles":[
            {"path":"C:\\dev\\hono\\benchmarks\\jsx\\yarn.lock","reason":"example / fixture / scaffolding path"},
            {"path":"C:\\dev\\hono\\benchmarks\\routers\\yarn.lock","reason":"example / fixture / scaffolding path"}],
           "unpinned_count":3,"unpinned":[{"path":"C:\\dev\\svc\\requirements.txt","count":3}],
           "manifest_only_count":1,"manifest_only_dependencies":12,
           "manifest_only":[{"path":"C:\\dev\\tool\\package.json","ecosystem":"javascript","dependencies":12}]}"#,
    )
    .unwrap();
    db.store_dependency("/project", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    db.update_osv_sync_status("crates.io", 0, None).unwrap();

    let r = advisory_sync(&db);
    assert_eq!(
        r.state,
        AdvisorySyncState::Ready,
        "the read part is checked"
    );
    let nc = r
        .not_covered
        .expect("unchecked dependencies are never clean");
    assert_eq!(nc.skipped_count, Some(2));
    let skipped = nc.skipped.expect("skipped lockfiles are named");
    assert!(skipped[0].path.ends_with("yarn.lock"));
    assert_eq!(skipped[0].reason, "example / fixture / scaffolding path");
    assert_eq!(nc.unpinned_count, Some(3));
    assert_eq!(
        nc.unpinned,
        Some(vec![CountedFile {
            path: "C:\\dev\\svc\\requirements.txt".into(),
            count: 3
        }])
    );
    assert_eq!(nc.manifest_only_count, Some(1));
    assert_eq!(nc.manifest_only.expect("named")[0].count, 12);
    assert_eq!(nc.count, 0);
    assert_eq!(nc.unversioned_count, None);
}

/// One project's pins no longer make a sibling's versionless dependencies
/// read as checked: each project's unversioned rows are counted.
#[test]
fn a_sibling_projects_pins_do_not_cover_a_versionless_project() {
    let db = crate::test_utils::test_db();
    db.set_kv(
        crate::ace::lockfile::report::REPORT_KV_KEY,
        r#"{"lockfiles_read":1}"#,
    )
    .unwrap();
    db.store_dependency("c:/dev/a", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    db.store_dependency("c:/dev/b", "tokio", None, "rust", false, None)
        .unwrap();
    db.store_dependency("c:/dev/b", "anyhow", None, "rust", false, None)
        .unwrap();
    // The same name pinned in its own project is covered.
    db.store_dependency("c:/dev/a", "anyhow", Some("1.0.80"), "rust", false, None)
        .unwrap();
    db.update_osv_sync_status("crates.io", 0, None).unwrap();

    let r = advisory_sync(&db);
    assert_eq!(r.state, AdvisorySyncState::Ready);
    let nc = r
        .not_covered
        .expect("project b has nothing an advisory can be matched against");
    assert_eq!(nc.unversioned_count, Some(2));
    let projects = nc.unversioned.expect("named");
    assert_eq!(projects.len(), 1);
    assert!(projects[0].path.ends_with("/b"), "{projects:?}");
    assert_eq!(projects[0].count, 2);
}

/// A project the report already names (a manifest with no lockfile) is not
/// counted again from its versionless rows.
#[test]
fn a_manifest_only_project_is_named_once() {
    let saved: serde_json::Value = serde_json::from_str(
        r#"{"manifest_only_count":1,"manifest_only":[
            {"path":"C:\\dev\\tool\\package.json","ecosystem":"javascript","dependencies":2}]}"#,
    )
    .unwrap();
    let nc = not_covered_with(
        Some(&saved),
        &[
            ("c:/dev/tool".to_string(), 2),
            ("c:/dev/other".to_string(), 1),
        ],
    )
    .expect("something unchecked");
    assert_eq!(nc.manifest_only_count, Some(1));
    assert_eq!(nc.unversioned_count, Some(1), "{nc:?}");
    assert!(nc.unversioned.expect("named")[0].path.ends_with("other"));
    // Nothing unchecked anywhere: no record at all.
    assert_eq!(not_covered_with(None, &[]), None);
    assert_eq!(not_covered_with(None, &[("c:/x".to_string(), 0)]), None);
}

/// A feed stamped by a build before W0-D still deserializes.
#[test]
fn a_not_covered_record_without_the_w0d_fields_still_reads() {
    let old: NotCovered = serde_json::from_str(
        r#"{"count":1,"files":[{"path":"a/pom.xml","ecosystem":"Maven"}],"unreadable_count":0,"unreadable":[]}"#,
    )
    .unwrap();
    assert_eq!(old.skipped_count, None);
    assert_eq!(old.unversioned, None);
}

// --- Advisory freshness truth (2026-11-11) --------------------------------
// With the network blocked, a sync re-read its cached archives and recorded
// every ecosystem as synced "now" with no error, so the mirror could never
// read as failed or stale. The sync time is now the last VERIFIED sync, a
// failed check keeps its error, and readiness says how old the data is.

/// A failed check does not move the last verified time, and readiness reports
/// that time as the age of the data still being served.
#[test]
fn a_failed_pass_keeps_the_last_verified_time_and_readiness_reports_it() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/project", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    db.update_osv_sync_status("crates.io", 12, None).unwrap();
    db.conn
        .lock()
        .execute(
            "UPDATE osv_sync_status SET last_synced_at = '2026-11-08 02:00:00'",
            [],
        )
        .unwrap();

    db.update_osv_sync_status("crates.io", 12, Some("could not reach the advisory host"))
        .unwrap();

    let rows = db.get_osv_sync_statuses().unwrap();
    assert_eq!(
        rows[0].last_synced_at.as_deref(),
        Some("2026-11-08 02:00:00"),
        "a pass that verified nothing must not look like a sync"
    );
    assert_eq!(
        rows[0].error.as_deref(),
        Some("could not reach the advisory host")
    );
    let r = advisory_sync(&db);
    assert_eq!(r.state, AdvisorySyncState::Failed);
    assert_eq!(r.synced_at.as_deref(), Some("2026-11-08 02:00:00"));
    assert!(
        crate::osv::sync::needs_sync(&db, crate::osv::sync::DEFAULT_SYNC_MAX_AGE_HOURS).unwrap(),
        "an unverified mirror keeps being retried"
    );
}

/// The age a failed state reports is the oldest verified sync among the
/// ecosystems the inventory needs, the failing one included.
#[test]
fn a_failed_check_reports_the_age_of_the_data_it_still_serves() {
    let r = classify(
        &req(&["crates.io", "npm"]),
        &[
            row("crates.io", Some("2026-10-12 06:00:00"), None),
            row("npm", Some("2026-10-09 11:00:00"), Some("could not check")),
        ],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::Failed);
    assert_eq!(r.error.as_deref(), Some("npm: could not check"));
    assert_eq!(
        r.synced_at.as_deref(),
        Some("2026-10-09 11:00:00"),
        "the failing ecosystem's last verified sync is the floor"
    );
}

/// A failing ecosystem that never had a verified sync has no data to date:
/// another ecosystem's fresh sync must not be shown as its age.
#[test]
fn a_failed_check_with_no_verified_data_claims_no_age() {
    let r = classify(
        &req(&["crates.io", "npm"]),
        &[
            row("crates.io", Some("2026-10-12 06:00:00"), None),
            row("npm", None, Some("offline on first run")),
        ],
        now(),
        false,
    );
    assert_eq!(r.state, AdvisorySyncState::Failed);
    assert_eq!(r.synced_at, None);
}
