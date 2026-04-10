// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Intelligence Packs for 4DA
//!
//! Domain-specific intelligence configurations that combine curated sources,
//! concept tracking, scoring overrides, and auto-created standing queries.
//! Packs create depth without noise — specialized intelligence for specific stacks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use tracing::{info, warn};

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct IntelligencePack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub concepts: Vec<PackConcept>,
    pub default_watches: Vec<String>,
    pub active: bool,
    pub activated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PackConcept {
    pub name: String,
    pub keywords: Vec<String>,
    pub importance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PackSuggestion {
    pub pack_id: String,
    pub reason: String,
}

// ============================================================================
// Built-in Packs
// ============================================================================

pub fn builtin_packs() -> Vec<IntelligencePack> {
    vec![
        IntelligencePack {
            id: "rust-security".into(),
            name: "Rust Security".into(),
            description: "Track RustSec advisories, unsafe code patterns, supply chain risks, and Rust ecosystem security updates.".into(),
            icon: "\u{1F980}".into(),
            concepts: vec![
                PackConcept { name: "CVE".into(), keywords: vec!["vulnerability".into(), "advisory".into(), "CVE".into(), "RUSTSEC".into(), "security".into()], importance: 1.0 },
                PackConcept { name: "Unsafe".into(), keywords: vec!["unsafe".into(), "soundness".into(), "undefined behavior".into(), "UB".into()], importance: 0.8 },
                PackConcept { name: "Supply Chain".into(), keywords: vec!["typosquat".into(), "malicious crate".into(), "dependency confusion".into()], importance: 0.9 },
            ],
            default_watches: vec![
                "Rust security advisories for my dependencies".into(),
                "New RUSTSEC entries affecting my crates".into(),
            ],
            active: false,
            activated_at: None,
        },
        IntelligencePack {
            id: "frontend-shifts".into(),
            name: "Frontend Shifts".into(),
            description: "Track React/Vue/Svelte/Angular changes, bundler evolution, TC39 proposals, and web platform updates.".into(),
            icon: "\u{269B}".into(),
            concepts: vec![
                PackConcept { name: "React".into(), keywords: vec!["React".into(), "RSC".into(), "Server Components".into(), "Suspense".into(), "Next.js".into()], importance: 1.0 },
                PackConcept { name: "Bundlers".into(), keywords: vec!["Vite".into(), "webpack".into(), "Turbopack".into(), "Rspack".into(), "esbuild".into(), "Rollup".into()], importance: 0.8 },
                PackConcept { name: "Web Platform".into(), keywords: vec!["CSS".into(), "Web API".into(), "Baseline".into(), "TC39".into(), "ECMAScript".into()], importance: 0.7 },
                PackConcept { name: "TypeScript".into(), keywords: vec!["TypeScript".into(), "tsc".into(), "type system".into(), "tsconfig".into()], importance: 0.8 },
            ],
            default_watches: vec![
                "Breaking changes in React ecosystem".into(),
                "New TC39 proposals reaching Stage 3+".into(),
            ],
            active: false,
            activated_at: None,
        },
        IntelligencePack {
            id: "ai-engineering".into(),
            name: "AI Engineering".into(),
            description: "Track model releases, API changes, MCP developments, agent frameworks, and AI safety updates.".into(),
            icon: "\u{1F916}".into(),
            concepts: vec![
                PackConcept { name: "Model Release".into(), keywords: vec!["GPT".into(), "Claude".into(), "Llama".into(), "Gemini".into(), "model".into(), "release".into(), "benchmark".into()], importance: 1.0 },
                PackConcept { name: "API Changes".into(), keywords: vec!["API".into(), "deprecation".into(), "breaking change".into(), "SDK".into(), "Anthropic".into(), "OpenAI".into()], importance: 0.9 },
                PackConcept { name: "MCP".into(), keywords: vec!["Model Context Protocol".into(), "MCP".into(), "tool use".into(), "function calling".into()], importance: 0.8 },
                PackConcept { name: "Agent Frameworks".into(), keywords: vec!["LangChain".into(), "LlamaIndex".into(), "CrewAI".into(), "AutoGen".into(), "agent".into()], importance: 0.7 },
            ],
            default_watches: vec![
                "New Claude or GPT model releases".into(),
                "MCP protocol updates and new tools".into(),
            ],
            active: false,
            activated_at: None,
        },
        IntelligencePack {
            id: "supply-chain-security".into(),
            name: "Supply Chain Security".into(),
            description: "Track OSV advisories, dependency vulnerabilities, package manager security, and SBOM/SLSA developments.".into(),
            icon: "\u{1F512}".into(),
            concepts: vec![
                PackConcept { name: "Vulnerability".into(), keywords: vec!["CVE".into(), "vulnerability".into(), "advisory".into(), "exploit".into(), "patch".into()], importance: 1.0 },
                PackConcept { name: "Dependency Risk".into(), keywords: vec!["dependency".into(), "transitive".into(), "supply chain".into(), "lockfile".into()], importance: 0.9 },
                PackConcept { name: "Supply Chain".into(), keywords: vec!["SBOM".into(), "SLSA".into(), "attestation".into(), "provenance".into(), "Sigstore".into()], importance: 0.8 },
            ],
            default_watches: vec![
                "Critical CVEs in my dependency tree".into(),
                "Supply chain security best practices".into(),
            ],
            active: false,
            activated_at: None,
        },
        IntelligencePack {
            id: "infrastructure".into(),
            name: "Infrastructure".into(),
            description: "Track Docker, Kubernetes, cloud provider changes, Terraform updates, and infrastructure tooling shifts.".into(),
            icon: "\u{2601}".into(),
            concepts: vec![
                PackConcept { name: "Containers".into(), keywords: vec!["Docker".into(), "container".into(), "OCI".into(), "Podman".into()], importance: 0.9 },
                PackConcept { name: "Kubernetes".into(), keywords: vec!["Kubernetes".into(), "k8s".into(), "Helm".into(), "operator".into()], importance: 0.9 },
                PackConcept { name: "Cloud".into(), keywords: vec!["AWS".into(), "GCP".into(), "Azure".into(), "cloud".into(), "serverless".into()], importance: 0.8 },
                PackConcept { name: "IaC".into(), keywords: vec!["Terraform".into(), "Pulumi".into(), "CDK".into(), "infrastructure as code".into()], importance: 0.7 },
            ],
            default_watches: vec![
                "Breaking changes in Docker or Kubernetes".into(),
                "Major cloud provider announcements".into(),
            ],
            active: false,
            activated_at: None,
        },
        IntelligencePack {
            id: "tauri-ecosystem".into(),
            name: "Tauri Ecosystem".into(),
            description: "Track Tauri releases, WRY/TAO updates, WebView changes, and cross-platform desktop development.".into(),
            icon: "\u{1F5A5}".into(),
            concepts: vec![
                PackConcept { name: "Tauri".into(), keywords: vec!["Tauri".into(), "tauri".into(), "WRY".into(), "TAO".into()], importance: 1.0 },
                PackConcept { name: "WebView".into(), keywords: vec!["WebView".into(), "WebKit".into(), "WebView2".into(), "WebKitGTK".into()], importance: 0.8 },
                PackConcept { name: "Desktop".into(), keywords: vec!["Electron".into(), "desktop app".into(), "cross-platform".into(), "native".into()], importance: 0.6 },
            ],
            default_watches: vec![
                "Tauri releases and breaking changes".into(),
                "WebView security updates".into(),
            ],
            active: false,
            activated_at: None,
        },
    ]
}

// ============================================================================
// Core Functions
// ============================================================================

/// Ensure the intelligence_packs table exists.
fn ensure_table(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS intelligence_packs (
            pack_id TEXT PRIMARY KEY,
            active INTEGER DEFAULT 1,
            activated_at TEXT DEFAULT (datetime('now'))
        )",
        [],
    )
    .context("Failed to create intelligence_packs table")?;
    Ok(())
}

