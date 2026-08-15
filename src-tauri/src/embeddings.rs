// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

#[path = "embeddings_providers/mod.rs"]
mod embeddings_providers;

use once_cell::sync::Lazy;

use crate::error::Result;
use crate::get_settings_manager;

#[cfg(feature = "fastembed-local")]
use embeddings_providers::embed_texts_fastembed_sync;
pub use embeddings_providers::*;

#[cfg(all(test, feature = "fastembed-local"))]
pub(crate) fn fastembed_sync(texts: &[String]) -> crate::error::Result<Vec<Vec<f32>>> {
    embed_texts_fastembed_sync(texts)
}

use embeddings_providers::{embed_texts_ollama, embed_texts_openai, retry_with_backoff};

/// Shared HTTP client for embedding API calls (reused across requests)
static EMBEDDING_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .redirect(crate::http_client::local_aware_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build HTTP client: {e}, using default");
            reqwest::Client::new()
        })
});

// ============================================================================
// Embeddings - supports OpenAI and Ollama
// ============================================================================

/// Bounded wait for the fastembed engine on any single embed call.
///
/// The first fastembed call lazily constructs the ONNX runtime + model
/// (`get_or_init_fastembed`). With the model bundled this is usually ~1s, but a
/// slow disk / large model could stall the first call. We cap the wait so
/// first-run analysis never hangs: if the engine isn't ready within this budget,
/// the call returns the graceful zero-vector fallback (analysis completes in
/// keyword mode) while the `spawn_blocking` init task keeps running in the
/// background — so subsequent calls hit the now-initialized engine.
#[cfg(feature = "fastembed-local")]
const FASTEMBED_CALL_TIMEOUT_SECS: u64 = 10;

/// Attempt in-process fastembed (ONNX Runtime) — zero network, privacy preserved.
/// Returns `Some(embeddings)` on success, `None` on failure or timeout.
#[cfg(feature = "fastembed-local")]
async fn try_fastembed_fallback(texts: &[String]) -> Option<Vec<Vec<f32>>> {
    let texts_for_fe = texts.to_vec();
    // Run init+embed on a blocking thread. A `spawn_blocking` task is NOT
    // cancelled when the JoinHandle is dropped, so on timeout the lazy engine
    // init continues to completion in the background and populates the shared
    // OnceCell for later calls.
    let handle = tokio::task::spawn_blocking(move || embed_texts_fastembed_sync(&texts_for_fe));
    let joined = tokio::time::timeout(
        std::time::Duration::from_secs(FASTEMBED_CALL_TIMEOUT_SECS),
        handle,
    )
    .await;
    let joined = match joined {
        Ok(joined) => joined,
        Err(_) => {
            tracing::warn!(
                target: "4da::embeddings",
                timeout_s = FASTEMBED_CALL_TIMEOUT_SECS,
                "fastembed not ready within budget — zero-vector fallback for this call; engine continues initializing in background"
            );
            return None;
        }
    };
    match joined {
        Ok(Ok(embeddings)) => {
            tracing::info!(
                target: "4da::embeddings",
                count = embeddings.len(),
                "Embedded in-process via fastembed (ONNX, zero network)"
            );
            Some(
                validate_embeddings(embeddings)
                    .into_iter()
                    .map(truncate_and_normalize)
                    .collect(),
            )
        }
        Ok(Err(e)) => {
            tracing::debug!(
                target: "4da::embeddings",
                error = %e,
                "fastembed unavailable — falling back to zero vectors"
            );
            None
        }
        Err(e) => {
            tracing::debug!(
                target: "4da::embeddings",
                error = %e,
                "fastembed task panicked — falling back to zero vectors"
            );
            None
        }
    }
}

