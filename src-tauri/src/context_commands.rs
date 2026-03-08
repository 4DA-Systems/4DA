//! Context management Tauri commands.
//!
//! Extracted from lib.rs. Contains context file reading, indexing,
//! clearing, settings, and directory management commands.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::error::Result;
use crate::{
    ace_commands, chunk_text, embed_texts, get_context_dir, get_database, get_settings_manager,
    ContextFile, SUPPORTED_EXTENSIONS,
};

/// Directories to skip during recursive context scanning
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    ".next",
    "dist",
    ".venv",
    "venv",
    ".cache",
    "build",
    "specs",
    "_future",
    "dev-tools",
    "ton-library",
    "test-context",
];

/// Files to skip — project meta-docs that pollute context with generic tech terms
const SKIP_FILES: &[&str] = &[
    "COMPARISON.md",
    "CONFIDENCE_SCORE_IMPLEMENTATION.md",
    "IMPLEMENTATION_PLAN.md",
    "MISSION_ACCOMPLISHED.md",
    "SHIP_READINESS_VERIFICATION.md",
    "README-MARKETING.md",
    "CHANGELOG.md",
    "LICENSE",
    "LICENSE.md",
];

/// Check if a filename is a project meta-doc (SCREAMING_CASE.md pattern)
fn is_meta_doc(name: &str) -> bool {
    if SKIP_FILES.iter().any(|&f| name.eq_ignore_ascii_case(f)) {
        return true;
    }
    // Skip SCREAMING_CASE markdown files (e.g., AI_ENGINEERING_CONTRACT.md, VALIDATION_CHECKLIST.md)
    // These are project management docs, not code context
    if let Some(stem) = name.strip_suffix(".md") {
        let has_upper = stem.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = stem.chars().any(|c| c.is_ascii_lowercase());
        // SCREAMING_CASE: has uppercase + underscores, no lowercase
        if has_upper && !has_lower && stem.contains('_') {
            return true;
        }
    }
    false
}

/// Recursively collect context files from a directory (max depth 3)
fn collect_context_files(dir: &Path, files: &mut Vec<ContextFile>, depth: usize) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if path.is_dir() {
            if !SKIP_DIRS.contains(&name) && !name.starts_with('.') {
                collect_context_files(&path, files, depth + 1);
            }
            continue;
        }

        // Skip meta-docs that pollute context
        if is_meta_doc(name) {
            debug!(target: "4da::context", file = name, "Skipping meta-doc");
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !SUPPORTED_EXTENSIONS.contains(&ext) {
            continue;
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                let lines = content.lines().count();
                let path_str = path.to_string_lossy().to_string();
                debug!(target: "4da::context", path = %path_str, lines = lines, "Loaded context file");
                files.push(ContextFile {
                    path: path_str,
                    content,
                    lines,
                });
            }
            Err(e) => {
                warn!(target: "4da::context", path = ?path, error = %e, "Failed to read context file");
            }
        }
    }
}

#[tauri::command]
pub async fn get_context_files() -> Result<Vec<ContextFile>> {
    let context_dir = match get_context_dir() {
        Some(dir) => dir,
        None => {
            debug!(target: "4da::context", "No context directory configured");
            return Ok(vec![]);
        }
    };
    debug!(target: "4da::context", path = ?context_dir, "Reading context files (recursive, depth 3)");

    if !context_dir.exists() {
        debug!(target: "4da::context", path = ?context_dir, "Context directory does not exist");
        return Ok(vec![]);
    }

    let mut files = Vec::new();
    collect_context_files(&context_dir, &mut files, 0);

    info!(target: "4da::context", count = files.len(), "Total context files loaded (recursive)");
    Ok(files)
}

