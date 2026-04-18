// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Calibration analysis — topic-level and accuracy feedback analysis.
//!
//! Provides deeper calibration by analyzing at the content-topic level
//! (not just source_type level) and bridging ACE behavior data into
//! the calibration system.

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use super::calibration::store_calibrations;
use crate::error::Result;

/// Analyze accuracy feedback from ACE behavior data to produce topic-level calibration.
///
/// Reads implicit interaction signals (save, click, dismiss, ignore, scroll) from the
/// ACE database, groups by content topic, and computes per-topic engagement deltas.
/// These deltas feed directly into the scoring pipeline's calibration correction.
///
/// Unlike `analyze_calibration()` which groups by source_type, this groups by content
/// topic — catching biases the source-level analysis misses (e.g., "we over-score
/// security articles regardless of source").
pub(crate) fn analyze_accuracy_feedback(
    ace_conn: &Connection,
    lookback_days: i64,
) -> Vec<super::CalibrationDelta> {
    let window = format!("-{lookback_days} days");

    // Query all interactions within the lookback window that have topics
    let mut stmt = match ace_conn.prepare(
        "SELECT item_topics, signal_strength
         FROM interactions
         WHERE timestamp >= datetime('now', ?1)
           AND item_topics IS NOT NULL
           AND item_topics != '[]'",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Accuracy feedback query failed");
            return vec![];
        }
    };

    // Accumulate per-topic stats: (total, positive_count, total_signal)
    let mut topic_stats: HashMap<String, (i64, i64, f64)> = HashMap::new();

    let rows = match stmt.query_map(params![window], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Accuracy feedback row iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (topics_json, signal_strength) = row;
        let topics: Vec<String> = match serde_json::from_str(&topics_json) {
            Ok(t) => t,
            Err(_) => continue,
        };

        for topic in topics {
            let entry = topic_stats.entry(topic).or_insert((0, 0, 0.0));
            entry.0 += 1; // total
            if signal_strength > 0.0 {
                entry.1 += 1; // positive count
            }
            entry.2 += signal_strength; // sum of signals
        }
    }

    if topic_stats.is_empty() {
        debug!(target: "4da::autophagy", "No accuracy feedback data available");
        return vec![];
    }

    let mut deltas = Vec::new();
    let min_samples = 5;

    for (topic, (total, positive, signal_sum)) in &topic_stats {
        if *total < min_samples {
            continue;
        }

        // Engagement rate: proportion of positive interactions
        let engaged_avg = *positive as f32 / *total as f32;
        // System baseline: we expect roughly 50% positive engagement
        // (if scoring is perfectly calibrated, users like half of what we show)
        let scored_avg = 0.5_f32;
        // Delta: positive = users like this topic more than expected (boost it)
        //        negative = users reject this topic (penalize it)
        let delta = engaged_avg - scored_avg;
        // Also factor in signal magnitude (save=1.0 is stronger than click=0.5)
        let avg_signal = *signal_sum as f32 / *total as f32;
        // Blend engagement rate delta with signal magnitude for richer correction
        let blended_delta = delta * 0.6 + avg_signal * 0.4;
        let confidence = (*total as f32 / 15.0).min(1.0);

        deltas.push(super::CalibrationDelta {
            topic: topic.clone(),
            scored_avg,
            engaged_avg,
            delta: blended_delta,
            sample_size: *total,
            confidence,
        });
    }

    info!(
        target: "4da::autophagy",
        topics = deltas.len(),
        total_interactions = topic_stats.values().map(|(t, _, _)| t).sum::<i64>(),
        "Accuracy feedback analysis complete"
    );

    deltas
}

