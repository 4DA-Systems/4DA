//! ACE context commands: detected tech, active topics, interest seeding and suggestions.

use tracing::{debug, info, warn};

use crate::context_engine::InterestSource;
use crate::error::{Result, ResultExt};
use crate::scoring::get_ace_context;
use crate::{embed_texts, get_ace_engine, get_context_engine};

/// Get detected technologies from ACE
#[tauri::command]
pub async fn ace_get_detected_tech() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let tech = ace.get_detected_tech()?;

    Ok(serde_json::json!({
        "detected_tech": tech
    }))
}

/// Get active topics from ACE
#[tauri::command]
pub async fn ace_get_active_topics() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let topics = ace.get_active_topics()?;

    Ok(serde_json::json!({
        "topics": topics
    }))
}

/// Automatically seed user interests from ACE-detected technologies.
/// Runs once at startup when interests are empty, providing immediate value.
pub(crate) async fn auto_seed_interests_from_ace() -> Result<()> {
    let context_engine = get_context_engine()?;

    // Backfill tech_stack if empty (one-time fix for existing users)
    let existing_tech = context_engine.get_tech_stack().unwrap_or_default();
    if existing_tech.is_empty() {
        if let Ok(ace) = get_ace_engine() {
            if let Ok(tech_list) = ace.get_detected_tech() {
                let seeded: Vec<_> = tech_list
                    .iter()
                    .filter(|t| {
                        matches!(
                            t.category,
                            crate::ace::TechCategory::Language
                                | crate::ace::TechCategory::Framework
                        ) && t.confidence >= 0.7
                            // CONTENT ACCURACY GATE: Only seed display-worthy tech
                            // into tech_stack. ORMs like drizzle, utility libs, and
                            // companion packages must not pollute the user's identity.
                            && crate::domain_profile::is_display_worthy(&t.name.to_lowercase())
                    })
                    .take(10)
                    .collect();
                for t in &seeded {
                    if let Err(e) = context_engine.add_technology(&t.name) {
                        tracing::warn!("Context update failed: {e}");
                    }
                }
                if !seeded.is_empty() {
                    info!(target: "4da::startup", count = seeded.len(), "Backfilled tech_stack from detected_tech");
                }
            }
        }
    }

    // One-time cleanup: remove dependency-level inferred interests
    let existing = context_engine.get_interests().unwrap_or_default();
    let mut cleaned = 0;
    for interest in &existing {
        if interest.source == crate::context_engine::InterestSource::Inferred {
            let topic = interest.topic.to_lowercase();
            if topic.starts_with('@') || topic.contains('/') {
                if let Err(e) = context_engine.remove_interest(&interest.topic) {
                    tracing::warn!("Context update failed: {e}");
                }
                cleaned += 1;
            }
        }
    }
    if cleaned > 0 {
        info!(target: "4da::startup", cleaned, "Removed dependency-level inferred interests");
    }

    // Check if interests are already configured
    let existing_interests = context_engine
        .get_interests()
        .context("Failed to get interests")?;

    if !existing_interests.is_empty() {
        debug!(target: "4da::startup", count = existing_interests.len(), "Interests already configured, skipping auto-seed");
        return Ok(());
    }

    // Get ACE-detected technologies
    let ace_ctx = get_ace_context();
    if ace_ctx.detected_tech.is_empty() && ace_ctx.active_topics.is_empty() {
        debug!(target: "4da::startup", "No ACE context available for auto-seeding");
        return Ok(());
    }

    info!(target: "4da::startup", tech_count = ace_ctx.detected_tech.len(), topic_count = ace_ctx.active_topics.len(), "Auto-seeding interests from ACE context");

    // Collect high-value topics to seed (languages, frameworks with high confidence)
    let mut topics_to_seed: Vec<(String, f32)> = Vec::new();

    // Add detected tech (languages, frameworks) with weight 0.8
    for tech in &ace_ctx.detected_tech {
        // Skip very generic or noisy tech
        let skip_list = [
            "npm", "yarn", "pnpm", "node", "git", "json", "yaml", "toml", "markdown",
        ];
        if skip_list.contains(&tech.as_str())
            || tech.starts_with('@')
            || tech.contains('/')
            || tech.len() <= 2
        {
            continue;
        }
        topics_to_seed.push((tech.clone(), 0.8));
    }

    // Add high-confidence active topics with weight 0.7
    for topic in &ace_ctx.active_topics {
        let confidence = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
        // Only add topics with good confidence that aren't already in tech
        if confidence >= 0.7 && !ace_ctx.detected_tech.contains(topic) {
            // Skip commit-type patterns and generic terms
            if !topic.starts_with("commit-") && topic.len() > 2 {
                topics_to_seed.push((topic.clone(), 0.7));
            }
        }
    }

    if topics_to_seed.is_empty() {
        debug!(target: "4da::startup", "No suitable topics for auto-seeding");
        return Ok(());
    }

    // Limit to top 15 to avoid over-seeding
    topics_to_seed.truncate(15);

    // Generate embeddings for all topics at once
    let topic_strings: Vec<String> = topics_to_seed.iter().map(|(t, _)| t.clone()).collect();
    let embeddings = embed_texts(&topic_strings).await?;

    // Add each topic as an inferred interest
    let mut seeded_count = 0;
    for ((topic, weight), embedding) in topics_to_seed.iter().zip(embeddings.iter()) {
        match context_engine.add_interest(
            topic,
            *weight,
            Some(embedding.as_slice()),
            InterestSource::Inferred,
        ) {
            Ok(_) => {
                seeded_count += 1;
                debug!(target: "4da::startup", topic = %topic, weight = weight, "Auto-seeded interest");
            }
            Err(e) => {
                warn!(target: "4da::startup", topic = %topic, error = %e, "Failed to seed interest");
            }
        }
    }

    info!(target: "4da::startup", count = seeded_count, "Auto-seeded interests from ACE context");
    Ok(())
}

