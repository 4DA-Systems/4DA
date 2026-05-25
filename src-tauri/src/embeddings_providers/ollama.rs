// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Ollama embedding provider with batch API support and single-item fallback.

use crate::error::{FourDaError, Result, ResultExt};

use super::{truncate_and_normalize, EMBEDDING_CLIENT};

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
pub(in crate::embeddings) async fn embed_texts_ollama(
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
