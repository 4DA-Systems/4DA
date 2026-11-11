// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One ecosystem's pass against a local stand-in for OSV (the batch API and
//! the archive bucket), through the code that ships: what is matched, and
//! what the sync status is allowed to say afterwards.

use std::path::Path;

use super::*;
use crate::osv::cache::tests::{archive_for, seed_archive, DeadHost, FakeOsv};
use crate::osv::readiness::{advisory_sync, AdvisorySyncState};
use crate::osv::sync::{needs_sync, DEFAULT_SYNC_MAX_AGE_HOURS, RATE_LIMITED};
use crate::osv::types::SyncStatus;

/// When the archive seeded on disk was downloaded, and that time as the
/// status stores it.
const DOWNLOADED: &str = "2026-11-09T03:00:00+00:00";
const DOWNLOADED_AT: &str = "2026-11-09 03:00:00";
/// A verified sync after that download (its ETag was confirmed).
const VERIFIED_AT: &str = "2026-11-09 04:00:00";

fn endpoints(api: &str, bucket: &str, dir: &Path) -> SyncEndpoints {
    SyncEndpoints {
        batch_url: format!("{api}/v1/querybatch"),
        mirror: Mirror::at(bucket, dir),
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("client")
}

/// A database whose inventory is one npm package, `lodash`.
fn db_with_lodash() -> Database {
    let db = crate::test_utils::test_db();
    db.store_dependency("/project", "lodash", Some("4.17.20"), "npm", false, None)
        .expect("dependency");
    db
}

/// The npm row as left by a verified sync at [`VERIFIED_AT`].
fn verified_before(db: &Database) {
    db.update_osv_sync_status("npm", 1, None).expect("status");
    db.conn
        .lock()
        .execute(
            "UPDATE osv_sync_status SET last_synced_at = ?1 WHERE ecosystem = 'npm'",
            [VERIFIED_AT],
        )
        .expect("set time");
}

fn npm_status(db: &Database) -> SyncStatus {
    db.get_osv_sync_statuses()
        .expect("statuses")
        .into_iter()
        .find(|s| s.ecosystem == "npm")
        .expect("npm status row")
}

fn lodash_advisories(db: &Database) -> Vec<String> {
    db.get_osv_advisories_for_package("lodash", "npm")
        .expect("advisories")
        .into_iter()
        .map(|a| a.advisory_id)
        .collect()
}

fn empty_result() -> SyncResult {
    SyncResult {
        ecosystems_synced: Vec::new(),
        advisories_stored: 0,
        advisories_matched: 0,
        duration_ms: 0,
        errors: Vec::new(),
    }
}

/// Run npm's pass for `packages` and record it, as `sync` does.
async fn pass_for(
    db: &Database,
    endpoints: &SyncEndpoints,
    packages: &[&str],
) -> (EcosystemOutcome, SyncResult) {
    let packages: Vec<String> = packages.iter().map(|p| (*p).to_string()).collect();
    let outcome = sync_one_ecosystem(&client(), db, endpoints, "npm", &packages).await;
    let mut result = empty_result();
    record_outcome(db, "npm", outcome.clone(), &mut result);
    (outcome, result)
}

async fn pass(db: &Database, endpoints: &SyncEndpoints) -> (EcosystemOutcome, SyncResult) {
    pass_for(db, endpoints, &["lodash"]).await
}

fn stamped_now(status: &SyncStatus) -> bool {
    let at = chrono::NaiveDateTime::parse_from_str(
        status.last_synced_at.as_deref().unwrap_or(""),
        "%Y-%m-%d %H:%M:%S",
    );
    at.is_ok_and(|at| {
        chrono::Utc::now()
            .naive_utc()
            .signed_duration_since(at)
            .num_minutes()
            < 2
    })
}

// --- The defect -------------------------------------------------------------

/// The reproduction (2026-11-11): the network is blocked and an archive is
/// on disk. The pass re-read it and recorded npm as synced "now" with no
/// error. Its advisories are still matched, and the status now says the check
/// did not happen and how old the data is.
#[tokio::test]
async fn offline_with_a_cached_archive_matches_it_but_is_not_a_fresh_sync() {
    let offline = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&offline.url, &offline.url, dir.path());
    seed_archive(
        &endpoints.mirror,
        "npm",
        "\"v1\"",
        DOWNLOADED,
        &archive_for("GHSA-cached", "lodash", "npm"),
    );
    let db = db_with_lodash();
    verified_before(&db);

    let (outcome, result) = pass(&db, &endpoints).await;

    assert!(
        matches!(
            outcome,
            EcosystemOutcome::Unverified {
                stored: Some(1),
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        lodash_advisories(&db),
        ["GHSA-cached"],
        "the cache is still matched"
    );
    let status = npm_status(&db);
    assert_eq!(
        status.last_synced_at.as_deref(),
        Some(VERIFIED_AT),
        "the last VERIFIED sync, not now"
    );
    let error = status.error.as_deref().expect("the pass records why");
    assert!(
        error.contains("could not check for newer advisories"),
        "{error}"
    );
    assert_eq!(status.advisory_count, 1);

    // The pass is not a sync to anyone who reads its result ...
    assert!(result.ecosystems_synced.is_empty(), "{result:?}");
    assert_eq!(result.errors.len(), 1, "{result:?}");
    assert!(result.errors[0].starts_with("npm: "), "{result:?}");
    // ... the mirror stays due ...
    assert!(needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).expect("needs_sync"));
    // ... and readiness reports a failed check with the age of the data.
    let readiness = advisory_sync(&db);
    assert_eq!(readiness.state, AdvisorySyncState::Failed);
    assert_eq!(readiness.synced_at.as_deref(), Some(VERIFIED_AT));
    assert!(readiness
        .error
        .as_deref()
        .is_some_and(|e| e.starts_with("npm: ")));
}

/// The first pass ever, offline, with an archive on disk (left by an earlier
/// install or the launch cache refresh): there is no verified sync to keep,
/// so the data is dated by the archive's download.
#[tokio::test]
async fn a_first_pass_offline_dates_the_data_by_the_archives_download() {
    let offline = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&offline.url, &offline.url, dir.path());
    seed_archive(
        &endpoints.mirror,
        "npm",
        "\"v1\"",
        DOWNLOADED,
        &archive_for("GHSA-cached", "lodash", "npm"),
    );
    let db = db_with_lodash();

    let (outcome, _) = pass(&db, &endpoints).await;

    match &outcome {
        EcosystemOutcome::Unverified {
            stored, data_as_of, ..
        } => {
            assert_eq!(*stored, Some(1));
            assert_eq!(data_as_of.as_deref(), Some(DOWNLOADED_AT));
        }
        other => panic!("expected unverified, got {other:?}"),
    }
    let status = npm_status(&db);
    assert_eq!(status.last_synced_at.as_deref(), Some(DOWNLOADED_AT));
    assert!(status.error.is_some());
    let readiness = advisory_sync(&db);
    assert_eq!(readiness.state, AdvisorySyncState::Failed);
    assert_eq!(readiness.synced_at.as_deref(), Some(DOWNLOADED_AT));
}