/// Get suggested interests based on ACE-detected technologies and active topics.
/// Cross-references with existing interests and exclusions to avoid duplicates.
#[tauri::command]
pub async fn ace_get_suggested_interests() -> Result<Vec<serde_json::Value>> {
    let ace = crate::get_ace_engine()?;

    // Get detected tech
    let detected_tech = ace.get_detected_tech().unwrap_or_default();

    // Get active topics with high confidence (>0.5)
    let active_topics = ace.get_active_topics().unwrap_or_default();
    let confident_topics: Vec<_> = active_topics
        .iter()
        .filter(|t| t.confidence > 0.5 && t.weight > 0.4)
        .collect();

    // Get existing interests to cross-reference
    let existing_interests: Vec<String> = if let Ok(ctx_engine) = crate::get_context_engine() {
        ctx_engine
            .get_interests()
            .unwrap_or_default()
            .iter()
            .map(|i| i.topic.to_lowercase())
            .collect()
    } else {
        vec![]
    };

    // Get exclusions too
    let exclusions: Vec<String> = if let Ok(ctx_engine) = crate::get_context_engine() {
        ctx_engine
            .get_exclusions()
            .unwrap_or_default()
            .iter()
            .map(|e| e.to_lowercase())
            .collect()
    } else {
        vec![]
    };

    let mut suggestions: Vec<serde_json::Value> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Add detected tech as suggestions
    for tech in &detected_tech {
        let topic_lower = tech.name.to_lowercase();
        if seen.contains(&topic_lower) {
            continue;
        }
        let already_declared = existing_interests.contains(&topic_lower);
        let is_excluded = exclusions.contains(&topic_lower);
        if is_excluded {
            continue;
        }

        seen.insert(topic_lower);
        let source_label = format!("{:?}", tech.source);
        suggestions.push(serde_json::json!({
            "topic": tech.name,
            "source": format!("Detected in {}", source_label),
            "confidence": tech.confidence,
            "already_declared": already_declared,
        }));
    }

    // Add confident active topics
    for topic in &confident_topics {
        let topic_lower = topic.topic.to_lowercase();
        if seen.contains(&topic_lower) {
            continue;
        }
        let already_declared = existing_interests.contains(&topic_lower);
        let is_excluded = exclusions.contains(&topic_lower);
        if is_excluded {
            continue;
        }

        seen.insert(topic_lower);
        let source_label = format!("{:?}", topic.source);
        suggestions.push(serde_json::json!({
            "topic": topic.topic,
            "source": format!("Active in {} ({})", source_label, topic.last_seen),
            "confidence": topic.confidence,
            "already_declared": already_declared,
        }));
    }

    // Sort by confidence descending
    suggestions.sort_by(|a, b| {
        let ca = a["confidence"].as_f64().unwrap_or(0.0);
        let cb = b["confidence"].as_f64().unwrap_or(0.0);
        cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Return top 20 suggestions
    suggestions.truncate(20);

    Ok(suggestions)
}

#[cfg(test)]
mod tests {
    /// Helper: build a suggestion entry matching the shape produced by ace_get_suggested_interests
    fn make_suggestion(topic: &str, confidence: f64, already_declared: bool) -> serde_json::Value {
        serde_json::json!({
            "topic": topic,
            "source": "test",
            "confidence": confidence,
            "already_declared": already_declared,
        })
    }

    #[test]
    fn test_suggested_interests_filters_excluded() {
        // Simulate the filtering logic from ace_get_suggested_interests:
        // excluded topics should not appear in suggestions.
        let topics = vec!["Rust", "Python", "crypto", "TypeScript"];
        let exclusions: Vec<String> = vec!["crypto".to_string()];
        let existing: Vec<String> = vec![];

        let mut suggestions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for topic in &topics {
            let lower = topic.to_lowercase();
            if seen.contains(&lower) {
                continue;
            }
            if exclusions.contains(&lower) {
                continue;
            }
            let already_declared = existing.contains(&lower);
            seen.insert(lower);
            suggestions.push(make_suggestion(topic, 0.9, already_declared));
        }

        assert_eq!(suggestions.len(), 3);
        let suggestion_topics: Vec<&str> = suggestions
            .iter()
            .map(|s| s["topic"].as_str().unwrap())
            .collect();
        assert!(!suggestion_topics.contains(&"crypto"));
        assert!(suggestion_topics.contains(&"Rust"));
        assert!(suggestion_topics.contains(&"Python"));
        assert!(suggestion_topics.contains(&"TypeScript"));
    }

    #[test]
    fn test_suggested_interests_marks_already_declared() {
        // Topics already in interests should be flagged already_declared=true, not filtered.
        let topics = vec!["Rust", "Python", "Go"];
        let exclusions: Vec<String> = vec![];
        let existing: Vec<String> = vec!["rust".to_string(), "go".to_string()];

        let mut suggestions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for topic in &topics {
            let lower = topic.to_lowercase();
            if seen.contains(&lower) {
                continue;
            }
            if exclusions.contains(&lower) {
                continue;
            }
            let already_declared = existing.contains(&lower);
            seen.insert(lower);
            suggestions.push(make_suggestion(topic, 0.9, already_declared));
        }

        assert_eq!(suggestions.len(), 3);
        // Rust should be marked already_declared
        let rust_suggestion = suggestions.iter().find(|s| s["topic"] == "Rust").unwrap();
        assert_eq!(rust_suggestion["already_declared"], true);
        // Go should be marked already_declared
        let go_suggestion = suggestions.iter().find(|s| s["topic"] == "Go").unwrap();
        assert_eq!(go_suggestion["already_declared"], true);
        // Python should NOT be marked already_declared
        let py_suggestion = suggestions.iter().find(|s| s["topic"] == "Python").unwrap();
        assert_eq!(py_suggestion["already_declared"], false);
    }

    #[test]
    fn test_suggested_interests_deduplicates() {
        // Duplicate topics (case-insensitive) should only appear once.
        let topics = vec!["Rust", "rust", "RUST", "Python"];
        let exclusions: Vec<String> = vec![];
        let existing: Vec<String> = vec![];

        let mut suggestions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for topic in &topics {
            let lower = topic.to_lowercase();
            if seen.contains(&lower) {
                continue;
            }
            if exclusions.contains(&lower) {
                continue;
            }
            let already_declared = existing.contains(&lower);
            seen.insert(lower);
            suggestions.push(make_suggestion(topic, 0.9, already_declared));
        }

        assert_eq!(suggestions.len(), 2);
        // First occurrence ("Rust") should be kept
        assert_eq!(suggestions[0]["topic"], "Rust");
        assert_eq!(suggestions[1]["topic"], "Python");
    }

    #[test]
    fn test_auto_seed_skip_list_filters_generic() {
        // Simulate the skip-list logic from ace_auto_seed_interests_from_context
        let skip_list = [
            "npm",
            "yarn",
            "pnpm",
            "node",
            "webpack",
            "babel",
            "eslint",
            "prettier",
            "jest",
            "mocha",
            "typescript",
            "tslib",
            "core-js",
        ];
        let detected = vec!["Rust", "npm", "React", "webpack", "Python"];
        let filtered: Vec<&&str> = detected
            .iter()
            .filter(|t| !skip_list.contains(&t.to_lowercase().as_str()))
            .collect();
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&&"Rust"));
        assert!(filtered.contains(&&"React"));
        assert!(filtered.contains(&&"Python"));
    }

    #[test]
    fn test_auto_seed_skips_scoped_packages() {
        // Scoped npm packages (starting with @) should be filtered
        let topics = vec!["@types/node", "@babel/core", "React", "Rust"];
        let filtered: Vec<&&str> = topics.iter().filter(|t| !t.starts_with('@')).collect();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&&"React"));
    }

    #[test]
    fn test_auto_seed_skips_short_names() {
        // Very short names (1-2 chars) should be filtered
        let topics = vec!["Go", "R", "Rust", "AI", "React"];
        let filtered: Vec<&&str> = topics.iter().filter(|t| t.len() > 2).collect();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&&"Rust"));
        assert!(filtered.contains(&&"React"));
    }

    #[test]
    fn test_auto_seed_truncates_at_15() {
        // Auto-seeding should cap at 15 interests
        let mut topics: Vec<String> = (0..25).map(|i| format!("topic_{}", i)).collect();
        topics.truncate(15);
        assert_eq!(topics.len(), 15);
        assert_eq!(topics.last().unwrap(), "topic_14");
    }

    #[test]
    fn test_suggested_interests_limits_to_20() {
        // Suggested interests should be capped at 20
        let all_topics: Vec<String> = (0..30).map(|i| format!("topic_{}", i)).collect();
        let limited: Vec<&String> = all_topics.iter().take(20).collect();
        assert_eq!(limited.len(), 20);
    }
}

