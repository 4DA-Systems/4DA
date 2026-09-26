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
        if is_local_endpoint(url) {
            return Ok(());
        }

        tracing::info!(
            target: "4da::security",
            url = %url,
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

/// Whether `url` is Ollama on this machine (the hosts HTTP is allowed to).
/// Parsed as a URL: splitting on ':' cut `[::1]` down to `[`.
fn is_local_endpoint(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .is_some_and(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]"))
}

/// An embed request body. On a local Ollama the embedding model runs on the
/// CPU (`num_gpu: 0`), leaving the GPU to the local judge. Measured 2026-09-26
/// on a 16 GB card with nomic-embed-text: the CPU vectors match the GPU ones
/// (cosine >= 0.99999 on 32 texts), so no re-embed is needed, and a 32-text
/// batch takes 1.56 s against 1.20 s on the GPU. With the embedder on the GPU,
/// Ollama evicted and reloaded gemma4:26b around every embed call, and the
/// judge's median went from 4.3 s to 101.5 s per item. A remote Ollama has its
/// own GPU, so its requests are left unchanged.
fn embed_body(base: &str, mut body: serde_json::Value) -> serde_json::Value {
    if is_local_endpoint(base) {
        body["options"] = serde_json::json!({ "num_gpu": 0 });
    }
    body
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

    let batch_body = embed_body(
        base,
        serde_json::json!({
            "model": embedding_model,
            "input": texts,
        }),
    );

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
        let single_body = embed_body(
            base,
            serde_json::json!({
                "model": &embedding_model,
                "prompt": text,
            }),
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_ollama_embeds_on_the_cpu() {
        for base in [
            "http://localhost:11434",
            "http://127.0.0.1:11434/",
            "http://[::1]:11434",
        ] {
            let body = embed_body(base, serde_json::json!({ "model": "m", "input": ["a"] }));
            assert_eq!(body["options"]["num_gpu"], 0, "{base}");
            assert_eq!(body["input"][0], "a");
        }
    }

    #[test]
    fn http_is_allowed_only_to_this_machine() {
        assert!(validate_ollama_endpoint("http://[::1]:11434").is_ok());
        assert!(validate_ollama_endpoint("http://localhost:11434").is_ok());
        assert!(validate_ollama_endpoint("http://localhost.evil.com:11434").is_err());
        assert!(validate_ollama_endpoint("http://10.0.0.5:11434").is_err());
        assert!(validate_ollama_endpoint("https://gpu.example.com").is_ok());
    }

    #[test]
    fn a_remote_ollama_keeps_its_own_gpu() {
        let body = embed_body(
            "https://gpu.example.com",
            serde_json::json!({ "model": "m", "prompt": "a" }),
        );
        assert!(body.get("options").is_none());
    }
}
