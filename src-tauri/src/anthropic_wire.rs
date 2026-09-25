// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Anthropic Messages API wire shape, shared by the blocking (`llm.rs`) and
//! streaming (`llm_stream.rs`) paths.
//!
//! Claude 5-generation models (Sonnet 5, Opus 5, Opus 5.5, Fable 5.x) THINK
//! when a request omits `thinking` — the API default flipped from "off" to
//! "adaptive". Measured 2026-09-25 against 4DA's own request shape:
//!
//! - Opus 5, one-line prompt: content blocks `[thinking, text]`, so the old
//!   `content[0].text` read returned nothing.
//! - Sonnet 5, a 4.7k-token digest prompt: 2,540 thinking tokens, stopped at
//!   `max_tokens` 4096 with no text at all, 45.6 s.
//! - Sonnet 5 with `output_config.effort = "low"`: the same digest in 17.6 s,
//!   1,387 output tokens, zero thinking, no `<thinking>` tags leaking into the
//!   text. Opus 5, Opus 5.5 and Fable 5.1 accept the same shape and answered a
//!   judge-style prompt with zero thinking.
//!
//! So thinking-by-default models get `effort: low` (Anthropic's documented
//! lever — `thinking: disabled` is rejected by Opus 5.5 and Fable, and on
//! Opus 5 it can leak internal tags into the answer), a larger `max_tokens`
//! because the cap covers thinking PLUS text, and every reply is read as the
//! concatenation of its text blocks with the stop reason checked. Earlier
//! models get exactly the request they always got.

use serde_json::{json, Value};

use crate::llm::Message;

/// `max_tokens` for models that answer without thinking (unchanged default).
const MAX_TOKENS_DEFAULT: u32 = 4096;

/// `max_tokens` for thinking-by-default models: the cap covers thinking AND
/// text, and Claude 5's tokenizer counts ~30% more tokens for the same text.
const MAX_TOKENS_THINKING_MODELS: u32 = 8192;

/// Model families in Anthropic's `claude-<family>-<major>…` naming.
const FAMILIES: [&str; 5] = ["haiku", "sonnet", "opus", "fable", "mythos"];

/// Major generation of a Claude model id: `claude-sonnet-5` → 5,
/// `claude-opus-5-5` → 5, `claude-haiku-4-5-20251001` → 4, and the older
/// version-first scheme `claude-3-5-sonnet-20241022` → 3. `None` for anything
/// that is not a recognisable Claude id.
fn generation(model: &str) -> Option<u32> {
    let lower = model.to_ascii_lowercase();
    let parts: Vec<&str> = lower.strip_prefix("claude-")?.split('-').collect();
    let family_at = parts.iter().position(|p| FAMILIES.contains(p))?;
    let version = if family_at == 0 {
        parts.get(1)
    } else {
        parts.first()
    }?;
    // A date suffix ("20241022") is never a generation.
    if version.len() > 2 {
        return None;
    }
    version.parse().ok()
}

/// True for Claude models that think when `thinking` is omitted: generation 5
/// and later, every family.
pub(crate) fn thinks_by_default(model: &str) -> bool {
    generation(model).is_some_and(|g| g >= 5)
}

/// The `max_tokens` a request for `model` should carry.
pub(crate) fn max_tokens_for(model: &str) -> u32 {
    if thinks_by_default(model) {
        MAX_TOKENS_THINKING_MODELS
    } else {
        MAX_TOKENS_DEFAULT
    }
}

/// Build a Messages API request body.
pub(crate) fn request_body(model: &str, system: &str, messages: &[Message], stream: bool) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens_for(model),
        "system": system,
        "messages": messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    });
    if stream {
        body["stream"] = json!(true);
    }
    if thinks_by_default(model) {
        body["output_config"] = json!({ "effort": "low" });
    }
    body
}

/// A parsed, non-streaming Messages API reply.
#[derive(Debug)]
pub(crate) struct AnthropicReply {
    pub text: String,
    pub stop_reason: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl AnthropicReply {
    /// The model hit `max_tokens` mid-answer: the text is usable but cut off.
    pub(crate) fn truncated(&self) -> bool {
        self.stop_reason.as_deref() == Some("max_tokens")
    }
}

/// Parse a Messages API response: the answer is the concatenation of every
/// `text` block (thinking blocks come first on Claude 5 models and carry no
/// answer), and a reply that is not an answer is an error, never "".
pub(crate) fn parse_reply(data: &Value) -> Result<AnthropicReply, String> {
    let text: String = data["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect()
        })
        .unwrap_or_default();
    let stop_reason = data["stop_reason"].as_str().map(str::to_string);
    check_stop(stop_reason.as_deref(), &text)?;
    Ok(AnthropicReply {
        text,
        stop_reason,
        input_tokens: data["usage"]["input_tokens"].as_u64().unwrap_or(0),
        output_tokens: data["usage"]["output_tokens"].as_u64().unwrap_or(0),
    })
}

