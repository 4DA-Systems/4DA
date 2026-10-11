// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! In-process fastembed provider (ONNX Runtime — zero network dependency).
//!
//! Handles ONNX runtime download/setup, loading the model from the copy
//! `model_store` resolves (bundled resource, then cache; never a download),
//! fastembed initialization, and the `prepare_embedding_engine` Tauri command.

#[cfg(feature = "fastembed-local")]
use once_cell::sync::OnceCell;

#[cfg(feature = "fastembed-local")]
use std::io::{Read as _, Write as _};

use crate::error::{FourDaError, Result};

#[cfg(feature = "fastembed-local")]
static FASTEMBED_MODEL: OnceCell<parking_lot::Mutex<fastembed::TextEmbedding>> = OnceCell::new();

#[cfg(feature = "fastembed-local")]
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct DownloadProgress {
    pub stage: String,
    pub percent: u32,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub message: String,
    pub done: bool,
}

#[cfg(feature = "fastembed-local")]
fn ort_lib_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    }
}

#[cfg(feature = "fastembed-local")]
#[allow(clippy::large_stack_arrays)]
fn ensure_ort_runtime(
    cache_dir: &std::path::Path,
    progress: Option<&std::sync::mpsc::Sender<DownloadProgress>>,
) -> std::result::Result<(), FourDaError> {
    let lib_name = ort_lib_filename();

    if std::env::var("ORT_DYLIB_PATH").is_ok() {
        return Ok(());
    }

    let bundled_path = crate::runtime_paths::RuntimePaths::get()
        .bundled_models_dir()
        .join("ort")
        .join(lib_name);
    if bundled_path.exists() {
        std::env::set_var("ORT_DYLIB_PATH", &bundled_path);
        tracing::info!(target: "4da::embeddings", path = %bundled_path.display(), "Using bundled ORT runtime");
        return Ok(());
    }

    let ort_dir = cache_dir.join("ort");
    let dll_path = ort_dir.join(lib_name);

    if dll_path.exists() {
        std::env::set_var("ORT_DYLIB_PATH", &dll_path);
        return Ok(());
    }

    let _ = std::fs::create_dir_all(&ort_dir);
    tracing::info!(
        target: "4da::embeddings",
        dest = %ort_dir.display(),
        "Downloading ONNX Runtime 1.24.2 (one-time, ~70MB)"
    );

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-arm64-1.24.2.zip";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-osx-arm64-1.24.2.tgz";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-osx-x86_64-1.24.2.tgz";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-linux-x64-1.24.2.tgz";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-linux-aarch64-1.24.2.tgz";

    let archive_path = ort_dir.join("ort_download.tmp");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_mins(10))
        .redirect(crate::http_client::ssrf_guarded_redirect_policy())
        .build()
        .map_err(|e| FourDaError::from(format!("HTTP client init: {e}")))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| FourDaError::from(format!("ORT download failed: {e}")))?;
    if !response.status().is_success() {
        return Err(FourDaError::from(format!(
            "ORT download HTTP {}",
            response.status()
        )));
    }
    let total = response.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(&archive_path)
        .map_err(|e| FourDaError::from(format!("create archive: {e}")))?;
    let mut downloaded: u64 = 0;
    let mut last_report: u64 = 0;
    let mut buf = [0u8; 32_768];
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| FourDaError::from(format!("download read: {e}")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| FourDaError::from(format!("write chunk: {e}")))?;
        downloaded += n as u64;
        if downloaded - last_report > 524_288 {
            last_report = downloaded;
            super::fastembed_setup::set_preparing("Downloading ONNX Runtime", downloaded, total);
            if let Some(tx) = progress.as_ref() {
                let pct = if total > 0 {
                    ((downloaded as f64 / total as f64) * 50.0) as u32
                } else {
                    0
                };
                let _ = tx.send(DownloadProgress {
                    stage: "ort-download".into(),
                    percent: pct,
                    bytes_downloaded: downloaded,
                    bytes_total: total,
                    message: format!(
                        "Downloading ONNX Runtime... {:.1}MB / {:.1}MB",
                        downloaded as f64 / 1_048_576.0,
                        total as f64 / 1_048_576.0
                    ),
                    done: false,
                });
            }
        }
    }
    drop(file);

    if let Some(tx) = progress.as_ref() {
        let _ = tx.send(DownloadProgress {
            stage: "ort-extract".into(),
            percent: 45,
            bytes_downloaded: downloaded,
            bytes_total: total,
            message: "Extracting ONNX Runtime...".into(),
            done: false,
        });
    }

    extract_ort_library(&archive_path, &dll_path)?;
    let _ = std::fs::remove_file(&archive_path);

    std::env::set_var("ORT_DYLIB_PATH", &dll_path);
    tracing::info!(target: "4da::embeddings", path = %dll_path.display(), "ONNX Runtime ready");
    Ok(())
}

