//! Context management Tauri commands.
//!
//! Extracted from lib.rs. Contains context file reading, indexing,
//! clearing, settings, and directory management commands.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};
use crate::utils::sanitize_path;
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

    let cleared = db.clear_contexts().context("Failed to clear context")?;

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
    if let Err(e) = db.clear_contexts() {
        tracing::warn!("Failed to clear contexts: {e}");
    }

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
            .context("Failed to store context")?;
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
/// Sync AWE wisdom into context — injects validated principles and anti-patterns
/// as high-weight context chunks so PASIFA scoring is informed by decision history.
///
/// Also scans configured context directories for decision-shaped git commits.
#[tauri::command]
pub async fn sync_awe_wisdom() -> Result<String> {
    info!(target: "4da::awe", "Syncing AWE wisdom into context system");

    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Ok("AWE binary not found. Wisdom sync skipped.".into());
    };

    let db = get_database()?;
    let mut wisdom_chunks = 0;
    let mut decisions_detected = 0;

    // 1. Get validated principles from AWE
    if let Ok(output) = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(["wisdom", "--domain", "software-engineering"]),
        30,
    ) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse principles and anti-patterns from output
        let mut current_section = "";
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.contains("VALIDATED PRINCIPLES") {
                current_section = "principle";
            } else if trimmed.contains("ANTI-PATTERNS") {
                current_section = "anti-pattern";
            } else if trimmed.starts_with('[') && !trimmed.is_empty() {
                // Extract the text after confidence bracket: "[85%] statement"
                if let Some(text) = trimmed.split(']').nth(1) {
                    let text = text.trim();
                    if !text.is_empty() {
                        let source = format!("awe://wisdom/{}", current_section);
                        let chunk_text = match current_section {
                            "principle" => {
                                format!("Validated principle from decision history: {}", text)
                            }
                            "anti-pattern" => {
                                format!("Known anti-pattern from decision history: {}", text)
                            }
                            _ => text.to_string(),
                        };

                        // Embed and store as high-weight context
                        if let Ok(embeddings) = embed_texts(&[chunk_text.clone()]).await {
                            if let Some(embedding) = embeddings.first() {
                                // Weight 1.5 = wisdom is more valuable than regular context
                                if db
                                    .upsert_context_weighted(&source, &chunk_text, embedding, 1.5)
                                    .is_ok()
                                {
                                    wisdom_chunks += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Scan git repos for new decisions
    let context_dirs = crate::get_context_dirs();
    for dir in &context_dirs {
        let dir_str = dir.to_string_lossy();
        if let Ok(output) = run_awe_with_timeout(
            std::process::Command::new(&awe_path).args([
                "scan",
                "--repo",
                &dir_str,
                "--domain",
                "software-engineering",
                "--limit",
                "50",
                "--json",
            ]),
            30,
        ) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(found) = result.get("decisions_detected").and_then(|v| v.as_u64()) {
                    decisions_detected += found;
                }
            }
        }
    }

    info!(
        target: "4da::awe",
        wisdom_chunks = wisdom_chunks,
        decisions_detected = decisions_detected,
        "AWE wisdom sync complete"
    );

    Ok(format!(
        "AWE: {} wisdom chunks indexed, {} decisions detected",
        wisdom_chunks, decisions_detected
    ))
}

/// Get AWE wisdom summary — lightweight read-only query.
/// Returns structured data about the Wisdom Graph state without syncing.
#[tauri::command]
pub async fn get_awe_summary() -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Ok(serde_json::json!({
            "available": false,
            "decisions": 0,
            "principles": 0,
            "pending": 0,
            "top_principle": null,
            "health": null,
        })
        .to_string());
    };

    let mut summary = serde_json::json!({
        "available": true,
        "decisions": 0,
        "principles": 0,
        "pending": 0,
        "top_principle": null,
        "health": null,
    });

    // Get health data
    if let Ok(output) =
        run_awe_with_timeout(std::process::Command::new(&awe_path).args(["health"]), 15)
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse "Decisions tracked: N"
        if let Some(cap) = stdout.lines().find(|l| l.contains("Decisions tracked")) {
            if let Some(num) = cap.split_whitespace().last() {
                if let Ok(n) = num.parse::<u64>() {
                    summary["decisions"] = serde_json::json!(n);
                }
            }
        }
        // Parse "Principles extracted: N"
        if let Some(cap) = stdout.lines().find(|l| l.contains("Principles extracted")) {
            if let Some(num) = cap.split_whitespace().last() {
                if let Ok(n) = num.parse::<u64>() {
                    summary["principles"] = serde_json::json!(n);
                }
            }
        }
        // Parse "Confirmed: N (X%)"
        if let Some(cap) = stdout.lines().find(|l| l.contains("Confirmed:")) {
            let parts: Vec<&str> = cap.split_whitespace().collect();
            if parts.len() >= 2 {
                summary["health"] = serde_json::json!(cap.trim());
            }
        }
    }

    // Get top principle from wisdom command
    if let Ok(output) = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(["wisdom", "-d", "software-engineering"]),
        15,
    ) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Find first principle line: "[85%] statement"
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.contains(']') {
                if let Some(text) = trimmed.split(']').nth(1) {
                    let text = text.trim();
                    if !text.is_empty()
                        && !text.starts_with("Evidence")
                        && !text.starts_with("Status")
                    {
                        summary["top_principle"] = serde_json::json!(text);
                        break;
                    }
                }
            }
        }
    }

    // Get pending count
    if let Ok(output) = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(["pending", "--limit", "100"]),
        15,
    ) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(cap) = stdout.lines().find(|l| l.contains("decision(s) need")) {
            if let Some(num) = cap.split_whitespace().next() {
                if let Ok(n) = num.parse::<u64>() {
                    summary["pending"] = serde_json::json!(n);
                }
            }
        }
    }

    Ok(summary.to_string())
}

