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
