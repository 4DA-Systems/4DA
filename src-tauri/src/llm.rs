// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

/// Structured output mode for LLM completions.
///
/// Controls how the LLM is constrained to produce structured (JSON) output.
/// Each provider translates this to its native mechanism.
#[derive(Debug, Clone)]
pub(crate) enum StructuredOutputMode {
    /// OpenAI/Ollama: `response_format: { type: "json_object" }` / `format: "json"`.
    /// Anthropic: schema instruction appended to system prompt.
    #[allow(dead_code)] // REMOVE BY 2026-07-01
    JsonSchema { schema: &'static str },
    /// llama-server only: GBNF grammar for constrained decoding.
    /// Falls back to `JsonSchema` for non-llama providers.
    Grammar {
        grammar: &'static str,
        schema: &'static str,
    },
}

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
        let timeout_secs: u64 = 120;
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
    // REMOVE BY 2026-08-01
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

        // Privacy gate: auto-heal disclosure flag for BYOK users.
        // Providing an API key IS consent — no separate acceptance needed.
        if !matches!(self.provider.provider.as_str(), "ollama" | "builtin") {
            if let Some(mut g) = crate::get_settings_manager().try_lock() {
                if !g.get().privacy.cloud_llm_disclosure_accepted
                    && !self.provider.api_key.is_empty()
                {
                    g.get_mut().privacy.cloud_llm_disclosure_accepted = true;
                    let _ = g.save();
                }
            }
        }

        let result = match self.provider.provider.as_str() {
            "anthropic" => self.complete_anthropic(system, messages.clone()).await,
            "openai" | "openai-compatible" | "builtin" => {
                self.complete_openai(system, messages.clone()).await
            }
            "ollama" => self.complete_ollama(system, messages.clone()).await,
            _ => return Err(format!("Unknown provider: {}", self.provider.provider).into()),
        };

