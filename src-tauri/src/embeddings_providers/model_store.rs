// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Where the in-process embedding model lives, and its on-demand download
//! (AD-054 decision 7).
//!
//! Release installers no longer carry the model (`scripts/release-embedding-model.cjs`
//! strips it at release time): the dependency engine needs no embeddings, and
//! the 261 MB of fp16 weights were most of every installer. Only opt-in
//! interests and natural-language search use them.
//!
//! Resolution order, first complete copy wins:
//! 1. the bundled resource (`<resources>/models/embeddings/...`): an older
//!    installer, a dev tree, or a build made with `--bundle`;
//! 2. the cache (`<cache>/models/nomic-embed-text-v1.5-fp16`): a previous
//!    download, kept across upgrades;
//! 3. a download, and only when something asks for one: an interest turned
//!    on, a natural-language search, or the Settings button. A background
//!    embed call never downloads; without the model it degrades (Ollama if it
//!    serves embeddings, else keyword matching).
//!
//! The download is the same pinned revision and the same bytes the installer
//! bundled (every file SHA-256 and size pinned), so the vector space is
//! identical. It resumes a partial file with an HTTP Range request, retries,
//! and reports progress through the engine status the UI polls.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::error::FourDaError;

pub(crate) const MODEL_REPO: &str = "nomic-ai/nomic-embed-text-v1.5";
/// Pinned: must equal `REVISION` in `scripts/download-embedding-model.cjs`.
pub(crate) const REVISION: &str = "e9b6763023c676ca8431644204f50c2b100d9aab";
/// The model's directory name, both in the bundle and in the cache.
pub(crate) const MODEL_DIR_NAME: &str = "nomic-embed-text-v1.5-fp16";

/// One pinned file of the model.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PinnedFile {
    pub remote: &'static str,
    pub local: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// Every file the engine loads, pinned to `REVISION`. Must equal the script's
/// `FILES` (`the_download_is_pinned_to_the_installer_bundle`).
pub(crate) const FILES: [PinnedFile; 5] = [
    PinnedFile {
        remote: "onnx/model_fp16.onnx",
        local: "model_fp16.onnx",
        sha256: "cf5b5a86edb00f895561803cfc04729090a958340b8ca2ad76c143f565f6bb04",
        size: 273_859_028,
    },
    PinnedFile {
        remote: "tokenizer.json",
        local: "tokenizer.json",
        sha256: "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66",
        size: 711_396,
    },
    PinnedFile {
        remote: "config.json",
        local: "config.json",
        sha256: "9ab00bd92cee80a569f708140b7b6c1661a65891ff3765b1519e181ba2f2c92b",
        size: 2_538,
    },
    PinnedFile {
        remote: "special_tokens_map.json",
        local: "special_tokens_map.json",
        sha256: "5d5b662e421ea9fac075174bb0688ee0d9431699900b90662acd44b2a350503a",
        size: 695,
    },
    PinnedFile {
        remote: "tokenizer_config.json",
        local: "tokenizer_config.json",
        sha256: "d7e0000bcc80134debd2222220427e6bf5fa20a669f40a0d0d1409cc18e0a9bc",
        size: 1_191,
    },
];

/// Bytes a full download fetches (the sum of `FILES`).
pub(crate) fn total_bytes(files: &[PinnedFile]) -> u64 {
    files.iter().map(|f| f.size).sum()
}

/// The download root: `{BASE_URL}/{remote}`.
pub(crate) fn base_url() -> String {
    format!("https://huggingface.co/{MODEL_REPO}/resolve/{REVISION}")
}

/// The error text when no copy is on disk. Callers match on the state, not
/// this text; it is what a log line says.
pub(crate) const NOT_DOWNLOADED: &str =
    "the local search model is not downloaded (Settings > Intelligence downloads it)";

// ============================================================================
// Resolution
// ============================================================================

/// Where a complete copy of the model was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModelLocation {
    Bundled(PathBuf),
    Cache(PathBuf),
}

