use super::ExpectedOutcome;
use crate::SourceRelevance;

#[derive(Debug, Default)]
pub(super) struct SimMetrics {
    pub tp: u32,
    pub fp: u32,
    pub tn: u32,
    pub r#fn: u32,
    pub relevant_scores: Vec<f64>,
    pub noise_scores: Vec<f64>,
}

impl SimMetrics {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn record(&mut self, result: &SourceRelevance, expected: ExpectedOutcome) {
        let score = result.top_score as f64;
        let predicted = result.relevant && !result.excluded;
        match expected {
            ExpectedOutcome::StrongRelevant | ExpectedOutcome::WeakRelevant => {
                self.relevant_scores.push(score);
                if predicted {
                    self.tp += 1;
                } else {
                    self.r#fn += 1;
                }
            }
            ExpectedOutcome::NotRelevant => {
                self.noise_scores.push(score);
                if predicted {
                    self.fp += 1;
                } else {
                    self.tn += 1;
                }
            }
            ExpectedOutcome::Excluded => {
                if result.excluded {
                    self.tn += 1;
                } else {
                    self.fp += 1;
                }
            }
            ExpectedOutcome::MildBorderline => {}
        }
    }

    pub(super) fn precision(&self) -> f64 {
        let d = (self.tp + self.fp) as f64;
        if d == 0.0 {
            1.0
        } else {
            self.tp as f64 / d
        }
    }
    pub(super) fn recall(&self) -> f64 {
        let d = (self.tp + self.r#fn) as f64;
        if d == 0.0 {
            1.0
        } else {
            self.tp as f64 / d
        }
    }
    pub(super) fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        let d = p + r;
        if d == 0.0 {
            0.0
        } else {
            2.0 * p * r / d
        }
    }
    pub(super) fn mean_relevant_score(&self) -> f64 {
        if self.relevant_scores.is_empty() {
            0.0
        } else {
            self.relevant_scores.iter().sum::<f64>() / self.relevant_scores.len() as f64
        }
    }
    pub(super) fn mean_noise_score(&self) -> f64 {
        if self.noise_scores.is_empty() {
            0.0
        } else {
            self.noise_scores.iter().sum::<f64>() / self.noise_scores.len() as f64
        }
    }
    pub(super) fn separation_gap(&self) -> f64 {
        self.mean_relevant_score() - self.mean_noise_score()
    }

    pub(super) fn assert_quality(&self, label: &str, min_p: f64, min_r: f64, min_f: f64) {
        let (p, r, f) = (self.precision(), self.recall(), self.f1());
        if p < min_p || r < min_r || f < min_f {
            panic!("\n{}\nFAILED: P={p:.3}(min {min_p:.2}) R={r:.3}(min {min_r:.2}) F1={f:.3}(min {min_f:.2})\n",
                self.format_report(label));
        }
    }

    pub(super) fn format_report(&self, label: &str) -> String {
        format!(
            "[{label}] TP={} FP={} TN={} FN={} | P={:.3} R={:.3} F1={:.3} | rel={:.3} noise={:.3} gap={:.3}",
            self.tp, self.fp, self.tn, self.r#fn,
            self.precision(), self.recall(), self.f1(),
            self.mean_relevant_score(), self.mean_noise_score(), self.separation_gap(),
        )
    }

    pub(super) fn merge(&mut self, other: &SimMetrics) {
        self.tp += other.tp;
        self.fp += other.fp;
        self.tn += other.tn;
        self.r#fn += other.r#fn;
        self.relevant_scores
            .extend_from_slice(&other.relevant_scores);
        self.noise_scores.extend_from_slice(&other.noise_scores);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn r(score: f32, relevant: bool) -> SourceRelevance {
        SourceRelevance {
            id: 1,
            title: "t".to_string(),
            url: None,
            top_score: score,
            matches: vec![],
            relevant,
            context_score: 0.0,
            interest_score: 0.0,
            excluded: false,
            excluded_by: None,
            source_type: "t".to_string(),
            explanation: None,
            confidence: None,
            score_breakdown: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
        }
    }

    #[test]
    fn metrics_basic() {
        let mut m = SimMetrics::new();
        m.record(&r(0.8, true), ExpectedOutcome::StrongRelevant);
        m.record(&r(0.7, true), ExpectedOutcome::StrongRelevant);
        m.record(&r(0.6, true), ExpectedOutcome::NotRelevant);
        m.record(&r(0.2, false), ExpectedOutcome::StrongRelevant);
        m.record(&r(0.1, false), ExpectedOutcome::NotRelevant);
        assert!((m.precision() - 2.0 / 3.0).abs() < 0.001);
        assert!((m.recall() - 2.0 / 3.0).abs() < 0.001);
        assert!(m.f1() > 0.6);
    }

    #[test]
    fn metrics_perfect() {
        let mut m = SimMetrics::new();
        m.record(&r(0.9, true), ExpectedOutcome::StrongRelevant);
        m.record(&r(0.1, false), ExpectedOutcome::NotRelevant);
        assert!((m.precision() - 1.0).abs() < 0.001);
        assert!((m.recall() - 1.0).abs() < 0.001);
    }

    #[test]
    fn metrics_gap() {
        let mut m = SimMetrics::new();
        m.record(&r(0.8, true), ExpectedOutcome::StrongRelevant);
        m.record(&r(0.1, false), ExpectedOutcome::NotRelevant);
        assert!(m.separation_gap() > 0.5);
    }
}
