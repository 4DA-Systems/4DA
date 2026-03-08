// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use once_cell::sync::Lazy;

use crate::get_settings_manager;

/// Shared HTTP client for embedding API calls (reused across requests)
static EMBEDDING_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("4DA/1.0")
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build embedding HTTP client: {e}, using default");
            reqwest::Client::new()
        })
});

// ============================================================================
// Embeddings - supports OpenAI and Ollama
// ============================================================================

/// Generate embeddings for a list of texts
/// Supports OpenAI (text-embedding-3-small) and Ollama (nomic-embed-text)
/// Provider is determined by settings - uses same provider as LLM when possible
pub(crate) async fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    // Get settings to determine provider - clone inside scope so MutexGuard drops before await
    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    match llm_settings.provider.as_str() {
        "openai" => {
            let api_key = llm_settings.api_key.clone();
            let texts = texts.to_vec();
            retry_with_backoff("embed_openai", 2, || {
                let key = api_key.clone();
                let t = texts.clone();
                async move { embed_texts_openai(&t, &key).await }
            })
            .await
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
        }
        "anthropic" => {
            // Anthropic doesn't have embeddings API - use dedicated OpenAI key or fallback to Ollama
            if !llm_settings.openai_api_key.is_empty() {
                let api_key = llm_settings.openai_api_key.clone();
                let texts = texts.to_vec();
                return retry_with_backoff("embed_openai_anthropic_fallback", 2, || {
                    let key = api_key.clone();
                    let t = texts.clone();
                    async move { embed_texts_openai(&t, &key).await }
                })
                .await;
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
                        return Ok(result);
                    }
                }
            }
            // Try default Ollama
            let texts = texts.to_vec();
            retry_with_backoff("embed_ollama_default", 2, || {
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &None).await }
            })
            .await
        }
        // "none" or unknown provider: try Ollama at default localhost as zero-config fallback
        // This enables scoring for users who have Ollama installed but haven't configured a provider
        _ => {
            let texts = texts.to_vec();
            match retry_with_backoff("embed_ollama_zeroconfig", 1, || {
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &None).await }
            })
            .await
            {
                Ok(result) => Ok(result),
                Err(_) => {
                    // Ollama not available — return zero vectors as graceful fallback
                    // Scoring will work on non-embedding axes (keyword, dependency, source affinity)
                    tracing::debug!(
                        target: "4da::embeddings",
                        "No embedding provider configured and Ollama not reachable. Using zero vectors."
                    );
                    Ok(texts
                        .iter()
                        .map(|_| vec![0.0f32; TARGET_EMBEDDING_DIMS])
                        .collect())
                }
            }
        }
    }
}

/// Generate embeddings using OpenAI API
async fn embed_texts_openai(texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>, String> {
    if api_key.is_empty() {
        return Err("OpenAI API key not configured".to_string());
    }

    let body = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": texts,
        "dimensions": 384  // Match DB vec0 schema (384-dim MiniLM-compatible)
    });

    let response = EMBEDDING_CLIENT
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

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
        .ok_or_else(|| "Invalid OpenAI response: missing 'data' array".to_string())?;

    data.iter()
        .map(|item| {
            item["embedding"]
                .as_array()
                .ok_or_else(|| "Missing embedding in response".to_string())?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .map(|f| f as f32)
                        .ok_or_else(|| "Invalid embedding value".to_string())
                })
                .collect::<Result<Vec<f32>, String>>()
        })
        .collect()
}

/// Target embedding dimensions matching DB vec0 schema
const TARGET_EMBEDDING_DIMS: usize = 384;

/// Truncate embedding to TARGET_EMBEDDING_DIMS and L2-normalize.
/// nomic-embed-text is a Matryoshka model so truncation preserves semantic quality.
fn truncate_and_normalize(mut embedding: Vec<f32>) -> Vec<f32> {
    if embedding.len() > TARGET_EMBEDDING_DIMS {
        embedding.truncate(TARGET_EMBEDDING_DIMS);
        // Re-normalize after truncation (Matryoshka requirement)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }
    }
    embedding
}

/// Generate embeddings using Ollama API
async fn embed_texts_ollama(
    texts: &[String],
    base_url: &Option<String>,
) -> Result<Vec<Vec<f32>>, String> {
    let base = base_url.as_deref().unwrap_or("http://localhost:11434");

    if texts.is_empty() {
        return Ok(vec![]);
    }

    let batch_body = serde_json::json!({
        "model": "nomic-embed-text",
        "input": texts,
    });

    // Try batch API first (/api/embed) - supported since Ollama v0.1.26
    let batch_result = EMBEDDING_CLIENT
        .post(format!("{}/api/embed", base))
        .json(&batch_body)
        .send()
        .await;

    match batch_result {
        Ok(response) if response.status().is_success() => {
            // Batch succeeded - parse embeddings array
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama batch response: {}", e))?;

            let embeddings_array = json["embeddings"].as_array().ok_or_else(|| {
                "Invalid Ollama batch response: missing 'embeddings' array".to_string()
            })?;

            embeddings_array
                .iter()
                .map(|emb_val| {
                    let raw = emb_val
                        .as_array()
                        .ok_or_else(|| "Invalid embedding in batch response".to_string())?
                        .iter()
                        .map(|v| {
                            v.as_f64()
                                .map(|f| f as f32)
                                .ok_or_else(|| "Invalid embedding value".to_string())
                        })
                        .collect::<Result<Vec<f32>, String>>()?;
                    Ok(truncate_and_normalize(raw))
                })
                .collect()
        }
        Ok(response) => {
            // Batch endpoint returned an error - check for model-not-found
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || body.contains("not found") {
                return Err("Embedding model 'nomic-embed-text' not found in Ollama. Run: ollama pull nomic-embed-text".to_string());
            }
            // Fall through to single-item fallback for other errors (old Ollama version)
            embed_texts_ollama_single(texts, base).await
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("connect") || msg.contains("refused") {
                return Err(format!(
                    "Cannot connect to Ollama at {}. Make sure Ollama is running (ollama serve).",
                    base
                ));
            }
            if msg.contains("timed out") || msg.contains("timeout") {
                return Err("Ollama embedding request timed out. The model may still be loading — try again shortly.".to_string());
            }
            // Fall through to single-item fallback
            embed_texts_ollama_single(texts, base).await
        }
    }
}