/// The API answers and the archive host does not (a proxy that blocks
/// storage.googleapis.com). The advisory data comes from the archive, so an
/// ETag check that could not run leaves the pass unverified.
#[tokio::test]
async fn an_etag_check_that_could_not_run_leaves_the_pass_unverified() {
    let osv = FakeOsv::start().await;
    let blocked = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &blocked.url, dir.path());
    seed_archive(
        &endpoints.mirror,
        "npm",
        "\"v1\"",
        DOWNLOADED,
        &archive_for("GHSA-cached", "lodash", "npm"),
    );
    let db = db_with_lodash();
    verified_before(&db);

    let (outcome, result) = pass(&db, &endpoints).await;

    assert_eq!(osv.count("POST /v1/querybatch"), 1, "the API was asked");
    match &outcome {
        EcosystemOutcome::Unverified { reason, .. } => {
            assert!(reason.contains("HEAD request failed"), "{reason}");
        }
        other => panic!("expected unverified, got {other:?}"),
    }
    let status = npm_status(&db);
    assert_eq!(status.last_synced_at.as_deref(), Some(VERIFIED_AT));
    assert!(status.error.is_some());
    assert_eq!(result.errors.len(), 1);
    assert_eq!(lodash_advisories(&db), ["GHSA-cached"]);
}

