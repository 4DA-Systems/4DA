// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Cross-encoder reranker via fastembed ONNX — fast local precision reranking.
//!
//! Runs after PASIFA scoring to reorder the top candidates using bidirectional
//! attention (query x document jointly), which is substantially more precise than
//! bi-encoder cosine similarity for ranking.

use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

use crate::scoring;

#[cfg(feature = "fastembed-local")]
use crate::error::FourDaError;

#[cfg(feature = "fastembed-local")]
use once_cell::sync::OnceCell;
#[cfg(feature = "fastembed-local")]
use parking_lot::Mutex;

#[cfg(feature = "fastembed-local")]
static RERANKER_MODEL: OnceCell<Mutex<fastembed::TextRerank>> = OnceCell::new();

static RERANKER_AVAILABLE: AtomicBool = AtomicBool::new(false);
static RERANKER_INIT_ATTEMPTED: AtomicBool = AtomicBool::new(false);

/// Maximum number of candidates to rerank (keeps latency under 200ms on 4-core CPU).
const MAX_RERANK_CANDIDATES: usize = 50;

/// Batch size for cross-encoder inference. CRITICAL: passing `None` to
/// `TextRerank::rerank` runs all candidates through the ONNX model in a single
/// batch, which materializes one giant activation tensor — measured at ~1.85 GB
/// for 50 candidates, the native-OOM abort that killed background analysis
/// ~2.5 min after launch (Victauri findings #3). Batching keeps peak activation
/// memory bounded (a few hundred MB) at negligible latency cost.
#[cfg(feature = "fastembed-local")]
const RERANK_BATCH_SIZE: usize = 8;

/// A candidate for cross-encoder reranking.
#[derive(Debug, Clone)]
pub(crate) struct RerankCandidate {
    pub id: u64,
    pub title: String,
    pub explanation: String,
    pub original_score: f32,
}

/// Result after cross-encoder reranking.
#[derive(Debug, Clone)]
pub(crate) struct RerankResult {
    pub id: u64,
    /// Raw cross-encoder score (diagnostic — logged in debug traces).
    #[allow(dead_code)] // REMOVE BY 2026-08-01 — wire into debug trace UI or drop
    pub cross_encoder_score: f32,
    /// Original PASIFA score before blending (diagnostic — logged in debug traces).
    #[allow(dead_code)] // REMOVE BY 2026-08-01 — wire into debug trace UI or drop
    pub original_score: f32,
    pub blended_score: f32,
}

/// Build a concise query string from the scoring context for cross-encoder reranking.
///
/// Cross-encoders work best with a focused query. We synthesize one from the user's
/// declared tech stack, interests, and current work topics — the same signals PASIFA
/// uses, but distilled into a single passage for bidirectional attention.
fn build_rerank_query(ctx: &scoring::ScoringContext) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Declared tech stack (highest signal)
    if !ctx.declared_tech.is_empty() {
        parts.push(ctx.declared_tech.join(", "));
    } else if !ctx.ace_ctx.detected_tech.is_empty() {
        let top: Vec<&str> = ctx
            .ace_ctx
            .detected_tech
            .iter()
            .take(6)
            .map(String::as_str)
            .collect();
        parts.push(top.join(", "));
    }

    // User interests
    if !ctx.interests.is_empty() {
        let names: Vec<&str> = ctx
            .interests
            .iter()
            .take(8)
            .map(|i| i.topic.as_str())
            .collect();
        parts.push(names.join(", "));
    }

    // Current work focus (recent git activity)
    if !ctx.work_topics.is_empty() {
        parts.push(ctx.work_topics.join(", "));
    }

    // Cap at 512 chars — cross-encoders have limited context windows and
    // a focused query produces better relevance judgments than a sprawling one.
    let joined = parts.join(". ");
    if joined.len() > 512 {
        joined.chars().take(512).collect()
    } else {
        joined
    }
}

/// Initialize the cross-encoder reranker (lazy, first call only).
/// Returns true if the reranker is available, false if init failed.
/// Minimum free system RAM (MB) required to load the cross-encoder model.
/// The bge-reranker-base ONNX model + Runtime arena need ~1.5-2 GB resident
/// (measured); attempting that load on a memory-constrained machine was the
/// native OOM abort that killed background analysis ~2.5 min after launch
/// (Victauri findings #3). Below this threshold we skip the reranker entirely
/// and fall back to PASIFA-only ranking — the precision boost is not worth
/// crashing the app. Re-checked until the model loads, so it self-enables once
/// memory frees up.
#[cfg(feature = "fastembed-local")]
const RERANKER_MIN_AVAILABLE_MB: u64 = 2560;

