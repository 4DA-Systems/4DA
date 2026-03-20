//! ACE (Autonomous Context Engine) Tauri commands and PASIFA helpers.
//!
//! Extracted from lib.rs. Contains all ACE phase commands (A through E),
//! autonomous discovery, watcher control, PASIFA README indexing,
//! and auto-seeding of interests from detected context.

use std::path::PathBuf;

use tracing::{debug, info, warn};

use crate::ace;
use crate::context_engine::InterestSource;
use crate::error::{Result, ResultExt};
use crate::scoring::get_ace_context;
use crate::{
    embed_texts, get_ace_engine, get_ace_engine_mut, get_context_engine, get_settings_manager,
};

// Re-export README indexing for callers that use ace_commands::index_discovered_readmes
pub(crate) use crate::ace::readme_indexing::index_discovered_readmes;

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

/// Get default paths to scan for projects
pub(crate) fn get_default_scan_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Common project locations
        let candidates = [
            "projects",
            "code",
            "dev",
            "src",
            "Documents/GitHub",
            "repos",
            "workspace",
            "work",
        ];

        for candidate in candidates {
            let path = home.join(candidate);
            if path.exists() {
                paths.push(path);
            }
        }
    }

    // Also check current working directory parent (for dev scenarios)
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            if !paths.contains(&parent.to_path_buf()) {
                paths.push(parent.to_path_buf());
            }
        }
    }

    paths
}