impl ModelLocation {
    pub(crate) fn dir(&self) -> &Path {
        match self {
            Self::Bundled(p) | Self::Cache(p) => p,
        }
    }
}

/// Every file present at its pinned size. Checksums are verified when a file
/// is downloaded (a file only gets its final name once it matches), so a size
/// check is enough here and costs nothing on every embed call.
pub(crate) fn is_complete(dir: &Path, files: &[PinnedFile]) -> bool {
    files.iter().all(|f| {
        std::fs::metadata(dir.join(f.local)).is_ok_and(|m| m.is_file() && m.len() == f.size)
    })
}

/// Every complete copy, in load order: the bundled resource, then the cache.
pub(crate) fn installed(bundled: &Path, cache: &Path, files: &[PinnedFile]) -> Vec<ModelLocation> {
    let mut out = Vec::with_capacity(2);
    if is_complete(bundled, files) {
        out.push(ModelLocation::Bundled(bundled.to_path_buf()));
    }
    if is_complete(cache, files) {
        out.push(ModelLocation::Cache(cache.to_path_buf()));
    }
    out
}

/// `(bundled, cache)` model directories for this install.
pub(crate) fn locations() -> (PathBuf, PathBuf) {
    let paths = crate::runtime_paths::RuntimePaths::get();
    (
        paths
            .bundled_models_dir()
            .join("embeddings")
            .join(MODEL_DIR_NAME),
        paths.model_cache_dir().join(MODEL_DIR_NAME),
    )
}

/// The complete copies on this machine, in load order.
pub(crate) fn installed_here() -> Vec<ModelLocation> {
    let (bundled, cache) = locations();
    installed(&bundled, &cache, &FILES)
}

/// Whether any complete copy is on this machine.
pub(crate) fn is_installed() -> bool {
    !installed_here().is_empty()
}

// ============================================================================
// Download
// ============================================================================

/// How hard a download tries before reporting failure.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Retry {
    pub attempts: u32,
    pub backoff: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            attempts: 4,
            backoff: Duration::from_secs(2),
        }
    }
}

/// No byte for this long fails the read (the per-read timeout of a blocking
/// reqwest client): a stalled connection fails and is retried, resuming
/// where it stopped, instead of hanging forever.
const STALL_TIMEOUT: Duration = Duration::from_mins(1);

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    hash_into(path, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

fn hash_into(path: &Path, hasher: &mut Sha256) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&buf[..n]);
    }
}

/// A downloaded file is usable when it has its pinned size and checksum.
pub(crate) fn cached_ok(path: &Path, file: &PinnedFile) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() == file.size)
        && sha256_file(path).is_ok_and(|got| got == file.sha256)
}

fn part_path(dir: &Path, file: &PinnedFile) -> PathBuf {
    dir.join(format!("{}.part", file.local))
}

/// Download whatever is missing from `dir`, file by file.
/// `report(done, total, file)` is called as bytes arrive, over the whole set.
pub(crate) fn ensure_model(
    base_url: &str,
    files: &[PinnedFile],
    dir: &Path,
    retry: Retry,
    mut report: impl FnMut(u64, u64, &str),
) -> Result<PathBuf, FourDaError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| FourDaError::from(format!("model dir {}: {e}", dir.display())))?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(STALL_TIMEOUT)
        .redirect(crate::http_client::ssrf_guarded_redirect_policy())
        .build()
        .map_err(|e| FourDaError::from(format!("HTTP client init: {e}")))?;
    let total = total_bytes(files);
    let mut done_before = 0u64;
    for file in files {
        let dest = dir.join(file.local);
        if cached_ok(&dest, file) {
            done_before += file.size;
            report(done_before, total, file.local);
            continue;
        }
        let url = format!("{}/{}", base_url.trim_end_matches('/'), file.remote);
        let mut last_err = String::new();
        let mut done = false;
        for attempt in 1..=retry.attempts.max(1) {
            match fetch(&client, &url, &dest, file, |n| {
                report(done_before + n, total, file.local);
            }) {
                Ok(()) => {
                    done = true;
                    break;
                }
                Err(e) => {
                    tracing::warn!(target: "4da::embeddings", file = file.local, attempt, error = %e, "Embedding model download attempt failed");
                    last_err = e;
                    if attempt < retry.attempts {
                        std::thread::sleep(retry.backoff * attempt);
                    }
                }
            }
        }
        if !done {
            return Err(FourDaError::from(format!(
                "embedding model download failed ({}): {last_err}",
                file.local
            )));
        }
        done_before += file.size;
    }
    Ok(dir.to_path_buf())
}

