// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Model download manager for the built-in LLM sidecar.
//!
//! Downloads GGUF model files from HuggingFace CDN with HTTP range resume,
//! SHA256 verification, and progress reporting. Models are stored in
//! `~/.4da/models/` so they persist across app updates.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

use crate::error::{FourDaError, Result, ResultExt};

/// A downloadable model in the catalog.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModelCatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub family: &'static str,
    pub size_bytes: u64,
    pub sha256: &'static str,
    pub url: &'static str,
    pub filename: &'static str,
    pub min_ram_gb: f64,
    pub quantization: &'static str,
}

/// Download progress event payload.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
    pub status: DownloadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DownloadStatus {
    Downloading,
    Verifying,
    Complete,
    Failed,
    Cancelled,
}

// Abort flag for cancelling downloads
static DOWNLOAD_ABORT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn cancel_download() {
    DOWNLOAD_ABORT.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn is_cancelled() -> bool {
    DOWNLOAD_ABORT.load(std::sync::atomic::Ordering::Relaxed)
}

/// The model download catalog.
/// HuggingFace URLs point to specific GGUF quantizations that balance quality and RAM.
/// SHA256 hashes are for the complete downloaded file.
static CATALOG: &[ModelCatalogEntry] = &[
    ModelCatalogEntry {
        id: "qwen3-14b-q4km",
        display_name: "Qwen 3 14B (Q4_K_M)",
        family: "qwen3:14b",
        size_bytes: 9_001_752_960,
        sha256: "500a8806e85ee9c83f3ae08420295592451379b4f8cf2d0f41c15dffeb6b81f0",
        url: "https://huggingface.co/Qwen/Qwen3-14B-GGUF/resolve/main/Qwen3-14B-Q4_K_M.gguf",
        filename: "qwen3-14b-q4_k_m.gguf",
        min_ram_gb: 11.0,
        quantization: "Q4_K_M",
    },
    ModelCatalogEntry {
        id: "gemma3-12b-q4",
        display_name: "Gemma 3 12B (Q4_0)",
        family: "gemma3:12b",
        size_bytes: 6_909_282_656,
        sha256: "80acb48f341b6c85744c1a45080a0de406a6f5449618f8ba5cde03fb5e1df5eb",
        url: "https://huggingface.co/unsloth/gemma-3-12b-it-GGUF/resolve/main/gemma-3-12b-it-Q4_0.gguf",
        filename: "gemma-3-12b-it-Q4_0.gguf",
        min_ram_gb: 9.0,
        quantization: "Q4_0 (QAT)",
    },
    ModelCatalogEntry {
        id: "qwen3-8b-q4km",
        display_name: "Qwen 3 8B (Q4_K_M)",
        family: "qwen3:8b",
        size_bytes: 5_027_783_488,
        sha256: "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785",
        url: "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf",
        filename: "Qwen3-8B-Q4_K_M.gguf",
        min_ram_gb: 7.0,
        quantization: "Q4_K_M",
    },
];

/// Returns the catalog of downloadable models.
pub(crate) fn model_catalog() -> &'static [ModelCatalogEntry] {
    CATALOG
}

/// Resolve the models directory: `~/.4da/models/`
pub(crate) fn models_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| FourDaError::Internal("Cannot determine home directory".into()))?;
    let dir = home.join(".4da").join("models");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).context("Failed to create models directory")?;
    }
    Ok(dir)
}

/// Check if a model is already downloaded and valid.
pub(crate) fn is_model_downloaded(entry: &ModelCatalogEntry) -> bool {
    let Ok(dir) = models_dir() else {
        return false;
    };
    let path = dir.join(entry.filename);
    if !path.exists() {
        return false;
    }
    match std::fs::metadata(&path) {
        Ok(meta) if entry.size_bytes > 0 => meta.len() == entry.size_bytes,
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Get the full path to a downloaded model file.
pub(crate) fn model_path(entry: &ModelCatalogEntry) -> Option<PathBuf> {
    let dir = models_dir().ok()?;
    let path = dir.join(entry.filename);
    path.exists().then_some(path)
}

/// List all downloaded models with their catalog info.
#[allow(dead_code)] // REMOVE BY 2026-07-01
pub(crate) fn list_downloaded_models() -> Vec<(&'static ModelCatalogEntry, PathBuf)> {
    let Ok(dir) = models_dir() else {
        return vec![];
    };
    CATALOG
        .iter()
        .filter_map(|entry| {
            let path = dir.join(entry.filename);
            if path.exists() {
                Some((entry, path))
            } else {
                None
            }
        })
        .collect()
}

/// Find a catalog entry by model ID.
pub(crate) fn find_model(id: &str) -> Option<&'static ModelCatalogEntry> {
    CATALOG.iter().find(|e| e.id == id)
}

/// Recommend the best model for the user's hardware.
pub(crate) fn recommend_model(available_ram_gb: f64) -> Option<&'static ModelCatalogEntry> {
    CATALOG.iter().find(|e| e.min_ram_gb <= available_ram_gb)
}

