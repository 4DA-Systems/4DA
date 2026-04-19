// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Autonomous directory discovery for user context.
//!
//! Discovers directories that define the user's context on their system.
//! This is the core of ACE autonomy — finding ALL relevant context, not just code.

use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Maximum directories to discover to prevent OOM
const MAX_DISCOVERED_DIRECTORIES: usize = 1000;

/// Discovers directories that define the user's context on their system.
/// Context can come from ANY directory: projects, documents, notes, research, etc.
pub fn discover_dev_directories() -> Vec<String> {
    let mut discovered: Vec<String> = Vec::new();

    // Get home directory
    let home = dirs::home_dir();

    if let Some(home_path) = home {
        // Context-relevant directories (not just dev!)
        let context_dirs = [
            // Development
            "Projects",
            "projects",
            "Development",
            "development",
            "dev",
            "Dev",
            "code",
            "Code",
            "src",
            "work",
            "Work",
            "repos",
            "Repos",
            "github",
            "GitHub",
            "git",
            "workspace",
            "Workspace",
            "source",
            "Source",
            "source/repos",
            "Source/Repos",
            // Documents & Notes (context!)
            "Documents",
            "documents",
            "Notes",
            "notes",
            "Obsidian",
            "Research",
            "research",
            "Writing",
            "writing",
            // Learning & Reference
            "Books",
            "books",
            "Articles",
            "articles",
            "Papers",
            "papers",
            // Creative/Work
            "Design",
            "design",
            "Creative",
            "creative",
        ];

        for dir_name in context_dirs {
            let dir_path = home_path.join(dir_name);
            if dir_path.exists() && dir_path.is_dir() {
                info!(target: "4da::discovery", path = %dir_path.display(), "Found context directory");
                discovered.push(dir_path.display().to_string());
            }
        }

        // Also check for common WSL mount points (for Windows users in WSL)
        #[cfg(target_os = "linux")]
        {
            let wsl_mounts = ["/mnt/c", "/mnt/d", "/mnt/e"];
            // Context indicators (code AND content)
            let context_markers = [
                "package.json",
                "Cargo.toml",
                "pyproject.toml",
                "go.mod",
                ".git",
                ".obsidian",
                "README.md",
                "index.md",
            ];
            let skip_dirs = [
                "$RECYCLE.BIN",
                "System Volume Information",
                "Windows",
                "Program Files",
                "Program Files (x86)",
                "ProgramData",
                "Recovery",
                "Users",
            ];

            for mount in wsl_mounts {
                let mount_path = PathBuf::from(mount);
                if !mount_path.exists() {
                    continue;
                }

                // Check common locations on mounted drives (including context dirs, not just dev)
                for subdir in [
                    "Users",
                    "projects",
                    "code",
                    "dev",
                    "Documents",
                    "Notes",
                    "Research",
                    "Work",
                ] {
                    let check_path = mount_path.join(subdir);
                    if check_path.exists() && check_path.is_dir() {
                        // Don't add entire Users folder, look for specific user folders
                        if subdir == "Users" {
                            // Try to find user's folder
                            if let Ok(entries) = fs::read_dir(&check_path) {
                                for entry in entries.flatten() {
                                    let user_path = entry.path();
                                    if user_path.is_dir() {
                                        for dev_dir in
                                            ["Projects", "code", "dev", "repos", "source"]
                                        {
                                            let dev_path = user_path.join(dev_dir);
                                            if dev_path.exists() && dev_path.is_dir() {
                                                info!(target: "4da::discovery", path = %dev_path.display(), "Found WSL dev directory");
                                                discovered.push(dev_path.display().to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            info!(target: "4da::discovery", path = %check_path.display(), "Found WSL dev directory");
                            discovered.push(check_path.display().to_string());
                        }
                    }
                }

                // CRITICAL: Also scan root of mounts for project directories
                // This finds projects like /mnt/d/4DA that aren't in standard folders
                info!(target: "4da::discovery", mount = mount, "Scanning mount root for projects");
                if let Ok(entries) = fs::read_dir(&mount_path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        let entry_name =
                            entry_path.file_name().unwrap_or_default().to_string_lossy();

                        // Skip system directories
                        if skip_dirs.iter().any(|s| entry_name.eq_ignore_ascii_case(s)) {
                            continue;
                        }

                        // Skip hidden directories and files
                        if entry_name.starts_with('.') || entry_name.starts_with('$') {
                            continue;
                        }

                        if entry_path.is_dir() {
                            // Check if this directory has context markers (code or content)
                            for marker in context_markers {
                                if entry_path.join(marker).exists() {
                                    info!(target: "4da::discovery", path = %entry_path.display(), "Found context directory at mount root");
                                    discovered.push(entry_path.display().to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Deduplicate
    discovered.sort();
    discovered.dedup();

    info!(target: "4da::discovery", count = discovered.len(), "Total directories discovered");
    discovered
}

/// Deep scan for context-defining directories
/// Returns directories containing projects OR significant context (notes, docs, etc.)
pub fn find_project_directories(base_dirs: &[String], max_depth: usize) -> Vec<String> {
    let mut project_dirs: Vec<String> = Vec::new();
    // Context indicators: code manifests AND content markers
    let manifest_files = [
        // Code projects
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "Gemfile",
        "composer.json",
        ".git",
        // Obsidian/notes vaults
        ".obsidian",
        // Documentation
        "README.md",
        "index.md",
    ];

    let skip_dirs = [
        "node_modules",
        "target",
        ".git",
        "dist",
        "build",
        "__pycache__",
        ".next",
        "vendor",
        ".cargo",
    ];

    fn scan_recursive(
        path: &std::path::Path,
        depth: usize,
        max_depth: usize,
        manifests: &[&str],
        skip: &[&str],
        results: &mut Vec<String>,
        max_results: usize,
    ) {
        // Bound check: stop if we've hit the limit
        if results.len() >= max_results || depth > max_depth || !path.is_dir() {
            return;
        }

        // Check if this directory has a manifest
        for manifest in manifests {
            if path.join(manifest).exists() {
                results.push(path.display().to_string());
                return; // Don't recurse deeper once we find a project
            }
        }

        // Recurse into subdirectories
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                // Check bound before each recursion
                if results.len() >= max_results {
                    return;
                }
                let entry_path = entry.path();
                if entry_path.is_dir() && !entry_path.is_symlink() {
                    let name = entry_path.file_name().unwrap_or_default().to_string_lossy();
                    if !skip.contains(&name.as_ref()) {
                        scan_recursive(
                            &entry_path,
                            depth + 1,
                            max_depth,
                            manifests,
                            skip,
                            results,
                            max_results,
                        );
                    }
                }
            }
        }
    }

    for base in base_dirs {
        // Stop if we've hit the limit
        if project_dirs.len() >= MAX_DISCOVERED_DIRECTORIES {
            break;
        }
        let base_path = PathBuf::from(base);
        scan_recursive(
            &base_path,
            0,
            max_depth,
            &manifest_files,
            &skip_dirs,
            &mut project_dirs,
            MAX_DISCOVERED_DIRECTORIES,
        );
    }

    project_dirs.sort();
    project_dirs.dedup();

    info!(target: "4da::discovery", count = project_dirs.len(), "Found project directories");
    project_dirs
}
