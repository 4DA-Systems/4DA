//! README indexing pipeline — discovers projects, parses READMEs, generates weighted embeddings.
//!
//! This is the PASIFA bridge between ACE discovery and embedding-based relevance.

use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::{chunk_text, embed_texts, get_database};

// ============================================================================
// Project Discovery
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

// ============================================================================
// README Parsing
// ============================================================================

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

// ============================================================================
// Main Indexing Pipeline
// ============================================================================

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
    let mut consecutive_failures = 0u32;
    const MAX_CONSECUTIVE_FAILURES: u32 = 3;
    let readme_names = ["README.md", "README.txt", "README", "readme.md"];
    let total_projects = all_projects.len();

    // Process each discovered project
    for project_dir in &all_projects {
        if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            warn!(target: "4da::pasifa",
                failures = consecutive_failures,
                "Stopping README indexing: {} consecutive embedding failures (check embedding provider)",
                consecutive_failures
            );
            break;
        }

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
                            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                                break;
                            }

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
                                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                                    break;
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
                                                consecutive_failures = 0; // Reset on success
                                                debug!(target: "4da::pasifa",
                                                    section = &section.heading,
                                                    weight = weight,
                                                    "Indexed weighted section chunk"
                                                );
                                            }
                                            Err(e) => {
                                                consecutive_failures += 1;
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
                                        consecutive_failures += 1;
                                        debug!(target: "4da::pasifa", "Embedding returned empty result");
                                    }
                                    Err(e) => {
                                        consecutive_failures += 1;
                                        warn!(target: "4da::pasifa",
                                            path = %readme_path.display(),
                                            section = &section.heading,
                                            error = %e,
                                            failures = consecutive_failures,
                                            "Embedding failed ({}/{})",
                                            consecutive_failures, MAX_CONSECUTIVE_FAILURES,
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
// Tests
// ============================================================================

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
}