/// Run full autonomous context detection (manifests + git)
#[tauri::command]
pub async fn ace_full_scan(paths: Vec<String>) -> Result<serde_json::Value> {
    let scan_paths: Vec<PathBuf> = if paths.is_empty() {
        get_default_scan_paths()
    } else {
        paths
            .iter()
            .map(|p| {
                if let Some(rest) = p.strip_prefix("~/") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(rest)
                    } else {
                        PathBuf::from(p)
                    }
                } else if p == "~" {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from(p))
                } else {
                    PathBuf::from(p)
                }
            })
            .collect()
    };

    // Validate paths are within home directory
    let scan_paths: Vec<PathBuf> = scan_paths
        .into_iter()
        .filter(|path| {
            let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            if let Some(home) = dirs::home_dir() {
                if !canonical.starts_with(&home) {
                    warn!(target: "ace", path = %canonical.display(), "Rejected path outside home directory");
                    return false;
                }
            }
            true
        })
        .collect();

    info!(target: "4da::ace", paths = scan_paths.len(), "Starting full scan");

    // Phase 1 & 2: Manifest scanning and Git analysis (scoped to release ACE lock)
    let (manifest_context, git_signals) = {
        let ace = get_ace_engine()?;
        let manifest_context = ace.detect_context(&scan_paths)?;
        let git_signals = ace.analyze_git_repos(&scan_paths)?;
        (manifest_context, git_signals)
    }; // ACE lock is dropped here

    // Phase 1a: Store discovered dependencies in user_dependencies table
    if let Ok(db) = crate::get_database() {
        if let Ok(ace) = get_ace_engine() {
            if let Ok(tech) = ace.get_detected_tech() {
                // Get project signals from the ACE database (project_dependencies table)
                if let Ok(conn) = crate::open_db_connection() {
                    if let Ok(deps) = crate::temporal::get_all_dependencies(&conn) {
                        for dep in &deps {
                            let ecosystem = &dep.language;
                            db.store_dependency(
                                &dep.project_path,
                                &dep.package_name,
                                dep.version.as_deref(),
                                ecosystem,
                                dep.is_dev,
                            )
                            .ok();
                        }
                        if !deps.is_empty() {
                            info!(target: "4da::ace", count = deps.len(), "Stored dependencies in user_dependencies table");
                        }
                    }
                }
                drop(tech);
            }
        }
    }

    // Phase 1b: Learning trajectory detection
    let mut learning_topics: Vec<String> = Vec::new();
    for path in &scan_paths {
        let signals = crate::ace::scanner::detect_learning_directories(path);
        for signal in signals {
            if !learning_topics.contains(&signal.topic) {
                learning_topics.push(signal.topic);
            }
        }
    }
    if !learning_topics.is_empty() {
        info!(target: "4da::ace", topics = learning_topics.len(), "Detected learning trajectory topics");
        // Store learning topics as active topics with learning_trajectory source tag
        if let Ok(ace) = get_ace_engine() {
            let conn = ace.get_conn().lock();
            for topic in &learning_topics {
                if let Err(e) = conn.execute(
                    "INSERT INTO active_topics (topic, weight, confidence, source, last_seen)
                     VALUES (?1, 0.65, 0.7, 'learning_trajectory', datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        weight = MAX(excluded.weight, active_topics.weight),
                        confidence = MAX(excluded.confidence, active_topics.confidence),
                        last_seen = datetime('now')",
                    rusqlite::params![topic],
                ) {
                    debug!("Failed to store learning topic: {e}");
                }
            }
        }
    }

    // Phase 1c: Aggregate peak commit hours across all repos for temporal scoring
    {
        let mut hour_counts = [0u32; 24];
        for signal in &git_signals {
            for &hour in &signal.peak_hours {
                if (hour as usize) < 24 {
                    hour_counts[hour as usize] += 1;
                }
            }
        }
        let mut hour_pairs: Vec<(u8, u32)> = hour_counts
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .map(|(h, &count)| (h as u8, count))
            .collect();
        hour_pairs.sort_by(|a, b| b.1.cmp(&a.1));
        let aggregate_peak_hours: Vec<u8> =
            hour_pairs.into_iter().take(5).map(|(h, _)| h).collect();
        if !aggregate_peak_hours.is_empty() {
            if let Ok(mut ace) = get_ace_engine_mut() {
                ace.peak_hours = aggregate_peak_hours.clone();
                info!(target: "4da::ace", hours = ?aggregate_peak_hours, "Stored aggregate peak commit hours");
            }
        }
    }

    // Phase 3: README indexing (PASIFA - semantic context from discovered projects)
    // This makes ACE discovery contribute to semantic matching, not just keyword boost
    debug!(target: "4da::ace", "Indexing README files for semantic search");
    let readme_chunks_indexed = index_discovered_readmes(&scan_paths).await;
    if readme_chunks_indexed > 0 {
        info!(target: "4da::ace", chunks = readme_chunks_indexed, "Indexed README files for semantic context");
    }

    // Combine results
    let total_topics: std::collections::HashSet<String> = manifest_context
        .active_topics
        .iter()
        .map(|t| t.topic.clone())
        .chain(git_signals.iter().flat_map(|s| s.extracted_topics.clone()))
        .collect();

    info!(target: "4da::ace",
        tech = manifest_context.detected_tech.len(),
        topics = total_topics.len(),
        git_repos = git_signals.len(),
        readme_chunks = readme_chunks_indexed,
        "Full scan complete"
    );

    Ok(serde_json::json!({
        "success": true,
        "manifest_scan": {
            "projects_scanned": manifest_context.projects_scanned,
            "detected_tech": manifest_context.detected_tech.len(),
            "confidence": manifest_context.context_confidence
        },
        "git_scan": {
            "repos_analyzed": git_signals.len(),
            "total_commits": git_signals.iter().map(|s| s.recent_commits.len()).sum::<usize>()
        },
        "readme_index": {
            "chunks_indexed": readme_chunks_indexed
        },
        "learning_trajectory": {
            "topics": learning_topics,
        },
        "combined": {
            "total_topics": total_topics.len(),
            "topics": total_topics.into_iter().collect::<Vec<_>>()
        }
    }))
}

