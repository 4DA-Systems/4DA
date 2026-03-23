//! ACE watcher and rate limit commands.

use std::path::PathBuf;

use crate::error::Result;
use crate::{get_ace_engine, get_ace_engine_mut};

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
