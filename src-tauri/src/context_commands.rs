//! Context management Tauri commands.
//!
//! Extracted from lib.rs. Contains context file reading, indexing,
//! clearing, settings, and directory management commands.

use std::fs;
use std::path::PathBuf;

use tracing::{debug, info, warn};

use crate::{
    ace_commands, chunk_text, embed_texts, get_context_dir, get_context_dirs, get_database,
    get_settings_manager, ContextFile, ContextSettings, SUPPORTED_EXTENSIONS,
};

#[tauri::command]
pub async fn get_context_files() -> Result<Vec<ContextFile>, String> {
    let context_dir = match get_context_dir() {
        Some(dir) => dir,
        None => {
            debug!(target: "4da::context", "No context directory configured");
            return Ok(vec![]);
        }
    };
    debug!(target: "4da::context", path = ?context_dir, "Reading context files");

    if !context_dir.exists() {
        debug!(target: "4da::context", "Context directory does not exist");
        return Ok(vec![]);
    }

    let mut files = Vec::new();
    let entries = fs::read_dir(&context_dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
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

    info!(target: "4da::context", count = files.len(), "Total context files loaded");
    Ok(files)
}

/// Clear all indexed context chunks from the database
#[tauri::command]
pub async fn clear_context() -> Result<String, String> {
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
pub async fn index_context() -> Result<String, String> {
    info!(target: "4da::context", "Indexing context files");

    let db = get_database()?;

    // First clear existing context to avoid duplicates
    let _ = db.clear_contexts();

    // Read context files from configured directories
    let context_files = get_context_files().await?;
    if context_files.is_empty() {
        return Err("No context files found. Add files to your context directory.".to_string());
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
        return Err("No content to index from context files.".to_string());
    }

    // Generate embeddings
    debug!(target: "4da::embed", chunks = all_chunks.len(), "Generating embeddings for chunks");
    let chunk_texts: Vec<String> = all_chunks.iter().map(|(_, text)| text.clone()).collect();
    let chunk_embeddings = embed_texts(&chunk_texts)?;

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
pub async fn index_project_readmes() -> Result<String, String> {
    info!(target: "4da::context", "Indexing READMEs from all configured directories");

    let context_dirs = get_context_dirs();
    if context_dirs.is_empty() {
        return Err("No context directories configured".to_string());
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

/// Get current context directory settings
#[tauri::command]
pub async fn get_context_settings() -> Result<ContextSettings, String> {
    let dirs = get_context_dirs();
    let settings = get_settings_manager().lock();
    let configured = settings.get().context_dirs.clone();
    drop(settings);

    let using_default = configured.is_empty();
    Ok(ContextSettings {
        configured_dirs: configured,
        active_dirs: dirs
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        using_default,
    })
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
pub async fn set_context_dirs(dirs: Vec<String>) -> Result<String, String> {
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
            return Err(format!(
                "Directory does not exist: {} (tried: {})",
                dir, converted
            ));
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", converted));
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
}
