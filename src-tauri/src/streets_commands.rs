//! STREETS Command Parser & Executor — transforms playbook code blocks into runnable commands.
//!
//! Parses markdown code blocks from STREETS lessons, classifies commands by OS target
//! and risk level, and executes them safely using the same pattern as command_runner.rs.

use crate::error::{FourDaError, Result};
use crate::playbook_commands;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCommand {
    pub id: String,
    pub command: String,
    pub os_target: OsTarget,
    pub language: String,
    pub risk_level: RiskLevel,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OsTarget {
    Linux,
    MacOs,
    Windows,
    Universal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Safe,
    Moderate,
    Elevated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecutionResult {
    pub command_id: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub executed_at: String,
}

// ============================================================================
// Constants
// ============================================================================

const MAX_STDOUT: usize = 50_000;
const MAX_STDERR: usize = 10_000;
const TIMEOUT_SECS: u64 = 60; // Longer than command_runner — installs take time

/// Patterns blocked for safety (same as command_runner.rs).
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

/// Commands that only read system state — always safe.
const SAFE_COMMANDS: &[&str] = &[
    "grep",
    "cat",
    "lscpu",
    "free",
    "nvidia-smi",
    "sysctl",
    "get-ciminstance",
    "df",
    "nproc",
    "system_profiler",
    "speedtest",
    "ping",
    "ollama --version",
    "ollama list",
    "echo",
    "whoami",
    "hostname",
    "uname",
    "wmic",
    "systeminfo",
    "lsblk",
    "ip addr",
    "ifconfig",
    "rocm-smi",
    "head",
    "get-psdrive",
    "spdisplaysdatatype",
];

/// Commands that install or download — moderate risk.
const MODERATE_COMMANDS: &[&str] = &[
    "curl",
    "brew install",
    "pip install",
    "winget install",
    "ollama pull",
    "ollama serve",
    "npm install",
    "cargo install",
    "apt install",
    "choco install",
    "wget",
];

// ============================================================================
// Risk Classification
// ============================================================================

fn classify_risk(command: &str) -> RiskLevel {
    let lower = command.to_lowercase();

    // Check safe patterns first (read-only commands)
    for pattern in SAFE_COMMANDS {
        if lower.starts_with(pattern) || lower.contains(&format!("| {}", pattern)) {
            return RiskLevel::Safe;
        }
    }

    // Check moderate patterns (install/download)
    for pattern in MODERATE_COMMANDS {
        if lower.contains(pattern) {
            return RiskLevel::Moderate;
        }
    }

    // Everything else is elevated
    RiskLevel::Elevated
}

// ============================================================================
// OS Detection
// ============================================================================

fn detect_os_from_comment(line: &str) -> Option<OsTarget> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let comment = trimmed.to_lowercase();

    if comment.contains("windows") || comment.contains("powershell") {
        Some(OsTarget::Windows)
    } else if comment.contains("linux/mac") || comment.contains("linux") {
        // Check "linux/mac" before standalone "mac" so "# Linux/Mac" maps to Linux
        Some(OsTarget::Linux)
    } else if comment.contains("macos") || (comment.contains("mac") && !comment.contains("machine"))
    {
        Some(OsTarget::MacOs)
    } else if comment.contains("nvidia") || comment.contains("amd") {
        Some(OsTarget::Universal)
    } else {
        None
    }
}

fn current_os_target() -> OsTarget {
    match std::env::consts::OS {
        "windows" => OsTarget::Windows,
        "macos" => OsTarget::MacOs,
        "linux" => OsTarget::Linux,
        _ => OsTarget::Universal,
    }
}

// ============================================================================
// Code Block Parsing
// ============================================================================

fn parse_code_blocks(content: &str, module_id: &str, lesson_idx: usize) -> Vec<ParsedCommand> {
    let mut commands = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut block_idx = 0;

    while i < lines.len() {
        let line = lines[i];

        // Detect code block start: ```bash, ```shell, ```powershell, ```sh
        if line.starts_with("```") {
            let lang_tag = line.trim_start_matches('`').trim().to_lowercase();
            let is_executable = matches!(lang_tag.as_str(), "bash" | "shell" | "sh" | "powershell");

            if is_executable {
                let language = lang_tag.clone();
                i += 1;

                // Default OS target: powershell implies Windows, others are universal
                let default_os = if language == "powershell" {
                    OsTarget::Windows
                } else {
                    OsTarget::Universal
                };

                let mut current_os = default_os.clone();
                let mut cmd_idx = 0;

                while i < lines.len() && !lines[i].starts_with("```") {
                    let code_line = lines[i];
                    let trimmed = code_line.trim();

                    // Skip empty lines
                    if trimmed.is_empty() {
                        i += 1;
                        continue;
                    }

                    // Check if this is an OS comment marker
                    if let Some(os) = detect_os_from_comment(trimmed) {
                        current_os = os;
                        i += 1;
                        continue;
                    }

                    // Skip pure comment lines (not OS markers)
                    if trimmed.starts_with('#') && detect_os_from_comment(trimmed).is_none() {
                        i += 1;
                        continue;
                    }

                    // This is an actual command
                    let command_text = trimmed.to_string();
                    let risk = classify_risk(&command_text);
                    let id = format!("{}-L{}-B{}-C{}", module_id, lesson_idx, block_idx, cmd_idx);

                    // Build a short description from the command
                    let description = if command_text.len() > 60 {
                        format!("{}...", &command_text[..57])
                    } else {
                        command_text.clone()
                    };

                    commands.push(ParsedCommand {
                        id,
                        command: command_text,
                        os_target: current_os.clone(),
                        language: language.clone(),
                        risk_level: risk,
                        description,
                    });

                    cmd_idx += 1;
                    i += 1;
                }
                block_idx += 1;
            }
            // Skip to closing ``` for non-executable blocks
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                i += 1;
            }
        }
        i += 1;
    }

    commands
}