/// Get all packs with their activation status from DB.
pub fn list_packs() -> Result<Vec<IntelligencePack>> {
    let packs = builtin_packs();
    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    let activated: std::collections::HashMap<String, String> = conn
        .prepare("SELECT pack_id, activated_at FROM intelligence_packs WHERE active = 1")?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(packs
        .into_iter()
        .map(|mut p| {
            if let Some(ts) = activated.get(&p.id) {
                p.active = true;
                p.activated_at = Some(ts.clone());
            }
            p
        })
        .collect())
}

/// Activate a pack — creates standing queries and marks as active.
pub fn activate_pack(pack_id: &str) -> Result<()> {
    let packs = builtin_packs();
    let pack = packs
        .iter()
        .find(|p| p.id == pack_id)
        .ok_or_else(|| crate::error::FourDaError::Internal(format!("Pack not found: {pack_id}")))?;

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    // Mark as active
    conn.execute(
        "INSERT OR REPLACE INTO intelligence_packs (pack_id, active, activated_at) VALUES (?1, 1, datetime('now'))",
        rusqlite::params![pack_id],
    )
    .context("Failed to activate pack")?;

    // Create standing queries for the pack.
    // We ensure the standing_queries table exists and insert directly
    // rather than calling the async Tauri command.
    crate::standing_queries::ensure_table(&conn)?;

    for watch in &pack.default_watches {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM standing_queries WHERE query_text = ?1",
                rusqlite::params![watch],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !exists {
            // Extract keywords the same way standing_queries does
            let keywords = extract_pack_keywords(watch);
            if keywords.is_empty() {
                continue;
            }
            let keywords_json =
                serde_json::to_string(&keywords).unwrap_or_else(|_| "[]".to_string());

            if let Err(e) = conn.execute(
                "INSERT INTO standing_queries (query_text, keywords) VALUES (?1, ?2)",
                rusqlite::params![watch, keywords_json],
            ) {
                warn!(target: "4da::packs", pack = %pack_id, error = %e, "Failed to create standing query for pack");
            }
        }
    }

    info!(target: "4da::packs", pack = %pack_id, "Intelligence pack activated");
    Ok(())
}

