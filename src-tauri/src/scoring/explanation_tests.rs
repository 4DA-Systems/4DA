// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

#[test]
fn test_temporal_freshness_very_recent() {
    let now = chrono::Utc::now();
    let freshness = compute_temporal_freshness(&now);
    assert_eq!(freshness, 1.10, "Items just created should get max boost");
}

#[test]
fn test_temporal_freshness_few_hours() {
    let four_hours_ago = chrono::Utc::now() - chrono::Duration::hours(4);
    let freshness = compute_temporal_freshness(&four_hours_ago);
    assert_eq!(freshness, 1.08, "Items 4h old should get 1.08x boost");
}

#[test]
fn test_temporal_freshness_half_day() {
    let thirteen_hours_ago = chrono::Utc::now() - chrono::Duration::hours(13);
    let freshness = compute_temporal_freshness(&thirteen_hours_ago);
    assert_eq!(freshness, 1.05, "Items 13h old should get 1.05x boost");
}

#[test]
fn test_temporal_freshness_one_day() {
    let thirty_hours_ago = chrono::Utc::now() - chrono::Duration::hours(30);
    let freshness = compute_temporal_freshness(&thirty_hours_ago);
    assert_eq!(freshness, 1.0, "Items 30h old should be neutral");
}

#[test]
fn test_temporal_freshness_old() {
    let four_days_ago = chrono::Utc::now() - chrono::Duration::hours(96);
    let freshness = compute_temporal_freshness(&four_days_ago);
    assert_eq!(freshness, 0.92, "Items 4 days old should decay to 0.92");
}

#[test]
fn test_temporal_freshness_very_old() {
    let old = chrono::Utc::now() - chrono::Duration::hours(200);
    let freshness = compute_temporal_freshness(&old);
    assert_eq!(freshness, 0.85, "Items 8+ days old should decay to 0.85");
}

// ====================================================================
// calculate_confidence tests
// ====================================================================

#[test]
fn test_calculate_confidence_no_signals() {
    let ctx = ACEContext::default();
    let confidence = calculate_confidence(0.0, 0.0, 0.0, &ctx, &[], 0, 0, 0);
    assert_eq!(confidence, scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL);
}

#[test]
fn test_calculate_confidence_context_only() {
    let ctx = ACEContext::default();
    let confidence = calculate_confidence(0.8, 0.0, 0.0, &ctx, &[], 10, 0, 1);
    assert!(confidence > scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL);
}

#[test]
fn test_calculate_confidence_higher_confirmation_boosts() {
    let ctx = ACEContext::default();
    let conf_1 = calculate_confidence(0.5, 0.5, 0.0, &ctx, &[], 10, 5, 1);
    let conf_3 = calculate_confidence(0.5, 0.5, 0.0, &ctx, &[], 10, 5, 3);
    assert!(
        conf_3 > conf_1,
        "More confirmed signals should increase confidence: {} > {}",
        conf_3,
        conf_1
    );
}

#[test]
fn test_calculate_confidence_clamped() {
    let ctx = ACEContext::default();
    let confidence = calculate_confidence(1.0, 1.0, 1.0, &ctx, &[], 100, 100, 5);
    assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
    assert!(confidence >= 0.0, "Confidence should not be negative");
}
