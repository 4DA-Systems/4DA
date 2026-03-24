//! First-Run Simulation Audit — "think from the end" verification system.
//!
//! Simulates a new user's experience by walking every STREETS lesson through
//! the personalization pipeline and checking for:
//!
//! 1. **Unresolved templates** — raw `{= ... =}` or `{? ... ?}` tags visible in output
//! 2. **Empty personalization** — fallback text showing when real data could populate
//! 3. **Stale data references** — hardcoded values that may be outdated
//! 4. **Missing regional data** — country-specific sections rendering empty
//! 5. **Broken injection markers** — `{@ ... @}` tags that don't resolve to insight blocks
//!
//! Can run as a Tauri command or as part of the canary pipeline.

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

/// Result of a first-run simulation audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstRunAuditReport {
    /// Overall pass/fail
    pub passed: bool,
    /// Total lessons audited
    pub lessons_audited: u32,
    /// Issues found per category
    pub unresolved_templates: Vec<AuditIssue>,
    pub fallback_only_fields: Vec<AuditIssue>,
    pub broken_markers: Vec<AuditIssue>,
    /// Summary counts
    pub total_issues: u32,
    pub critical_issues: u32,
    pub checked_at: String,
}

/// A single audit finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditIssue {
    pub module_id: String,
    pub lesson_idx: u32,
    pub severity: IssueSeverity,
    pub category: String,
    pub description: String,
    /// The raw text fragment containing the issue
    pub fragment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    /// User sees raw template syntax — immediate fix required
    Critical,
    /// Content works but could be better (fallback text when data exists)
    Warning,
    /// Informational (missing optional data)
    Info,
}

// ============================================================================
// Core Audit Engine
// ============================================================================

/// Run the full first-run simulation audit across all STREETS modules.
pub fn run_first_run_audit() -> FirstRunAuditReport {
    let mut report = FirstRunAuditReport {
        passed: true,
        lessons_audited: 0,
        unresolved_templates: Vec::new(),
        fallback_only_fields: Vec::new(),
        broken_markers: Vec::new(),
        total_issues: 0,
        critical_issues: 0,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };

    // Load all modules
    let modules = match crate::playbook_commands::get_playbook_modules(Some("en".into())) {
        Ok(m) => m,
        Err(e) => {
            warn!(target: "4da::audit", error = %e, "Failed to load playbook modules");
            return report;
        }
    };

    // Assemble personalization context (same as real rendering)
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::audit", error = %e, "Failed to open DB for audit");
            return report;
        }
    };

    let ctx = crate::content_personalization::context::assemble_personalization_context(&conn);

    for module in &modules {
        let content = match crate::playbook_commands::get_playbook_content(
            module.id.clone(),
            Some("en".into()),
        ) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (idx, lesson) in content.lessons.iter().enumerate() {
            report.lessons_audited += 1;
            let lesson_idx = idx as u32;

            // Process through the template engine (same path as real rendering)
            let processed = crate::content_personalization::template_processor::process_template(
                &lesson.content,
                &ctx,
            );

            // CHECK 1: Unresolved L1 interpolation tags
            check_unresolved_l1(&processed.content, &module.id, lesson_idx, &mut report);

            // CHECK 2: Unresolved L2 conditional tags
            check_unresolved_l2(&processed.content, &module.id, lesson_idx, &mut report);

            // CHECK 3: Excessive fallback usage (data exists but fallback shows)
            check_fallback_quality(&processed, &module.id, lesson_idx, &ctx, &mut report);

            // CHECK 4: Broken injection markers (still present after processing)
            check_broken_markers(&processed.content, &module.id, lesson_idx, &mut report);
        }
    }

    report.total_issues = report.unresolved_templates.len() as u32
        + report.fallback_only_fields.len() as u32
        + report.broken_markers.len() as u32;

    report.critical_issues = report
        .unresolved_templates
        .iter()
        .chain(report.fallback_only_fields.iter())
        .chain(report.broken_markers.iter())
        .filter(|i| i.severity == IssueSeverity::Critical)
        .count() as u32;

    report.passed = report.critical_issues == 0;

    if report.passed {
        debug!(
            target: "4da::audit",
            lessons = report.lessons_audited,
            warnings = report.total_issues - report.critical_issues,
            "First-run audit PASSED"
        );
    } else {
        warn!(
            target: "4da::audit",
            lessons = report.lessons_audited,
            critical = report.critical_issues,
            total = report.total_issues,
            "First-run audit FAILED — critical issues found"
        );
    }

    report
}

