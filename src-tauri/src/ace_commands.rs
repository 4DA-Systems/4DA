//! ACE (Autonomous Context Engine) Tauri commands and PASIFA helpers.
//!
//! Extracted from lib.rs. Contains all ACE phase commands (A through E),
//! autonomous discovery, watcher control, PASIFA README indexing,
//! and auto-seeding of interests from detected context.

use std::path::PathBuf;
use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::ace;
use crate::context_engine::InterestSource;
use crate::scoring::get_ace_context;
use crate::{
    chunk_text, embed_texts, get_ace_engine, get_ace_engine_mut, get_context_engine, get_database,
    get_settings_manager,
};

// ============================================================================
// ACE (Autonomous Context Engine) Commands
// ============================================================================

/// Trigger autonomous context detection
/// Scans specified paths for project manifests and extracts tech stack
#[tauri::command]
pub async fn ace_detect_context(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    // Convert string paths to PathBuf, expanding ~ to home directory
    let scan_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if p.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&p[2..]) // Skip "~/"
                } else {
                    PathBuf::from(p)
                }
            } else {
                PathBuf::from(p)
            }
        })
        .collect();

    // If no paths provided, use default locations
    let paths_to_scan = if scan_paths.is_empty() {
        get_default_scan_paths()
    } else {
        scan_paths
    };

    let context = ace.detect_context(&paths_to_scan)?;

    Ok(serde_json::json!({
        "success": true,
        "detected_tech": context.detected_tech,
        "active_topics": context.active_topics,
        "projects_scanned": context.projects_scanned,
        "context_confidence": context.context_confidence,
        "detection_time": context.detection_time
    }))
}

/// Get detected technologies from ACE
#[tauri::command]
pub async fn ace_get_detected_tech() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let tech = ace.get_detected_tech()?;

    Ok(serde_json::json!({
        "detected_tech": tech
    }))
}

/// Get active topics from ACE
#[tauri::command]
pub async fn ace_get_active_topics() -> Result<serde_json::Value, String> {
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

// ============================================================================
// ACE Phase B: Real-Time Context Commands
// ============================================================================

/// Analyze git repositories for context extraction
#[tauri::command]
pub async fn ace_analyze_git(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    let scan_paths: Vec<PathBuf> = if paths.is_empty() {
        get_default_scan_paths()
    } else {
        paths
            .iter()
            .map(|p| {
                if p.starts_with("~") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(&p[2..])
                    } else {
                        PathBuf::from(p)
                    }
                } else {
                    PathBuf::from(p)
                }
            })
            .collect()
    };

    let signals = ace.analyze_git_repos(&scan_paths)?;

    Ok(serde_json::json!({
        "success": true,
        "repos_analyzed": signals.len(),
        "signals": signals.iter().map(|s| serde_json::json!({
            "repo_name": s.repo_name,
            "repo_path": s.repo_path,
            "commit_count": s.recent_commits.len(),
            "branch_count": s.active_branches.len(),
            "commit_frequency": s.commit_frequency,
            "topics": s.extracted_topics,
            "confidence": s.confidence
        })).collect::<Vec<_>>()
    }))
}

/// Get real-time context (active topics + detected tech)
#[tauri::command]
pub async fn ace_get_realtime_context() -> Result<serde_json::Value, String> {
    let _ace = get_ace_engine()?;

    let conn = crate::open_db_connection()?;
    let conn = Arc::new(parking_lot::Mutex::new(conn));

    let context = ace::get_realtime_context(&conn)?;

    Ok(serde_json::json!({
        "active_topics": context.active_topics,
        "detected_tech": context.detected_tech,
        "context_confidence": context.context_confidence,
        "last_updated": context.last_updated
    }))
}

/// Apply freshness decay to active topics
#[tauri::command]
pub async fn ace_apply_decay() -> Result<serde_json::Value, String> {
    let conn = crate::open_db_connection()?;
    let conn = Arc::new(parking_lot::Mutex::new(conn));

    let updated = ace::apply_freshness_decay(&conn)?;

    Ok(serde_json::json!({
        "success": true,
        "topics_updated": updated
    }))
}