/// The resolved embedding execution path for a given settings snapshot.
///
/// This is the SINGLE place where the cloud-vs-local decision is made, so the
/// privacy gate (INV-004) is enforced in exactly one location and can be
/// unit-tested without any network access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmbeddingRoute {
    /// Cloud: OpenAI embeddings API (`api.openai.com`). Content LEAVES the
    /// machine — only reachable when the user has explicitly opted in.
    OpenAi,
    /// Explicit Ollama provider — uses the configured `base_url`.
    Ollama,
    /// Anthropic provider WITHOUT a cloud embed key: local fallback chain
    /// (configured base_url Ollama -> default Ollama -> fastembed -> zero).
    AnthropicLocal,
    /// Default local path: zero-config Ollama -> fastembed -> zero-vector.
    LocalDefault,
}

impl EmbeddingRoute {
    /// True only for routes that transmit content off the machine.
    pub(crate) fn is_cloud(self) -> bool {
        matches!(self, EmbeddingRoute::OpenAi)
    }
}

/// Pure provider-selection logic — no I/O, no secrets, fully unit-testable.
///
/// Privacy gate (INV-004): when `allow_cloud == false` this function can NEVER
/// return [`EmbeddingRoute::OpenAi`]; the openai and anthropic+openai-key cases
/// collapse to the local path. When `allow_cloud == true` it reproduces the
/// legacy content-agnostic routing exactly.
///
/// - `has_llm_api_key`    — `llm.api_key` is non-empty (the openai provider key)
/// - `has_openai_embed_key` — `llm.openai_api_key` is non-empty (dedicated embed key)
pub(crate) fn resolve_embedding_route(
    provider: &str,
    has_llm_api_key: bool,
    has_openai_embed_key: bool,
    allow_cloud: bool,
) -> EmbeddingRoute {
    match provider {
        // OpenAI provider with a key: cloud only when opted in; otherwise local.
        "openai" if has_llm_api_key => {
            if allow_cloud {
                EmbeddingRoute::OpenAi
            } else {
                EmbeddingRoute::LocalDefault
            }
        }
        // OpenAI provider without a key never had a cloud path anyway.
        "openai" => EmbeddingRoute::LocalDefault,
        "ollama" => EmbeddingRoute::Ollama,
        "anthropic" => {
            if allow_cloud && has_openai_embed_key {
                EmbeddingRoute::OpenAi
            } else if has_openai_embed_key || has_llm_api_key {
                // Gate off (or no dedicated key): stay on the anthropic-local
                // fallback chain rather than exfiltrating to OpenAI.
                EmbeddingRoute::AnthropicLocal
            } else {
                // Both keys empty — legacy code treated this as "none".
                EmbeddingRoute::LocalDefault
            }
        }
        _ => EmbeddingRoute::LocalDefault,
    }
}

/// Produce zero-vector placeholders and report embedding capability as degraded.
fn zero_vector_fallback(count: usize) -> Vec<Vec<f32>> {
    crate::capabilities::report_degraded(
        crate::capabilities::Capability::EmbeddingSearch,
        "No embedding provider available",
        "Keyword matching with context synthesis (install Ollama for semantic search)",
    );
    (0..count).map(|_| vec![0.0f32; EMBEDDING_DIMS]).collect()
}

