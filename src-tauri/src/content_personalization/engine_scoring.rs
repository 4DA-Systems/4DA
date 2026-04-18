// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Engine scoring helpers — static engine definitions and scoring computations.
//!
//! Contains the `EngineSpec` definitions, stack/hardware scoring, GB parsing,
//! and insight confidence/source inference functions used by the NoLLM engine.

use super::context::PersonalizationContext;

// ============================================================================
// Static Data: Engine Requirements
// ============================================================================

/// Revenue engine definitions with hardware/stack requirements.
pub(crate) struct EngineSpec {
    pub id: u8,
    pub name: &'static str,
    pub min_ram_gb: f64,
    pub needs_gpu: bool,
    pub prereq_stacks: &'static [&'static str],
    pub topics: &'static [&'static str],
}

pub(crate) const ENGINES: &[EngineSpec] = &[
    EngineSpec {
        id: 1,
        name: "Open Source + Sponsorship",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &["git"],
        topics: &["open-source", "github", "community"],
    },
    EngineSpec {
        id: 2,
        name: "Technical Writing",
        min_ram_gb: 4.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["writing", "blog", "documentation", "tutorial"],
    },
    EngineSpec {
        id: 3,
        name: "Freelance / Consulting",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["freelance", "consulting", "client", "contract"],
    },
    EngineSpec {
        id: 4,
        name: "SaaS / Micro-SaaS",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &["react", "typescript", "javascript", "python", "node"],
        topics: &["saas", "startup", "product", "subscription"],
    },
    EngineSpec {
        id: 5,
        name: "Developer Tools",
        min_ram_gb: 16.0,
        needs_gpu: false,
        prereq_stacks: &["rust", "go", "typescript", "python"],
        topics: &["devtools", "cli", "sdk", "api", "tooling"],
    },
    EngineSpec {
        id: 6,
        name: "Education / Courses",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["education", "course", "teaching", "workshop"],
    },
    EngineSpec {
        id: 7,
        name: "API / Infrastructure",
        min_ram_gb: 16.0,
        needs_gpu: true,
        prereq_stacks: &["python", "rust", "go", "typescript"],
        topics: &["api", "infrastructure", "cloud", "backend"],
    },
    EngineSpec {
        id: 8,
        name: "Productized Services",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["service", "automation", "productized"],
    },
];

// ============================================================================
// Scoring Helpers
// ============================================================================

/// Score how well the user's stack matches an engine's prerequisites.
pub(crate) fn engine_stack_score(engine: &EngineSpec, ctx: &PersonalizationContext) -> f64 {
    if engine.prereq_stacks.is_empty() {
        return 0.5; // No specific stack required
    }

    let all_tech: Vec<String> = ctx
        .stack
        .primary
        .iter()
        .chain(ctx.stack.adjacent.iter())
        .map(|s| s.to_lowercase())
        .collect();

    let matched = engine
        .prereq_stacks
        .iter()
        .filter(|req| all_tech.iter().any(|t| t.contains(*req)))
        .count();

    (matched as f64 / engine.prereq_stacks.len() as f64).min(1.0)
}

/// Score how well the user's hardware matches an engine's requirements.
pub(crate) fn engine_hardware_score(engine: &EngineSpec, ctx: &PersonalizationContext) -> f64 {
    let ram_str = ctx.profile.ram.get("total").cloned().unwrap_or_default();
    let ram_gb = parse_gb(&ram_str);

    let mut score = 1.0;

    // RAM check
    if ram_gb > 0.0 && ram_gb < engine.min_ram_gb {
        score *= ram_gb / engine.min_ram_gb;
    }

    // GPU check
    if engine.needs_gpu && ctx.computed.gpu_tier == "none" {
        score *= 0.5;
    }

    score.min(1.0)
}

/// Parse a string like "32 GB" or "16384 MB" into GB as f64.
pub(crate) fn parse_gb(s: &str) -> f64 {
    let s = s.trim().to_lowercase();
    if let Some(gb) = s.strip_suffix("gb") {
        gb.trim().parse().unwrap_or(0.0)
    } else if let Some(mb) = s.strip_suffix("mb") {
        mb.trim().parse::<f64>().unwrap_or(0.0) / 1024.0
    } else if let Some(tb) = s.strip_suffix("tb") {
        tb.trim().parse::<f64>().unwrap_or(0.0) * 1024.0
    } else {
        // Try parsing as plain number (assume GB)
        s.split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0.0)
    }
}

/// RAM tier description for context.
pub(crate) fn ram_tier_context(gb: f64) -> String {
    if gb >= 64.0 {
        "Workstation-grade — can run multiple large models simultaneously".into()
    } else if gb >= 32.0 {
        "Strong — comfortable for most local AI workloads".into()
    } else if gb >= 16.0 {
        "Adequate — can run small-medium local models".into()
    } else if gb >= 8.0 {
        "Minimum for development — cloud LLM recommended".into()
    } else {
        "Limited — cloud LLM strongly recommended".into()
    }
}

