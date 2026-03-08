//! Streaming LLM completion for progressive token delivery.
//!
//! Extracted from `llm.rs` to keep file sizes within limits.
//! Supports SSE (Anthropic, OpenAI) and NDJSON (Ollama) streaming formats.

use crate::llm::{LLMResponse, Message};
use crate::settings::LLMProvider;
use futures::StreamExt;
use tracing::{debug, warn};

// ============================================================================
// SSE / NDJSON Parsing Helpers (pub for testing)
// ============================================================================

/// Extract token text from an Anthropic SSE data line.
/// Returns `Some(token)` for content_block_delta events, `None` otherwise.
pub(crate) fn parse_anthropic_sse_token(data: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    if v.get("type")?.as_str()? == "content_block_delta" {
        let delta = v.get("delta")?;
        if delta.get("type")?.as_str()? == "text_delta" {
            return delta.get("text")?.as_str().map(String::from);
        }
    }
    None
}

/// Extract input token count from Anthropic message_start event.
pub(crate) fn parse_anthropic_input_tokens(data: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    if v.get("type")?.as_str()? == "message_start" {
        return v
            .pointer("/message/usage/input_tokens")
            .and_then(|t| t.as_u64());
    }
    None
}

/// Extract output token count from Anthropic message_delta event.
pub(crate) fn parse_anthropic_output_tokens(data: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    if v.get("type")?.as_str()? == "message_delta" {
        return v.pointer("/usage/output_tokens").and_then(|t| t.as_u64());
    }
    None
}

/// Extract token text from an OpenAI SSE data line.
pub(crate) fn parse_openai_sse_token(data: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    v.pointer("/choices/0/delta/content")
        .and_then(|c| c.as_str())
        .map(String::from)
}

/// Parse an Ollama NDJSON line. Returns `(Option<token>, done, input_tokens, output_tokens)`.
pub(crate) fn parse_ollama_ndjson(line: &str) -> (Option<String>, bool, u64, u64) {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return (None, false, 0, 0),
    };

    let done = v.get("done").and_then(|d| d.as_bool()).unwrap_or(false);

    let token = if !done {
        v.pointer("/message/content")
            .and_then(|c| c.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
    } else {
        None
    };

    let input_tokens = v
        .get("prompt_eval_count")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let output_tokens = v
        .get("eval_count")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    (token, done, input_tokens, output_tokens)
}

// ============================================================================
// Streaming Provider Implementations
// ============================================================================

/// Stream completion from Anthropic's Messages API (SSE).
pub(crate) async fn stream_anthropic<F>(
    client: &reqwest::Client,
    provider: &LLMProvider,
    system: &str,
    messages: Vec<Message>,
    on_token: F,
) -> Result<LLMResponse, String>
where
    F: Fn(&str) + Send + 'static,
{
    let url = "https://api.anthropic.com/v1/messages";

    let body = serde_json::json!({
        "model": provider.model,
        "max_tokens": 4096,
        "stream": true,
        "system": system,
        "messages": messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        }).collect::<Vec<_>>()
    });

    let response = client
        .post(url)
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic streaming request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error {}: {}", status, text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_text = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream read error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }

                // Try extracting a token
                if let Some(token) = parse_anthropic_sse_token(data) {
                    full_text.push_str(&token);
                    on_token(&token);
                }

                // Try extracting input tokens from message_start
                if let Some(t) = parse_anthropic_input_tokens(data) {
                    input_tokens = t;
                }

                // Try extracting output tokens from message_delta
                if let Some(t) = parse_anthropic_output_tokens(data) {
                    output_tokens = t;
                }
            }
        }
    }

    debug!(
        target: "4da::llm",
        input_tokens = input_tokens,
        output_tokens = output_tokens,
        len = full_text.len(),
        "Anthropic streaming complete"
    );

    Ok(LLMResponse {
        content: full_text,
        input_tokens,
        output_tokens,
    })
}

/// Stream completion from OpenAI-compatible API (SSE).
pub(crate) async fn stream_openai<F>(
    client: &reqwest::Client,
    provider: &LLMProvider,
    system: &str,
    messages: Vec<Message>,
    on_token: F,
) -> Result<LLMResponse, String>
where
    F: Fn(&str) + Send + 'static,
{
    let url = provider
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
        "model": provider.model,
        "max_tokens": 4096,
        "stream": true,
        "messages": all_messages
    });

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI streaming request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_text = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream read error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Some(token) = parse_openai_sse_token(data) {
                    full_text.push_str(&token);
                    on_token(&token);
                }
            }
        }
    }

    // OpenAI streaming doesn't provide token counts; estimate from content
    let input_tokens = 0_u64; // Not available in streaming
    let output_tokens = (full_text.len() as u64) / 4; // ~4 chars per token estimate

    debug!(
        target: "4da::llm",
        output_tokens_est = output_tokens,
        len = full_text.len(),
        "OpenAI streaming complete"
    );

    Ok(LLMResponse {
        content: full_text,
        input_tokens,
        output_tokens,
    })
}

/// Stream completion from Ollama API (NDJSON).
pub(crate) async fn stream_ollama<F>(
    client: &reqwest::Client,
    provider: &LLMProvider,
    system: &str,
    messages: Vec<Message>,
    on_token: F,
) -> Result<LLMResponse, String>
where
    F: Fn(&str) + Send + 'static,
{
    let base_url = provider
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
        "model": provider.model,
        "messages": all_messages,
        "stream": true
    });

    let response = client
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
            } else {
                format!("Ollama streaming request failed: {}", e)
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama error {}: {}", status, text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_text = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream read error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let (token, done, in_tok, out_tok) = parse_ollama_ndjson(&line);

            if let Some(t) = token {
                full_text.push_str(&t);
                on_token(&t);
            }

            if done {
                input_tokens = in_tok;
                output_tokens = out_tok;
            }
        }
    }

    debug!(
        target: "4da::llm",
        input_tokens = input_tokens,
        output_tokens = output_tokens,
        len = full_text.len(),
        "Ollama streaming complete"
    );

    Ok(LLMResponse {
        content: full_text,
        input_tokens,
        output_tokens,
    })
}