#[cfg(feature = "fastembed-local")]
fn available_memory_mb() -> u64 {
    use sysinfo::{MemoryRefreshKind, RefreshKind};
    let mut sys = sysinfo::System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    sys.available_memory() / (1024 * 1024)
}

#[cfg(feature = "fastembed-local")]
fn ensure_reranker() -> bool {
    if RERANKER_AVAILABLE.load(Ordering::Relaxed) {
        return true;
    }
    // RAM gate (checked before the heavy load, re-checked until it succeeds).
    let available_mb = available_memory_mb();
    if available_mb < RERANKER_MIN_AVAILABLE_MB {
        warn!(
            target: "4da::reranker",
            available_mb,
            threshold_mb = RERANKER_MIN_AVAILABLE_MB,
            "Insufficient free RAM for cross-encoder reranker — using PASIFA-only ranking (findings #3 OOM guard)"
        );
        return false;
    }
    if RERANKER_INIT_ATTEMPTED.swap(true, Ordering::SeqCst) {
        // Another thread already attempted init — return whatever it decided.
        return RERANKER_AVAILABLE.load(Ordering::Relaxed);
    }

    let result = RERANKER_MODEL.get_or_try_init(|| {
        let cache_dir = crate::runtime_paths::RuntimePaths::get().model_cache_dir();
        info!(
            target: "4da::reranker",
            cache = %cache_dir.display(),
            "Initializing cross-encoder reranker (bge-reranker-base, ~85MB first download)"
        );

        let options = fastembed::RerankInitOptions::new(fastembed::RerankerModel::BGERerankerBase)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(true);

        fastembed::TextRerank::try_new(options)
            .map(Mutex::new)
            .map_err(|e| {
                warn!(
                    target: "4da::reranker",
                    error = %e,
                    "Cross-encoder reranker init failed — falling back to PASIFA-only ranking"
                );
                FourDaError::from(format!("reranker init: {e}"))
            })
    });

    let available = result.is_ok();
    RERANKER_AVAILABLE.store(available, Ordering::Relaxed);
    available
}

#[cfg(not(feature = "fastembed-local"))]
fn ensure_reranker() -> bool {
    false
}