/// Clear all indexed context chunks from the database
#[tauri::command]
pub async fn clear_context() -> Result<String> {
    info!(target: "4da::context", "Clearing indexed context");

    // Use the singleton database connection (same one used by analysis)
    let db = get_database()?;

    let cleared = db
        .clear_contexts()
        .map_err(|e| format!("Failed to clear context: {}", e))?;

    info!(target: "4da::context", chunks_removed = cleared, "Context cleared successfully");
    Ok(format!(
        "Context cleared successfully ({} chunks removed)",
        cleared
    ))
}

/// Index context files - read, chunk, embed, and store in database
#[tauri::command]
pub async fn index_context() -> Result<String> {
    info!(target: "4da::context", "Indexing context files");

    let db = get_database()?;

    // First clear existing context to avoid duplicates
    let _ = db.clear_contexts();

    // Read context files from configured directories
    let context_files = get_context_files().await?;
    if context_files.is_empty() {
        return Err("No context files found. Add files to your context directory.".into());
    }

    // Chunk the files
    let mut all_chunks: Vec<(String, String)> = Vec::new();
    for file in &context_files {
        let filename = file
            .path
            .split('/')
            .next_back()
            .and_then(|s| s.split('\\').next_back())
            .unwrap_or(&file.path);
        let chunks = chunk_text(&file.content, filename);
        debug!(target: "4da::context", file = filename, chunks = chunks.len(), "Chunked file");
        all_chunks.extend(chunks);
    }

    if all_chunks.is_empty() {
        return Err("No content to index from context files.".into());
    }

    // Generate embeddings
    debug!(target: "4da::embed", chunks = all_chunks.len(), "Generating embeddings for chunks");
    let chunk_texts: Vec<String> = all_chunks.iter().map(|(_, text)| text.clone()).collect();
    let chunk_embeddings = embed_texts(&chunk_texts).await?;

    // Store in database
    debug!(target: "4da::context", chunks = all_chunks.len(), "Storing context chunks in database");
    for ((source, text), embedding) in all_chunks.iter().zip(chunk_embeddings.iter()) {
        db.upsert_context(source, text, embedding)
            .map_err(|e| format!("Failed to store context: {}", e))?;
    }

    info!(target: "4da::context", files = context_files.len(), chunks = all_chunks.len(), "Context indexed successfully");
    Ok(format!(
        "Indexed {} files ({} chunks)",
        context_files.len(),
        all_chunks.len()
    ))
}

/// Index READMEs from all configured context directories
/// This scans all context_dirs and indexes README files for semantic search
#[tauri::command]
pub async fn index_project_readmes() -> Result<String> {
    info!(target: "4da::context", "Indexing READMEs from all configured directories");

    let context_dirs = crate::get_context_dirs();
    if context_dirs.is_empty() {
        return Err("No context directories configured".into());
    }

    let indexed_count = ace_commands::index_discovered_readmes(&context_dirs).await;

    if indexed_count > 0 {
        info!(target: "4da::context", count = indexed_count, "README chunks indexed");
        Ok(format!(
            "Indexed {} README chunks from {} directories",
            indexed_count,
            context_dirs.len()
        ))
    } else {
        Ok("No README files found in configured directories".to_string())
    }
}
/// Convert Windows path to WSL path if needed (e.g., D:\projects -> /mnt/d/projects)
fn convert_windows_to_wsl_path(path: &str) -> String {
    // Check if it looks like a Windows path (e.g., "D:\something" or "D:/something")
    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        let drive = path
            .chars()
            .next()
            .unwrap_or('c')
            .to_lowercase()
            .next()
            .unwrap_or('c');
        let rest = &path[2..].replace('\\', "/");
        format!("/mnt/{}{}", drive, rest)
    } else {
        path.to_string()
    }
}