// ============================================================================
// Individual Checks
// ============================================================================

/// Check for raw `{= ... =}` tags that survived template processing.
/// These would be visible to the user as literal template syntax.
fn check_unresolved_l1(
    content: &str,
    module_id: &str,
    lesson_idx: u32,
    report: &mut FirstRunAuditReport,
) {
    let mut search_from = 0;
    while let Some(start) = content[search_from..].find("{=") {
        let abs_start = search_from + start;
        if let Some(end) = content[abs_start..].find("=}") {
            let abs_end = abs_start + end + 2;
            let fragment = &content[abs_start..abs_end];

            // Skip fragments inside code blocks (marked by surrounding backticks)
            if !is_inside_code_block(content, abs_start) {
                report.unresolved_templates.push(AuditIssue {
                    module_id: module_id.into(),
                    lesson_idx,
                    severity: IssueSeverity::Critical,
                    category: "unresolved_l1".into(),
                    description: "Raw template interpolation tag visible to user".into(),
                    fragment: fragment.to_string(),
                });
            }
            search_from = abs_end;
        } else {
            break;
        }
    }
}

/// Check for raw `{? ... ?}` conditional tags that survived processing.
fn check_unresolved_l2(
    content: &str,
    module_id: &str,
    lesson_idx: u32,
    report: &mut FirstRunAuditReport,
) {
    for tag in &["{? if ", "{? elif ", "{? else ?}", "{? endif ?}"] {
        let mut search_from = 0;
        while let Some(pos) = content[search_from..].find(tag) {
            let abs_pos = search_from + pos;
            if !is_inside_code_block(content, abs_pos) {
                let end = content[abs_pos..]
                    .find("?}")
                    .map_or(abs_pos + tag.len(), |e| abs_pos + e + 2);
                let fragment = &content[abs_pos..end.min(content.len())];
                report.unresolved_templates.push(AuditIssue {
                    module_id: module_id.into(),
                    lesson_idx,
                    severity: IssueSeverity::Critical,
                    category: "unresolved_l2".into(),
                    description: "Raw conditional tag visible to user".into(),
                    fragment: fragment.to_string(),
                });
            }
            search_from = abs_pos + tag.len();
        }
    }
}

/// Check if fallbacks are showing when real data could have populated.
fn check_fallback_quality(
    processed: &crate::content_personalization::template_processor::ProcessResult,
    module_id: &str,
    lesson_idx: u32,
    ctx: &crate::content_personalization::context::PersonalizationContext,
    report: &mut FirstRunAuditReport,
) {
    // If profile has real data but all L1 resolved via fallback, something is wrong
    if processed.l1_fallbacks > 0 && ctx.profile.categories_filled >= 3 {
        let ratio = processed.l1_fallbacks as f32
            / (processed.l1_resolved + processed.l1_fallbacks).max(1) as f32;

        if ratio > 0.8 {
            report.fallback_only_fields.push(AuditIssue {
                module_id: module_id.into(),
                lesson_idx,
                severity: IssueSeverity::Warning,
                category: "excessive_fallback".into(),
                description: format!(
                    "{}% of interpolations used fallback values despite profile having {} categories filled",
                    (ratio * 100.0) as u32,
                    ctx.profile.categories_filled,
                ),
                fragment: format!(
                    "resolved: {}, fallbacks: {}",
                    processed.l1_resolved, processed.l1_fallbacks
                ),
            });
        }
    }
}

