// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Agent hand-off: open the user's OWN coding agent with a 4DA prompt pre-filled.
//!
//! People who already pay for Claude, ChatGPT or Cursor should not need a separate
//! API key to act on what 4DA found. Anthropic does not let third-party apps offer
//! Claude.ai login or use a plan's limits (code.claude.com legal-and-compliance and
//! the Agent SDK overview), so 4DA never runs an agent itself. Instead it hands a
//! prompt to the agent through the vendor's documented link handler:
//!
//! - Claude Code (terminal): `claude-cli://open?cwd=<abs path>&q=<prompt>`
//!   (code.claude.com/docs/en/deep-links; `q` is capped at 5,000 characters).
//! - Claude Code in VS Code: `vscode://anthropic.claude-code/open?prompt=<prompt>`.
//! - Cursor: `cursor://anysphere.cursor-deeplink/prompt?text=<prompt>`.
//!
//! Every one of these only PRE-FILLS the prompt: nothing reaches a model until the
//! user reads it and presses Enter in their own tool, under their own account.
//!
//! **Windows without Windows Terminal.** Claude Code's `claude-cli://` handler then
//! falls back to a PowerShell window that exits at once (Claude Code 2.1.319,
//! traced 2026-11-09), so the link cannot work there. 4DA uses only documented
//! behaviour instead: the frontend copies the prompt, and 4DA starts a plain,
//! empty interactive Claude Code in a new console in the project folder for the
//! user to paste into. The prompt is never auto-sent, and no undocumented flag is
//! used.
//!
//! This module makes no network call and never reads a credential. Detection reads
//! only link-handler registrations, install folders and PATH. Every process it
//! starts has the variables of any Claude Code session 4DA itself was launched
//! from removed, because Claude Code refuses to start nested inside one.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::error::{FourDaError, Result};

/// Claude Code's documented cap on the `q` parameter (decoded characters).
pub(crate) const CLAUDE_PROMPT_MAX_CHARS: usize = 5_000;
/// Keep every hand-off URL under Cursor's ~8,000-character deep-link cap.
pub(crate) const HANDOFF_URL_MAX_CHARS: usize = 8_000;

/// The agents 4DA can hand a prompt to. A closed set: the webview never chooses
/// a URL scheme.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum HandoffTarget {
    ClaudeCode,
    VscodeClaude,
    Cursor,
}

/// Whether a hand-off to one agent would work on this machine right now.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum HandoffState {
    /// A click opens the agent.
    Ready,
    /// Claude Code is installed but has not registered `claude-cli://` yet. It does
    /// that the first time the user sends a message in an interactive session.
    NeedsFirstRun,
    /// Not installed (or no link handler): the button is not offered.
    Missing,
}

/// How a Claude Code hand-off reaches the user.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum ClaudeCodeRoute {
    /// The `claude-cli://` link opens Claude Code with the prompt pre-filled.
    Prefill,
    /// Windows without Windows Terminal: the frontend copies the prompt and 4DA
    /// opens an empty Claude Code for the user to paste into.
    Paste,
}

/// Hand-off availability per agent, for the frontend to decide what to offer.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "bindings/")]
pub struct AgentHandoffTargets {
    pub claude_code: HandoffState,
    pub claude_code_route: ClaudeCodeRoute,
    pub vscode_claude: HandoffState,
    pub cursor: HandoffState,
}

/// What a hand-off did, so the frontend can tell the user what happens next.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum HandoffOutcome {
    /// The agent opened with the prompt in its input box.
    Prefilled,
    /// Claude Code opened empty; the user pastes the copied prompt.
    Paste,
}

/// Which agents can take a hand-off on this machine. Reads only link-handler
/// registrations, install folders and PATH; never runs anything.
#[tauri::command]
pub fn agent_handoff_targets() -> AgentHandoffTargets {
    detect::detect().targets
}