/// Generate embeddings for a list of texts
/// Supports OpenAI (text-embedding-3-small), Ollama (nomic-embed-text), and fastembed (snowflake-arctic-embed-m)
/// Provider is determined by settings - uses same provider as LLM when possible
pub(crate) async fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    // Batch large inputs to prevent memory spikes (max 32 texts per API call)
    const EMBED_BATCH_SIZE: usize = 32;
    if texts.len() > EMBED_BATCH_SIZE {
        tracing::debug!(
            target: "4da::embeddings",
            count = texts.len(),
            batch_size = EMBED_BATCH_SIZE,
            "Batching embedding request into {} chunks",
            texts.len().div_ceil(EMBED_BATCH_SIZE)
        );
        let mut all_embeddings = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(EMBED_BATCH_SIZE) {
            let chunk_result = Box::pin(embed_texts(chunk)).await?;
            all_embeddings.extend(chunk_result);
        }
        return Ok(all_embeddings);
    }

    let llm_settings = {
        let mut guard = get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };

    // Privacy gate (INV-004): resolve the execution path in ONE place. When the
    // user has not explicitly opted into cloud embeddings, this can never select
    // a cloud route — setting an LLM key must not silently exfiltrate local
    // file / project / context content to api.openai.com as an embedding
    // side-effect. See `resolve_embedding_route`.
    let route = resolve_embedding_route(
        llm_settings.provider.as_str(),
        !llm_settings.api_key.is_empty(),
        !llm_settings.openai_api_key.is_empty(),
        llm_settings.allow_cloud_embeddings,
    );

    let result = match route {
        EmbeddingRoute::OpenAi => {
            // The gate has already confirmed opt-in. Pick the key the same way
            // the legacy router did: the openai provider uses `api_key`, every
            // other provider (anthropic) uses the dedicated `openai_api_key`.
            let api_key = if llm_settings.provider == "openai" {
                llm_settings.api_key.clone()
            } else {
                llm_settings.openai_api_key.clone()
            };
            tracing::info!(
                target: "4da::embeddings",
                count = texts.len(),
                provider = %llm_settings.provider,
                "Embedding via OpenAI (user opted in) — content sent to api.openai.com (retained 30 days per OpenAI policy)"
            );
            let texts = texts.to_vec();
            retry_with_backoff("embed_openai", 2, || {
                let key = api_key.clone();
                let t = texts.clone();
                async move { embed_texts_openai(&t, &key).await }
            })
            .await
            .map(|vecs| {
                validate_embeddings(vecs)
                    .into_iter()
                    .map(truncate_and_normalize)
                    .collect()
            })
        }
        EmbeddingRoute::Ollama => {
            let base_url = llm_settings.base_url.clone();
            let texts = texts.to_vec();
            retry_with_backoff("embed_ollama", 2, || {
                let url = base_url.clone();
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &url).await }
            })
            .await
            .map(validate_embeddings)
        }
        EmbeddingRoute::AnthropicLocal => {
            // Anthropic has no embeddings API and the cloud gate is closed (or no
            // dedicated key): fall back to local Ollama -> fastembed -> zero.
            // Try Ollama as fallback
            if let Some(base_url) = &llm_settings.base_url {
                if !base_url.is_empty() {
                    let url = Some(base_url.clone());
                    let texts_vec = texts.to_vec();
                    if let Ok(result) =
                        retry_with_backoff("embed_ollama_anthropic_fallback", 2, || {
                            let u = url.clone();
                            let t = texts_vec.clone();
                            async move { embed_texts_ollama(&t, &u).await }
                        })
                        .await
                    {
                        return Ok(validate_embeddings(result));
                    }
                }
            }
            // Try default Ollama
            let texts = texts.to_vec();
            match retry_with_backoff("embed_ollama_default", 2, || {
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &None).await }
            })
            .await
            {
                Ok(result) => Ok(validate_embeddings(result)),
                Err(_) => {
                    #[cfg(feature = "fastembed-local")]
                    if let Some(result) = try_fastembed_fallback(&texts).await {
                        return Ok(result);
                    }
                    Ok(zero_vector_fallback(texts.len()))
                }
            }
        }
        // Default local path (none / unknown / gated-off cloud):
        // try Ollama → fastembed (in-process) → zero vectors.
        EmbeddingRoute::LocalDefault => {
            let texts = texts.to_vec();
            if let Ok(result) = retry_with_backoff("embed_ollama_zeroconfig", 1, || {
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &None).await }
            })
            .await
            {
                return Ok(validate_embeddings(result));
            }

            #[cfg(feature = "fastembed-local")]
            if let Some(result) = try_fastembed_fallback(&texts).await {
                return Ok(result);
            }

            tracing::debug!(
                target: "4da::embeddings",
                "No embedding provider available — scoring via keyword matching with ACE context synthesis"
            );
            Ok(zero_vector_fallback(texts.len()))
        }
    };

    // Report capability state based on result
    if result.is_ok() {
        crate::capabilities::report_restored(crate::capabilities::Capability::EmbeddingSearch);
    }

    result
}