#[tauri::command]
pub async fn set_context_dirs(dirs: Vec<String>) -> Result<String> {
    info!(target: "4da::context", dirs = ?dirs, "Setting context directories");

    // Convert Windows paths to WSL paths and validate
    let mut converted_dirs: Vec<String> = Vec::new();
    for dir in &dirs {
        let converted = convert_windows_to_wsl_path(dir);
        if converted != *dir {
            debug!(target: "4da::context", from = dir, to = %converted, "Converted Windows path");
        }

        let path = PathBuf::from(&converted);
        if !path.exists() {
            return Err(format!("Directory does not exist: {} (tried: {})", dir, converted).into());
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", converted).into());
        }
        converted_dirs.push(converted);
    }

    let mut settings = get_settings_manager().lock();
    settings.get_mut().context_dirs = converted_dirs.clone();
    settings.save()?;
    drop(settings);

    info!(target: "4da::context", dirs = ?converted_dirs, "Context directories updated");
    Ok(format!(
        "Context directories updated: {} directories configured",
        converted_dirs.len()
    ))
}

#[tauri::command]
pub async fn get_context_dirs() -> Result<Vec<String>> {
    Ok(crate::get_context_dirs()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_windows_to_wsl_path() {
        assert_eq!(
            convert_windows_to_wsl_path("D:\\projects\\test"),
            "/mnt/d/projects/test"
        );
        assert_eq!(
            convert_windows_to_wsl_path("C:\\Users\\foo"),
            "/mnt/c/Users/foo"
        );
    }

    #[test]
    fn test_convert_windows_to_wsl_path_already_unix() {
        let unix_path = "/mnt/d/already/unix";
        assert_eq!(convert_windows_to_wsl_path(unix_path), unix_path);
    }

    // -- is_meta_doc --

    #[test]
    fn is_meta_doc_explicit_skip_list() {
        assert!(is_meta_doc("COMPARISON.md"));
        assert!(is_meta_doc("IMPLEMENTATION_PLAN.md"));
        assert!(is_meta_doc("MISSION_ACCOMPLISHED.md"));
        assert!(is_meta_doc("SHIP_READINESS_VERIFICATION.md"));
        assert!(is_meta_doc("README-MARKETING.md"));
        assert!(is_meta_doc("CHANGELOG.md"));
        assert!(is_meta_doc("LICENSE"));
        assert!(is_meta_doc("LICENSE.md"));
    }

    #[test]
    fn is_meta_doc_case_insensitive_skip() {
        assert!(is_meta_doc("changelog.md"));
        assert!(is_meta_doc("Changelog.md"));
        assert!(is_meta_doc("license"));
    }

    #[test]
    fn is_meta_doc_screaming_case_with_underscores() {
        assert!(is_meta_doc("AI_ENGINEERING_CONTRACT.md"));
        assert!(is_meta_doc("VALIDATION_CHECKLIST.md"));
        assert!(is_meta_doc("BUILD_CONFIG.md"));
    }

    #[test]
    fn is_meta_doc_single_word_caps_no_underscore() {
        // Single-word allcaps WITHOUT underscore — fails screaming case check
        // Only matches if in explicit skip list
        assert!(!is_meta_doc("SECURITY.md"));
        assert!(!is_meta_doc("CONTRIBUTING.md"));
    }

    #[test]
    fn is_meta_doc_regular_markdown_not_filtered() {
        assert!(!is_meta_doc("api.md"));
        assert!(!is_meta_doc("setup.md"));
        assert!(!is_meta_doc("getting-started.md"));
        assert!(!is_meta_doc("README.md"));
    }

    #[test]
    fn is_meta_doc_non_md_not_filtered() {
        assert!(!is_meta_doc("BUILD_CONFIG.toml"));
        assert!(!is_meta_doc("Cargo.toml"));
        assert!(!is_meta_doc("lib.rs"));
    }

    #[test]
    fn is_meta_doc_empty_string() {
        assert!(!is_meta_doc(""));
    }

    #[test]
    fn is_meta_doc_mixed_case_with_underscore() {
        // Has lowercase — not screaming case
        assert!(!is_meta_doc("My_Custom_Doc.md"));
    }
}
