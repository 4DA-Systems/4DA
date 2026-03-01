//! Calibration Probes — domain-aware probe corpus, signal axis audit,
//! and 4-dimension scoring for the calibration system.

use crate::calibration_commands::RigRequirements;
use crate::scoring::{score_item, ScoringContext, ScoringInput, ScoringOptions};

// ============================================================================
// Probe Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Domain {
    Systems,
    Web,
    Ml,
    Devops,
    Mobile,
    Universal,
    PureNoise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeExpected {
    Strong,
    Weak,
    Noise,
}

pub(crate) struct CalibrProbe {
    pub title: &'static str,
    pub content: &'static str,
    pub domain: Domain,
    pub expected: ProbeExpected,
}

// ============================================================================
// Probe Corpus — 28 items across 5 domains + universal + pure noise
// ============================================================================

pub(crate) fn all_calibration_probes() -> Vec<CalibrProbe> {
    vec![
        // === Systems (Rust, C++, kernel, perf) ===
        CalibrProbe {
            title: "Rust 2026 Edition stabilizes async iterators",
            content: "The Rust 2026 edition stabilizes async iterators and improves borrow checker ergonomics for self-referential structs.",
            domain: Domain::Systems,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Linux kernel 6.12 merges Rust filesystem driver",
            content: "The Linux kernel now ships a production Rust filesystem driver, marking a milestone for memory-safe systems programming.",
            domain: Domain::Systems,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "LLVM 19 improves autovectorization for ARM targets",
            content: "LLVM 19 brings better autovectorization passes and improved codegen for ARM Neoverse cores.",
            domain: Domain::Systems,
            expected: ProbeExpected::Weak,
        },
        CalibrProbe {
            title: "New Go garbage collector reduces tail latency",
            content: "Go 1.24 ships a redesigned GC that cuts p99 latency by 40% for high-throughput services.",
            domain: Domain::Systems,
            expected: ProbeExpected::Noise,
        },
        // === Web (React, Next.js, TypeScript, CSS) ===
        CalibrProbe {
            title: "React 20 server components become the default",
            content: "React 20 makes server components the default rendering mode with automatic code-splitting and streaming.",
            domain: Domain::Web,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "TypeScript 6.0 ships pattern matching and pipe operator",
            content: "TypeScript 6.0 adds native pattern matching syntax and the pipe operator for functional composition.",
            domain: Domain::Web,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Tailwind CSS v4 drops config file requirement",
            content: "Tailwind CSS v4 uses a new CSS-first configuration approach, eliminating the JavaScript config file.",
            domain: Domain::Web,
            expected: ProbeExpected::Weak,
        },
        CalibrProbe {
            title: "TensorFlow 3.0 introduces JAX-compatible API",
            content: "TensorFlow 3.0 adds a JAX-compatible functional API for research workflows.",
            domain: Domain::Web,
            expected: ProbeExpected::Noise,
        },
        // === ML (Python, PyTorch, LLMs, data science) ===
        CalibrProbe {
            title: "PyTorch 3.0 unifies eager and compiled execution",
            content: "PyTorch 3.0 merges eager and compiled modes into a single API with automatic graph optimization.",
            domain: Domain::Ml,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Anthropic releases Claude 5 with 1M context window",
            content: "Claude 5 ships with a 1 million token context window and improved reasoning capabilities.",
            domain: Domain::Ml,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Polars DataFrame library reaches 1.0",
            content: "Polars, the Rust-based DataFrame library for Python, reaches 1.0 with GPU acceleration.",
            domain: Domain::Ml,
            expected: ProbeExpected::Weak,
        },
        CalibrProbe {
            title: "Kubernetes 1.32 adds sidecar container support",
            content: "Kubernetes 1.32 natively supports sidecar containers with proper lifecycle management.",
            domain: Domain::Ml,
            expected: ProbeExpected::Noise,
        },
        // === DevOps (Docker, K8s, CI/CD, infra) ===
        CalibrProbe {
            title: "Terraform 2.0 introduces native drift detection",
            content: "Terraform 2.0 adds continuous drift detection and automatic remediation for infrastructure state.",
            domain: Domain::Devops,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "GitHub Actions adds matrix strategy for ARM builds",
            content: "GitHub Actions now supports ARM64 runners in matrix strategies for cross-platform CI/CD.",
            domain: Domain::Devops,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Prometheus 3.0 ships native OpenTelemetry support",
            content: "Prometheus 3.0 adds native OTLP ingestion, eliminating the need for separate collectors.",
            domain: Domain::Devops,
            expected: ProbeExpected::Weak,
        },
        CalibrProbe {
            title: "Swift 6.0 concurrency model finalized",
            content: "Swift 6.0 finalizes its strict concurrency model with complete data race safety.",
            domain: Domain::Devops,
            expected: ProbeExpected::Noise,
        },
        // === Mobile (Swift, Kotlin, React Native, Flutter) ===
        CalibrProbe {
            title: "Kotlin Multiplatform reaches stable for iOS",
            content: "Kotlin Multiplatform for iOS reaches stable, enabling shared business logic across Android and iOS.",
            domain: Domain::Mobile,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Flutter 4 ships Impeller renderer as default",
            content: "Flutter 4 makes Impeller the default renderer, eliminating shader compilation jank on all platforms.",
            domain: Domain::Mobile,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "Android 16 requires target SDK 35 for Play Store",
            content: "Google announces Android 16 Play Store requirement for targetSdkVersion 35 by November 2026.",
            domain: Domain::Mobile,
            expected: ProbeExpected::Weak,
        },
        CalibrProbe {
            title: "PostgreSQL 17 adds JSON table functions",
            content: "PostgreSQL 17 introduces SQL/JSON table functions for native JSON querying.",
            domain: Domain::Mobile,
            expected: ProbeExpected::Noise,
        },
        // === Universal (relevant to any developer) ===
        CalibrProbe {
            title: "Critical CVE in widely-used open source library",
            content: "A critical remote code execution vulnerability has been discovered in a popular dependency. All developers should update immediately.",
            domain: Domain::Universal,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "GitHub Copilot major update: multi-file editing",
            content: "GitHub Copilot now supports multi-file editing, workspace-aware suggestions, and improved code review integration.",
            domain: Domain::Universal,
            expected: ProbeExpected::Strong,
        },
        CalibrProbe {
            title: "VS Code ships AI-assisted refactoring tools",
            content: "VS Code ships new debugging features, improved terminal performance, and AI-assisted refactoring tools.",
            domain: Domain::Universal,
            expected: ProbeExpected::Strong,
        },
        // === Pure Noise (never relevant to a developer) ===
        CalibrProbe {
            title: "Best restaurants in downtown Brisbane",
            content: "Top 10 dining spots for lunch in Brisbane CBD. From Asian fusion to Italian classics.",
            domain: Domain::PureNoise,
            expected: ProbeExpected::Noise,
        },
        CalibrProbe {
            title: "Premier League transfer window recap",
            content: "Manchester United, Chelsea, and Arsenal made significant signings during the January transfer window.",
            domain: Domain::PureNoise,
            expected: ProbeExpected::Noise,
        },
        CalibrProbe {
            title: "New season of The Bachelor announced",
            content: "Reality TV dating show returns with a new cast of contestants and surprise twist format.",
            domain: Domain::PureNoise,
            expected: ProbeExpected::Noise,
        },
        CalibrProbe {
            title: "Celebrity couple announces surprise wedding",
            content: "Hollywood A-listers tie the knot in a secret ceremony in the Maldives.",
            domain: Domain::PureNoise,
            expected: ProbeExpected::Noise,
        },
        CalibrProbe {
            title: "Crypto price predictions for next quarter",
            content: "Bitcoin analysts predict a bull run to $200k by Q3 based on halving cycle analysis and ETF inflows.",
            domain: Domain::PureNoise,
            expected: ProbeExpected::Noise,
        },
    ]
}

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