/// Analyze calibration at the content-topic level (not source_type level).
///
/// Extracts topics from item titles in the pruning window, cross-references with
/// feedback, and computes per-topic engagement deltas. This catches biases that
/// source-level analysis misses — e.g., "we over-score articles about kubernetes
/// regardless of whether they come from HN or Reddit."
pub(crate) fn analyze_topic_calibration(
    conn: &Connection,
    max_age_days: i64,
) -> Vec<super::CalibrationDelta> {
    let window_start_days = max_age_days;
    let window_end_days = max_age_days.saturating_sub(7);
    let window_start = format!("-{window_start_days} days");
    let window_end = format!("-{window_end_days} days");

    // Query items in pruning window with their engagement status
    let mut stmt = match conn.prepare(
        "SELECT si.title,
                COALESCE((SELECT MAX(f.relevant) FROM feedback f WHERE f.source_item_id = si.id), 0) AS engaged
         FROM source_items si
         WHERE si.last_seen < datetime('now', ?1)
           AND si.last_seen >= datetime('now', ?2)",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Topic calibration query failed");
            return vec![];
        }
    };

    // Accumulate per-topic stats: (total, engaged_count)
    let mut topic_stats: HashMap<String, (i64, i64)> = HashMap::new();

    let rows = match stmt.query_map(params![window_end, window_start], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Topic calibration iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (title, engaged) = row;
        let topics = extract_title_topics(&title);
        for topic in topics {
            let entry = topic_stats.entry(topic).or_insert((0, 0));
            entry.0 += 1;
            if engaged > 0 {
                entry.1 += 1;
            }
        }
    }

    if topic_stats.is_empty() {
        debug!(target: "4da::autophagy", "No items for topic-level calibration");
        return vec![];
    }

    let min_samples = 3;
    let mut deltas = Vec::new();

    for (topic, (total, engaged)) in &topic_stats {
        if *total < min_samples {
            continue;
        }

        let scored_avg = 1.0_f32;
        let engaged_avg = *engaged as f32 / *total as f32;
        let delta = engaged_avg - scored_avg;
        let confidence = (*total as f32 / 10.0).min(1.0);

        deltas.push(super::CalibrationDelta {
            topic: topic.clone(),
            scored_avg,
            engaged_avg,
            delta,
            sample_size: *total,
            confidence,
        });
    }

    info!(
        target: "4da::autophagy",
        topics = deltas.len(),
        total_items = topic_stats.values().map(|(t, _)| t).sum::<i64>(),
        "Topic-level calibration analysis complete"
    );

    deltas
}

/// Extract meaningful topic keywords from a title.
///
/// Filters to lowercase words > 3 chars, removes common stop words,
/// keeps at most 5 topics per title.
fn extract_title_topics(title: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "the", "this", "that", "with", "from", "your", "about", "what", "when", "where", "which",
        "their", "there", "they", "have", "been", "will", "would", "could", "should", "more",
        "most", "some", "than", "then", "into", "also", "just", "very", "much", "does", "like",
        "each", "every", "only", "over", "under", "after", "before", "show", "need", "make",
        "here", "best", "good", "know",
    ];

    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .filter(|w| !STOP_WORDS.contains(w))
        .take(5)
        .map(std::string::ToString::to_string)
        .collect()
}

/// Bridge accuracy feedback from ACE behavior data into the main calibration system.
///
/// Reads from the ACE database, analyzes per-topic engagement, and stores the resulting
/// calibration deltas in the main database's `digested_intelligence` table.
/// Returns the number of calibration entries produced.
pub(crate) fn bridge_accuracy_feedback(
    ace_conn: &Connection,
    main_conn: &Connection,
    lookback_days: i64,
) -> Result<usize> {
    let deltas = analyze_accuracy_feedback(ace_conn, lookback_days);
    if deltas.is_empty() {
        return Ok(0);
    }

    let count = deltas.len();
    store_calibrations(main_conn, &deltas)?;

    info!(
        target: "4da::autophagy",
        count,
        "Bridged accuracy feedback to calibration system"
    );

    Ok(count)
}

#[cfg(test)]
#[path = "calibration_analysis_tests.rs"]
mod tests;
