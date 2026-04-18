// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! PersonalizationContext — structured data assembly from all sovereign sources.
//!
//! Unlike CoachContext (which produces string summaries for LLM prompts), this
//! module produces structured data for template interpolation and card computation.
//! Context hash via SHA-256 enables content-addressed caching.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

/// Full personalization context assembled from all sovereign data sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizationContext {
    // Source 1: Sovereign Profile facts (category → key → value)
    pub profile: ProfileData,
    // Source 2: Domain profile (stack, adjacent tech)
    pub stack: StackData,
    // Source 3: Tech radar entries
    pub radar: RadarData,
    // Source 4: Regional localization data
    pub regional: RegionalData,
    // Source 5: Active developer decisions
    pub decisions: Vec<DecisionEntry>,
    // Source 6: Playbook progress
    pub progress: ProgressData,
    // Source 7: Settings / LLM config
    pub settings: SettingsData,
    // Source 8: Developer DNA (Signal only, partial for free)
    pub dna: DnaData,
    // Derived / computed fields
    pub computed: ComputedFields,
}

/// Sovereign Profile facts organized by category.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileData {
    pub cpu: HashMap<String, String>,
    pub ram: HashMap<String, String>,
    pub gpu: HashMap<String, String>,
    pub storage: HashMap<String, String>,
    pub network: HashMap<String, String>,
    pub os: HashMap<String, String>,
    pub llm: HashMap<String, String>,
    pub legal: HashMap<String, String>,
    pub budget: HashMap<String, String>,
    pub categories_filled: u32,
}

/// Stack data from domain profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StackData {
    pub primary: Vec<String>,
    pub adjacent: Vec<String>,
    pub interests: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Tech radar summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RadarData {
    pub adopt: Vec<String>,
    pub trial: Vec<String>,
    pub assess: Vec<String>,
    pub hold: Vec<String>,
}

/// Regional localization data (mirrors streets_localization::RegionalData).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegionalData {
    pub country: String,
    pub currency: String,
    pub currency_symbol: String,
    pub electricity_kwh: f64,
    pub internet_monthly: f64,
    pub business_registration_cost: f64,
    pub business_entity_type: String,
    pub tax_note: String,
    pub payment_processors: Vec<String>,
}

/// Active developer decision summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub subject: String,
    pub decision: String,
}

/// Playbook progress summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgressData {
    pub completed_modules: Vec<String>,
    pub completed_lesson_count: u32,
    pub total_lesson_count: u32,
    pub per_module: HashMap<String, Vec<u32>>,
}

/// Settings relevant to personalization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsData {
    pub has_llm: bool,
    pub llm_provider: String,
    pub llm_model: String,
}

/// Developer DNA data (Signal features gated).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnaData {
    pub is_full: bool,
    pub primary_stack: Vec<String>,
    pub interests: Vec<String>,
    pub identity_summary: String,
    pub blind_spots: Vec<String>,
    pub top_engaged_topics: Vec<String>,
}

/// Derived fields computed from the assembled data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComputedFields {
    /// LLM tier classification: "none", "local", "cloud"
    pub llm_tier: String,
    /// GPU tier classification: "none", "integrated", "discrete", "workstation"
    pub gpu_tier: String,
    /// Whether NVIDIA GPU is present
    pub has_nvidia: bool,
    /// Monthly electricity cost estimate in user's currency
    pub monthly_electricity_estimate: f64,
    /// Profile completeness percentage (0-100)
    pub profile_completeness: f64,
    /// OS family: "windows", "macos", "linux"
    pub os_family: String,
}

// ============================================================================
// Assembly
// ============================================================================

/// Assemble the full PersonalizationContext from all data sources.
/// Gracefully degrades — missing data sources produce empty defaults.
pub fn assemble_personalization_context(conn: &Connection) -> PersonalizationContext {
    let profile = assemble_profile(conn);
    let stack = assemble_stack(conn);
    let radar = assemble_radar(conn);
    let regional = assemble_regional(conn);
    let decisions = assemble_decisions(conn);
    let progress = assemble_progress(conn);
    let settings = assemble_settings();
    let dna = assemble_dna();
    let computed = compute_derived(&profile, &settings);

    PersonalizationContext {
        profile,
        stack,
        radar,
        regional,
        decisions,
        progress,
        settings,
        dna,
        computed,
    }
}