pub(crate) fn domain_name(domain: Domain) -> &'static str {
    match domain {
        Domain::Systems => "systems",
        Domain::Web => "web",
        Domain::Ml => "ml",
        Domain::Devops => "devops",
        Domain::Mobile => "mobile",
        Domain::Universal => "universal",
        Domain::PureNoise => "noise",
    }
}

// ============================================================================
// Probe Selection (12 per run)
// ============================================================================

pub(crate) fn select_probes_for_user(ctx: &ScoringContext) -> (Vec<&'static CalibrProbe>, Domain) {
    // Leak the probes vec to get 'static references. This runs once per
    // calibration and the vec lives for the process lifetime — acceptable.
    let probes: &'static Vec<CalibrProbe> = Box::leak(Box::new(all_calibration_probes()));
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
    };

    let result = score_item(&input, ctx, db, &opts, None);
    let bd = result.score_breakdown.as_ref();

    let context_fires =
        bd.map_or(false, |b| b.context_score >= 0.45) || ctx.cached_context_count > 0;
    let interest_fires = bd.map_or(false, |b| {
        b.interest_score >= 0.50 || b.keyword_score >= 0.60
    });
    let ace_fires =
        bd.map_or(false, |b| b.ace_boost >= 0.12) || !ctx.ace_ctx.active_topics.is_empty();
    let learned_fires = bd.map_or(false, |b| {
        b.feedback_boost > 0.05 || b.affinity_mult >= 1.15
    });
    let dependency_fires = bd.map_or(false, |b| b.dep_match_score >= 0.20);

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
// 4-Dimension Score Computation
// ============================================================================

