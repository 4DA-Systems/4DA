//! Sovereign Developer Profile — unified view across all profile systems.
//!
//! Computed assembly layer that reads from 12+ existing tables and computes
//! cross-category intelligence. No new database tables — everything is derived
//! from existing data. Follows the PersonalizationContext assembler pattern.
//!
//! 5 Dimensions:
//! 1. Infrastructure — sovereign_profile (CPU/RAM/GPU/Storage/Network/OS/LLM/Legal/Budget)
//! 2. Stack — tech_stack + detected_tech + project_dependencies + selected_stacks
//! 3. Skills — topic_affinities + feedback + playbook_progress + behavior
//! 4. Preferences — interests + exclusions + decisions + tech_radar
//! 5. Context — active_topics + git_signals + file_signals + scan dirs

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

/// Unified Sovereign Developer Profile — 5 dimensions + intelligence + completeness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignDeveloperProfile {
    pub generated_at: String,
    pub identity_summary: String,
    pub infrastructure: InfrastructureDimension,
    pub stack: StackDimension,
    pub skills: SkillsDimension,
    pub preferences: PreferencesDimension,
    pub context: ContextDimension,
    pub intelligence: IntelligenceReport,
    pub completeness: CompletenessReport,
}

// ---- Dimension 1: Infrastructure ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InfrastructureDimension {
    pub cpu: HashMap<String, String>,
    pub ram: HashMap<String, String>,
    pub gpu: HashMap<String, String>,
    pub storage: HashMap<String, String>,
    pub network: HashMap<String, String>,
    pub os: HashMap<String, String>,
    pub llm: HashMap<String, String>,
    pub legal: HashMap<String, String>,
    pub budget: HashMap<String, String>,
    pub gpu_tier: String,
    pub llm_tier: String,
}

// ---- Dimension 2: Stack ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StackDimension {
    pub primary_stack: Vec<String>,
    pub adjacent_tech: Vec<String>,
    pub detected_tech: Vec<DetectedTechEntry>,
    pub dependencies: Vec<String>,
    pub selected_profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedTechEntry {
    pub name: String,
    pub confidence: f32,
}