        let response = match result {
            Ok(resp) => resp,
            Err(err) => {
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

    /// Send a completion request with structured output constraints.
    ///
    /// Identical to `complete()` but instructs the LLM to produce JSON
    /// conforming to the given schema/grammar. Provider-specific:
    /// - OpenAI/builtin: `response_format` + optional grammar
    /// - Anthropic: schema instruction appended to system prompt
    /// - Ollama: `format: "json"` parameter
    pub(crate) async fn complete_structured(
        &self,
        system: &str,
        messages: Vec<Message>,
        mode: &StructuredOutputMode,
    ) -> Result<LLMResponse> {
        if is_llm_limit_reached() {
            let (tokens_used, tokens_limit) = crate::state::get_llm_token_usage();
            let (cost_used, cost_limit) = crate::state::get_llm_cost_usage();
            warn!(
                target: "4da::llm",
                tokens_used, tokens_limit, cost_used_cents = cost_used, cost_limit_cents = cost_limit,
                "LLM call blocked — daily limit reached"
            );
            return Err(
                Self::format_limit_error(tokens_used, tokens_limit, cost_used, cost_limit).into(),
            );
        }

        if !matches!(self.provider.provider.as_str(), "ollama" | "builtin") {
            if let Some(mut g) = crate::get_settings_manager().try_lock() {
                if !g.get().privacy.cloud_llm_disclosure_accepted
                    && !self.provider.api_key.is_empty()
                {
                    g.get_mut().privacy.cloud_llm_disclosure_accepted = true;
                    let _ = g.save();
                }
            }
        }

        let result = match self.provider.provider.as_str() {
            "anthropic" => {
                self.complete_anthropic_structured(system, messages.clone(), mode)
                    .await
            }
            "openai" | "openai-compatible" | "builtin" => {
                self.complete_openai_structured(system, messages.clone(), mode)
                    .await
            }
            "ollama" => {
                self.complete_ollama_structured(system, messages.clone(), mode)
                    .await
            }
            _ => return Err(format!("Unknown provider: {}", self.provider.provider).into()),
        };

        let response = match result {
            Ok(resp) => resp,
            Err(err) => {
                crate::telemetry::record_error_async(
                    "llm",
                    &format!("{err}"),
                    Some(&self.provider.provider),
                );
                return Err(err);
            }
        };

        let total_tokens = response.input_tokens + response.output_tokens;
        if total_tokens > 0 {
            let tokens_ok = record_llm_tokens(total_tokens);
            let cost_cents =
                self.estimate_cost_cents(response.input_tokens, response.output_tokens);
            let cost_ok = record_llm_cost(cost_cents);
            if !tokens_ok || !cost_ok {
                debug!(
                    target: "4da::llm",
                    tokens = total_tokens, cost_cents,
                    "Limit exceeded after this call — future calls will be blocked"
                );
            }
            record_ai_usage(
                &self.provider.provider,
                &self.provider.model,
                "structured_completion",
                response.input_tokens,
                response.output_tokens,
            );
        }

        Ok(response)
    }

    /// Complete a request for translation — bypasses daily token limits.
    ///
    /// Translation is infrastructure (ingest-time cache warming, on-demand display),
    /// not user-initiated analysis. It must never be blocked by the daily budget.
    /// Usage is still tracked for visibility but doesn't trigger the hard cutoff.
    ///
    /// Includes retry with backoff: 1 retry after 2s on transient failures.
    pub async fn complete_for_translation(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse> {
        // First attempt — NO limit check (translation is infrastructure)
        let result = match self.provider.provider.as_str() {
            "anthropic" => self.complete_anthropic(system, messages.clone()).await,
            "openai" | "openai-compatible" | "builtin" => {
                self.complete_openai(system, messages.clone()).await
            }
            "ollama" => self.complete_ollama(system, messages.clone()).await,
            _ => return Err(format!("Unknown provider: {}", self.provider.provider).into()),
        };

        let response = match result {
            Ok(resp) => resp,
            Err(_err) => {
                debug!(target: "4da::i18n", error = %_err, "Translation LLM failed, retrying after 2s");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                match self.provider.provider.as_str() {
                    "anthropic" => self.complete_anthropic(system, messages).await?,
                    "openai" | "openai-compatible" | "builtin" => {
                        self.complete_openai(system, messages).await?
                    }
                    "ollama" => self.complete_ollama(system, messages).await?,
                    _ => return Err(format!("Unknown provider: {}", self.provider.provider).into()),
                }
            }
        };

        // Record usage for visibility (no limit enforcement)
        let total_tokens = response.input_tokens + response.output_tokens;
        if total_tokens > 0 {
            let _ = record_llm_tokens(total_tokens);
            let cost_cents =
                self.estimate_cost_cents(response.input_tokens, response.output_tokens);
            let _ = record_llm_cost(cost_cents);
            record_ai_usage(
                &self.provider.provider,
                &self.provider.model,
                "translation",
                response.input_tokens,
                response.output_tokens,
            );
        }

        Ok(response)
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
            // Trim defensively: a pasted key with a trailing newline/space is
            // sent verbatim and rejected by Anthropic as "invalid x-api-key",
            // which is indistinguishable from a genuinely bad key to the user.
            .header("x-api-key", self.provider.api_key.trim())
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

    /// OpenAI API (also used for openai-compatible and builtin providers)
    async fn complete_openai(&self, system: &str, messages: Vec<Message>) -> Result<LLMResponse> {
        let url = if self.provider.provider == "builtin" {
            let base = crate::llm_engine::sidecar_base_url().ok_or_else(|| {
                crate::error::FourDaError::Llm("Built-in LLM sidecar is not running".into())
            })?;
            format!("{base}/chat/completions")
        } else if self.provider.provider == "openai-compatible" {
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

        // Re-validate URL at use-time to prevent SSRF from tampered settings
        if !matches!(self.provider.provider.as_str(), "ollama" | "builtin") {
            crate::url_validation::validate_not_internal(&url)?;
        }

        // Qwen3 defaults to "thinking mode" which wastes tokens on reasoning_content
        // and leaves content empty. /no_think forces direct response.
        let system_content = if self.provider.provider == "builtin" {
            format!("/no_think\n{system}")
        } else {
            system.to_string()
        };

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": system_content
        })];

        for m in &messages {
            all_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let body = serde_json::json!({
            "model": self.provider.model,
            "max_tokens": 4096,
            "messages": all_messages
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.provider.api_key.trim()))
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

        let is_qwen3 = self.provider.model.to_lowercase().contains("qwen3");
        let system_content = if is_qwen3 {
            format!("/no_think\n{system}")
        } else {
            system.to_string()
        };

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": system_content
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

    // ========================================================================
    // Structured output variants (JSON mode / grammar-constrained)
    // ========================================================================

    async fn complete_anthropic_structured(
        &self,
        system: &str,
        messages: Vec<Message>,
        mode: &StructuredOutputMode,
    ) -> Result<LLMResponse> {
        let schema = match mode {
            StructuredOutputMode::JsonSchema { schema } => schema,
            StructuredOutputMode::Grammar { schema, .. } => schema,
        };
        let structured_system = format!(
            "{system}\n\nYou MUST respond with valid JSON matching this schema:\n{schema}\n\nRespond ONLY with the JSON object. No markdown, no explanation, no wrapping."
        );
        self.complete_anthropic(&structured_system, messages).await
    }

    async fn complete_openai_structured(
        &self,
        system: &str,
        messages: Vec<Message>,
        mode: &StructuredOutputMode,
    ) -> Result<LLMResponse> {
        let url = if self.provider.provider == "builtin" {
            let base = crate::llm_engine::sidecar_base_url().ok_or_else(|| {
                crate::error::FourDaError::Llm("Built-in LLM sidecar is not running".into())
            })?;
            format!("{base}/chat/completions")
        } else if self.provider.provider == "openai-compatible" {
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

        if !matches!(self.provider.provider.as_str(), "ollama" | "builtin") {
            crate::url_validation::validate_not_internal(&url)?;
        }

        let schema = match mode {
            StructuredOutputMode::JsonSchema { schema } => schema,
            StructuredOutputMode::Grammar { schema, .. } => schema,
        };

        let no_think_prefix = if self.provider.provider == "builtin" {
            "/no_think\n"
        } else {
            ""
        };
        let structured_system = format!(
            "{no_think_prefix}{system}\n\nRespond ONLY with valid JSON matching this schema:\n{schema}"
        );

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": structured_system
        })];
        for m in &messages {
            all_messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let mut body = serde_json::json!({
            "model": self.provider.model,
            "max_tokens": 4096,
            "messages": all_messages,
            "response_format": { "type": "json_object" }
        });

        // llama-server supports GBNF grammar for hard-constrained decoding
        if self.provider.provider == "builtin" {
            if let StructuredOutputMode::Grammar { grammar, .. } = mode {
                body["grammar"] = serde_json::Value::String(grammar.to_string());
            }
        }

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.provider.api_key.trim()))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("OpenAI structured request failed")?;

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
            .context("Failed to parse OpenAI structured response")?;

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

    async fn complete_ollama_structured(
        &self,
        system: &str,
        messages: Vec<Message>,
        mode: &StructuredOutputMode,
    ) -> Result<LLMResponse> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let url = format!("{base_url}/api/chat");

        let schema = match mode {
            StructuredOutputMode::JsonSchema { schema } => schema,
            StructuredOutputMode::Grammar { schema, .. } => schema,
        };

        let is_qwen3 = self.provider.model.to_lowercase().contains("qwen3");
        let no_think_prefix = if is_qwen3 { "/no_think\n" } else { "" };
        let structured_system = format!(
            "{no_think_prefix}{system}\n\nRespond ONLY with valid JSON matching this schema:\n{schema}"
        );

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": structured_system
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
            "stream": false,
            "format": "json"
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
                } else {
                    format!("Ollama structured request failed: {e}")
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("Ollama error {}: {}", status, sanitize_api_error(&text)).into());
        }

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Ollama structured response")?;