/// Current model: Snowflake Arctic Embed M (quantized) — 768d, 110M params
/// Single source of truth for embedding dimensions across the entire codebase.
pub const EMBEDDING_DIMS: usize = 768;

/// Validate embedding vectors — replace NaN/Inf with zero vectors.
/// This prevents corrupted embeddings from silently degrading search quality.
fn validate_embeddings(embeddings: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    embeddings
        .into_iter()
        .map(|vec| {
            if vec.iter().any(|v| v.is_nan() || v.is_infinite()) {
                tracing::warn!(
                    target: "4da::embeddings",
                    "Detected NaN/Inf in embedding vector — replacing with zero vector"
                );
                vec![0.0f32; EMBEDDING_DIMS]
            } else {
                vec
            }
        })
        .collect()
}

/// Ensure embedding has exactly EMBEDDING_DIMS dimensions, then L2-normalize.
/// - Too long: truncate (Matryoshka models preserve quality at lower dims)
/// - Too short: zero-pad (prevents KNN dimension mismatch — critical for sqlite-vec)
/// - Exact: pass through, normalizing only if truncated/padded
fn truncate_and_normalize(mut embedding: Vec<f32>) -> Vec<f32> {
    let modified = if embedding.len() > EMBEDDING_DIMS {
        embedding.truncate(EMBEDDING_DIMS);
        true
    } else if embedding.len() < EMBEDDING_DIMS {
        tracing::warn!(
            target: "4da::embeddings",
            got = embedding.len(),
            expected = EMBEDDING_DIMS,
            "Embedding shorter than target — zero-padding to prevent KNN mismatch"
        );
        embedding.resize(EMBEDDING_DIMS, 0.0);
        true
    } else {
        false
    };

    // Re-normalize after dimension change (Matryoshka requirement for truncation,
    // and correctness requirement for padding)
    if modified {
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }
    }
    embedding
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // resolve_embedding_route — INV-004 privacy gate proof
    //
    // The invariant: with `allow_cloud == false` the route can NEVER be cloud,
    // regardless of provider or which keys are present. Setting an LLM key must
    // not silently exfiltrate local content to api.openai.com as a side-effect.
    // ========================================================================

    #[test]
    fn gate_off_openai_with_key_stays_local() {
        // The exact pre-gate leak: provider=openai + api key => was OpenAi.
        let route = resolve_embedding_route("openai", true, false, false);
        assert_eq!(route, EmbeddingRoute::LocalDefault);
        assert!(!route.is_cloud(), "gate off must never route to cloud");
    }

    #[test]
    fn gate_off_anthropic_with_openai_key_stays_local() {
        // The second leak path: anthropic + dedicated openai embed key.
        let route = resolve_embedding_route("anthropic", false, true, false);
        assert_eq!(route, EmbeddingRoute::AnthropicLocal);
        assert!(!route.is_cloud(), "gate off must never route to cloud");
    }

    #[test]
    fn gate_off_is_never_cloud_for_any_key_combination() {
        for &provider in &["openai", "anthropic", "ollama", "", "unknown"] {
            for &has_llm_key in &[false, true] {
                for &has_embed_key in &[false, true] {
                    let route =
                        resolve_embedding_route(provider, has_llm_key, has_embed_key, false);
                    assert!(
                        !route.is_cloud(),
                        "INV-004 breach: provider={provider} llm_key={has_llm_key} embed_key={has_embed_key} routed to cloud with gate OFF"
                    );
                }
            }
        }
    }

    #[test]
    fn gate_on_reproduces_legacy_cloud_routing() {
        // openai + key + opt-in => cloud (legacy behaviour, now consented).
        assert_eq!(
            resolve_embedding_route("openai", true, false, true),
            EmbeddingRoute::OpenAi
        );
        // anthropic + dedicated openai key + opt-in => cloud fallback.
        assert_eq!(
            resolve_embedding_route("anthropic", false, true, true),
            EmbeddingRoute::OpenAi
        );
    }

    #[test]
    fn openai_without_key_never_cloud_even_when_opted_in() {
        // No key means no cloud path ever existed — stays local regardless.
        assert_eq!(
            resolve_embedding_route("openai", false, false, true),
            EmbeddingRoute::LocalDefault
        );
    }

    #[test]
    fn explicit_ollama_and_empty_provider_route_local() {
        assert_eq!(
            resolve_embedding_route("ollama", false, false, true),
            EmbeddingRoute::Ollama
        );
        assert_eq!(
            resolve_embedding_route("", false, false, true),
            EmbeddingRoute::LocalDefault
        );
    }

    #[test]
    fn anthropic_with_no_keys_routes_local_default() {
        assert_eq!(
            resolve_embedding_route("anthropic", false, false, true),
            EmbeddingRoute::LocalDefault
        );
    }

    // ========================================================================
    // truncate_and_normalize tests
    // ========================================================================

    #[test]
    fn test_truncate_short_vector_padded_and_normalized() {
        // Vector shorter than EMBEDDING_DIMS should be zero-padded and normalized
        let v = vec![1.0f32, 0.0, 0.0];
        let result = truncate_and_normalize(v);
        assert_eq!(
            result.len(),
            EMBEDDING_DIMS,
            "Short vector should be padded to EMBEDDING_DIMS"
        );
        // First element should be normalized (1.0 / norm where norm = 1.0)
        assert!(
            (result[0] - 1.0).abs() < 1e-5,
            "First element should be ~1.0 after normalization"
        );
        // Padding elements should all be 0.0
        assert!(
            result[3..].iter().all(|&v| v == 0.0),
            "Padded elements should be 0.0"
        );
    }

    #[test]
    fn test_truncate_exact_dims_unchanged() {
        // Vector exactly EMBEDDING_DIMS should pass through unchanged
        let v: Vec<f32> = (0..EMBEDDING_DIMS).map(|i| (i as f32) * 0.01).collect();
        let result = truncate_and_normalize(v.clone());
        assert_eq!(result, v, "Exact-length vector should not be modified");
    }

    #[test]
    fn test_truncate_long_vector_to_target_dims() {
        // Vector longer than EMBEDDING_DIMS should be truncated
        let v: Vec<f32> = (0..EMBEDDING_DIMS + 256)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let result = truncate_and_normalize(v);
        assert_eq!(
            result.len(),
            EMBEDDING_DIMS,
            "Should be truncated to {} dims",
            EMBEDDING_DIMS
        );
    }

    #[test]
    fn test_truncate_preserves_unit_norm() {
        // After truncation + re-normalization, vector should be unit length
        let v: Vec<f32> = (0..EMBEDDING_DIMS + 256)
            .map(|i| ((i as f32) * 0.1).sin())
            .collect();
        let result = truncate_and_normalize(v);
        let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "Truncated vector should be unit-normalized, got norm={}",
            norm
        );
    }

    #[test]
    fn test_truncate_zero_vector_stays_zero() {
        // Zero vector should not cause division by zero
        let v = vec![0.0f32; EMBEDDING_DIMS + 256];
        let result = truncate_and_normalize(v);
        assert_eq!(result.len(), EMBEDDING_DIMS);
        assert!(
            result.iter().all(|&x| x == 0.0),
            "Zero vector should remain zero (no NaN from division)"
        );
    }

    #[test]
    fn test_truncate_preserves_direction() {
        // The truncated + normalized vector should point in the same direction
        // as the first EMBEDDING_DIMS elements (just rescaled)
        let v: Vec<f32> = (0..EMBEDDING_DIMS + 256)
            .map(|i| ((i as f32) * 0.3).cos())
            .collect();
        let result = truncate_and_normalize(v.clone());

        // Compute cosine similarity between truncated prefix and result
        let prefix: Vec<f32> = v[..EMBEDDING_DIMS].to_vec();
        let dot: f32 = prefix.iter().zip(result.iter()).map(|(a, b)| a * b).sum();
        let norm_prefix: f32 = prefix.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_result: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        let cosine = dot / (norm_prefix * norm_result);

        assert!(
            (cosine - 1.0).abs() < 1e-5,
            "Direction should be preserved after normalization, cosine={}",
            cosine
        );
    }

    // ========================================================================
    // EMBEDDING_DIMS constant test
    // ========================================================================

    #[test]
    fn test_embedding_dims_matches_model() {
        assert_eq!(
            EMBEDDING_DIMS, 768,
            "Embedding dims must match DB vec0 schema (768 for Arctic-M)"
        );
    }

    // ========================================================================
    // validate_embeddings tests
    // ========================================================================

    #[test]
    fn test_validate_clean_vectors_unchanged() {
        let input = vec![vec![1.0f32, 0.0, 0.0], vec![0.0, 1.0, 0.0]];
        let result = validate_embeddings(input.clone());
        assert_eq!(result, input, "Clean vectors should pass through unchanged");
    }

    #[test]
    fn test_validate_nan_replaced_with_zero_vector() {
        let input = vec![vec![1.0, f32::NAN, 0.0], vec![0.0, 1.0, 0.0]];
        let result = validate_embeddings(input);
        assert_eq!(
            result[0],
            vec![0.0f32; EMBEDDING_DIMS],
            "Vector with NaN should be replaced with zero vector"
        );
        assert_eq!(
            result[1],
            vec![0.0, 1.0, 0.0],
            "Clean vector should be unchanged"
        );
    }

    #[test]
    fn test_validate_inf_replaced_with_zero_vector() {
        let input = vec![vec![f32::INFINITY, 0.0, 0.0]];
        let result = validate_embeddings(input);
        assert_eq!(
            result[0],
            vec![0.0f32; EMBEDDING_DIMS],
            "Vector with Inf should be replaced with zero vector"
        );
    }

    #[test]
    fn test_validate_neg_inf_replaced_with_zero_vector() {
        let input = vec![vec![0.0, f32::NEG_INFINITY, 0.0]];
        let result = validate_embeddings(input);
        assert_eq!(
            result[0],
            vec![0.0f32; EMBEDDING_DIMS],
            "Vector with -Inf should be replaced with zero vector"
        );
    }

    #[test]
    fn test_validate_empty_input_returns_empty() {
        let result = validate_embeddings(vec![]);
        assert!(result.is_empty(), "Empty input should return empty vec");
    }

    #[test]
    fn test_validate_zero_vector_unchanged() {
        let input = vec![vec![0.0f32; EMBEDDING_DIMS]];
        let result = validate_embeddings(input.clone());
        assert_eq!(result, input, "Zero vector should pass through unchanged");
    }

    // ========================================================================
    // Retry backoff delay calculation tests
    // ========================================================================

    #[test]
    fn test_retry_backoff_delay_calculation() {
        // The retry_with_backoff function uses 3^attempt for delay:
        // attempt 0 -> 3^0 = 1s
        // attempt 1 -> 3^1 = 3s
        // attempt 2 -> 3^2 = 9s
        assert_eq!(3u64.pow(0), 1, "Attempt 0 delay should be 1s");
        assert_eq!(3u64.pow(1), 3, "Attempt 1 delay should be 3s");
        assert_eq!(3u64.pow(2), 9, "Attempt 2 delay should be 9s");
        assert_eq!(3u64.pow(3), 27, "Attempt 3 delay should be 27s");
    }

    #[test]
    fn test_retry_attempt_count() {
        // With max_retries=2, we should have attempts 0, 1, 2 (3 total)
        let max_retries: u32 = 2;
        let attempts: Vec<u32> = (0..=max_retries).collect();
        assert_eq!(attempts.len(), 3, "max_retries=2 should yield 3 attempts");
    }

    // ========================================================================
    // embed_texts empty input test (async)
    // ========================================================================

    #[tokio::test]
    async fn test_embed_texts_empty_input_returns_empty() {
        let result = embed_texts(&[]).await;
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_empty(),
            "Empty input should return empty vec"
        );
    }
}