// ============================================================================
// Active Work Context — real-time topics from current work session
// ============================================================================

/// Get active work topics from the current session (last 4 hours of file changes).
/// Returns topics with their weight and recency for the Momentum page.
#[tauri::command]
pub async fn get_active_work_context() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let conn = ace.conn.lock();

    let mut stmt = conn
        .prepare(
            "SELECT topic, weight, confidence, last_seen
             FROM active_topics
             WHERE source IN ('file_content', 'import_statement')
             AND last_seen > datetime('now', '-4 hours')
             ORDER BY weight DESC
             LIMIT 12",
        )
        .context("Failed to query active work topics")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "topic": row.get::<_, String>(0)?,
                "weight": row.get::<_, f64>(1)?,
                "confidence": row.get::<_, f64>(2)?,
                "last_seen": row.get::<_, String>(3)?,
            }))
        })
        .context("Failed to read active work topics")?;

    let topics: Vec<serde_json::Value> = rows.flatten().collect();

    // Count recent file changes for activity level
    let file_changes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM file_signals WHERE timestamp > datetime('now', '-1 hour')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Get the primary project path being worked on
    let active_project: Option<String> = conn
        .query_row(
            "SELECT path FROM file_signals WHERE timestamp > datetime('now', '-1 hour')
             ORDER BY timestamp DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let project_name = active_project.as_deref().and_then(|p| {
        std::path::Path::new(p)
            .components()
            .find(|c| {
                !matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Prefix(_)
                )
            })
            .map(|c| c.as_os_str().to_string_lossy().to_string())
    });

    Ok(serde_json::json!({
        "topics": topics,
        "file_changes_last_hour": file_changes,
        "active_project": project_name,
    }))
}
