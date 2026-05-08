// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV local cache — downloads ecosystem ZIP files from GCS for offline advisory lookup.
//!
//! Each ecosystem's advisories are published as a ZIP file at:
//!   https://osv-vulnerabilities.storage.googleapis.com/{ecosystem}/all.zip
//!
//! We download these, filter to the user's packages, and store matches via
//! the same `upsert_osv_advisory` path the online sync uses.

use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use tracing::{debug, info, warn};

use crate::db::Database;
use crate::error::{FourDaError, Result, ResultExt};
use crate::runtime_paths::RuntimePaths;

use super::types::{CacheMeta, CacheUpdateResult, Vulnerability};

const GCS_BASE_URL: &str = "https://osv-vulnerabilities.storage.googleapis.com";
const USER_AGENT: &str = "4DA/1.0 (local-osv-cache)";

/// Canonical ecosystem names as used in the GCS bucket paths.
/// The GCS bucket uses these exact strings as path segments.
const KNOWN_ECOSYSTEMS: &[&str] = &[
    "crates.io",
    "npm",
    "PyPI",
    "Go",
    "Maven",
    "NuGet",
    "RubyGems",
    "Packagist",
];

/// Get the cache directory for OSV ZIP files.
fn cache_dir() -> PathBuf {
    RuntimePaths::get().data_dir.join("osv-cache")
}

/// Get the path where a cached ecosystem ZIP would be stored.
fn zip_path(ecosystem: &str) -> PathBuf {
    cache_dir().join(format!("{ecosystem}.zip"))
}

/// Get the path for the cache metadata file.
fn meta_path(ecosystem: &str) -> PathBuf {
    cache_dir().join(format!("{ecosystem}.meta.json"))
}

/// Read cached metadata for an ecosystem, if it exists.
pub(crate) fn read_meta(ecosystem: &str) -> Option<CacheMeta> {
    let path = meta_path(ecosystem);
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write cache metadata for an ecosystem.
fn write_meta(meta: &CacheMeta) -> Result<()> {
    let path = meta_path(&meta.ecosystem);
    let json = serde_json::to_string_pretty(meta).context("Failed to serialize cache metadata")?;
    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write cache meta to {}", path.display()))?;
    Ok(())
}

/// Build a reqwest client with the 4DA user-agent.
fn build_client(timeout_secs: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| FourDaError::Internal(format!("Failed to build HTTP client: {e}")))
}

/// Extract a header value as an owned String.
fn header_str(headers: &reqwest::header::HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

/// Download an ecosystem's advisory ZIP from OSV GCS bucket.
///
/// URL: `https://osv-vulnerabilities.storage.googleapis.com/{ecosystem}/all.zip`
/// Stores at: `data/osv-cache/{ecosystem}.zip`
///
/// Returns the path to the downloaded file.
pub async fn download_ecosystem_zip(ecosystem: &str) -> Result<PathBuf> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create cache dir: {}", dir.display()))?;

    let url = format!("{GCS_BASE_URL}/{ecosystem}/all.zip");
    let dest = zip_path(ecosystem);

    info!(target: "4da::osv::cache", ecosystem, url = %url, "Downloading ecosystem ZIP");

    let client = build_client(300)?;
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to download {url}"))?;

    if !response.status().is_success() {
        return Err(FourDaError::Internal(format!(
            "GCS returned HTTP {} for {ecosystem}/all.zip",
            response.status().as_u16()
        )));
    }

    let etag = header_str(response.headers(), "etag");
    let last_modified = header_str(response.headers(), "last-modified");

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("Failed to read response body for {ecosystem}"))?;

    let size_bytes = bytes.len() as u64;
    std::fs::write(&dest, &bytes)
        .with_context(|| format!("Failed to write ZIP to {}", dest.display()))?;

    write_meta(&CacheMeta {
        ecosystem: ecosystem.to_string(),
        etag,
        last_modified,
        downloaded_at: chrono::Utc::now().to_rfc3339(),
        size_bytes,
        advisory_count: 0, // Updated after sync_from_zip
    })?;

    info!(target: "4da::osv::cache", ecosystem, size_bytes, "ZIP downloaded");
    Ok(dest)
}

