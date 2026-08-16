// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Streaming LLM completion for progressive token delivery.
//!
//! Extracted from `llm.rs` to keep file sizes within limits.
//! Supports SSE (Anthropic, OpenAI) and NDJSON (Ollama) streaming formats.

// UTF-8 safety gate (see the `clippy::string_slice` note in Cargo.toml).
// Byte-slicing a `str` panics on any index that is not a char boundary. This
// module was hardened against that class, so the lint is denied here to keep it
// at zero: every future slice must carry an explicit char-boundary proof
// (`floor_char_boundary`, an offset from `find` of an ASCII needle, or one of
// the `utils::text` helpers) or an `#[allow]` that states why it is safe.
#![deny(clippy::string_slice)]

use crate::error::{Result, ResultExt};
use crate::llm::{sanitize_api_error, LLMResponse, Message};
use crate::settings::LLMProvider;
use futures::StreamExt;
use tracing::debug;

// ============================================================================
// Chunk reassembly
// ============================================================================

/// Accumulates raw network bytes and yields complete lines.
///
/// All three streaming paths used to do `buffer.push_str(&String::
/// from_utf8_lossy(&bytes))` **per network chunk**. TCP does not respect
/// character boundaries: a multi-byte char whose bytes land in two chunks is
/// decoded as an incomplete sequence in each half, so `from_utf8_lossy`
/// replaces BOTH halves with U+FFFD and the character is gone before any parser
/// sees it. Not a panic — silent corruption of user-visible LLM output, and it
/// only shows up on non-English text and emoji, which is exactly the content
/// least likely to be in anyone's test fixture.
///
/// Decoding is therefore deferred to a line boundary, where a
/// well-formed stream always has whole characters. `from_utf8_lossy` is still
/// the decoder for the completed line — a genuinely malformed line degrades to
/// U+FFFD rather than dropping the line.
#[derive(Default)]
pub(crate) struct StreamLineBuffer {
    buf: Vec<u8>,
}

impl StreamLineBuffer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Append one raw network chunk. No decoding happens here.
    pub(crate) fn push_chunk(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Pop the next complete line (up to and including `\n`), decoded.
    /// Returns `None` while the buffer holds no newline — the partial line
    /// stays buffered until the chunk carrying its terminator arrives.
    pub(crate) fn next_line(&mut self) -> Option<String> {
        let newline = self.buf.iter().position(|b| *b == b'\n')?;
        let line: Vec<u8> = self.buf.drain(..=newline).collect();
        // Exclude the trailing '\n' from the decoded line.
        Some(String::from_utf8_lossy(&line[..newline]).into_owned())
    }
}

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
            .and_then(serde_json::Value::as_u64);
    }
    None
}