/// Fetch one file into `<dest>.part`, resuming a partial one with a Range
/// request, and give it its final name only once size and checksum match.
/// A short read keeps the partial file so the next attempt resumes it; a
/// checksum mismatch deletes it.
fn fetch(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    file: &PinnedFile,
    mut report: impl FnMut(u64),
) -> Result<(), String> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let part = part_path(dir, file);
    let mut have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if have >= file.size {
        // A part as long as the whole file failed its checksum last time.
        let _ = std::fs::remove_file(&part);
        have = 0;
    }
    let mut request = client.get(url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().map_err(|e| e.to_string())?;
    let status = response.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        let _ = std::fs::remove_file(&part);
        return Err("HTTP 416 (partial file discarded)".into());
    }
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let resumed = have > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut hasher = Sha256::new();
    let mut out = if resumed {
        hash_into(&part, &mut hasher).map_err(|e| e.to_string())?;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&part)
            .map_err(|e| e.to_string())?
    } else {
        // The server ignored the Range (200): start over.
        have = 0;
        std::fs::File::create(&part).map_err(|e| e.to_string())?
    };
    let mut downloaded = have;
    let mut last_report = 0u64;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = response.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if downloaded + n as u64 > file.size {
            drop(out);
            let _ = std::fs::remove_file(&part);
            return Err(format!("longer than the pinned {} bytes", file.size));
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        downloaded += n as u64;
        if downloaded - last_report >= 1 << 20 {
            last_report = downloaded;
            report(downloaded);
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    drop(out);
    if downloaded != file.size {
        // Keep the partial file: the next attempt resumes it.
        return Err(format!("truncated: {downloaded} of {} bytes", file.size));
    }
    let got = hex::encode(hasher.finalize());
    if got != file.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(format!("SHA-256 mismatch (got {got})"));
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
    report(downloaded);
    Ok(())
}

// ============================================================================
// On demand
// ============================================================================

static DOWNLOADING: AtomicBool = AtomicBool::new(false);

/// Whether the one-time download is running now.
pub(crate) fn is_downloading() -> bool {
    DOWNLOADING.load(Ordering::SeqCst)
}

struct DownloadFlag;
impl Drop for DownloadFlag {
    fn drop(&mut self) {
        DOWNLOADING.store(false, Ordering::SeqCst);
    }
}

/// Start the one-time download on a background thread, then load the engine.
/// Returns false when a copy is already on disk or a download is running.
pub(crate) fn start_download(reason: &'static str) -> bool {
    if is_installed() {
        return false;
    }
    if DOWNLOADING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }
    let total = total_bytes(&FILES);
    super::fastembed_setup::set_preparing("Downloading the local search model", 0, total);
    tracing::info!(target: "4da::embeddings", reason, bytes = total, "Downloading the local search model (one time)");
    let spawned = std::thread::Builder::new()
        .name("4da-embedding-model-download".into())
        .spawn(move || {
            let _flag = DownloadFlag;
            let (_, cache) = locations();
            let result = ensure_model(&base_url(), &FILES, &cache, Retry::default(), |done, total, _| {
                super::fastembed_setup::set_preparing("Downloading the local search model", done, total);
            });
            match result {
                Ok(dir) => {
                    tracing::info!(target: "4da::embeddings", dir = %dir.display(), "Local search model downloaded and verified");
                    super::fastembed::load_after_download();
                }
                Err(e) => super::fastembed_setup::set_failed(&e.to_string()),
            }
        });
    if let Err(e) = spawned {
        DOWNLOADING.store(false, Ordering::SeqCst);
        super::fastembed_setup::set_failed(&format!("could not start the download: {e}"));
        return false;
    }
    true
}

