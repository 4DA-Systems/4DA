// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Link-handler detection for the agent hand-off. Reads registry keys, files and
//! PATH only; never runs a program.
use std::path::{Path, PathBuf};

use super::{claude_code_route, AgentHandoffTargets, HandoffState};

/// What detection found: the frontend's view plus the Claude Code program path
/// the paste route starts (never sent to the webview).
pub(super) struct Detection {
    pub(super) targets: AgentHandoffTargets,
    pub(super) claude_path: Option<PathBuf>,
}

pub(super) fn detect() -> Detection {
    let home = dirs::home_dir();
    let handler = handler_command("claude-cli");
    let claude_path = handler
        .as_deref()
        .and_then(exe_from_handler_command)
        .filter(|p| p.is_file())
        .or_else(|| find_claude_binary(home.as_deref()));
    let (claude_code, claude_code_route) = claude_code_route(
        cfg!(windows),
        windows_terminal_installed(),
        handler.is_some(),
        claude_path.is_some(),
    );
    Detection {
        targets: AgentHandoffTargets {
            claude_code,
            claude_code_route,
            vscode_claude: if handler_command("vscode").is_some()
                && home.as_deref().is_some_and(has_claude_vscode_extension)
            {
                HandoffState::Ready
            } else {
                HandoffState::Missing
            },
            cursor: if handler_command("cursor").is_some() {
                HandoffState::Ready
            } else {
                HandoffState::Missing
            },
        },
        claude_path,
    }
}

/// The Claude Code extension installs as `anthropic.claude-code-<version>`
/// under VS Code's (or VS Code Insiders') extensions folder.
pub(crate) fn has_claude_vscode_extension(home: &Path) -> bool {
    [".vscode", ".vscode-insiders"].iter().any(|dir| {
        std::fs::read_dir(home.join(dir).join("extensions"))
            .map(|entries| {
                entries.flatten().any(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .starts_with("anthropic.claude-code-")
                })
            })
            .unwrap_or(false)
    })
}

/// The program a Windows handler command runs: `"C:\...\claude.exe" --handle-uri "%1"`
/// (quoted) or `C:\...\claude.exe --handle-uri "%1"` (unquoted, no spaces).
pub(crate) fn exe_from_handler_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let exe = if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        command.split_whitespace().next()?
    };
    let exe = exe.trim();
    (!exe.is_empty() && Path::new(exe).is_absolute()).then(|| PathBuf::from(exe))
}

/// `claude` on PATH, or in the native installer's `~/.local/bin`.
fn find_claude_binary(home: Option<&Path>) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["claude.exe", "claude.cmd"]
    } else {
        &["claude"]
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = home {
        dirs.push(home.join(".local").join("bin"));
    }
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| p.is_file())
}

/// Windows Terminal's app-execution alias. The alias is a reparse point that
/// `metadata()` cannot always follow, so its existence is read without following.
#[cfg(windows)]
fn windows_terminal_installed() -> bool {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| {
            Path::new(&d)
                .join("Microsoft")
                .join("WindowsApps")
                .join("wt.exe")
                .symlink_metadata()
                .is_ok()
        })
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn windows_terminal_installed() -> bool {
    false
}

/// The registered handler for `scheme`, if any (on Windows, the command line).
#[cfg(windows)]
fn handler_command(scheme: &str) -> Option<String> {
    use winreg::{enums::HKEY_CLASSES_ROOT, RegKey};
    // HKEY_CLASSES_ROOT merges the per-user (HKCU\Software\Classes, where
    // Claude Code registers) and machine-wide registrations.
    let key = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(format!(r"{scheme}\shell\open\command"))
        .ok()?;
    Some(key.get_value::<String, _>("").unwrap_or_default())
}

#[cfg(target_os = "macos")]
fn handler_command(scheme: &str) -> Option<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let apps: &[&str] = match scheme {
        "claude-cli" => &["Claude Code URL Handler.app"],
        "vscode" => &["Visual Studio Code.app"],
        "cursor" => &["Cursor.app"],
        _ => &[],
    };
    apps.iter()
        .find(|app| {
            home.join("Applications").join(app).is_dir()
                || Path::new("/Applications").join(app).is_dir()
        })
        .map(|app| (*app).to_string())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn handler_command(scheme: &str) -> Option<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local").join("share"));
    if scheme == "claude-cli"
        && data_home
            .join("applications")
            .join("claude-code-url-handler.desktop")
            .is_file()
    {
        return Some("claude-code-url-handler.desktop".into());
    }
    let mut lists = vec![
        home.join(".config").join("mimeapps.list"),
        data_home.join("applications").join("mimeinfo.cache"),
    ];
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for dir in data_dirs.split(':').filter(|d| !d.is_empty()) {
        lists.push(Path::new(dir).join("applications").join("mimeinfo.cache"));
    }
    lists.push(PathBuf::from(
        "/var/lib/snapd/desktop/applications/mimeinfo.cache",
    ));
    lists.iter().find_map(|f| {
        std::fs::read_to_string(f)
            .ok()
            .filter(|text| mime_lists_scheme(text, scheme))
            .map(|_| format!("x-scheme-handler/{scheme}"))
    })
}

/// Does a `mimeapps.list` / `mimeinfo.cache` text name a handler for `scheme`?
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
pub(crate) fn mime_lists_scheme(text: &str, scheme: &str) -> bool {
    let key = format!("x-scheme-handler/{scheme}=");
    text.lines().any(|line| {
        line.trim()
            .strip_prefix(&key)
            .is_some_and(|handlers| handlers.split(';').any(|h| !h.trim().is_empty()))
    })
}
