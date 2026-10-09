// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The in-process embedding engine's setup state, and the one-time model
//! download for a build that does not carry the bundled model.
//!
//! Status: the onboarding summary said "Private semantic search active" with a
//! check mark while the model was still downloading (fresh-profile E2E
//! 2026-10-09). [`engine_status`] reports what is actually true, and
//! `get_embedding_engine_status` hands it to the UI.
//!
//! Download: installers bundle the model (`scripts/download-embedding-model.cjs`
//! runs in `beforeBuildCommand`), so a shipped build normally downloads
//! nothing. When the bundle is missing or fails to load, the same pinned files
//! are fetched here: same revision, same SHA-256, same fp16 build, so the
//! vector space is identical to the installer's. It replaces fastembed's
//! hf-hub download, which had no stall timeout and no progress the app could
//! read: in the E2E it showed no progress for six minutes with nothing logged.

#[cfg(feature = "fastembed-local")]
use std::sync::LazyLock;

#[cfg(feature = "fastembed-local")]
use parking_lot::Mutex;
use serde::Serialize;

/// Where the in-process engine is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    not(feature = "fastembed-local"),
    allow(
        dead_code,
        reason = "a build without fastembed-local only reports Unavailable"
    )
)]
pub enum EngineState {
    /// Not started: it starts on first use or when onboarding prepares it.
    Idle,
    /// Loading, or downloading the model once (see the byte counts).
    Preparing,
    Ready,
    Failed,
    /// This build has no in-process engine (`fastembed-local` off).
    #[cfg_attr(
        feature = "fastembed-local",
        expect(
            dead_code,
            reason = "constructed only in builds without fastembed-local"
        )
    )]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddingEngineStatus {
    pub state: EngineState,
    /// A short reason when `Failed`; the step under way when `Preparing`.
    pub message: Option<String>,
    pub bytes_downloaded: u64,
    /// 0 when no download is under way or its size is unknown.
    pub bytes_total: u64,
}

impl EmbeddingEngineStatus {
    const fn of(state: EngineState) -> Self {
        Self {
            state,
            message: None,
            bytes_downloaded: 0,
            bytes_total: 0,
        }
    }
}

#[cfg(feature = "fastembed-local")]
static STATUS: LazyLock<Mutex<EmbeddingEngineStatus>> =
    LazyLock::new(|| Mutex::new(EmbeddingEngineStatus::of(EngineState::Idle)));

/// The engine's current state.
#[cfg(feature = "fastembed-local")]
pub fn engine_status() -> EmbeddingEngineStatus {
    STATUS.lock().clone()
}

/// The engine's current state: this build has none.
#[cfg(not(feature = "fastembed-local"))]
pub fn engine_status() -> EmbeddingEngineStatus {
    EmbeddingEngineStatus::of(EngineState::Unavailable)
}

#[cfg(feature = "fastembed-local")]
pub(crate) fn set_preparing(message: &str, bytes_downloaded: u64, bytes_total: u64) {
    *STATUS.lock() = EmbeddingEngineStatus {
        state: EngineState::Preparing,
        message: Some(message.to_string()),
        bytes_downloaded,
        bytes_total,
    };
}

#[cfg(feature = "fastembed-local")]
pub(crate) fn set_ready() {
    *STATUS.lock() = EmbeddingEngineStatus::of(EngineState::Ready);
}

#[cfg(feature = "fastembed-local")]
pub(crate) fn set_failed(reason: &str) {
    *STATUS.lock() = EmbeddingEngineStatus {
        state: EngineState::Failed,
        message: Some(reason.chars().take(300).collect()),
        bytes_downloaded: 0,
        bytes_total: 0,
    };
}

/// The in-process embedding engine's setup state, for the onboarding summary
/// and anywhere else that would otherwise assume it works.
#[tauri::command]
pub async fn get_embedding_engine_status() -> Result<EmbeddingEngineStatus, String> {
    Ok(engine_status())
}

// ============================================================================
// One-time download of the pinned model (mirrors download-embedding-model.cjs)
// ============================================================================

#[cfg(feature = "fastembed-local")]
pub(crate) mod download {
    use std::io::{Read as _, Write as _};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use sha2::{Digest, Sha256};

    use crate::error::FourDaError;

    const MODEL_REPO: &str = "nomic-ai/nomic-embed-text-v1.5";
    /// Pinned: must equal `REVISION` in `scripts/download-embedding-model.cjs`.
    pub(crate) const REVISION: &str = "e9b6763023c676ca8431644204f50c2b100d9aab";
    /// (remote path, local name, SHA-256). Must equal the script's `FILES`.
    pub(crate) const FILES: [(&str, &str, Option<&str>); 5] = [
        (
            "onnx/model_fp16.onnx",
            "model_fp16.onnx",
            Some("cf5b5a86edb00f895561803cfc04729090a958340b8ca2ad76c143f565f6bb04"),
        ),
        ("tokenizer.json", "tokenizer.json", None),
        ("config.json", "config.json", None),
        ("special_tokens_map.json", "special_tokens_map.json", None),
        ("tokenizer_config.json", "tokenizer_config.json", None),
    ];
    /// No byte for this long fails the read (the per-read timeout of a
    /// blocking reqwest client): a stalled connection fails and is retried
    /// instead of hanging the engine forever.
    const STALL_TIMEOUT: Duration = Duration::from_mins(1);
    const ATTEMPTS: u32 = 3;