/// Infer which data sources contributed to a given insight card.
pub(crate) fn infer_sources(marker_id: &str, ctx: &PersonalizationContext) -> Vec<String> {
    match marker_id {
        "hardware_benchmark" => {
            let mut sources = vec!["Sovereign Profile".to_string()];
            if ctx.settings.has_llm {
                sources.push("LLM Config".into());
            }
            sources
        }
        "stack_fit" => vec!["Domain Profile".into(), "Tech Stack".into()],
        "cost_projection" => vec!["Regional Data".into(), "Sovereign Profile".into()],
        "t_shape" => vec!["Domain Profile".into(), "ACE Detection".into()],
        "engine_ranking" => vec![
            "Domain Profile".into(),
            "Sovereign Profile".into(),
            "Tech Radar".into(),
        ],
        "blind_spot_alert" => vec!["Developer DNA".into()],
        _ => vec!["Unknown".into()],
    }
}

/// Compute data coverage confidence for a given insight card.
pub(crate) fn compute_confidence(marker_id: &str, ctx: &PersonalizationContext) -> f64 {
    match marker_id {
        "hardware_benchmark" => {
            let mut score = 0.0;
            if !ctx.profile.cpu.is_empty() {
                score += 0.3;
            }
            if !ctx.profile.ram.is_empty() {
                score += 0.3;
            }
            if !ctx.profile.gpu.is_empty() {
                score += 0.4;
            }
            score
        }
        "stack_fit" | "engine_ranking" => {
            if ctx.stack.primary.len() >= 2 {
                0.9
            } else if !ctx.stack.primary.is_empty() {
                0.6
            } else {
                0.2
            }
        }
        "cost_projection" => {
            if ctx.regional.electricity_kwh > 0.0 {
                0.7
            } else {
                0.3
            }
        }
        "t_shape" => {
            if !ctx.stack.primary.is_empty() && !ctx.stack.adjacent.is_empty() {
                0.8
            } else if !ctx.stack.primary.is_empty() {
                0.5
            } else {
                0.1
            }
        }
        "blind_spot_alert" => {
            if ctx.dna.is_full {
                0.8
            } else {
                0.3
            }
        }
        _ => 0.5,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_personalization::context::*;

    fn test_ctx() -> PersonalizationContext {
        let mut ram = std::collections::HashMap::new();
        ram.insert("total".into(), "64 GB".into());

        let mut cpu = std::collections::HashMap::new();
        cpu.insert("model".into(), "AMD Ryzen 9 7950X".into());
        cpu.insert("cores".into(), "16".into());

        let mut gpu = std::collections::HashMap::new();
        gpu.insert("name".into(), "NVIDIA RTX 4090".into());
        gpu.insert("memory_total".into(), "24 GB".into());

        PersonalizationContext {
            profile: ProfileData {
                cpu,
                ram,
                gpu,
                ..Default::default()
            },
            stack: StackData {
                primary: vec!["rust".into(), "typescript".into()],
                adjacent: vec!["wasm".into(), "tauri".into(), "react".into()],
                ..Default::default()
            },
            radar: RadarData::default(),
            regional: RegionalData::default(),
            decisions: Vec::new(),
            progress: ProgressData::default(),
            settings: SettingsData {
                has_llm: true,
                llm_provider: "ollama".into(),
                llm_model: "llama3".into(),
            },
            dna: DnaData {
                is_full: true,
                primary_stack: vec!["rust".into()],
                blind_spots: vec!["kubernetes".into()],
                top_engaged_topics: vec!["api".into()],
                ..Default::default()
            },
            computed: ComputedFields {
                llm_tier: "local".into(),
                gpu_tier: "workstation".into(),
                has_nvidia: true,
                os_family: "windows".into(),
                profile_completeness: 66.0,
                monthly_electricity_estimate: 48.0,
            },
        }
    }

    #[test]
    fn test_stack_fit_scoring() {
        let ctx = test_ctx();
        // Engine 5 (Dev Tools) requires rust/go/ts/python — user has rust+ts
        let score = engine_stack_score(&ENGINES[4], &ctx);
        assert!(score >= 0.5); // At least 2/4 match
    }

    #[test]
    fn test_parse_gb() {
        assert_eq!(parse_gb("32 GB"), 32.0);
        assert_eq!(parse_gb("16384 MB"), 16.0);
        assert_eq!(parse_gb("1 TB"), 1024.0);
        assert_eq!(parse_gb("32"), 32.0);
        assert_eq!(parse_gb(""), 0.0);
    }

    #[test]
    fn test_confidence_scoring() {
        let ctx = test_ctx();
        let conf = compute_confidence("hardware_benchmark", &ctx);
        assert!(conf >= 0.9); // Has CPU + RAM + GPU
    }
}
