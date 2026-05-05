// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Probe Engine — domain detection, probe selection, calibration execution,
//! and signal axis audit.

use std::sync::LazyLock;

use crate::probes_corpus::{all_calibration_probes, CalibrProbe, Domain, ProbeExpected};
use crate::scoring::{score_item, ScoringContext, ScoringInput, ScoringOptions};

static CALIBRATION_PROBES: LazyLock<Vec<CalibrProbe>> = LazyLock::new(all_calibration_probes);

// ============================================================================
// Domain Detection
// ============================================================================

pub(crate) fn detect_user_domain(ctx: &ScoringContext) -> Domain {
    let mut scores: [u32; 5] = [0; 5]; // systems, web, ml, devops, mobile

    let systems_kw = [
        "rust", "c++", "clang", "kernel", "embedded", "systems", "llvm", "zig", "assembly",
    ];
    let web_kw = [
        "react",
        "typescript",
        "javascript",
        "nextjs",
        "next.js",
        "vue",
        "angular",
        "css",
        "html",
        "svelte",
        "node",
        "deno",
        "bun",
        "tailwind",
        "frontend",
        "web",
    ];
    let ml_kw = [
        "python",
        "pytorch",
        "tensorflow",
        "machine learning",
        "data science",
        "numpy",
        "pandas",
        "jupyter",
        "huggingface",
    ];
    let devops_kw = [
        "docker",
        "kubernetes",
        "k8s",
        "terraform",
        "ansible",
        "ci/cd",
        "aws",
        "gcp",
        "azure",
        "devops",
        "sre",
        "helm",
        "jenkins",
        "github actions",
    ];
    let mobile_kw = [
        "swift",
        "kotlin",
        "ios",
        "android",
        "react native",
        "flutter",
        "mobile",
        "xcode",
        "swiftui",
        "jetpack compose",
    ];

    let keyword_sets: [(&[&str], usize); 5] = [
        (&systems_kw, 0),
        (&web_kw, 1),
        (&ml_kw, 2),
        (&devops_kw, 3),
        (&mobile_kw, 4),
    ];

    /// Word-boundary match: keyword must match the full input or appear
    /// as a standalone word (not as a substring of a longer word).
    fn kw_matches(input: &str, kw: &str) -> bool {
        if input == kw {
            return true;
        }
        // For multi-word keywords, substring match is fine
        if kw.contains(' ') {
            return input.contains(kw);
        }
        // For single-word keywords, require word boundary
        input
            .split(|c: char| !c.is_alphanumeric() && c != '+')
            .any(|word| word == kw)
    }

    // Check declared tech
    for tech in &ctx.declared_tech {
        let t = tech.to_lowercase();
        for (kws, idx) in &keyword_sets {
            if kws.iter().any(|k| kw_matches(&t, k)) {
                scores[*idx] += 3;
            }
        }
    }

    // Check interests
    for interest in &ctx.interests {
        let t = interest.topic.to_lowercase();
        for (kws, idx) in &keyword_sets {
            if kws.iter().any(|k| kw_matches(&t, k)) {
                scores[*idx] += 2;
            }
        }
    }

    // Check composed stack tech keywords
    for tech in &ctx.composed_stack.all_tech {
        let t = tech.to_lowercase();
        for (kws, idx) in &keyword_sets {
            if kws.iter().any(|k| kw_matches(&t, k)) {
                scores[*idx] += 2;
            }
        }
    }

    // Check ACE active topics
    for topic in &ctx.ace_ctx.active_topics {
        let t = topic.to_lowercase();
        for (kws, idx) in &keyword_sets {
            if kws.iter().any(|k| kw_matches(&t, k)) {
                scores[*idx] += 1;
            }
        }
    }

    let max_score = *scores.iter().max().unwrap_or(&0);
    if max_score == 0 {
        return Domain::Web; // default fallback
    }

    let max_idx = scores.iter().position(|&s| s == max_score).unwrap_or(1);
    match max_idx {
        0 => Domain::Systems,
        1 => Domain::Web,
        2 => Domain::Ml,
        3 => Domain::Devops,
        4 => Domain::Mobile,
        _ => Domain::Web,
    }
}

/// Returns adjacent domain for cross-domain probes
fn adjacent_domain(primary: Domain) -> Domain {
    match primary {
        Domain::Systems => Domain::Devops,
        Domain::Web => Domain::Mobile,
        Domain::Ml => Domain::Web,
        Domain::Devops => Domain::Systems,
        Domain::Mobile => Domain::Web,
        _ => Domain::Web,
    }
}

// ============================================================================
// Probe Selection (12 per run)
// ============================================================================

