//! LLM client module for 4DA
//!
//! Provides a unified interface for interacting with different LLM providers:
//! - Anthropic Claude (recommended for relevance judging)
//! - OpenAI GPT
//! - Ollama (local, free)

use crate::error::{Result, ResultExt};
use crate::settings::LLMProvider;
use crate::state::{is_llm_limit_reached, record_llm_cost, record_llm_tokens};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Static HTTP client for Ollama fallback — avoids creating a new client per call.
static OLLAMA_FALLBACK_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    });

pub use crate::llm_judge::RelevanceJudge;

/// Sanitize API error response text to prevent leaking secrets.
/// - Truncates to 200 characters
/// - Redacts strings that look like API keys (sk-, pk_, or long alphanumeric runs)
pub(crate) fn sanitize_api_error(text: &str) -> String {
    let truncated = if text.len() > 200 {
        format!("{}...", &text[..text.floor_char_boundary(200)])
    } else {
        text.to_string()
    };

    // Redact potential API key patterns
    let mut result = String::with_capacity(truncated.len());
    for word in truncated.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        // Check for API key patterns: sk-*, pk_*, or long alphanumeric tokens
        let trimmed = word.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ':');
        if trimmed.starts_with("sk-")
            || trimmed.starts_with("pk_")
            || (trimmed.len() > 32
                && trimmed
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_'))
        {
            result.push_str("[REDACTED]");
        } else {
            result.push_str(word);
        }
    }
    result
}

// ============================================================================
// Types
// ============================================================================

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

/// Response from LLM
#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Result of a relevance judgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevanceJudgment {
    pub item_id: String,
    pub relevant: bool,
    pub confidence: f32, // 0.0 - 1.0
    pub reasoning: String,
    pub key_connections: Vec<String>,
}

// ============================================================================
// AI Usage Recording
// ============================================================================

/// Record an AI usage entry for cost tracking.
/// Best-effort: silently ignores errors to never interfere with LLM operations.
fn record_ai_usage(provider: &str, model: &str, task_type: &str, tokens_in: u64, tokens_out: u64) {
    if let Ok(conn) = crate::open_db_connection() {
        let cost =
            crate::ai_costs::estimate_cost(provider, model, tokens_in as u32, tokens_out as u32);
        conn.execute(
            "INSERT INTO ai_usage (provider, model, task_type, tokens_in, tokens_out, estimated_cost_usd, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            rusqlite::params![provider, model, task_type, tokens_in as i64, tokens_out as i64, cost],
        )
        .ok(); // Best-effort — never fail the LLM call
    }
}

// ============================================================================
// LLM Client
// ============================================================================

pub struct LLMClient {
    provider: LLMProvider,
    client: reqwest::Client,
}