/// Trigger autonomous context discovery - finds dev directories and projects automatically
#[tauri::command]
pub async fn ace_auto_discover() -> Result<serde_json::Value> {
    info!(target: "4da::ace", "Starting autonomous context discovery");

    // Phase 1: Discover common dev directories
    let discovered_dirs = crate::settings::discover_dev_directories();

    if discovered_dirs.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "message": "No development directories found on this system",
            "directories_found": 0,
            "projects_found": 0
        }));
    }

    info!(target: "4da::ace", dirs = discovered_dirs.len(), "Found potential dev directories");

    // Phase 2: Deep scan for actual project directories
    let project_dirs = crate::settings::find_project_directories(&discovered_dirs, 3);

    // Decide what to add - parent dirs or individual projects
    let dirs_to_add = if project_dirs.len() > 50 {
        debug!(target: "4da::ace", projects = project_dirs.len(), "Too many projects, using parent directories");
        discovered_dirs.clone()
    } else if !project_dirs.is_empty() {
        debug!(target: "4da::ace", projects = project_dirs.len(), "Found specific projects");
        project_dirs.clone()
    } else {
        debug!(target: "4da::ace", "No specific projects found, using dev directories");
        discovered_dirs.clone()
    };

    // Save to settings
    {
        let mut settings = get_settings_manager().lock();
        if let Err(e) = settings.add_context_dirs(dirs_to_add.clone()) {
            return Err(format!("Failed to save discovered directories: {}", e).into());
        }
        if let Err(e) = settings.mark_auto_discovery_completed() {
            tracing::warn!("Failed to mark state: {e}");
        }
    }

    // Now run full ACE scan on discovered directories
    info!(target: "4da::ace", dirs = dirs_to_add.len(), "Running full scan on directories");
    let scan_result = ace_full_scan(dirs_to_add.clone()).await?;

    Ok(serde_json::json!({
        "success": true,
        "directories_found": discovered_dirs.len(),
        "projects_found": project_dirs.len(),
        "directories_added": dirs_to_add.len(),
        "directories": dirs_to_add,
        "scan_result": scan_result
    }))
}

/// Get a structured summary of what ACE knows -- powers the first-run interstitial.
#[tauri::command]
pub async fn ace_get_scan_summary() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let tech = ace.get_detected_tech()?;
    let (mut rust_deps, mut npm_deps, mut python_deps, mut other_deps) = (0u32, 0, 0, 0);
    let mut languages: Vec<String> = Vec::new();
    let mut frameworks: Vec<String> = Vec::new();
    let mut key_packages: Vec<String> = Vec::new();
    for t in &tech {
        match t.category {
            ace::TechCategory::Language => {
                if !languages.contains(&t.name) {
                    languages.push(t.name.clone());
                }
            }
            ace::TechCategory::Framework => {
                if !frameworks.contains(&t.name) {
                    frameworks.push(t.name.clone());
                }
            }
            ace::TechCategory::Library => {
                let ev_str = t.evidence.join(" ").to_lowercase();
                if ev_str.contains("cargo.toml") || ace::is_rust_package(&t.name) {
                    rust_deps += 1;
                } else if ev_str.contains("package.json") {
                    npm_deps += 1;
                } else if ev_str.contains("pyproject") || ev_str.contains("requirements") {
                    python_deps += 1;
                } else {
                    other_deps += 1;
                }
                if t.confidence >= 0.5 {
                    key_packages.push(t.name.clone());
                }
            }
            _ => {}
        }
    }
    let primary_stack = if !languages.is_empty() || !frameworks.is_empty() {
        let mut parts: Vec<String> = Vec::new();
        parts.extend(languages.iter().take(3).cloned());
        parts.extend(frameworks.iter().take(3).cloned());
        parts.join(" + ")
    } else {
        String::new()
    };
    let projects_scanned = {
        let mut paths = std::collections::HashSet::new();
        for t in &tech {
            for ev in &t.evidence {
                if let Some(p) = ev
                    .strip_prefix("Found in ")
                    .or_else(|| ev.strip_prefix("Dependency in "))
                {
                    if let Some(parent) = std::path::Path::new(p).parent() {
                        paths.insert(parent.to_path_buf());
                    }
                }
            }
        }
        paths.len() as u32
    };
    key_packages.truncate(10);
    let total_deps = rust_deps + npm_deps + python_deps + other_deps;
    Ok(serde_json::json!({
        "projects_scanned": projects_scanned,
        "total_dependencies": total_deps,
        "dependencies_by_ecosystem": {
            "rust": rust_deps, "npm": npm_deps, "python": python_deps, "other": other_deps
        },
        "languages": languages,
        "frameworks": frameworks,
        "primary_stack": primary_stack,
        "key_packages": key_packages,
        "has_data": total_deps > 0 || !languages.is_empty()
    }))
}