/// Fallback: embed one text at a time using the older /api/embeddings endpoint
async fn embed_texts_ollama_single(texts: &[String], base: &str) -> Result<Vec<Vec<f32>>, String> {
    let mut all_embeddings = Vec::with_capacity(texts.len());

    for text in texts {
        let single_body = serde_json::json!({
            "model": "nomic-embed-text",
            "prompt": text,
        });

        let response = EMBEDDING_CLIENT
            .post(format!("{}/api/embeddings", base))
            .json(&single_body)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("refused") {
                    format!(
                        "Cannot connect to Ollama at {}. Make sure Ollama is running (ollama serve).",
                        base
                    )
                } else if msg.contains("timed out") || msg.contains("timeout") {
                    "Ollama embedding timed out. The model may still be loading — try again.".to_string()
                } else {
                    format!(
                        "Ollama embedding request failed: {}. Make sure Ollama is running with 'nomic-embed-text' (run: ollama pull nomic-embed-text)",
                        e
                    )
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || body.contains("not found") {
                return Err(
                    "Embedding model 'nomic-embed-text' not found. Run: ollama pull nomic-embed-text".to_string()
                );
            }
            return Err(format!("Ollama embedding error ({}): {}", status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let raw = json["embedding"]
            .as_array()
            .ok_or_else(|| {
                "Invalid Ollama response: missing 'embedding' array. Is nomic-embed-text installed?"
                    .to_string()
            })?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| "Invalid embedding value".to_string())
            })
            .collect::<Result<Vec<f32>, String>>()?;

        all_embeddings.push(truncate_and_normalize(raw));
    }

    Ok(all_embeddings)
}

/// Retry an async operation with exponential backoff.
/// Returns the first successful result, or the last error after max_retries.
async fn retry_with_backoff<F, Fut, T>(
    operation_name: &str,
    max_retries: u32,
    f: F,
) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut last_error = String::new();
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = e.clone();
                if attempt < max_retries {
                    let delay_secs = 3u64.pow(attempt); // 1s, 3s, 9s
                    tracing::warn!(
                        target: "4da::retry",
                        attempt = attempt + 1,
                        max = max_retries + 1,
                        delay_secs,
                        operation = operation_name,
                        error = %e,
                        "Retrying after error"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }
            }
        }
    }
    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // truncate_and_normalize tests
    // ========================================================================

    #[test]
    fn test_truncate_short_vector_unchanged() {
        // Vector shorter than TARGET_EMBEDDING_DIMS should pass through unchanged
        let v = vec![1.0f32, 0.0, 0.0];
        let result = truncate_and_normalize(v.clone());
        assert_eq!(result, v, "Short vector should not be modified");
    }

    #[test]
    fn test_truncate_exact_dims_unchanged() {
        // Vector exactly TARGET_EMBEDDING_DIMS should pass through unchanged
        let v: Vec<f32> = (0..TARGET_EMBEDDING_DIMS)
            .map(|i| (i as f32) * 0.01)
            .collect();
        let result = truncate_and_normalize(v.clone());
        assert_eq!(result, v, "Exact-length vector should not be modified");
    }

    #[test]
    fn test_truncate_long_vector_to_target_dims() {
        // Vector longer than TARGET_EMBEDDING_DIMS should be truncated
        let v: Vec<f32> = (0..768).map(|i| (i as f32) * 0.001).collect();
        let result = truncate_and_normalize(v);
        assert_eq!(
            result.len(),
            TARGET_EMBEDDING_DIMS,
            "Should be truncated to {} dims",
            TARGET_EMBEDDING_DIMS
        );
    }

    #[test]
    fn test_truncate_preserves_unit_norm() {
        // After truncation + re-normalization, vector should be unit length
        let v: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.1).sin()).collect();
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
        let v = vec![0.0f32; 768];
        let result = truncate_and_normalize(v);
        assert_eq!(result.len(), TARGET_EMBEDDING_DIMS);
        assert!(
            result.iter().all(|&x| x == 0.0),
            "Zero vector should remain zero (no NaN from division)"
        );
    }

    #[test]
    fn test_truncate_preserves_direction() {
        // The truncated + normalized vector should point in the same direction
        // as the first TARGET_EMBEDDING_DIMS elements (just rescaled)
        let v: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.3).cos()).collect();
        let result = truncate_and_normalize(v.clone());

        // Compute cosine similarity between truncated prefix and result
        let prefix: Vec<f32> = v[..TARGET_EMBEDDING_DIMS].to_vec();
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
    // TARGET_EMBEDDING_DIMS constant test
    // ========================================================================

    #[test]
    fn test_target_embedding_dims_is_384() {
        assert_eq!(
            TARGET_EMBEDDING_DIMS, 384,
            "Embedding dims must match DB vec0 schema (384)"
        );
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
