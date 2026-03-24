//! ACE scanning commands: full scan, auto-discover, scan summary, and path helpers.

use std::path::PathBuf;

use tracing::{debug, info, warn};

use crate::ace;
use crate::error::Result;
use crate::{get_ace_engine, get_ace_engine_mut, get_settings_manager};

use super::index_discovered_readmes;

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
        super::dependencies::store_direct_dependencies(db);
    }

    // Phase 1a-lockfiles: Parse lockfiles for transitive dependency discovery
    if let Ok(db) = crate::get_database() {
        super::dependencies::store_lockfile_dependencies(db, &scan_paths);
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
            return Err(format!("Failed to save discovered directories: {e}").into());
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_default_scan_paths_returns_vec() {
        // get_default_scan_paths is pub(crate), test it returns non-empty on any system
        let paths = super::get_default_scan_paths();
        // Should return a Vec (even if empty on CI), no panic
        assert!(paths.len() <= 20, "Should not return excessive paths");
    }
}
