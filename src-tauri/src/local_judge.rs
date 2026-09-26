// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Local judge routing: the judge lanes (ingest judge, pending-verdict
//! drain, rerank) run on the best MEASURED local judge that fits this
//! machine's GPU, whatever the user's main model is. Briefs, digests and
//! every other surface keep the configured model.
//!
//! Why: on 400 blind real items, one item per call and thinking off,
//! gemma4:26b scored 0.930 AUC, gemma4:12b 0.906 and qwen3:14b 0.893, all
//! above claude-haiku-4-5 (0.883), the cloud judge sibling
//! (`llm_judge::judge_provider`). A local judge is at least as accurate and
//! the judged text never leaves the machine.
//!
//! Three guards:
//! - **Fit.** A model is eligible only when its size plus
//!   [`VRAM_HEADROOM_MB`] fits in VRAM, or when it was measured to run well
//!   with partial offload on a card this size ([`MEASURED_OFFLOAD_FITS`]).
//!   Measured 2026-09-26 on a 16 GB card: gemma4:26b stalled (one call took
//!   7m15s) whenever 4DA's embedding model loaded on the GPU beside it.
//!   Embeddings now run on the CPU, and the stall is gone.
//! - **Detection is cached** ([`REFRESH_EVERY`]). If Ollama is down or no
//!   measured judge fits, judging stays on the cloud sibling as before.
//! - **Circuit breaker.** A local judge call that fails or runs longer than
//!   [`SLOW_CALL`] sends judging back to the cloud for [`COOL_OFF`]. Until
//!   then, local judge calls fail fast rather than each stalling.

use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tracing::{info, warn};

use crate::settings::LLMProvider;

pub(crate) const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const REFRESH_EVERY: Duration = Duration::from_mins(10);
/// A warm judge call takes 2-7 s. The model load is not part of it: [`warm`]
/// loads the judge before a lane's first timed call.
const SLOW_CALL: Duration = Duration::from_secs(90);
/// How long a judge model may take to load. Measured 2026-09-27: gemma4:26b
/// loaded in 46 s on an idle machine but 94 s during an engine cycle (from
/// NVMe). Timed inside the first judge call, that load tripped the breaker
/// and 47 of 48 rerank batches of the pass failed fast.
const LOAD_TIMEOUT: Duration = Duration::from_mins(5);
const COOL_OFF: Duration = Duration::from_mins(30);
/// Wall-clock budget for one rerank pass. The cloud keeps the 2-minute guard
/// against a hung provider. A local judge gets longer: each of its calls is
/// already bounded (the client's 120 s timeout, then the breaker fails the
/// rest fast), and a timed-out pass stores nothing. Measured 2026-09-27 on
/// the live app with gemma4:26b: 48 items took 78-84 s, but one pass ran at
/// ~5 s per call (12 calls a minute instead of 25-35) and timed out at 22 of 48.
const CLOUD_RERANK_BUDGET: Duration = Duration::from_mins(2);
const LOCAL_RERANK_BUDGET: Duration = Duration::from_mins(5);
/// VRAM kept free beside the judge: the desktop's own use (about 1.4 GB idle
/// on the measured machine), 4DA's embedding model, and the judge's 8k
/// context cache.
pub(crate) const VRAM_HEADROOM_MB: u64 = 4096;

/// Local models that have PASSED a real-item measurement as a feed judge,
/// despite sitting in the Basic capability tier (every unlisted Ollama model
/// defaults to Basic, `llm_capability::get_model_tier`). Deliberately a
/// judge-lane allowlist, not a tier promotion: a tier change would also turn
/// on reranking (batched), adversarial deliberation and LLM explanations for
/// the model, none of which were measured.
///
/// Each passed BOTH bars, one item per call, thinking off: >= fresh Haiku 4.5
/// (0.883) on 400 blind real items — gemma4:26b 0.930, gemma4:12b 0.906,
/// qwen3:14b 0.893 — and the `bench:judge` MCC floor through this code path
/// (0.75 / 0.65 / 0.60). qwen2.5:14b was removed: MCC 0.497-0.556 (mean 0.527,
/// 10-11 false demotions of 49). Not listed: qwen3.5:9b, qwen3.8:27b.
pub(crate) const MEASURED_LOCAL_JUDGES: &[&str] = &["gemma4:26b", "gemma4:12b", "qwen3:14b"];