pub(crate) fn compute_infrastructure_score(rig: &RigRequirements) -> u32 {
    let mut score = 0u32;
    if rig.ollama_running {
        score += 8;
    }
    if rig.embedding_available {
        score += 12;
    }
    if rig.gpu_detected {
        score += 5;
    }
    score.min(25)
}

pub(crate) fn compute_context_score(ctx: &ScoringContext) -> u32 {
    let mut score = 0.0_f64;
    // Interests: min(count, 5) * 2.5
    let interest_pts = (ctx.interest_count.min(5) as f64) * 2.5;
    score += interest_pts;
    // Stack profiles active
    if ctx.composed_stack.active {
        score += 5.0;
    }
    // ACE active topics exist
    if !ctx.ace_ctx.active_topics.is_empty() {
        score += 3.0;
    }
    // Feedback: min(count / 2, 4.5)
    let feedback_pts = (ctx.feedback_interaction_count as f64 / 2.0).min(4.5);
    score += feedback_pts;
    (score as u32).min(25)
}

pub(crate) fn compute_signal_score(audit: &SignalAudit) -> u32 {
    let mut score = 0u32;
    if audit.context_fires {
        score += 5;
    }
    if audit.interest_fires {
        score += 5;
    }
    if audit.ace_fires {
        score += 5;
    }
    if audit.learned_fires {
        score += 5;
    }
    if audit.dependency_fires {
        score += 5;
    }
    score.min(25)
}

pub(crate) fn compute_discrimination_score(probes: &ProbeResults) -> u32 {
    // F1 * 15 + separation_gap.clamp(0, 1) * 10
    let f1_pts = probes.f1 * 15.0;
    let sep_pts = probes.separation_gap.clamp(0.0, 1.0) * 10.0;
    ((f1_pts + sep_pts) as u32).min(25)
}

