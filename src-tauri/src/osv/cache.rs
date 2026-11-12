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
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::db::Database;
use crate::error::{FourDaError, Result, ResultExt};
use crate::runtime_paths::RuntimePaths;

use super::types::{CacheMeta, CacheUpdateResult, Vulnerability};

const GCS_BASE_URL: &str = "https://osv-vulnerabilities.storage.googleapis.com";
const USER_AGENT: &str = "4DA/1.0 (local-osv-cache)";

/// Hard ceiling on a downloaded ecosystem ZIP (1 GiB).
///
/// The largest production bucket (npm) is a few hundred MB and growing. The
/// download previously called `.bytes()`, which buffers the entire body in
/// memory with no bound at all — whatever the far end sends, we allocate.
/// Streaming to disk with this cap means a truncated, redirected, or hostile
/// response costs a bounded amount of disk and no heap.
const MAX_ECOSYSTEM_ZIP_BYTES: u64 = 1024 * 1024 * 1024;

/// Hard ceiling on a single advisory JSON inside an ecosystem ZIP (8 MiB).
///
/// Real OSV records are kilobytes. `read_to_string` on a ZIP entry is
/// unbounded — the declared size is metadata, and the DEFLATE reader keeps
/// producing bytes for as long as the compressed stream does.
const MAX_ADVISORY_JSON_BYTES: u64 = 8 * 1024 * 1024;

/// A partial download left by a process that died is removed once it is this
/// old. A live download cannot be: the client gives up after 300 s.
const ABANDONED_PART_SECS: u64 = 15 * 60;

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
    "Pub",
];

/// What asking the remote about an ecosystem's archive established. "Could
/// not ask" is its own answer: it used to be read as "fresh", so an offline
/// machine recorded a successful sync every time it re-read its old archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Freshness {
    /// The remote's ETag matches the archive on disk: it is the current one.
    Current,
    /// No usable archive on disk, or the remote publishes a different one.
    Outdated,
    /// The remote could not be asked (offline, a proxy, the host blocked).
    Unknown(String),
}

/// Where the advisory archives come from and where they are kept.
/// [`Mirror::live`] is OSV's bucket and this install's `osv-cache` directory;
/// tests point one at a local server and a temp directory, so the code that
/// runs in production is the code under test.
#[derive(Debug, Clone)]
pub(crate) struct Mirror {
    base_url: String,
    dir: PathBuf,
}

impl Mirror {
    pub(crate) fn live() -> Self {
        Self {
            base_url: crate::e2e_gate::osv_mirror_base_url()
                .unwrap_or_else(|| GCS_BASE_URL.to_string()),
            dir: RuntimePaths::get().data_dir.join("osv-cache"),
        }
    }