/// Streaming fallback to local Ollama (when primary provider fails).
pub(crate) async fn stream_ollama_fallback<F>(
    system: &str,
    messages: Vec<Message>,
    on_token: F,
) -> Result<LLMResponse, String>
where
    F: Fn(&str) + Send + 'static,
{
    let fallback_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let fallback_provider = LLMProvider {
        provider: "ollama".to_string(),
        api_key: String::new(),
        model: "llama3".to_string(),
        base_url: Some("http://localhost:11434".to_string()),
        openai_api_key: String::new(),
    };

    warn!(
        target: "4da::llm",
        model = "llama3",
        "Falling back to local Ollama for streaming"
    );

    stream_ollama(&fallback_client, &fallback_provider, system, messages, on_token).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Anthropic SSE parsing ---

    #[test]
    fn parse_anthropic_content_block_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        assert_eq!(
            parse_anthropic_sse_token(data),
            Some("Hello".to_string())
        );
    }

    #[test]
    fn parse_anthropic_message_start_input_tokens() {
        let data = r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-3-haiku","usage":{"input_tokens":42}}}"#;
        assert_eq!(parse_anthropic_input_tokens(data), Some(42));
    }

    #[test]
    fn parse_anthropic_message_delta_output_tokens() {
        let data = r#"{"type":"message_delta","usage":{"output_tokens":128}}"#;
        assert_eq!(parse_anthropic_output_tokens(data), Some(128));
    }

    #[test]
    fn parse_anthropic_ignores_non_delta_events() {
        let data = r#"{"type":"message_stop"}"#;
        assert_eq!(parse_anthropic_sse_token(data), None);
    }

    #[test]
    fn parse_anthropic_ignores_ping() {
        let data = r#"{"type":"ping"}"#;
        assert_eq!(parse_anthropic_sse_token(data), None);
        assert_eq!(parse_anthropic_input_tokens(data), None);
        assert_eq!(parse_anthropic_output_tokens(data), None);
    }

    #[test]
    fn parse_anthropic_content_block_start_no_token() {
        let data = r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#;
        assert_eq!(parse_anthropic_sse_token(data), None);
    }

    #[test]
    fn parse_anthropic_handles_special_chars() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello \"world\" <&>"}}"#;
        assert_eq!(
            parse_anthropic_sse_token(data),
            Some("Hello \"world\" <&>".to_string())
        );
    }

    // --- OpenAI SSE parsing ---

    #[test]
    fn parse_openai_delta_content() {
        let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#;
        assert_eq!(parse_openai_sse_token(data), Some("Hi".to_string()));
    }

    #[test]
    fn parse_openai_empty_delta() {
        let data = r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        assert_eq!(parse_openai_sse_token(data), None);
    }

    #[test]
    fn parse_openai_role_delta_no_content() {
        let data = r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        assert_eq!(parse_openai_sse_token(data), None);
    }

    #[test]
    fn parse_openai_invalid_json() {
        assert_eq!(parse_openai_sse_token("not json"), None);
    }

    // --- Ollama NDJSON parsing ---

    #[test]
    fn parse_ollama_token_line() {
        let line = r#"{"model":"llama3","message":{"role":"assistant","content":"Hi"},"done":false}"#;
        let (token, done, in_t, out_t) = parse_ollama_ndjson(line);
        assert_eq!(token, Some("Hi".to_string()));
        assert!(!done);
        assert_eq!(in_t, 0);
        assert_eq!(out_t, 0);
    }

    #[test]
    fn parse_ollama_done_line() {
        let line = r#"{"model":"llama3","message":{"role":"assistant","content":""},"done":true,"prompt_eval_count":50,"eval_count":100}"#;
        let (token, done, in_t, out_t) = parse_ollama_ndjson(line);
        assert_eq!(token, None);
        assert!(done);
        assert_eq!(in_t, 50);
        assert_eq!(out_t, 100);
    }

    #[test]
    fn parse_ollama_invalid_json() {
        let (token, done, in_t, out_t) = parse_ollama_ndjson("broken{json");
        assert_eq!(token, None);
        assert!(!done);
        assert_eq!(in_t, 0);
        assert_eq!(out_t, 0);
    }

    #[test]
    fn parse_ollama_empty_content_skipped() {
        let line = r#"{"model":"llama3","message":{"role":"assistant","content":""},"done":false}"#;
        let (token, done, _, _) = parse_ollama_ndjson(line);
        assert_eq!(token, None);
        assert!(!done);
    }

    #[test]
    fn parse_ollama_done_without_counts() {
        let line = r#"{"done":true}"#;
        let (token, done, in_t, out_t) = parse_ollama_ndjson(line);
        assert_eq!(token, None);
        assert!(done);
        assert_eq!(in_t, 0);
        assert_eq!(out_t, 0);
    }

    // --- Edge cases ---

    #[test]
    fn parse_anthropic_sse_invalid_json() {
        assert_eq!(parse_anthropic_sse_token("not json at all"), None);
    }

    #[test]
    fn parse_anthropic_sse_wrong_type() {
        let data = r#"{"type":"content_block_delta","delta":{"type":"wrong_type","text":"nope"}}"#;
        assert_eq!(parse_anthropic_sse_token(data), None);
    }
}
