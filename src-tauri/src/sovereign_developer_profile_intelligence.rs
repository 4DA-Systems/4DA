// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Intelligence computation, completeness, export, and Tauri commands for
//! the Sovereign Developer Profile.

use std::collections::{HashMap, HashSet};

use super::*;

// ============================================================================
// Intelligence (pure computation, no side effects)
// ============================================================================

pub(super) fn compute_intelligence(
    infra: &InfrastructureDimension,
    stack: &StackDimension,
    skills: &SkillsDimension,
    _preferences: &PreferencesDimension,
) -> IntelligenceReport {
    let skill_gaps = detect_skill_gaps(stack, skills);
    let optimization_opportunities = detect_optimization_opportunities(stack);
    let infrastructure_mismatches = detect_infrastructure_mismatches(infra);
    let ecosystem_alerts = detect_ecosystem_alerts(stack);

    IntelligenceReport {
        skill_gaps,
        optimization_opportunities,
        infrastructure_mismatches,
        ecosystem_alerts,
    }
}

/// Dependencies used but never engaged with in content.
pub(super) fn detect_skill_gaps(stack: &StackDimension, skills: &SkillsDimension) -> Vec<SkillGap> {
    let engaged: HashSet<String> = skills
        .top_affinities
        .iter()
        .map(|a| a.topic.to_lowercase())
        .collect();

    let mut gaps = Vec::new();
    for dep in &stack.dependencies {
        let dep_lower = dep.to_lowercase();
        // Skip very short names (likely false positives)
        if dep_lower.len() < 4 {
            continue;
        }
        if !engaged.contains(&dep_lower) {
            gaps.push(SkillGap {
                dependency: dep.clone(),
                reason: "Dependency in your projects but no content engagement detected"
                    .to_string(),
            });
        }
    }
    // Limit to top 10
    gaps.truncate(10);
    gaps
}

/// Stack pain points matched to user's detected tech.
pub(super) fn detect_optimization_opportunities(
    stack: &StackDimension,
) -> Vec<OptimizationOpportunity> {
    let mut opportunities = Vec::new();
    let composed = crate::stacks::compose_profiles(&stack.selected_profiles);
    if !composed.active {
        return opportunities;
    }

    let user_tech: HashSet<String> = stack
        .detected_tech
        .iter()
        .map(|t| t.name.to_lowercase())
        .chain(stack.primary_stack.iter().map(|s| s.to_lowercase()))
        .collect();

    for pp in &composed.pain_points {
        // Check if pain point keywords match user's tech
        let keyword_matches = pp
            .keywords
            .iter()
            .filter(|kw| user_tech.iter().any(|t| t.contains(&kw.to_lowercase())))
            .count();
        if keyword_matches >= 1 {
            opportunities.push(OptimizationOpportunity {
                area: pp.keywords.first().unwrap_or(&"general").to_string(),
                suggestion: pp.description.to_string(),
                severity: pp.severity,
            });
        }
    }
    opportunities.truncate(8);
    opportunities
}

/// Cross-references GPU/LLM/RAM tiers.
pub(super) fn detect_infrastructure_mismatches(
    infra: &InfrastructureDimension,
) -> Vec<InfrastructureMismatch> {
    let mut mismatches = Vec::new();

    // GPU + LLM mismatch: discrete GPU but no local LLM configured
    if (infra.gpu_tier == "discrete" || infra.gpu_tier == "workstation") && infra.llm_tier == "none"
    {
        mismatches.push(InfrastructureMismatch {
            category: "GPU + LLM".to_string(),
            issue: "You have a capable GPU but no LLM configured — consider running Ollama for free local AI".to_string(),
        });
    }

    // No GPU but using local LLM
    if infra.gpu_tier == "none" && infra.llm_tier == "local" {
        mismatches.push(InfrastructureMismatch {
            category: "GPU + LLM".to_string(),
            issue: "Running local LLM without GPU — inference will be CPU-only (slow). Consider a cloud LLM provider".to_string(),
        });
    }

    // RAM check: parse total RAM for potential issue detection
    if let Some(total_str) = infra.ram.get("total") {
        let total_gb = parse_gb(total_str);
        if total_gb > 0.0 && total_gb < 8.0 && infra.llm_tier == "local" {
            mismatches.push(InfrastructureMismatch {
                category: "RAM + LLM".to_string(),
                issue: format!(
                    "Only {total_gb:.0}GB RAM detected with local LLM — may cause swap thrashing. Consider a cloud provider or smaller model"
                ),
            });
        }
    }

    mismatches
}

