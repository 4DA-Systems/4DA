// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::io::Write;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use super::*;

/// Helper: create a minimal OSV Vulnerability JSON string.
pub(crate) fn make_vuln_json(id: &str, package: &str, ecosystem: &str) -> String {
    serde_json::json!({
        "id": id,
        "summary": format!("Test vulnerability in {package}"),
        "affected": [{
            "package": {
                "name": package,
                "ecosystem": ecosystem
            },
            "ranges": [{
                "type": "SEMVER",
                "events": [
                    {"introduced": "0"},
                    {"fixed": "99.0.0"}
                ]
            }]
        }]
    })
    .to_string()
}

/// Helper: create an in-memory ZIP containing advisory JSON files.
pub(crate) fn make_test_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        for (name, content) in entries {
            writer.start_file(*name, options).expect("start_file");
            writer.write_all(content.as_bytes()).expect("write");
        }
        writer.finish().expect("finish");
    }
    buf
}

/// An archive holding one advisory for `package`.
pub(crate) fn archive_for(id: &str, package: &str, ecosystem: &str) -> Vec<u8> {
    let name = format!("{id}.json");
    make_test_zip(&[(name.as_str(), &make_vuln_json(id, package, ecosystem))])
}

/// What the stand-in archive bucket serves.
#[derive(Default)]
struct Served {
    /// `(etag, body)` of every ecosystem's `all.zip`; `None` answers 503.
    archive: parking_lot::Mutex<Option<(String, Vec<u8>)>>,
    /// Status of the batch API (200 answers an empty result set).
    api_status: AtomicU16,
    /// `"GET /npm/all.zip"`, in arrival order.
    requests: parking_lot::Mutex<Vec<String>>,
}

/// A local stand-in for OSV: the batch API at `/v1/querybatch` and the
/// archive bucket at `/{ecosystem}/all.zip`. The real HTTP code runs against
/// it; nothing leaves the machine.
pub(crate) struct FakeOsv {
    pub(crate) url: String,
    served: Arc<Served>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeOsv {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeOsv {
    pub(crate) async fn start() -> Self {
        use axum::extract::State;
        use axum::http::{header, Method, StatusCode, Uri};
        use axum::response::IntoResponse;

        async fn archive(
            State(served): State<Arc<Served>>,
            method: Method,
            uri: Uri,
        ) -> axum::response::Response {
            served
                .requests
                .lock()
                .push(format!("{method} {}", uri.path()));
            match served.archive.lock().clone() {
                Some((etag, body)) => ([(header::ETAG, etag)], body).into_response(),
                None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            }
        }
        async fn batch(State(served): State<Arc<Served>>) -> axum::response::Response {
            served
                .requests
                .lock()
                .push("POST /v1/querybatch".to_string());
            let status = StatusCode::from_u16(served.api_status.load(Ordering::SeqCst))
                .unwrap_or(StatusCode::OK);
            (status, axum::Json(serde_json::json!({ "results": [] }))).into_response()
        }

        let served = Arc::new(Served::default());
        served.api_status.store(200, Ordering::SeqCst);
        let app = axum::Router::new()
            .route("/v1/querybatch", axum::routing::post(batch))
            .route("/{ecosystem}/all.zip", axum::routing::get(archive))
            .with_state(served.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("addr"));
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self { url, served, task }
    }

    /// Publish `body` as every ecosystem's archive, under `etag`.
    pub(crate) fn publish(&self, etag: &str, body: Vec<u8>) {
        *self.served.archive.lock() = Some((etag.to_string(), body));
    }

    /// The bucket answers 503 from now on.
    pub(crate) fn archive_down(&self) {
        *self.served.archive.lock() = None;
    }

    pub(crate) fn api_status(&self, status: u16) {
        self.served.api_status.store(status, Ordering::SeqCst);
    }

    /// How many requests arrived whose line starts with `prefix`.
    pub(crate) fn count(&self, prefix: &str) -> usize {
        self.served
            .requests
            .lock()
            .iter()
            .filter(|r| r.starts_with(prefix))
            .count()
    }
}

/// A host that accepts a connection and drops it: every request to it fails
/// at once, the way an offline or blocked host does (a refused port takes
/// seconds to fail on Windows).
pub(crate) struct DeadHost {
    pub(crate) url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for DeadHost {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl DeadHost {
    pub(crate) async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("addr"));
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                drop(socket);
            }
        });
        Self { url, task }
    }
}

