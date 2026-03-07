//! LLM client module for 4DA
//!
//! Provides a unified interface for interacting with different LLM providers:
//! - Anthropic Claude (recommended for relevance judging)
//! - OpenAI GPT
//! - Ollama (local, free)

use crate::settings::LLMProvider;
use crate::state::{is_llm_limit_reached, record_llm_tokens};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

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
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Check if the client is configured
    #[allow(dead_code)] // Future: settings validation
    pub fn is_configured(&self) -> bool {
        match self.provider.provider.as_str() {
            "anthropic" | "openai" => !self.provider.api_key.is_empty(),
            "ollama" => true, // Ollama doesn't need an API key
            _ => false,
        }
    }

    /// Send a completion request.
    /// Enforces the daily token limit — returns an error if the budget is exhausted.
    pub async fn complete(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse, String> {
        // Hard cutoff: refuse to call the LLM if daily limit is already reached
        if is_llm_limit_reached() {
            let (used, limit) = crate::state::get_llm_token_usage();
            warn!(
                target: "4da::llm",
                used = used,
                limit = limit,
                "LLM call blocked — daily token limit reached"
            );
            return Err(format!(
                "Daily LLM token limit reached ({}/{} tokens). Resets at midnight.",
                used, limit
            ));
        }

        let response = match self.provider.provider.as_str() {
            "anthropic" => self.complete_anthropic(system, messages).await,
            "openai" => self.complete_openai(system, messages).await,
            "ollama" => self.complete_ollama(system, messages).await,
            _ => return Err(format!("Unknown provider: {}", self.provider.provider)),
        }?;

        // Record token usage (atomic, lock-free for the counter itself)
        let total_tokens = response.input_tokens + response.output_tokens;
        if total_tokens > 0 {
            let within_limit = record_llm_tokens(total_tokens);
            if !within_limit {
                debug!(
                    target: "4da::llm",
                    tokens = total_tokens,
                    "Token limit exceeded after this call — future calls will be blocked"
                );
            }
        }

        Ok(response)
    }

    /// Anthropic Claude API
    async fn complete_anthropic(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse, String> {
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
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("Anthropic API error {}: {}", status, text));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

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

    /// OpenAI API
    async fn complete_openai(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse, String> {
        let url = self
            .provider
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1/chat/completions");

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
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("OpenAI API error {}: {}", status, text));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

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
    async fn complete_ollama(
        &self,
        system: &str,
        messages: Vec<Message>,
    ) -> Result<LLMResponse, String> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let url = format!("{}/api/chat", base_url);

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
                        "Cannot connect to Ollama at {}. Make sure Ollama is running (ollama serve).",
                        base_url
                    )
                } else if msg.contains("timed out") || msg.contains("timeout") {
                    format!(
                        "Ollama request timed out. Model '{}' may be loading or the prompt is too large. Try again.",
                        self.provider.model
                    )
                } else {
                    format!("Ollama request failed: {}", e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || text.contains("not found") {
                return Err(format!(
                    "Model '{}' not found in Ollama. Run: ollama pull {}",
                    self.provider.model, self.provider.model
                ));
            }
            if text.contains("out of memory") || text.contains("OOM") || text.contains("CUDA") {
                return Err(format!(
                    "Not enough GPU memory for '{}'. Try a smaller model.",
                    self.provider.model
                ));
            }
            return Err(format!("Ollama error {}: {}", status, text));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

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

    /// Estimate cost in cents based on provider and tokens
    pub fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        match self.provider.provider.as_str() {
            "anthropic" => {
                // Prices per 1M tokens (as of 2024)
                let (input_price, output_price) = match self.provider.model.as_str() {
                    m if m.contains("opus") => (15.0, 75.0),
                    m if m.contains("sonnet") => (3.0, 15.0),
                    m if m.contains("haiku") => (0.25, 1.25),
                    _ => (3.0, 15.0), // Default to Sonnet pricing
                };
                let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price * 100.0;
                let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price * 100.0;
                (input_cost + output_cost) as u64
            }
            "openai" => {
                // GPT-4o-mini pricing
                let (input_price, output_price) = match self.provider.model.as_str() {
                    m if m.contains("gpt-4o-mini") => (0.15, 0.60),
                    m if m.contains("gpt-4o") => (2.5, 10.0),
                    m if m.contains("gpt-4") => (30.0, 60.0),
                    m if m.contains("gpt-3.5") => (0.5, 1.5),
                    _ => (0.15, 0.60), // Default to gpt-4o-mini
                };
                let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price * 100.0;
                let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price * 100.0;
                (input_cost + output_cost) as u64
            }
            "ollama" => 0, // Free!
            _ => 0,
        }
    }
}

// ============================================================================
// Ollama Utilities
// ============================================================================

/// List available models from Ollama API
#[allow(dead_code)] // Utility function for future use
pub async fn list_ollama_models(base_url: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/api/tags", base_url);
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned error: {}", response.status()));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

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
// Relevance Judge
// ============================================================================

/// The relevance judge uses an LLM to determine true relevance
pub struct RelevanceJudge {
    client: LLMClient,
}

impl RelevanceJudge {
    pub fn new(provider: LLMProvider) -> Self {
        Self {
            client: LLMClient::new(provider),
        }
    }

    /// Judge relevance of multiple items against user context.
    /// Uses a 1-5 scoring rubric and sends real article content.
    pub async fn judge_batch(
        &self,
        context_summary: &str,
        items: Vec<(String, String, String)>, // (id, title, content_snippet)
    ) -> Result<(Vec<RelevanceJudgment>, u64, u64), String> {
        if items.is_empty() {
            return Ok((vec![], 0, 0));
        }

        let system_prompt = r#"You are a relevance judge for a developer intelligence tool. Rate each article's genuine usefulness to THIS specific developer — not whether it mentions their tech, but whether they'd actually benefit from reading it.

## Scoring Rubric (be strict — most items should score 1-2)
5 = MUST-READ: Security alert for their dependency, breaking change they must act on, directly solves a problem they're currently working on
4 = HIGH VALUE: Advanced technique for their core tech, important release for a dependency they use daily, architectural pattern directly applicable to their project
3 = WORTH KNOWING: Relevant ecosystem news, useful tool that fits their exact stack, technical deep-dive in their specific domain
2 = MARGINAL: Mentions their tech but isn't actionable, generic advice, tangentially related
1 = NOISE: Wrong domain, competing tech focused, beginner content for tech they know well, self-promotional "I built X", career/hiring, academic papers outside their domain

## Critical Rules
- "Mentions Rust" does NOT mean relevant. A Supabase SDK in Rust is irrelevant if they don't use Supabase. Judge the TOPIC, not the language.
- "I built X" and "Show HN" posts are almost always score 1-2 unless X is directly applicable to their specific project.
- Content about competing/alternative technologies they've chosen against = score 1.
- Tutorials for technologies they already use expertly = score 1-2.
- Score >= 3 should mean: "This developer would thank me for showing them this."

Output JSON array (one per article):
[{"id": N, "score": N, "reason": "one sentence"}]"#;

        let items_text = items
            .iter()
            .enumerate()
            .map(|(i, (id, title, content))| {
                let snippet = if content.len() > 400 {
                    let truncated: String = content.chars().take(400).collect();
                    truncated
                } else {
                    content.clone()
                };
                format!("{}. [ID: {}] \"{}\"\n   {}", i + 1, id, title, snippet)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let user_message = format!(
            "## Developer Context\n{}\n\n## Articles to Judge\n{}\n\nRate each article 1-5 per the rubric. Output JSON array:",
            context_summary,
            items_text
        );

        let response = self
            .client
            .complete(
                system_prompt,
                vec![Message {
                    role: "user".to_string(),
                    content: user_message,
                }],
            )
            .await?;

        // Parse the score-based JSON response
        let judgments = self.parse_judgments(&response.content, &items)?;

        Ok((judgments, response.input_tokens, response.output_tokens))
    }

    fn parse_judgments(
        &self,
        response: &str,
        items: &[(String, String, String)],
    ) -> Result<Vec<RelevanceJudgment>, String> {
        // Try to extract JSON from the response
        let json_str = if let Some(start) = response.find('[') {
            if let Some(end) = response.rfind(']') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|e| {
            format!(
                "Failed to parse LLM response as JSON: {}. Response: {}",
                e, response
            )
        })?;

        let mut judgments = Vec::new();

        for value in parsed {
            // Handle ID as string or number
            let id = value["id"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| value["id"].as_u64().map(|n| n.to_string()))
                .or_else(|| value["id"].as_i64().map(|n| n.to_string()))
                .unwrap_or_default();

            // New: parse score (1-5) instead of relevant boolean
            let score = value["score"]
                .as_f64()
                .or_else(|| value["score"].as_i64().map(|n| n as f64))
                .or_else(|| value["score"].as_str().and_then(|s| s.parse::<f64>().ok()))
                .unwrap_or(1.0)
                .clamp(1.0, 5.0) as f32;

            // Map score to relevant/confidence
            let relevant = score >= 3.0;
            let confidence = score / 5.0;

            // Support both "reason" and "reasoning" keys
            let reasoning = value["reason"]
                .as_str()
                .or_else(|| value["reasoning"].as_str())
                .unwrap_or("")
                .to_string();

            // Legacy support: if "relevant" field exists and "score" doesn't, use old format
            let (relevant, confidence) = if value.get("score").is_none() {
                if let Some(rel) = value["relevant"].as_bool() {
                    let conf = value["confidence"]
                        .as_f64()
                        .unwrap_or(if rel { 0.6 } else { 0.2 })
                        as f32;
                    (rel, conf)
                } else {
                    (relevant, confidence)
                }
            } else {
                (relevant, confidence)
            };

            let key_connections: Vec<String> = value["key_connections"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            // Debug log first few judgments
            if judgments.len() < 3 {
                debug!(
                    target: "4da::llm",
                    id = %id,
                    score = score,
                    relevant = %relevant,
                    confidence = confidence,
                    reason = %&reasoning[..reasoning.len().min(50)],
                    "Parsed judgment"
                );
            }

            judgments.push(RelevanceJudgment {
                item_id: id,
                relevant,
                confidence,
                reasoning,
                key_connections,
            });
        }

        // Ensure we have judgments for all items (in case LLM missed some)
        for (id, _, _) in items {
            if !judgments.iter().any(|j| j.item_id == *id) {
                judgments.push(RelevanceJudgment {
                    item_id: id.clone(),
                    relevant: false,
                    confidence: 0.0,
                    reasoning: "No judgment provided by LLM".to_string(),
                    key_connections: vec![],
                });
            }
        }

        Ok(judgments)
    }

    /// Estimate cost for judging items
    pub fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        self.client.estimate_cost_cents(input_tokens, output_tokens)
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
            model: "claude-3-haiku-20240307".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);

        // 10k input, 1k output
        let cost = client.estimate_cost_cents(10_000, 1_000);
        // Haiku: $0.25/1M input, $1.25/1M output
        // 10k input = $0.0025, 1k output = $0.00125 = ~0.4 cents
        assert!(cost < 1); // Should be less than 1 cent
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
}
