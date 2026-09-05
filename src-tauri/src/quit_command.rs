// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `quit_app`: the one sanctioned way to stop 4DA from outside the tray menu.
//!
//! The exit guard (#501) keeps the tray-resident process alive when its window
//! closes, so a window close is a hide, never an exit. Only the tray menu's
//! "Quit" reached `app.exit(0)` — which meant every scripted stop (activation
//! drains, Victauri sessions, `stop-fourda.ps1`) was a force-kill that skipped
//! `Drop`, and each one left a ghost tray icon behind (two on 2026-09-05 alone;
//! see FAILURE_MODES "A window close is not a quit"). This command is the same
//! `app.exit(0)` the tray uses, callable from the About panel and from Victauri's
//! `invoke_command`.
use tauri::AppHandle;

/// Exit the application cleanly — the same path as the tray menu's Quit.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    tracing::info!(target: "4da::lifecycle", "quit_app invoked — exiting cleanly");
    app.exit(0);
}
