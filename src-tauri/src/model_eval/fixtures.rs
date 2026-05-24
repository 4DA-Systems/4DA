// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Eval fixtures — synthetic signal sets with known-good and known-bad patterns.
//!
//! Each fixture feeds a set of signals through the synthesis pipeline and checks
//! the output against forbidden patterns (hallucinated entities, invented versions,
//! false security claims) and required patterns (grounded terms, evidence citation).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalFixture {
    pub name: &'static str,
    pub description: &'static str,
    pub signals: Vec<EvalSignal>,
    pub forbidden_patterns: Vec<ForbiddenPattern>,
    pub required_patterns: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalSignal {
    pub title: String,
    pub source_type: String,
    pub description: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ForbiddenPattern {
    pub pattern: &'static str,
    pub reason: &'static str,
    pub severity: PatternSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PatternSeverity {
    Critical,
    High,
    Medium,
}

pub(crate) fn all_fixtures() -> Vec<EvalFixture> {
    vec![
        fixture_security_grounding(),
        fixture_version_accuracy(),
        fixture_dependency_claims(),
        fixture_multi_topic_clustering(),
        fixture_low_signal_abstention(),
    ]
}

fn fixture_security_grounding() -> EvalFixture {
    EvalFixture {
        name: "security_grounding",
        description: "Verifies the model does not fabricate CVE IDs or severity ratings",
        signals: vec![
            EvalSignal {
                title: "React 19.2 released with concurrent rendering improvements".into(),
                source_type: "hackernews".into(),
                description: "New React release focuses on performance and developer experience. Concurrent rendering is now stable.".into(),
                score: 0.85,
            },
            EvalSignal {
                title: "Node.js v22.5.0 security patch for HTTP/2 vulnerability".into(),
                source_type: "github_releases".into(),
                description: "Fixes a denial-of-service vulnerability in the HTTP/2 implementation. Update recommended for all production deployments.".into(),
                score: 0.92,
            },
            EvalSignal {
                title: "Rust 1.82 stabilizes async closures".into(),
                source_type: "reddit".into(),
                description: "The Rust team has stabilized async closures in the 1.82 release, enabling more ergonomic async programming patterns.".into(),
                score: 0.78,
            },
        ],
        forbidden_patterns: vec![
            ForbiddenPattern {
                pattern: "CVE-",
                reason: "No CVE IDs appear in the source signals — any CVE reference is hallucinated",
                severity: PatternSeverity::Critical,
            },
            ForbiddenPattern {
                pattern: "critical vulnerability",
                reason: "The Node.js signal says 'denial-of-service', not 'critical' — severity escalation is hallucinated",
                severity: PatternSeverity::High,
            },
            ForbiddenPattern {
                pattern: "remote code execution",
                reason: "No RCE mentioned in any signal — this is a common hallucination pattern",
                severity: PatternSeverity::Critical,
            },
        ],
        required_patterns: vec![
            "React",
            "Node",
        ],
    }
}

fn fixture_version_accuracy() -> EvalFixture {
    EvalFixture {
        name: "version_accuracy",
        description: "Verifies the model does not invent version numbers",
        signals: vec![
            EvalSignal {
                title: "Vite 6.1 released with improved SSR support".into(),
                source_type: "hackernews".into(),
                description: "Vite 6.1 brings better server-side rendering, faster HMR, and reduced memory usage during development builds.".into(),
                score: 0.88,
            },
            EvalSignal {
                title: "Deno 2.3 adds npm workspaces support".into(),
                source_type: "reddit".into(),
                description: "Deno 2.3 now supports npm workspaces natively, making monorepo migration from Node.js easier.".into(),
                score: 0.75,
            },
        ],
        forbidden_patterns: vec![
            ForbiddenPattern {
                pattern: "Vite 7",
                reason: "Only Vite 6.1 is mentioned in signals — Vite 7 is hallucinated",
                severity: PatternSeverity::High,
            },
            ForbiddenPattern {
                pattern: "Deno 3",
                reason: "Only Deno 2.3 is mentioned — Deno 3 is hallucinated",
                severity: PatternSeverity::High,
            },
            ForbiddenPattern {
                pattern: "breaking change",
                reason: "No breaking changes mentioned in any signal",
                severity: PatternSeverity::Medium,
            },
        ],
        required_patterns: vec![
            "Vite",
            "Deno",
        ],
    }
}

fn fixture_dependency_claims() -> EvalFixture {
    EvalFixture {
        name: "dependency_claims",
        description: "Verifies the model does not fabricate dependency relationships",
        signals: vec![
            EvalSignal {
                title: "SQLite 3.48 adds JSON5 support".into(),
                source_type: "hackernews".into(),
                description: "SQLite now parses JSON5 in its json_extract functions, bringing comments and trailing commas to JSON columns.".into(),
                score: 0.82,
            },
            EvalSignal {
                title: "Tauri 2.2 improves plugin system".into(),
                source_type: "github_releases".into(),
                description: "Tauri 2.2 introduces a simplified plugin API and better TypeScript bindings for IPC commands.".into(),
                score: 0.90,
            },
        ],
        forbidden_patterns: vec![
            ForbiddenPattern {
                pattern: "depends on",
                reason: "No dependency relationships stated in signals — inferring them is hallucination",
                severity: PatternSeverity::High,
            },
            ForbiddenPattern {
                pattern: "requires",
                reason: "No requirements mentioned between these tools",
                severity: PatternSeverity::Medium,
            },
            ForbiddenPattern {
                pattern: "migration",
                reason: "No migration mentioned in signals — this is a common filler pattern",
                severity: PatternSeverity::Medium,
            },
        ],
        required_patterns: vec![
            "SQLite",
            "Tauri",
        ],
    }
}

fn fixture_multi_topic_clustering() -> EvalFixture {
    EvalFixture {
        name: "multi_topic_clustering",
        description: "Verifies the model separates unrelated topics into distinct clusters",
        signals: vec![
            EvalSignal {
                title: "PyTorch 2.5 introduces torch.compile improvements".into(),
                source_type: "hackernews".into(),
                description: "Major compilation speed improvements for PyTorch models, reducing cold-start compilation time by 40%.".into(),
                score: 0.91,
            },
            EvalSignal {
                title: "CSS anchor positioning shipped in Chrome 131".into(),
                source_type: "reddit".into(),
                description: "CSS anchor positioning is now available without flags in Chrome 131, enabling popover and tooltip positioning without JavaScript.".into(),
                score: 0.73,
            },
            EvalSignal {
                title: "Go 1.24 adds range-over-func iterators".into(),
                source_type: "hackernews".into(),
                description: "Go 1.24 stabilizes range-over-function iterators, the most requested language feature since generics.".into(),
                score: 0.85,
            },
        ],
        forbidden_patterns: vec![
            ForbiddenPattern {
                pattern: "AI and web development",
                reason: "These topics are unrelated — lumping them is lazy synthesis",
                severity: PatternSeverity::Medium,
            },
        ],
        required_patterns: vec![
            "PyTorch",
            "CSS",
            "Go",
        ],
    }
}

fn fixture_low_signal_abstention() -> EvalFixture {
    EvalFixture {
        name: "low_signal_abstention",
        description: "Verifies the model produces minimal output when signals are weak",
        signals: vec![EvalSignal {
            title: "Minor typo fix in lodash docs".into(),
            source_type: "github".into(),
            description: "Fixed a typo in the README.".into(),
            score: 0.15,
        }],
        forbidden_patterns: vec![
            ForbiddenPattern {
                pattern: "significant",
                reason: "A docs typo fix is not significant — model should not inflate importance",
                severity: PatternSeverity::High,
            },
            ForbiddenPattern {
                pattern: "recommend",
                reason: "A typo fix warrants no action recommendation",
                severity: PatternSeverity::Medium,
            },
            ForbiddenPattern {
                pattern: "security",
                reason: "Nothing security-related in the signal",
                severity: PatternSeverity::Critical,
            },
        ],
        required_patterns: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fixtures_are_valid() {
        let fixtures = all_fixtures();
        assert!(
            fixtures.len() >= 4,
            "Need at least 4 fixtures for meaningful eval"
        );
        for f in &fixtures {
            assert!(!f.name.is_empty());
            assert!(!f.signals.is_empty() || f.name == "low_signal_abstention");
            assert!(
                !f.forbidden_patterns.is_empty(),
                "Fixture '{}' needs at least one forbidden pattern",
                f.name
            );
        }
    }

    #[test]
    fn fixture_names_are_unique() {
        let fixtures = all_fixtures();
        let names: Vec<&str> = fixtures.iter().map(|f| f.name).collect();
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len(), "Duplicate fixture names");
    }

    #[test]
    fn forbidden_patterns_have_reasons() {
        for f in all_fixtures() {
            for fp in &f.forbidden_patterns {
                assert!(
                    !fp.reason.is_empty(),
                    "Fixture '{}': forbidden pattern '{}' has no reason",
                    f.name,
                    fp.pattern
                );
            }
        }
    }
}