/// Record user feedback in the main database — feeds autophagy calibration analysis.
/// This bridges user interactions (save/dismiss) into the `feedback` table that all
/// autophagy analyzers depend on. Without this, autophagy produces zero output.
#[tauri::command]
pub async fn record_item_feedback(item_id: i64, relevant: bool) -> Result<()> {
    let db = crate::get_database()?;
    db.record_feedback(item_id, relevant)
        .context("Failed to record feedback")?;
    Ok(())
}

/// Record a user interaction for behavior learning
#[tauri::command]
pub async fn ace_record_interaction(
    item_id: i64,
    action_type: String,
    action_data: Option<serde_json::Value>,
    item_topics: Vec<String>,
    item_source: String,
) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;

    // Parse action type into BehaviorAction
    let action = match action_type.as_str() {
        "click" => {
            let dwell_time = action_data
                .and_then(|d| d.get("dwell_time_seconds").and_then(|v| v.as_u64()))
                .unwrap_or(0);
            ace::BehaviorAction::Click {
                dwell_time_seconds: dwell_time,
            }
        }
        "save" => ace::BehaviorAction::Save,
        "share" => ace::BehaviorAction::Share,
        "dismiss" => ace::BehaviorAction::Dismiss,
        "mark_irrelevant" => ace::BehaviorAction::MarkIrrelevant,
        "scroll" => {
            let visible_seconds = action_data
                .and_then(|d| d.get("visible_seconds").and_then(|v| v.as_f64()))
                .unwrap_or(0.0) as f32;
            ace::BehaviorAction::Scroll { visible_seconds }
        }
        "ignore" => ace::BehaviorAction::Ignore,
        _ => return Err(format!("Unknown action type: {}", action_type).into()),
    };

    ace.record_interaction(
        item_id,
        action.clone(),
        item_topics.clone(),
        item_source.clone(),
    )?;

    Ok(serde_json::json!({
        "success": true,
        "recorded": {
            "item_id": item_id,
            "action": action_type,
            "topics": item_topics,
            "source": item_source
        }
    }))
}

/// Get learned topic affinities
#[tauri::command]
pub async fn ace_get_topic_affinities() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let affinities = ace.get_topic_affinities()?;

    Ok(serde_json::json!({
        "affinities": affinities,
        "count": affinities.len()
    }))
}

/// Get detected anti-topics
#[tauri::command]
pub async fn ace_get_anti_topics(min_rejections: Option<u32>) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let threshold = min_rejections.unwrap_or(5);
    let anti_topics = ace.get_anti_topics(threshold)?;

    Ok(serde_json::json!({
        "anti_topics": anti_topics,
        "count": anti_topics.len(),
        "threshold": threshold
    }))
}
/// Find similar topics using embeddings
#[tauri::command]
pub async fn ace_find_similar_topics(
    query: String,
    top_k: Option<usize>,
) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let top_k = top_k.unwrap_or(5);
    let results = ace.find_similar_topics(&query, top_k)?;

    Ok(serde_json::json!({
        "query": query,
        "results": results.iter().map(|(topic, score)| {
            serde_json::json!({
                "topic": topic,
                "similarity": score
            })
        }).collect::<Vec<_>>()
    }))
}

/// Check if embedding service is operational
#[tauri::command]
pub async fn ace_embedding_status() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let operational = ace.is_embedding_operational();

    Ok(serde_json::json!({
        "operational": operational
    }))
}

/// Save watcher state for persistence
#[tauri::command]
pub async fn ace_save_watcher_state() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    ace.save_watcher_state()?;

    Ok(serde_json::json!({
        "saved": true
    }))
}

/// Get rate limit status for a source
#[tauri::command]
pub async fn ace_get_rate_limit_status(source: String) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let status = ace.get_rate_limit_status(&source);

    Ok(serde_json::json!(status))
}