/// Deactivate a pack.
pub fn deactivate_pack(pack_id: &str) -> Result<()> {
    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    conn.execute(
        "UPDATE intelligence_packs SET active = 0 WHERE pack_id = ?1",
        rusqlite::params![pack_id],
    )
    .context("Failed to deactivate pack")?;

    info!(target: "4da::packs", pack = %pack_id, "Intelligence pack deactivated");
    Ok(())
}

/// Suggest packs based on detected stack.
pub fn suggest_packs_for_stack() -> Result<Vec<PackSuggestion>> {
    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    let mut suggestions = Vec::new();

    // Get detected tech (table may not exist yet)
    let tech: Vec<String> = conn
        .prepare("SELECT DISTINCT LOWER(name) FROM detected_tech")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Get dependency types (table may not exist yet)
    let dep_types: Vec<String> = conn
        .prepare("SELECT DISTINCT dep_type FROM user_dependencies")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Suggest based on detected tech
    if tech.iter().any(|t| t.contains("rust")) || dep_types.contains(&"cargo".to_string()) {
        suggestions.push(PackSuggestion {
            pack_id: "rust-security".into(),
            reason: "Rust dependencies detected in your projects".into(),
        });
    }

    if tech.iter().any(|t| {
        t.contains("react")
            || t.contains("vue")
            || t.contains("svelte")
            || t.contains("angular")
    }) || dep_types.contains(&"npm".to_string())
    {
        suggestions.push(PackSuggestion {
            pack_id: "frontend-shifts".into(),
            reason: "Frontend framework detected in your projects".into(),
        });
    }

    if tech.iter().any(|t| {
        t.contains("openai")
            || t.contains("anthropic")
            || t.contains("langchain")
            || t.contains("llm")
    }) {
        suggestions.push(PackSuggestion {
            pack_id: "ai-engineering".into(),
            reason: "AI/ML libraries detected in your projects".into(),
        });
    }

    if tech.iter().any(|t| t.contains("tauri")) {
        suggestions.push(PackSuggestion {
            pack_id: "tauri-ecosystem".into(),
            reason: "Tauri framework detected".into(),
        });
    }

    // Always suggest supply chain security if there are dependencies
    if !dep_types.is_empty() {
        suggestions.push(PackSuggestion {
            pack_id: "supply-chain-security".into(),
            reason: format!("You have {} dependency types to monitor", dep_types.len()),
        });
    }

    if tech.iter().any(|t| {
        t.contains("docker") || t.contains("kubernetes") || t.contains("terraform")
    }) {
        suggestions.push(PackSuggestion {
            pack_id: "infrastructure".into(),
            reason: "Infrastructure tooling detected".into(),
        });
    }

    // Filter out already-activated packs
    let activated: std::collections::HashSet<String> = conn
        .prepare("SELECT pack_id FROM intelligence_packs WHERE active = 1")
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    suggestions.retain(|s| !activated.contains(&s.pack_id));

    Ok(suggestions)
}