// ============================================================================
// Command Execution (mirrors command_runner.rs pattern)
// ============================================================================

fn execute_command_blocking(command: &str) -> Result<CommandExecutionResult> {
    let start = Instant::now();
    let command_str = command.to_string();

    #[cfg(target_os = "windows")]
    let child = std::process::Command::new("cmd")
        .args(["/C", &command_str])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    #[cfg(not(target_os = "windows"))]
    let child = std::process::Command::new("sh")
        .args(["-c", &command_str])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child =
        child.map_err(|e| FourDaError::Internal(format!("Failed to execute command: {}", e)))?;

    let deadline = start + std::time::Duration::from_secs(TIMEOUT_SECS);

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    break None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(FourDaError::Internal(format!(
                    "Failed waiting on command: {}",
                    e
                )));
            }
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let executed_at = chrono_now_iso();

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

            let exit_code = exit_status.code().unwrap_or(-1);
            Ok(CommandExecutionResult {
                command_id: String::new(), // Filled by caller
                success: exit_code == 0,
                stdout,
                stderr,
                exit_code,
                duration_ms,
                executed_at,
            })
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            warn!(target: "4da::streets_cmd", timeout_secs = TIMEOUT_SECS, "Command timed out");
            Ok(CommandExecutionResult {
                command_id: String::new(),
                success: false,
                stdout: String::new(),
                stderr: format!("Command timed out after {}s and was killed", TIMEOUT_SECS),
                exit_code: -1,
                duration_ms,
                executed_at,
            })
        }
    }
}

/// Simple ISO-ish timestamp without pulling in chrono crate.
fn chrono_now_iso() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", now.as_secs())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn parse_lesson_commands(
    module_id: String,
    lesson_idx: usize,
) -> Result<Vec<ParsedCommand>> {
    let content_dir = playbook_commands::get_content_dir();
    let filename = playbook_commands::module_id_to_filename(&module_id)
        .ok_or_else(|| FourDaError::Config(format!("Unknown module: {}", module_id)))?;
    let path = content_dir.join(filename);

    if !path.exists() {
        return Err(FourDaError::Config(format!(
            "Module file not found: {}",
            path.display()
        )));
    }

    let raw = std::fs::read_to_string(&path).map_err(FourDaError::Io)?;
    let lessons = playbook_commands::parse_lessons(&raw);

    if lesson_idx >= lessons.len() {
        return Err(FourDaError::Config(format!(
            "Lesson index {} out of range (module has {} lessons)",
            lesson_idx,
            lessons.len()
        )));
    }

    let lesson = &lessons[lesson_idx];
    let commands = parse_code_blocks(&lesson.content, &module_id, lesson_idx);
    debug!(target: "4da::streets_cmd", module = %module_id, lesson = lesson_idx, count = commands.len(), "Parsed commands");

    Ok(commands)
}

