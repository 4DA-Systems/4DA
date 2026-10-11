// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The in-process embedding engine's setup state, and the commands that report
//! it and start the one-time model download.
//!
//! Status: the onboarding summary said "Private semantic search active" with a
//! check mark while the model was still downloading (fresh-profile E2E
//! 2026-10-09). [`engine_status`] reports what is actually true, and
//! `get_embedding_engine_status` hands it to the UI.
//!
//! Download: release installers do not carry the model (AD-054 decision 7).
//! It is fetched once, on demand, by `model_store` (pinned revision, every
//! file SHA-256 and size pinned, resumable, with progress). Until then the
//! state is `not_downloaded`, which Settings shows with a download button.

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
    /// No copy of the model is on this machine. Nothing downloads it until a
    /// feature needs it (an interest, natural-language search) or the user
    /// asks from Settings; embeddings degrade until then.
    #[serde(rename = "not_downloaded")]
    NotDownloaded,
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

/// The engine's current state. An engine that has not started reports
/// `NotDownloaded` when no copy of the model is on disk.
#[cfg(feature = "fastembed-local")]
pub fn engine_status() -> EmbeddingEngineStatus {
    let raw = STATUS.lock().clone();
    presented(raw, super::model_store::is_installed)
}

/// `raw` as the UI should see it: `Idle` means "on disk, loads on first use"
/// only when a copy is installed (checked only for `Idle`).
#[cfg(feature = "fastembed-local")]
fn presented(
    raw: EmbeddingEngineStatus,
    installed: impl FnOnce() -> bool,
) -> EmbeddingEngineStatus {
    if raw.state == EngineState::Idle && !installed() {
        EmbeddingEngineStatus::of(EngineState::NotDownloaded)
    } else {
        raw
    }
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

/// The user asked for the local search model (Settings). Starts the one-time
/// download unless a copy is on disk or a download is running, and returns
/// the status to poll. An explicit request downloads even when Ollama is up.
#[cfg(feature = "fastembed-local")]
#[tauri::command]
pub async fn download_embedding_model() -> Result<EmbeddingEngineStatus, String> {
    if !super::model_store::start_download("settings") && super::model_store::is_installed() {
        // On disk already: load it so the status ends at Ready, not "on disk".
        tokio::task::spawn_blocking(super::fastembed::load_after_download)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(engine_status())
}

/// This build has no in-process engine, so there is nothing to download.
#[cfg(not(feature = "fastembed-local"))]
#[tauri::command]
pub async fn download_embedding_model() -> Result<EmbeddingEngineStatus, String> {
    Ok(engine_status())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test owns the process-global status (tests run in parallel), and
    /// restores it, so a test that initialises the real engine is unaffected.
    #[cfg(feature = "fastembed-local")]
    #[test]
    fn the_status_follows_the_engine_and_never_reports_ready_early() {
        let before = STATUS.lock().clone();
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

    /// Without a copy on disk an idle engine is "not downloaded"; a download
    /// in progress or a failure is reported as it is, never masked.
    #[cfg(feature = "fastembed-local")]
    #[test]
    fn an_idle_engine_without_a_model_reports_not_downloaded() {
        let idle = EmbeddingEngineStatus::of(EngineState::Idle);
        assert_eq!(
            presented(idle.clone(), || false).state,
            EngineState::NotDownloaded
        );
        assert_eq!(presented(idle, || true).state, EngineState::Idle);
        let preparing = EmbeddingEngineStatus {
            state: EngineState::Preparing,
            message: Some("Downloading the local search model".into()),
            bytes_downloaded: 1,
            bytes_total: 2,
        };
        assert_eq!(presented(preparing.clone(), || false), preparing);
        let json =
            serde_json::to_value(EmbeddingEngineStatus::of(EngineState::NotDownloaded)).unwrap();
        assert_eq!(json["state"], "not_downloaded");
    }
}