/// Compute SHA-256 hash of the serialized context for cache invalidation.
pub fn context_hash(ctx: &PersonalizationContext) -> String {
    let json = serde_json::to_string(ctx).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// Individual Source Assemblers
// ============================================================================

fn assemble_profile(conn: &Connection) -> ProfileData {
    let mut data = ProfileData::default();
    let mut categories_seen = HashSet::new();

    let mut stmt = match conn
        .prepare("SELECT category, key, value FROM sovereign_profile ORDER BY category, key")
    {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::personalize", error = %e, "Failed to query sovereign_profile");
            return data;
        }
    };

    let rows: Vec<(String, String, String)> = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) {
        Ok(mapped) => mapped
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in content_personalization_context: {e}");
                    None
                }
            })
            .collect(),
        Err(e) => {
            warn!(target: "4da::personalize", error = %e, "Failed to map sovereign_profile rows");
            return data;
        }
    };

    for (category, key, value) in rows {
        categories_seen.insert(category.clone());
        let map = match category.as_str() {
            "cpu" => &mut data.cpu,
            "ram" => &mut data.ram,
            "gpu" => &mut data.gpu,
            "storage" => &mut data.storage,
            "network" => &mut data.network,
            "os" => &mut data.os,
            "llm" => &mut data.llm,
            "legal" => &mut data.legal,
            "budget" => &mut data.budget,
            _ => continue,
        };
        map.insert(key, value);
    }

    data.categories_filled = categories_seen.len() as u32;
    data
}

fn assemble_stack(conn: &Connection) -> StackData {
    let profile = crate::domain_profile::build_domain_profile(conn);

    // CONTENT ACCURACY GATE (defense-in-depth): Even if upstream data is dirty,
    // the personalization context must only contain display-worthy tech in primary.
    // This is the last filter before data reaches template interpolation.
    let mut primary: Vec<String> = profile
        .primary_stack
        .into_iter()
        .filter(|t| {
            let worthy = crate::domain_profile::is_display_worthy(t);
            if !worthy {
                debug!(
                    target: "4da::personalize",
                    tech = %t,
                    "Filtered non-display-worthy tech from personalization primary stack"
                );
            }
            worthy
        })
        .collect();
    primary.sort();
    let mut adjacent: Vec<String> = profile.adjacent_tech.into_iter().collect();
    adjacent.sort();
    let mut interests: Vec<String> = profile.interest_topics.into_iter().collect();
    interests.sort();
    let mut deps: Vec<String> = profile.dependency_names.into_iter().take(20).collect();
    deps.sort();

    StackData {
        primary,
        adjacent,
        interests,
        dependencies: deps,
    }
}

fn assemble_radar(conn: &Connection) -> RadarData {
    match crate::tech_radar::compute_radar(conn) {
        Ok(radar) => {
            let by_ring = |ring: &str| {
                radar
                    .entries
                    .iter()
                    .filter(|e| format!("{:?}", e.ring).to_lowercase() == ring)
                    .take(10)
                    .map(|e| e.name.clone())
                    .collect()
            };
            RadarData {
                adopt: by_ring("adopt"),
                trial: by_ring("trial"),
                assess: by_ring("assess"),
                hold: by_ring("hold"),
            }
        }
        Err(e) => {
            debug!(target: "4da::personalize", error = %e, "Tech radar unavailable");
            RadarData::default()
        }
    }
}

