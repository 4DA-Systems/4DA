//! Command Runner — Safe shell command execution for the Command Deck.
//!
//! Provides a general-purpose command runner with timeout, output caps,
//! and blocked destructive patterns. History is persisted in SQLite.

use crate::error::{FourDaError, Result};
use crate::state::get_database;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistoryEntry {
    pub id: i64,
    pub command: String,
    pub working_dir: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub output_preview: Option<String>,
    pub created_at: String,
}

// ============================================================================
// Constants
// ============================================================================

const MAX_STDOUT: usize = 50_000; // 50KB
const MAX_STDERR: usize = 10_000; // 10KB
const MAX_HISTORY: u32 = 200;
const TIMEOUT_SECS: u64 = 30;

/// Patterns that are blocked for safety.
const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "format c:",
    ":(){ :|:& };:",
    "mkfs",
    "dd if=/dev/zero",
    "> /dev/sda",
    "chmod -R 777 /",
];

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn run_shell_command(
    command: String,
    working_dir: Option<String>,
) -> Result<crate::git_deck::CommandOutput> {
    let cmd_lower = command.to_lowercase();
    for pattern in BLOCKED_PATTERNS {
        if cmd_lower.contains(pattern) {
            return Err(FourDaError::Config(format!(
                "Blocked destructive command pattern: {}",
                pattern
            )));
        }
    }

    let work_dir = working_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    if !work_dir.exists() {
        return Err(FourDaError::Config(format!(
            "Working directory does not exist: {}",
            work_dir.display()
        )));
    }

    let command_clone = command.clone();
    let work_dir_str = work_dir.to_string_lossy().to_string();

    let output = tokio::task::spawn_blocking(move || {
        let start = Instant::now();

        #[cfg(target_os = "windows")]
        let child = std::process::Command::new("cmd")
            .args(["/C", &command_clone])
            .current_dir(&work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        #[cfg(not(target_os = "windows"))]
        let child = std::process::Command::new("sh")
            .args(["-c", &command_clone])
            .current_dir(&work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let mut child = child.map_err(|e| {
            FourDaError::Internal(format!("Failed to execute command: {}", e))
        })?;

        let deadline = start + std::time::Duration::from_secs(TIMEOUT_SECS);

        // Poll try_wait until process exits or timeout
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break None; // timed out
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(FourDaError::Internal(format!(
                        "Failed waiting on command: {}", e
                    )));
                }
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match status {
            Some(exit_status) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_string(&mut stderr);
                }

                if stdout.len() > MAX_STDOUT {
                    stdout.truncate(MAX_STDOUT);
                    stdout.push_str("\n...(output truncated)");
                }
                if stderr.len() > MAX_STDERR {
                    stderr.truncate(MAX_STDERR);
                    stderr.push_str("\n...(output truncated)");
                }

                Ok(crate::git_deck::CommandOutput {
                    stdout,
                    stderr,
                    exit_code: exit_status.code().unwrap_or(-1),
                    duration_ms,
                })
            }
            None => {
                // Timeout — kill the process
                let _ = child.kill();
                let _ = child.wait();
                warn!(target: "4da::cmd_runner", timeout_secs = TIMEOUT_SECS, "Command timed out, killed");
                Ok(crate::git_deck::CommandOutput {
                    stdout: String::new(),
                    stderr: format!("Command timed out after {}s and was killed", TIMEOUT_SECS),
                    exit_code: -1,
                    duration_ms,
                })
            }
        }
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))??;

    // Save to history (non-fatal)
    let preview = if output.stdout.len() > 200 {
        Some(format!("{}...", &output.stdout[..200]))
    } else if !output.stdout.is_empty() {
        Some(output.stdout.clone())
    } else {
        None
    };

    if let Ok(db) = get_database() {
        if let Err(e) = db.save_command_history(
            &command,
            &work_dir_str,
            output.exit_code,
            output.exit_code == 0,
            preview.as_deref(),
        ) {
            warn!(target: "4da::cmd_runner", error = %e, "Failed to save command history");
        }
    }

    debug!(target: "4da::cmd_runner", command = %command, exit_code = output.exit_code, duration_ms = output.duration_ms, "Command executed");

    Ok(output)
}

#[tauri::command]
pub async fn get_command_history(limit: Option<u32>) -> Result<Vec<CommandHistoryEntry>> {
    let limit = limit.unwrap_or(50).min(MAX_HISTORY);
    let db = get_database()?;

    let rows = db.get_command_history(limit).map_err(FourDaError::Db)?;

    Ok(rows
        .into_iter()
        .map(|r| CommandHistoryEntry {
            id: r.id,
            command: r.command,
            working_dir: r.working_dir,
            exit_code: r.exit_code,
            success: r.success,
            output_preview: r.output_preview,
            created_at: r.created_at,
        })
        .collect())
}
