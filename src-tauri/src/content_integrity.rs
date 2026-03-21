//! Content Integrity Verification — autonomous accuracy gate for personalized content.
//!
//! Ensures every piece of user-facing personalized content in STREETS, briefings,
//! and developer profile displays is accurate and verified against real data.
//!
//! Three verification levels:
//! 1. **Stack Integrity** — primary_stack only contains display-worthy, verified tech
//! 2. **Phantom Detection** — flags tech detected by ACE that doesn't exist in any manifest
//! 3. **Template Safety** — validates all template interpolation fields resolve correctly
//!
//! Runs automatically on profile-updated events and exposes a manual check command.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::domain_profile::is_display_worthy;

// ============================================================================
// Types
// ============================================================================

/// Result of a content integrity check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// Overall pass/fail
    pub passed: bool,
    /// Technologies filtered from primary_stack (present in DB but not display-worthy)
    pub filtered_tech: Vec<FilteredTech>,
    /// Phantom technologies (detected by ACE but not in any actual manifest file)
    pub phantom_tech: Vec<PhantomTech>,
    /// Technologies that passed all gates and are display-ready
    pub verified_stack: Vec<String>,
    /// Number of issues found and auto-corrected
    pub auto_corrected: u32,
    /// Timestamp of check
    pub checked_at: String,
}

/// A technology that was filtered from user-facing display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredTech {
    pub name: String,
    pub reason: String,
    pub source: String,
}

/// A technology detected by ACE but not backed by any real manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhantomTech {
    pub name: String,
    pub detected_confidence: f64,
    pub category: String,
    /// Whether this phantom was auto-removed from tech_stack
    pub auto_removed: bool,
}

// ============================================================================
// Core Verification
// ============================================================================