#[tauri::command]
pub async fn execute_streets_command(
    command_id: String,
    command: String,
    risk_level: RiskLevel,
) -> Result<CommandExecutionResult> {
    // Validate against blocked patterns
    let cmd_lower = command.to_lowercase();
    for pattern in BLOCKED_PATTERNS {
        if cmd_lower.contains(pattern) {
            return Err(FourDaError::Config(format!(
                "Blocked destructive command pattern: {}",
                pattern
            )));
        }
    }

    info!(target: "4da::streets_cmd", id = %command_id, risk = ?risk_level, "Executing STREETS command");

    let cmd_clone = command.clone();
    let id_clone = command_id.clone();

    let mut result = tokio::task::spawn_blocking(move || execute_command_blocking(&cmd_clone))
        .await
        .map_err(|e| FourDaError::Internal(format!("Task join error: {}", e)))??;

    result.command_id = id_clone;

    // Extract module_id and lesson_idx from the command_id pattern: "MODULE-LINDEX-BBLOCK-CCMD"
    let parts: Vec<&str> = command_id.split('-').collect();
    let module_id_str = parts.first().copied().unwrap_or("?");
    let lesson_idx_val: usize = parts
        .get(1)
        .and_then(|p| p.strip_prefix('L'))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Log execution to command_execution_log (non-fatal)
    crate::sovereign_profile::log_command_execution(
        module_id_str,
        lesson_idx_val,
        &command_id,
        &command,
        result.success,
        result.exit_code,
        &result.stdout,
        &result.stderr,
        result.duration_ms,
    );

    // Extract and store sovereign profile facts from successful commands
    if result.success {
        crate::sovereign_profile::store_facts_from_execution(
            &command,
            &result.stdout,
            &format!("{}:L{}", module_id_str, lesson_idx_val),
        );
    }

    debug!(target: "4da::streets_cmd",
        id = %command_id,
        success = result.success,
        exit_code = result.exit_code,
        duration_ms = result.duration_ms,
        "Command completed"
    );

    Ok(result)
}

#[tauri::command]
pub async fn execute_lesson_commands(
    module_id: String,
    lesson_idx: usize,
    max_risk: RiskLevel,
) -> Result<Vec<CommandExecutionResult>> {
    let all_commands = parse_lesson_commands(module_id.clone(), lesson_idx).await?;

    let runtime_os = current_os_target();
    let filtered: Vec<&ParsedCommand> = all_commands
        .iter()
        .filter(|cmd| {
            // Filter by OS: match current OS or Universal
            let os_match = cmd.os_target == runtime_os || cmd.os_target == OsTarget::Universal;
            // Filter by risk: only execute up to max_risk level
            let risk_ok = cmd.risk_level <= max_risk;
            os_match && risk_ok
        })
        .collect();

    info!(target: "4da::streets_cmd",
        module = %module_id,
        lesson = lesson_idx,
        total = all_commands.len(),
        filtered = filtered.len(),
        max_risk = ?max_risk,
        "Executing filtered lesson commands"
    );

    let mut results = Vec::new();
    for cmd in filtered {
        let result =
            execute_streets_command(cmd.id.clone(), cmd.command.clone(), cmd.risk_level.clone())
                .await?;

        let success = result.success;
        results.push(result);

        // Stop on failure
        if !success {
            warn!(target: "4da::streets_cmd", id = %cmd.id, "Command failed, stopping sequence");
            break;
        }
    }

    Ok(results)
}

#[cfg(test)]
#[path = "streets_commands_tests.rs"]
mod tests;
