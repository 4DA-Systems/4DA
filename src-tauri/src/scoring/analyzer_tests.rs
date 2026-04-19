// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use crate::{ScoreBreakdown, SourceRelevance};

/// Helper: create a ScoreBreakdown with a specific signal_count via serde defaults
fn make_breakdown(signal_count: u8) -> ScoreBreakdown {
    let json = serde_json::json!({
        "context_score": 0.0,
        "interest_score": 0.0,
        "ace_boost": 0.0,
        "affinity_mult": 1.0,
        "anti_penalty": 0.0,
        "confidence_by_signal": {},
        "signal_count": signal_count,
    });
    serde_json::from_value(json).expect("ScoreBreakdown from JSON")
}

/// Helper: create a minimal SourceRelevance for testing
fn make_result(title: &str, score: f32, relevant: bool, signal_count: u8) -> SourceRelevance {
    let json = serde_json::json!({
        "id": 0,
        "title": title,
        "url": null,
        "top_score": score,
        "matches": [],
        "relevant": relevant,
        "source_type": "test",
    });
    let mut result: SourceRelevance =
        serde_json::from_value(json).expect("SourceRelevance from JSON");
    result.score_breakdown = Some(make_breakdown(signal_count));
    result
}

// ========================================================================
// Signal distribution counting (mirrors run_background_analysis logic)
// ========================================================================

#[test]
fn test_signal_count_distribution() {
    let results = vec![
        make_result("Zero signals", 0.2, false, 0),
        make_result("One signal", 0.3, false, 1),
        make_result("Two signals", 0.6, true, 2),
        make_result("Three signals", 0.8, true, 3),
        make_result("Four signals", 0.9, true, 4),
    ];

    let mut dist = [0usize; 5];
    let mut rel_by_sig = [0usize; 5];
    for r in &results {
        if let Some(ref bd) = r.score_breakdown {
            let idx = (bd.signal_count as usize).min(4);
            dist[idx] += 1;
            if r.relevant && !r.excluded {
                rel_by_sig[idx] += 1;
            }
        }
    }

    assert_eq!(dist, [1, 1, 1, 1, 1], "One item per signal count");
    assert_eq!(
        rel_by_sig,
        [0, 0, 1, 1, 1],
        "Only 2+ signal items are relevant"
    );
}

#[test]
fn test_signal_count_clamped_to_4() {
    // Signal count above 4 should be clamped to index 4
    let results = vec![make_result("Five signals", 0.95, true, 5)];

    let mut dist = [0usize; 5];
    for r in &results {
        if let Some(ref bd) = r.score_breakdown {
            let idx = (bd.signal_count as usize).min(4);
            dist[idx] += 1;
        }
    }
    assert_eq!(dist[4], 1, "Signal count 5 should clamp to index 4");
}

// ========================================================================
// Signal summary building (mirrors run_background_analysis logic)
// ========================================================================

#[test]
fn test_signal_summary_counts_priorities() {
    let mut results = [
        make_result("Critical vuln", 0.9, true, 3),
        make_result("High alert", 0.8, true, 2),
        make_result("Normal item", 0.5, true, 2),
    ];
    results[0].signal_priority = Some("critical".to_string());
    results[0].signal_type = Some("security_alert".to_string());
    results[1].signal_priority = Some("alert".to_string());
    results[1].signal_type = Some("breaking_change".to_string());

    let critical = results
        .iter()
        .filter(|r| r.signal_priority.as_deref() == Some("critical"))
        .count();
    let high = results
        .iter()
        .filter(|r| r.signal_priority.as_deref() == Some("alert"))
        .count();

    assert_eq!(critical, 1, "Should have 1 critical signal");
    assert_eq!(high, 1, "Should have 1 high signal");
}

#[test]
fn test_signal_summary_top_signal_is_highest_score() {
    let mut results = [
        make_result("Low signal", 0.3, false, 1),
        make_result("High signal", 0.9, true, 3),
    ];
    results[0].signal_type = Some("deprecation".to_string());
    results[0].signal_action = Some("review".to_string());
    results[1].signal_type = Some("security_alert".to_string());
    results[1].signal_action = Some("update now".to_string());

    let top_signal = results
        .iter()
        .filter(|r| r.signal_type.is_some())
        .max_by(|a, b| {
            a.top_score
                .partial_cmp(&b.top_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|r| {
            Some((
                r.signal_type.clone()?,
                r.signal_action.clone().unwrap_or_default(),
            ))
        });

    assert!(top_signal.is_some());
    let (sig_type, sig_action) = top_signal.unwrap();
    assert_eq!(sig_type, "security_alert");
    assert_eq!(sig_action, "update now");
}

#[test]
fn test_signal_summary_no_signals() {
    let results = [make_result("Normal", 0.5, true, 2)];

    let critical = results
        .iter()
        .filter(|r| r.signal_priority.as_deref() == Some("critical"))
        .count();
    let top_signal = results
        .iter()
        .filter(|r| r.signal_type.is_some())
        .max_by(|a, b| {
            a.top_score
                .partial_cmp(&b.top_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|r| {
            Some((
                r.signal_type.clone()?,
                r.signal_action.clone().unwrap_or_default(),
            ))
        });

    assert_eq!(critical, 0);
    assert!(top_signal.is_none());
}

// ========================================================================
// Relevant/excluded counting (mirrors score_items_full logic)
// ========================================================================

#[test]
fn test_relevant_count_excludes_excluded() {
    let mut results = [
        make_result("Relevant", 0.8, true, 3),
        make_result("Excluded", 0.7, true, 2),
        make_result("Irrelevant", 0.2, false, 0),
    ];
    results[1].excluded = true;

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let excluded_count = results.iter().filter(|r| r.excluded).count();

    assert_eq!(
        relevant_count, 1,
        "Only 1 item is relevant and not excluded"
    );
    assert_eq!(excluded_count, 1, "1 item is excluded");
}