/// Reject replies that are not answers. A refusal is an abstention (callers
/// must not treat it as "nothing relevant"), and a reply that spent its whole
/// `max_tokens` before writing any text has nothing to parse.
pub(crate) fn check_stop(stop_reason: Option<&str>, text: &str) -> Result<(), String> {
    match stop_reason {
        Some("refusal") => {
            Err("Anthropic declined to answer this request (stop_reason: refusal)".to_string())
        }
        Some("max_tokens") if text.trim().is_empty() => Err(
            "Anthropic reply hit max_tokens before writing any text (the budget went to thinking)"
                .to_string(),
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(content: &str) -> Message {
        Message {
            role: "user".to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn generation_reads_both_naming_schemes() {
        assert_eq!(generation("claude-sonnet-5"), Some(5));
        assert_eq!(generation("claude-opus-5-5"), Some(5));
        assert_eq!(generation("claude-fable-5-1"), Some(5));
        assert_eq!(generation("claude-sonnet-5-20260801"), Some(5));
        assert_eq!(generation("claude-haiku-4-5-20251001"), Some(4));
        assert_eq!(generation("claude-sonnet-4-6"), Some(4));
        assert_eq!(generation("claude-3-5-sonnet-20241022"), Some(3));
        assert_eq!(generation("claude-3-haiku"), Some(3));
        assert_eq!(generation("gpt-5"), None);
        assert_eq!(generation("claude-sonnet-20241022"), None);
    }

    #[test]
    fn only_generation_five_and_later_thinks_by_default() {
        for m in [
            "claude-sonnet-5",
            "claude-opus-5",
            "claude-opus-5-5",
            "claude-fable-5-1",
        ] {
            assert!(thinks_by_default(m), "{m}");
        }
        for m in [
            "claude-haiku-4-5",
            "claude-sonnet-4-6",
            "claude-opus-4-6",
            "claude-3-haiku",
        ] {
            assert!(!thinks_by_default(m), "{m}");
        }
    }

    #[test]
    fn legacy_models_get_the_unchanged_request() {
        let body = request_body("claude-sonnet-4-6", "sys", &[msg("hi")], false);
        assert_eq!(body["max_tokens"], 4096);
        assert!(body.get("output_config").is_none());
        assert!(body.get("thinking").is_none());
        assert!(body.get("stream").is_none());
    }

    #[test]
    fn claude_five_gets_low_effort_and_headroom_and_never_a_thinking_param() {
        let body = request_body("claude-sonnet-5", "sys", &[msg("hi")], true);
        assert_eq!(body["max_tokens"], 8192);
        assert_eq!(body["output_config"]["effort"], "low");
        // `thinking: disabled` is a 400 on Opus 5.5 and Fable — never sent.
        assert!(body.get("thinking").is_none());
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["content"], "hi");
    }

    #[test]
    fn reply_text_skips_a_leading_thinking_block() {
        let data = json!({
            "content": [
                {"type": "thinking", "thinking": "", "signature": "x"},
                {"type": "text", "text": "[{\"id\":1,"},
                {"type": "text", "text": "\"score\":0.9}]"}
            ],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 65, "output_tokens": 109}
        });
        let reply = parse_reply(&data).expect("an answer");
        assert_eq!(reply.text, "[{\"id\":1,\"score\":0.9}]");
        assert_eq!(reply.input_tokens, 65);
        assert_eq!(reply.output_tokens, 109);
        assert!(!reply.truncated());
    }

    #[test]
    fn a_refusal_is_an_error_not_an_empty_answer() {
        let data = json!({"content": [], "stop_reason": "refusal", "usage": {}});
        assert!(parse_reply(&data).unwrap_err().contains("refusal"));
    }

    #[test]
    fn thinking_that_exhausts_max_tokens_is_an_error() {
        let data = json!({
            "content": [{"type": "thinking", "thinking": ""}],
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 4710, "output_tokens": 4096}
        });
        assert!(parse_reply(&data).unwrap_err().contains("max_tokens"));
    }

    #[test]
    fn a_truncated_answer_is_kept_and_flagged() {
        let data = json!({
            "content": [{"type": "text", "text": "## Action Required\n- partial"}],
            "stop_reason": "max_tokens",
            "usage": {}
        });
        let reply = parse_reply(&data).expect("partial text is still text");
        assert!(reply.truncated());
    }
}
