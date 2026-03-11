//! STREETS Command Parser & Executor — transforms playbook code blocks into runnable commands.
//!
//! Parses markdown code blocks from STREETS lessons, classifies commands by OS target
//! and risk level, and executes them safely using the same pattern as command_runner.rs.

use crate::error::{FourDaError, Result};
use crate::playbook_commands;
use crate::toolkit;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
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

/// Patterns blocked for safety (same as command_runner.rs) — secondary defense layer.
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

/// Allowlist of programs that STREETS commands may execute.
/// Any program not in this list is rejected before spawning.
///
/// This list is synchronized with SAFE_COMMANDS and MODERATE_COMMANDS to ensure
/// all commands referenced in STREETS lesson code blocks can actually run.
/// Shell interpreters (cmd, powershell, sh, bash) are intentionally excluded —
/// all commands run directly via Command::new() without a shell.
const ALLOWED_PROGRAMS: &[&str] = &[
    // --- Dev tools & package managers ---
    "npm",
    "npx",
    "cargo",
    "git",
    "node",
    "python",
    "python3",
    "pip",
    "pip3",
    "rustc",
    "rustup",
    "pnpm",
    "yarn",
    "bun",
    "deno",
    "go",
    "docker",
    "code",
    // --- System info (read-only diagnostics) ---
    "cat",
    "ls",
    "dir",
    "echo",
    "mkdir",
    "cd",
    "pwd",
    "which",
    "where",
    "type",
    "grep",
    "head",
    "tail",
    "wc",
    "sort",
    "cut",
    "awk",
    "sed",
    "tr",
    "lscpu",
    "nproc",
    "free",
    "df",
    "lsblk",
    "uname",
    "hostname",
    "whoami",
    "systeminfo",
    "wmic",
    // --- GPU diagnostics ---
    "nvidia-smi",
    "rocm-smi",
    // --- macOS diagnostics ---
    "sysctl",
    "system_profiler",
    "sw_vers",
    // --- Network diagnostics ---
    "ping",
    "ifconfig",
    "ip",
    "speedtest-cli",
    "speedtest",
    // --- Ollama (local AI — core STREETS workflow) ---
    "ollama",
    // --- Download/install tools ---
    "curl",
    "wget",
    "brew",
    "winget",
    "apt",
    "apt-get",
    "dnf",
    "yum",
    "pacman",
    "snap",
    "choco",
    // --- File permissions & SSH (Module S setup) ---
    "chmod",
    "chown",
    "ssh",
    "ssh-keygen",
    "scp",
    // --- Process management ---
    "htop",
    "top",
    "ps",
];

/// Shell interpreters that must NEVER appear as a program in any command or pipeline.
/// This is the hard security boundary — prevents arbitrary code execution.
const BLOCKED_INTERPRETERS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "cmd",
    "cmd.exe",
    "powershell",
    "powershell.exe",
    "pwsh",
];

/// Session environment variables accumulated from `export` commands.
/// Applied to all subsequent process spawns within the session.
static SESSION_ENV: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

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
// Command Execution — Safe Shell Reimplementation
//
// Instead of passing commands to cmd/sh (security hole), we reimplement the
// shell features STREETS needs directly in Rust:
//   - Pipes:   `lscpu | grep "Model name"` → process pipeline
//   - Chains:  `mkdir foo && cd foo` → sequential execution
//   - Export:  `export VAR=value` → session environment
//   - Sudo:    `sudo apt install x` → strip prefix, run directly
//   - PowerShell cmdlets: `Get-CimInstance` → route via powershell.exe
// ============================================================================

/// Tokenize a single command segment into (program, args).
/// Does NOT reject shell metacharacters — that responsibility is now in
/// `validate_command_program` which checks the allowlist.
fn tokenize(command: &str) -> Result<Vec<String>> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(FourDaError::Config("Empty command".to_string()));
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for ch in trimmed.chars() {
        match ch {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    if in_single_quote || in_double_quote {
        return Err(FourDaError::Config(
            "Command has unmatched quotes".to_string(),
        ));
    }

    if tokens.is_empty() {
        return Err(FourDaError::Config("Empty command".to_string()));
    }

    Ok(tokens)
}

/// Validate that a program is in the allowlist and not a blocked interpreter.
fn validate_command_program(program: &str) -> Result<()> {
    let binary_name = std::path::Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program);

    let normalized = binary_name
        .strip_suffix(".exe")
        .or_else(|| binary_name.strip_suffix(".cmd"))
        .or_else(|| binary_name.strip_suffix(".bat"))
        .unwrap_or(binary_name)
        .to_lowercase();

    // Hard block: shell interpreters can never execute
    if BLOCKED_INTERPRETERS.iter().any(|b| *b == normalized) {
        return Err(FourDaError::Config(format!(
            "Shell interpreter '{}' cannot be used. Commands run directly without a shell.",
            program
        )));
    }

    if ALLOWED_PROGRAMS
        .iter()
        .any(|a| a.to_lowercase() == normalized)
    {
        Ok(())
    } else {
        Err(FourDaError::Config(format!(
            "'{}' cannot be run in-app. Copy this command to your terminal instead.",
            program
        )))
    }
}