/// Ecosystem shifts matched to user's detected technology.
pub(super) fn detect_ecosystem_alerts(stack: &StackDimension) -> Vec<EcosystemAlert> {
    let mut alerts = Vec::new();
    let composed = crate::stacks::compose_profiles(&stack.selected_profiles);
    if !composed.active {
        return alerts;
    }

    let user_tech: HashSet<String> = stack
        .detected_tech
        .iter()
        .map(|t| t.name.to_lowercase())
        .chain(stack.dependencies.iter().map(|d| d.to_lowercase()))
        .collect();

    for shift in &composed.ecosystem_shifts {
        if user_tech.contains(&shift.from.to_lowercase()) {
            alerts.push(EcosystemAlert {
                from_tech: shift.from.to_string(),
                to_tech: shift.to.to_string(),
                description: format!(
                    "You use {} — the ecosystem is shifting toward {}",
                    shift.from, shift.to
                ),
            });
        }
    }
    alerts.truncate(5);
    alerts
}

/// Parse GB from a string like "16 GB", "16.0", "16384 MB".
pub(super) fn parse_gb(s: &str) -> f64 {
    let s_lower = s.to_lowercase();
    if let Some(val) = s_lower
        .strip_suffix("gb")
        .or_else(|| s_lower.strip_suffix(" gb"))
    {
        return val.trim().parse::<f64>().unwrap_or(0.0);
    }
    if let Some(val) = s_lower
        .strip_suffix("mb")
        .or_else(|| s_lower.strip_suffix(" mb"))
    {
        return val.trim().parse::<f64>().unwrap_or(0.0) / 1024.0;
    }
    // Try bare number (assume GB)
    s.split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0)
}

// ============================================================================
// Completeness (depth-aware)
// ============================================================================

pub(super) fn compute_completeness(
    infra: &InfrastructureDimension,
    stack: &StackDimension,
    skills: &SkillsDimension,
    prefs: &PreferencesDimension,
    ctx: &ContextDimension,
) -> CompletenessReport {
    let dimensions = vec![
        compute_dimension_completeness("Infrastructure", count_infra_facts(infra), 15),
        compute_dimension_completeness(
            "Stack",
            stack.primary_stack.len() + stack.detected_tech.len() + stack.dependencies.len(),
            20,
        ),
        compute_dimension_completeness(
            "Skills",
            skills.top_affinities.len()
                + skills.playbook_progress.completed_lessons as usize
                + skills.engagement_sources.len(),
            15,
        ),
        compute_dimension_completeness(
            "Preferences",
            prefs.interests.len()
                + prefs.active_decisions.len()
                + prefs.tech_radar.adopt.len()
                + prefs.tech_radar.trial.len(),
            10,
        ),
        compute_dimension_completeness(
            "Context",
            ctx.active_topics.len() + ctx.projects_monitored as usize,
            10,
        ),
    ];

    // Weighted average: Infrastructure 25%, Stack 30%, Skills 20%, Preferences 15%, Context 10%
    let weights = [0.25, 0.30, 0.20, 0.15, 0.10];
    let overall_percentage = dimensions
        .iter()
        .zip(weights.iter())
        .map(|(d, w)| d.percentage * w)
        .sum::<f64>();

    CompletenessReport {
        overall_percentage,
        dimensions,
    }
}

fn count_infra_facts(infra: &InfrastructureDimension) -> usize {
    infra.cpu.len()
        + infra.ram.len()
        + infra.gpu.len()
        + infra.storage.len()
        + infra.network.len()
        + infra.os.len()
        + infra.llm.len()
        + infra.legal.len()
        + infra.budget.len()
}

pub(super) fn compute_dimension_completeness(
    name: &str,
    fact_count: usize,
    target: usize,
) -> DimensionCompleteness {
    let percentage = ((fact_count as f64 / target as f64) * 100.0).min(100.0);
    let depth = match fact_count {
        0 => "empty",
        1..=2 => "minimal",
        3..=5 => "partial",
        6..=10 => "good",
        _ => "comprehensive",
    };

    DimensionCompleteness {
        name: name.to_string(),
        depth: depth.to_string(),
        fact_count: fact_count as u32,
        percentage,
    }
}

pub(super) fn build_identity_summary(stack: &StackDimension) -> String {
    if stack.primary_stack.is_empty() {
        return "Developer — configure your stack to personalize".to_string();
    }
    // Only use identity-worthy tech for the title (languages, frameworks, platforms)
    let worthy: Vec<&String> = stack
        .primary_stack
        .iter()
        .filter(|s| is_identity_worthy(s))
        .take(3)
        .collect();
    if worthy.is_empty() {
        // Fallback to first 3 if nothing is "worthy"
        let stack_str = stack
            .primary_stack
            .iter()
            .take(3)
            .map(|s| capitalize(s))
            .collect::<Vec<_>>()
            .join("/");
        return format!("{stack_str} developer");
    }
    let stack_str = worthy
        .iter()
        .map(|s| capitalize(s))
        .collect::<Vec<_>>()
        .join("/");
    format!("{stack_str} developer")
}

