// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Startup health self-check for 4DA.
//!
//! Validates the app is in a good state when it launches.
//! All checks are fast (< 100ms total), offline, and never panic.

use serde::Serialize;
use std::path::PathBuf;
use tracing::{info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HealthIssue {
    pub component: &'static str,
    pub severity: HealthSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum HealthSeverity {
    Warning,
    Error,
}

// ============================================================================
// Public API
// ============================================================================

/// Run all startup health checks and return any issues found.
///
/// Designed to be fast (< 100ms), offline (no network calls), and infallible.
pub(crate) fn run_startup_health_check() -> Vec<HealthIssue> {
    let mut issues = Vec::new();
    let data_dir = get_data_dir();

    check_database(&data_dir, &mut issues);
    check_settings(&data_dir, &mut issues);
    check_embedding_provider(&data_dir, &mut issues);
    check_source_adapters(&mut issues);
    check_disk_write(&data_dir, &mut issues);

    // Log results
    if issues.is_empty() {
        info!(target: "4da::startup", "Health check passed: all systems nominal");
    } else {
        let errors = issues
            .iter()
            .filter(|i| i.severity == HealthSeverity::Error)
            .count();
        let warnings = issues
            .iter()
            .filter(|i| i.severity == HealthSeverity::Warning)
            .count();
        for issue in &issues {
            match issue.severity {
                HealthSeverity::Error => {
                    warn!(target: "4da::startup", component = issue.component, "HEALTH ERROR: {}", issue.message);
                }
                HealthSeverity::Warning => {
                    warn!(target: "4da::startup", component = issue.component, "HEALTH WARNING: {}", issue.message);
                }
            }
        }
        info!(target: "4da::startup", errors, warnings, "Health check complete with issues");
    }

    issues
}

// ============================================================================
// Individual Checks
// ============================================================================

/// Resolve the data directory (same logic as state.rs get_db_path).
fn get_data_dir() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base
}

/// Check 1: Database file exists and is readable.
pub(crate) fn check_database(data_dir: &PathBuf, issues: &mut Vec<HealthIssue>) {
    let db_path = data_dir.join("4da.db");
    if !db_path.exists() {
        // Not an error on first run — the DB will be created.
        issues.push(HealthIssue {
            component: "database",
            severity: HealthSeverity::Warning,
            message: format!(
                "Database not found at {}. Will be created on first use.",
                db_path.display()
            ),
        });
        return;
    }
    match std::fs::metadata(&db_path) {
        Ok(meta) => {
            if meta.len() == 0 {
                issues.push(HealthIssue {
                    component: "database",
                    severity: HealthSeverity::Warning,
                    message: "Database file exists but is empty".to_string(),
                });
            }
        }
        Err(e) => {
            issues.push(HealthIssue {
                component: "database",
                severity: HealthSeverity::Error,
                message: format!("Cannot read database file: {e}"),
            });
        }
    }
}

/// Check 2: Settings file parses without error.
pub(crate) fn check_settings(data_dir: &PathBuf, issues: &mut Vec<HealthIssue>) {
    let settings_path = data_dir.join("settings.json");
    if !settings_path.exists() {
        // Not an error — first run uses defaults.
        issues.push(HealthIssue {
            component: "settings",
            severity: HealthSeverity::Warning,
            message: "No settings.json found. Using defaults (first run).".to_string(),
        });
        return;
    }
    match std::fs::read_to_string(&settings_path) {
        Ok(content) => {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
                issues.push(HealthIssue {
                    component: "settings",
                    severity: HealthSeverity::Error,
                    message: format!("settings.json is invalid JSON: {e}"),
                });
            }
        }
        Err(e) => {
            issues.push(HealthIssue {
                component: "settings",
                severity: HealthSeverity::Error,
                message: format!("Cannot read settings.json: {e}"),
            });
        }
    }
}

/// Check 3: If an LLM provider is configured, verify the API key is non-empty.
/// No network calls — just validates the config looks plausible.
pub(crate) fn check_embedding_provider(data_dir: &PathBuf, issues: &mut Vec<HealthIssue>) {
    let settings_path = data_dir.join("settings.json");
    if !settings_path.exists() {
        return; // No settings means no provider configured — that's fine.
    }
    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return, // Already reported in check_settings.
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return, // Already reported in check_settings.
    };

    let provider = parsed
        .get("llm")
        .and_then(|llm| llm.get("provider"))
        .and_then(|p| p.as_str())
        .unwrap_or("none");

    if provider == "none" || provider.is_empty() {
        // No provider configured — that's a valid state (embedding uses Ollama fallback).
        return;
    }

    // Ollama doesn't need an API key.
    if provider == "ollama" {
        return;
    }

    let api_key = parsed
        .get("llm")
        .and_then(|llm| llm.get("api_key"))
        .and_then(|k| k.as_str())
        .unwrap_or("");

    if api_key.is_empty() {
        issues.push(HealthIssue {
            component: "embedding",
            severity: HealthSeverity::Warning,
            message: format!(
                "LLM provider '{}' is configured but API key is empty. LLM features will not work.",
                provider
            ),
        });
    }
}

/// Check 4: At least one content source is configured/enabled.
fn check_source_adapters(issues: &mut Vec<HealthIssue>) {
    let registry = crate::get_source_registry();
    let count = {
        let reg = registry.lock();
        reg.count()
    };
    if count == 0 {
        issues.push(HealthIssue {
            component: "sources",
            severity: HealthSeverity::Error,
            message: "No content sources registered. The app will have no content to display."
                .to_string(),
        });
    }
}

/// Check 5: Data directory is writable (create + delete a temp file).
pub(crate) fn check_disk_write(data_dir: &PathBuf, issues: &mut Vec<HealthIssue>) {
    // Ensure the data directory exists first.
    if !data_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(data_dir) {
            issues.push(HealthIssue {
                component: "disk",
                severity: HealthSeverity::Error,
                message: format!("Cannot create data directory {}: {e}", data_dir.display()),
            });
            return;
        }
    }

    let probe = data_dir.join(".4da_health_probe");
    match std::fs::write(&probe, b"health") {
        Ok(()) => {
            // Clean up — failure here is not critical.
            let _ = std::fs::remove_file(&probe);
        }
        Err(e) => {
            issues.push(HealthIssue {
                component: "disk",
                severity: HealthSeverity::Error,
                message: format!("Data directory is not writable: {e}"),
            });
        }
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns startup health issues for the frontend to optionally display.
#[tauri::command]
pub(crate) fn get_startup_health() -> Vec<HealthIssue> {
    // Re-run checks so frontend gets a fresh snapshot.
    run_startup_health_check()
}