/// What an on-demand request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OnDemand {
    /// A complete copy is on disk already.
    Installed,
    /// A download is already running.
    InProgress,
    /// Embeddings come from elsewhere (Ollama, or opted-in cloud), so the
    /// in-process model would never be used.
    NotNeeded,
    Started,
}

/// Whether a route ever reaches the in-process engine. The explicit Ollama
/// route and the opted-in cloud route never do.
pub(crate) fn route_uses_in_process_model(route: super::super::EmbeddingRoute) -> bool {
    use super::super::EmbeddingRoute as R;
    matches!(route, R::LocalDefault | R::AnthropicLocal)
}

/// A feature that needs embeddings is being used: download the model if this
/// install would embed in-process and has no copy. Never downloads when
/// Ollama already serves embeddings.
pub(crate) async fn request_on_demand(reason: &'static str) -> OnDemand {
    if is_installed() {
        return OnDemand::Installed;
    }
    if is_downloading() {
        return OnDemand::InProgress;
    }
    let llm = {
        let mut guard = crate::get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };
    let route = super::super::resolve_embedding_route(
        llm.provider.as_str(),
        !llm.api_key.is_empty(),
        !llm.openai_api_key.is_empty(),
        llm.allow_cloud_embeddings,
    );
    if !route_uses_in_process_model(route) {
        return OnDemand::NotNeeded;
    }
    let probe = vec!["4DA embedding probe".to_string()];
    let configured = llm.base_url.clone().filter(|u| !u.is_empty());
    for base in [configured, None] {
        if super::embed_texts_ollama(&probe, &base).await.is_ok() {
            tracing::debug!(target: "4da::embeddings", reason, "Ollama serves embeddings; the local search model is not needed");
            return OnDemand::NotNeeded;
        }
    }
    if start_download(reason) {
        OnDemand::Started
    } else {
        OnDemand::InProgress
    }
}

/// Spawn [`request_on_demand`] without waiting for it.
pub(crate) fn request_on_demand_in_background(reason: &'static str) {
    if is_installed() || is_downloading() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let outcome = request_on_demand(reason).await;
        tracing::debug!(target: "4da::embeddings", reason, ?outcome, "On-demand model request");
    });
}

/// An embed call found no model. If any interest is on, interests need
/// embeddings, so this is the "first use" that downloads the model (it also
/// covers an upgrade that removed a bundled copy). Checked at most every ten
/// minutes; with every interest off it never downloads.
pub(crate) fn on_missing_model() {
    static LAST: parking_lot::Mutex<Option<std::time::Instant>> = parking_lot::Mutex::new(None);
    // Unit tests embed without a model all the time; they must never reach
    // the settings database or the network from here.
    if cfg!(test) {
        return;
    }
    {
        let mut last = LAST.lock();
        if last.is_some_and(|t| t.elapsed() < Duration::from_mins(10)) {
            return;
        }
        *last = Some(std::time::Instant::now());
    }
    tauri::async_runtime::spawn(async {
        let interests_on = tokio::task::spawn_blocking(any_interest_enabled)
            .await
            .unwrap_or(false);
        if interests_on {
            let outcome = request_on_demand("interests-enabled").await;
            tracing::info!(target: "4da::embeddings", ?outcome, "An interest is on and the local search model is missing");
        }
    });
}

fn any_interest_enabled() -> bool {
    crate::get_database()
        .ok()
        .and_then(|db| db.get_all_sources().ok())
        .is_some_and(|rows| {
            rows.iter().any(|(source_type, _, enabled, _)| {
                *enabled && crate::sources::source_class::is_interest(source_type)
            })
        })
}

#[cfg(test)]
#[path = "model_store_tests.rs"]
mod tests;