/// Run full autonomous context detection (manifests + git)
#[tauri::command]
pub async fn ace_full_scan(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let scan_paths: Vec<PathBuf> = if paths.is_empty() {
        get_default_scan_paths()
    } else {
        paths
            .iter()
            .map(|p| {
                if p.starts_with("~") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(&p[2..])
                    } else {
                        PathBuf::from(p)
                    }
                } else {
                    PathBuf::from(p)
                }
            })
            .collect()
    };

    info!(target: "4da::ace", paths = scan_paths.len(), "Starting full scan");

    // Phase 1 & 2: Manifest scanning and Git analysis (scoped to release ACE lock)
    let (manifest_context, git_signals) = {
        let ace = get_ace_engine()?;
        let manifest_context = ace.detect_context(&scan_paths)?;
        let git_signals = ace.analyze_git_repos(&scan_paths)?;
        (manifest_context, git_signals)
    }; // ACE lock is dropped here

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
        "combined": {
            "total_topics": total_topics.len(),
            "topics": total_topics.into_iter().collect::<Vec<_>>()
        }
    }))
}

/// Trigger autonomous context discovery - finds dev directories and projects automatically
/// This is the "just make it work" button - discovers context without user configuration
#[tauri::command]
pub async fn ace_auto_discover() -> Result<serde_json::Value, String> {
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
            return Err(format!("Failed to save discovered directories: {}", e));
        }
        let _ = settings.mark_auto_discovery_completed();
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

/// Reset auto-discovery flag to allow re-discovery
#[tauri::command]
pub async fn ace_reset_discovery() -> Result<serde_json::Value, String> {
    let mut settings = get_settings_manager().lock();
    settings.get_mut().auto_discovery_completed = false;
    settings.save()?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Auto-discovery reset. Next startup will re-discover directories."
    }))
}

/// Get current context directories and discovery status
#[tauri::command]
pub async fn ace_get_discovery_status() -> Result<serde_json::Value, String> {
    let settings = get_settings_manager().lock();
    let context_dirs = settings.get().context_dirs.clone();
    let auto_discovery_completed = settings.get().auto_discovery_completed;

    Ok(serde_json::json!({
        "auto_discovery_completed": auto_discovery_completed,
        "context_dirs": context_dirs,
        "context_dirs_count": context_dirs.len()
    }))
}

// ============================================================================
// ACE Phase C: Behavior Learning Commands
// ============================================================================

/// Record a user interaction for behavior learning
#[tauri::command]
pub async fn ace_record_interaction(
    item_id: i64,
    action_type: String,
    action_data: Option<serde_json::Value>,
    item_topics: Vec<String>,
    item_source: String,
) -> Result<serde_json::Value, String> {
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
        _ => return Err(format!("Unknown action type: {}", action_type)),
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
pub async fn ace_get_topic_affinities() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let affinities = ace.get_topic_affinities()?;

    Ok(serde_json::json!({
        "affinities": affinities,
        "count": affinities.len()
    }))
}

/// Get detected anti-topics
#[tauri::command]
pub async fn ace_get_anti_topics(min_rejections: Option<u32>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let threshold = min_rejections.unwrap_or(5);
    let anti_topics = ace.get_anti_topics(threshold)?;

    Ok(serde_json::json!({
        "anti_topics": anti_topics,
        "count": anti_topics.len(),
        "threshold": threshold
    }))
}

/// Confirm an auto-detected anti-topic
#[tauri::command]
pub async fn ace_confirm_anti_topic(topic: String) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    ace.confirm_anti_topic(&topic)?;

    Ok(serde_json::json!({
        "success": true,
        "confirmed": topic
    }))
}

/// Get behavior modifier for an item
#[tauri::command]
pub async fn ace_get_behavior_modifier(
    topics: Vec<String>,
    source: String,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let modifier = ace.get_behavior_modifier(&topics, &source)?;

    Ok(serde_json::json!({
        "modifier": modifier,
        "topics": topics,
        "source": source
    }))
}

/// Get full learned behavior summary
#[tauri::command]
pub async fn ace_get_learned_behavior() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let summary = ace.get_learned_behavior()?;

    Ok(serde_json::json!(summary))
}

/// Apply temporal decay to behavior learning
#[tauri::command]
pub async fn ace_apply_behavior_decay() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let updated = ace.apply_behavior_decay()?;

    Ok(serde_json::json!({
        "success": true,
        "affinities_updated": updated
    }))
}

// ============================================================================
// ACE Phase E: Embedding Commands
// ============================================================================