/// Open the user's agent with `prompt` pre-filled (and, for Claude Code, in `cwd`).
/// The prompt is NOT sent: the user reviews it and presses Enter in their agent.
#[tauri::command]
pub fn open_agent_handoff(
    target: HandoffTarget,
    prompt: String,
    cwd: Option<String>,
) -> Result<HandoffOutcome> {
    let found = detect::detect();
    match state_of(&found.targets, target) {
        HandoffState::Ready => {}
        HandoffState::NeedsFirstRun => return Err(FourDaError::Validation(
            "Claude Code has not registered its link handler yet. Open Claude Code once and send any message, then try again.".into(),
        )),
        HandoffState::Missing => {
            return Err(FourDaError::Validation(
                "That agent is not installed on this machine.".into(),
            ))
        }
    }

    let outcome = if target == HandoffTarget::ClaudeCode
        && found.targets.claude_code_route == ClaudeCodeRoute::Paste
    {
        validate_prompt(target, &prompt)?;
        let dir = match cwd.as_deref() {
            Some(dir) => validate_cwd(dir)?,
            None => dirs::home_dir().ok_or_else(|| {
                FourDaError::Validation("Could not find your home folder.".into())
            })?,
        };
        let exe = found.claude_path.ok_or_else(|| {
            FourDaError::Validation("Could not find the Claude Code program.".into())
        })?;
        launch::start_claude_console(&exe, &dir)?;
        HandoffOutcome::Paste
    } else {
        let url = build_handoff_url(target, &prompt, cwd.as_deref())?;
        launch::open_link(&url)?;
        HandoffOutcome::Prefilled
    };

    // The prompt text stays out of the log: it can name projects and packages.
    info!(
        target: "4da::agent_handoff",
        ?target,
        ?outcome,
        prompt_chars = prompt.chars().count(),
        with_cwd = cwd.is_some(),
        "Handed a prompt to the user's agent (not sent until they press Enter)"
    );
    Ok(outcome)
}

pub(crate) fn state_of(targets: &AgentHandoffTargets, target: HandoffTarget) -> HandoffState {
    match target {
        HandoffTarget::ClaudeCode => targets.claude_code,
        HandoffTarget::VscodeClaude => targets.vscode_claude,
        HandoffTarget::Cursor => targets.cursor,
    }
}

/// Claude Code's state and route from what detection found.
///
/// On Windows the `claude-cli://` handler only works when Windows Terminal is
/// installed (its fallback PowerShell window exits at once), so without it the
/// paste route is used whenever the program can be found.
pub(crate) fn claude_code_route(
    windows: bool,
    windows_terminal: bool,
    handler_registered: bool,
    binary_found: bool,
) -> (HandoffState, ClaudeCodeRoute) {
    if windows && !windows_terminal {
        return if binary_found {
            (HandoffState::Ready, ClaudeCodeRoute::Paste)
        } else {
            (HandoffState::Missing, ClaudeCodeRoute::Prefill)
        };
    }
    let state = if handler_registered {
        HandoffState::Ready
    } else if binary_found {
        HandoffState::NeedsFirstRun
    } else {
        HandoffState::Missing
    };
    (state, ClaudeCodeRoute::Prefill)
}

/// Build the hand-off URL, refusing anything the agent's handler would reject or
/// that could hide text from the person reviewing the prompt.
pub(crate) fn build_handoff_url(
    target: HandoffTarget,
    prompt: &str,
    cwd: Option<&str>,
) -> Result<String> {
    validate_prompt(target, prompt)?;
    let q = urlencoding::encode(prompt);
    let url = match target {
        HandoffTarget::ClaudeCode => match cwd {
            Some(dir) => {
                let dir = validate_cwd(dir)?;
                format!(
                    "claude-cli://open?cwd={}&q={q}",
                    urlencoding::encode(&dir.to_string_lossy())
                )
            }
            None => format!("claude-cli://open?q={q}"),
        },
        // VS Code and Cursor open in the window the user already has focused, so a
        // working directory has nowhere to go; it is ignored rather than refused.
        HandoffTarget::VscodeClaude => format!("vscode://anthropic.claude-code/open?prompt={q}"),
        HandoffTarget::Cursor => format!("cursor://anysphere.cursor-deeplink/prompt?text={q}"),
    };
    if url.chars().count() > HANDOFF_URL_MAX_CHARS {
        return Err(FourDaError::Validation(format!(
            "The prompt is too long to hand off (the link would be over {HANDOFF_URL_MAX_CHARS} characters)."
        )));
    }
    Ok(url)
}