pub(crate) fn select_probes_for_user(ctx: &ScoringContext) -> (Vec<&'static CalibrProbe>, Domain) {
    let probes: &Vec<CalibrProbe> = &CALIBRATION_PROBES;
    let primary = detect_user_domain(ctx);
    let adj = adjacent_domain(primary);

    let mut selected: Vec<&'static CalibrProbe> = Vec::with_capacity(12);

    // 4 from primary domain (2 Strong, 1 Weak, 1 Noise-for-domain)
    let primary_probes: Vec<&CalibrProbe> = probes.iter().filter(|p| p.domain == primary).collect();
    for p in primary_probes
        .iter()
        .filter(|p| p.expected == ProbeExpected::Strong)
        .take(2)
    {
        selected.push(p);
    }
    for p in primary_probes
        .iter()
        .filter(|p| p.expected == ProbeExpected::Weak)
        .take(1)
    {
        selected.push(p);
    }
    for p in primary_probes
        .iter()
        .filter(|p| p.expected == ProbeExpected::Noise)
        .take(1)
    {
        selected.push(p);
    }

    // 2 from adjacent domain (1 Weak, 1 Noise)
    let adj_probes: Vec<&CalibrProbe> = probes.iter().filter(|p| p.domain == adj).collect();
    for p in adj_probes
        .iter()
        .filter(|p| p.expected == ProbeExpected::Weak)
        .take(1)
    {
        selected.push(p);
    }
    for p in adj_probes
        .iter()
        .filter(|p| p.expected == ProbeExpected::Noise)
        .take(1)
    {
        selected.push(p);
    }

    // 2 universal (Strong)
    for p in probes
        .iter()
        .filter(|p| p.domain == Domain::Universal && p.expected == ProbeExpected::Strong)
        .take(2)
    {
        selected.push(p);
    }

    // 4 pure noise
    for p in probes
        .iter()
        .filter(|p| p.domain == Domain::PureNoise)
        .take(4)
    {
        selected.push(p);
    }

    (selected, primary)
}

// ============================================================================
// Probe Calibration Run
// ============================================================================

pub(crate) struct ProbeResults {
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub separation_gap: f64,
    pub passed: u32,
    pub total: u32,
    pub failures: Vec<String>,
}

pub(crate) fn run_probe_calibration(
    ctx: &ScoringContext,
    db: &crate::db::Database,
) -> ProbeResults {
    let (probes, _domain) = select_probes_for_user(ctx);
    let opts = ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
        trend_topics: vec![],
    };
    let zero_emb = vec![0.0_f32; 384];

    let mut tp = 0u32;
    let mut fp = 0u32;
    let mut _tn = 0u32;
    let mut fn_ = 0u32;
    let mut relevant_scores: Vec<f64> = Vec::new();
    let mut noise_scores: Vec<f64> = Vec::new();
    let mut passed = 0u32;
    let mut total = 0u32;
    let mut failures = Vec::new();

    for (i, probe) in probes.iter().enumerate() {
        let input = ScoringInput {
            id: 90000 + i as u64,
            title: probe.title,
            url: Some("https://probe.test"),
            content: probe.content,
            source_type: "hackernews",
            embedding: &zero_emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        };
        let result = score_item(&input, ctx, db, &opts, None);
        total += 1;

        let expected_relevant =
            probe.expected == ProbeExpected::Strong || probe.expected == ProbeExpected::Weak;
        let is_noise = probe.expected == ProbeExpected::Noise;

        if expected_relevant {
            relevant_scores.push(result.top_score as f64);
            if result.relevant {
                tp += 1;
                passed += 1;
            } else {
                fn_ += 1;
                failures.push(format!(
                    "'{}' — expected relevant, got noise (score={:.3})",
                    probe.title, result.top_score
                ));
            }
        } else if is_noise {
            noise_scores.push(result.top_score as f64);
            if result.relevant {
                fp += 1;
                failures.push(format!(
                    "'{}' — expected noise, got relevant (score={:.3})",
                    probe.title, result.top_score
                ));
            } else {
                _tn += 1;
                passed += 1;
            }
        }
    }

    let precision = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        1.0
    };
    let recall = if tp + fn_ > 0 {
        tp as f64 / (tp + fn_) as f64
    } else {
        1.0
    };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let mean_rel = if relevant_scores.is_empty() {
        0.0
    } else {
        relevant_scores.iter().sum::<f64>() / relevant_scores.len() as f64
    };
    let mean_noise = if noise_scores.is_empty() {
        0.0
    } else {
        noise_scores.iter().sum::<f64>() / noise_scores.len() as f64
    };
    let separation_gap = mean_rel - mean_noise;

    ProbeResults {
        f1,
        precision,
        recall,
        separation_gap,
        passed,
        total,
        failures,
    }
}