pub(crate) fn compute_grade_from_dimensions(
    infra: u32,
    context: u32,
    signal: u32,
    discrimination: u32,
) -> (String, u32) {
    let score = (infra + context + signal + discrimination).min(100);
    let grade = match score {
        90..=100 => "A",
        80..=89 => "B+",
        70..=79 => "B",
        60..=69 => "C+",
        50..=59 => "C",
        40..=49 => "D",
        _ => "F",
    }
    .to_string();
    (grade, score)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_probes_has_28_items() {
        assert_eq!(all_calibration_probes().len(), 28);
    }

    #[test]
    fn probes_per_domain_count() {
        let probes = all_calibration_probes();
        for domain in [
            Domain::Systems,
            Domain::Web,
            Domain::Ml,
            Domain::Devops,
            Domain::Mobile,
        ] {
            let count = probes.iter().filter(|p| p.domain == domain).count();
            assert_eq!(count, 4, "domain {:?} should have 4 probes", domain);
        }
        let universal = probes
            .iter()
            .filter(|p| p.domain == Domain::Universal)
            .count();
        assert_eq!(universal, 3);
        let noise = probes
            .iter()
            .filter(|p| p.domain == Domain::PureNoise)
            .count();
        assert_eq!(noise, 5);
    }

    #[test]
    fn probes_all_have_content() {
        for p in all_calibration_probes() {
            assert!(!p.title.is_empty());
            assert!(!p.content.is_empty());
        }
    }

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
    fn infrastructure_score_nothing() {
        let rig = RigRequirements {
            ollama_running: false,
            ollama_url: String::new(),
            embedding_model: None,
            embedding_available: false,
            gpu_detected: false,
            recommended_model: String::new(),
            estimated_ram_gb: 0.0,
            can_reach_grade_a: false,
            grade_a_requirements: vec![],
        };
        assert_eq!(compute_infrastructure_score(&rig), 0);
    }

    #[test]
    fn infrastructure_score_full() {
        let rig = RigRequirements {
            ollama_running: true,
            ollama_url: String::new(),
            embedding_model: Some("nomic".into()),
            embedding_available: true,
            gpu_detected: true,
            recommended_model: String::new(),
            estimated_ram_gb: 0.0,
            can_reach_grade_a: true,
            grade_a_requirements: vec![],
        };
        assert_eq!(compute_infrastructure_score(&rig), 25);
    }

    #[test]
    fn context_score_empty() {
        let ctx = ScoringContext::builder().build();
        assert_eq!(compute_context_score(&ctx), 0);
    }

    #[test]
    fn context_score_3_interests() {
        let ctx = ScoringContext::builder().interest_count(3).build();
        // 3 * 2.5 = 7.5 → 7
        assert_eq!(compute_context_score(&ctx), 7);
    }

    #[test]
    fn signal_score_zero_axes() {
        let audit = SignalAudit {
            axes: vec![],
            context_fires: false,
            interest_fires: false,
            ace_fires: false,
            learned_fires: false,
            dependency_fires: false,
        };
        assert_eq!(compute_signal_score(&audit), 0);
    }

    #[test]
    fn signal_score_all_axes() {
        let audit = SignalAudit {
            axes: vec![
                "context".into(),
                "interest".into(),
                "ace".into(),
                "learned".into(),
                "dependency".into(),
            ],
            context_fires: true,
            interest_fires: true,
            ace_fires: true,
            learned_fires: true,
            dependency_fires: true,
        };
        assert_eq!(compute_signal_score(&audit), 25);
    }

    #[test]
    fn discrimination_score_perfect() {
        let probes = ProbeResults {
            f1: 1.0,
            precision: 1.0,
            recall: 1.0,
            separation_gap: 1.0,
            passed: 12,
            total: 12,
            failures: vec![],
        };
        assert_eq!(compute_discrimination_score(&probes), 25);
    }

    #[test]
    fn discrimination_score_zero() {
        let probes = ProbeResults {
            f1: 0.0,
            precision: 0.0,
            recall: 0.0,
            separation_gap: 0.0,
            passed: 0,
            total: 12,
            failures: vec![],
        };
        assert_eq!(compute_discrimination_score(&probes), 0);
    }

    #[test]
    fn grade_from_dimensions_a() {
        let (grade, score) = compute_grade_from_dimensions(25, 25, 25, 25);
        assert_eq!(grade, "A");
        assert_eq!(score, 100);
    }

    #[test]
    fn grade_from_dimensions_f() {
        let (grade, score) = compute_grade_from_dimensions(0, 0, 0, 0);
        assert_eq!(grade, "F");
        assert_eq!(score, 0);
    }

    #[test]
    fn grade_from_dimensions_zero_setup() {
        // No infra, no context, no signal, small discrimination from keyword matching
        let (grade, score) = compute_grade_from_dimensions(0, 0, 0, 5);
        assert_eq!(grade, "F");
        assert_eq!(score, 5);
    }

    #[test]
    fn grade_from_dimensions_moderate() {
        // Ollama+embed(20) + 3 interests(7) + 2 axes(10) + ok discrimination(12)
        let (grade, score) = compute_grade_from_dimensions(20, 7, 10, 12);
        assert_eq!(score, 49);
        assert_eq!(grade, "D");
    }

    #[test]
    fn domain_name_coverage() {
        assert_eq!(domain_name(Domain::Systems), "systems");
        assert_eq!(domain_name(Domain::Web), "web");
        assert_eq!(domain_name(Domain::Ml), "ml");
        assert_eq!(domain_name(Domain::Devops), "devops");
        assert_eq!(domain_name(Domain::Mobile), "mobile");
    }
}
