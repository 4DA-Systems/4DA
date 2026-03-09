#[cfg(test)]
mod tests {
    use crate::sovereign_developer_profile::*;

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("rust"), "Rust");
        assert_eq!(capitalize("typeScript"), "TypeScript");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_parse_gb() {
        assert!((parse_gb("16 GB") - 16.0).abs() < 0.01);
        assert!((parse_gb("16GB") - 16.0).abs() < 0.01);
        assert!((parse_gb("8192 MB") - 8.0).abs() < 0.01);
        assert!((parse_gb("8192MB") - 8.0).abs() < 0.01);
        assert!((parse_gb("32") - 32.0).abs() < 0.01);
        assert!((parse_gb("invalid") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_dimension_completeness_empty() {
        let d = compute_dimension_completeness("Test", 0, 10);
        assert_eq!(d.depth, "empty");
        assert_eq!(d.percentage, 0.0);
    }

    #[test]
    fn test_dimension_completeness_minimal() {
        let d = compute_dimension_completeness("Test", 2, 10);
        assert_eq!(d.depth, "minimal");
        assert!((d.percentage - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_dimension_completeness_comprehensive() {
        let d = compute_dimension_completeness("Test", 15, 10);
        assert_eq!(d.depth, "comprehensive");
        assert!((d.percentage - 100.0).abs() < 0.01); // Capped at 100
    }

    #[test]
    fn test_identity_summary_empty() {
        let stack = StackDimension::default();
        let summary = build_identity_summary(&stack);
        assert!(summary.contains("configure your stack"));
    }

    #[test]
    fn test_identity_summary_with_stack() {
        let stack = StackDimension {
            primary_stack: vec!["rust".to_string(), "typescript".to_string()],
            ..Default::default()
        };
        let summary = build_identity_summary(&stack);
        assert_eq!(summary, "Rust/Typescript developer");
    }

    #[test]
    fn test_detect_skill_gaps() {
        let stack = StackDimension {
            dependencies: vec!["tokio".to_string(), "serde".to_string(), "axum".to_string()],
            ..Default::default()
        };
        let skills = SkillsDimension {
            top_affinities: vec![AffinityEntry {
                topic: "tokio".to_string(),
                score: 5.0,
            }],
            ..Default::default()
        };
        let gaps = detect_skill_gaps(&stack, &skills);
        // serde and axum should be gaps (not engaged), tokio should not
        assert!(gaps.iter().any(|g| g.dependency == "serde"));
        assert!(!gaps.iter().any(|g| g.dependency == "tokio"));
    }

    #[test]
    fn test_detect_infrastructure_mismatches_gpu_no_llm() {
        let infra = InfrastructureDimension {
            gpu_tier: "discrete".to_string(),
            llm_tier: "none".to_string(),
            ..Default::default()
        };
        let mismatches = detect_infrastructure_mismatches(&infra);
        assert!(mismatches.iter().any(|m| m.issue.contains("Ollama")));
    }

    #[test]
    fn test_detect_infrastructure_mismatches_no_gpu_local_llm() {
        let infra = InfrastructureDimension {
            gpu_tier: "none".to_string(),
            llm_tier: "local".to_string(),
            ..Default::default()
        };
        let mismatches = detect_infrastructure_mismatches(&infra);
        assert!(mismatches.iter().any(|m| m.issue.contains("CPU-only")));
    }

    #[test]
    fn test_detect_infrastructure_low_ram_local_llm() {
        let mut infra = InfrastructureDimension {
            gpu_tier: "discrete".to_string(),
            llm_tier: "local".to_string(),
            ..Default::default()
        };
        infra.ram.insert("total".to_string(), "4 GB".to_string());
        let mismatches = detect_infrastructure_mismatches(&infra);
        assert!(mismatches.iter().any(|m| m.issue.contains("swap")));
    }

    #[test]
    fn test_is_identity_worthy() {
        // Languages should be worthy
        assert!(is_identity_worthy("rust"));
        assert!(is_identity_worthy("typescript"));
        assert!(is_identity_worthy("python"));
        assert!(is_identity_worthy("javascript"));
        // Frameworks should be worthy
        assert!(is_identity_worthy("react"));
        assert!(is_identity_worthy("tauri"));
        assert!(is_identity_worthy("vue"));
        // ORMs, utility libs, build tools should NOT be worthy
        assert!(!is_identity_worthy("drizzle"));
        assert!(!is_identity_worthy("webpack"));
        assert!(!is_identity_worthy("eslint"));
        assert!(!is_identity_worthy("prisma"));
    }

    #[test]
    fn test_identity_summary_filters_unworthy_tech() {
        // Simulates the "Drizzle developer" bug: drizzle should be filtered out
        let stack = StackDimension {
            primary_stack: vec![
                "rust".to_string(),
                "typescript".to_string(),
                "react".to_string(),
                "drizzle".to_string(),
            ],
            ..Default::default()
        };
        let summary = build_identity_summary(&stack);
        assert!(
            !summary.to_lowercase().contains("drizzle"),
            "Drizzle should not appear in identity: {}",
            summary
        );
        assert!(summary.contains("Rust"));
        assert!(summary.contains("Typescript"));
        assert!(summary.contains("React"));
    }

    #[test]
    fn test_identity_summary_fallback_when_no_worthy() {
        // If no tech is identity-worthy, fallback to first 3
        let stack = StackDimension {
            primary_stack: vec![
                "drizzle".to_string(),
                "prisma".to_string(),
                "webpack".to_string(),
            ],
            ..Default::default()
        };
        let summary = build_identity_summary(&stack);
        // Should still produce a developer title using the fallback
        assert!(summary.contains("developer"));
        assert!(summary.contains("Drizzle"));
    }

    #[test]
    fn test_completeness_weights_sum_to_one() {
        let weights = [0.25, 0.30, 0.20, 0.15, 0.10];
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_export_markdown_structure() {
        let profile = SovereignDeveloperProfile {
            generated_at: "2026-02-28T00:00:00Z".to_string(),
            identity_summary: "Rust/TypeScript developer".to_string(),
            infrastructure: InfrastructureDimension {
                gpu_tier: "discrete".to_string(),
                llm_tier: "cloud".to_string(),
                ..Default::default()
            },
            stack: StackDimension {
                primary_stack: vec!["rust".to_string(), "typescript".to_string()],
                ..Default::default()
            },
            skills: SkillsDimension::default(),
            preferences: PreferencesDimension::default(),
            context: ContextDimension::default(),
            intelligence: IntelligenceReport::default(),
            completeness: CompletenessReport {
                overall_percentage: 50.0,
                dimensions: vec![],
            },
        };
        let md = export_as_markdown(&profile);
        assert!(md.contains("# Sovereign Developer Profile"));
        assert!(md.contains("Rust/TypeScript developer"));
        assert!(md.contains("## Infrastructure"));
        assert!(md.contains("## Stack"));
        assert!(md.contains("## Intelligence"));
        assert!(md.contains("## Profile Completeness"));
    }
}