/// Judges measured to run well on a card SMALLER than size plus headroom, and
/// the smallest VRAM they were measured on. gemma4:26b is a mixture-of-experts
/// model: Ollama offloads part of it to the CPU and it still judges quickly.
/// Measured 2026-09-26 on a 16,376 MiB card (production request shape, one
/// item per call): 3.97 s median per item alone, 4.26 s with 4DA's embedding
/// model running beside it on the CPU. With the embedding model on the GPU the
/// median was 101.5 s, because Ollama evicted and reloaded the judge around
/// every embed call, so this entry relies on the CPU-pinned embeddings in
/// `embeddings_providers::ollama`.
const MEASURED_OFFLOAD_FITS: &[(&str, u64)] = &[("gemma4:26b", 16_000)];

/// The purposes (`LLMClient::with_purpose`) of the three judge lanes.
const JUDGE_PURPOSES: &[&str] = &["ingest_judge", "verdict_drain", "rerank_judge"];

#[derive(Default)]
struct State {
    checked_at: Option<Instant>,
    model: Option<String>,
    base_url: String,
    tripped_until: Option<Instant>,
}

static STATE: Lazy<Mutex<State>> = Lazy::new(|| Mutex::new(State::default()));

/// An installed Ollama model: name and on-disk size.
#[derive(Debug, Clone)]
pub(crate) struct Installed {
    pub name: String,
    pub size_mb: u64,
}

/// The best measured judge that fits the memory budget, in measured order.
/// `budget_mb` is `None` when the GPU size is unknown, and then nothing is
/// chosen: routing a judge onto a GPU of unknown size could stall every
/// judge lane, and the cloud sibling works today.
pub(crate) fn pick_judge(installed: &[Installed], budget_mb: Option<u64>) -> Option<String> {
    let budget = budget_mb?;
    MEASURED_LOCAL_JUDGES
        .iter()
        .find_map(|measured| {
            installed
                .iter()
                .find(|i| i.name.starts_with(measured) && fits(measured, i.size_mb, budget))
        })
        .map(|i| i.name.clone())
}

fn fits(measured: &str, size_mb: u64, budget_mb: u64) -> bool {
    size_mb + VRAM_HEADROOM_MB <= budget_mb
        || MEASURED_OFFLOAD_FITS
            .iter()
            .any(|(name, min_mb)| *name == measured && budget_mb >= *min_mb)
}

/// This machine's judge memory budget: dedicated VRAM when the GPU reports
/// it; on Apple Silicon (unified memory, no VRAM figure) half the RAM.
fn memory_budget_mb() -> Option<u64> {
    let hw = crate::hardware_detect::detect_hardware();
    match hw.gpu {
        Some(gpu) if gpu.vram_mb.is_some() => gpu.vram_mb,
        Some(gpu) if gpu.vendor.eq_ignore_ascii_case("apple") => {
            Some((hw.ram_total_gb * 1024.0 / 2.0) as u64)
        }
        _ => None,
    }
}

async fn fetch_installed(base_url: &str) -> Option<Vec<Installed>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let body: serde_json::Value = client
        .get(format!("{base_url}/api/tags"))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    Some(
        body["models"]
            .as_array()?
            .iter()
            .filter_map(|m| {
                Some(Installed {
                    name: m["name"].as_str()?.to_string(),
                    size_mb: m["size"].as_u64().unwrap_or(u64::MAX / 2) / (1024 * 1024),
                })
            })
            .collect(),
    )
}

/// Called at the top of each judge lane: re-detect the local judge when the
/// cached answer is older than [`REFRESH_EVERY`], then make sure it is loaded.
/// Cheap when fresh and warm.
pub(crate) async fn refresh_if_stale() {
    redetect_if_stale().await;
    warm().await;
}

/// Load the routed judge before the lane's first timed call, so a cold load
/// never counts as a slow judge call. Ollama answers an empty-prompt
/// `/api/generate` once the model is in memory, at once if it already is.
/// It must ask for the judge calls' context size: measured on Ollama 0.34.4,
/// a load at the default 4,096 made the next 8,192 judge call reload for 36 s.
/// A load that fails or outlasts [`LOAD_TIMEOUT`] opens the breaker.
async fn warm() {
    let Some((model, base_url)) = route(&STATE.lock(), Instant::now()) else {
        return;
    };
    let started = Instant::now();
    let loaded = load_model(&base_url, &model).await;
    let elapsed = started.elapsed();
    if !loaded {
        STATE.lock().tripped_until = Some(Instant::now() + COOL_OFF);
        warn!(
            target: "4da::local_judge",
            model,
            elapsed_s = elapsed.as_secs(),
            cool_off_min = COOL_OFF.as_secs() / 60,
            "Local judge failed to load; judging returns to the cloud judge for the cool-off"
        );
    } else if elapsed > Duration::from_secs(5) {
        info!(target: "4da::local_judge", model, elapsed_s = elapsed.as_secs(), "Local judge loaded");
    }
}