// ============================================================================
// Helpers
// ============================================================================

/// Simple keyword extraction for pack watches (mirrors standing_queries logic).
fn extract_pack_keywords(query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "is", "it", "that", "this", "was", "are", "be", "has", "have", "had", "do",
        "does", "did", "will", "would", "could", "should", "may", "might", "can", "shall", "not",
        "no", "so", "if", "then", "than", "when", "where", "what", "which", "who", "how", "all",
        "each", "every", "any", "some", "such", "only", "own", "same", "other", "into", "about",
        "up", "out", "just", "also", "very", "my", "me", "i", "we", "you", "your", "our", "they",
        "them", "their", "show", "find", "get", "give", "tell", "list", "display",
    ];
    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() > 2 && !stop_set.contains(w))
        .map(std::string::ToString::to_string)
        .collect()
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn list_intelligence_packs() -> Result<Vec<IntelligencePack>> {
    list_packs()
}

#[tauri::command]
pub async fn activate_intelligence_pack(pack_id: String) -> Result<()> {
    activate_pack(&pack_id)
}

#[tauri::command]
pub async fn deactivate_intelligence_pack(pack_id: String) -> Result<()> {
    deactivate_pack(&pack_id)
}

#[tauri::command]
pub async fn suggest_intelligence_packs() -> Result<Vec<PackSuggestion>> {
    suggest_packs_for_stack()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packs_have_valid_ids() {
        let packs = builtin_packs();
        assert!(packs.len() >= 6, "Should have at least 6 built-in packs");
        for pack in &packs {
            assert!(!pack.id.is_empty(), "Pack id should not be empty");
            assert!(!pack.name.is_empty(), "Pack name should not be empty");
            assert!(
                !pack.concepts.is_empty(),
                "Pack {} should have concepts",
                pack.id
            );
            assert!(
                !pack.default_watches.is_empty(),
                "Pack {} should have default watches",
                pack.id
            );
            assert!(!pack.active, "Built-in packs should default to inactive");
        }
    }

    #[test]
    fn extract_pack_keywords_works() {
        let kw = extract_pack_keywords("Rust security advisories for my dependencies");
        assert!(kw.contains(&"rust".to_string()));
        assert!(kw.contains(&"security".to_string()));
        assert!(kw.contains(&"advisories".to_string()));
        assert!(kw.contains(&"dependencies".to_string()));
        // Stop words filtered
        assert!(!kw.contains(&"for".to_string()));
    }

    #[test]
    fn pack_ids_are_unique() {
        let packs = builtin_packs();
        let ids: std::collections::HashSet<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), packs.len(), "Pack IDs must be unique");
    }

    #[test]
    fn concept_importance_in_range() {
        for pack in builtin_packs() {
            for concept in &pack.concepts {
                assert!(
                    (0.0..=1.0).contains(&concept.importance),
                    "Concept '{}' in pack '{}' has out-of-range importance: {}",
                    concept.name,
                    pack.id,
                    concept.importance
                );
            }
        }
    }
}