/// First run with no archive and no network: nothing was read. The status
/// carries the error and no time at all; nothing reads as synced.
#[tokio::test]
async fn a_first_run_with_no_archive_and_no_network_fails_with_no_time() {
    let offline = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&offline.url, &offline.url, dir.path());
    let db = db_with_lodash();

    let (outcome, result) = pass(&db, &endpoints).await;

    assert!(
        matches!(outcome, EcosystemOutcome::Failed { .. }),
        "{outcome:?}"
    );
    assert!(lodash_advisories(&db).is_empty());
    let status = npm_status(&db);
    assert_eq!(status.last_synced_at, None, "no verified data to date");
    assert!(status.error.is_some());
    assert!(result.ecosystems_synced.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).expect("needs_sync"));
    let readiness = advisory_sync(&db);
    assert_eq!(readiness.state, AdvisorySyncState::Failed);
    assert_eq!(readiness.synced_at, None);
    assert!(
        !endpoints.mirror.zip_path("npm").exists(),
        "a failed download leaves no archive behind"
    );
}

// --- What still counts as verified -----------------------------------------

#[tokio::test]
async fn a_confirmed_etag_is_a_verified_sync_without_a_download() {
    let osv = FakeOsv::start().await;
    let archive = archive_for("GHSA-current", "lodash", "npm");
    osv.publish("\"v1\"", archive.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    seed_archive(&endpoints.mirror, "npm", "\"v1\"", DOWNLOADED, &archive);
    let db = db_with_lodash();
    verified_before(&db);

    let (outcome, result) = pass(&db, &endpoints).await;

    assert_eq!(outcome, EcosystemOutcome::Verified { stored: 1 });
    assert_eq!(osv.count("HEAD /npm/all.zip"), 1);
    assert_eq!(osv.count("GET /npm/all.zip"), 0, "no download needed");
    let status = npm_status(&db);
    assert_eq!(status.error, None);
    assert!(stamped_now(&status), "{status:?}");
    assert_eq!(result.ecosystems_synced, ["npm"]);
    assert!(result.errors.is_empty());
    assert_eq!(advisory_sync(&db).state, AdvisorySyncState::Ready);
}

#[tokio::test]
async fn a_newer_archive_is_downloaded_and_the_pass_is_verified() {
    let osv = FakeOsv::start().await;
    let newer = archive_for("GHSA-newer", "lodash", "npm");
    osv.publish("\"v2\"", newer.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    seed_archive(
        &endpoints.mirror,
        "npm",
        "\"v1\"",
        DOWNLOADED,
        &archive_for("GHSA-older", "lodash", "npm"),
    );
    let db = db_with_lodash();
    verified_before(&db);

    let (outcome, _) = pass(&db, &endpoints).await;

    assert_eq!(outcome, EcosystemOutcome::Verified { stored: 1 });
    assert_eq!(osv.count("GET /npm/all.zip"), 1);
    assert_eq!(
        std::fs::read(endpoints.mirror.zip_path("npm")).expect("read"),
        newer
    );
    assert_eq!(lodash_advisories(&db), ["GHSA-newer"]);
    let status = npm_status(&db);
    assert_eq!(status.error, None);
    assert!(stamped_now(&status), "{status:?}");
}

/// The first run online: nothing on disk, the archive downloads, verified.
#[tokio::test]
async fn a_first_run_online_downloads_and_is_verified() {
    let osv = FakeOsv::start().await;
    osv.publish("\"v1\"", archive_for("GHSA-first", "lodash", "npm"));
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    let db = db_with_lodash();

    let (outcome, result) = pass(&db, &endpoints).await;

    assert_eq!(outcome, EcosystemOutcome::Verified { stored: 1 });
    assert_eq!(result.ecosystems_synced, ["npm"]);
    assert_eq!(lodash_advisories(&db), ["GHSA-first"]);
    assert_eq!(npm_status(&db).error, None);
}

/// OSV published a newer archive, but what comes back is not it (a proxy's
/// page, a cut connection). The archive on disk survives and keeps being
/// matched, and the pass is unverified: newer advisories exist unread.
#[tokio::test]
async fn a_failed_download_keeps_matching_the_old_archive_unverified() {
    let osv = FakeOsv::start().await;
    osv.publish("\"v2\"", b"<html>blocked by policy</html>".to_vec());
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    let older = archive_for("GHSA-older", "lodash", "npm");
    seed_archive(&endpoints.mirror, "npm", "\"v1\"", DOWNLOADED, &older);
    let db = db_with_lodash();
    verified_before(&db);

    let (outcome, _) = pass(&db, &endpoints).await;

    match &outcome {
        EcosystemOutcome::Unverified { reason, stored, .. } => {
            assert_eq!(*stored, Some(1));
            assert!(reason.contains("not a ZIP archive"), "{reason}");
        }
        other => panic!("expected unverified, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(endpoints.mirror.zip_path("npm")).expect("read"),
        older,
        "the good archive is still there"
    );
    assert_eq!(lodash_advisories(&db), ["GHSA-older"]);
    let status = npm_status(&db);
    assert_eq!(status.last_synced_at.as_deref(), Some(VERIFIED_AT));
    assert!(status.error.is_some());
}

/// The batch API is down and the archive is confirmed current. The API
/// returns ID stubs that store nothing, so the mirror holds exactly what a
/// fully successful pass would have stored: verified.
#[tokio::test]
async fn an_api_error_with_a_confirmed_archive_is_still_verified() {
    let osv = FakeOsv::start().await;
    osv.api_status(503);
    let archive = archive_for("GHSA-current", "lodash", "npm");
    osv.publish("\"v1\"", archive.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    seed_archive(&endpoints.mirror, "npm", "\"v1\"", DOWNLOADED, &archive);
    let db = db_with_lodash();

    let (outcome, result) = pass(&db, &endpoints).await;

    assert_eq!(outcome, EcosystemOutcome::Verified { stored: 1 });
    assert!(result.errors.is_empty());
    assert_eq!(npm_status(&db).error, None);
}

/// A rate limit is not routed around through the cache: the pass fails, and
/// the last verified time stays where it was.
#[tokio::test]
async fn a_rate_limited_api_fails_the_pass_and_keeps_the_verified_time() {
    let osv = FakeOsv::start().await;
    osv.api_status(429);
    let archive = archive_for("GHSA-current", "lodash", "npm");
    osv.publish("\"v1\"", archive.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    seed_archive(&endpoints.mirror, "npm", "\"v1\"", DOWNLOADED, &archive);
    let db = db_with_lodash();
    verified_before(&db);

    let (outcome, result) = pass(&db, &endpoints).await;

    match &outcome {
        EcosystemOutcome::Failed { error } => assert_eq!(error, RATE_LIMITED),
        other => panic!("expected failed, got {other:?}"),
    }
    assert_eq!(
        osv.count("HEAD /npm/all.zip"),
        0,
        "the archive is not consulted"
    );
    let status = npm_status(&db);
    assert_eq!(status.last_synced_at.as_deref(), Some(VERIFIED_AT));
    assert_eq!(status.advisory_count, 1, "the stored count stays");
    assert_eq!(status.error.as_deref(), Some(RATE_LIMITED));
    assert_eq!(result.errors.len(), 1);
}

// --- Retrying an unverified pass is cheap ---------------------------------

/// While the remote stays away the pass is retried every few minutes. A retry
/// over the same archive file and the same packages reads nothing; a changed
/// inventory, a replaced archive or deleted rows make it read again.
#[tokio::test]
async fn an_unverified_retry_does_not_reread_an_archive_already_in_the_mirror() {
    let offline = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&offline.url, &offline.url, dir.path());
    let both = crate::osv::cache::tests::make_test_zip(&[
        (
            "GHSA-lodash.json",
            &crate::osv::cache::tests::make_vuln_json("GHSA-lodash", "lodash", "npm"),
        ),
        (
            "GHSA-express.json",
            &crate::osv::cache::tests::make_vuln_json("GHSA-express", "express", "npm"),
        ),
    ]);
    seed_archive(&endpoints.mirror, "npm", "\"v1\"", DOWNLOADED, &both);
    let db = db_with_lodash();
    let stored = |outcome: &EcosystemOutcome| match outcome {
        EcosystemOutcome::Unverified { stored, .. } => *stored,
        other => panic!("expected unverified, got {other:?}"),
    };

    let (first, _) = pass(&db, &endpoints).await;
    assert_eq!(stored(&first), Some(1), "the first pass reads the archive");
    let (retry, result) = pass(&db, &endpoints).await;
    assert_eq!(stored(&retry), None, "the retry has nothing new to read");
    assert_eq!(result.advisories_stored, 0);
    assert_eq!(result.errors.len(), 1, "and is still not a sync");
    assert_eq!(lodash_advisories(&db), ["GHSA-lodash"]);
    assert_eq!(npm_status(&db).advisory_count, 1, "the count stays");

    // A package the mirror has not been read for: read again.
    let (wider, _) = pass_for(&db, &endpoints, &["lodash", "express"]).await;
    assert_eq!(stored(&wider), Some(2));
    let (wider_retry, _) = pass_for(&db, &endpoints, &["express", "lodash"]).await;
    assert_eq!(stored(&wider_retry), None, "package order does not matter");

    // Rows were deleted (a prune, a withdrawn cleanup): read again.
    forget_ingested_archives(&db);
    let (after_delete, _) = pass_for(&db, &endpoints, &["lodash", "express"]).await;
    assert_eq!(stored(&after_delete), Some(2));

    // A different archive file on disk: read again.
    seed_archive(
        &endpoints.mirror,
        "npm",
        "\"v1\"",
        DOWNLOADED,
        &archive_for("GHSA-other", "lodash", "npm"),
    );
    let (replaced, _) = pass_for(&db, &endpoints, &["lodash", "express"]).await;
    assert_eq!(stored(&replaced), Some(1));
}

/// A verified pass always reads the archive: the skip is for unverified
/// retries only.
#[tokio::test]
async fn a_verified_pass_always_reads_the_archive() {
    let osv = FakeOsv::start().await;
    let archive = archive_for("GHSA-current", "lodash", "npm");
    osv.publish("\"v1\"", archive.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let endpoints = endpoints(&osv.url, &osv.url, dir.path());
    seed_archive(&endpoints.mirror, "npm", "\"v1\"", DOWNLOADED, &archive);
    let db = db_with_lodash();

    let (first, _) = pass(&db, &endpoints).await;
    let (second, _) = pass(&db, &endpoints).await;
    assert_eq!(first, EcosystemOutcome::Verified { stored: 1 });
    assert_eq!(second, EcosystemOutcome::Verified { stored: 1 });
}

/// Going offline right after a verified pass: the archive that pass read is
/// already in the mirror, so the first unverified pass reads nothing either.
#[tokio::test]
async fn going_offline_after_a_verified_pass_does_not_reread_its_archive() {
    let osv = FakeOsv::start().await;
    let archive = archive_for("GHSA-current", "lodash", "npm");
    osv.publish("\"v1\"", archive.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let online = endpoints(&osv.url, &osv.url, dir.path());
    seed_archive(&online.mirror, "npm", "\"v1\"", DOWNLOADED, &archive);
    let db = db_with_lodash();
    let (verified, _) = pass(&db, &online).await;
    assert_eq!(verified, EcosystemOutcome::Verified { stored: 1 });
    let verified_at = npm_status(&db).last_synced_at;

    let offline = DeadHost::start().await;
    let offline = endpoints(&offline.url, &offline.url, dir.path());
    let (outcome, _) = pass(&db, &offline).await;

    assert!(
        matches!(outcome, EcosystemOutcome::Unverified { stored: None, .. }),
        "{outcome:?}"
    );
    let status = npm_status(&db);
    assert_eq!(
        status.last_synced_at, verified_at,
        "the verified time stays"
    );
    assert!(status.error.is_some());
    assert_eq!(status.advisory_count, 1);
    assert_eq!(lodash_advisories(&db), ["GHSA-current"]);
}