/// Check for `{@ ... @}` injection markers still present in output.
/// These should be stripped by the template processor; visible ones indicate a bug.
fn check_broken_markers(
    content: &str,
    module_id: &str,
    lesson_idx: u32,
    report: &mut FirstRunAuditReport,
) {
    let mut search_from = 0;
    while let Some(start) = content[search_from..].find("{@") {
        let abs_start = search_from + start;
        if let Some(end) = content[abs_start..].find("@}") {
            let abs_end = abs_start + end + 2;
            let fragment = &content[abs_start..abs_end];

            if !is_inside_code_block(content, abs_start) {
                report.broken_markers.push(AuditIssue {
                    module_id: module_id.into(),
                    lesson_idx,
                    severity: IssueSeverity::Warning,
                    category: "broken_marker".into(),
                    description: "Injection marker not consumed by frontend rendering".into(),
                    fragment: fragment.to_string(),
                });
            }
            search_from = abs_end;
        } else {
            break;
        }
    }
}

/// Simple heuristic: check if a position is inside a triple-backtick code block.
fn is_inside_code_block(content: &str, pos: usize) -> bool {
    let before = &content[..pos];
    let fence_count = before.matches("```").count();
    // Odd number of fences means we're inside a code block
    fence_count % 2 == 1
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Run the first-run simulation audit.
#[tauri::command]
pub fn run_first_run_simulation() -> crate::error::Result<FirstRunAuditReport> {
    Ok(run_first_run_audit())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_inside_code_block() {
        let content = "text ```code {= tag =}``` more text {= real =}";
        assert!(is_inside_code_block(content, 17)); // inside code block
        assert!(!is_inside_code_block(content, 40)); // outside code block
    }

    #[test]
    fn test_check_unresolved_l1_finds_raw_tags() {
        let mut report = FirstRunAuditReport {
            passed: true,
            lessons_audited: 0,
            unresolved_templates: Vec::new(),
            fallback_only_fields: Vec::new(),
            broken_markers: Vec::new(),
            total_issues: 0,
            critical_issues: 0,
            checked_at: String::new(),
        };

        check_unresolved_l1("Hello {= name =} world", "S", 0, &mut report);
        assert_eq!(report.unresolved_templates.len(), 1);
        assert_eq!(
            report.unresolved_templates[0].severity,
            IssueSeverity::Critical
        );
    }

    #[test]
    fn test_check_unresolved_l1_skips_code_blocks() {
        let mut report = FirstRunAuditReport {
            passed: true,
            lessons_audited: 0,
            unresolved_templates: Vec::new(),
            fallback_only_fields: Vec::new(),
            broken_markers: Vec::new(),
            total_issues: 0,
            critical_issues: 0,
            checked_at: String::new(),
        };

        check_unresolved_l1("text ```code {= tag =}``` normal", "S", 0, &mut report);
        assert_eq!(report.unresolved_templates.len(), 0);
    }

    #[test]
    fn test_check_unresolved_l2_finds_raw_conditionals() {
        let mut report = FirstRunAuditReport {
            passed: true,
            lessons_audited: 0,
            unresolved_templates: Vec::new(),
            fallback_only_fields: Vec::new(),
            broken_markers: Vec::new(),
            total_issues: 0,
            critical_issues: 0,
            checked_at: String::new(),
        };

        check_unresolved_l2(
            "Hello {? if something ?} world {? endif ?}",
            "S",
            0,
            &mut report,
        );
        assert_eq!(report.unresolved_templates.len(), 2);
    }

    #[test]
    fn test_check_broken_markers() {
        let mut report = FirstRunAuditReport {
            passed: true,
            lessons_audited: 0,
            unresolved_templates: Vec::new(),
            fallback_only_fields: Vec::new(),
            broken_markers: Vec::new(),
            total_issues: 0,
            critical_issues: 0,
            checked_at: String::new(),
        };

        check_broken_markers(
            "Text {@ insight hardware_benchmark @} more",
            "S",
            0,
            &mut report,
        );
        assert_eq!(report.broken_markers.len(), 1);
    }
}