// ---- Dimension 3: Skills ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsDimension {
    pub top_affinities: Vec<AffinityEntry>,
    pub playbook_progress: PlaybookProgressSummary,
    pub engagement_sources: Vec<SourceEngagementEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityEntry {
    pub topic: String,
    pub score: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybookProgressSummary {
    pub completed_lessons: u32,
    pub total_lessons: u32,
    pub completed_modules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEngagementEntry {
    pub source_type: String,
    pub items_seen: u32,
    pub items_saved: u32,
}

// ---- Dimension 4: Preferences ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreferencesDimension {
    pub interests: Vec<String>,
    pub exclusions: Vec<String>,
    pub active_decisions: Vec<DecisionEntry>,
    pub tech_radar: TechRadarSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub subject: String,
    pub decision: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TechRadarSummary {
    pub adopt: Vec<String>,
    pub trial: Vec<String>,
    pub assess: Vec<String>,
    pub hold: Vec<String>,
}

// ---- Dimension 5: Context ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextDimension {
    pub active_topics: Vec<String>,
    pub scan_directories: Vec<String>,
    pub projects_monitored: u32,
}

// ---- Intelligence ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntelligenceReport {
    pub skill_gaps: Vec<SkillGap>,
    pub optimization_opportunities: Vec<OptimizationOpportunity>,
    pub infrastructure_mismatches: Vec<InfrastructureMismatch>,
    pub ecosystem_alerts: Vec<EcosystemAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillGap {
    pub dependency: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOpportunity {
    pub area: String,
    pub suggestion: String,
    pub severity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureMismatch {
    pub category: String,
    pub issue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemAlert {
    pub from_tech: String,
    pub to_tech: String,
    pub description: String,
}

// ---- Completeness ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletenessReport {
    pub overall_percentage: f64,
    pub dimensions: Vec<DimensionCompleteness>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionCompleteness {
    pub name: String,
    pub depth: String, // "empty", "minimal", "partial", "good", "comprehensive"
    pub fact_count: u32,
    pub percentage: f64,
}

// ============================================================================
// Assembly
// ============================================================================

/// Assemble the unified Sovereign Developer Profile from all existing data sources.
/// No new tables — reads from 12+ existing tables and computes intelligence.
pub fn assemble_profile(conn: &Connection) -> SovereignDeveloperProfile {
    let infrastructure = assemble_infrastructure(conn);
    // Build domain_profile once — previously built separately in assemble_stack
    // AND again inside compute_radar (called by assemble_preferences).
    let domain = crate::domain_profile::build_domain_profile(conn);
    let stack = assemble_stack(conn, &domain);
    let skills = assemble_skills(conn);
    let preferences = assemble_preferences(conn);
    let context = assemble_context(conn);

    let intelligence = compute_intelligence(&infrastructure, &stack, &skills, &preferences);
    let completeness =
        compute_completeness(&infrastructure, &stack, &skills, &preferences, &context);
    let identity_summary = build_identity_summary(&stack);

    SovereignDeveloperProfile {
        generated_at: chrono::Utc::now().to_rfc3339(),
        identity_summary,
        infrastructure,
        stack,
        skills,
        preferences,
        context,
        intelligence,
        completeness,
    }
}

// ============================================================================
// Dimension Assemblers
// ============================================================================

fn assemble_infrastructure(conn: &Connection) -> InfrastructureDimension {
    let mut dim = InfrastructureDimension::default();

    let mut stmt = match conn
        .prepare("SELECT category, key, value FROM sovereign_profile ORDER BY category, key")
    {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::sdp", error = %e, "Failed to query sovereign_profile");
            return dim;
        }
    };

    let rows: Vec<(String, String, String)> =
        match stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))) {
            Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
            Err(_) => return dim,
        };

    for (category, key, value) in rows {
        let map = match category.as_str() {
            "cpu" => &mut dim.cpu,
            "ram" => &mut dim.ram,
            "gpu" => &mut dim.gpu,
            "storage" => &mut dim.storage,
            "network" => &mut dim.network,
            "os" => &mut dim.os,
            "llm" => &mut dim.llm,
            "legal" => &mut dim.legal,
            "budget" => &mut dim.budget,
            _ => continue,
        };
        map.insert(key, value);
    }

    // Compute GPU tier
    let gpu_name = dim
        .gpu
        .get("name")
        .cloned()
        .unwrap_or_default()
        .to_lowercase();
    let gpu_memory = dim
        .gpu
        .get("memory_total")
        .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0);
    dim.gpu_tier = if gpu_name.is_empty() || gpu_name.contains("not found") {
        "none".to_string()
    } else if gpu_memory >= 16.0 {
        "workstation".to_string()
    } else if gpu_name.contains("nvidia")
        || gpu_name.contains("geforce")
        || gpu_name.contains("rtx")
        || gpu_name.contains("gtx")
        || gpu_memory >= 4.0
    {
        "discrete".to_string()
    } else {
        "integrated".to_string()
    };

    // Compute LLM tier
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let llm = &guard.get().llm;
    dim.llm_tier = if llm.api_key.is_empty() && llm.provider != "ollama" {
        "none".to_string()
    } else if llm.provider == "ollama" {
        "local".to_string()
    } else {
        "cloud".to_string()
    };

    dim
}