impl LLMClient {
    pub fn new(provider: LLMProvider) -> Self {
        // Ollama needs longer timeout for cold model loads
        let timeout_secs = if provider.provider == "ollama" {
            120
        } else {
            60
        };
        Self {
            provider,
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(timeout_secs))
                .build()
                .unwrap_or_else(|e| {
                    warn!("Failed to build HTTP client: {e}, using default");
                    reqwest::Client::new()
                }),
        }
    }

    /// Check if the client is configured
    #[allow(dead_code)] // Reason: used in tests for provider validation
    pub fn is_configured(&self) -> bool {
        match self.provider.provider.as_str() {
            "anthropic" | "openai" | "openai-compatible" => !self.provider.api_key.is_empty(),
            "ollama" => true, // Ollama doesn't need an API key
            _ => false,
        }
    }

    /// Send a completion request.
    /// Enforces daily token and cost limits — returns an error if the budget is exhausted.
    /// When a cloud provider (anthropic/openai) fails with a network or API error,
    /// transparently falls back to local Ollama at localhost:11434.
    pub async fn complete(&self, system: &str, messages: Vec<Message>) -> Result<LLMResponse> {
        // Hard cutoff: refuse to call the LLM if daily limit is already reached
        if is_llm_limit_reached() {
            let (tokens_used, tokens_limit) = crate::state::get_llm_token_usage();
            let (cost_used, cost_limit) = crate::state::get_llm_cost_usage();
            warn!(
                target: "4da::llm",
                tokens_used = tokens_used,
                tokens_limit = tokens_limit,
                cost_used_cents = cost_used,
                cost_limit_cents = cost_limit,
                "LLM call blocked — daily limit reached"
            );
            return Err(
                Self::format_limit_error(tokens_used, tokens_limit, cost_used, cost_limit).into(),
            );
        }

        let result = match self.provider.provider.as_str() {
            "anthropic" => self.complete_anthropic(system, messages.clone()).await,
            "openai" | "openai-compatible" => self.complete_openai(system, messages.clone()).await,
            "ollama" => self.complete_ollama(system, messages.clone()).await,
            _ => return Err(format!("Unknown provider: {}", self.provider.provider).into()),
        };

        // If a cloud provider failed, attempt Ollama fallback
        let response = match result {
            Ok(resp) => resp,
            Err(ref err) if self.should_fallback_to_ollama(err) => {
                warn!(
                    target: "4da::llm",
                    provider = %self.provider.provider,
                    error = %err,
                    "Cloud LLM failed, falling back to local Ollama"
                );
                // Record the cloud failure to error telemetry (fallback may still succeed)
                crate::telemetry::record_error_async(
                    "llm",
                    &format!("{err}"),
                    Some(&self.provider.provider),
                );
                self.complete_ollama_fallback(system, messages).await?
            }
            Err(err) => {
                // Record hard LLM failure to error telemetry
                crate::telemetry::record_error_async(
                    "llm",
                    &format!("{err}"),
                    Some(&self.provider.provider),
                );
                return Err(err);
            }
        };

        // Record token + cost usage (atomic, lock-free for the counters)
        let total_tokens = response.input_tokens + response.output_tokens;
        if total_tokens > 0 {
            let tokens_ok = record_llm_tokens(total_tokens);
            let cost_cents =
                self.estimate_cost_cents(response.input_tokens, response.output_tokens);
            let cost_ok = record_llm_cost(cost_cents);
            if !tokens_ok || !cost_ok {
                debug!(
                    target: "4da::llm",
                    tokens = total_tokens,
                    cost_cents = cost_cents,
                    "Limit exceeded after this call — future calls will be blocked"
                );
            }

            // Record AI usage for cost tracking (best-effort, never fail the LLM call)
            record_ai_usage(
                &self.provider.provider,
                &self.provider.model,
                "completion",
                response.input_tokens,
                response.output_tokens,
            );
        }

        Ok(response)
    }

    /// Determine whether a failed LLM call should fall back to Ollama.
    /// Only falls back when:
    /// - The current provider is NOT already Ollama
    /// - The error looks like a network/API issue (not a token-limit or budget error)
    fn should_fallback_to_ollama(&self, error: &crate::error::FourDaError) -> bool {
        // Never fallback if already using Ollama
        if self.provider.provider == "ollama" {
            return false;
        }

        // Don't fallback for token/budget/cost limit errors (these are intentional caps)
        let error_str = error.to_string();
        let is_limit_error = error_str.contains("token limit")
            || error_str.contains("cost limit")
            || error_str.contains("rate limit")
            || error_str.contains("quota")
            || error_str.contains("billing")
            || error_str.contains("insufficient_quota");

        !is_limit_error
    }

    /// Ollama fallback: uses localhost:11434 with a sensible default model.
    /// This is only called when the primary cloud provider has failed.
    async fn complete_ollama_fallback(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse> {
        let fallback_base_url = "http://localhost:11434";
        let fallback_model = "llama3.2";
        let url = format!("{fallback_base_url}/api/chat");

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": system
        })];

        for m in &messages {
            all_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let body = serde_json::json!({
            "model": fallback_model,
            "messages": all_messages,
            "stream": false
        });

        let response = OLLAMA_FALLBACK_CLIENT
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Ollama fallback also failed (is Ollama running?)")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!(
                "Ollama fallback error {}: {}",
                status,
                sanitize_api_error(&text)
            )
            .into());
        }

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Ollama fallback response")?;

        let content = data["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let input_tokens = data["prompt_eval_count"].as_u64().unwrap_or(0);
        let output_tokens = data["eval_count"].as_u64().unwrap_or(0);

        warn!(
            target: "4da::llm",
            model = fallback_model,
            input_tokens = input_tokens,
            output_tokens = output_tokens,
            "Ollama fallback succeeded"
        );

        Ok(LLMResponse {
            content,
            input_tokens,
            output_tokens,
        })
    }

    /// Anthropic Claude API
    async fn complete_anthropic(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse> {
        let url = "https://api.anthropic.com/v1/messages";

        let body = serde_json::json!({
            "model": self.provider.model,
            "max_tokens": 4096,  // Increased for batch judgments (15 items need ~2000+ tokens)
            "system": system,
            "messages": messages.iter().map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            }).collect::<Vec<_>>()
        });

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.provider.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Anthropic API request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!(
                "Anthropic API error {}: {}",
                status,
                sanitize_api_error(&text)
            )
            .into());
        }

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        let content = data["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let input_tokens = data["usage"]["input_tokens"].as_u64().unwrap_or(0);
        let output_tokens = data["usage"]["output_tokens"].as_u64().unwrap_or(0);

        Ok(LLMResponse {
            content,
            input_tokens,
            output_tokens,
        })
    }

    /// OpenAI API (also used for openai-compatible providers)
    async fn complete_openai(&self, system: &str, messages: Vec<Message>) -> Result<LLMResponse> {
        let url = if self.provider.provider == "openai-compatible" {
            // OpenAI-compatible: base_url is the API base (e.g. https://api.groq.com/openai/v1)
            let base = self
                .provider
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let base = base.trim_end_matches('/');
            if base.ends_with("/chat/completions") {
                base.to_string()
            } else {
                format!("{base}/chat/completions")
            }
        } else {
            self.provider
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1/chat/completions")
                .to_string()
        };

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": system
        })];

        for m in &messages {
            all_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let body = serde_json::json!({
            "model": self.provider.model,
            "max_tokens": 4096,  // Increased for batch judgments
            "messages": all_messages
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.provider.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("OpenAI API request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(
                format!("OpenAI API error {}: {}", status, sanitize_api_error(&text)).into(),
            );
        }

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let input_tokens = data["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
        let output_tokens = data["usage"]["completion_tokens"].as_u64().unwrap_or(0);

        Ok(LLMResponse {
            content,
            input_tokens,
            output_tokens,
        })
    }

    /// Ollama API (local)
    async fn complete_ollama(&self, system: &str, messages: Vec<Message>) -> Result<LLMResponse> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let url = format!("{base_url}/api/chat");

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": system
        })];

        for m in &messages {
            all_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let body = serde_json::json!({
            "model": self.provider.model,
            "messages": all_messages,
            "stream": false
        });

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("refused") {
                    format!(
                        "Cannot connect to Ollama at {base_url}. Make sure Ollama is running (ollama serve)."
                    )
                } else if msg.contains("timed out") || msg.contains("timeout") {
                    format!(
                        "Ollama request timed out. Model '{}' may be loading or the prompt is too large. Try again.",
                        self.provider.model
                    )
                } else {
                    format!("Ollama request failed: {e}")
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || text.contains("not found") {
                return Err(format!(
                    "Model '{}' not found in Ollama. Run: ollama pull {}",
                    self.provider.model, self.provider.model
                )
                .into());
            }
            if text.contains("out of memory") || text.contains("OOM") || text.contains("CUDA") {
                return Err(format!(
                    "Not enough GPU memory for '{}'. Try a smaller model.",
                    self.provider.model
                )
                .into());
            }
            return Err(format!("Ollama error {}: {}", status, sanitize_api_error(&text)).into());
        }

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        let content = data["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Ollama doesn't report token usage the same way
        let input_tokens = data["prompt_eval_count"].as_u64().unwrap_or(0);
        let output_tokens = data["eval_count"].as_u64().unwrap_or(0);

        Ok(LLMResponse {
            content,
            input_tokens,
            output_tokens,
        })
    }

    /// Send a streaming completion request.
    /// Tokens are delivered progressively via `on_token` callback.
    /// Falls back to local Ollama on cloud provider failure.
    /// Returns the complete `LLMResponse` when finished.
    pub async fn stream_complete<F>(
        &self,
        system: &str,
        messages: Vec<Message>,
        on_token: F,
    ) -> Result<LLMResponse>
    where
        F: Fn(&str) + Send + 'static,
    {
        // Hard cutoff: refuse to call the LLM if daily limit is already reached
        if is_llm_limit_reached() {
            let (tokens_used, tokens_limit) = crate::state::get_llm_token_usage();
            let (cost_used, cost_limit) = crate::state::get_llm_cost_usage();
            warn!(
                target: "4da::llm",
                tokens_used = tokens_used,
                tokens_limit = tokens_limit,
                cost_used_cents = cost_used,
                cost_limit_cents = cost_limit,
                "Streaming LLM call blocked — daily limit reached"
            );
            return Err(
                Self::format_limit_error(tokens_used, tokens_limit, cost_used, cost_limit).into(),
            );
        }

        let result = match self.provider.provider.as_str() {
            "anthropic" => {
                crate::llm_stream::stream_anthropic(
                    &self.client,
                    &self.provider,
                    system,
                    messages.clone(),
                    on_token,
                )
                .await
            }
            "openai" | "openai-compatible" => {
                crate::llm_stream::stream_openai(
                    &self.client,
                    &self.provider,
                    system,
                    messages.clone(),
                    on_token,
                )
                .await
            }
            "ollama" => {
                crate::llm_stream::stream_ollama(
                    &self.client,
                    &self.provider,
                    system,
                    messages.clone(),
                    on_token,
                )
                .await
            }
            _ => return Err(format!("Unknown provider: {}", self.provider.provider).into()),
        };

        // If a cloud provider failed, attempt streaming Ollama fallback
        let response = match result {
            Ok(resp) => resp,
            Err(ref err) if self.should_fallback_to_ollama(err) => {
                warn!(
                    target: "4da::llm",
                    provider = %self.provider.provider,
                    error = %err,
                    "Cloud LLM streaming failed, falling back to local Ollama"
                );
                // on_token was consumed by the first attempt; create a no-op for fallback
                // since partial tokens may have already been emitted
                crate::llm_stream::stream_ollama_fallback(system, messages, |_| {}).await?
            }
            Err(err) => return Err(err),
        };

        // Record token + cost usage
        let total_tokens = response.input_tokens + response.output_tokens;
        if total_tokens > 0 {
            let tokens_ok = record_llm_tokens(total_tokens);
            let cost_cents =
                self.estimate_cost_cents(response.input_tokens, response.output_tokens);
            let cost_ok = record_llm_cost(cost_cents);
            if !tokens_ok || !cost_ok {
                debug!(
                    target: "4da::llm",
                    tokens = total_tokens,
                    cost_cents = cost_cents,
                    "Limit exceeded after streaming call — future calls will be blocked"
                );
            }

            // Record AI usage for cost tracking (best-effort, never fail the LLM call)
            record_ai_usage(
                &self.provider.provider,
                &self.provider.model,
                "streaming",
                response.input_tokens,
                response.output_tokens,
            );
        }

        Ok(response)
    }

    /// Format a human-readable error message when daily limits are reached.
    fn format_limit_error(
        tokens_used: u64,
        tokens_limit: u64,
        cost_used: u64,
        cost_limit: u64,
    ) -> String {
        let mut parts = Vec::new();

        if tokens_limit > 0 && tokens_used >= tokens_limit {
            parts.push(format!(
                "Daily LLM token limit exceeded (used: {tokens_used}, limit: {tokens_limit})"
            ));
        }
        if cost_limit > 0 && cost_used >= cost_limit {
            parts.push(format!(
                "Daily LLM cost limit exceeded (used: {}c, limit: {}c / ${:.2})",
                cost_used,
                cost_limit,
                cost_limit as f64 / 100.0
            ));
        }

        if parts.is_empty() {
            // Fallback (shouldn't happen, but be defensive)
            parts.push("Daily LLM limit reached".to_string());
        }

        parts.push("Adjust in Settings > Intelligence, or wait until midnight reset.".to_string());
        parts.join(". ")
    }

    /// Estimate cost in cents based on provider and tokens.
    /// Returns 0 for unknown models (backward compat).
    pub fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        crate::model_registry::estimate_cost_or_zero(
            &self.provider.provider,
            &self.provider.model,
            input_tokens,
            output_tokens,
        )
    }
}