pub(crate) fn validate_prompt(target: HandoffTarget, prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() {
        return Err(FourDaError::Validation(
            "There is no prompt to hand off.".into(),
        ));
    }
    if prompt.chars().any(is_hidden_char) {
        return Err(FourDaError::Validation(
            "The prompt contains control or direction-changing characters.".into(),
        ));
    }
    if target == HandoffTarget::ClaudeCode && prompt.chars().count() > CLAUDE_PROMPT_MAX_CHARS {
        return Err(FourDaError::Validation(format!(
            "Claude Code accepts at most {CLAUDE_PROMPT_MAX_CHARS} characters in a link prompt."
        )));
    }
    Ok(())
}

/// Characters that can hide or reorder text in the reviewer's view. Line breaks
/// and tabs are ordinary prompt formatting and are allowed.
pub(crate) fn is_hidden_char(c: char) -> bool {
    if c == '\n' || c == '\t' {
        return false;
    }
    c.is_control()
        || matches!(c,
            '\u{200B}'..='\u{200F}' // zero-width space/joiners, LRM/RLM
            | '\u{202A}'..='\u{202E}' // bidi embeddings and overrides
            | '\u{2066}'..='\u{2069}' // bidi isolates
            | '\u{FEFF}' // zero-width no-break space
        )
}

/// Claude Code's own `cwd` rules (deep-links docs): absolute, no network or UNC
/// path, no `..`, no hidden characters. It must also exist and be a directory.
pub(crate) fn validate_cwd(dir: &str) -> Result<PathBuf> {
    let reject = |why: &str| {
        Err(FourDaError::Validation(format!(
            "Cannot open the agent there: {why}."
        )))
    };
    if dir.chars().any(is_hidden_char) {
        return reject("the path contains control or direction-changing characters");
    }
    if dir.starts_with("\\\\") || dir.starts_with("//") {
        return reject("network and UNC paths are not allowed");
    }
    let path = Path::new(dir);
    if !path.is_absolute() {
        return reject("the path is not absolute");
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return reject("the path contains '..'");
    }
    if !path.is_dir() {
        return reject("the folder does not exist");
    }
    Ok(path.to_path_buf())
}

/// A variable that marks a Claude Code session (or carries its config into a
/// child). Claude Code refuses to start nested when it sees `CLAUDECODE`, and a
/// 4DA started from a Claude Code session inherits all of them.
pub(crate) fn is_claude_session_var(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper == "CLAUDECODE" || upper.starts_with("CLAUDE_CODE_") || upper.starts_with("CLAUDE_AGENT_")
}

/// Which of the current variables to remove from a hand-off child. A name the
/// user set in their persistent environment (Windows: HKCU/HKLM Environment) is
/// their own Claude Code configuration and is kept.
pub(crate) fn vars_to_scrub<'a>(
    current: impl IntoIterator<Item = &'a str>,
    persistent: &[String],
) -> Vec<String> {
    current
        .into_iter()
        .filter(|name| is_claude_session_var(name))
        .filter(|name| !persistent.iter().any(|p| p.eq_ignore_ascii_case(name)))
        .map(str::to_string)
        .collect()
}

/// The `cmd.exe` arguments that open an empty, interactive Claude Code in a new
/// console window in `dir`. Refuses characters cmd.exe would interpret inside
/// quotes (`%` expansion) or that would end the quoting (`"`).
#[cfg(any(windows, test))]
pub(crate) fn console_start_args(exe: &Path, dir: &Path) -> Result<String> {
    let exe = exe.to_string_lossy();
    let mut dir = dir.to_string_lossy().to_string();
    for s in [exe.as_ref(), dir.as_str()] {
        if s.contains('"') || s.contains('%') || s.chars().any(is_hidden_char) {
            return Err(FourDaError::Validation(
                "Cannot open Claude Code for a path containing %, quotes or control characters."
                    .into(),
            ));
        }
    }
    // A trailing backslash before the closing quote is harmless to cmd.exe but
    // pointless; keep it only for a drive root ("C:\").
    while dir.len() > 3 && (dir.ends_with('\\') || dir.ends_with('/')) {
        dir.pop();
    }
    Ok(format!("/c start \"4DA\" /D \"{dir}\" \"{exe}\""))
}

mod detect;
mod launch;

#[cfg(test)]
pub(crate) use detect::{exe_from_handler_command, has_claude_vscode_extension, mime_lists_scheme};

#[cfg(test)]
#[path = "agent_handoff_tests.rs"]
mod tests;