/// Get embedding for a topic
#[tauri::command]
pub async fn ace_embed_topic(topic: String) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let embedding = ace.embed_topic(&topic)?;

    Ok(serde_json::json!({
        "topic": topic,
        "embedding": embedding,
        "dimension": embedding.len()
    }))
}

/// Find similar topics using embeddings
#[tauri::command]
pub async fn ace_find_similar_topics(
    query: String,
    top_k: Option<usize>,
) -> Result<serde_json::Value, String> {
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
pub async fn ace_embedding_status() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let operational = ace.is_embedding_operational();

    Ok(serde_json::json!({
        "operational": operational
    }))
}

// ============================================================================
// ACE Phase E: Watcher Persistence Commands
// ============================================================================

/// Save watcher state for persistence
#[tauri::command]
pub async fn ace_save_watcher_state() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    ace.save_watcher_state()?;

    Ok(serde_json::json!({
        "saved": true
    }))
}

/// Restore watcher state from persistence
/// Note: This returns the saved state info. Actual restoration happens on app restart.
#[tauri::command]
pub async fn ace_get_watcher_state() -> Result<serde_json::Value, String> {
    // Watcher state is restored automatically on ACE initialization
    // This command provides info about the current state
    Ok(serde_json::json!({
        "info": "Watcher state is restored automatically on app startup. Use ace_save_watcher_state to persist current state."
    }))
}

/// Clear watcher state
#[tauri::command]
pub async fn ace_clear_watcher_state() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    ace.clear_watcher_state()?;

    Ok(serde_json::json!({
        "cleared": true
    }))
}

// ============================================================================
// ACE Phase E: Rate Limiting Commands
// ============================================================================

/// Get rate limit status for a source
#[tauri::command]
pub async fn ace_get_rate_limit_status(source: String) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let status = ace.get_rate_limit_status(&source);

    Ok(serde_json::json!(status))
}

// ============================================================================
// ACE Watcher Control Commands
// ============================================================================

/// Start file watching on specified directories
#[tauri::command]
pub async fn ace_start_watcher(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let mut ace = get_ace_engine_mut()?;

    let watch_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if p.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&p[2..])
                } else {
                    PathBuf::from(p)
                }
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

/// Stop file watching
#[tauri::command]
pub async fn ace_stop_watcher() -> Result<serde_json::Value, String> {
    let mut ace = get_ace_engine_mut()?;
    ace.stop_watching();

    Ok(serde_json::json!({
        "success": true,
        "watching": false
    }))
}

/// Check if watcher is active
#[tauri::command]
pub async fn ace_is_watching() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let watching = ace.is_watching();

    Ok(serde_json::json!({
        "watching": watching
    }))
}

// ============================================================================
// PASIFA: Discovered Context Indexing
// ============================================================================

/// Check if directory contains a project manifest
fn has_manifest(dir: &PathBuf) -> bool {
    let manifests = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "composer.json",
        "Gemfile",
        "pom.xml",
        "build.gradle",
        "CMakeLists.txt",
        "pubspec.yaml",
    ];

    for manifest in &manifests {
        if dir.join(manifest).exists() {
            return true;
        }
    }

    // Check for .csproj files
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "csproj" {
                    return true;
                }
            }
        }
    }

    false
}

/// Recursively discover project directories by finding manifests
/// Stops recursing when a manifest is found (don't nest into projects)
fn discover_projects_recursive(
    root: &PathBuf,
    max_depth: usize,
    skip_dirs: &[&str],
) -> Vec<PathBuf> {
    fn walk(
        dir: &PathBuf,
        depth: usize,
        max_depth: usize,
        skip_dirs: &[&str],
        projects: &mut Vec<PathBuf>,
    ) {
        if depth > max_depth {
            return;
        }

        // Check if this directory is a project
        if has_manifest(dir) {
            projects.push(dir.clone());
            return; // Stop recursing - we found a project
        }

        // Recurse into subdirectories
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if !path.is_dir() {
                    continue;
                }

                // Skip excluded directories
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if skip_dirs.contains(&name) {
                        continue;
                    }
                }

                walk(&path, depth + 1, max_depth, skip_dirs, projects);
            }
        }
    }

    let mut projects = Vec::new();
    walk(root, 0, max_depth, skip_dirs, &mut projects);
    projects
}

/// Parse README into sections with headings
#[derive(Debug)]
struct ReadmeSection {
    heading: String,
    content: String,
    #[allow(dead_code)] // Kept for future section hierarchy processing
    level: usize,
}

