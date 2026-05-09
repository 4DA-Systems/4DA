// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Provider-specific embedding functions (OpenAI, Ollama) and retry logic.
//!
//! Split from `embeddings.rs` to keep both modules under the 700-line threshold.

use crate::error::{FourDaError, Result, ResultExt};
use crate::get_settings_manager;

use super::{truncate_and_normalize, EMBEDDING_CLIENT};

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
        "dimensions": 384  // Match DB vec0 schema (384-dim MiniLM-compatible)
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