/// Download a model from HuggingFace with resume support.
///
/// Emits `model-download-progress` events via the Tauri app handle.
/// Supports cancellation via `cancel_download()`.
pub(crate) async fn download_model<F>(entry: &ModelCatalogEntry, on_progress: F) -> Result<PathBuf>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    DOWNLOAD_ABORT.store(false, std::sync::atomic::Ordering::Relaxed);

    let dir = models_dir()?;
    let final_path = dir.join(entry.filename);
    let partial_path = dir.join(format!("{}.part", entry.filename));

    let existing_bytes = if partial_path.exists() {
        std::fs::metadata(&partial_path)
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    if final_path.exists() {
        let meta = std::fs::metadata(&final_path).context("Failed to read model file")?;
        if entry.size_bytes == 0 || meta.len() == entry.size_bytes {
            info!(model = entry.id, "Model already downloaded");
            on_progress(DownloadProgress {
                model_id: entry.id.to_string(),
                downloaded_bytes: meta.len(),
                total_bytes: meta.len(),
                percent: 100.0,
                status: DownloadStatus::Complete,
            });
            return Ok(final_path);
        }
        warn!(
            model = entry.id,
            expected = entry.size_bytes,
            actual = meta.len(),
            "Model file exists but size mismatch — re-downloading"
        );
        std::fs::remove_file(&final_path).context("Failed to remove corrupt model file")?;
    }

    info!(
        model = entry.id,
        url = entry.url,
        resume_from = existing_bytes,
        "Starting model download"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;

    let mut request = client.get(entry.url);
    if existing_bytes > 0 {
        request = request.header("Range", format!("bytes={existing_bytes}-"));
        debug!(
            model = entry.id,
            resume_from = existing_bytes,
            "Resuming download"
        );
    }

    let response = request.send().await.context("Download request failed")?;

    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(FourDaError::Llm(format!(
            "Download failed: HTTP {}",
            response.status()
        )));
    }

    let total_bytes = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(entry.size_bytes)
    } else {
        response.content_length().unwrap_or(entry.size_bytes)
    };

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&partial_path)
        .await
        .context("Failed to open partial file for writing")?;

    let mut stream = response.bytes_stream();
    let mut downloaded = existing_bytes;
    let mut last_progress_pct = 0u8;

    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        if is_cancelled() {
            on_progress(DownloadProgress {
                model_id: entry.id.to_string(),
                downloaded_bytes: downloaded,
                total_bytes,
                percent: (downloaded as f64 / total_bytes as f64) * 100.0,
                status: DownloadStatus::Cancelled,
            });
            return Err(FourDaError::Llm("Download cancelled".into()));
        }

        let chunk = chunk.context("Error reading download stream")?;
        file.write_all(&chunk)
            .await
            .context("Failed to write to model file")?;
        downloaded += chunk.len() as u64;

        let pct = if total_bytes > 0 {
            ((downloaded as f64 / total_bytes as f64) * 100.0) as u8
        } else {
            0
        };

        if pct != last_progress_pct {
            last_progress_pct = pct;
            on_progress(DownloadProgress {
                model_id: entry.id.to_string(),
                downloaded_bytes: downloaded,
                total_bytes,
                percent: pct as f64,
                status: DownloadStatus::Downloading,
            });
        }
    }

    file.flush().await.context("Failed to flush model file")?;
    drop(file);

    // Verify size
    let final_size = std::fs::metadata(&partial_path)
        .context("Failed to read downloaded file")?
        .len();

    if entry.size_bytes > 0 && final_size != entry.size_bytes {
        error!(
            model = entry.id,
            expected = entry.size_bytes,
            actual = final_size,
            "Downloaded file size mismatch"
        );
        on_progress(DownloadProgress {
            model_id: entry.id.to_string(),
            downloaded_bytes: final_size,
            total_bytes: entry.size_bytes,
            percent: 0.0,
            status: DownloadStatus::Failed,
        });
        return Err(FourDaError::Llm(format!(
            "Download size mismatch: expected {} bytes, got {final_size}",
            entry.size_bytes
        )));
    }

    // SHA256 verification (if hash is provided)
    if !entry.sha256.is_empty() {
        on_progress(DownloadProgress {
            model_id: entry.id.to_string(),
            downloaded_bytes: final_size,
            total_bytes: final_size,
            percent: 100.0,
            status: DownloadStatus::Verifying,
        });

        let hash = hash_file_sha256(&partial_path).await?;
        if hash != entry.sha256 {
            error!(
                model = entry.id,
                expected = entry.sha256,
                actual = hash,
                "SHA256 verification failed"
            );
            on_progress(DownloadProgress {
                model_id: entry.id.to_string(),
                downloaded_bytes: final_size,
                total_bytes: final_size,
                percent: 0.0,
                status: DownloadStatus::Failed,
            });
            std::fs::remove_file(&partial_path).ok();
            return Err(FourDaError::Llm(
                "SHA256 verification failed — file may be corrupt".into(),
            ));
        }
        info!(model = entry.id, "SHA256 verification passed");
    }

    // Rename .part to final
    std::fs::rename(&partial_path, &final_path)
        .context("Failed to rename downloaded model file")?;

    on_progress(DownloadProgress {
        model_id: entry.id.to_string(),
        downloaded_bytes: final_size,
        total_bytes: final_size,
        percent: 100.0,
        status: DownloadStatus::Complete,
    });

    info!(
        model = entry.id,
        path = %final_path.display(),
        size_mb = final_size / (1024 * 1024),
        "Model download complete"
    );

    Ok(final_path)
}