/// Run AWE transmute and return the result as structured output.
#[tauri::command]
pub async fn run_awe_transmute(query: String, mode: String) -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Err("AWE binary not found. Build with: cargo build --release -p awe-cli".into());
    };

    let mut args: Vec<String> = vec![
        "transmute".into(),
        query.clone(),
        "--json".into(),
        "-d".into(),
        "software-engineering".into(),
    ];

    match mode.as_str() {
        "voice" => args.push("--voice".into()),
        "challenge" => args.push("--challenge".into()),
        _ => {} // structured is default
    }

    // Inject developer profile context for personalized wisdom
    let ctx_path = std::env::temp_dir().join("awe_dev_ctx.json");
    if let Ok(conn) = crate::open_db_connection() {
        let profile = crate::sovereign_developer_profile::assemble_profile(&conn);
        let dev_ctx = serde_json::json!({
            "primary_stack": profile.stack.primary_stack,
            "adjacent_tech": profile.stack.adjacent_tech,
            "domain_concerns": [],
            "infrastructure_summary": format!(
                "{} RAM, GPU: {}, {}",
                profile.infrastructure.ram.get("total").map(|s| s.as_str()).unwrap_or("unknown"),
                profile.infrastructure.gpu_tier,
                profile.infrastructure.os.get("name").map(|s| s.as_str()).unwrap_or("unknown OS"),
            ),
            "identity_summary": profile.identity_summary,
        });
        if let Ok(json) = serde_json::to_string(&dev_ctx) {
            if std::fs::write(&ctx_path, &json).is_ok() {
                args.push("--context-file".into());
                args.push(ctx_path.to_string_lossy().to_string());
            }
        }
    }

    let output = run_awe_with_timeout(std::process::Command::new(&awe_path).args(&args), 30)
        .map_err(|e| format!("Failed to run AWE: {e}"))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&ctx_path);

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the JSON output and extract wisdom
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
        // Find the Articulated stage for wisdom text
        if let Some(stages) = parsed.get("stages").and_then(|s| s.as_array()) {
            for stage in stages {
                if let Some(arr) = stage.as_array() {
                    if arr.len() >= 2 {
                        if let Some(articulated) = arr[1].get("Articulated") {
                            if let Some(wisdom) = articulated.get("wisdom").and_then(|w| w.as_str())
                            {
                                let confidence = articulated
                                    .get("confidence")
                                    .and_then(|c| c.as_f64())
                                    .unwrap_or(0.5);
                                let watch_for: Vec<String> = articulated
                                    .get("watch_for")
                                    .and_then(|w| w.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                return Ok(serde_json::json!({
                                    "wisdom": wisdom,
                                    "confidence": confidence,
                                    "watch_for": watch_for,
                                    "mode": mode,
                                })
                                .to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: return raw output
    Ok(stdout.to_string())
}

/// Quick sanity check on a decision.
#[tauri::command]
pub async fn run_awe_quick_check(query: String) -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Err("AWE binary not found".into());
    };

    let output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "transmute",
            &query,
            "--json",
            "-d",
            "software-engineering",
            "--stages",
            "receive,interrogate,articulate",
        ]),
        30,
    )
    .map_err(|e| format!("Failed to run AWE: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Model consequences of a decision.
#[tauri::command]
pub async fn run_awe_consequence_scan(query: String) -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Err("AWE binary not found".into());
    };

    let output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "transmute",
            &query,
            "--json",
            "-d",
            "software-engineering",
            "--stages",
            "receive,consequent,articulate",
        ]),
        30,
    )
    .map_err(|e| format!("Failed to run AWE: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Record outcome feedback for a previous AWE decision.
#[tauri::command]
pub async fn run_awe_feedback(
    decision_id: String,
    outcome: String,
    details: String,
) -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Err("AWE binary not found".into());
    };

    let output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "feedback",
            &decision_id,
            "--outcome",
            &outcome,
            "--details",
            &details,
        ]),
        15,
    )
    .map_err(|e| format!("Failed to run AWE: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Recall accumulated wisdom for a domain.
#[tauri::command]
pub async fn run_awe_recall(domain: String) -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Err("AWE binary not found".into());
    };

    let output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(["wisdom", "-d", &domain]),
        15,
    )
    .map_err(|e| format!("Failed to run AWE: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get calibration data for a domain.
#[tauri::command]
pub async fn run_awe_calibration(domain: String) -> Result<String> {
    let awe_bin = find_awe_binary();
    let Some(awe_path) = awe_bin else {
        return Err("AWE binary not found".into());
    };

    let output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(["calibration", "-d", &domain]),
        15,
    )
    .map_err(|e| format!("Failed to run AWE: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run an AWE subprocess with a timeout to prevent indefinite blocking.
fn run_awe_with_timeout(
    cmd: &mut std::process::Command,
    timeout_secs: u64,
) -> std::result::Result<std::process::Output, String> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start AWE: {e}"))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| format!("Failed to read AWE output: {e}"));
            }
            Ok(None) => {
                if start.elapsed().as_secs() > timeout_secs {
                    child.kill().ok();
                    return Err(format!("AWE timed out after {timeout_secs}s"));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Failed to wait for AWE: {e}")),
        }
    }
}

/// Find the AWE binary (release build).
fn find_awe_binary() -> Option<String> {
    let candidates = [
        "D:\\runyourempire\\awe\\target\\release\\awe.exe",
        "/d/runyourempire/awe/target/release/awe",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    // Check AWE_BIN env var
    std::env::var("AWE_BIN")
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
}

/// Convert Windows path to WSL path if needed (e.g., D:\projects -> /mnt/d/projects).
/// Only called at runtime on Linux (WSL); on other platforms it's used only in tests.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
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

    // Convert Windows paths to WSL paths on Linux (WSL) only; skip on native Windows
    let mut converted_dirs: Vec<String> = Vec::new();
    for dir in &dirs {
        #[cfg(target_os = "linux")]
        let converted = convert_windows_to_wsl_path(dir);
        #[cfg(not(target_os = "linux"))]
        let converted = dir.to_string();

        if converted != *dir {
            debug!(target: "4da::context", from = dir, to = %converted, "Converted Windows path");
        }

        let path = PathBuf::from(&converted);
        if !path.exists() {
            return Err(format!("Directory does not exist: {}", sanitize_path(&converted)).into());
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", sanitize_path(&converted)).into());
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

/// Generate a formatted CLI briefing string.
///
/// Pulls from in-memory analysis state first, falls back to DB query.
/// Designed for both CLI output and frontend consumption.
#[tauri::command]
pub async fn generate_cli_briefing() -> Result<String> {
    info!(target: "4da::briefing", "Generating CLI briefing");
    Ok(crate::monitoring_notifications::generate_briefing_text())
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