        let content = data["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

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
            "openai" | "openai-compatible" | "builtin" => {
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

        let response = match result {
            Ok(resp) => resp,
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

        let lang = crate::i18n::get_user_language();
        parts.push(crate::i18n::t("errors:errorMsg.limitAdjust", &lang, &[]));
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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
            embedding_model: String::new(),
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

    // ========================================================================
    // is_configured — openai-compatible provider
    // ========================================================================

    #[test]
    fn test_is_configured_openai_compatible_needs_key() {
        let provider = LLMProvider {
            provider: "openai-compatible".to_string(),
            api_key: String::new(),
            model: "mistral-large".to_string(),
            base_url: Some("https://api.mistral.ai/v1".to_string()),
            openai_api_key: String::new(),
            embedding_model: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            !client.is_configured(),
            "openai-compatible with empty key should not be configured"
        );
    }

    #[test]
    fn test_is_configured_openai_compatible_with_key() {
        let provider = LLMProvider {
            provider: "openai-compatible".to_string(),
            api_key: "test-key-12345".to_string(),
            model: "mistral-large".to_string(),
            base_url: Some("https://api.mistral.ai/v1".to_string()),
            openai_api_key: String::new(),
            embedding_model: String::new(),
        };
        let client = LLMClient::new(provider);
        assert!(
            client.is_configured(),
            "openai-compatible with API key should be configured"
        );
    }

    // ========================================================================
    // format_limit_error — pure formatting logic
    // ========================================================================

    #[test]
    fn test_format_limit_error_token_exceeded() {
        let msg = LLMClient::format_limit_error(150_000, 100_000, 0, 0);
        assert!(
            msg.contains("token limit exceeded"),
            "Should mention token limit: {msg}"
        );
        assert!(msg.contains("150000"), "Should include actual usage: {msg}");
    }

    #[test]
    fn test_format_limit_error_cost_exceeded() {
        let msg = LLMClient::format_limit_error(0, 0, 600, 500);
        assert!(
            msg.contains("cost limit exceeded"),
            "Should mention cost limit: {msg}"
        );
        assert!(
            msg.contains("$5.00"),
            "Should format cost in dollars: {msg}"
        );
    }

    #[test]
    fn test_format_limit_error_both_exceeded() {
        let msg = LLMClient::format_limit_error(200_000, 100_000, 1200, 1000);
        assert!(
            msg.contains("token limit exceeded"),
            "Should mention token limit: {msg}"
        );
        assert!(
            msg.contains("cost limit exceeded"),
            "Should also mention cost limit: {msg}"
        );
    }

    #[test]
    fn test_format_limit_error_fallback_when_no_limits() {
        // Edge case: called with limits that aren't actually exceeded
        let msg = LLMClient::format_limit_error(50, 100, 10, 500);
        // Neither condition triggers, so the fallback message should appear
        assert!(
            msg.contains("LLM limit reached"),
            "Should produce fallback message when no specific limit matched: {msg}"
        );
    }

    // ========================================================================
    // sanitize_api_error — additional edge cases
    // ========================================================================

    #[test]
    fn test_sanitize_preserves_normal_error_message() {
        let text = "Model overloaded, please retry in 30 seconds";
        let result = sanitize_api_error(text);
        assert_eq!(
            result, text,
            "Normal error messages should pass through unchanged"
        );
    }

    #[test]
    fn test_sanitize_handles_empty_string() {
        let result = sanitize_api_error("");
        assert_eq!(result, "", "Empty string should produce empty result");
    }
}
