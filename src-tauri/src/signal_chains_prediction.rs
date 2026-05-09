// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Chain lifecycle prediction for signal chains.
//!
//! Analyzes temporal patterns in chain links to classify lifecycle phase
//! (nascent → active → escalating → peak → resolving) and forecast
//! the next signal's arrival.

use serde::{Deserialize, Serialize};

use super::{ChainLink, SignalChain};
use crate::scoring_config;

// ============================================================================
// Chain Lifecycle Prediction
// ============================================================================

/// Lifecycle phase of a signal chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChainPhase {
    /// Just detected, 1-2 signals — may fizzle
    Nascent,
    /// Multiple signals confirmed, pattern emerging
    Active,
    /// Signal frequency increasing (acceleration detected)
    Escalating,
    /// Highest signal density, maximum relevance
    Peak,
    /// Signals slowing, topic fading
    Resolving,
}

/// Prediction attached to a signal chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainPrediction {
    /// Current lifecycle phase
    pub phase: ChainPhase,
    /// Inter-event intervals in hours (newest first)
    pub intervals_hours: Vec<f64>,
    /// Acceleration: negative = speeding up, positive = slowing down
    pub acceleration: f64,
    /// Estimated hours until next signal (based on trend)
    pub predicted_next_hours: Option<f64>,
    /// Confidence in prediction (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable forecast
    pub forecast: String,
}

/// Analyze a chain's lifecycle and generate predictions
pub fn predict_chain_lifecycle(chain: &SignalChain) -> ChainPrediction {
    let links = &chain.links;

    if links.len() < 2 {
        return ChainPrediction {
            phase: ChainPhase::Nascent,
            intervals_hours: vec![],
            acceleration: 0.0,
            predicted_next_hours: None,
            confidence: 0.1,
            forecast: "Too early to predict — watching for more signals".to_string(),
        };
    }

    // Calculate inter-event intervals in hours
    let intervals = compute_intervals(links);
    let acceleration = compute_acceleration(&intervals);

    // Determine phase
    let phase = classify_phase(links.len(), acceleration, &intervals);

    // Predict next event timing
    let predicted_next = predict_next_interval(&intervals, acceleration);
    let confidence = compute_confidence(links.len(), &intervals);

    let forecast = build_forecast(&phase, &chain.chain_name, predicted_next, acceleration);

    ChainPrediction {
        phase,
        intervals_hours: intervals,
        acceleration,
        predicted_next_hours: predicted_next,
        confidence,
        forecast,
    }
}

/// Compute time intervals between consecutive chain links (in hours)
fn compute_intervals(links: &[ChainLink]) -> Vec<f64> {
    if links.len() < 2 {
        return vec![];
    }

    let timestamps: Vec<chrono::DateTime<chrono::Utc>> = links
        .iter()
        .filter_map(|l| chrono::DateTime::parse_from_rfc3339(&l.timestamp).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .collect();

    if timestamps.len() < 2 {
        return vec![];
    }

    timestamps
        .windows(2)
        .map(|w| {
            let diff = w[1] - w[0];
            diff.num_minutes() as f64 / 60.0
        })
        .collect()
}

/// Compute acceleration: slope of interval changes (negative = speeding up)
fn compute_acceleration(intervals: &[f64]) -> f64 {
    if intervals.len() < 2 {
        return 0.0;
    }

    // Simple linear regression on interval sequence
    let n = intervals.len() as f64;
    let sum_x: f64 = (0..intervals.len()).map(|i| i as f64).sum();
    let sum_y: f64 = intervals.iter().sum();
    let sum_xy: f64 = intervals
        .iter()
        .enumerate()
        .map(|(i, y)| i as f64 * y)
        .sum();
    let sum_x2: f64 = (0..intervals.len()).map(|i| (i as f64).powi(2)).sum();

    let denom = n * sum_x2 - sum_x.powi(2);
    if denom.abs() < 1e-10 {
        return 0.0;
    }

    (n * sum_xy - sum_x * sum_y) / denom
}

/// Classify chain lifecycle phase
fn classify_phase(link_count: usize, acceleration: f64, intervals: &[f64]) -> ChainPhase {
    if link_count <= 2 {
        return ChainPhase::Nascent;
    }

    let avg_interval = if intervals.is_empty() {
        f64::MAX
    } else {
        intervals.iter().sum::<f64>() / intervals.len() as f64
    };

    if acceleration < scoring_config::SIGNAL_CHAIN_PHASE_ESCALATING_ACCELERATION as f64
        && link_count >= scoring_config::SIGNAL_CHAIN_PHASE_ESCALATING_MIN_LINKS as usize
    {
        return ChainPhase::Escalating;
    }

    if link_count >= scoring_config::SIGNAL_CHAIN_PHASE_PEAK_MIN_LINKS as usize
        && avg_interval < scoring_config::SIGNAL_CHAIN_PHASE_PEAK_MAX_INTERVAL as f64
    {
        return ChainPhase::Peak;
    }

    if acceleration > scoring_config::SIGNAL_CHAIN_PHASE_RESOLVING_ACCELERATION as f64
        && link_count >= scoring_config::SIGNAL_CHAIN_PHASE_RESOLVING_MIN_LINKS as usize
    {
        return ChainPhase::Resolving;
    }

    ChainPhase::Active
}

/// Predict the next interval based on trend
fn predict_next_interval(intervals: &[f64], acceleration: f64) -> Option<f64> {
    if intervals.is_empty() {
        return None;
    }

    let last = *intervals.last()?;
    let predicted = last + acceleration;

    Some(predicted.clamp(
        scoring_config::SIGNAL_CHAIN_PREDICTION_MIN_HOURS as f64,
        scoring_config::SIGNAL_CHAIN_PREDICTION_MAX_HOURS as f64,
    ))
}

/// Compute prediction confidence based on data quality
fn compute_confidence(link_count: usize, intervals: &[f64]) -> f64 {
    // Base: more data = more confidence
    let data_confidence = (link_count as f64 / 6.0).min(1.0);

    // Regularity: consistent intervals = more predictable
    let regularity = if intervals.len() >= 2 {
        let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let variance =
            intervals.iter().map(|i| (i - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
        let cv = if mean > 0.0 {
            variance.sqrt() / mean
        } else {
            1.0
        };
        (1.0 - cv.min(1.0)).max(0.0)
    } else {
        0.3
    };

    (data_confidence * 0.6 + regularity * 0.4).min(0.85)
}

/// Build a human-readable forecast
fn build_forecast(
    phase: &ChainPhase,
    chain_name: &str,
    predicted_hours: Option<f64>,
    acceleration: f64,
) -> String {
    let timing = predicted_hours
        .map(|h| {
            if h < 2.0 {
                "within hours".to_string()
            } else if h < 24.0 {
                format!("within ~{h:.0}h")
            } else {
                format!("within ~{:.0} days", h / 24.0)
            }
        })
        .unwrap_or_else(|| "timing uncertain".to_string());

    match phase {
        ChainPhase::Nascent => format!("Early signal for {chain_name} — monitoring"),
        ChainPhase::Active => format!("{chain_name} is developing — next signal expected {timing}"),
        ChainPhase::Escalating => {
            let rate = if acceleration < -5.0 {
                "rapidly"
            } else {
                "steadily"
            };
            format!("{chain_name} is {rate} accelerating — act {timing}")
        }
        ChainPhase::Peak => {
            format!("{chain_name} at peak intensity — high activity expected {timing}")
        }
        ChainPhase::Resolving => format!("{chain_name} is cooling down — signals slowing"),
    }
}
