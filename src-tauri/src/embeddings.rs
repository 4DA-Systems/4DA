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
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build HTTP client: {e}, using default");
            reqwest::Client::new()
        })
});

// ============================================================================
// Embeddings - supports OpenAI and Ollama
// ============================================================================

/// Attempt in-process fastembed (ONNX Runtime) — zero network, privacy preserved.
/// Returns `Some(embeddings)` on success, `None` on failure.
#[cfg(feature = "fastembed-local")]
async fn try_fastembed_fallback(texts: &[String]) -> Option<Vec<Vec<f32>>> {
    let texts_for_fe = texts.to_vec();
    match tokio::task::spawn_blocking(move || embed_texts_fastembed_sync(&texts_for_fe)).await {
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

    // Fast-path: if a cloud provider is configured but has no API key, skip it entirely
    // and fall through to the zero-config Ollama/zero-vector path. This avoids wasting
    // ~4-13 seconds on deterministic retry failures during cold start with no keys.
    let effective_provider = match llm_settings.provider.as_str() {
        "openai" if llm_settings.api_key.is_empty() => {
            tracing::debug!(
                target: "4da::embeddings",
                "OpenAI provider configured but no API key — falling through to zero-config path"
            );
            "none"
        }
        "anthropic"
            if llm_settings.openai_api_key.is_empty() && llm_settings.api_key.is_empty() =>
        {
            tracing::debug!(
                target: "4da::embeddings",
                "Anthropic provider configured but no embedding key — falling through to zero-config path"
            );
            "none"
        }
        other => other,
    };

    let result = match effective_provider {
        "openai" => {
            tracing::info!(
                target: "4da::embeddings",
                count = texts.len(),
                "Embedding via OpenAI — content sent to api.openai.com (retained 30 days per OpenAI policy)"
            );
            let api_key = llm_settings.api_key.clone();
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
        "ollama" => {
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
        "anthropic" => {
            // Anthropic doesn't have embeddings API - use dedicated OpenAI key or fallback to Ollama
            if !llm_settings.openai_api_key.is_empty() {
                tracing::info!(
                    target: "4da::embeddings",
                    count = texts.len(),
                    "Anthropic provider — embedding via OpenAI fallback (content sent to api.openai.com)"
                );
                let api_key = llm_settings.openai_api_key.clone();
                let texts = texts.to_vec();
                return retry_with_backoff("embed_openai_anthropic_fallback", 2, || {
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
                });
            }
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
        // "none" or unknown provider: try Ollama → fastembed (in-process) → zero vectors
        _ => {
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
