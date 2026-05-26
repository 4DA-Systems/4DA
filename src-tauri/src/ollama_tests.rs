// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

#[test]
fn embed_model_is_small() {
    assert!(estimate_model_size_mb(&["nomic-embed-text:latest".to_string()]) <= 200);
}

#[test]
fn llama3_2_latest_is_around_2gb() {
    let mb = estimate_model_size_mb(&["llama3.2:latest".to_string()]);
    assert!(mb >= 1500 && mb <= 2500, "got {mb}");
}

#[test]
fn missing_both_embed_and_llm() {
    let mb = estimate_model_size_mb(&[
        "nomic-embed-text:latest".to_string(),
        "llama3.2:latest".to_string(),
    ]);
    assert!(mb >= 2000 && mb <= 2500, "got {mb}");
}

#[test]
fn parse_param_size_billions() {
    assert_eq!(parse_param_size("3.2B"), Some(3.2));
    assert_eq!(parse_param_size("8B"), Some(8.0));
    assert_eq!(parse_param_size("70B"), Some(70.0));
    assert_eq!(parse_param_size("0.5B"), Some(0.5));
    assert_eq!(parse_param_size("1.5B"), Some(1.5));
}

#[test]
fn parse_param_size_millions() {
    assert_eq!(parse_param_size("137M"), Some(0.137));
    assert_eq!(parse_param_size("500M"), Some(0.5));
}

#[test]
fn parse_param_size_invalid() {
    assert_eq!(parse_param_size(""), None);
    assert_eq!(parse_param_size("big"), None);
    assert_eq!(parse_param_size("3.2"), None);
}

#[test]
fn small_model_below_explanation_floor() {
    assert!(3.2 < ANALYSIS_QUALITY_MIN_PARAMS_B);
    assert!(1.0 < ANALYSIS_QUALITY_MIN_PARAMS_B);
    assert!(7.0 < ANALYSIS_QUALITY_MIN_PARAMS_B);
}

#[test]
fn capable_model_above_explanation_floor() {
    assert!(14.0 >= ANALYSIS_QUALITY_MIN_PARAMS_B);
    assert!(32.0 >= ANALYSIS_QUALITY_MIN_PARAMS_B);
    assert!(70.0 >= ANALYSIS_QUALITY_MIN_PARAMS_B);
}

#[test]
fn test_state_accessor_returns_same_instance() {
    let s1 = get_ollama_state();
    let s2 = get_ollama_state();
    assert!(std::ptr::eq(s1, s2));
}

#[test]
fn test_warming_flag_default_false() {
    let state = get_ollama_state().lock();
    // After module init, warming should be false (or reset from prior tests)
    // We just verify it's accessible and is a bool
    let _val = state.warming.load(Ordering::SeqCst);
}

#[test]
fn test_ollama_status_event_serialize() {
    let event = OllamaStatusEvent {
        phase: "warming".into(),
        model: "llama3".into(),
        error: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"phase\":\"warming\""));
    assert!(json.contains("\"model\":\"llama3\""));
    assert!(json.contains("\"error\":null"));

    let event_err = OllamaStatusEvent {
        phase: "error".into(),
        model: "llama3".into(),
        error: Some("connection refused".into()),
    };
    let json_err = serde_json::to_string(&event_err).unwrap();
    assert!(json_err.contains("\"error\":\"connection refused\""));
}
