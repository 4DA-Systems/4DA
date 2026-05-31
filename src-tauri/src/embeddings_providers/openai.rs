// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! OpenAI embedding provider (text-embedding-3-small).

use crate::error::{FourDaError, Result, ResultExt};
use crate::get_settings_manager;

use super::EMBEDDING_CLIENT;

/// Generate embeddings using OpenAI API
pub(in crate::embeddings) async fn embed_texts_openai(
    texts: &[String],
    api_key: &str,
) -> Result<Vec<Vec<f32>>> {
    if api_key.is_empty() {
        return Err("OpenAI API key not configured".into());
    }

    let body = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": texts,
        "dimensions": crate::EMBEDDING_DIMS
    });

    let response = EMBEDDING_CLIENT
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key.trim()))
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
