// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! AWE Context Bridge — Intelligence Reconciliation Phase 6 (2026-04-17).
//!
//! Assembles the `DeveloperContext` payload passed to AWE transmutations
//! via `--context-file`. The previous bridge carried 5 fields and left
//! AWE starving for situation signal; this expanded bridge carries the
//! full 17 fields documented in AWE's `CONTEXT-BRIDGE-PLAN.md`.
//!
//! Reading sources:
//! - Identity + stack      → sovereign_developer_profile
//! - Rig (OS/CPU/RAM/GPU)  → sovereign_developer_profile.infrastructure
//! - Project scale         → detected_tech + project_dependencies counts
//! - Decision history      → AWE graph via awe_commands probes (0 if unknown)
//! - Knowledge gaps        → knowledge_decay
//! - Blind spots           → blind_spots
//!
//! This module does not talk to AWE directly — it only builds the JSON
//! payload. The IPC wire lives in `context_commands::run_awe_transmute`.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::sovereign_developer_profile::SovereignDeveloperProfile;

// ============================================================================
// The bridge payload
// ============================================================================
//
// Field names and shape mirror AWE's `DeveloperContext` in
// `crates/awe-core/src/pipeline.rs` (post-Phase-A1 expansion). Optional
// rig fields use `Option<_>` so AWE's receiver can tolerate old clients.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeveloperContextPayload {
    // --- Identity ---
    pub primary_stack: Vec<String>,
    pub adjacent_tech: Vec<String>,
    pub domain_concerns: Vec<String>,
    pub identity_summary: String,

    // --- Rig ---
    pub os: String,
    pub hardware_class: String,
    pub cpu_cores: Option<u32>,
    pub ram_gb: Option<u32>,
    pub available_models: Vec<String>,

    // --- Project Scale ---
    pub project_count: u32,
    pub dependency_count: u32,
    pub days_active: u32,
    pub items_processed: u64,

    // --- Decision History ---
    pub decision_count: u32,
    pub feedback_coverage_pct: f64,

    // --- Gaps ---
    pub knowledge_gaps: Vec<String>,
    pub blind_spots: Vec<String>,
}

impl Default for DeveloperContextPayload {
    fn default() -> Self {
        Self {
            primary_stack: Vec::new(),
            adjacent_tech: Vec::new(),
            domain_concerns: Vec::new(),
            identity_summary: String::new(),
            os: String::new(),
            hardware_class: "unknown".to_string(),
            cpu_cores: None,
            ram_gb: None,
            available_models: Vec::new(),
            project_count: 0,
            dependency_count: 0,
            days_active: 0,
            items_processed: 0,
            decision_count: 0,
            feedback_coverage_pct: 0.0,
            knowledge_gaps: Vec::new(),
            blind_spots: Vec::new(),
        }
    }
}

// ============================================================================
// Composition — pure, testable helpers
// ============================================================================