async fn load_model(base_url: &str, model: &str) -> bool {
    let Ok(client) = reqwest::Client::builder().timeout(LOAD_TIMEOUT).build() else {
        return false;
    };
    client
        .post(format!("{base_url}/api/generate"))
        .json(&serde_json::json!({
            "model": model,
            "options": { "num_ctx": crate::llm::OLLAMA_NUM_CTX },
        }))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .is_ok()
}

async fn redetect_if_stale() {
    let base_url = {
        let mut s = STATE.lock();
        if s.checked_at.is_some_and(|t| t.elapsed() < REFRESH_EVERY) {
            return;
        }
        // Claim the refresh so concurrent lanes do not all probe at once.
        s.checked_at = Some(Instant::now());
        let settings = crate::get_settings_manager().lock();
        let llm = &settings.get().llm;
        if llm.provider == "ollama" {
            llm.base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string())
        } else {
            DEFAULT_OLLAMA_URL.to_string()
        }
    };
    let installed = fetch_installed(&base_url).await;
    let model = installed
        .as_deref()
        .and_then(|list| pick_judge(list, memory_budget_mb()));
    let mut s = STATE.lock();
    if s.model != model {
        info!(
            target: "4da::local_judge",
            judge = model.as_deref().unwrap_or("(cloud sibling)"),
            ollama_reachable = installed.is_some(),
            "Judge lanes routed"
        );
    }
    s.model = model;
    s.base_url = base_url;
}

fn route(state: &State, now: Instant) -> Option<(String, String)> {
    if state.tripped_until.is_some_and(|t| now < t) {
        return None;
    }
    state.model.clone().map(|m| (m, state.base_url.clone()))
}

/// The local judge provider to use now, derived from `base` (so embedding and
/// key fields carry over), or `None` for the cloud sibling.
pub(crate) fn local_provider(base: &LLMProvider) -> Option<LLMProvider> {
    let (model, base_url) = route(&STATE.lock(), Instant::now())?;
    let mut p = base.clone();
    p.provider = "ollama".to_string();
    p.api_key = String::new();
    p.model = model;
    p.base_url = Some(base_url);
    Some(p)
}

/// The daily cap guards SPEND: a local (Ollama) judge costs nothing and is
/// never blocked by it (`LLMClient::metered`). No provider: the cap decides.
pub(crate) fn budget_blocks(provider: Option<&LLMProvider>) -> bool {
    provider.is_none_or(|p| p.provider != "ollama") && crate::state::is_llm_limit_reached()
}

pub(crate) fn is_judge_purpose(purpose: Option<&str>) -> bool {
    purpose.is_some_and(|p| JUDGE_PURPOSES.contains(&p))
}

/// The rerank pass's wall-clock budget: longer when a local judge will run it.
pub(crate) fn rerank_budget() -> Duration {
    budget_for(route(&STATE.lock(), Instant::now()).is_some())
}

fn budget_for(local: bool) -> Duration {
    if local {
        LOCAL_RERANK_BUDGET
    } else {
        CLOUD_RERANK_BUDGET
    }
}

/// Whether the breaker is open: local judge calls should fail fast.
pub(crate) fn cooling_off() -> bool {
    STATE
        .lock()
        .tripped_until
        .is_some_and(|t| Instant::now() < t)
}

fn note(state: &mut State, elapsed: Duration, ok: bool, now: Instant) -> bool {
    if ok && elapsed <= SLOW_CALL {
        return false;
    }
    state.tripped_until = Some(now + COOL_OFF);
    true
}

/// Record one local judge call. A failure or a call slower than
/// [`SLOW_CALL`] opens the breaker for [`COOL_OFF`].
pub(crate) fn record_call(model: &str, elapsed: Duration, ok: bool) {
    if note(&mut STATE.lock(), elapsed, ok, Instant::now()) {
        warn!(
            target: "4da::local_judge",
            model,
            elapsed_s = elapsed.as_secs(),
            ok,
            cool_off_min = COOL_OFF.as_secs() / 60,
            "Local judge call failed or was too slow; judging returns to the cloud judge for the cool-off"
        );
    }
}

#[cfg(test)]
#[path = "local_judge_tests.rs"]
mod tests;