fn assemble_regional(conn: &Connection) -> RegionalData {
    // Try to get country from sovereign profile, then from locale settings
    let country_code = conn
        .query_row(
            "SELECT value FROM sovereign_profile WHERE category = 'legal' AND key = 'country'",
            [],
            |row| row.get::<_, String>(0),
        )
        .or_else(|_| {
            // Fallback: check settings locale
            let manager = crate::get_settings_manager();
            let guard = manager.lock();
            let locale = &guard.get().locale;
            Ok(locale.country.clone())
        })
        .unwrap_or_else(|_: rusqlite::Error| "US".to_string());

    if let Some(rd) = crate::streets_localization::load_regional_data_for_context(&country_code) {
        RegionalData {
            country: rd.country,
            currency: rd.currency,
            currency_symbol: rd.currency_symbol,
            electricity_kwh: rd.electricity_kwh,
            internet_monthly: rd.internet_typical_monthly,
            business_registration_cost: rd.business_registration_cost,
            business_entity_type: rd.business_entity_type,
            tax_note: rd.tax_note,
            payment_processors: rd.payment_processors,
        }
    } else {
        debug!(target: "4da::personalize", country = %country_code, "No regional data found");
        RegionalData::default()
    }
}

fn assemble_decisions(conn: &Connection) -> Vec<DecisionEntry> {
    let mut stmt = match conn.prepare(
        "SELECT subject, decision FROM developer_decisions WHERE status = 'active' LIMIT 10",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let result: Vec<DecisionEntry> = match stmt.query_map([], |row| {
        Ok(DecisionEntry {
            subject: row.get(0)?,
            decision: row.get(1)?,
        })
    }) {
        Ok(mapped) => mapped
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in content_personalization_context: {e}");
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    result
}

fn assemble_progress(conn: &Connection) -> ProgressData {
    let mut data = ProgressData::default();

    // Get per-module completions
    let mut stmt = match conn.prepare("SELECT module_id, lesson_idx FROM playbook_progress") {
        Ok(s) => s,
        Err(_) => return data,
    };

    let rows: Vec<(String, u32)> = match stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?))) {
        Ok(mapped) => mapped
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in content_personalization_context: {e}");
                    None
                }
            })
            .collect(),
        Err(_) => return data,
    };

    for (module_id, lesson_idx) in &rows {
        data.per_module
            .entry(module_id.clone())
            .or_default()
            .push(*lesson_idx);
    }

    data.completed_lesson_count = rows.len() as u32;

    // Check which modules are fully completed
    let module_ids = ["S", "T", "R", "E1", "E2", "T2", "S2"];
    for mid in &module_ids {
        if let Some(filename) = crate::playbook_commands::module_id_to_filename(mid) {
            let content_dir = crate::playbook_commands::get_content_dir();
            let path = content_dir.join(filename);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let lesson_count = crate::playbook_commands::parse_lessons(&content).len();
                    data.total_lesson_count += lesson_count as u32;

                    let completed = data.per_module.get(*mid).map_or(0, std::vec::Vec::len);
                    if completed >= lesson_count && lesson_count > 0 {
                        data.completed_modules.push(mid.to_string());
                    }
                }
            }
        }
    }

    data
}

fn assemble_settings() -> SettingsData {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let llm = &guard.get().llm;

    SettingsData {
        has_llm: !llm.api_key.is_empty() || llm.provider == "ollama",
        llm_provider: llm.provider.clone(),
        llm_model: llm.model.clone(),
    }
}

fn assemble_dna() -> DnaData {
    if let Ok(dna) = crate::developer_dna::generate_dna() {
        DnaData {
            is_full: true,
            primary_stack: dna.primary_stack,
            interests: dna.interests,
            identity_summary: dna.identity_summary,
            blind_spots: dna
                .blind_spots
                .iter()
                .take(5)
                .map(|b| b.dependency.clone())
                .collect(),
            top_engaged_topics: dna
                .top_engaged_topics
                .iter()
                .take(5)
                .map(|t| t.topic.clone())
                .collect(),
        }
    } else {
        // Free-tier fallback: use stack data from domain profile
        debug!(target: "4da::personalize", "DNA generation failed, using free-tier stack data");
        DnaData::default()
    }
}