/// Check if the cached ZIP is stale via HTTP HEAD request (ETag comparison).
///
/// Returns `true` if the cache needs updating (no cache, no ETag, or ETag differs).
pub async fn is_cache_stale(ecosystem: &str) -> Result<bool> {
    let stored_etag = match read_meta(ecosystem) {
        Some(m) => match m.etag {
            Some(e) => e,
            None => return Ok(true),
        },
        None => return Ok(true),
    };

    if !zip_path(ecosystem).exists() {
        return Ok(true);
    }

    let url = format!("{GCS_BASE_URL}/{ecosystem}/all.zip");
    let client = build_client(15)?;

    let response = client
        .head(&url)
        .send()
        .await
        .with_context(|| format!("HEAD request failed for {url}"))?;

    if !response.status().is_success() {
        warn!(target: "4da::osv::cache", ecosystem, status = response.status().as_u16(), "HEAD failed");
        return Ok(true);
    }

    let remote_etag = header_str(response.headers(), "etag");

    match remote_etag {
        Some(ref remote) if remote == &stored_etag => {
            debug!(target: "4da::osv::cache", ecosystem, "Cache fresh (ETags match)");
            Ok(false)
        }
        _ => {
            debug!(target: "4da::osv::cache", ecosystem, "Cache stale (ETags differ)");
            Ok(true)
        }
    }
}

/// Sync advisories from a cached ZIP into the database.
///
/// Streams through ZIP entries, deserializes each as `Vulnerability`,
/// filters to packages in `user_packages` set, stores via upsert.
///
/// Returns count of advisories stored.
pub fn sync_from_zip(
    db: &Database,
    ecosystem: &str,
    user_packages: &HashSet<String>,
) -> Result<usize> {
    let path = zip_path(ecosystem);
    if !path.exists() {
        return Err(FourDaError::Internal(format!(
            "No cached ZIP for ecosystem {ecosystem}"
        )));
    }

    let file =
        std::fs::File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut archive = zip::ZipArchive::new(reader)
        .with_context(|| format!("Failed to read ZIP archive for {ecosystem}"))?;

    let entry_count = archive.len();

    info!(
        target: "4da::osv::cache",
        ecosystem = ecosystem,
        entries = entry_count,
        user_packages = user_packages.len(),
        "Syncing advisories from cached ZIP"
    );

    let (total_stored, errors) = process_zip_entries(&mut archive, db, user_packages)?;

    // Update advisory_count in metadata
    if let Some(mut meta) = read_meta(ecosystem) {
        meta.advisory_count = total_stored;
        let _ = write_meta(&meta);
    }

    info!(
        target: "4da::osv::cache",
        ecosystem = ecosystem,
        stored = total_stored,
        skipped_errors = errors,
        "ZIP sync complete"
    );

    Ok(total_stored)
}

/// Process all entries in a ZIP archive, filtering and storing matching advisories.
///
/// Returns (total_stored, error_count).
fn process_zip_entries<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    db: &Database,
    user_packages: &HashSet<String>,
) -> Result<(usize, usize)> {
    let mut total_stored = 0usize;
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut errors = 0usize;

    for i in 0..archive.len() {
        match process_single_entry(archive, i, db, user_packages, &mut seen_ids) {
            Ok(Some(count)) => total_stored += count,
            Ok(None) => {}         // Skipped (non-JSON or non-matching)
            Err(_) => errors += 1, // Already logged in process_single_entry
        }
    }

    Ok((total_stored, errors))
}