fn parse_readme_sections(content: &str) -> Vec<ReadmeSection> {
    let mut sections = Vec::new();
    let mut current_heading = String::from("Overview");
    let mut current_content = String::new();
    let mut current_level = 1;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for markdown heading
        if trimmed.starts_with('#') {
            // Save previous section if it has content
            if !current_content.trim().is_empty() {
                sections.push(ReadmeSection {
                    heading: current_heading.clone(),
                    content: current_content.trim().to_string(),
                    level: current_level,
                });
                current_content.clear();
            }

            // Parse new heading
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            let heading_text = trimmed.trim_start_matches('#').trim();

            if !heading_text.is_empty() {
                current_heading = heading_text.to_string();
                current_level = level;
            }
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Add final section
    if !current_content.trim().is_empty() {
        sections.push(ReadmeSection {
            heading: current_heading,
            content: current_content.trim().to_string(),
            level: current_level,
        });
    }

    sections
}

/// Determine weight for a README section based on heading
fn section_weight(heading: &str) -> f32 {
    let heading_lower = heading.to_lowercase();

    // High value sections
    if heading_lower.contains("feature")
        || heading_lower.contains("overview")
        || heading_lower.contains("about")
    {
        return 1.0;
    }

    // API and usage documentation
    if heading_lower.contains("api")
        || heading_lower.contains("usage")
        || heading_lower.contains("how to")
    {
        return 0.9;
    }

    // Architecture and design
    if heading_lower.contains("architect")
        || heading_lower.contains("design")
        || heading_lower.contains("structure")
    {
        return 0.85;
    }

    // Examples and demos
    if heading_lower.contains("example")
        || heading_lower.contains("demo")
        || heading_lower.contains("tutorial")
    {
        return 0.8;
    }

    // Installation and setup
    if heading_lower.contains("install")
        || heading_lower.contains("setup")
        || heading_lower.contains("getting started")
        || heading_lower.contains("quickstart")
    {
        return 0.7;
    }

    // Low value sections
    if heading_lower.contains("license")
        || heading_lower.contains("credit")
        || heading_lower.contains("author")
        || heading_lower.contains("contributor")
    {
        return 0.3;
    }

    // Default weight for other sections
    0.6
}

/// Index README files from discovered projects for semantic search
/// This is the bridge between ACE discovery and embedding-based relevance
/// Now with DEEP recursive project discovery and section-aware weighting
pub(crate) async fn index_discovered_readmes(context_dirs: &[PathBuf]) -> usize {
    info!(target: "4da::pasifa", dirs = context_dirs.len(), "Starting DEEP README indexing with recursive project discovery");

    if context_dirs.is_empty() {
        warn!(target: "4da::pasifa", "No context directories configured - cannot index READMEs");
        return 0;
    }

    let db = match get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::pasifa", error = %e, "Database not available");
            return 0;
        }
    };

    // Directories to skip during recursive scan
    let skip_dirs = [
        "node_modules",
        "target",
        ".git",
        "dist",
        "build",
        ".next",
        "__pycache__",
        ".venv",
        "venv",
        "vendor",
        ".cargo",
        "pkg",
    ];

    // Discover all projects recursively (max depth 3)
    let mut all_projects = Vec::new();
    for dir in context_dirs {
        if !dir.exists() {
            warn!(target: "4da::pasifa", dir = %dir.display(), "Context directory does not exist");
            continue;
        }

        let discovered = discover_projects_recursive(dir, 3, &skip_dirs);
        debug!(target: "4da::pasifa", dir = %dir.display(), projects = discovered.len(), "Discovered projects recursively");
        all_projects.extend(discovered);
    }

    info!(target: "4da::pasifa", total_projects = all_projects.len(), "Completed recursive project discovery");

    let mut indexed_chunks = 0;
    let mut found_readme_count = 0;
    let mut section_count = 0;
    let readme_names = ["README.md", "README.txt", "README", "readme.md"];
    let total_projects = all_projects.len();

    // Process each discovered project
    for project_dir in &all_projects {
        // Find README in this project
        let mut readme_found = false;
        for readme_name in &readme_names {
            let readme_path = project_dir.join(readme_name);
            if readme_path.exists() && readme_path.is_file() {
                found_readme_count += 1;
                readme_found = true;
                debug!(target: "4da::pasifa", path = %readme_path.display(), "Found README file");

                match std::fs::read_to_string(&readme_path) {
                    Ok(content) => {
                        if content.len() < 100 {
                            debug!(target: "4da::pasifa", path = %readme_path.display(), len = content.len(), "README too short, skipping");
                            continue;
                        }

                        // Parse README into sections
                        let sections = parse_readme_sections(&content);
                        let num_sections = sections.len();
                        section_count += num_sections;
                        debug!(target: "4da::pasifa", path = %readme_path.display(), sections = num_sections, "Parsed README sections");

                        // Process each section with appropriate weight
                        for section in &sections {
                            let weight = section_weight(&section.heading);

                            // Skip very short sections
                            if section.content.len() < 50 {
                                continue;
                            }

                            // Chunk the section content
                            let source_info =
                                format!("{}#{}", readme_path.to_string_lossy(), section.heading);
                            let chunks = chunk_text(&section.content, &source_info);

                            for (chunk_source, chunk_content) in chunks {
                                if chunk_content.len() < 50 {
                                    continue;
                                }

                                // Generate embedding
                                match embed_texts(std::slice::from_ref(&chunk_content)).await {
                                    Ok(embeddings) if !embeddings.is_empty() => {
                                        // Store with weight in context_chunks table
                                        match db.upsert_context_weighted(
                                            &chunk_source,
                                            &chunk_content,
                                            &embeddings[0],
                                            weight,
                                        ) {
                                            Ok(_) => {
                                                indexed_chunks += 1;
                                                debug!(target: "4da::pasifa",
                                                    section = &section.heading,
                                                    weight = weight,
                                                    "Indexed weighted section chunk"
                                                );
                                            }
                                            Err(e) => {
                                                warn!(target: "4da::pasifa",
                                                    path = %readme_path.display(),
                                                    section = &section.heading,
                                                    error = %e,
                                                    "Failed to upsert weighted context"
                                                );
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        debug!(target: "4da::pasifa", "Embedding returned empty result");
                                    }
                                    Err(e) => {
                                        warn!(target: "4da::pasifa",
                                            path = %readme_path.display(),
                                            section = &section.heading,
                                            error = %e,
                                            "Failed to embed - check API key configuration"
                                        );
                                    }
                                }
                            }
                        }

                        info!(target: "4da::pasifa",
                            path = %readme_path.display(),
                            sections = sections.len(),
                            chunks = indexed_chunks,
                            "Indexed README with section weighting"
                        );
                        break; // Only index first README found per project
                    }
                    Err(e) => {
                        debug!(target: "4da::pasifa", path = %readme_path.display(), error = %e, "Failed to read");
                    }
                }
            }
        }

        if !readme_found {
            debug!(target: "4da::pasifa", project = %project_dir.display(), "No README found in project");
        }
    }

    if found_readme_count == 0 {
        info!(target: "4da::pasifa", "No README files found in discovered projects");
    } else if indexed_chunks == 0 {
        warn!(target: "4da::pasifa", found = found_readme_count, "Found READMEs but failed to index - check embedding API key");
    } else {
        info!(target: "4da::pasifa",
            projects = total_projects,
            readmes = found_readme_count,
            sections = section_count,
            chunks = indexed_chunks,
            "DEEP README indexing complete with section weighting"
        );
    }

    indexed_chunks
}

// ============================================================================
// Auto-Seed Interests from ACE Context
// ============================================================================

/// Automatically seed user interests from ACE-detected technologies
/// This runs once at startup when interests are empty, providing immediate value
/// without requiring manual configuration.
pub(crate) async fn auto_seed_interests_from_ace() -> Result<(), String> {
    // Check if interests are already configured
    let context_engine = get_context_engine()?;
    let existing_interests = context_engine
        .get_interests()
        .map_err(|e| format!("Failed to get interests: {}", e))?;

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
        if !skip_list.contains(&tech.as_str()) {
            topics_to_seed.push((tech.clone(), 0.8));
        }
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

// ============================================================================
// Auto-Interest Discovery: Suggested Interests from ACE Context
// ============================================================================

/// Get suggested interests based on ACE-detected technologies and active topics.
/// Cross-references with existing interests and exclusions to avoid duplicates.
#[tauri::command]
pub async fn ace_get_suggested_interests() -> Result<Vec<serde_json::Value>, String> {
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

// ============================================================================
// ACE Phase 1C: Anomaly Detection Commands
// ============================================================================

/// Get all unresolved anomalies
#[tauri::command]
pub async fn ace_get_unresolved_anomalies() -> Result<serde_json::Value, String> {
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
pub async fn ace_detect_anomalies() -> Result<serde_json::Value, String> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = crate::anomaly::detect_all(&conn)?;
    for a in &anomalies {
        let _ = crate::anomaly::store_anomaly(&conn, a);
    }
    Ok(serde_json::json!({
        "anomalies": anomalies,
        "count": anomalies.len()
    }))
}

/// Resolve (dismiss) an anomaly by id
#[tauri::command]
pub async fn ace_resolve_anomaly(anomaly_id: i64) -> Result<(), String> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    crate::anomaly::resolve_anomaly(&conn, anomaly_id)
}

/// Get accuracy metrics calculated from interactions
#[tauri::command]
pub async fn ace_get_accuracy_metrics() -> Result<serde_json::Value, String> {
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
) -> Result<(), String> {
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
    .map_err(|e| format!("Failed to record accuracy feedback: {}", e))?;

    Ok(())
}

/// Get system health report
#[tauri::command]
pub async fn ace_get_system_health() -> Result<serde_json::Value, String> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let report = crate::health::check_all_components(&conn)?;
    serde_json::to_value(&report).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_weight() {
        // High value sections
        assert_eq!(section_weight("Features"), 1.0);
        assert_eq!(section_weight("Overview"), 1.0);
        assert_eq!(section_weight("About"), 1.0);

        // API/Usage sections
        assert_eq!(section_weight("API Reference"), 0.9);
        assert_eq!(section_weight("Usage Guide"), 0.9);

        // Architecture sections
        assert_eq!(section_weight("Architecture"), 0.85);
        assert_eq!(section_weight("Design Patterns"), 0.85);

        // Examples sections
        assert_eq!(section_weight("Examples"), 0.8);
        assert_eq!(section_weight("Demo"), 0.8);

        // Installation sections
        assert_eq!(section_weight("Installation"), 0.7);
        assert_eq!(section_weight("Getting Started"), 0.7);

        // Low value sections
        assert_eq!(section_weight("License"), 0.3);
        assert_eq!(section_weight("Contributors"), 0.3);

        // Default weight
        assert_eq!(section_weight("Random Section"), 0.6);
    }

    #[test]
    fn test_parse_readme_sections() {
        let readme = r#"# Project Title

Some intro text here.

## Features

- Feature 1
- Feature 2

## Installation

Install with npm:

```bash
npm install
```

## License

MIT License
"#;

        let sections = parse_readme_sections(readme);

        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].heading, "Project Title");
        assert!(sections[0].content.contains("Some intro text"));
        assert_eq!(sections[1].heading, "Features");
        assert!(sections[1].content.contains("Feature 1"));
        assert_eq!(sections[2].heading, "Installation");
        assert!(sections[2].content.contains("npm install"));
        assert_eq!(sections[3].heading, "License");
        assert!(sections[3].content.contains("MIT"));
    }

    #[test]
    fn test_has_manifest_logic() {
        // Test manifest detection on current project (should have Cargo.toml)
        let current_dir = std::env::current_dir().unwrap();
        assert!(
            has_manifest(&current_dir),
            "Current directory should have Cargo.toml"
        );

        // Test on a directory that definitely won't have a manifest
        let non_project_dir = PathBuf::from("/nonexistent/path");
        assert!(
            !has_manifest(&non_project_dir),
            "Nonexistent directory should not have manifest"
        );
    }

    #[test]
    fn test_discover_projects_on_current_dir() {
        // Test recursive discovery on current project
        let current_dir = std::env::current_dir().unwrap();
        let skip_dirs = ["node_modules", "target", ".git", "dist", "build"];

        // Discover projects with depth 2
        let projects = discover_projects_recursive(&current_dir, 2, &skip_dirs);

        // Should find at least the current project (has Cargo.toml)
        assert!(
            !projects.is_empty(),
            "Should discover at least the current project"
        );

        // Current directory should be in the list
        assert!(
            projects.contains(&current_dir),
            "Should discover current project directory"
        );
    }

    // ========================================================================
    // Suggested Interests Filtering Tests
    // ========================================================================

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
}