    fn url(remote: &str) -> String {
        format!("https://huggingface.co/{MODEL_REPO}/resolve/{REVISION}/{remote}")
    }

    fn sha256_file(path: &Path) -> std::io::Result<String> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    /// A cached file is usable when present, non-empty, and (where pinned)
    /// matches its SHA-256.
    pub(crate) fn cached_ok(path: &Path, sha256: Option<&str>) -> bool {
        let present = std::fs::metadata(path).is_ok_and(|m| m.len() > 0);
        present && sha256.is_none_or(|want| sha256_file(path).is_ok_and(|got| got == want))
    }

    /// The model directory in the cache, downloading whatever is missing.
    /// `report(downloaded, total, label)` is called as bytes arrive.
    pub(crate) fn ensure_model(
        dir: &Path,
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
        for (remote, local, sha256) in FILES {
            let dest = dir.join(local);
            if cached_ok(&dest, sha256) {
                continue;
            }
            let mut last_err = String::new();
            let mut done = false;
            for attempt in 1..=ATTEMPTS {
                match fetch(&client, remote, &dest, sha256, &mut report) {
                    Ok(()) => {
                        done = true;
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(target: "4da::embeddings", file = local, attempt, error = %e, "Embedding model download attempt failed");
                        last_err = e;
                        std::thread::sleep(Duration::from_secs(2 * u64::from(attempt)));
                    }
                }
            }
            if !done {
                return Err(FourDaError::from(format!(
                    "embedding model download failed ({local}): {last_err}"
                )));
            }
        }
        Ok(dir.to_path_buf())
    }

    fn fetch(
        client: &reqwest::blocking::Client,
        remote: &str,
        dest: &Path,
        sha256: Option<&str>,
        report: &mut impl FnMut(u64, u64, &str),
    ) -> Result<(), String> {
        let mut response = client.get(url(remote)).send().map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let total = response.content_length().unwrap_or(0);
        let part = dest.with_extension("part");
        let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0u64;
        let mut last_report = 0u64;
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = response.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            hasher.update(&buf[..n]);
            downloaded += n as u64;
            if downloaded - last_report >= 1 << 20 {
                last_report = downloaded;
                report(downloaded, total, remote);
            }
        }
        file.flush().map_err(|e| e.to_string())?;
        drop(file);
        if total > 0 && downloaded != total {
            let _ = std::fs::remove_file(&part);
            return Err(format!("truncated: {downloaded} of {total} bytes"));
        }
        if let Some(want) = sha256 {
            let got = hex::encode(hasher.finalize());
            if got != want {
                let _ = std::fs::remove_file(&part);
                return Err(format!("SHA-256 mismatch (got {got})"));
            }
        }
        std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
        report(downloaded, total, remote);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test owns the process-global status (tests run in parallel), and
    /// restores it, so a test that initialises the real engine is unaffected.
    #[cfg(feature = "fastembed-local")]
    #[test]
    fn the_status_follows_the_engine_and_never_reports_ready_early() {
        let before = engine_status();
        set_preparing("Downloading the local search model", 5 << 20, 274 << 20);
        let s = engine_status();
        assert_eq!(s.state, EngineState::Preparing);
        assert_eq!(s.bytes_total, 274 << 20);
        set_failed("embedding model download failed (model_fp16.onnx): HTTP 503");
        let s = engine_status();
        assert_eq!(s.state, EngineState::Failed);
        assert!(s.message.unwrap().contains("HTTP 503"));
        set_ready();
        assert_eq!(engine_status().state, EngineState::Ready);
        let json = serde_json::to_value(engine_status()).unwrap();
        assert_eq!(
            json["state"], "ready",
            "the UI reads a lowercase state: {json}"
        );
        *STATUS.lock() = before;
    }

    /// The fallback download must fetch exactly what the installer bundles,
    /// or a user without the bundle embeds in a different space.
    #[cfg(feature = "fastembed-local")]
    #[test]
    fn the_download_is_pinned_to_the_installer_bundle() {
        let script = include_str!("../../../scripts/download-embedding-model.cjs");
        assert!(
            script.contains(&format!("const REVISION = '{}';", download::REVISION)),
            "revision drifted from scripts/download-embedding-model.cjs"
        );
        for (remote, local, sha) in download::FILES {
            assert!(script.contains(&format!("remote: '{remote}'")), "{remote}");
            assert!(script.contains(&format!("local: '{local}'")), "{local}");
            if let Some(sha) = sha {
                assert!(script.contains(sha), "{local} checksum drifted");
            }
        }
    }

    #[cfg(feature = "fastembed-local")]
    #[test]
    fn a_cached_file_must_be_whole_and_match_its_checksum() {
        let dir = std::env::temp_dir().join(format!("4da-embed-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("config.json");
        assert!(!download::cached_ok(&f, None), "missing");
        std::fs::write(&f, b"").unwrap();
        assert!(!download::cached_ok(&f, None), "empty");
        std::fs::write(&f, b"{}").unwrap();
        assert!(download::cached_ok(&f, None));
        // sha256("{}")
        let sha = "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
        assert!(download::cached_ok(&f, Some(sha)));
        assert!(!download::cached_ok(&f, Some("00")), "checksum mismatch");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