#[cfg(feature = "fastembed-local")]
fn extract_ort_library(
    archive_path: &std::path::Path,
    dll_dest: &std::path::Path,
) -> std::result::Result<(), FourDaError> {
    let lib_name = ort_lib_filename();
    if archive_path
        .extension()
        .is_some_and(|e| e == "zip" || e == "tmp")
    {
        let file = std::fs::File::open(archive_path)
            .map_err(|e| FourDaError::from(format!("open archive: {e}")))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| FourDaError::from(format!("invalid zip: {e}")))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| FourDaError::from(format!("zip entry: {e}")))?;
            let name = entry.name().to_string();
            if name.ends_with(lib_name) {
                let mut out = std::fs::File::create(dll_dest)
                    .map_err(|e| FourDaError::from(format!("create dll: {e}")))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| FourDaError::from(format!("extract dll: {e}")))?;
                return Ok(());
            }
        }
        return Err(FourDaError::from(format!(
            "{lib_name} not found in archive"
        )));
    }
    let file = std::fs::File::open(archive_path)
        .map_err(|e| FourDaError::from(format!("open tgz: {e}")))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| FourDaError::from(format!("tar entries: {e}")))?
    {
        let mut entry = entry.map_err(|e| FourDaError::from(format!("tar entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| FourDaError::from(format!("tar path: {e}")))?;
        if path.to_string_lossy().ends_with(lib_name) {
            let mut out = std::fs::File::create(dll_dest)
                .map_err(|e| FourDaError::from(format!("create lib: {e}")))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| FourDaError::from(format!("extract lib: {e}")))?;
            return Ok(());
        }
    }
    Err(FourDaError::from(format!("{lib_name} not found in tgz")))
}

// ============================================================================
// The in-process model: read straight from the installer's bundled resources
// ============================================================================

/// nomic-embed-text v1.5, fp16 weights (274 MB; inputs and outputs stay f32).
/// It is the model Ollama serves by default and writes the SAME vector space:
/// measured median cosine 1.00000 (p5 0.99999) against Ollama's nomic on 400
/// real items. The model it replaced, Arctic-embed-M, shared no space with
/// nomic at all (median cosine 0.007) and scored significantly worse on the
/// blind gold (combined AUC 0.689 vs 0.764). The smaller int8 build was
/// rejected: ~0.90 agreement is too noisy for the top-1 match over thousands
/// of code chunks (context AUC 0.499). See `reembed_space`.
#[cfg(feature = "fastembed-local")]
const BUNDLED_ONNX: &str = "model_fp16.onnx";

/// Caches of retired in-process models (hf-hub layout). Nothing reads them any
/// more; each is reclaimed on first init. The nomic entry is the f32 build
/// fastembed's own downloader used to fetch (~550 MB); the fallback now
/// fetches the installer's fp16 files instead (`fastembed_setup::download`).
#[cfg(feature = "fastembed-local")]
const LEGACY_CACHE_DIRS: [&str; 2] = [
    "models--Snowflake--snowflake-arctic-embed-m",
    "models--nomic-ai--nomic-embed-text-v1.5",
];

#[cfg(feature = "fastembed-local")]
fn remove_legacy_caches(cache_dir: &std::path::Path) {
    for name in LEGACY_CACHE_DIRS {
        let dir = cache_dir.join(name);
        if dir.is_dir() {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => {
                    tracing::info!(target: "4da::embeddings", path = %dir.display(), "Removed a retired embedding model cache")
                }
                Err(e) => {
                    tracing::debug!(target: "4da::embeddings", error = %e, "Could not remove a retired embedding model cache")
                }
            }
        }
    }
}