/// Put `body` on disk as `ecosystem`'s archive, as a download at
/// `downloaded_at` (RFC 3339) under `etag` would have left it.
pub(crate) fn seed_archive(
    mirror: &Mirror,
    ecosystem: &str,
    etag: &str,
    downloaded_at: &str,
    body: &[u8],
) {
    std::fs::create_dir_all(&mirror.dir).expect("mkdir");
    std::fs::write(mirror.zip_path(ecosystem), body).expect("write zip");
    mirror
        .write_meta(&CacheMeta {
            ecosystem: ecosystem.to_string(),
            etag: Some(etag.to_string()),
            last_modified: None,
            downloaded_at: downloaded_at.to_string(),
            size_bytes: body.len() as u64,
            advisory_count: 0,
        })
        .expect("write meta");
}

fn parts_in(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .expect("read dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_part_file(n))
        .collect()
}

type Chunk = std::result::Result<Vec<u8>, String>;

fn body_of(chunks: Vec<Chunk>) -> impl futures::Stream<Item = Chunk> {
    futures::stream::iter(chunks)
}

#[test]
fn test_cache_meta_round_trip() {
    let meta = CacheMeta {
        ecosystem: "npm".to_string(),
        etag: Some("\"abc123\"".to_string()),
        last_modified: Some("Mon, 01 Jan 2026 00:00:00 GMT".to_string()),
        downloaded_at: "2026-01-01T00:00:00Z".to_string(),
        size_bytes: 1024,
        advisory_count: 42,
    };

    let json = serde_json::to_string(&meta).expect("serialize");
    let parsed: CacheMeta = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.ecosystem, "npm");
    assert_eq!(parsed.etag.as_deref(), Some("\"abc123\""));
    assert_eq!(parsed.size_bytes, 1024);
    assert_eq!(parsed.advisory_count, 42);
}

#[test]
fn test_vulnerability_matches_packages() {
    let vuln_json = make_vuln_json("GHSA-test-001", "lodash", "npm");
    let vuln: Vulnerability = serde_json::from_str(&vuln_json).expect("parse");

    let mut packages = HashSet::new();
    packages.insert("lodash".to_string());

    assert!(
        vulnerability_matches_packages(&vuln, &packages),
        "lodash should match"
    );

    let mut other_packages = HashSet::new();
    other_packages.insert("express".to_string());

    assert!(
        !vulnerability_matches_packages(&vuln, &other_packages),
        "express should not match lodash vuln"
    );
}

#[test]
fn test_package_filtering_case_insensitive() {
    let vuln_json = make_vuln_json("GHSA-test-002", "Lodash", "npm");
    let vuln: Vulnerability = serde_json::from_str(&vuln_json).expect("parse");

    let mut packages = HashSet::new();
    packages.insert("lodash".to_string()); // lowercase

    assert!(
        vulnerability_matches_packages(&vuln, &packages),
        "Case-insensitive match should work"
    );
}

/// The archive on disk is read through the real path: the matching advisory
/// is stored, the other is not, and the metadata records the count.
#[test]
fn test_sync_from_zip_filters_and_stores() {
    let db = crate::test_utils::test_db();
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at("http://127.0.0.1:9", dir.path());
    let lodash_vuln = make_vuln_json("GHSA-match-001", "lodash", "npm");
    let express_vuln = make_vuln_json("GHSA-skip-001", "express", "npm");
    let zip_bytes = make_test_zip(&[
        ("GHSA-match-001.json", &lodash_vuln),
        ("GHSA-skip-001.json", &express_vuln),
        ("README.md", "This is not JSON"),
    ]);
    seed_archive(
        &mirror,
        "npm",
        "\"v1\"",
        "2026-11-09T03:00:00+00:00",
        &zip_bytes,
    );

    let user_packages: HashSet<String> = ["lodash".to_string()].into();
    let stored = mirror
        .sync_from_zip(&db, "npm", &user_packages)
        .expect("sync");
    assert_eq!(stored, 1, "Only lodash advisory should be stored");

    let advisories = db
        .get_osv_advisories_for_package("lodash", "npm")
        .expect("query");
    assert_eq!(advisories.len(), 1);
    assert_eq!(advisories[0].advisory_id, "GHSA-match-001");
    assert!(db
        .get_osv_advisories_for_package("express", "npm")
        .expect("query")
        .is_empty());
    assert_eq!(mirror.read_meta("npm").expect("meta").advisory_count, 1);
}

