// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::collections::HashMap;
use tracing::info;

/// Per-batch scoring telemetry — tracks how many items each pipeline stage affected.
/// Logged after each analysis run for debugging and tuning.
#[derive(Debug, Default)]
pub(crate) struct ScoringTelemetry {
    pub total_scored: usize,
    pub excluded_count: usize,
    pub relevant_count: usize,
    /// How many items had N confirmed signals (index = signal count)
    pub gate_distribution: [usize; 6],
    pub domain_diversity_adjusted: usize,
    pub anti_topic_penalized: usize,
    pub commodity_capped: usize,
    pub serendipity_injected: usize,
    pub dedup_removed: usize,
    pub fuzzy_dedup_removed: usize,
    pub topic_dedup_removed: usize,
    /// Per-source-type counts: source_type -> (total, relevant)
    pub source_breakdown: HashMap<String, (usize, usize)>,
}

impl ScoringTelemetry {
    pub fn log_summary(&self) {
        let gate_dist = format!(
            "0sig={} 1sig={} 2sig={} 3sig={} 4sig={} 5sig={}",
            self.gate_distribution[0],
            self.gate_distribution[1],
            self.gate_distribution[2],
            self.gate_distribution[3],
            self.gate_distribution[4],
            self.gate_distribution[5],
        );

        info!(
            target: "4da::scoring::telemetry",
            total = self.total_scored,
            excluded = self.excluded_count,
            relevant = self.relevant_count,
            anti_topic = self.anti_topic_penalized,
            commodity = self.commodity_capped,
            domain_div = self.domain_diversity_adjusted,
            dedup = self.dedup_removed,
            fuzzy = self.fuzzy_dedup_removed,
            topic = self.topic_dedup_removed,
            serendipity = self.serendipity_injected,
            "Scoring telemetry: gate=[{gate_dist}]"
        );

        // Log per-source breakdown if there are multiple sources
        if self.source_breakdown.len() > 1 {
            let mut sources: Vec<_> = self.source_breakdown.iter().collect();
            sources.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
            let summary: Vec<String> = sources
                .iter()
                .map(|(src, (total, relevant))| format!("{src}={relevant}/{total}"))
                .collect();
            info!(
                target: "4da::scoring::telemetry",
                "Source breakdown: {}",
                summary.join(", ")
            );
        }
    }
}