/// Load the fp16 build from `dir` (a bundled resource is read in place,
/// never copied).
#[cfg(feature = "fastembed-local")]
fn load_bundled_nomic(
    dir: &std::path::Path,
) -> std::result::Result<fastembed::TextEmbedding, String> {
    let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let tokenizer_files = fastembed::TokenizerFiles {
        tokenizer_file: read("tokenizer.json")?,
        config_file: read("config.json")?,
        special_tokens_map_file: read("special_tokens_map.json")?,
        tokenizer_config_file: read("tokenizer_config.json")?,
    };
    let model = fastembed::UserDefinedEmbeddingModel::new(read(BUNDLED_ONNX)?, tokenizer_files)
        .with_pooling(fastembed::Pooling::Mean);
    fastembed::TextEmbedding::try_new_from_user_defined(
        model,
        fastembed::InitOptionsUserDefined::new(),
    )
    .map_err(|e| e.to_string())
}

/// The in-process embedding model, from the first copy that loads, in
/// `model_store` resolution order (bundled resource, then the cache). Never
/// downloads: that is `model_store::start_download`, on demand.
#[cfg(feature = "fastembed-local")]
fn load_embedding_model(
    copies: &[super::model_store::ModelLocation],
) -> std::result::Result<fastembed::TextEmbedding, FourDaError> {
    let mut last_err = String::from(super::model_store::NOT_DOWNLOADED);
    for copy in copies {
        let dir = copy.dir();
        tracing::info!(target: "4da::embeddings", ?copy, "Loading the local search model (nomic-embed-text v1.5, fp16)");
        match load_bundled_nomic(dir) {
            Ok(model) => return Ok(model),
            Err(e) => {
                tracing::warn!(target: "4da::embeddings", dir = %dir.display(), error = %e, "Embedding model copy failed to load");
                last_err = e;
            }
        }
    }
    Err(FourDaError::from(format!("fastembed init: {last_err}")))
}

/// Build the engine once, recording each step in the status the UI reads.
/// With no copy of the model on disk it fails at once and leaves the status
/// alone (`not_downloaded`, or a download's progress).
#[cfg(feature = "fastembed-local")]
fn init_engine(
    progress: Option<&std::sync::mpsc::Sender<DownloadProgress>>,
) -> std::result::Result<parking_lot::Mutex<fastembed::TextEmbedding>, FourDaError> {
    let cache_dir = crate::runtime_paths::RuntimePaths::get().model_cache_dir();
    remove_legacy_caches(&cache_dir);
    let copies = super::model_store::installed_here();
    if copies.is_empty() {
        return Err(FourDaError::from(super::model_store::NOT_DOWNLOADED));
    }
    super::fastembed_setup::set_preparing("Starting the local search model", 0, 0);
    let result = ensure_ort_runtime(&cache_dir, progress)
        .and_then(|()| load_embedding_model(&copies))
        .map(parking_lot::Mutex::new);
    match &result {
        Ok(_) => super::fastembed_setup::set_ready(),
        Err(e) => super::fastembed_setup::set_failed(&e.to_string()),
    }
    result
}

#[cfg(feature = "fastembed-local")]
fn get_or_init_fastembed(
) -> std::result::Result<&'static parking_lot::Mutex<fastembed::TextEmbedding>, FourDaError> {
    FASTEMBED_MODEL.get_or_try_init(|| init_engine(None))
}

/// Load the engine now that a copy is on disk (after a download, or when the
/// user asks for a model that is already there), so the status reaches
/// Ready, or Failed with the reason.
#[cfg(feature = "fastembed-local")]
pub(crate) fn load_after_download() {
    if let Err(e) = get_or_init_fastembed() {
        tracing::warn!(target: "4da::embeddings", error = %e, "Local search model is on disk but did not load");
    }
}

/// Embedding chunk size. Same OOM class as the cross-encoder reranker
/// (findings #3 antibody): embedding ALL texts in a single ONNX batch spikes
/// activation memory on large inputs (e.g. re-embedding ~1000 items). We bound
/// peak memory by chunking the input HERE, then pass `None` to fastembed's own
/// batching (a dynamically-quantized model rejects `Some(n)` with "Dynamic
/// quantization cannot be used with batching"; chunking here keeps the memory
/// guard for any model, regardless of the caller).
#[cfg(feature = "fastembed-local")]
const EMBED_BATCH_SIZE: usize = 32;

