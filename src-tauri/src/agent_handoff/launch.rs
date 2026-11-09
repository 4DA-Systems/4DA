// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Starting the hand-off: give a link to the OS handler, or (Windows without
//! Windows Terminal) open an empty Claude Code console. Every child process has
//! the variables of any Claude Code session 4DA was started from removed.
use std::path::Path;
use std::process::Command;

use super::vars_to_scrub;
use crate::error::{FourDaError, Result};

/// Remove inherited Claude Code session variables, keeping any the user set in
/// their persistent environment (their own Claude Code configuration).
fn scrub_claude_session(cmd: &mut Command) {
    let current: Vec<String> = std::env::vars_os()
        .filter_map(|(k, _)| k.into_string().ok())
        .collect();
    let persistent = persistent_env_names();
    for name in vars_to_scrub(current.iter().map(String::as_str), &persistent) {
        cmd.env_remove(name);
    }
}

#[cfg(windows)]
fn persistent_env_names() -> Vec<String> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
        RegKey,
    };
    let mut names = Vec::new();
    for (root, path) in [
        (HKEY_CURRENT_USER, "Environment"),
        (
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ),
    ] {
        if let Ok(key) = RegKey::predef(root).open_subkey(path) {
            names.extend(key.enum_values().flatten().map(|(name, _)| name));
        }
    }
    names
}

#[cfg(not(windows))]
fn persistent_env_names() -> Vec<String> {
    Vec::new()
}

/// Wait for a short-lived opener in the background so it leaves no zombie.
#[cfg(unix)]
fn reap(mut child: std::process::Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

fn spawn_failed(e: std::io::Error) -> FourDaError {
    FourDaError::Internal(format!("Could not open the agent: {e}"))
}

/// Hand a link to the OS. On Windows a fresh `explorer.exe` passes it to the
/// registered handler (the opener plugin's ShellExecute would pass on 4DA's own
/// environment). explorer.exe exits non-zero even on success, so it is not waited on.
#[cfg(windows)]
pub(super) fn open_link(url: &str) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(url).creation_flags(CREATE_NO_WINDOW);
    scrub_claude_session(&mut cmd);
    cmd.spawn().map(drop).map_err(spawn_failed)
}

#[cfg(target_os = "macos")]
pub(super) fn open_link(url: &str) -> Result<()> {
    let mut cmd = Command::new("open");
    cmd.arg(url);
    scrub_claude_session(&mut cmd);
    reap(cmd.spawn().map_err(spawn_failed)?);
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn open_link(url: &str) -> Result<()> {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(url);
    scrub_claude_session(&mut cmd);
    reap(cmd.spawn().map_err(spawn_failed)?);
    Ok(())
}

/// Open an empty interactive Claude Code in a new console window in `dir`.
/// `cmd.exe` itself gets CREATE_NO_WINDOW; `start` gives Claude Code its own
/// visible console. Nothing is typed into it: the user pastes the prompt.
#[cfg(windows)]
pub(super) fn start_claude_console(exe: &Path, dir: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let args = super::console_start_args(exe, dir)?;
    let mut cmd = Command::new("cmd.exe");
    cmd.raw_arg(args).creation_flags(CREATE_NO_WINDOW);
    scrub_claude_session(&mut cmd);
    cmd.spawn().map(drop).map_err(spawn_failed)
}

/// The paste route exists only on Windows (see `claude_code_route`).
#[cfg(not(windows))]
pub(super) fn start_claude_console(_exe: &Path, _dir: &Path) -> Result<()> {
    Err(FourDaError::Validation(
        "Opening an empty Claude Code console is only used on Windows.".into(),
    ))
}