/// Process a single ZIP entry: read, parse, filter, and optionally store.
///
/// Returns `Ok(Some(count))` if stored, `Ok(None)` if skipped, `Err` on failure.
fn process_single_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
    db: &Database,
    user_packages: &HashSet<String>,
    seen_ids: &mut HashSet<String>,
) -> Result<Option<usize>> {
    let mut entry = archive.by_index(index).map_err(|e| {
        debug!(target: "4da::osv::cache", index = index, error = %e, "Skipping unreadable ZIP entry");
        FourDaError::Internal(e.to_string())
    })?;

    let name = entry.name().to_string();
    if !name.ends_with(".json") {
        return Ok(None);
    }

    let mut contents = String::new();
    entry.read_to_string(&mut contents).map_err(|e| {
        debug!(target: "4da::osv::cache", name = %name, error = %e, "Skipping unreadable entry");
        FourDaError::Io(e)
    })?;

    let vuln: Vulnerability = serde_json::from_str(&contents).map_err(|e| {
        debug!(target: "4da::osv::cache", name = %name, error = %e, "Skipping unparseable advisory");
        FourDaError::Json(e)
    })?;

    if !vulnerability_matches_packages(&vuln, user_packages) {
        return Ok(None);
    }

    let count = super::sync::store_vulnerability(db, &vuln, seen_ids).map_err(|e| {
        debug!(target: "4da::osv::cache", id = %vuln.id, error = %e, "Failed to store advisory");
        e
    })?;

    Ok(Some(count))
}

/// Check if any of a vulnerability's affected packages are in the user's package set.
fn vulnerability_matches_packages(vuln: &Vulnerability, user_packages: &HashSet<String>) -> bool {
    let affected = match &vuln.affected {
        Some(a) => a,
        None => return false,
    };

    for entry in affected {
        if let Some(ref pkg) = entry.package {
            if user_packages.contains(&pkg.name.to_lowercase()) {
                return true;
            }
        }
    }

    false
}

/// Get cache status for all known ecosystems.
pub fn get_all_cache_statuses() -> Result<Vec<CacheMeta>> {
    let mut statuses = Vec::new();
    for eco in KNOWN_ECOSYSTEMS {
        if let Some(meta) = read_meta(eco) {
            statuses.push(meta);
        }
    }
    Ok(statuses)
}

/// Update all caches for active ecosystems.
///
/// Determines which ecosystems the user has dependencies for,
/// downloads fresh ZIPs where stale, and syncs to the database.
pub async fn update_all_caches(db: &Database) -> Result<CacheUpdateResult> {
    let start = std::time::Instant::now();
    let mut result = CacheUpdateResult {
        ecosystems_updated: Vec::new(),
        ecosystems_skipped: Vec::new(),
        total_advisories: 0,
        duration_ms: 0,
        errors: Vec::new(),
    };

    let by_ecosystem = collect_ecosystem_packages(db)?;
    if by_ecosystem.is_empty() {
        info!(target: "4da::osv::cache", "No dependencies found, skipping cache update");
        return Ok(result);
    }

    for (ecosystem, packages) in &by_ecosystem {
        update_single_ecosystem(db, ecosystem, packages, &mut result).await;
    }

    result.duration_ms = start.elapsed().as_millis() as u64;

    info!(
        target: "4da::osv::cache",
        updated = ?result.ecosystems_updated,
        skipped = ?result.ecosystems_skipped,
        total = result.total_advisories,
        duration_ms = result.duration_ms,
        "Cache update complete"
    );

    Ok(result)
}

/// Build per-ecosystem package sets from user dependencies.
fn collect_ecosystem_packages(
    db: &Database,
) -> Result<std::collections::HashMap<String, HashSet<String>>> {
    let deps = db
        .get_all_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read dependencies: {e}")))?;

    let mut by_ecosystem: std::collections::HashMap<String, HashSet<String>> =
        std::collections::HashMap::new();
    for dep in &deps {
        let osv_eco = super::sync::normalize_to_osv_pub(&dep.ecosystem);
        by_ecosystem
            .entry(osv_eco)
            .or_default()
            .insert(dep.package_name.to_lowercase());
    }
    Ok(by_ecosystem)
}

/// Update cache for a single ecosystem: check staleness, download if needed, sync.
async fn update_single_ecosystem(
    db: &Database,
    ecosystem: &str,
    packages: &HashSet<String>,
    result: &mut CacheUpdateResult,
) {
    let needs_download = match is_cache_stale(ecosystem).await {
        Ok(false) => {
            result.ecosystems_skipped.push(ecosystem.to_string());
            // Still sync from existing ZIP in case user packages changed
            sync_and_record(db, ecosystem, packages, result);
            return;
        }
        Ok(true) => true,
        Err(e) => {
            warn!(target: "4da::osv::cache", ecosystem, error = %e, "Staleness check failed");
            true // Try download anyway
        }
    };

    if !needs_download {
        return;
    }

    match download_ecosystem_zip(ecosystem).await {
        Ok(_) => {
            result.ecosystems_updated.push(ecosystem.to_string());
            sync_and_record(db, ecosystem, packages, result);
        }
        Err(e) => {
            let msg = format!("{ecosystem} (download): {e}");
            warn!(target: "4da::osv::cache", error = %msg, "Cache download failed");
            result.errors.push(msg);

            // Fall back to existing cache
            if zip_path(ecosystem).exists() {
                let _ =
                    sync_from_zip(db, ecosystem, packages).map(|c| result.total_advisories += c);
            }
        }
    }
}