#[cfg(feature = "fastembed-local")]
pub(in crate::embeddings) fn embed_texts_fastembed_sync(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let model_mutex = get_or_init_fastembed()?;
    let mut model = model_mutex.lock();
    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(EMBED_BATCH_SIZE) {
        let str_refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        // `None`: bound memory via the chunk above, not fastembed's internal
        // batching.
        let embedded = model
            .embed(str_refs, None)
            .map_err(|e| FourDaError::from(format!("fastembed embed: {e}")))?;
        out.extend(embedded);
    }
    Ok(out)
}

#[cfg(feature = "fastembed-local")]
pub(in crate::embeddings) fn init_fastembed_with_progress(
    progress: Option<std::sync::mpsc::Sender<DownloadProgress>>,
) -> std::result::Result<(), FourDaError> {
    let _ = FASTEMBED_MODEL.get_or_try_init(|| {
        if let Some(tx) = progress.as_ref() {
            let _ = tx.send(DownloadProgress {
                stage: "model-init".into(),
                percent: 50,
                bytes_downloaded: 0,
                bytes_total: 0,
                message: "Loading the local search model...".to_string(),
                done: false,
            });
        }

        let result = init_engine(progress.as_ref());

        if let Some(tx) = progress.as_ref() {
            let _ = tx.send(DownloadProgress {
                stage: "ready".into(),
                percent: 100,
                bytes_downloaded: 0,
                bytes_total: 0,
                message: "Embedding engine ready".into(),
                done: true,
            });
        }

        result
    })?;
    Ok(())
}

#[cfg(feature = "fastembed-local")]
#[tauri::command]
pub async fn prepare_embedding_engine(
    app: tauri::AppHandle,
) -> std::result::Result<serde_json::Value, String> {
    use tauri::Emitter;

    if FASTEMBED_MODEL.get().is_some() {
        return Ok(serde_json::json!({"status": "ready", "cached": true}));
    }
    // AD-054 decision 7: preparing loads a copy that is on disk; it never
    // starts the one-time download (`download_embedding_model` does).
    if !super::model_store::is_installed() {
        return Ok(serde_json::json!({"status": "not_downloaded"}));
    }

    let (tx, rx) = std::sync::mpsc::channel::<DownloadProgress>();
    let init_handle = tokio::task::spawn_blocking(move || init_fastembed_with_progress(Some(tx)));
    loop {
        match rx.try_recv() {
            Ok(progress) => {
                let _ = app.emit("embedding-setup-progress", &progress);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    init_handle
        .await
        .map_err(|e| format!("init task panicked: {e}"))?
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({"status": "ready", "cached": false}))
}

#[cfg(not(feature = "fastembed-local"))]
#[tauri::command]
pub async fn prepare_embedding_engine() -> std::result::Result<serde_json::Value, String> {
    Ok(
        serde_json::json!({"status": "unavailable", "reason": "fastembed-local feature not enabled"}),
    )
}

#[cfg(all(test, feature = "fastembed-local"))]
mod tests {
    use super::*;

    /// No copy on disk: loading fails at once with the "not downloaded"
    /// reason. It never reaches the network (the download is on demand only).
    #[test]
    fn with_no_copy_loading_fails_fast_without_downloading() {
        let started = std::time::Instant::now();
        let err = load_embedding_model(&[]).err().map(|e| e.to_string());
        assert!(
            err.as_deref()
                .is_some_and(|e| e.contains(super::super::model_store::NOT_DOWNLOADED)),
            "{err:?}"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    /// A copy that is present but unreadable reports why it failed.
    #[test]
    fn a_broken_copy_reports_its_error() {
        let dir = std::env::temp_dir().join(format!("4da-broken-model-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let copies = [super::super::model_store::ModelLocation::Cache(dir.clone())];
        let err = load_embedding_model(&copies).err().map(|e| e.to_string());
        assert!(
            err.as_deref().is_some_and(|e| e.contains("tokenizer.json")),
            "{err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
