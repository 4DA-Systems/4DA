// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `llm_stream` — SSE/NDJSON parsing and chunk reassembly.
//!
//! Split out of `llm_stream.rs` (repo convention: `#[path = "*_tests.rs"]`)
//! because the fixtures pushed that file past the 700-line warn threshold. The
//! production code there is ~420 lines and is one coherent concern, so moving
//! the tests is the right cut, not splitting the streaming logic.

use super::*;

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
    let data =
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
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