// ============================================================================
// Ollama Utilities
// ============================================================================

/// List available models from Ollama API
#[allow(dead_code)] // Reason: utility for future Ollama model discovery; no callers yet
pub async fn list_ollama_models(base_url: &str) -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!("{base_url}/api/tags");
    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to connect to Ollama")?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned error: {}", response.status()).into());
    }

    let data: serde_json::Value = response.json().await.context("Failed to parse response")?;

    let models = data["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_estimation_haiku() {
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: "test".to_string(),
            model: "claude-haiku-4-5-20251001".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);

        // 10k input, 1k output
        let cost = client.estimate_cost_cents(10_000, 1_000);
        // Haiku 4.5: $1.00/1M input, $5.00/1M output (updated pricing)
        // 10k input = $0.01, 1k output = $0.005 = ~1.5 cents
        // Allow up to 5 cents to handle pricing updates from LiteLLM registry
        assert!(cost <= 5, "Haiku 10k/1k should cost <5 cents, got {cost}");
    }

    #[test]
    fn test_ollama_is_free() {
        let provider = LLMProvider {
            provider: "ollama".to_string(),
            api_key: String::new(),
            model: "llama3".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);

        let cost = client.estimate_cost_cents(100_000, 10_000);
        assert_eq!(cost, 0);
    }

    // ========================================================================
    // is_configured — empty API key handling
    // ========================================================================

    #[test]
    fn test_is_configured_empty_api_key_anthropic() {
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: String::new(),
            model: "claude-haiku-4-5-20251001".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            !client.is_configured(),
            "Anthropic with empty API key should not be configured"
        );
    }

    #[test]
    fn test_is_configured_empty_api_key_openai() {
        let provider = LLMProvider {
            provider: "openai".to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            !client.is_configured(),
            "OpenAI with empty API key should not be configured"
        );
    }

    #[test]
    fn test_is_configured_ollama_no_key_needed() {
        let provider = LLMProvider {
            provider: "ollama".to_string(),
            api_key: String::new(),
            model: "llama3".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            client.is_configured(),
            "Ollama should be configured without an API key"
        );
    }

    #[test]
    fn test_is_configured_with_valid_api_key() {
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: "sk-ant-test-key-12345".to_string(),
            model: "claude-haiku-4-5-20251001".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            client.is_configured(),
            "Anthropic with API key should be configured"
        );
    }

    #[test]
    fn test_is_configured_unknown_provider() {
        let provider = LLMProvider {
            provider: "unknown_provider".to_string(),
            api_key: "some-key".to_string(),
            model: "some-model".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            !client.is_configured(),
            "Unknown provider should not be configured"
        );
    }

    // ========================================================================
    // Cost estimation edge cases
    // ========================================================================

    #[test]
    fn test_cost_estimation_unknown_provider() {
        let provider = LLMProvider {
            provider: "unknown".to_string(),
            api_key: String::new(),
            model: "whatever".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        let cost = client.estimate_cost_cents(100_000, 10_000);
        assert_eq!(cost, 0, "Unknown provider should have zero cost");
    }

    #[test]
    fn test_cost_estimation_zero_tokens() {
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: "test".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        let cost = client.estimate_cost_cents(0, 0);
        assert_eq!(cost, 0, "Zero tokens should cost zero");
    }

    #[test]
    fn test_cost_estimation_openai_models() {
        let provider = LLMProvider {
            provider: "openai".to_string(),
            api_key: "test".to_string(),
            model: "gpt-4o-mini".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        // gpt-4o-mini is the cheapest OpenAI model
        let cost = client.estimate_cost_cents(10_000, 1_000);
        assert!(cost < 1, "gpt-4o-mini should be very cheap for small usage");
    }

    // ========================================================================
    // sanitize_api_error
    // ========================================================================

    #[test]
    fn test_sanitize_api_error_truncates_long_text() {
        let long = "a".repeat(500);
        let result = sanitize_api_error(&long);
        assert!(result.len() <= 210, "Should truncate to ~200 chars + ...");
    }

    #[test]
    fn test_sanitize_api_error_redacts_sk_key() {
        // Fake key for testing sanitization (not a real credential)
        let text = concat!(
            r#"{"error": "Invalid API key: "#,
            "sk-ant-",
            "api03-",
            "abcdef",
            "1234567890",
            r#""}"#
        );
        let result = sanitize_api_error(text);
        assert!(
            result.contains("[REDACTED]"),
            "Should redact sk- prefixed key"
        );
        assert!(!result.contains("sk-ant"), "Should not contain the key");
    }

    #[test]
    fn test_sanitize_api_error_redacts_pk_key() {
        // Fake key for testing sanitization (not a real credential)
        let text = concat!(
            "Error with key ",
            "pk_live_",
            "abcdefghijklmnopqrstuvwxyz",
            "1234567890"
        );
        let result = sanitize_api_error(text);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("pk_live_"));
    }

    #[test]
    fn test_sanitize_api_error_redacts_long_alphanumeric() {
        let text = "token: AAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD is invalid";
        let result = sanitize_api_error(text);
        assert!(result.contains("[REDACTED]"));
    }

    #[test]
    fn test_sanitize_api_error_preserves_short_text() {
        let text = "rate limit exceeded";
        assert_eq!(sanitize_api_error(text), text);
    }
}