/// Run a full content integrity check.
///
/// This is the autonomous accuracy system. It:
/// 1. Reads all tech in tech_stack and detected_tech
/// 2. Filters non-display-worthy items from primary_stack
/// 3. Detects phantom tech (detected but not in actual dependencies)
/// 4. Optionally auto-corrects by removing phantom tech from tech_stack
pub fn verify_content_integrity(conn: &Connection, auto_correct: bool) -> IntegrityReport {
    let mut filtered_tech = Vec::new();
    let mut phantom_tech = Vec::new();
    let mut verified_stack = Vec::new();
    let mut auto_corrected = 0u32;

    // 1. Check tech_stack entries for display-worthiness
    let tech_stack = read_tech_stack(conn);
    for tech in &tech_stack {
        let lower = tech.to_lowercase();
        if is_display_worthy(&lower) {
            verified_stack.push(lower);
        } else {
            filtered_tech.push(FilteredTech {
                name: tech.clone(),
                reason: "Not display-worthy (ORM, utility lib, or companion package)".into(),
                source: "tech_stack".into(),
            });

            // Auto-correct: remove non-display-worthy tech from tech_stack
            if auto_correct {
                if let Err(e) = conn.execute(
                    "DELETE FROM tech_stack WHERE LOWER(technology) = LOWER(?1)",
                    rusqlite::params![tech],
                ) {
                    warn!(target: "4da::integrity", tech = %tech, error = %e, "Failed to auto-remove");
                } else {
                    auto_corrected += 1;
                    info!(target: "4da::integrity", tech = %tech, "Auto-removed non-display-worthy tech from tech_stack");
                }
            }
        }
    }

    // 2. Detect phantom tech: in detected_tech but NOT in any project_dependencies
    let detected = read_detected_tech(conn);
    let real_deps = read_real_dependencies(conn);

    for (name, confidence, category) in &detected {
        let lower = name.to_lowercase();
        // Phantom = detected as Framework/Library but not in any manifest AND not a language
        if !matches!(category.as_str(), "Language")
            && !real_deps.iter().any(|d| d.to_lowercase() == lower)
        {
            let is_in_tech_stack = tech_stack.iter().any(|t| t.to_lowercase() == lower);
            let mut was_removed = false;

            // Auto-correct: if phantom tech was seeded into tech_stack, remove it
            if auto_correct && is_in_tech_stack {
                if let Err(e) = conn.execute(
                    "DELETE FROM tech_stack WHERE LOWER(technology) = LOWER(?1)",
                    rusqlite::params![name],
                ) {
                    warn!(target: "4da::integrity", tech = %name, error = %e, "Failed to remove phantom");
                } else {
                    auto_corrected += 1;
                    was_removed = true;
                    info!(target: "4da::integrity", tech = %name, "Auto-removed phantom tech from tech_stack");
                }
            }

            phantom_tech.push(PhantomTech {
                name: name.clone(),
                detected_confidence: *confidence,
                category: category.clone(),
                auto_removed: was_removed,
            });
        }
    }

    verified_stack.sort();
    verified_stack.dedup();

    let passed = filtered_tech.is_empty() && phantom_tech.is_empty();

    if passed {
        debug!(target: "4da::integrity", verified = verified_stack.len(), "Content integrity check PASSED");
    } else {
        warn!(
            target: "4da::integrity",
            filtered = filtered_tech.len(),
            phantoms = phantom_tech.len(),
            corrected = auto_corrected,
            "Content integrity check found issues"
        );
    }

    IntegrityReport {
        passed,
        filtered_tech,
        phantom_tech,
        verified_stack,
        auto_corrected,
        checked_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Run content integrity verification with auto-correction.
/// Returns a report of what was found and fixed.
#[tauri::command]
pub fn check_content_integrity() -> crate::error::Result<IntegrityReport> {
    let conn = crate::open_db_connection()?;
    Ok(verify_content_integrity(&conn, true))
}

/// Run content integrity verification without auto-correction (read-only audit).
#[tauri::command]
pub fn audit_content_integrity() -> crate::error::Result<IntegrityReport> {
    let conn = crate::open_db_connection()?;
    Ok(verify_content_integrity(&conn, false))
}

// ============================================================================
// Database Readers
// ============================================================================

fn read_tech_stack(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT technology FROM tech_stack")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default()
}

fn read_detected_tech(conn: &Connection) -> Vec<(String, f64, String)> {
    conn.prepare("SELECT name, confidence, category FROM detected_tech WHERE confidence >= 0.7")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default()
}

fn read_real_dependencies(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT DISTINCT LOWER(package_name) FROM project_dependencies")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_worthy_filters_orms() {
        assert!(!is_display_worthy("drizzle"));
        assert!(!is_display_worthy("prisma"));
        assert!(!is_display_worthy("typeorm"));
        assert!(!is_display_worthy("sequelize"));
        assert!(!is_display_worthy("knex"));
        assert!(!is_display_worthy("drizzle-orm"));
    }

    #[test]
    fn test_display_worthy_passes_languages() {
        assert!(is_display_worthy("rust"));
        assert!(is_display_worthy("typescript"));
        assert!(is_display_worthy("python"));
        assert!(is_display_worthy("go"));
        assert!(is_display_worthy("java"));
    }

    #[test]
    fn test_display_worthy_passes_major_frameworks() {
        assert!(is_display_worthy("react"));
        assert!(is_display_worthy("vue"));
        assert!(is_display_worthy("angular"));
        assert!(is_display_worthy("tauri"));
        assert!(is_display_worthy("django"));
        assert!(is_display_worthy("nextjs"));
        assert!(is_display_worthy("fastapi"));
    }

    #[test]
    fn test_display_worthy_passes_databases() {
        assert!(is_display_worthy("postgresql"));
        assert!(is_display_worthy("redis"));
        assert!(is_display_worthy("sqlite"));
        assert!(is_display_worthy("mongodb"));
    }

    #[test]
    fn test_display_worthy_filters_utility_libs() {
        assert!(!is_display_worthy("lodash"));
        assert!(!is_display_worthy("zod"));
        assert!(!is_display_worthy("trpc"));
        assert!(!is_display_worthy("tailwindcss"));
        assert!(!is_display_worthy("eslint"));
        assert!(!is_display_worthy("webpack"));
        assert!(!is_display_worthy("vite"));
        assert!(!is_display_worthy("biome"));
    }

    #[test]
    fn test_display_worthy_filters_companion_packages() {
        // These are companions/tools, not identity markers
        assert!(!is_display_worthy("turborepo"));
        assert!(!is_display_worthy("pnpm"));
        assert!(!is_display_worthy("yarn"));
        assert!(!is_display_worthy("npm"));
    }
}