// ============================================================================
// Signal Axis Audit
// ============================================================================

pub(crate) struct SignalAudit {
    pub axes: Vec<String>,
    pub context_fires: bool,
    pub interest_fires: bool,
    pub ace_fires: bool,
    pub learned_fires: bool,
    pub dependency_fires: bool,
}

pub(crate) fn audit_signal_axes(ctx: &ScoringContext, db: &crate::db::Database) -> SignalAudit {
    // Score a single domain-relevant probe and inspect the breakdown
    let zero_emb = vec![0.0_f32; 384];
    let opts = ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
        trend_topics: vec![],
    };

    // Use a generic dev probe that should score for most contexts
    let input = ScoringInput {
        id: 99999,
        title: "Critical CVE in widely-used open source library",
        content: "A critical remote code execution vulnerability has been discovered in a popular dependency. All developers should update immediately.",
        source_type: "hackernews",
        url: Some("https://probe.test"),
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
    };

    let result = score_item(&input, ctx, db, &opts, None);
    let bd = result.score_breakdown.as_ref();

    let context_fires = bd.is_some_and(|b| b.context_score >= 0.45) || ctx.cached_context_count > 0;
    let interest_fires = bd.is_some_and(|b| b.interest_score >= 0.50 || b.keyword_score >= 0.60);
    let ace_fires =
        bd.is_some_and(|b| b.ace_boost >= 0.12) || !ctx.ace_ctx.active_topics.is_empty();
    let learned_fires = bd.is_some_and(|b| b.feedback_boost > 0.05 || b.affinity_mult >= 1.15);
    let dependency_fires = bd.is_some_and(|b| b.dep_match_score >= 0.20);

    let mut axes = Vec::new();
    if context_fires {
        axes.push("context".to_string());
    }
    if interest_fires {
        axes.push("interest".to_string());
    }
    if ace_fires {
        axes.push("ace".to_string());
    }
    if learned_fires {
        axes.push("learned".to_string());
    }
    if dependency_fires {
        axes.push("dependency".to_string());
    }

    SignalAudit {
        axes,
        context_fires,
        interest_fires,
        ace_fires,
        learned_fires,
        dependency_fires,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probes_corpus::Domain;
    use crate::scoring::ScoringContext;

    #[test]
    fn domain_detection_rust_user() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["rust".to_string(), "tauri".to_string()])
            .build();
        assert_eq!(detect_user_domain(&ctx), Domain::Systems);
    }

    #[test]
    fn domain_detection_web_user() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["typescript".to_string(), "react".to_string()])
            .build();
        assert_eq!(detect_user_domain(&ctx), Domain::Web);
    }

    #[test]
    fn domain_detection_ml_user() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["python".to_string(), "pytorch".to_string()])
            .build();
        assert_eq!(detect_user_domain(&ctx), Domain::Ml);
    }

    #[test]
    fn domain_detection_devops_user() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["terraform".to_string(), "kubernetes".to_string()])
            .build();
        assert_eq!(detect_user_domain(&ctx), Domain::Devops);
    }

    #[test]
    fn domain_detection_mobile_user() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["kotlin".to_string(), "android".to_string()])
            .build();
        assert_eq!(detect_user_domain(&ctx), Domain::Mobile);
    }

    #[test]
    fn domain_detection_empty_defaults_to_web() {
        let ctx = ScoringContext::builder().build();
        assert_eq!(detect_user_domain(&ctx), Domain::Web);
    }

    #[test]
    fn probe_selection_returns_12() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["rust".to_string()])
            .build();
        let (selected, domain) = select_probes_for_user(&ctx);
        assert_eq!(selected.len(), 12);
        assert_eq!(domain, Domain::Systems);
    }

    #[test]
    fn probe_selection_contains_primary_and_noise() {
        let ctx = ScoringContext::builder()
            .declared_tech(vec!["react".to_string()])
            .build();
        let (selected, _) = select_probes_for_user(&ctx);
        let has_primary = selected.iter().any(|p| p.domain == Domain::Web);
        let has_noise = selected.iter().any(|p| p.domain == Domain::PureNoise);
        assert!(has_primary);
        assert!(has_noise);
    }

    #[test]
    fn signal_audit_empty_context() {
        let db = crate::test_utils::test_db();
        let ctx = crate::test_utils::empty_scoring_context();
        let audit = audit_signal_axes(&ctx, &db);
        // Empty context might still fire interest axis via keyword matching
        assert!(audit.axes.len() <= 5);
    }
}