#[test]
fn no_archive_on_disk_is_an_error_not_zero_advisories() {
    let db = crate::test_utils::test_db();
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at("http://127.0.0.1:9", dir.path());
    assert!(mirror.sync_from_zip(&db, "npm", &HashSet::new()).is_err());
    assert_eq!(mirror.archive_downloaded_at("npm"), None);
}

#[test]
fn the_archives_download_time_is_reported_as_the_status_stores_times() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at("http://127.0.0.1:9", dir.path());
    seed_archive(
        &mirror,
        "npm",
        "\"v1\"",
        "2026-11-10T23:16:58.556960300+00:00",
        &archive_for("GHSA-a", "lodash", "npm"),
    );
    assert_eq!(
        mirror.archive_downloaded_at("npm").as_deref(),
        Some("2026-11-10 23:16:58")
    );
    // A local-offset timestamp is still reported in UTC.
    seed_archive(
        &mirror,
        "npm",
        "\"v1\"",
        "2026-11-11T09:16:58+10:00",
        &archive_for("GHSA-a", "lodash", "npm"),
    );
    assert_eq!(
        mirror.archive_downloaded_at("npm").as_deref(),
        Some("2026-11-10 23:16:58")
    );
    // The metadata without the archive dates nothing.
    std::fs::remove_file(mirror.zip_path("npm")).expect("remove zip");
    assert_eq!(mirror.archive_downloaded_at("npm"), None);
}

// --- A download never destroys the archive it would replace ---------------
// The download used to `File::create` (truncate) the destination and stream
// into it, so a dropped connection left a truncated archive where the good
// one had been, and the offline fallback had nothing to read.

#[tokio::test]
async fn a_download_that_fails_midway_leaves_the_good_archive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("npm.zip");
    let good = archive_for("GHSA-good", "lodash", "npm");
    std::fs::write(&dest, &good).expect("seed");
    let newer = archive_for("GHSA-newer", "lodash", "npm");

    let result = replace_archive(
        body_of(vec![
            Ok(newer[..newer.len() / 2].to_vec()),
            Err("connection reset by peer".to_string()),
        ]),
        &dest,
        "npm",
        MAX_ECOSYSTEM_ZIP_BYTES,
    )
    .await;

    let err = result.expect_err("a cut body is a failed download");
    assert!(err.to_string().contains("connection reset"), "{err}");
    assert_eq!(
        std::fs::read(&dest).expect("read"),
        good,
        "the good archive is untouched"
    );
    assert!(parts_in(dir.path()).is_empty(), "no partial file left");
}

/// A complete body that is cut short of a whole archive (no error from the
/// transport) is still not an archive.
#[tokio::test]
async fn a_truncated_archive_is_rejected_and_the_good_one_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("npm.zip");
    let good = archive_for("GHSA-good", "lodash", "npm");
    std::fs::write(&dest, &good).expect("seed");
    let newer = archive_for("GHSA-newer", "lodash", "npm");

    let result = replace_archive(
        body_of(vec![Ok(newer[..newer.len() - 30].to_vec())]),
        &dest,
        "npm",
        MAX_ECOSYSTEM_ZIP_BYTES,
    )
    .await;

    assert!(result.is_err());
    assert_eq!(std::fs::read(&dest).expect("read"), good);
    assert!(parts_in(dir.path()).is_empty());
}

/// Behind a proxy or a captive portal the "archive" is an HTML page with a
/// 200 status. It must not replace the archive.
#[tokio::test]
async fn a_proxy_page_is_not_an_archive_and_the_good_one_is_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("npm.zip");
    let good = archive_for("GHSA-good", "lodash", "npm");
    std::fs::write(&dest, &good).expect("seed");

    let result = replace_archive(
        body_of(vec![Ok(
            b"<html><body>Access to this site is blocked</body></html>".to_vec(),
        )]),
        &dest,
        "npm",
        MAX_ECOSYSTEM_ZIP_BYTES,
    )
    .await;

    let err = result.expect_err("an HTML page is not an archive");
    assert!(err.to_string().contains("not a ZIP archive"), "{err}");
    assert_eq!(std::fs::read(&dest).expect("read"), good);
    assert!(parts_in(dir.path()).is_empty());
}

