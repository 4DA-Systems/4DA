//! Calibration Probe Corpus — 28 domain-aware items across 5 tech domains,
//! universal developer probes, and pure noise controls.

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
    fn domain_name_coverage() {
        assert_eq!(domain_name(Domain::Systems), "systems");
        assert_eq!(domain_name(Domain::Web), "web");
        assert_eq!(domain_name(Domain::Ml), "ml");
        assert_eq!(domain_name(Domain::Devops), "devops");
        assert_eq!(domain_name(Domain::Mobile), "mobile");
    }
}