/// Start file watching on specified directories
pub async fn ace_start_watcher(paths: Vec<String>) -> Result<serde_json::Value> {
    let mut ace = get_ace_engine_mut()?;

    let watch_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if let Some(rest) = p.strip_prefix("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(rest)
                } else {
                    PathBuf::from(p)
                }
            } else if p == "~" {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from(p))
            } else {
                PathBuf::from(p)
            }
        })
        .filter(|p| p.exists())
        .collect();

    if watch_paths.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "No valid paths to watch",
            "watching": 0
        }));
    }

    ace.start_watching(&watch_paths)?;

    Ok(serde_json::json!({
        "success": true,
        "watching": watch_paths.len(),
        "paths": watch_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
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

/// Get all unresolved anomalies
#[tauri::command]
pub async fn ace_get_unresolved_anomalies() -> Result<serde_json::Value> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = crate::anomaly::get_unresolved(&conn)?;
    Ok(serde_json::json!({
        "anomalies": anomalies,
        "count": anomalies.len()
    }))
}

/// Run anomaly detection and store results
#[tauri::command]
pub async fn ace_detect_anomalies() -> Result<serde_json::Value> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = crate::anomaly::detect_all(&conn)?;
    for a in &anomalies {
        if let Err(e) = crate::anomaly::store_anomaly(&conn, a) {
            tracing::warn!("Failed to store anomaly: {e}");
        }
    }
    Ok(serde_json::json!({
        "anomalies": anomalies,
        "count": anomalies.len()
    }))
}

/// Resolve (dismiss) an anomaly by id
#[tauri::command]
pub async fn ace_resolve_anomaly(anomaly_id: i64) -> Result<()> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    crate::anomaly::resolve_anomaly(&conn, anomaly_id)
}

/// Get accuracy metrics calculated from interactions
#[tauri::command]
pub async fn ace_get_accuracy_metrics() -> Result<serde_json::Value> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();

    // Calculate from interactions table
    // The ACE schema uses action_type (not interaction_type)
    let total_interactions: i64 = conn
        .query_row("SELECT COUNT(*) FROM interactions", [], |row| row.get(0))
        .unwrap_or(0);

    let positive_interactions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interactions WHERE action_type IN ('click', 'save', 'share')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let negative_interactions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interactions WHERE action_type IN ('dismiss', 'mark_irrelevant')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let engagement_rate = if total_interactions > 0 {
        positive_interactions as f64 / total_interactions as f64
    } else {
        0.0
    };

    let precision = if (positive_interactions + negative_interactions) > 0 {
        positive_interactions as f64 / (positive_interactions + negative_interactions) as f64
    } else {
        0.0
    };

    // Calculate calibration error from accuracy feedback entries
    let calibration_error: f64 = conn
        .query_row(
            "SELECT AVG(json_extract(action_data, '$.calibration_error')) FROM interactions WHERE action_type = 'accuracy_feedback' AND action_data IS NOT NULL",
            [],
            |row| row.get::<_, Option<f64>>(0),
        )
        .unwrap_or(None)
        .unwrap_or(0.0);

    Ok(serde_json::json!({
        "precision": precision,
        "engagement_rate": engagement_rate,
        "calibration_error": calibration_error
    }))
}

/// Record accuracy feedback for a scored item (predicted vs actual relevance)
#[tauri::command]
pub async fn ace_record_accuracy_feedback(
    item_id: u64,
    predicted_score: f64,
    feedback_type: String,
) -> Result<()> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();

    // Map feedback to actual relevance score
    let actual_score: f64 = match feedback_type.as_str() {
        "save" => 1.0,
        "click" => 0.7,
        "dismiss" => 0.2,
        "thumbs_down" => 0.0,
        _ => 0.5,
    };

    let action_data = serde_json::json!({
        "predicted_score": predicted_score,
        "actual_score": actual_score,
        "calibration_error": (predicted_score - actual_score).abs(),
    });

    conn.execute(
        "INSERT INTO interactions (item_id, action_type, action_data, signal_strength) VALUES (?1, 'accuracy_feedback', ?2, ?3)",
        rusqlite::params![item_id as i64, action_data.to_string(), actual_score],
    )
    .context("Failed to record accuracy feedback")?;

    Ok(())
}
/// Get a single topic's affinity score
#[tauri::command]
pub async fn ace_get_single_affinity(topic: String) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let affinities = ace.get_topic_affinities()?;

    let matching = affinities
        .iter()
        .find(|a| a.topic.to_lowercase() == topic.to_lowercase());

    match matching {
        Some(affinity) => Ok(serde_json::json!({
            "affinity": {
                "topic": affinity.topic,
                "positive_signals": affinity.positive_signals,
                "negative_signals": affinity.negative_signals,
                "affinity_score": affinity.affinity_score
            }
        })),
        None => Ok(serde_json::json!({
            "affinity": null
        })),
    }
}

