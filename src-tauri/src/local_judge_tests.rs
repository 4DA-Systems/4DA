use super::*;

fn inst(name: &str, size_mb: u64) -> Installed {
    Installed {
        name: name.to_string(),
        size_mb,
    }
}

/// The measured machine: 16 GB card with gemma4:26b, gemma4:12b and qwen3:14b
/// installed. The 26B model is larger than the card, but it was measured to
/// judge at about 4 s per item with partial offload there, so it is picked.
/// A 12 GB card was not measured with it and gets gemma4:12b.
#[test]
fn picks_the_best_measured_judge_that_fits() {
    let installed = [
        inst("qwen3:14b", 8_900),
        inst("gemma4:12b", 7_250),
        inst("gemma4:26b", 17_750),
        inst("llama3.2:latest", 1_900),
    ];
    assert_eq!(
        pick_judge(&installed, Some(16_376)).as_deref(),
        Some("gemma4:26b"),
        "measured with partial offload on this 16 GB card"
    );
    assert_eq!(
        pick_judge(&installed, Some(12_288)).as_deref(),
        Some("gemma4:12b"),
        "a 12 GB card was never measured with the 26B judge"
    );
    assert_eq!(
        pick_judge(&installed, Some(24_564)).as_deref(),
        Some("gemma4:26b"),
        "a 24 GB card holds the most accurate judge"
    );
}

#[test]
fn unmeasured_or_oversized_models_never_judge() {
    let installed = [inst("llama3.2:latest", 1_900), inst("qwen2.5:14b", 8_600)];
    assert_eq!(pick_judge(&installed, Some(24_564)), None);
    assert_eq!(
        pick_judge(&[inst("gemma4:12b", 7_250)], Some(8_192)),
        None,
        "an 8 GB card cannot hold the 12B judge plus headroom"
    );
    assert_eq!(
        pick_judge(&[inst("qwen3:14b", 20_000)], Some(16_376)),
        None,
        "the offload allowance is per measured model, not a general one"
    );
}

#[test]
fn unknown_gpu_size_keeps_the_cloud_judge() {
    assert_eq!(pick_judge(&[inst("gemma4:12b", 7_250)], None), None);
}

#[test]
fn a_slow_or_failed_call_opens_the_breaker_for_the_cool_off() {
    let now = Instant::now();
    let mut s = State {
        model: Some("gemma4:12b".into()),
        base_url: DEFAULT_OLLAMA_URL.into(),
        ..State::default()
    };
    assert!(route(&s, now).is_some());

    assert!(
        !note(&mut s, Duration::from_secs(2), true, now),
        "a warm call is fine"
    );
    assert!(route(&s, now).is_some());

    assert!(note(&mut s, SLOW_CALL + Duration::from_secs(1), true, now));
    assert!(route(&s, now).is_none(), "tripped: back to the cloud judge");
    assert!(
        route(&s, now + COOL_OFF + Duration::from_secs(1)).is_some(),
        "the cool-off ends"
    );

    let mut s2 = State {
        model: Some("gemma4:12b".into()),
        ..State::default()
    };
    assert!(
        note(&mut s2, Duration::from_secs(1), false, now),
        "an error trips it too"
    );
    assert!(route(&s2, now).is_none());
}

#[test]
fn only_the_three_judge_lanes_route_locally() {
    for p in ["ingest_judge", "verdict_drain", "rerank_judge"] {
        assert!(is_judge_purpose(Some(p)));
    }
    for p in [
        "digest",
        "blind_spots",
        "judge_benchmark",
        "connection_test",
    ] {
        assert!(!is_judge_purpose(Some(p)));
    }
    assert!(!is_judge_purpose(None));
}

