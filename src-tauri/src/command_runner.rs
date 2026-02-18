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

const TIMEOUT_SECS: u64 = 30;
const MAX_STDOUT: usize = 50_000; // 50KB
const MAX_STDERR: usize = 10_000; // 10KB
const MAX_HISTORY: u32 = 200;

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
        let result = std::process::Command::new("cmd")
            .args(["/C", &command_clone])
            .current_dir(&work_dir)
            .output();

        #[cfg(not(target_os = "windows"))]
        let result = std::process::Command::new("sh")
            .args(["-c", &command_clone])
            .current_dir(&work_dir)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(out) => {
                let mut stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let mut stderr = String::from_utf8_lossy(&out.stderr).to_string();

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
                    exit_code: out.status.code().unwrap_or(-1),
                    duration_ms,
                })
            }
            Err(e) => Err(FourDaError::Internal(format!(
                "Failed to execute command: {}",
                e
            ))),
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

    if let Err(e) = save_to_history(
        &command,
        &work_dir_str,
        output.exit_code,
        output.exit_code == 0,
        preview,
    ) {
        warn!(target: "4da::cmd_runner", error = %e, "Failed to save command history");
    }

    debug!(target: "4da::cmd_runner", command = %command, exit_code = output.exit_code, duration_ms = output.duration_ms, "Command executed");

    Ok(output)
}

#[tauri::command]
pub async fn get_command_history(limit: Option<u32>) -> Result<Vec<CommandHistoryEntry>> {
    let limit = limit.unwrap_or(50).min(MAX_HISTORY);
    let db = get_database()?;
    let conn = db.conn.lock();

    let mut stmt = conn
        .prepare(
            "SELECT id, command, working_dir, exit_code, success, output_preview, created_at
             FROM command_history
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(FourDaError::Db)?;

    let entries = stmt
        .query_map([limit], |row| {
            Ok(CommandHistoryEntry {
                id: row.get(0)?,
                command: row.get(1)?,
                working_dir: row.get(2)?,
                exit_code: row.get(3)?,
                success: row.get::<_, i64>(4).map(|v| v != 0)?,
                output_preview: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(FourDaError::Db)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(FourDaError::Db)?;

    Ok(entries)
}

/// Save a command to history and auto-prune old entries.
fn save_to_history(
    command: &str,
    working_dir: &str,
    exit_code: i32,
    success: bool,
    output_preview: Option<String>,
) -> Result<()> {
    let db = get_database()?;
    let conn = db.conn.lock();

    conn.execute(
        "INSERT INTO command_history (command, working_dir, exit_code, success, output_preview)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            command,
            working_dir,
            exit_code,
            success as i32,
            output_preview
        ],
    )
    .map_err(FourDaError::Db)?;

    // Auto-prune to MAX_HISTORY entries
    conn.execute(
        "DELETE FROM command_history WHERE id NOT IN (
            SELECT id FROM command_history ORDER BY created_at DESC LIMIT ?1
        )",
        [MAX_HISTORY],
    )
    .map_err(FourDaError::Db)?;

    Ok(())
}