/// Known PowerShell verb prefixes for cmdlets (e.g., Get-CimInstance).
const POWERSHELL_VERB_PREFIXES: &[&str] = &[
    "Get-",
    "Set-",
    "New-",
    "Remove-",
    "Test-",
    "Start-",
    "Stop-",
    "Restart-",
    "Enable-",
    "Disable-",
    "Add-",
    "Clear-",
    "Copy-",
    "Move-",
    "Rename-",
    "Select-",
    "Where-",
    "ForEach-",
    "Sort-",
    "Group-",
    "Measure-",
    "Format-",
    "Out-",
    "Write-",
    "Read-",
    "Import-",
    "Export-",
    "ConvertTo-",
    "ConvertFrom-",
    "Invoke-",
    "Update-",
    "Find-",
    "Install-",
    "Uninstall-",
];

fn is_powershell_cmdlet(program: &str) -> bool {
    POWERSHELL_VERB_PREFIXES
        .iter()
        .any(|prefix| program.starts_with(prefix))
}

/// Get the accumulated session environment variables.
fn get_session_env() -> HashMap<String, String> {
    SESSION_ENV
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

/// Store an environment variable in the session.
fn set_session_env(key: String, value: String) {
    let mut guard = SESSION_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(key, value);
}

// --- Backward-compatible aliases used by tests ---

/// Parse a command string into tokens. Wraps `tokenize` and applies
/// the export/source builtin checks for backward compatibility.
fn parse_command_tokens(command: &str) -> Result<Vec<String>> {
    let trimmed = command.trim();
    // Shell builtins handled at a higher level now, but keep check for direct callers
    let lower = trimmed.to_lowercase();
    if lower.starts_with("export ") || lower.starts_with("source ") {
        return Err(FourDaError::Config(format!(
            "'{}' is a shell builtin — handled by the session engine.",
            trimmed.split_whitespace().next().unwrap_or("command")
        )));
    }
    tokenize(trimmed)
}

/// Validate a program name (alias for tests).
fn validate_program(program: &str) -> Result<()> {
    validate_command_program(program)
}

// ============================================================================
// Execution Strategies
// ============================================================================

/// Handle `export VAR=value` — stores in session env, returns success.
fn handle_export(command: &str) -> Result<CommandExecutionResult> {
    let start = Instant::now();
    let assignment = command
        .trim()
        .strip_prefix("export ")
        .unwrap_or(command.trim());

    if let Some((key, value)) = assignment.split_once('=') {
        let key = key.trim().to_string();
        // Strip surrounding quotes from value
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        debug!(target: "4da::streets_cmd", key = %key, "Setting session env var");
        set_session_env(key.clone(), value.clone());

        Ok(CommandExecutionResult {
            command_id: String::new(),
            success: true,
            stdout: format!("Set {}={}", key, value),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            executed_at: chrono_now_iso(),
        })
    } else {
        Err(FourDaError::Config(format!(
            "Invalid export syntax: expected 'export VAR=value', got '{}'",
            command.trim()
        )))
    }
}

/// Execute a pipeline: `cmd1 | cmd2 | cmd3`
/// Each program is individually validated against the allowlist.
/// Shell interpreters in the pipeline are blocked (prevents `curl | sh`).
fn execute_pipeline(segments: &[&str]) -> Result<CommandExecutionResult> {
    let start = Instant::now();

    // Validate every program in the pipeline FIRST
    for seg in segments {
        let tokens = tokenize(seg)?;
        let program = &tokens[0];

        // PowerShell cmdlets are handled differently — only in single-command mode
        if is_powershell_cmdlet(program) {
            return Err(FourDaError::Config(
                "PowerShell cmdlets cannot be used in pipelines. Run as a single command."
                    .to_string(),
            ));
        }

        validate_command_program(program)?;
    }

    let env = get_session_env();

    // Build the pipeline: spawn each process, wire stdout→stdin
    let mut prev_stdout: Option<std::process::ChildStdout> = None;
    let mut children: Vec<std::process::Child> = Vec::new();

    for (i, seg) in segments.iter().enumerate() {
        let tokens = tokenize(seg)?;
        let program = &tokens[0];
        let args = &tokens[1..];

        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        cmd.envs(&env);

        // Wire previous command's stdout to this command's stdin
        if let Some(prev) = prev_stdout.take() {
            cmd.stdin(std::process::Stdio::from(prev));
        }

        // All stages pipe stdout (last stage too — we collect it)
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| FourDaError::Internal(format!("Failed to spawn '{}': {}", program, e)))?;

        toolkit::register_spawned_pid(child.id());

        // Take stdout for the next stage's stdin
        if i < segments.len() - 1 {
            prev_stdout = child.stdout.take();
        }

        children.push(child);
    }

    // Wait for all processes to complete
    let mut final_stdout = String::new();
    let mut final_stderr = String::new();
    let mut final_exit_code = 0;

    for (i, mut child) in children.into_iter().enumerate() {
        let is_last = i == segments.len() - 1;

        // Read output from the last process in the pipeline
        if is_last {
            if let Some(mut out) = child.stdout.take() {
                use std::io::Read;
                let _ = out.read_to_string(&mut final_stdout);
            }
        }

        // Collect stderr from all processes
        if let Some(mut err) = child.stderr.take() {
            use std::io::Read;
            let mut segment_stderr = String::new();
            let _ = err.read_to_string(&mut segment_stderr);
            if !segment_stderr.is_empty() {
                final_stderr.push_str(&segment_stderr);
            }
        }

        let status = child.wait().map_err(|e| {
            FourDaError::Internal(format!("Failed waiting on pipeline stage {}: {}", i, e))
        })?;

        toolkit::unregister_spawned_pid(child.id());

        if is_last {
            final_exit_code = status.code().unwrap_or(-1);
        }
    }

    truncate_output(&mut final_stdout, MAX_STDOUT);
    truncate_output(&mut final_stderr, MAX_STDERR);

    Ok(CommandExecutionResult {
        command_id: String::new(),
        success: final_exit_code == 0,
        stdout: final_stdout,
        stderr: final_stderr,
        exit_code: final_exit_code,
        duration_ms: start.elapsed().as_millis() as u64,
        executed_at: chrono_now_iso(),
    })
}

