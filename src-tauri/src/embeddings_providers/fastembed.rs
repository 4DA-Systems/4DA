// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! In-process fastembed provider (ONNX Runtime — zero network dependency).
//!
//! Handles ONNX runtime download/setup, bundled model copy, fastembed initialization,
//! and the `prepare_embedding_engine` Tauri command.

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
        .timeout(std::time::Duration::from_secs(600))
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
        if let Some(tx) = progress.as_ref() {
            if downloaded - last_report > 524_288 || n == 0 {
                last_report = downloaded;
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
// Bundled embedding model — copy from installer to writable cache on first run
// ============================================================================

#[cfg(feature = "fastembed-local")]
const EMBEDDING_CACHE_DIR_NAME: &str = "models--Snowflake--snowflake-arctic-embed-m";

#[cfg(feature = "fastembed-local")]
fn ensure_embedding_model(cache_dir: &std::path::Path) -> bool {
    let model_dir = cache_dir.join(EMBEDDING_CACHE_DIR_NAME);
    let refs_file = model_dir.join("refs").join("main");

    if refs_file.exists() {
        tracing::debug!(target: "4da::embeddings", "Embedding model already cached");
        return true;
    }

    let bundled_dir = crate::runtime_paths::RuntimePaths::get()
        .bundled_models_dir()
        .join("embeddings")
        .join(EMBEDDING_CACHE_DIR_NAME);

    if !bundled_dir.exists() {
        tracing::debug!(target: "4da::embeddings", "No bundled embedding model — will download on first use");
        return false;
    }

    tracing::info!(
        target: "4da::embeddings",
        bundled = %bundled_dir.display(),
        cache = %model_dir.display(),
        "Copying bundled embedding model to cache"
    );

    if let Err(e) = copy_dir_recursive(&bundled_dir, &model_dir) {
        tracing::warn!(target: "4da::embeddings", error = %e, "Bundled model copy failed — will download");
        return false;
    }
    true
}

#[cfg(feature = "fastembed-local")]
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(feature = "fastembed-local")]
fn get_or_init_fastembed(
) -> std::result::Result<&'static parking_lot::Mutex<fastembed::TextEmbedding>, FourDaError> {
    FASTEMBED_MODEL.get_or_try_init(|| {
        let cache_dir = crate::runtime_paths::RuntimePaths::get().model_cache_dir();
        ensure_ort_runtime(&cache_dir, None)?;
        let cached = ensure_embedding_model(&cache_dir);
        let msg = if cached {
            "Loading bundled embedding model (snowflake-arctic-embed-m)"
        } else {
            "Downloading embedding model (snowflake-arctic-embed-m, ~220MB first run)"
        };
        tracing::info!(target: "4da::embeddings", cache = %cache_dir.display(), "{msg}");
        let options =
            fastembed::InitOptions::new(fastembed::EmbeddingModel::SnowflakeArcticEmbedMQ)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(!cached);
        fastembed::TextEmbedding::try_new(options)
            .map(parking_lot::Mutex::new)
            .map_err(|e| {
                tracing::warn!(target: "4da::embeddings", error = %e, "fastembed init failed");
                FourDaError::from(format!("fastembed init: {e}"))
            })
    })
}

/// Embedding chunk size. Same OOM class as the cross-encoder reranker
/// (findings #3 antibody): embedding ALL texts in a single ONNX batch spikes
/// activation memory on large inputs (e.g. re-embedding ~1000 items). We bound
/// peak memory by chunking the input HERE, then pass `None` to fastembed's own
/// batching — which is incompatible with the dynamically-quantized model
/// (SnowflakeArcticEmbedMQ): `model.embed(.., Some(n))` errors with "Dynamic
/// quantization cannot be used with batching." Chunking at this layer keeps the
/// memory guard without tripping that constraint, regardless of the caller.
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
        // batching (which the quantized model rejects).
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
        let cache_dir = crate::runtime_paths::RuntimePaths::get().model_cache_dir();
        ensure_ort_runtime(&cache_dir, progress.as_ref())?;
        let cached = ensure_embedding_model(&cache_dir);

        if let Some(tx) = progress.as_ref() {
            let msg = if cached {
                "Loading bundled embedding model...".to_string()
            } else {
                "Downloading embedding model (~220MB first run)...".to_string()
            };
            let _ = tx.send(DownloadProgress {
                stage: "model-init".into(),
                percent: 50,
                bytes_downloaded: 0,
                bytes_total: 0,
                message: msg,
                done: false,
            });
        }

        let options =
            fastembed::InitOptions::new(fastembed::EmbeddingModel::SnowflakeArcticEmbedMQ)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(!cached);
        let result = fastembed::TextEmbedding::try_new(options)
            .map(parking_lot::Mutex::new)
            .map_err(|e| {
                tracing::warn!(target: "4da::embeddings", error = %e, "fastembed init failed");
                FourDaError::from(format!("fastembed init: {e}"))
            });

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