/// Extract output token count from Anthropic message_delta event.
pub(crate) fn parse_anthropic_output_tokens(data: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    if v.get("type")?.as_str()? == "message_delta" {
        return v
            .pointer("/usage/output_tokens")
            .and_then(serde_json::Value::as_u64);
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

    let done = v
        .get("done")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let token = if done {
        None
    } else {
        v.pointer("/message/content")
            .and_then(|c| c.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
    };

    let input_tokens = v
        .get("prompt_eval_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = v
        .get("eval_count")
        .and_then(serde_json::Value::as_u64)
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
) -> Result<LLMResponse>
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
        .header("x-api-key", provider.api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Anthropic streaming request failed")?;

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

    let mut stream = response.bytes_stream();
    let mut lines = StreamLineBuffer::new();
    let mut full_text = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.context("Stream read error")?;
        lines.push_chunk(&bytes);

        // Process complete lines
        while let Some(raw) = lines.next_line() {
            let line = raw.trim();

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
) -> Result<LLMResponse>
where
    F: Fn(&str) + Send + 'static,
{
    let url = if provider.provider == "openai-compatible" {
        let base = provider
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
        provider
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

    let mut body = serde_json::json!({
        "model": provider.model,
        "max_tokens": 4096,
        "stream": true,
        "messages": all_messages
    });
    crate::llm::apply_openai_retention(&mut body, &provider.provider);

    let response = client
        .post(url)
        .header(
            "Authorization",
            format!("Bearer {}", provider.api_key.trim()),
        )
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("OpenAI streaming request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, sanitize_api_error(&text)).into());
    }

    let mut stream = response.bytes_stream();
    let mut lines = StreamLineBuffer::new();
    let mut full_text = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.context("Stream read error")?;
        lines.push_chunk(&bytes);

        while let Some(raw) = lines.next_line() {
            let line = raw.trim();

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
) -> Result<LLMResponse>
where
    F: Fn(&str) + Send + 'static,
{
    let base_url = provider
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
                    "Cannot connect to Ollama at {base_url}. Make sure Ollama is running (ollama serve)."
                )
            } else {
                format!("Ollama streaming request failed: {e}")
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama error {}: {}", status, sanitize_api_error(&text)).into());
    }

    let mut stream = response.bytes_stream();
    let mut lines = StreamLineBuffer::new();
    let mut full_text = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.context("Stream read error")?;
        lines.push_chunk(&bytes);

        while let Some(raw) = lines.next_line() {
            let line = raw.trim();

            if line.is_empty() {
                continue;
            }

            let (token, done, in_tok, out_tok) = parse_ollama_ndjson(line);

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Chunk reassembly (StreamLineBuffer) ---

    /// Drain every line the buffer currently holds.
    fn drain(buf: &mut StreamLineBuffer) -> Vec<String> {
        std::iter::from_fn(|| buf.next_line()).collect()
    }

    /// THE corruption case: a multi-byte char split across two network chunks.
    ///
    /// The old code decoded each chunk independently with
    /// `String::from_utf8_lossy`, so both halves of the split char became
    /// U+FFFD and the character was destroyed before any parser ran. Byte
    /// boundaries in a TCP stream are arbitrary; character boundaries are not.
    #[test]
    fn multibyte_char_split_across_chunks_is_reassembled() {
        let line = "data: {\"text\":\"héllo 世界 🦀\"}\n";
        let bytes = line.as_bytes();

        // Split at every byte offset — most of them land mid-character.
        for split in 1..bytes.len() {
            let mut buf = StreamLineBuffer::new();
            buf.push_chunk(&bytes[..split]);
            // The newline is the final byte, so no split can complete a line.
            assert!(
                drain(&mut buf).is_empty(),
                "no line before the newline (split {split})"
            );
            buf.push_chunk(&bytes[split..]);
            let out = drain(&mut buf);
            assert_eq!(out.len(), 1, "one complete line (split {split})");
            assert_eq!(
                out[0],
                line.trim_end(),
                "char split at byte {split} must survive"
            );
            assert!(
                !out[0].contains('\u{FFFD}'),
                "no replacement char (split {split})"
            );
        }
    }

    /// One byte at a time — the pathological case for per-chunk decoding.
    #[test]
    fn byte_at_a_time_stream_reassembles() {
        let payload = "héllo\nsecond 世界 line\n🦀 third\n";
        let mut buf = StreamLineBuffer::new();
        let mut out = Vec::new();
        for b in payload.as_bytes() {
            buf.push_chunk(&[*b]);
            out.extend(drain(&mut buf));
        }
        assert_eq!(out, vec!["héllo", "second 世界 line", "🦀 third"]);
    }

    /// End-to-end through the parser a streaming path actually calls.
    #[test]
    fn ollama_token_survives_a_chunk_split_mid_char() {
        let line = "{\"message\":{\"content\":\"héllo 🦀\"},\"done\":false}\n";
        let bytes = line.as_bytes();
        // Split inside the 'é' (its 2 bytes straddle this offset).
        let split = line.find('é').expect("é present") + 1;
        let mut buf = StreamLineBuffer::new();
        buf.push_chunk(&bytes[..split]);
        buf.push_chunk(&bytes[split..]);
        let decoded = buf.next_line().expect("complete line");
        let (token, _, _, _) = parse_ollama_ndjson(&decoded);
        assert_eq!(token, Some("héllo 🦀".to_string()));
    }

    #[test]
    fn partial_line_without_newline_is_held_not_emitted() {
        let mut buf = StreamLineBuffer::new();
        buf.push_chunk(b"data: partial");
        assert!(buf.next_line().is_none());
        buf.push_chunk(b" rest\n");
        assert_eq!(buf.next_line(), Some("data: partial rest".to_string()));
        assert!(buf.next_line().is_none());
    }

    #[test]
    fn multiple_lines_in_one_chunk_all_emit() {
        let mut buf = StreamLineBuffer::new();
        buf.push_chunk("a\nb\n\nc\n".as_bytes());
        assert_eq!(drain(&mut buf), vec!["a", "b", "", "c"]);
    }

    #[test]
    fn crlf_terminated_lines_keep_the_cr_for_trim() {
        // SSE may use CRLF; callers `.trim()` the returned line.
        let mut buf = StreamLineBuffer::new();
        buf.push_chunk(b"data: x\r\n");
        assert_eq!(buf.next_line().as_deref().map(str::trim), Some("data: x"));
    }

    // --- Anthropic SSE parsing ---

    #[test]
    fn parse_anthropic_content_block_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        assert_eq!(parse_anthropic_sse_token(data), Some("Hello".to_string()));
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
        let data =
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#;
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
        let data =
            r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
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
        let line =
            r#"{"model":"llama3","message":{"role":"assistant","content":"Hi"},"done":false}"#;
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