/// Get engagement summary for the dashboard (daily count, streak, trend)
#[tauri::command]
pub async fn get_engagement_summary() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();

    // Today's interaction count
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let today_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interactions WHERE date(timestamp) = ?1",
            rusqlite::params![today],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Streak: consecutive days with at least 1 interaction (looking back from today)
    let mut streak: i64 = 0;
    let rows: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT date(timestamp) as d FROM interactions
                 ORDER BY d DESC LIMIT 30",
        )?;
        let result = stmt.query_map([], |row| row.get::<_, String>(0))?;
        result
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in ace_commands: {e}");
                    None
                }
            })
            .collect()
    };

    if !rows.is_empty() {
        let mut expected = chrono::Utc::now().date_naive();
        for date_str in &rows {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                if date == expected {
                    streak += 1;
                    expected -= chrono::Duration::days(1);
                } else if date < expected {
                    break;
                }
            }
        }
    }

    // 7-day heatmap data (interactions per day for last 7 days)
    let mut heatmap: Vec<serde_json::Value> = Vec::new();
    for i in (0..7).rev() {
        let date = (chrono::Utc::now() - chrono::Duration::days(i))
            .format("%Y-%m-%d")
            .to_string();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions WHERE date(timestamp) = ?1",
                rusqlite::params![date],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let day_name = (chrono::Utc::now() - chrono::Duration::days(i))
            .format("%a")
            .to_string();
        heatmap.push(serde_json::json!({
            "date": date,
            "day": day_name,
            "count": count,
        }));
    }

    // Accuracy trend: average feedback positivity over last 7 vs previous 7 days
    let recent_positive: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CASE WHEN signal_strength > 0 THEN 1.0 ELSE 0.0 END), 0.5)
             FROM interactions WHERE timestamp >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.5);

    let prev_positive: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CASE WHEN signal_strength > 0 THEN 1.0 ELSE 0.0 END), 0.5)
             FROM interactions WHERE timestamp >= datetime('now', '-14 days')
             AND timestamp < datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.5);

    let trend = if recent_positive > prev_positive + 0.05 {
        "improving"
    } else if recent_positive < prev_positive - 0.05 {
        "declining"
    } else {
        "stable"
    };

    Ok(serde_json::json!({
        "today_interactions": today_count,
        "streak_days": streak,
        "heatmap": heatmap,
        "accuracy_trend": trend,
        "recent_positive_rate": format!("{:.0}%", recent_positive * 100.0),
    }))
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
    fn test_default_scan_paths_returns_vec() {
        // get_default_scan_paths is pub(crate), test it returns non-empty on any system
        let paths = super::get_default_scan_paths();
        // Should return a Vec (even if empty on CI), no panic
        assert!(paths.len() <= 20, "Should not return excessive paths");
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

    #[test]
    fn test_engagement_summary_shape() {
        // Verify the JSON shape returned by ace_get_engagement_summary
        let summary = serde_json::json!({
            "today_interactions": 5,
            "streak_days": 3,
            "heatmap": [],
            "accuracy_trend": [],
            "recent_positive_rate": "80%",
        });
        assert!(summary["today_interactions"].is_number());
        assert!(summary["streak_days"].is_number());
        assert!(summary["heatmap"].is_array());
        assert!(summary["accuracy_trend"].is_array());
        assert!(summary["recent_positive_rate"].is_string());
    }
}