/// Compute SHA256 hash of a file (streaming, constant memory).
async fn hash_file_sha256(path: &Path) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path)
        .await
        .context("Failed to open file for hashing")?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024]; // 1MB buffer
    loop {
        let n = file
            .read(&mut buf)
            .await
            .context("Failed to read file for hashing")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Delete a downloaded model.
pub(crate) fn delete_model(entry: &ModelCatalogEntry) -> Result<()> {
    let dir = models_dir()?;
    let path = dir.join(entry.filename);
    if path.exists() {
        std::fs::remove_file(&path).context("Failed to delete model file")?;
        info!(model = entry.id, "Model deleted");
    }
    let partial = dir.join(format!("{}.part", entry.filename));
    if partial.exists() {
        std::fs::remove_file(&partial).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_entries() {
        assert!(!CATALOG.is_empty());
        assert!(CATALOG.len() >= 3);
    }

    #[test]
    fn catalog_entries_have_required_fields() {
        for entry in CATALOG {
            assert!(!entry.id.is_empty());
            assert!(!entry.display_name.is_empty());
            assert!(!entry.family.is_empty());
            assert!(!entry.url.is_empty());
            assert!(!entry.filename.is_empty());
            assert!(entry.min_ram_gb > 0.0);
            assert!(entry.url.starts_with("https://"));
            assert!(entry.filename.ends_with(".gguf"));
        }
    }

    #[test]
    fn find_model_works() {
        assert!(find_model("qwen3-14b-q4km").is_some());
        assert!(find_model("nonexistent").is_none());
    }

    #[test]
    fn recommend_model_respects_ram() {
        let rec = recommend_model(32.0);
        assert!(rec.is_some());
        assert_eq!(rec.map(|e| e.id), Some("qwen3-14b-q4km"));

        let rec = recommend_model(8.0);
        assert!(rec.is_some());

        let rec = recommend_model(1.0);
        assert!(rec.is_none());
    }

    #[test]
    fn models_dir_creates_directory() {
        let dir = models_dir();
        assert!(dir.is_ok());
        assert!(dir.as_ref().is_ok_and(|d| d.exists()));
    }

    #[test]
    fn download_status_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Downloading).ok(),
            Some("\"downloading\"".to_string())
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Complete).ok(),
            Some("\"complete\"".to_string())
        );
    }

    #[test]
    fn catalog_sorted_by_size_descending() {
        for w in CATALOG.windows(2) {
            assert!(
                w[0].size_bytes >= w[1].size_bytes,
                "Catalog should be sorted largest-first: {} ({}) before {} ({})",
                w[0].id,
                w[0].size_bytes,
                w[1].id,
                w[1].size_bytes
            );
        }
    }
}
