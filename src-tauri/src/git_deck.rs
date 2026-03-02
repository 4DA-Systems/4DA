//! Git Deck — Interactive git operations for the Command Deck panel.
//!
//! Provides Tauri commands for git status, staging, commits, push, and
//! AI-powered commit message suggestions. All operations route through
//! a validated `run_git` helper with path traversal protection.

use crate::ace::git::GitAnalyzer;
use crate::error::{FourDaError, Result};
use crate::state::{get_context_dirs, get_settings_manager};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDeckStatus {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub untracked: Vec<String>,
    pub has_conflicts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub status: String, // "M", "A", "D", "R", "C", "U"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitResult {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub files_changed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub files_changed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedCommitMessage {
    pub message: String,
    pub summary: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub branch: Option<String>,
    pub has_changes: bool,
}

// ============================================================================
// Private Helpers
// ============================================================================

/// Validate a repo path: must exist, no traversal, must have .git
fn validate_repo_path(repo_path: &str) -> Result<PathBuf> {
    if repo_path.contains("..") {
        return Err(FourDaError::Config(
            "Path traversal not allowed".to_string(),
        ));
    }
    let path = PathBuf::from(repo_path);
    if !path.exists() {
        return Err(FourDaError::Config(format!(
            "Path does not exist: {}",
            repo_path
        )));
    }
    if !path.join(".git").exists() {
        return Err(FourDaError::Config(format!(
            "Not a git repository: {}",
            repo_path
        )));
    }
    Ok(path)
}

/// Run a git command in a validated repo directory.
fn run_git(repo_path: &Path, args: &[&str]) -> Result<CommandOutput> {
    let start = Instant::now();
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| FourDaError::Internal(format!("Failed to run git: {}", e)))?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    debug!(target: "4da::git_deck", cmd = %args.join(" "), exit_code, duration_ms, "git command");

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code,
        duration_ms,
    })
}