#[tokio::test]
async fn an_unreachable_ollama_fails_the_load_rather_than_hanging() {
    let started = Instant::now();
    assert!(!load_model("http://127.0.0.1:9", "gemma4:26b").await);
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn a_local_rerank_pass_gets_the_longer_budget() {
    assert_eq!(
        budget_for(false),
        Duration::from_mins(2),
        "cloud keeps the hang guard"
    );
    assert!(
        budget_for(true) >= Duration::from_mins(4), // 48 items x 5 s
        "48 local items at the slowest measured 5 s per call must fit"
    );
}

#[test]
fn a_missed_probe_keeps_the_local_judge() {
    let installed = [inst("gemma4:26b", 17_750), inst("gemma4:12b", 7_250)];
    assert_eq!(
        next_route(Some("gemma4:26b".into()), None, Some(16_376)).as_deref(),
        Some("gemma4:26b"),
        "a busy Ollama missing the probe does not send judging to the cloud"
    );
    assert_eq!(
        next_route(None, None, Some(16_376)),
        None,
        "with no judge known yet, an unanswered probe routes nowhere"
    );
    assert_eq!(
        next_route(
            Some("gemma4:26b".into()),
            Some(&installed[1..]),
            Some(16_376)
        )
        .as_deref(),
        Some("gemma4:12b"),
        "an answered probe decides afresh"
    );
    assert_eq!(
        next_route(Some("gemma4:26b".into()), Some(&[]), Some(16_376)),
        None,
        "an answered probe with no judge installed stops local judging"
    );
}

fn llm(provider: &str, model: &str, key: &str) -> LLMProvider {
    let mut p = LLMProvider::default();
    p.provider = provider.to_string();
    p.model = model.to_string();
    p.api_key = key.to_string();
    p
}

/// Fresh-profile E2E 2026-10-09: a user with provider `none` had gemma4:12b
/// loaded into Ollama (8.4 GB VRAM, 30-minute keep-alive) by the rerank
/// warm-up, before the rerank lane said it was disabled.
#[test]
fn nothing_is_loaded_for_a_user_who_configured_no_ai() {
    let routed = Some(("gemma4:12b".to_string(), DEFAULT_OLLAMA_URL.to_string()));
    assert_eq!(warm_target(&llm("none", "", ""), routed.clone()), None);
    assert_eq!(warm_target(&llm("", "", ""), routed.clone()), None);
    assert_eq!(
        warm_target(&llm("anthropic", "claude-sonnet-5", ""), routed),
        None,
        "a cloud provider without a key is not a configured AI"
    );
    assert!(!may_route_local("none", ""));
    assert!(!may_route_local("openai", ""));
}

/// The warm-up still loads the model the judge lanes will call for users who
/// chose AI: the routed local judge for a keyed cloud user, and their own
/// model for an Ollama user (`route_judge` never swaps an Ollama main model).
#[test]
fn the_judge_that_will_run_is_loaded_for_users_who_chose_ai() {
    let routed = Some(("gemma4:12b".to_string(), DEFAULT_OLLAMA_URL.to_string()));
    assert_eq!(
        warm_target(&llm("anthropic", "claude-sonnet-5", "k"), routed.clone()),
        routed,
        "a keyed cloud user's judge lanes run on the routed local judge"
    );
    assert_eq!(
        warm_target(&llm("anthropic", "claude-sonnet-5", "k"), None),
        None,
        "no local judge routed: nothing to load, the cloud sibling judges"
    );
    let mut ollama = llm("ollama", "qwen3:8b", "");
    ollama.base_url = Some("http://127.0.0.1:11500".into());
    assert_eq!(
        warm_target(&ollama, routed),
        Some(("qwen3:8b".to_string(), "http://127.0.0.1:11500".to_string())),
        "an Ollama user's own model is the one their judge lanes call"
    );
    assert_eq!(
        warm_target(&llm("ollama", "qwen3:8b", ""), None).map(|(_, url)| url),
        Some(DEFAULT_OLLAMA_URL.to_string())
    );
    assert!(
        !may_route_local("ollama", ""),
        "an Ollama main model is kept"
    );
    assert!(may_route_local("anthropic", "k"));
}