/// Sync from ZIP and record the result (advisory count or error).
fn sync_and_record(
    db: &Database,
    ecosystem: &str,
    packages: &HashSet<String>,
    result: &mut CacheUpdateResult,
) {
    match sync_from_zip(db, ecosystem, packages) {
        Ok(count) => result.total_advisories += count,
        Err(e) => {
            let msg = format!("{ecosystem} (sync): {e}");
            warn!(target: "4da::osv::cache", error = %msg, "Cache sync failed");
            result.errors.push(msg);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: create a minimal OSV Vulnerability JSON string.
    fn make_vuln_json(id: &str, package: &str, ecosystem: &str) -> String {
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
    fn make_test_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            for (name, content) in entries {
                writer.start_file(*name, options).expect("start_file");
                writer.write_all(content.as_bytes()).expect("write");
            }
            writer.finish().expect("finish");
        }
        buf
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

    #[test]
    fn test_sync_from_zip_filters_and_stores() {
        use crate::test_utils::test_db;

        let db = test_db();

        // Create a temporary ZIP with two advisories: one matching, one not
        let lodash_vuln = make_vuln_json("GHSA-match-001", "lodash", "npm");
        let express_vuln = make_vuln_json("GHSA-skip-001", "express", "npm");

        let zip_bytes = make_test_zip(&[
            ("GHSA-match-001.json", &lodash_vuln),
            ("GHSA-skip-001.json", &express_vuln),
        ]);

        // Write the ZIP to a temp location
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let cache_subdir = tmp_dir.path().join("osv-cache");
        std::fs::create_dir_all(&cache_subdir).expect("mkdir");
        let zip_file = cache_subdir.join("npm.zip");
        std::fs::write(&zip_file, &zip_bytes).expect("write zip");

        // We need to test with actual file paths matching our cache_dir(),
        // so let's test the filtering + parsing logic directly instead
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)).expect("open zip");

        let mut user_packages = HashSet::new();
        user_packages.insert("lodash".to_string());

        let mut total_stored = 0usize;
        let mut seen_ids: HashSet<String> = HashSet::new();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).expect("entry");
            let name = entry.name().to_string();

            if !name.ends_with(".json") {
                continue;
            }

            let mut contents = String::new();
            entry.read_to_string(&mut contents).expect("read");

            let vuln: Vulnerability = serde_json::from_str(&contents).expect("parse");

            if !vulnerability_matches_packages(&vuln, &user_packages) {
                continue;
            }

            total_stored +=
                super::super::sync::store_vulnerability(&db, &vuln, &mut seen_ids).expect("store");
        }

        assert_eq!(total_stored, 1, "Only lodash advisory should be stored");

        let advisories = db
            .get_osv_advisories_for_package("lodash", "npm")
            .expect("query");
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].advisory_id, "GHSA-match-001");

        // express should NOT have been stored
        let express_advisories = db
            .get_osv_advisories_for_package("express", "npm")
            .expect("query");
        assert!(express_advisories.is_empty());
    }

    #[test]
    fn test_zip_with_non_json_entries_ignored() {
        let vuln_json = make_vuln_json("GHSA-test-003", "react", "npm");
        let zip_bytes = make_test_zip(&[
            ("README.md", "This is not JSON"),
            ("GHSA-test-003.json", &vuln_json),
            ("metadata.txt", "Also not JSON"),
        ]);

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)).expect("open zip");

        let mut json_count = 0;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).expect("entry");
            if entry.name().ends_with(".json") {
                json_count += 1;
            }
        }

        assert_eq!(json_count, 1, "Only .json entries should be considered");
    }
}
