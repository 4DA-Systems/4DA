// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Data-driven user-role inference from the detected tech stack.
//!
//! Replaces the old hardcoded cascade in `context.rs` (tauri -> desktop,
//! react -> frontend, rust -> backend, everything else -> "developer") which
//! left ML, data, security, DevOps, mobile, game, and embedded personas
//! falling through to a generic role. Each persona is a declarative
//! `RoleProfile` scored by distinct whole-token signal hits against the
//! user's stack; the highest-priority profile meeting its `min_hits` wins,
//! with ties (equal priority) broken by hit count.
//!
//! Matching is case-insensitive and whole-token: stack entries are already
//! discrete tech names ("react-native" is one token and must NOT count as a
//! "react" hit), so no substring matching is performed.

use std::collections::HashSet;

/// A declarative role profile: which tech signals imply the role, how many
/// distinct hits are required, and how it ranks against competing profiles.
struct RoleProfile {
    role: &'static str,
    signals: &'static [&'static str],
    min_hits: usize,
    priority: u8,
}

/// Profiles ordered by priority (informational; selection is priority-based,
/// not order-based). Priorities are all distinct so selection is stable.
const PROFILES: &[RoleProfile] = &[
    RoleProfile {
        role: "desktop_app_developer",
        signals: &[
            "tauri",
            "electron",
            "wails",
            "qt",
            "gtk",
            "winui",
            "swiftui-mac",
        ],
        min_hits: 1,
        priority: 100,
    },
    RoleProfile {
        role: "security_engineer",
        signals: &[
            "metasploit",
            "burp",
            "burpsuite",
            "nmap",
            "wireshark",
            "ghidra",
            "frida",
            "yara",
            "semgrep",
            "osquery",
        ],
        min_hits: 1,
        priority: 90,
    },
    RoleProfile {
        role: "embedded_developer",
        signals: &[
            "embassy",
            "rtic",
            "zephyr",
            "freertos",
            "esp-idf",
            "arduino",
            "no_std",
            "no-std",
            "platformio",
        ],
        min_hits: 1,
        priority: 85,
    },
    RoleProfile {
        role: "game_developer",
        signals: &[
            "unity", "unreal", "godot", "bevy", "raylib", "phaser", "love2d",
        ],
        min_hits: 1,
        priority: 80,
    },
    RoleProfile {
        role: "ml_engineer",
        signals: &[
            "pytorch",
            "tensorflow",
            "scikit-learn",
            "sklearn",
            "keras",
            "transformers",
            "huggingface",
            "jax",
            "xgboost",
            "lightgbm",
            "onnx",
        ],
        min_hits: 1,
        priority: 75,
    },
    RoleProfile {
        role: "mobile_developer",
        signals: &[
            "react-native",
            "flutter",
            "swift",
            "kotlin",
            "swiftui",
            "jetpack-compose",
            "ionic",
            "expo",
            "android",
            "ios",
        ],
        min_hits: 1,
        priority: 70,
    },
    // Priority deliberately below ml_engineer: pandas/jupyter are common in
    // ML stacks too, so pytorch+pandas resolves to ml_engineer.
    RoleProfile {
        role: "data_engineer",
        signals: &[
            "spark",
            "airflow",
            "dbt",
            "kafka",
            "flink",
            "snowflake",
            "databricks",
            "pandas",
            "polars",
            "duckdb",
            "jupyter",
        ],
        min_hits: 1,
        priority: 65,
    },
    // min_hits 2: docker alone is ubiquitous across every persona.
    RoleProfile {
        role: "devops_engineer",
        signals: &[
            "kubernetes",
            "docker",
            "terraform",
            "ansible",
            "helm",
            "prometheus",
            "grafana",
            "pulumi",
            "nomad",
            "argocd",
        ],
        min_hits: 2,
        priority: 60,
    },
    // fullstack_developer (priority 50) is a special two-bucket rule below.
    RoleProfile {
        role: "frontend_developer",
        signals: &[
            "react", "vue", "svelte", "angular", "solid", "nextjs", "next.js", "nuxt",
        ],
        min_hits: 1,
        priority: 40,
    },
    RoleProfile {
        role: "backend_developer",
        signals: &[
            "rust", "go", "python", "java", "ruby", "php", "elixir", "axum", "django", "rails",
            "spring", "csharp", "c#", "dotnet", ".net", "asp.net", "laravel", "symfony",
        ],
        min_hits: 1,
        priority: 30,
    },
];

/// Fullstack two-bucket rule: at least one frontend AND one backend signal.
const FULLSTACK_PRIORITY: u8 = 50;
const FULLSTACK_FRONTEND: &[&str] = &[
    "react", "vue", "svelte", "angular", "solid", "nextjs", "next.js", "nuxt",
];
const FULLSTACK_BACKEND: &[&str] = &[
    "express", "fastify", "nestjs", "axum", "actix", "rocket", "django", "flask", "fastapi",
    "rails", "laravel", "spring", "gin", "fiber",
];

fn count_hits(stack_lower: &HashSet<String>, signals: &[&str]) -> usize {
    signals.iter().filter(|s| stack_lower.contains(**s)).count()
}