pub(super) fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Rank primary stack by relevance instead of alphabetically.
/// Scoring: identity-worthy techs first, then by dependency frequency, then by
/// detected_tech confidence, with alphabetical as tiebreaker.
pub(super) fn rank_primary_stack(
    conn: &rusqlite::Connection,
    raw_stack: HashSet<String>,
) -> Vec<String> {
    // Get dependency frequency counts
    let mut freq: HashMap<String, i64> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT LOWER(package_name), COUNT(*) FROM project_dependencies GROUP BY LOWER(package_name)",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) {
            for pair in rows.flatten() {
                freq.insert(pair.0, pair.1);
            }
        }
    }

    // Get detected tech confidence scores
    let mut confidence: HashMap<String, f64> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT LOWER(name), confidence FROM detected_tech") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        }) {
            for pair in rows.flatten() {
                confidence.insert(pair.0, pair.1);
            }
        }
    }

    let mut ranked: Vec<(String, i64)> = raw_stack
        .into_iter()
        .map(|tech| {
            let tech_lower = tech.to_lowercase();
            let score = if is_identity_worthy(&tech_lower) {
                1000
            } else {
                0
            } + freq.get(&tech_lower).copied().unwrap_or(0) * 10
                + (confidence.get(&tech_lower).copied().unwrap_or(0.0) * 100.0) as i64;
            (tech, score)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.into_iter().map(|(t, _)| t).collect()
}

/// Filter for techs that should appear in the developer identity title.
/// Languages, frameworks, platforms = yes. ORMs, utility libs, build tools = no.
pub(super) fn is_identity_worthy(tech: &str) -> bool {
    const WORTHY: &[&str] = &[
        "rust",
        "typescript",
        "javascript",
        "python",
        "go",
        "java",
        "kotlin",
        "swift",
        "c",
        "cpp",
        "c++",
        "csharp",
        "c#",
        "ruby",
        "php",
        "scala",
        "elixir",
        "haskell",
        "dart",
        "zig",
        "nim",
        "lua",
        "r",
        "julia",
        "wgsl",
        "glsl",
        "sql",
        // Major frameworks / platforms
        "react",
        "vue",
        "angular",
        "svelte",
        "nextjs",
        "next.js",
        "nuxt",
        "remix",
        "tauri",
        "electron",
        "flutter",
        "react-native",
        "django",
        "flask",
        "fastapi",
        "rails",
        "spring",
        "express",
        "nest",
        "nestjs",
        "actix",
        "axum",
        "rocket",
        "tensorflow",
        "pytorch",
        "node",
        "nodejs",
        "deno",
        "bun",
        // Platforms
        "aws",
        "gcp",
        "azure",
        "docker",
        "kubernetes",
        "linux",
        "wasm",
        "webgpu",
    ];

    WORTHY.contains(&tech)
}

// ============================================================================
// Export
// ============================================================================

pub fn export_as_markdown(profile: &SovereignDeveloperProfile) -> String {
    let mut md = String::with_capacity(4096);

    md.push_str("# Sovereign Developer Profile\n\n");
    md.push_str(&format!("**{}**\n\n", profile.identity_summary));
    md.push_str(&format!(
        "*Generated by [4DA](https://4da.dev) on {}*\n\n",
        &profile.generated_at[..10]
    ));

    // Infrastructure
    md.push_str("## Infrastructure\n\n");
    let infra = &profile.infrastructure;
    write_map_section(&mut md, "CPU", &infra.cpu);
    write_map_section(&mut md, "RAM", &infra.ram);
    write_map_section(&mut md, "GPU", &infra.gpu);
    write_map_section(&mut md, "Storage", &infra.storage);
    write_map_section(&mut md, "Network", &infra.network);
    write_map_section(&mut md, "OS", &infra.os);
    md.push_str(&format!("- **GPU Tier:** {}\n", infra.gpu_tier));
    md.push_str(&format!("- **LLM Tier:** {}\n\n", infra.llm_tier));

    // Stack
    md.push_str("## Stack\n\n");
    if !profile.stack.primary_stack.is_empty() {
        md.push_str(&format!(
            "**Primary:** {}\n\n",
            profile.stack.primary_stack.join(", ")
        ));
    }
    if !profile.stack.selected_profiles.is_empty() {
        md.push_str(&format!(
            "**Stack Profiles:** {}\n\n",
            profile.stack.selected_profiles.join(", ")
        ));
    }
    if !profile.stack.dependencies.is_empty() {
        md.push_str("**Key Dependencies:** ");
        md.push_str(
            &profile
                .stack
                .dependencies
                .iter()
                .take(15)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        md.push_str("\n\n");
    }

    // Skills
    md.push_str("## Skills & Engagement\n\n");
    if !profile.skills.top_affinities.is_empty() {
        md.push_str("| Topic | Affinity |\n|-------|----------|\n");
        for a in profile.skills.top_affinities.iter().take(10) {
            md.push_str(&format!("| {} | {:.2} |\n", capitalize(&a.topic), a.score));
        }
        md.push('\n');
    }
    let pp = &profile.skills.playbook_progress;
    md.push_str(&format!(
        "**STREETS Progress:** {}/{} lessons",
        pp.completed_lessons, pp.total_lessons
    ));
    if !pp.completed_modules.is_empty() {
        md.push_str(&format!(
            " (completed: {})",
            pp.completed_modules.join(", ")
        ));
    }
    md.push_str("\n\n");

    // Preferences
    md.push_str("## Preferences\n\n");
    if !profile.preferences.interests.is_empty() {
        md.push_str(&format!(
            "**Interests:** {}\n\n",
            profile.preferences.interests.join(", ")
        ));
    }
    if !profile.preferences.exclusions.is_empty() {
        md.push_str(&format!(
            "**Exclusions:** {}\n\n",
            profile.preferences.exclusions.join(", ")
        ));
    }
    if !profile.preferences.active_decisions.is_empty() {
        md.push_str("**Active Decisions:**\n");
        for d in &profile.preferences.active_decisions {
            md.push_str(&format!("- {}: {}\n", d.subject, d.decision));
        }
        md.push('\n');
    }

    // Context
    md.push_str("## Active Context\n\n");
    if !profile.context.active_topics.is_empty() {
        md.push_str(&format!(
            "**Active Topics:** {}\n\n",
            profile
                .context
                .active_topics
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    md.push_str(&format!(
        "**Projects Monitored:** {}\n\n",
        profile.context.projects_monitored
    ));

    // Intelligence
    md.push_str("## Intelligence\n\n");
    let intel = &profile.intelligence;
    if !intel.skill_gaps.is_empty() {
        md.push_str("### Skill Gaps\n\n");
        for gap in intel.skill_gaps.iter().take(5) {
            md.push_str(&format!("- **{}** — {}\n", gap.dependency, gap.reason));
        }
        md.push('\n');
    }
    if !intel.optimization_opportunities.is_empty() {
        md.push_str("### Optimization Opportunities\n\n");
        for opp in &intel.optimization_opportunities {
            md.push_str(&format!("- **{}** — {}\n", opp.area, opp.suggestion));
        }
        md.push('\n');
    }
    if !intel.infrastructure_mismatches.is_empty() {
        md.push_str("### Infrastructure Mismatches\n\n");
        for m in &intel.infrastructure_mismatches {
            md.push_str(&format!("- **{}** — {}\n", m.category, m.issue));
        }
        md.push('\n');
    }
    if !intel.ecosystem_alerts.is_empty() {
        md.push_str("### Ecosystem Alerts\n\n");
        for a in &intel.ecosystem_alerts {
            md.push_str(&format!(
                "- **{} -> {}** — {}\n",
                a.from_tech, a.to_tech, a.description
            ));
        }
        md.push('\n');
    }

    // Completeness
    md.push_str("## Profile Completeness\n\n");
    md.push_str(&format!(
        "**Overall: {:.0}%**\n\n",
        profile.completeness.overall_percentage
    ));
    md.push_str("| Dimension | Depth | Facts | % |\n|-----------|-------|-------|---|\n");
    for d in &profile.completeness.dimensions {
        md.push_str(&format!(
            "| {} | {} | {} | {:.0}% |\n",
            d.name, d.depth, d.fact_count, d.percentage
        ));
    }

    md
}

fn write_map_section(md: &mut String, label: &str, map: &HashMap<String, String>) {
    if map.is_empty() {
        return;
    }
    for (key, value) in map {
        md.push_str(&format!("- **{label}/{key}:** {value}\n"));
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_sovereign_developer_profile() -> crate::error::Result<SovereignDeveloperProfile> {
    let conn = crate::open_db_connection()?;
    Ok(assemble_profile(&conn))
}

#[tauri::command]
pub fn export_sovereign_profile_markdown() -> crate::error::Result<String> {
    let conn = crate::open_db_connection()?;
    let profile = assemble_profile(&conn);
    Ok(export_as_markdown(&profile))
}

#[tauri::command]
pub fn export_sovereign_profile_json() -> crate::error::Result<String> {
    let conn = crate::open_db_connection()?;
    let profile = assemble_profile(&conn);
    Ok(serde_json::to_string_pretty(&profile)?)
}
