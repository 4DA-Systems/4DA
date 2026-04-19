// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

use std::collections::HashMap;

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
            Ok(mapped) => mapped
                .filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!("Row processing failed in sovereign_developer_profile: {e}");
                        None
                    }
                })
                .collect(),
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
            .map(|rows| {
                rows.filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!("Row processing failed in sovereign_developer_profile: {e}");
                        None
                    }
                })
                .collect::<Vec<_>>()
            })
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
            .map(|rows| rows.filter_map(|r| match r {
    Ok(v) => Some(v),
    Err(e) => {
        tracing::warn!("Row processing failed in sovereign_developer_profile: {e}");
        None
    }
}).collect::<Vec<_>>())
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
            .map(|rows| rows.filter_map(|r| match r {
    Ok(v) => Some(v),
    Err(e) => {
        tracing::warn!("Row processing failed in sovereign_developer_profile: {e}");
        None
    }
}).collect::<Vec<_>>())
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
                .map(|rows| {
                    rows.filter_map(|r| match r {
                        Ok(v) => Some(v),
                        Err(e) => {
                            tracing::warn!(
                                "Row processing failed in sovereign_developer_profile: {e}"
                            );
                            None
                        }
                    })
                    .collect()
                })
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
                    let completed = per_module.get(*mid).map_or(0, std::vec::Vec::len);
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
                .map(|rows| {
                    rows.filter_map(|r| match r {
                        Ok(v) => Some(v),
                        Err(e) => {
                            tracing::warn!(
                                "Row processing failed in sovereign_developer_profile: {e}"
                            );
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();

    // Exclusions
    let exclusions = conn
        .prepare("SELECT topic FROM exclusions ORDER BY topic")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| {
                    rows.filter_map(|r| match r {
                        Ok(v) => Some(v),
                        Err(e) => {
                            tracing::warn!(
                                "Row processing failed in sovereign_developer_profile: {e}"
                            );
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                })
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
            .map(|rows| {
                rows.filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!("Row processing failed in sovereign_developer_profile: {e}");
                        None
                    }
                })
                .collect::<Vec<_>>()
            })
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

    let data = if let Some(d) = data {
        d
    } else {
        debug!(target: "4da::sdp", "No radar snapshot found — returning empty summary");
        return TechRadarSummary::default();
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
                .map(|rows| {
                    rows.filter_map(|r| match r {
                        Ok(v) => Some(v),
                        Err(e) => {
                            tracing::warn!(
                                "Row processing failed in sovereign_developer_profile: {e}"
                            );
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                })
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

// --- Sibling modules ---

#[path = "sovereign_developer_profile_intelligence.rs"]
mod sovereign_developer_profile_intelligence;
pub use sovereign_developer_profile_intelligence::*;

#[path = "sovereign_developer_profile_tests.rs"]
mod sovereign_developer_profile_tests;