    #[cfg(test)]
    pub(crate) fn at(base_url: &str, dir: &Path) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            dir: dir.to_path_buf(),
        }
    }

    /// Where a cached ecosystem ZIP is stored.
    pub(crate) fn zip_path(&self, ecosystem: &str) -> PathBuf {
        self.dir.join(format!("{ecosystem}.zip"))
    }

    fn meta_path(&self, ecosystem: &str) -> PathBuf {
        self.dir.join(format!("{ecosystem}.meta.json"))
    }

    fn archive_url(&self, ecosystem: &str) -> String {
        format!("{}/{ecosystem}/all.zip", self.base_url)
    }

    /// Read cached metadata for an ecosystem, if it exists.
    pub(crate) fn read_meta(&self, ecosystem: &str) -> Option<CacheMeta> {
        let data = std::fs::read_to_string(self.meta_path(ecosystem)).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn write_meta(&self, meta: &CacheMeta) -> Result<()> {
        let path = self.meta_path(&meta.ecosystem);
        let json =
            serde_json::to_string_pretty(meta).context("Failed to serialize cache metadata")?;
        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write cache meta to {}", path.display()))?;
        Ok(())
    }

    /// When the archive on disk finished downloading, in the form the sync
    /// status stores times (UTC `YYYY-MM-DD HH:MM:SS`): the last moment that
    /// data is known to have been current. `None` without an archive or
    /// without a readable record of its download.
    pub(crate) fn archive_downloaded_at(&self, ecosystem: &str) -> Option<String> {
        if !self.zip_path(ecosystem).exists() {
            return None;
        }
        let meta = self.read_meta(ecosystem)?;
        let at = chrono::DateTime::parse_from_rfc3339(&meta.downloaded_at).ok()?;
        Some(
            at.with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    }

    /// Download an ecosystem's advisory ZIP from the OSV bucket
    /// (`{base}/{ecosystem}/all.zip`) to `osv-cache/{ecosystem}.zip`.
    ///
    /// The archive on disk is replaced only by a complete download that
    /// opens as a ZIP ([`replace_archive`]): a failed, partial or non-archive
    /// response (a proxy's block page) leaves the previous archive as it was.
    pub(crate) async fn download(&self, ecosystem: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create cache dir: {}", self.dir.display()))?;
        remove_abandoned_parts(&self.dir, ecosystem);

        let url = self.archive_url(ecosystem);
        let dest = self.zip_path(ecosystem);

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

        let size_bytes = replace_archive(
            response.bytes_stream(),
            &dest,
            ecosystem,
            MAX_ECOSYSTEM_ZIP_BYTES,
        )
        .await?;

        self.write_meta(&CacheMeta {
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

    /// Ask the remote whether the archive on disk is still the current one:
    /// an HTTP HEAD and an ETag comparison, no download.
    pub(crate) async fn freshness(&self, ecosystem: &str) -> Freshness {
        let Some(stored_etag) = self.read_meta(ecosystem).and_then(|m| m.etag) else {
            return Freshness::Outdated;
        };
        if !self.zip_path(ecosystem).exists() {
            return Freshness::Outdated;
        }

        let url = self.archive_url(ecosystem);
        let client = match build_client(15) {
            Ok(client) => client,
            Err(e) => return Freshness::Unknown(e.to_string()),
        };
        let response = match client.head(&url).send().await {
            Ok(response) => response,
            Err(e) => return Freshness::Unknown(format!("HEAD request failed for {url}: {e}")),
        };

        // The host answered, with a refusal. Some proxies refuse HEAD and
        // serve GET, so the download is tried, and its result decides.
        if !response.status().is_success() {
            warn!(target: "4da::osv::cache", ecosystem, status = response.status().as_u16(), "HEAD failed");
            return Freshness::Outdated;
        }

        match header_str(response.headers(), "etag") {
            Some(ref remote) if remote == &stored_etag => {
                debug!(target: "4da::osv::cache", ecosystem, "Cache fresh (ETags match)");
                Freshness::Current
            }
            _ => {
                debug!(target: "4da::osv::cache", ecosystem, "Cache stale (ETags differ)");
                Freshness::Outdated
            }
        }
    }

    /// Sync advisories from the cached ZIP into the database.
    ///
    /// Streams through ZIP entries, deserializes each as `Vulnerability`,
    /// filters to packages in `user_packages` set, stores via upsert.
    ///
    /// Returns count of advisories stored.
    pub(crate) fn sync_from_zip(
        &self,
        db: &Database,
        ecosystem: &str,
        user_packages: &HashSet<String>,
    ) -> Result<usize> {
        let path = self.zip_path(ecosystem);
        if !path.exists() {
            return Err(FourDaError::Internal(format!(
                "No cached ZIP for ecosystem {ecosystem}"
            )));
        }

        let file = std::fs::File::open(&path)
            .with_context(|| format!("Failed to open {}", path.display()))?;
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

        // Parsing every entry of the npm archive (230k advisories) is one to five
        // minutes of synchronous work, and the async OSV sync calls this inline.
        // `cpu_bound` hands this worker's queued tasks to another worker first
        // (isolated run, 2026-10-24: a 280 s inline sync overlapped a 56 s
        // whole-runtime stall).
        let (total_stored, errors) = crate::ipc_blocking::cpu_bound(|| {
            process_zip_entries(&mut archive, db, user_packages)
        })?;

        // Update advisory_count in metadata
        if let Some(mut meta) = self.read_meta(ecosystem) {
            meta.advisory_count = total_stored;
            let _ = self.write_meta(&meta);
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
}

/// Build a reqwest client with the 4DA user-agent.
fn build_client(timeout_secs: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .redirect(crate::http_client::ssrf_guarded_redirect_policy())
        .build()
        .map_err(|e| FourDaError::Internal(format!("Failed to build HTTP client: {e}")))
}

/// Stream a response body to `dest`, refusing to write more than `cap`
/// bytes. Returns the number of bytes written. Nothing is buffered in memory
/// beyond one chunk. The caller owns `dest` and removes it on an error.
async fn stream_to_file<S, B, E>(body: S, dest: &Path, ecosystem: &str, cap: u64) -> Result<u64>
where
    S: futures::Stream<Item = std::result::Result<B, E>>,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    use futures::StreamExt;
    use std::io::Write;

    let mut file = std::fs::File::create(dest)
        .with_context(|| format!("Failed to create {}", dest.display()))?;
    let mut written: u64 = 0;
    let mut body = std::pin::pin!(body);

    while let Some(chunk) = body.next().await {
        let chunk =
            chunk.with_context(|| format!("Failed to read response body for {ecosystem}"))?;
        let chunk = chunk.as_ref();
        written = written.saturating_add(chunk.len() as u64);

        if written > cap {
            return Err(FourDaError::Internal(format!(
                "{ecosystem}/all.zip exceeds the {} MB download cap — refusing",
                cap / (1024 * 1024)
            )));
        }

        file.write_all(chunk)
            .with_context(|| format!("Failed to write ZIP to {}", dest.display()))?;
    }

    file.flush()
        .with_context(|| format!("Failed to flush {}", dest.display()))?;
    Ok(written)
}

/// The temp file a download of `dest` streams into: beside it (the same
/// volume, so the final rename is atomic) and named for this process, so a
/// desktop app and a background engine downloading the same ecosystem never
/// write one file.
fn part_path(dest: &Path) -> PathBuf {
    let name = dest.file_name().map_or_else(
        || "archive.zip".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    dest.with_file_name(format!("{name}.{}.part", std::process::id()))
}

/// Whether a file name is one of [`part_path`]'s temp files.
fn is_part_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("part"))
}

/// Remove `ecosystem`'s partial downloads that no live download can own.
/// Best effort.
fn remove_abandoned_parts(dir: &Path, ecosystem: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let prefix = format!("{ecosystem}.zip.");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !is_part_file(&name) {
            continue;
        }
        let abandoned = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age.as_secs() >= ABANDONED_PART_SECS);
        if abandoned {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Whether `path` opens as a ZIP archive (its central directory reads).
fn check_is_archive(path: &Path) -> Result<()> {
    let file =
        std::fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    zip::ZipArchive::new(BufReader::new(file))
        .map(|_| ())
        .map_err(|e| FourDaError::Internal(format!("the response is not a ZIP archive ({e})")))
}

/// Put a downloaded archive at `dest` without ever exposing a partial or
/// invalid file there.
///
/// The body streams to a temp file beside `dest`, must open as a ZIP archive,
/// and only then replaces `dest` by a rename. The download used to create
/// (truncate) `dest` first and write into it, so a connection that dropped
/// mid-body, an over-cap body or a proxy's HTML page destroyed the good
/// archive the offline fallback depends on. Any failure removes the temp file
/// and leaves `dest` as it was. Returns the bytes written.
async fn replace_archive<S, B, E>(body: S, dest: &Path, ecosystem: &str, cap: u64) -> Result<u64>
where
    S: futures::Stream<Item = std::result::Result<B, E>>,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    let part = part_path(dest);
    let downloaded = async {
        let written = stream_to_file(body, &part, ecosystem, cap).await?;
        crate::ipc_blocking::cpu_bound(|| check_is_archive(&part))
            .with_context(|| format!("Downloaded {ecosystem}/all.zip was rejected"))?;
        std::fs::rename(&part, dest)
            .with_context(|| format!("Failed to move the download to {}", dest.display()))?;
        Ok(written)
    }
    .await;
    if downloaded.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    downloaded
}

/// Extract a header value as an owned String.
fn header_str(headers: &reqwest::header::HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
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
    let entry = archive.by_index(index).map_err(|e| {
        debug!(target: "4da::osv::cache", index = index, error = %e, "Skipping unreadable ZIP entry");
        FourDaError::Internal(e.to_string())
    })?;

    // zip 9: `name()` fails when the raw name is neither UTF-8 nor CP437.
    // Counted as an entry error, like an unreadable entry above.
    let name = entry
        .name()
        .map_err(|e| {
            debug!(target: "4da::osv::cache", index = index, error = %e, "Skipping ZIP entry with an undecodable name");
            FourDaError::Internal(e.to_string())
        })?
        .into_owned();
    if !std::path::Path::new(&name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return Ok(None);
    }

    // Bounded read: one byte past the cap, so an over-cap entry is detectable
    // rather than silently truncated into "invalid JSON".
    let mut raw = Vec::new();
    entry
        .take(MAX_ADVISORY_JSON_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|e| {
            debug!(target: "4da::osv::cache", name = %name, error = %e, "Skipping unreadable entry");
            FourDaError::Io(e)
        })?;

    if raw.len() as u64 > MAX_ADVISORY_JSON_BYTES {
        warn!(
            target: "4da::osv::cache",
            name = %name,
            cap_bytes = MAX_ADVISORY_JSON_BYTES,
            "Skipping oversized advisory entry (decompression bomb guard)"
        );
        return Ok(None);
    }

    let contents = String::from_utf8(raw).map_err(|e| {
        debug!(target: "4da::osv::cache", name = %name, error = %e, "Skipping non-UTF8 entry");
        FourDaError::Internal(format!("non-UTF8 advisory entry: {name}"))
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
            if user_packages.contains(&super::matching::package_key(&pkg.name, &pkg.ecosystem)) {
                return true;
            }
        }
    }

    false
}

/// Get cache status for all known ecosystems.
pub fn get_all_cache_statuses() -> Result<Vec<CacheMeta>> {
    let mirror = Mirror::live();
    Ok(KNOWN_ECOSYSTEMS
        .iter()
        .filter_map(|eco| mirror.read_meta(eco))
        .collect())
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

    let mirror = Mirror::live();
    for (ecosystem, packages) in &by_ecosystem {
        update_single_ecosystem(&mirror, db, ecosystem, packages, &mut result).await;
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
    let mut deps = db
        .get_auditable_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read dependencies: {e}")))?;

    // Merge scanned deps while preserving project-hygiene filters.
    if let Ok(scanned) = db.get_auditable_scanned_dependencies() {
        deps.extend(scanned);
    }

    let mut by_ecosystem: std::collections::HashMap<String, HashSet<String>> =
        std::collections::HashMap::new();
    for dep in &deps {
        let osv_eco = super::sync::normalize_to_osv_pub(&dep.ecosystem);
        by_ecosystem
            .entry(osv_eco)
            .or_default()
            .insert(super::matching::package_key(
                &dep.package_name,
                &dep.ecosystem,
            ));
    }
    Ok(by_ecosystem)
}

/// Update cache for a single ecosystem: check staleness, download if needed, sync.
async fn update_single_ecosystem(
    mirror: &Mirror,
    db: &Database,
    ecosystem: &str,
    packages: &HashSet<String>,
    result: &mut CacheUpdateResult,
) {
    match mirror.freshness(ecosystem).await {
        Freshness::Current => {
            result.ecosystems_skipped.push(ecosystem.to_string());
            // Still sync from existing ZIP in case user packages changed
            sync_and_record(mirror, db, ecosystem, packages, result);
            return;
        }
        Freshness::Outdated => {}
        Freshness::Unknown(reason) => {
            // The remote could not be asked, so there is nothing to refresh,
            // and the archive on disk is unchanged: the sync pass that runs
            // before this at launch has matched it and recorded the failed
            // check. Downloading anyway and then re-reading the archive cost
            // an offline launch minutes of CPU (npm) for the rows it had.
            let msg = format!("{ecosystem} (check): {reason}");
            warn!(target: "4da::osv::cache", error = %msg, "Staleness check failed");
            result.errors.push(msg);
            return;
        }
    }

    match mirror.download(ecosystem).await {
        Ok(_) => {
            result.ecosystems_updated.push(ecosystem.to_string());
            sync_and_record(mirror, db, ecosystem, packages, result);
        }
        Err(e) => {
            let msg = format!("{ecosystem} (download): {e}");
            warn!(target: "4da::osv::cache", error = %msg, "Cache download failed");
            result.errors.push(msg);

            // Fall back to existing cache
            if mirror.zip_path(ecosystem).exists() {
                let _ = mirror
                    .sync_from_zip(db, ecosystem, packages)
                    .map(|c| result.total_advisories += c);
            }
        }
    }
}

/// Sync from ZIP and record the result (advisory count or error).
fn sync_and_record(
    mirror: &Mirror,
    db: &Database,
    ecosystem: &str,
    packages: &HashSet<String>,
    result: &mut CacheUpdateResult,
) {
    match mirror.sync_from_zip(db, ecosystem, packages) {
        Ok(count) => result.total_advisories += count,
        Err(e) => {
            let msg = format!("{ecosystem} (sync): {e}");
            warn!(target: "4da::osv::cache", error = %msg, "Cache sync failed");
            result.errors.push(msg);
        }
    }
}

// `pub(crate)`: the sync tests reuse its local stand-in for OSV.
#[cfg(test)]
#[path = "cache_tests.rs"]
pub(crate) mod tests;