/// Parse porcelain v2 status output into GitDeckStatus.
fn parse_porcelain_v2(output: &str) -> GitDeckStatus {
    let mut branch = String::from("HEAD");
    let mut ahead: u32 = 0;
    let mut behind: u32 = 0;
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut has_conflicts = false;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            // Format: +N -M
            for part in rest.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    behind = n.parse().unwrap_or(0);
                }
            }
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            // Ordinary/rename entries: "1 XY ..." or "2 XY ..."
            let parts: Vec<&str> = line.splitn(9, ' ').collect();
            if parts.len() >= 9 {
                let xy = parts[1];
                let path = parts[8];
                let x = xy.chars().next().unwrap_or('.');
                let y = xy.chars().nth(1).unwrap_or('.');

                if x != '.' && x != '?' {
                    staged.push(FileChange {
                        path: path.to_string(),
                        status: x.to_string(),
                    });
                }
                if y != '.' && y != '?' {
                    unstaged.push(FileChange {
                        path: path.to_string(),
                        status: y.to_string(),
                    });
                }
            }
        } else if line.starts_with("? ") {
            // Untracked: "? path"
            if let Some(path) = line.strip_prefix("? ") {
                untracked.push(path.to_string());
            }
        } else if line.starts_with("u ") {
            // Unmerged (conflict)
            has_conflicts = true;
            let parts: Vec<&str> = line.splitn(11, ' ').collect();
            if let Some(path) = parts.last() {
                unstaged.push(FileChange {
                    path: path.to_string(),
                    status: "U".to_string(),
                });
            }
        }
    }

    GitDeckStatus {
        branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        has_conflicts,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn git_deck_status(repo_path: String) -> Result<GitDeckStatus> {
    let path = validate_repo_path(&repo_path)?;
    let output = tokio::task::spawn_blocking(move || {
        run_git(&path, &["status", "--porcelain=v2", "--branch"])
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))??;

    if output.exit_code != 0 {
        return Err(FourDaError::Internal(format!(
            "git status failed: {}",
            output.stderr
        )));
    }

    Ok(parse_porcelain_v2(&output.stdout))
}

#[tauri::command]
pub async fn git_deck_stage(repo_path: String, paths: Vec<String>) -> Result<CommandOutput> {
    let path = validate_repo_path(&repo_path)?;
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["add", "--"];
        let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        args.extend(path_refs);
        let output = run_git(&path, &args)?;
        if output.exit_code != 0 {
            return Err(FourDaError::Internal(format!(
                "git add failed: {}",
                output.stderr
            )));
        }
        Ok(output)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn git_deck_unstage(repo_path: String, paths: Vec<String>) -> Result<CommandOutput> {
    let path = validate_repo_path(&repo_path)?;
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["restore", "--staged", "--"];
        let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        args.extend(path_refs);
        let output = run_git(&path, &args)?;
        if output.exit_code != 0 {
            return Err(FourDaError::Internal(format!(
                "git restore --staged failed: {}",
                output.stderr
            )));
        }
        Ok(output)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn git_deck_commit(repo_path: String, message: String) -> Result<CommitResult> {
    if message.trim().is_empty() {
        return Err(FourDaError::Config(
            "Commit message cannot be empty".to_string(),
        ));
    }
    let path = validate_repo_path(&repo_path)?;
    tokio::task::spawn_blocking(move || {
        let output = run_git(&path, &["commit", "-m", &message])?;
        if output.exit_code != 0 {
            return Err(FourDaError::Internal(format!(
                "git commit failed: {}",
                output.stderr
            )));
        }

        // Parse commit hash from output
        let hash_output = run_git(&path, &["rev-parse", "HEAD"])?;
        let hash = hash_output.stdout.trim().to_string();
        let short_hash = hash.get(..7).unwrap_or(&hash).to_string();

        // Count files changed from commit output
        let files_changed = output
            .stdout
            .lines()
            .find(|l| l.contains("file") && l.contains("changed"))
            .and_then(|l| l.split_whitespace().next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);

        Ok(CommitResult {
            hash,
            short_hash,
            message,
            files_changed,
        })
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn git_deck_push(repo_path: String, branch: Option<String>) -> Result<CommandOutput> {
    let path = validate_repo_path(&repo_path)?;
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["push"];
        if let Some(ref b) = branch {
            args.push("origin");
            args.push(b);
        }
        let output = run_git(&path, &args)?;
        if output.exit_code != 0 {
            return Err(FourDaError::Internal(format!(
                "git push failed: {}",
                output.stderr
            )));
        }
        Ok(output)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn git_deck_diff_stat(repo_path: String, staged: bool) -> Result<String> {
    let path = validate_repo_path(&repo_path)?;
    tokio::task::spawn_blocking(move || {
        let args = if staged {
            vec!["diff", "--cached", "--stat"]
        } else {
            vec!["diff", "--stat"]
        };
        let output = run_git(&path, &args)?;
        Ok(output.stdout)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn git_deck_log(repo_path: String, count: u32) -> Result<Vec<CommitSummary>> {
    let path = validate_repo_path(&repo_path)?;
    let count = count.min(50); // Cap at 50
    tokio::task::spawn_blocking(move || {
        let count_str = format!("-{}", count);
        let output = run_git(
            &path,
            &[
                "log",
                &count_str,
                "--pretty=format:%H|%h|%s|%an|%ai",
                "--shortstat",
            ],
        )?;

        let mut commits = Vec::new();
        let mut current: Option<CommitSummary> = None;

        for line in output.stdout.lines() {
            if line.contains('|') && line.len() > 40 {
                if let Some(c) = current.take() {
                    commits.push(c);
                }
                let parts: Vec<&str> = line.splitn(5, '|').collect();
                if parts.len() >= 5 {
                    current = Some(CommitSummary {
                        hash: parts[0].to_string(),
                        short_hash: parts[1].to_string(),
                        message: parts[2].to_string(),
                        author: parts[3].to_string(),
                        date: parts[4].to_string(),
                        files_changed: 0,
                    });
                }
            } else if line.contains("file") && line.contains("changed") {
                if let Some(ref mut c) = current {
                    c.files_changed = line
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                }
            }
        }
        if let Some(c) = current {
            commits.push(c);
        }

        Ok(commits)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn git_deck_suggest_commit(repo_path: String) -> Result<SuggestedCommitMessage> {
    let path = validate_repo_path(&repo_path)?;

    // Get the staged diff
    let diff_output = tokio::task::spawn_blocking({
        let path = path.clone();
        move || run_git(&path, &["diff", "--cached", "--stat"])
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))??;

    let diff_text = diff_output.stdout.trim().to_string();
    if diff_text.is_empty() {
        return Err(FourDaError::Config(
            "No staged changes to describe".to_string(),
        ));
    }

    // Also get the detailed diff (truncated for token limits)
    let detailed = tokio::task::spawn_blocking({
        let path = path.clone();
        move || run_git(&path, &["diff", "--cached"])
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))??;

    let detail_truncated = if detailed.stdout.len() > 4000 {
        format!("{}...(truncated)", &detailed.stdout[..4000])
    } else {
        detailed.stdout.clone()
    };

    // Try LLM suggestion
    let provider = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    if provider.api_key.is_empty() && provider.provider != "ollama" {
        // No LLM configured — generate a simple heuristic message
        return Ok(heuristic_commit_message(&diff_text));
    }

    let client = crate::llm::LLMClient::new(provider.clone());
    let system = "You are a git commit message generator. Given a diff, write a concise conventional commit message (type: description). Keep it under 72 characters. Reply with ONLY the commit message, nothing else.";
    let user_msg = format!(
        "Generate a commit message for this diff:\n\n{}\n\nDetailed changes:\n{}",
        diff_text, detail_truncated
    );

    match client
        .complete(
            system,
            vec![crate::llm::Message {
                role: "user".to_string(),
                content: user_msg,
            }],
        )
        .await
    {
        Ok(response) => {
            let message = response.content.trim().to_string();
            Ok(SuggestedCommitMessage {
                message: message.clone(),
                summary: diff_text,
                model: Some(provider.model),
            })
        }
        Err(e) => {
            warn!(target: "4da::git_deck", error = %e, "LLM commit suggestion failed, using heuristic");
            Ok(heuristic_commit_message(&diff_text))
        }
    }
}

/// Simple heuristic commit message when LLM is unavailable.
fn heuristic_commit_message(stat: &str) -> SuggestedCommitMessage {
    let lines: Vec<&str> = stat.lines().collect();
    let file_count = lines.len().saturating_sub(1); // Last line is summary
    let message = if file_count == 1 {
        let file = lines
            .first()
            .and_then(|l| l.split('|').next())
            .map(|f| f.trim())
            .unwrap_or("file");
        format!("chore: update {}", file)
    } else {
        format!("chore: update {} files", file_count)
    };

    SuggestedCommitMessage {
        message,
        summary: stat.to_string(),
        model: None,
    }
}

#[tauri::command]
pub async fn git_deck_list_repos() -> Result<Vec<RepoInfo>> {
    let context_dirs = get_context_dirs();
    if context_dirs.is_empty() {
        return Ok(Vec::new());
    }

    tokio::task::spawn_blocking(move || {
        let analyzer = GitAnalyzer::default();
        let mut repos = Vec::new();

        for dir in &context_dirs {
            // Check if the dir itself is a repo
            if GitAnalyzer::is_git_repo(dir) {
                if let Some(info) = repo_info(dir) {
                    repos.push(info);
                }
                continue;
            }
            // Otherwise search for repos (max depth 3)
            for repo_path in analyzer.find_repos(dir, 3) {
                if let Some(info) = repo_info(&repo_path) {
                    repos.push(info);
                }
            }
        }

        // Deduplicate by path
        repos.sort_by(|a, b| a.path.cmp(&b.path));
        repos.dedup_by(|a, b| a.path == b.path);

        Ok(repos)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))?
}

/// Build a RepoInfo for a single repo path.
fn repo_info(path: &Path) -> Option<RepoInfo> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(path)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|b| !b.is_empty());

    let has_changes = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    Some(RepoInfo {
        path: path.to_string_lossy().to_string(),
        name,
        branch,
        has_changes,
    })
}

#[cfg(test)]
#[path = "git_deck_tests.rs"]
mod tests;
