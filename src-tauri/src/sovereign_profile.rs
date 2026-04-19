// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Sovereign Profile — accumulates hardware/system facts from STREETS commands.
//!
//! Facts are extracted automatically from command output and stored in the
//! `sovereign_profile` table. The module also generates a "Sovereign Stack
//! Document" (STREETS Lesson 6 deliverable) from accumulated facts.
//!
//! Fact extraction and storage helpers live in [`crate::sovereign_facts`].

use crate::error::{FourDaError, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::debug;

// Re-export fact helpers so callers using `crate::sovereign_profile::*` still work.
pub use crate::sovereign_facts::{log_command_execution, store_facts_from_execution};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignFact {
    pub category: String,
    pub key: String,
    pub value: String,
    pub source_lesson: Option<String>,
    pub confidence: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignProfileData {
    pub facts: Vec<SovereignFact>,
    pub categories: Vec<CategorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: String,
    pub fact_count: usize,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCompleteness {
    pub total_categories: usize,
    pub filled_categories: usize,
    pub percentage: f64,
    pub missing: Vec<String>,
}

/// All categories tracked by the sovereign profile.
const ALL_CATEGORIES: &[&str] = &[
    "cpu", "ram", "gpu", "storage", "network", "os", "llm", "legal", "budget",
];

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_sovereign_profile() -> Result<SovereignProfileData> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn
        .prepare(
            "SELECT category, key, value, source_lesson, confidence, updated_at
             FROM sovereign_profile ORDER BY category, key",
        )
        .map_err(FourDaError::Db)?;

    let facts: Vec<SovereignFact> = stmt
        .query_map([], |row| {
            Ok(SovereignFact {
                category: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                source_lesson: row.get(3)?,
                confidence: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in sovereign_profile: {e}");
                None
            }
        })
        .collect();

    // Build category summaries
    let mut cat_map: std::collections::HashMap<String, (usize, Option<String>)> =
        std::collections::HashMap::new();
    for fact in &facts {
        let entry = cat_map.entry(fact.category.clone()).or_insert((0, None));
        entry.0 += 1;
        if entry.1.is_none() || entry.1.as_deref() < Some(&fact.updated_at) {
            entry.1 = Some(fact.updated_at.clone());
        }
    }

    let categories = cat_map
        .into_iter()
        .map(|(cat, (count, last))| CategorySummary {
            category: cat,
            fact_count: count,
            last_updated: last,
        })
        .collect();

    Ok(SovereignProfileData { facts, categories })
}

#[tauri::command]
pub async fn get_sovereign_profile_completeness() -> Result<ProfileCompleteness> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn
        .prepare("SELECT DISTINCT category FROM sovereign_profile")
        .map_err(FourDaError::Db)?;

    let filled: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(FourDaError::Db)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in sovereign_profile: {e}");
                None
            }
        })
        .collect();

    let total = ALL_CATEGORIES.len();
    let filled_count = ALL_CATEGORIES
        .iter()
        .filter(|c| filled.contains(**c))
        .count();
    let missing: Vec<String> = ALL_CATEGORIES
        .iter()
        .filter(|c| !filled.contains(**c))
        .map(std::string::ToString::to_string)
        .collect();

    let percentage = if total > 0 {
        (filled_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(ProfileCompleteness {
        total_categories: total,
        filled_categories: filled_count,
        percentage,
        missing,
    })
}

#[tauri::command]
pub async fn generate_sovereign_stack_document() -> Result<String> {
    let profile = get_sovereign_profile().await?;
    let completeness = get_sovereign_profile_completeness().await?;

    let mut doc = String::new();
    doc.push_str("# Sovereign Stack Document\n\n");
    doc.push_str(&format!(
        "**Profile Completeness:** {:.0}% ({}/{} categories)\n\n",
        completeness.percentage, completeness.filled_categories, completeness.total_categories,
    ));
    doc.push_str("---\n\n");

    // Group facts by category
    let mut by_category: std::collections::BTreeMap<String, Vec<&SovereignFact>> =
        std::collections::BTreeMap::new();
    for fact in &profile.facts {
        by_category
            .entry(fact.category.clone())
            .or_default()
            .push(fact);
    }

    let category_labels: std::collections::HashMap<&str, &str> = [
        ("cpu", "CPU / Processor"),
        ("ram", "Memory (RAM)"),
        ("gpu", "GPU / Accelerator"),
        ("storage", "Storage"),
        ("network", "Network"),
        ("os", "Operating System"),
        ("llm", "LLM Infrastructure"),
        ("legal", "Legal Entity"),
        ("budget", "Budget / Runway"),
    ]
    .into_iter()
    .collect();

    for cat in ALL_CATEGORIES {
        let label = category_labels.get(cat).unwrap_or(cat);
        doc.push_str(&format!("## {label}\n\n"));

        if let Some(facts) = by_category.get(*cat) {
            for fact in facts {
                doc.push_str(&format!("- **{}:** {}\n", fact.key, fact.value));
            }
        } else {
            doc.push_str(
                "_No data collected yet. Run the relevant STREETS commands to populate._\n",
            );
        }
        doc.push('\n');
    }

    doc.push_str("---\n\n");
    doc.push_str(
        "_Generated by 4DA Sovereign Profile. Run STREETS Module S commands to auto-populate._\n",
    );

    debug!(target: "4da::sovereign", doc_len = doc.len(), "Generated Sovereign Stack Document");
    Ok(doc)
}

#[tauri::command]
pub async fn save_sovereign_fact(category: String, key: String, value: String) -> Result<()> {
    if category.is_empty() || key.is_empty() || value.is_empty() {
        return Err(FourDaError::Config(
            "Category, key, and value are all required".into(),
        ));
    }

    let conn = crate::open_db_connection()?;

    conn.execute(
        "INSERT INTO sovereign_profile (category, key, value, source_command, source_lesson, confidence)
         VALUES (?1, ?2, ?3, 'manual', 'manual', 1.0)
         ON CONFLICT(category, key) DO UPDATE SET
            value = excluded.value,
            source_command = 'manual',
            source_lesson = 'manual',
            updated_at = datetime('now')",
        params![category, key, value],
    )
    .map_err(FourDaError::Db)?;

    debug!(target: "4da::sovereign", category = %category, key = %key, "Saved manual sovereign fact");
    Ok(())
}

#[tauri::command]
pub async fn get_execution_log(
    module_id: String,
    lesson_idx: Option<usize>,
) -> Result<Vec<serde_json::Value>> {
    let conn = crate::open_db_connection()?;

    let rows: Vec<serde_json::Value> = if let Some(idx) = lesson_idx {
        let mut stmt = conn
            .prepare(
                "SELECT id, module_id, lesson_idx, command_id, command_text, success,
                        exit_code, stdout, stderr, duration_ms, executed_at
                 FROM command_execution_log
                 WHERE module_id = ?1 AND lesson_idx = ?2
                 ORDER BY executed_at DESC LIMIT 100",
            )
            .map_err(FourDaError::Db)?;
        let result: Vec<serde_json::Value> = stmt
            .query_map(params![module_id, idx as i64], row_to_json)
            .map_err(FourDaError::Db)?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in sovereign_profile: {e}");
                    None
                }
            })
            .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, module_id, lesson_idx, command_id, command_text, success,
                        exit_code, stdout, stderr, duration_ms, executed_at
                 FROM command_execution_log
                 WHERE module_id = ?1
                 ORDER BY executed_at DESC LIMIT 100",
            )
            .map_err(FourDaError::Db)?;
        let result: Vec<serde_json::Value> = stmt
            .query_map(params![module_id], row_to_json)
            .map_err(FourDaError::Db)?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in sovereign_profile: {e}");
                    None
                }
            })
            .collect();
        result
    };

    Ok(rows)
}

fn row_to_json(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "module_id": row.get::<_, String>(1)?,
        "lesson_idx": row.get::<_, i64>(2)?,
        "command_id": row.get::<_, String>(3)?,
        "command_text": row.get::<_, String>(4)?,
        "success": row.get::<_, i32>(5)? != 0,
        "exit_code": row.get::<_, Option<i32>>(6)?,
        "stdout": row.get::<_, Option<String>>(7)?,
        "stderr": row.get::<_, Option<String>>(8)?,
        "duration_ms": row.get::<_, Option<i64>>(9)?,
        "executed_at": row.get::<_, Option<String>>(10)?,
    }))
}