fn assemble_stack(
    conn: &Connection,
    domain: &crate::domain_profile::DomainProfile,
) -> StackDimension {
    let primary_stack = rank_primary_stack(conn, domain.primary_stack.clone());
    let mut adjacent_tech: Vec<String> = domain.adjacent_tech.iter().cloned().collect();
    adjacent_tech.sort();
    let mut dependencies: Vec<String> = domain.dependency_names.iter().take(30).cloned().collect();
    dependencies.sort();

    // Detected tech with confidence
    let detected_tech = conn
        .prepare("SELECT name, confidence FROM detected_tech ORDER BY confidence DESC LIMIT 30")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(DetectedTechEntry {
                    name: row.get(0)?,
                    confidence: row.get(1)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    // Selected stack profiles
    let selected_profiles = crate::stacks::load_selected_stacks(conn);

    StackDimension {
        primary_stack,
        adjacent_tech,
        detected_tech,
        dependencies,
        selected_profiles,
    }
}

fn assemble_skills(conn: &Connection) -> SkillsDimension {
    // Topic affinities (top 15)
    let top_affinities = conn
        .prepare("SELECT topic, affinity_score FROM topic_affinities ORDER BY affinity_score DESC LIMIT 15")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(AffinityEntry {
                    topic: row.get(0)?,
                    score: row.get(1)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    // Playbook progress
    let playbook_progress = assemble_playbook_progress(conn);

    // Source engagement
    let engagement_sources = conn
        .prepare(
            "SELECT source_type, COUNT(*) as total,
             SUM(CASE WHEN id IN (SELECT item_id FROM feedback WHERE action = 'save') THEN 1 ELSE 0 END) as saved
             FROM source_items GROUP BY source_type ORDER BY total DESC",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(SourceEngagementEntry {
                    source_type: row.get(0)?,
                    items_seen: row.get(1)?,
                    items_saved: row.get(2)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    SkillsDimension {
        top_affinities,
        playbook_progress,
        engagement_sources,
    }
}

fn assemble_playbook_progress(conn: &Connection) -> PlaybookProgressSummary {
    let mut summary = PlaybookProgressSummary::default();
    let mut per_module: HashMap<String, Vec<u32>> = HashMap::new();

    let rows: Vec<(String, u32)> = conn
        .prepare("SELECT module_id, lesson_idx FROM playbook_progress")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    for (module_id, lesson_idx) in &rows {
        per_module
            .entry(module_id.clone())
            .or_default()
            .push(*lesson_idx);
    }
    summary.completed_lessons = rows.len() as u32;

    let module_ids = ["S", "T", "R", "E1", "E2", "T2", "S2"];
    for mid in &module_ids {
        if let Some(filename) = crate::playbook_commands::module_id_to_filename(mid) {
            let content_dir = crate::playbook_commands::get_content_dir();
            let path = content_dir.join(filename);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let lesson_count = crate::playbook_commands::parse_lessons(&content).len();
                    summary.total_lessons += lesson_count as u32;
                    let completed = per_module.get(*mid).map(|v| v.len()).unwrap_or(0);
                    if completed >= lesson_count && lesson_count > 0 {
                        summary.completed_modules.push(mid.to_string());
                    }
                }
            }
        }
    }

    summary
}

fn assemble_preferences(conn: &Connection) -> PreferencesDimension {
    // Interests
    let interests = conn
        .prepare("SELECT topic FROM explicit_interests ORDER BY topic")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    // Exclusions
    let exclusions = conn
        .prepare("SELECT topic FROM exclusions ORDER BY topic")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    // Active decisions
    let active_decisions = conn
        .prepare(
            "SELECT subject, decision FROM developer_decisions WHERE status = 'active' LIMIT 15",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(DecisionEntry {
                    subject: row.get(0)?,
                    decision: row.get(1)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    // Tech radar — use last cached snapshot instead of full recomputation.
    // compute_radar() rebuilds domain_profile internally, which duplicates the
    // work already done in assemble_stack. Reading the snapshot is O(1) vs O(N).
    let tech_radar = load_radar_summary_from_snapshot(conn);

    PreferencesDimension {
        interests,
        exclusions,
        active_decisions,
        tech_radar,
    }
}

/// Load a TechRadarSummary from the latest temporal_events snapshot.
/// Falls back to an empty summary if no snapshot exists yet (the full radar
/// will be computed on its own schedule and the snapshot will be available
/// on the next profile load).
fn load_radar_summary_from_snapshot(conn: &Connection) -> TechRadarSummary {
    let data: Option<String> = conn
        .query_row(
            "SELECT data FROM temporal_events
             WHERE event_type = 'radar_snapshot'
             ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let data = match data {
        Some(d) => d,
        None => {
            debug!(target: "4da::sdp", "No radar snapshot found — returning empty summary");
            return TechRadarSummary::default();
        }
    };

    // Snapshot format is HashMap<tech_name, ring_name> (e.g. {"rust": "adopt"})
    let map: HashMap<String, String> = serde_json::from_str(&data).unwrap_or_default();

    let mut summary = TechRadarSummary::default();
    for (name, ring) in &map {
        let list = match ring.as_str() {
            "adopt" => &mut summary.adopt,
            "trial" => &mut summary.trial,
            "assess" => &mut summary.assess,
            "hold" => &mut summary.hold,
            _ => continue,
        };
        if list.len() < 10 {
            list.push(name.clone());
        }
    }
    summary
}

fn assemble_context(conn: &Connection) -> ContextDimension {
    // Active topics
    let active_topics = conn
        .prepare("SELECT topic FROM active_topics ORDER BY last_seen DESC LIMIT 20")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    // Projects monitored
    let projects_monitored: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT project_path) FROM project_dependencies",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Scan directories from settings
    let scan_directories: Vec<String> = crate::get_context_dirs()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    ContextDimension {
        active_topics,
        scan_directories,
        projects_monitored,
    }
}

// ============================================================================
// Intelligence (pure computation, no side effects)
// ============================================================================

fn compute_intelligence(
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
fn detect_skill_gaps(stack: &StackDimension, skills: &SkillsDimension) -> Vec<SkillGap> {
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
fn detect_optimization_opportunities(stack: &StackDimension) -> Vec<OptimizationOpportunity> {
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
fn detect_infrastructure_mismatches(
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
                    "Only {:.0}GB RAM detected with local LLM — may cause swap thrashing. Consider a cloud provider or smaller model",
                    total_gb
                ),
            });
        }
    }

    mismatches
}

/// Ecosystem shifts matched to user's detected technology.
fn detect_ecosystem_alerts(stack: &StackDimension) -> Vec<EcosystemAlert> {
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
fn parse_gb(s: &str) -> f64 {
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

fn compute_completeness(
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

fn compute_dimension_completeness(
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

fn build_identity_summary(stack: &StackDimension) -> String {
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
        return format!("{} developer", stack_str);
    }
    let stack_str = worthy
        .iter()
        .map(|s| capitalize(s))
        .collect::<Vec<_>>()
        .join("/");
    format!("{} developer", stack_str)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Rank primary stack by relevance instead of alphabetically.
/// Scoring: identity-worthy techs first, then by dependency frequency, then by
/// detected_tech confidence, with alphabetical as tiebreaker.
fn rank_primary_stack(conn: &Connection, raw_stack: HashSet<String>) -> Vec<String> {
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
fn is_identity_worthy(tech: &str) -> bool {
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
        md.push_str(&format!("- **{}/{}:** {}\n", label, key, value));
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_sovereign_developer_profile() -> Result<SovereignDeveloperProfile, String> {
    let conn = crate::open_db_connection()?;
    Ok(assemble_profile(&conn))
}

#[tauri::command]
pub fn export_sovereign_profile_markdown() -> Result<String, String> {
    let conn = crate::open_db_connection()?;
    let profile = assemble_profile(&conn);
    Ok(export_as_markdown(&profile))
}

#[tauri::command]
pub fn export_sovereign_profile_json() -> Result<String, String> {
    let conn = crate::open_db_connection()?;
    let profile = assemble_profile(&conn);
    serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