#[tokio::test]
async fn an_over_cap_download_is_refused_and_the_good_archive_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("npm.zip");
    let good = archive_for("GHSA-good", "lodash", "npm");
    std::fs::write(&dest, &good).expect("seed");

    let result = replace_archive(
        body_of(vec![Ok(vec![0u8; 64]), Ok(vec![0u8; 64])]),
        &dest,
        "npm",
        100,
    )
    .await;

    let err = result.expect_err("over the cap");
    assert!(err.to_string().contains("download cap"), "{err}");
    assert_eq!(std::fs::read(&dest).expect("read"), good);
    assert!(parts_in(dir.path()).is_empty());
}

#[tokio::test]
async fn a_complete_download_replaces_the_archive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("npm.zip");
    std::fs::write(&dest, archive_for("GHSA-old", "lodash", "npm")).expect("seed");
    let newer = archive_for("GHSA-newer", "lodash", "npm");
    let (head, tail) = newer.split_at(newer.len() / 2);

    let written = replace_archive(
        body_of(vec![Ok(head.to_vec()), Ok(tail.to_vec())]),
        &dest,
        "npm",
        MAX_ECOSYSTEM_ZIP_BYTES,
    )
    .await
    .expect("download");

    assert_eq!(written, newer.len() as u64);
    assert_eq!(std::fs::read(&dest).expect("read"), newer);
    assert!(parts_in(dir.path()).is_empty());
}

#[test]
fn an_abandoned_partial_download_is_removed_and_a_live_one_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let old = dir.path().join("npm.zip.4242.part");
    let live = dir.path().join("npm.zip.4243.part");
    let other = dir.path().join("crates.io.zip.4242.part");
    for path in [&old, &live, &other] {
        std::fs::write(path, b"partial").expect("write");
    }
    let long_ago =
        std::time::SystemTime::now() - std::time::Duration::from_secs(ABANDONED_PART_SECS + 60);
    for path in [&old, &other] {
        std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open")
            .set_modified(long_ago)
            .expect("set mtime");
    }

    remove_abandoned_parts(dir.path(), "npm");

    assert!(!old.exists(), "an old partial of this ecosystem is removed");
    assert!(live.exists(), "a recent one may be a running download");
    assert!(other.exists(), "another ecosystem's is not this call's");
}

// --- The mirror against a (local) remote ----------------------------------

