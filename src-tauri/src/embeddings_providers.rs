// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Provider-specific embedding functions (OpenAI, Ollama, fastembed) and retry logic.
//!
//! Split from `embeddings.rs` to keep both modules under the 700-line threshold.

use crate::error::{FourDaError, Result, ResultExt};
use crate::get_settings_manager;

use super::{truncate_and_normalize, EMBEDDING_CLIENT};

// ============================================================================
// In-process fastembed provider (ONNX Runtime — zero network dependency)
// ============================================================================

#[cfg(feature = "fastembed-local")]
use once_cell::sync::OnceCell;

#[cfg(feature = "fastembed-local")]
use std::io::{Read as _, Write as _};

#[cfg(feature = "fastembed-local")]
static FASTEMBED_MODEL: OnceCell<parking_lot::Mutex<fastembed::TextEmbedding>> = OnceCell::new();

#[cfg(feature = "fastembed-local")]
#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct DownloadProgress {
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
fn ensure_ort_runtime(
    cache_dir: &std::path::Path,
    progress: Option<&std::sync::mpsc::Sender<DownloadProgress>>,
) -> std::result::Result<(), FourDaError> {
    let lib_name = ort_lib_filename();

    if std::env::var("ORT_DYLIB_PATH").is_ok() {
        return Ok(());
    }

    let bundled_path = crate::runtime_paths::RuntimePaths::get()
        .resource_dir
        .join("models")
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
        .resource_dir
        .join("models")
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

#[cfg(feature = "fastembed-local")]
pub(super) fn embed_texts_fastembed_sync(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let model_mutex = get_or_init_fastembed()?;
    let mut model = model_mutex.lock();
    let str_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    model
        .embed(str_refs, None)
        .map_err(|e| FourDaError::from(format!("fastembed embed: {e}")))
}

#[cfg(feature = "fastembed-local")]
pub(super) fn init_fastembed_with_progress(
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

// ============================================================================
// OpenAI provider
// ============================================================================

/// Generate embeddings using OpenAI API
pub(super) async fn embed_texts_openai(texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>> {
    if api_key.is_empty() {
        return Err("OpenAI API key not configured".into());
    }

    let body = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": texts,
        "dimensions": crate::EMBEDDING_DIMS
    });

    let response = EMBEDDING_CLIENT
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .context("OpenAI API request failed")?;

    // Check for rate limiting (HTTP 429) before consuming the response body
    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        tracing::warn!(
            target: "4da::embeddings",
            retry_after_secs = retry_after,
            "OpenAI rate limited — backing off"
        );
        return Err(format!("Rate limited by OpenAI (retry after {}s)", retry_after).into());
    }

    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let truncated = if body_text.len() > 200 {
            format!("{}...", &body_text[..body_text.floor_char_boundary(200)])
        } else {
            body_text
        };
        return Err(format!("OpenAI API error {}: {}", status.as_u16(), truncated).into());
    }

    let json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse OpenAI response")?;

    // Phase 5: Record usage from API response
    if let Some(usage) = json.get("usage") {
        let total_tokens = usage["total_tokens"].as_u64().unwrap_or(0);
        // text-embedding-3-small: $0.02 per 1M tokens = 0.002 cents per token
        let cost_cents = (total_tokens as f64 * 0.002 / 1000.0) as u64;
        let mut settings = get_settings_manager().lock();
        settings.record_usage(total_tokens, cost_cents);
    }

    let data = json["data"]
        .as_array()
        .ok_or_else(|| -> FourDaError { "Invalid OpenAI response: missing 'data' array".into() })?;

    data.iter()
        .map(|item| {
            item["embedding"]
                .as_array()
                .ok_or_else(|| -> FourDaError { "Missing embedding in response".into() })?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .map(|f| f as f32)
                        .ok_or_else(|| -> FourDaError { "Invalid embedding value".into() })
                })
                .collect::<Result<Vec<f32>>>()
        })
        .collect()
}

// ============================================================================
// Ollama provider
// ============================================================================

/// Validate that an Ollama endpoint URL is safe to use.
///
/// HTTP (unencrypted) connections are only permitted to localhost addresses
/// (127.0.0.1, localhost, [::1]) to prevent sending embedding data in cleartext
/// over the network. HTTPS connections are allowed to any host.
fn validate_ollama_endpoint(url: &str) -> Result<()> {
    // HTTPS is always safe — encryption protects the connection
    if url.starts_with("https://") {
        return Ok(());
    }

    // For HTTP, only allow localhost addresses
    if url.starts_with("http://") {
        let after_scheme = &url[7..]; // len("http://") == 7
        let host = after_scheme
            .split(|c: char| c == ':' || c == '/')
            .next()
            .unwrap_or("");

        if matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
            return Ok(());
        }

        tracing::info!(
            target: "4da::security",
            host = %host,
            "Blocked Ollama request to non-localhost HTTP endpoint"
        );
        return Err(FourDaError::Validation(
            "Ollama over HTTP is only allowed on localhost. Use HTTPS for remote Ollama instances."
                .into(),
        ));
    }

    // Unknown scheme — reject
    Err(FourDaError::Validation(format!(
        "Unsupported Ollama endpoint scheme: {url}"
    )))
}