/// Rerank candidates using the cross-encoder model.
///
/// Takes the top candidates (by PASIFA score) and reranks them using bidirectional
/// attention for higher precision. Returns blended scores that combine the cross-encoder
/// judgment with the original PASIFA score.
///
/// If the reranker is unavailable, returns candidates with original scores unchanged.
pub(crate) fn rerank_candidates(query: &str, candidates: &[RerankCandidate]) -> Vec<RerankResult> {
    if candidates.is_empty() || query.trim().is_empty() {
        return passthrough(candidates);
    }

    #[cfg(feature = "fastembed-local")]
    {
        if !ensure_reranker() {
            return passthrough(candidates);
        }

        let reranker = match RERANKER_MODEL.get() {
            Some(m) => m,
            None => return passthrough(candidates),
        };

        let capped = candidates.len().min(MAX_RERANK_CANDIDATES);
        let documents: Vec<String> = candidates[..capped]
            .iter()
            .map(|c| {
                if c.explanation.is_empty() {
                    c.title.clone()
                } else {
                    format!("{} — {}", c.title, c.explanation)
                }
            })
            .collect();

        let doc_refs: Vec<&str> = documents.iter().map(String::as_str).collect();

        let rerank_result = {
            let mut model = reranker.lock();
            // Some(batch_size) — NOT None. None batches all candidates at once,
            // spiking activation memory to ~1.85 GB → native OOM (findings #3).
            model.rerank(query, doc_refs.as_slice(), false, Some(RERANK_BATCH_SIZE))
        };

        match rerank_result {
            Ok(results) => {
                let mut output: Vec<RerankResult> = Vec::with_capacity(candidates.len());

                for result in &results {
                    let idx = result.index;
                    if idx < capped {
                        let candidate = &candidates[idx];
                        let ce_score = result.score.clamp(0.0, 1.0);
                        // 0.4 x PASIFA + 0.6 x cross-encoder — cross-encoder gets more
                        // weight because bidirectional attention is more precise for ranking
                        let blended = candidate.original_score * 0.4 + ce_score * 0.6;

                        output.push(RerankResult {
                            id: candidate.id,
                            cross_encoder_score: ce_score,
                            original_score: candidate.original_score,
                            blended_score: blended,
                        });
                    }
                }

                // Append any candidates beyond the rerank cap with original scores
                for candidate in candidates.iter().skip(capped) {
                    output.push(RerankResult {
                        id: candidate.id,
                        cross_encoder_score: candidate.original_score,
                        original_score: candidate.original_score,
                        blended_score: candidate.original_score,
                    });
                }

                output.sort_by(|a, b| {
                    b.blended_score
                        .partial_cmp(&a.blended_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                debug!(
                    target: "4da::reranker",
                    candidates = capped,
                    "Cross-encoder reranking complete"
                );

                output
            }
            Err(e) => {
                warn!(
                    target: "4da::reranker",
                    error = %e,
                    "Cross-encoder rerank failed — using original scores"
                );
                passthrough(candidates)
            }
        }
    }

    #[cfg(not(feature = "fastembed-local"))]
    {
        passthrough(candidates)
    }
}

fn passthrough(candidates: &[RerankCandidate]) -> Vec<RerankResult> {
    candidates
        .iter()
        .map(|c| RerankResult {
            id: c.id,
            cross_encoder_score: c.original_score,
            original_score: c.original_score,
            blended_score: c.original_score,
        })
        .collect()
}

/// Build rerank query and run cross-encoder reranking on scored results in-place.
///
/// This is the integration entry point called from the analysis pipeline. It:
/// 1. Synthesizes a query from the scoring context (tech stack + interests + work topics)
/// 2. Builds candidates from the current results
/// 3. Runs cross-encoder reranking
/// 4. Updates `top_score` on each result with the blended score
/// 5. Re-sorts by the new scores
///
/// No-op when: results have <= 1 item, reranker unavailable, or query is empty.
pub(crate) fn apply_cross_encoder_reranking(
    results: &mut Vec<crate::SourceRelevance>,
    scoring_ctx: &scoring::ScoringContext,
) {
    if results.len() <= 1 {
        return;
    }

    let query = build_rerank_query(scoring_ctx);
    if query.is_empty() {
        debug!(
            target: "4da::reranker",
            "No context available for cross-encoder query — skipping reranking"
        );
        return;
    }

    let candidates: Vec<RerankCandidate> = results
        .iter()
        .map(|r| RerankCandidate {
            id: r.id,
            title: r.title.clone(),
            explanation: r.explanation.clone().unwrap_or_default(),
            original_score: r.top_score,
        })
        .collect();

    let reranked = rerank_candidates(&query, &candidates);

    // Apply blended scores back to results
    for rr in &reranked {
        if let Some(item) = results.iter_mut().find(|r| r.id == rr.id) {
            item.top_score = rr.blended_score;
        }
    }

    // Re-sort by updated scores
    results.sort_by(|a, b| {
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    debug!(
        target: "4da::reranker",
        items = results.len(),
        query_len = query.len(),
        "Cross-encoder reranking applied to analysis results"
    );
}

/// Check if the cross-encoder reranker is available (without initializing it).
#[allow(dead_code)] // REMOVE BY 2026-08-01 — expose in settings UI or drop
pub(crate) fn is_reranker_available() -> bool {
    RERANKER_AVAILABLE.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_candidates_returns_empty() {
        let results = rerank_candidates("test query", &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn empty_query_returns_passthrough() {
        let candidates = vec![RerankCandidate {
            id: 1,
            title: "Test".into(),
            explanation: "content".into(),
            original_score: 0.5,
        }];
        let results = rerank_candidates("", &candidates);
        assert_eq!(results.len(), 1);
        assert!((results[0].blended_score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn passthrough_preserves_scores() {
        let candidates = vec![
            RerankCandidate {
                id: 1,
                title: "A".into(),
                explanation: String::new(),
                original_score: 0.8,
            },
            RerankCandidate {
                id: 2,
                title: "B".into(),
                explanation: String::new(),
                original_score: 0.5,
            },
        ];
        let results = passthrough(&candidates);
        assert!((results[0].blended_score - 0.8).abs() < f32::EPSILON);
        assert!((results[1].blended_score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn whitespace_only_query_returns_passthrough() {
        let candidates = vec![RerankCandidate {
            id: 1,
            title: "Test".into(),
            explanation: String::new(),
            original_score: 0.7,
        }];
        let results = rerank_candidates("   ", &candidates);
        assert_eq!(results.len(), 1);
        assert!((results[0].blended_score - 0.7).abs() < f32::EPSILON);
    }
}