/// Infer the user's professional role from their detected tech stack.
///
/// Returns `None` when no profile matches; callers keep their existing
/// generic "developer" fallback.
pub(crate) fn infer_role(stack: &[String]) -> Option<String> {
    if stack.is_empty() {
        return None;
    }
    let stack_lower: HashSet<String> = stack.iter().map(|s| s.trim().to_lowercase()).collect();

    // (priority, hits, role) - replaced only when strictly greater, so equal
    // candidates keep the earlier (deterministic) winner.
    let mut best: Option<(u8, usize, &'static str)> = None;
    let mut consider = |priority: u8, hits: usize, role: &'static str| {
        if best.is_none_or(|(bp, bh, _)| (priority, hits) > (bp, bh)) {
            best = Some((priority, hits, role));
        }
    };

    for profile in PROFILES {
        let hits = count_hits(&stack_lower, profile.signals);
        if hits >= profile.min_hits {
            consider(profile.priority, hits, profile.role);
        }
    }

    // Fullstack: one frontend signal AND one backend signal.
    let fe_hits = count_hits(&stack_lower, FULLSTACK_FRONTEND);
    let be_hits = count_hits(&stack_lower, FULLSTACK_BACKEND);
    if fe_hits >= 1 && be_hits >= 1 {
        consider(FULLSTACK_PRIORITY, fe_hits + be_hits, "fullstack_developer");
    }

    best.map(|(_, _, role)| role.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn role(items: &[&str]) -> Option<String> {
        infer_role(&stack(items))
    }

    // -- Founder regression: tauri must still yield desktop_app_developer --

    #[test]
    fn founder_tauri_stack_is_desktop() {
        assert_eq!(
            role(&["tauri", "rust", "react", "typescript", "sqlite"]).as_deref(),
            Some("desktop_app_developer"),
            "a stack containing tauri must keep resolving to desktop_app_developer"
        );
    }

    // -- One test per profile --

    #[test]
    fn desktop_profile() {
        assert_eq!(
            role(&["electron"]).as_deref(),
            Some("desktop_app_developer")
        );
    }

    #[test]
    fn mobile_profile() {
        assert_eq!(role(&["flutter"]).as_deref(), Some("mobile_developer"));
        assert_eq!(role(&["swift", "ios"]).as_deref(), Some("mobile_developer"));
    }

    #[test]
    fn ml_profile() {
        assert_eq!(role(&["pytorch"]).as_deref(), Some("ml_engineer"));
        assert_eq!(
            role(&["scikit-learn", "python"]).as_deref(),
            Some("ml_engineer"),
            "ml signal must outrank the backend python signal"
        );
    }

    #[test]
    fn data_profile() {
        assert_eq!(role(&["airflow", "dbt"]).as_deref(), Some("data_engineer"));
        assert_eq!(
            role(&["pandas", "jupyter"]).as_deref(),
            Some("data_engineer"),
            "pandas/jupyter without ML frameworks is a data stack"
        );
    }

    #[test]
    fn devops_profile_requires_two_hits() {
        assert_eq!(
            role(&["docker"]),
            None,
            "docker alone is ubiquitous and must not classify"
        );
        assert_eq!(
            role(&["docker", "kubernetes"]).as_deref(),
            Some("devops_engineer")
        );
    }

    #[test]
    fn security_profile() {
        assert_eq!(role(&["nmap"]).as_deref(), Some("security_engineer"));
        assert_eq!(
            role(&["metasploit", "wireshark", "python"]).as_deref(),
            Some("security_engineer"),
            "security tooling must outrank the backend python signal"
        );
    }

    #[test]
    fn game_profile() {
        assert_eq!(role(&["godot"]).as_deref(), Some("game_developer"));
        assert_eq!(
            role(&["bevy", "rust"]).as_deref(),
            Some("game_developer"),
            "game engine must outrank the backend rust signal"
        );
    }

    #[test]
    fn embedded_profile() {
        assert_eq!(role(&["zephyr"]).as_deref(), Some("embedded_developer"));
        assert_eq!(
            role(&["embassy", "no_std", "rust"]).as_deref(),
            Some("embedded_developer")
        );
    }

    #[test]
    fn fullstack_two_bucket_rule() {
        assert_eq!(
            role(&["react", "express"]).as_deref(),
            Some("fullstack_developer")
        );
        assert_eq!(
            role(&["vue", "fastapi"]).as_deref(),
            Some("fullstack_developer")
        );
        // Frontend-only or backend-only must NOT be fullstack.
        assert_eq!(role(&["react"]).as_deref(), Some("frontend_developer"));
        assert_eq!(role(&["express"]), None, "express alone matches no profile");
    }

    #[test]
    fn frontend_profile() {
        assert_eq!(role(&["svelte"]).as_deref(), Some("frontend_developer"));
    }

    #[test]
    fn backend_profile() {
        assert_eq!(role(&["rust", "go"]).as_deref(), Some("backend_developer"));
        assert_eq!(role(&["python"]).as_deref(), Some("backend_developer"));
    }

    // -- Tie-breaks and matching semantics --

    #[test]
    fn priority_breaks_equal_hit_ties() {
        // pytorch (ml, 1 hit) vs pandas (data, 1 hit): ml_engineer has the
        // higher priority so a mixed ML/data stack resolves to ml_engineer.
        assert_eq!(role(&["pytorch", "pandas"]).as_deref(), Some("ml_engineer"));
    }

    #[test]
    fn whole_token_match_no_substrings() {
        // "react-native" is a single discrete token: it must hit mobile,
        // never frontend via a "react" substring.
        assert_eq!(role(&["react-native"]).as_deref(), Some("mobile_developer"));
    }

    #[test]
    fn case_insensitive_matching() {
        assert_eq!(
            role(&["PyTorch", "TensorFlow"]).as_deref(),
            Some("ml_engineer")
        );
        assert_eq!(
            role(&["  Tauri  "]).as_deref(),
            Some("desktop_app_developer")
        );
    }

    #[test]
    fn empty_stack_returns_none() {
        assert_eq!(infer_role(&[]), None);
    }

    #[test]
    fn unknown_stack_returns_none() {
        assert_eq!(role(&["cobol", "fortran"]), None);
    }
}