/// Generate embeddings using Ollama API
pub(super) async fn embed_texts_ollama(
    texts: &[String],
    base_url: &Option<String>,
) -> Result<Vec<Vec<f32>>> {
    let env_host = std::env::var("OLLAMA_HOST").ok();
    let base = base_url
        .as_deref()
        .or(env_host.as_deref())
        .unwrap_or("http://localhost:11434");

    // Security: block unencrypted connections to non-localhost endpoints
    validate_ollama_endpoint(base)?;

    if texts.is_empty() {
        return Ok(vec![]);
    }

    let embedding_model = crate::reembed::get_embedding_model();

    let batch_body = serde_json::json!({
        "model": embedding_model,
        "input": texts,
    });

    // Try batch API first (/api/embed) - supported since Ollama v0.1.26
    let batch_result = EMBEDDING_CLIENT
        .post(format!("{base}/api/embed"))
        .json(&batch_body)
        .send()
        .await;

    match batch_result {
        Ok(response) if response.status().is_success() => {
            // Batch succeeded - parse embeddings array
            let json: serde_json::Value = response
                .json()
                .await
                .context("Failed to parse Ollama batch response")?;

            let embeddings_array =
                json["embeddings"]
                    .as_array()
                    .ok_or_else(|| -> FourDaError {
                        "Invalid Ollama batch response: missing 'embeddings' array".into()
                    })?;

            embeddings_array
                .iter()
                .map(|emb_val| {
                    let raw = emb_val
                        .as_array()
                        .ok_or_else(|| -> FourDaError {
                            "Invalid embedding in batch response".into()
                        })?
                        .iter()
                        .map(|v| {
                            v.as_f64()
                                .map(|f| f as f32)
                                .ok_or_else(|| -> FourDaError { "Invalid embedding value".into() })
                        })
                        .collect::<Result<Vec<f32>>>()?;
                    Ok(truncate_and_normalize(raw))
                })
                .collect()
        }
        Ok(response) => {
            // Batch endpoint returned an error - check for model-not-found
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || body.contains("not found") {
                return Err(format!(
                    "Embedding model '{}' not found in Ollama. Run: ollama pull {}",
                    embedding_model, embedding_model
                )
                .into());
            }
            // Fall through to single-item fallback for other errors (old Ollama version)
            embed_texts_ollama_single(texts, base).await
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("connect") || msg.contains("refused") {
                return Err(format!(
                    "Cannot connect to Ollama at {base}. Make sure Ollama is running (ollama serve)."
                )
                .into());
            }
            if msg.contains("timed out") || msg.contains("timeout") {
                return Err("Ollama embedding request timed out. The model may still be loading — try again shortly.".into());
            }
            // Fall through to single-item fallback
            embed_texts_ollama_single(texts, base).await
        }
    }
}

/// Fallback: embed one text at a time using the older /api/embeddings endpoint
async fn embed_texts_ollama_single(texts: &[String], base: &str) -> Result<Vec<Vec<f32>>> {
    let mut all_embeddings = Vec::with_capacity(texts.len());
    let embedding_model = crate::reembed::get_embedding_model();

    for text in texts {
        let single_body = serde_json::json!({
            "model": &embedding_model,
            "prompt": text,
        });

        let response = EMBEDDING_CLIENT
            .post(format!("{base}/api/embeddings"))
            .json(&single_body)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("refused") {
                    format!(
                        "Cannot connect to Ollama at {base}. Make sure Ollama is running (ollama serve)."
                    )
                } else if msg.contains("timed out") || msg.contains("timeout") {
                    "Ollama embedding timed out. The model may still be loading — try again.".to_string()
                } else {
                    format!(
                        "Ollama embedding request failed: {e}. Make sure Ollama is running with '{}' (run: ollama pull {})",
                        embedding_model, embedding_model
                    )
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || body.contains("not found") {
                return Err(format!(
                    "Embedding model '{}' not found. Run: ollama pull {}",
                    embedding_model, embedding_model
                )
                .into());
            }
            return Err(format!("Ollama embedding error ({status}): {body}").into());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        let raw = json["embedding"]
            .as_array()
            .ok_or_else(|| -> FourDaError {
                "Invalid Ollama response: missing 'embedding' array. Is the embedding model installed?"
                    .into()
            })?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| -> FourDaError {
                        "Invalid embedding value".into()
                    })
            })
            .collect::<Result<Vec<f32>>>()?;

        all_embeddings.push(truncate_and_normalize(raw));
    }

    Ok(all_embeddings)
}

// ============================================================================
// Retry logic
// ============================================================================

/// Retry an async operation with exponential backoff.
/// Returns the first successful result, or the last error after max_retries.
/// Rate-limit errors (containing "rate limit" or "429") use an extended backoff
/// of 30s instead of the normal exponential schedule.
pub(super) async fn retry_with_backoff<F, Fut, T>(
    operation_name: &str,
    max_retries: u32,
    f: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = String::new();
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = e.to_string();
                if attempt < max_retries {
                    // Detect rate-limit errors and use extended backoff
                    let lower = last_error.to_lowercase();
                    let is_rate_limited = lower.contains("rate limit")
                        || lower.contains("429")
                        || lower.contains("too many requests");

                    let delay_secs = if is_rate_limited {
                        // Parse retry-after hint from error message if present
                        let retry_after = lower
                            .find("retry after ")
                            .and_then(|pos| {
                                let after = &last_error[pos + 12..];
                                after
                                    .chars()
                                    .take_while(|c| c.is_ascii_digit())
                                    .collect::<String>()
                                    .parse::<u64>()
                                    .ok()
                            })
                            .unwrap_or(30);
                        tracing::warn!(
                            target: "4da::retry",
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            delay_secs = retry_after,
                            operation = operation_name,
                            "Rate limited — using extended backoff"
                        );
                        retry_after
                    } else {
                        let delay = 3u64.pow(attempt); // 1s, 3s, 9s
                        tracing::warn!(
                            target: "4da::retry",
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            delay_secs = delay,
                            operation = operation_name,
                            error = %last_error,
                            "Retrying after error"
                        );
                        delay
                    };
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }
            }
        }
    }
    Err(last_error.into())
}