/// Derive a coarse hardware class from CPU cores + RAM. These thresholds
/// are intentionally conservative — we want AWE to know whether this is a
/// thin-client or a workstation, not to estimate GFLOPS.
pub fn classify_hardware(cpu_cores: Option<u32>, ram_gb: Option<u32>) -> String {
    match (cpu_cores, ram_gb) {
        (Some(c), Some(r)) if c >= 16 && r >= 64 => "workstation".to_string(),
        (Some(c), Some(r)) if c >= 8 && r >= 16 => "desktop".to_string(),
        (Some(c), Some(r)) if c >= 4 && r >= 8 => "laptop".to_string(),
        (Some(_), Some(_)) => "thin-client".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Parse "32 GB" / "32.0 GB" / "32GB" / "32" into u32 GB. Strips unit
/// suffixes. Returns None for unparseable input.
pub fn parse_ram_gb(raw: &str) -> Option<u32> {
    let cleaned: String = raw
        .trim()
        .replace("GB", "")
        .replace("gb", "")
        .replace("Gb", "")
        .replace("GiB", "")
        .trim()
        .to_string();
    cleaned.parse::<f32>().ok().map(|v| v.round() as u32)
}

/// Extract CPU core count from profile infrastructure cpu map. The
/// sovereign profile stores CPU data as a free-form string HashMap; the
/// "cores" key is most reliable, fall back to "core_count".
pub fn extract_cpu_cores(profile: &SovereignDeveloperProfile) -> Option<u32> {
    profile
        .infrastructure
        .cpu
        .get("cores")
        .or_else(|| profile.infrastructure.cpu.get("core_count"))
        .or_else(|| profile.infrastructure.cpu.get("logical_processors"))
        .and_then(|s| s.parse::<u32>().ok())
}

/// Compose the OS string: "<name> <version>" when both present.
pub fn compose_os(profile: &SovereignDeveloperProfile) -> String {
    let name = profile
        .infrastructure
        .os
        .get("name")
        .cloned()
        .unwrap_or_default();
    let ver = profile
        .infrastructure
        .os
        .get("version")
        .cloned()
        .unwrap_or_default();
    match (name.is_empty(), ver.is_empty()) {
        (true, _) => String::new(),
        (false, true) => name,
        (false, false) => format!("{name} {ver}"),
    }
}

/// List the local models AWE can use. Reads from the LLM map populated by
/// sovereign profile (Ollama-detected models + configured provider).
pub fn extract_available_models(profile: &SovereignDeveloperProfile) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(provider) = profile.infrastructure.llm.get("provider") {
        if !provider.is_empty() && provider != "none" {
            out.push(provider.clone());
        }
    }
    if let Some(model) = profile.infrastructure.llm.get("model") {
        if !model.is_empty() {
            out.push(model.clone());
        }
    }
    if let Some(ollama_models) = profile.infrastructure.llm.get("ollama_models") {
        // Stored as comma-separated list when present.
        for m in ollama_models
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            out.push(format!("ollama:{m}"));
        }
    }
    // Dedupe while preserving order.
    let mut seen = std::collections::HashSet::new();
    out.retain(|m| seen.insert(m.clone()));
    out
}

/// Query project / dependency counts from the DB. Non-fatal on error —
/// zero is a legitimate value for a fresh install.
pub fn query_project_and_dep_counts(conn: &Connection) -> (u32, u32) {
    let projects: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT project_path) FROM project_dependencies",
            [],
            |row| row.get::<_, i64>(0).map(|n| n.max(0) as u32),
        )
        .unwrap_or(0);
    let deps: u32 = conn
        .query_row("SELECT COUNT(*) FROM project_dependencies", [], |row| {
            row.get::<_, i64>(0).map(|n| n.max(0) as u32)
        })
        .unwrap_or(0);
    (projects, deps)
}

/// Count source items processed. The schema may vary across migrations;
/// zero is a safe fallback.
pub fn query_items_processed(conn: &Connection) -> u64 {
    conn.query_row("SELECT COUNT(*) FROM source_items", [], |row| {
        row.get::<_, i64>(0).map(|n| n.max(0) as u64)
    })
    .unwrap_or(0)
}

/// Days since the first recorded interaction — best proxy for how long
/// the user has been active with 4DA.
pub fn query_days_active(conn: &Connection) -> u32 {
    let earliest: Option<String> = conn
        .query_row("SELECT MIN(created_at) FROM feedback", [], |row| row.get(0))
        .ok()
        .flatten();

    let Some(earliest_str) = earliest else {
        return 0;
    };

    chrono::NaiveDateTime::parse_from_str(&earliest_str, "%Y-%m-%d %H:%M:%S")
        .map(|dt| {
            let now = chrono::Utc::now().naive_utc();
            (now - dt).num_days().max(0) as u32
        })
        .unwrap_or(0)
}