#[tokio::test]
async fn a_download_writes_the_archive_and_then_its_etag_reads_current() {
    let osv = FakeOsv::start().await;
    let body = archive_for("GHSA-a", "lodash", "npm");
    osv.publish("\"v1\"", body.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at(&osv.url, dir.path());

    assert_eq!(
        mirror.freshness("npm").await,
        Freshness::Outdated,
        "nothing on disk yet"
    );
    mirror.download("npm").await.expect("download");
    assert_eq!(std::fs::read(mirror.zip_path("npm")).expect("read"), body);
    let meta = mirror.read_meta("npm").expect("meta");
    assert_eq!(meta.etag.as_deref(), Some("\"v1\""));
    assert_eq!(meta.size_bytes, body.len() as u64);
    assert!(mirror.archive_downloaded_at("npm").is_some());

    assert_eq!(mirror.freshness("npm").await, Freshness::Current);
    osv.publish("\"v2\"", archive_for("GHSA-b", "lodash", "npm"));
    assert_eq!(mirror.freshness("npm").await, Freshness::Outdated);
}

/// "Could not ask" is not "fresh" and not "outdated".
#[tokio::test]
async fn an_unreachable_remote_is_unknown_not_fresh() {
    let dead = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at(&dead.url, dir.path());
    seed_archive(
        &mirror,
        "npm",
        "\"v1\"",
        "2026-11-09T03:00:00+00:00",
        &archive_for("GHSA-a", "lodash", "npm"),
    );

    match mirror.freshness("npm").await {
        Freshness::Unknown(reason) => assert!(reason.contains("HEAD request failed"), "{reason}"),
        other => panic!("an unreachable host must read as unknown, got {other:?}"),
    }
}

#[tokio::test]
async fn a_refused_download_keeps_the_archive_and_its_metadata() {
    let osv = FakeOsv::start().await;
    osv.archive_down();
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at(&osv.url, dir.path());
    let good = archive_for("GHSA-good", "lodash", "npm");
    seed_archive(&mirror, "npm", "\"v1\"", "2026-11-09T03:00:00+00:00", &good);

    let err = mirror.download("npm").await.expect_err("503");
    assert!(err.to_string().contains("HTTP 503"), "{err}");
    assert_eq!(std::fs::read(mirror.zip_path("npm")).expect("read"), good);
    assert_eq!(
        mirror.read_meta("npm").expect("meta").etag.as_deref(),
        Some("\"v1\"")
    );
    assert_eq!(
        mirror.archive_downloaded_at("npm").as_deref(),
        Some("2026-11-09 03:00:00")
    );
}

/// The host answers 200 with a page that is not an archive (a proxy block
/// page): the download fails and the archive on disk survives.
#[tokio::test]
async fn a_block_page_download_keeps_the_archive() {
    let osv = FakeOsv::start().await;
    osv.publish("\"page\"", b"<html>blocked by policy</html>".to_vec());
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at(&osv.url, dir.path());
    let good = archive_for("GHSA-good", "lodash", "npm");
    seed_archive(&mirror, "npm", "\"v1\"", "2026-11-09T03:00:00+00:00", &good);

    assert!(mirror.download("npm").await.is_err());
    assert_eq!(std::fs::read(mirror.zip_path("npm")).expect("read"), good);
    assert_eq!(
        mirror.read_meta("npm").expect("meta").etag.as_deref(),
        Some("\"v1\""),
        "the metadata still describes the archive that is on disk"
    );
    assert!(parts_in(dir.path()).is_empty());
}

/// The launch cache refresh, offline: the remote cannot be asked, so there
/// is nothing to refresh. It reports that and leaves the archive alone; it
/// does not download blindly or read the whole archive again.
#[tokio::test]
async fn an_offline_cache_refresh_reports_the_failed_check_and_reads_nothing() {
    let dead = DeadHost::start().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at(&dead.url, dir.path());
    let good = archive_for("GHSA-a", "lodash", "npm");
    seed_archive(&mirror, "npm", "\"v1\"", "2026-11-09T03:00:00+00:00", &good);
    let db = crate::test_utils::test_db();
    let packages: HashSet<String> = ["lodash".to_string()].into();
    let mut result = CacheUpdateResult {
        ecosystems_updated: Vec::new(),
        ecosystems_skipped: Vec::new(),
        total_advisories: 0,
        duration_ms: 0,
        errors: Vec::new(),
    };

    update_single_ecosystem(&mirror, &db, "npm", &packages, &mut result).await;

    assert_eq!(result.errors.len(), 1, "{result:?}");
    assert!(result.errors[0].starts_with("npm (check): "), "{result:?}");
    assert!(result.ecosystems_updated.is_empty());
    assert!(result.ecosystems_skipped.is_empty());
    assert_eq!(result.total_advisories, 0, "the archive was not read again");
    assert_eq!(std::fs::read(mirror.zip_path("npm")).expect("read"), good);
}

/// Online with a current archive, the refresh still matches it (the user's
/// packages may have changed since it was read).
#[tokio::test]
async fn a_cache_refresh_over_a_current_archive_matches_it() {
    let osv = FakeOsv::start().await;
    let archive = archive_for("GHSA-a", "lodash", "npm");
    osv.publish("\"v1\"", archive.clone());
    let dir = tempfile::tempdir().expect("tempdir");
    let mirror = Mirror::at(&osv.url, dir.path());
    seed_archive(
        &mirror,
        "npm",
        "\"v1\"",
        "2026-11-09T03:00:00+00:00",
        &archive,
    );
    let db = crate::test_utils::test_db();
    let packages: HashSet<String> = ["lodash".to_string()].into();
    let mut result = CacheUpdateResult {
        ecosystems_updated: Vec::new(),
        ecosystems_skipped: Vec::new(),
        total_advisories: 0,
        duration_ms: 0,
        errors: Vec::new(),
    };

    update_single_ecosystem(&mirror, &db, "npm", &packages, &mut result).await;

    assert!(result.errors.is_empty(), "{result:?}");
    assert_eq!(result.ecosystems_skipped, ["npm"]);
    assert_eq!(result.total_advisories, 1);
    assert_eq!(osv.count("GET /npm/all.zip"), 0);
}