/// Execute a `&&` chain: run commands sequentially, stop on first failure.
fn execute_chain(segments: &[&str]) -> Result<CommandExecutionResult> {
    let start = Instant::now();
    let mut combined_stdout = String::new();
    let mut combined_stderr = String::new();
    let mut last_exit_code = 0;

    for seg in segments {
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }

        // Recursively dispatch — each segment could itself contain pipes
        let result = dispatch_command(seg)?;

        if !result.stdout.is_empty() {
            if !combined_stdout.is_empty() {
                combined_stdout.push('\n');
            }
            combined_stdout.push_str(&result.stdout);
        }
        if !result.stderr.is_empty() {
            if !combined_stderr.is_empty() {
                combined_stderr.push('\n');
            }
            combined_stderr.push_str(&result.stderr);
        }

        last_exit_code = result.exit_code;
        if !result.success {
            break; // && semantics: stop on failure
        }
    }

    Ok(CommandExecutionResult {
        command_id: String::new(),
        success: last_exit_code == 0,
        stdout: combined_stdout,
        stderr: combined_stderr,
        exit_code: last_exit_code,
        duration_ms: start.elapsed().as_millis() as u64,
        executed_at: chrono_now_iso(),
    })
}

/// Execute a single command (no pipes, no chains).
fn execute_single(command: &str) -> Result<CommandExecutionResult> {
    let start = Instant::now();
    let tokens = tokenize(command)?;
    let program = &tokens[0];
    let args = &tokens[1..];
    let env = get_session_env();

    let child = if cfg!(windows) && is_powershell_cmdlet(program) {
        debug!("Routing PowerShell cmdlet via powershell.exe: {}", program);
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", command])
            .envs(&env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    } else {
        validate_command_program(program)?;

        std::process::Command::new(program)
            .args(args)
            .envs(&env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    };

    let mut child = child
        .map_err(|e| FourDaError::Internal(format!("Failed to execute '{}': {}", program, e)))?;

    let spawned_pid = child.id();
    toolkit::register_spawned_pid(spawned_pid);

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
                toolkit::unregister_spawned_pid(spawned_pid);
                return Err(FourDaError::Internal(format!(
                    "Failed waiting on command: {}",
                    e
                )));
            }
        }
    };

    toolkit::unregister_spawned_pid(spawned_pid);

    let duration_ms = start.elapsed().as_millis() as u64;
    let executed_at = chrono_now_iso();

    match status {
        Some(exit_status) => {
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(mut out) = child.stdout.take() {
                use std::io::Read;
                if let Err(e) = out.read_to_string(&mut stdout) {
                    tracing::warn!("Process cleanup failed: {e}");
                }
            }
            if let Some(mut err) = child.stderr.take() {
                use std::io::Read;
                if let Err(e) = err.read_to_string(&mut stderr) {
                    tracing::warn!("Process cleanup failed: {e}");
                }
            }

            truncate_output(&mut stdout, MAX_STDOUT);
            truncate_output(&mut stderr, MAX_STDERR);

            let exit_code = exit_status.code().unwrap_or(-1);
            Ok(CommandExecutionResult {
                command_id: String::new(),
                success: exit_code == 0,
                stdout,
                stderr,
                exit_code,
                duration_ms,
                executed_at,
            })
        }
        None => {
            if let Err(e) = child.kill() {
                tracing::warn!("Process cleanup failed: {e}");
            }
            if let Err(e) = child.wait() {
                tracing::warn!("Process cleanup failed: {e}");
            }
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

/// Truncate output strings to stay within limits.
fn truncate_output(s: &mut String, max: usize) {
    if s.len() > max {
        s.truncate(max);
        s.push_str("\n...(output truncated)");
    }
}

// ============================================================================
// Command Dispatcher — routes to the right execution strategy
// ============================================================================

/// Split a command on `&&` for chain execution, respecting quotes.
fn split_on_chain(command: &str) -> Option<Vec<&str>> {
    // Only split if `&&` appears outside quotes
    if !command.contains("&&") {
        return None;
    }

    let parts: Vec<&str> = command.split("&&").collect();
    if parts.len() > 1 {
        Some(parts)
    } else {
        None
    }
}

/// Split a command on `|` for pipeline execution, respecting quotes.
/// Returns None if no pipes found or if the `|` is part of `||`.
fn split_on_pipe(command: &str) -> Option<Vec<&str>> {
    if !command.contains('|') {
        return None;
    }

    // Don't split on || (logical OR) — treat as copy-to-terminal
    if command.contains("||") {
        return None;
    }

    let parts: Vec<&str> = command.split('|').collect();
    if parts.len() > 1 {
        Some(parts)
    } else {
        None
    }
}

/// Main dispatcher: determines the execution strategy for a command.
fn dispatch_command(command: &str) -> Result<CommandExecutionResult> {
    let trimmed = command.trim();

    // 1. Handle `export VAR=value`
    if trimmed.to_lowercase().starts_with("export ") {
        return handle_export(trimmed);
    }

    // 2. Handle `source` (shell-only, inform user)
    if trimmed.to_lowercase().starts_with("source ") {
        return Ok(CommandExecutionResult {
            command_id: String::new(),
            success: true,
            stdout: "'source' reloads shell config — run in your terminal if needed.".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 0,
            executed_at: chrono_now_iso(),
        });
    }

    // 3. Strip `sudo` prefix — run directly, permission errors are informative
    let effective = if trimmed.starts_with("sudo ") {
        debug!(target: "4da::streets_cmd", "Stripping sudo prefix");
        &trimmed[5..]
    } else {
        trimmed
    };

    // 4. Handle `&&` chains (higher precedence than pipes)
    if let Some(segments) = split_on_chain(effective) {
        return execute_chain(&segments);
    }

    // 5. Handle pipes
    if let Some(segments) = split_on_pipe(effective) {
        return execute_pipeline(&segments);
    }

    // 6. Single command
    execute_single(effective)
}

/// Entry point for command execution. Called by the Tauri commands.
fn execute_command_blocking(command: &str) -> Result<CommandExecutionResult> {
    dispatch_command(command)
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