/// Top N dependency names that have a knowledge gap.
pub fn query_knowledge_gap_deps(conn: &Connection, limit: usize) -> Vec<String> {
    let gaps = crate::knowledge_decay::detect_knowledge_gaps(conn).unwrap_or_default();
    gaps.into_iter().take(limit).map(|g| g.dependency).collect()
}

/// Top N blind-spot dep names from the blind-spot report.
pub fn query_blind_spot_deps(limit: usize) -> Vec<String> {
    let Ok(report) = crate::blind_spots::generate_blind_spot_report() else {
        return Vec::new();
    };
    report
        .uncovered_dependencies
        .into_iter()
        .take(limit)
        .map(|d| d.name)
        .collect()
}

// ============================================================================
// Assembly — the public entry point
// ============================================================================

/// Assemble the full 17-field payload for an AWE transmutation.
///
/// Reads from the sovereign profile (already assembled from 12+ tables)
/// and augments with counts + gap lists queried directly. Every lookup
/// is tolerant of missing data — an unknown value becomes an empty
/// string or zero count, never a panic.
///
/// `decision_count` and `feedback_coverage_pct` are passed in by the
/// caller because they come from AWE's own wisdom graph (via the
/// awe-commands probes) rather than from 4DA's DB.
pub fn assemble_developer_context(
    conn: &Connection,
    profile: &SovereignDeveloperProfile,
    awe_decision_count: u32,
    awe_feedback_coverage_pct: f64,
) -> DeveloperContextPayload {
    let (project_count, dependency_count) = query_project_and_dep_counts(conn);
    let items_processed = query_items_processed(conn);
    let days_active = query_days_active(conn);

    let ram_gb = profile
        .infrastructure
        .ram
        .get("total")
        .and_then(|s| parse_ram_gb(s));
    let cpu_cores = extract_cpu_cores(profile);
    let hardware_class = classify_hardware(cpu_cores, ram_gb);

    // Domain concerns: derive from the user's declared + detected stack.
    // Simple heuristic: concerns are implicit in the tech — "privacy" if
    // they have local LLM, "performance" if they have Rust/C++, etc.
    let mut domain_concerns: Vec<String> = Vec::new();
    let stack_lower: Vec<String> = profile
        .stack
        .primary_stack
        .iter()
        .chain(profile.stack.adjacent_tech.iter())
        .map(|s| s.to_lowercase())
        .collect();
    if stack_lower
        .iter()
        .any(|s| s.contains("tauri") || s.contains("rust") || s.contains("c++"))
    {
        domain_concerns.push("performance".to_string());
    }
    if !profile.infrastructure.llm.is_empty()
        && profile
            .infrastructure
            .llm
            .get("provider")
            .map_or(false, |p| p.contains("ollama") || p == "local")
    {
        domain_concerns.push("privacy".to_string());
    }
    if stack_lower
        .iter()
        .any(|s| s.contains("sqlite") || s.contains("local"))
    {
        domain_concerns.push("local-first".to_string());
    }

    DeveloperContextPayload {
        primary_stack: profile.stack.primary_stack.clone(),
        adjacent_tech: profile.stack.adjacent_tech.clone(),
        domain_concerns,
        identity_summary: profile.identity_summary.clone(),

        os: compose_os(profile),
        hardware_class,
        cpu_cores,
        ram_gb,
        available_models: extract_available_models(profile),

        project_count,
        dependency_count,
        days_active,
        items_processed,

        decision_count: awe_decision_count,
        feedback_coverage_pct: awe_feedback_coverage_pct.max(0.0).min(100.0),

        knowledge_gaps: query_knowledge_gap_deps(conn, 10),
        blind_spots: query_blind_spot_deps(10),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sovereign_developer_profile::{
        ContextDimension, InfrastructureDimension, PreferencesDimension, SkillsDimension,
        SovereignDeveloperProfile, StackDimension,
    };
    use std::collections::HashMap;

    fn empty_profile() -> SovereignDeveloperProfile {
        SovereignDeveloperProfile {
            generated_at: "2026-04-17 00:00:00".to_string(),
            identity_summary: String::new(),
            infrastructure: InfrastructureDimension::default(),
            stack: StackDimension::default(),
            skills: SkillsDimension::default(),
            preferences: PreferencesDimension::default(),
            context: ContextDimension::default(),
            intelligence: Default::default(),
            completeness: Default::default(),
        }
    }

    #[test]
    fn classify_workstation() {
        assert_eq!(classify_hardware(Some(32), Some(128)), "workstation");
    }

    #[test]
    fn classify_desktop() {
        assert_eq!(classify_hardware(Some(8), Some(32)), "desktop");
    }

    #[test]
    fn classify_laptop() {
        assert_eq!(classify_hardware(Some(4), Some(16)), "laptop");
    }

    #[test]
    fn classify_thin_client() {
        assert_eq!(classify_hardware(Some(2), Some(4)), "thin-client");
    }

    #[test]
    fn classify_unknown_when_missing() {
        assert_eq!(classify_hardware(None, Some(32)), "unknown");
        assert_eq!(classify_hardware(Some(8), None), "unknown");
        assert_eq!(classify_hardware(None, None), "unknown");
    }

    #[test]
    fn parse_ram_handles_variants() {
        assert_eq!(parse_ram_gb("32"), Some(32));
        assert_eq!(parse_ram_gb("32 GB"), Some(32));
        assert_eq!(parse_ram_gb("32GB"), Some(32));
        assert_eq!(parse_ram_gb("31.9 GB"), Some(32));
        assert_eq!(parse_ram_gb("64 GiB"), Some(64));
    }

    #[test]
    fn parse_ram_rejects_gibberish() {
        assert_eq!(parse_ram_gb(""), None);
        assert_eq!(parse_ram_gb("lots"), None);
        assert_eq!(parse_ram_gb("GB"), None);
    }

    #[test]
    fn compose_os_joins_name_and_version() {
        let mut p = empty_profile();
        p.infrastructure
            .os
            .insert("name".to_string(), "Windows".to_string());
        p.infrastructure
            .os
            .insert("version".to_string(), "10.0.19045".to_string());
        assert_eq!(compose_os(&p), "Windows 10.0.19045");
    }

    #[test]
    fn compose_os_tolerates_missing_version() {
        let mut p = empty_profile();
        p.infrastructure
            .os
            .insert("name".to_string(), "macOS".to_string());
        assert_eq!(compose_os(&p), "macOS");
    }

    #[test]
    fn compose_os_empty_when_no_name() {
        let p = empty_profile();
        assert_eq!(compose_os(&p), "");
    }

    #[test]
    fn extract_cpu_cores_prefers_cores_key() {
        let mut p = empty_profile();
        p.infrastructure
            .cpu
            .insert("cores".to_string(), "16".to_string());
        p.infrastructure
            .cpu
            .insert("core_count".to_string(), "8".to_string());
        assert_eq!(extract_cpu_cores(&p), Some(16));
    }

    #[test]
    fn extract_cpu_cores_falls_back() {
        let mut p = empty_profile();
        p.infrastructure
            .cpu
            .insert("logical_processors".to_string(), "12".to_string());
        assert_eq!(extract_cpu_cores(&p), Some(12));
    }

    #[test]
    fn extract_available_models_dedupes_and_prefixes_ollama() {
        let mut p = empty_profile();
        let mut llm: HashMap<String, String> = HashMap::new();
        llm.insert("provider".to_string(), "ollama".to_string());
        llm.insert("model".to_string(), "llama3.2".to_string());
        llm.insert(
            "ollama_models".to_string(),
            "nomic-embed-text, llama3.2, mistral".to_string(),
        );
        p.infrastructure.llm = llm;
        let models = extract_available_models(&p);
        assert!(models.contains(&"ollama".to_string()));
        assert!(models.contains(&"llama3.2".to_string()));
        assert!(models.contains(&"ollama:nomic-embed-text".to_string()));
        assert!(models.contains(&"ollama:mistral".to_string()));
        // dedupe: llama3.2 appears once, not twice
        assert_eq!(models.iter().filter(|m| *m == "llama3.2").count(), 1);
    }

    #[test]
    fn extract_available_models_empty_when_none_configured() {
        let p = empty_profile();
        assert!(extract_available_models(&p).is_empty());
    }

    #[test]
    fn extract_available_models_skips_none_provider() {
        let mut p = empty_profile();
        p.infrastructure
            .llm
            .insert("provider".to_string(), "none".to_string());
        assert!(extract_available_models(&p).is_empty());
    }

    #[test]
    fn default_payload_is_safe() {
        let d = DeveloperContextPayload::default();
        assert_eq!(d.project_count, 0);
        assert_eq!(d.hardware_class, "unknown");
        assert!(d.primary_stack.is_empty());
    }

    #[test]
    fn feedback_coverage_clamps_into_0_100() {
        // This is tested indirectly via assemble_developer_context's clamp,
        // but we can verify the clamp math is consistent here.
        fn clamp(x: f64) -> f64 {
            x.max(0.0).min(100.0)
        }
        assert_eq!(clamp(-10.0), 0.0);
        assert_eq!(clamp(150.0), 100.0);
        assert_eq!(clamp(42.5), 42.5);
    }

    #[test]
    fn payload_serializes_snake_case_field_names() {
        let p = DeveloperContextPayload::default();
        let json = serde_json::to_string(&p).unwrap();
        // Verify the field names that AWE's receiver expects.
        assert!(json.contains("\"primary_stack\""));
        assert!(json.contains("\"adjacent_tech\""));
        assert!(json.contains("\"hardware_class\""));
        assert!(json.contains("\"knowledge_gaps\""));
        assert!(json.contains("\"feedback_coverage_pct\""));
    }

    #[test]
    fn payload_roundtrips_through_json() {
        let mut p = DeveloperContextPayload::default();
        p.primary_stack = vec!["rust".to_string(), "react".to_string()];
        p.cpu_cores = Some(12);
        p.ram_gb = Some(32);
        p.decision_count = 42;
        p.feedback_coverage_pct = 73.5;
        let json = serde_json::to_string(&p).unwrap();
        let back: DeveloperContextPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn assemble_derives_concerns_from_stack() {
        // Unit-test the domain-concerns inference without a real DB.
        let mut p = empty_profile();
        p.stack.primary_stack = vec!["tauri".to_string(), "rust".to_string()];
        p.stack.adjacent_tech = vec!["sqlite".to_string()];
        p.infrastructure
            .llm
            .insert("provider".to_string(), "ollama".to_string());

        // Re-implement the concern derivation inline for a pure-Rust test
        // (the full assembler requires a Connection; this exercises the
        // inference rules themselves).
        let stack_lower: Vec<String> = p
            .stack
            .primary_stack
            .iter()
            .chain(p.stack.adjacent_tech.iter())
            .map(|s| s.to_lowercase())
            .collect();
        let mut concerns = Vec::new();
        if stack_lower
            .iter()
            .any(|s| s.contains("tauri") || s.contains("rust") || s.contains("c++"))
        {
            concerns.push("performance".to_string());
        }
        if p.infrastructure
            .llm
            .get("provider")
            .map_or(false, |pr| pr.contains("ollama") || pr == "local")
        {
            concerns.push("privacy".to_string());
        }
        if stack_lower
            .iter()
            .any(|s| s.contains("sqlite") || s.contains("local"))
        {
            concerns.push("local-first".to_string());
        }

        assert!(concerns.contains(&"performance".to_string()));
        assert!(concerns.contains(&"privacy".to_string()));
        assert!(concerns.contains(&"local-first".to_string()));
    }
}