fn compute_derived(profile: &ProfileData, settings: &SettingsData) -> ComputedFields {
    // LLM tier
    let llm_tier = if !settings.has_llm {
        "none"
    } else if settings.llm_provider == "ollama" {
        "local"
    } else {
        "cloud"
    }
    .to_string();

    // GPU tier
    let gpu_name = profile.gpu.get("name").cloned().unwrap_or_default();
    let gpu_memory = profile
        .gpu
        .get("memory_total")
        .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0);
    let has_nvidia = gpu_name.to_lowercase().contains("nvidia")
        || gpu_name.to_lowercase().contains("geforce")
        || gpu_name.to_lowercase().contains("rtx")
        || gpu_name.to_lowercase().contains("gtx");
    let gpu_tier = if gpu_name.is_empty() || gpu_name.to_lowercase().contains("not found") {
        "none"
    } else if gpu_memory >= 16.0 {
        "workstation"
    } else if has_nvidia || gpu_memory >= 4.0 {
        "discrete"
    } else {
        "integrated"
    }
    .to_string();

    // Monthly electricity estimate (assuming ~200W average for dev machine, 8h/day)
    // This is a rough estimate — 200W * 8h * 30 days = 48 kWh/month
    let monthly_electricity_estimate = 48.0; // kWh, multiplied by rate in template

    // Profile completeness
    let profile_completeness = (profile.categories_filled as f64 / 9.0) * 100.0;

    // OS family
    let os_name = profile
        .os
        .get("name")
        .cloned()
        .unwrap_or_else(|| std::env::consts::OS.to_string());
    let os_family = if os_name.to_lowercase().contains("windows") {
        "windows"
    } else if os_name.to_lowercase().contains("mac") || os_name.to_lowercase().contains("darwin") {
        "macos"
    } else {
        "linux"
    }
    .to_string();

    ComputedFields {
        llm_tier,
        gpu_tier,
        has_nvidia,
        monthly_electricity_estimate,
        profile_completeness,
        os_family,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_hash_deterministic() {
        let ctx = PersonalizationContext {
            profile: ProfileData::default(),
            stack: StackData::default(),
            radar: RadarData::default(),
            regional: RegionalData::default(),
            decisions: Vec::new(),
            progress: ProgressData::default(),
            settings: SettingsData::default(),
            dna: DnaData::default(),
            computed: ComputedFields::default(),
        };
        let h1 = context_hash(&ctx);
        let h2 = context_hash(&ctx);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_compute_derived_llm_tiers() {
        let profile = ProfileData::default();

        let none = compute_derived(
            &profile,
            &SettingsData {
                has_llm: false,
                ..Default::default()
            },
        );
        assert_eq!(none.llm_tier, "none");

        let local = compute_derived(
            &profile,
            &SettingsData {
                has_llm: true,
                llm_provider: "ollama".into(),
                ..Default::default()
            },
        );
        assert_eq!(local.llm_tier, "local");

        let cloud = compute_derived(
            &profile,
            &SettingsData {
                has_llm: true,
                llm_provider: "anthropic".into(),
                ..Default::default()
            },
        );
        assert_eq!(cloud.llm_tier, "cloud");
    }

    #[test]
    fn test_compute_derived_gpu_tiers() {
        let mut profile = ProfileData::default();

        // No GPU
        let derived = compute_derived(&profile, &SettingsData::default());
        assert_eq!(derived.gpu_tier, "none");

        // NVIDIA discrete
        profile.gpu.insert("name".into(), "NVIDIA RTX 4090".into());
        profile.gpu.insert("memory_total".into(), "24 GB".into());
        let derived = compute_derived(&profile, &SettingsData::default());
        assert_eq!(derived.gpu_tier, "workstation");
        assert!(derived.has_nvidia);

        // Small discrete
        profile.gpu.insert("name".into(), "NVIDIA GTX 1650".into());
        profile.gpu.insert("memory_total".into(), "4 GB".into());
        let derived = compute_derived(&profile, &SettingsData::default());
        assert_eq!(derived.gpu_tier, "discrete");
    }
}
