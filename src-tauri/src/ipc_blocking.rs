// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Keep blocking work off the UI thread (audit 2026-10-07, wave 2c).
//!
//! Tauri runs a NON-async `#[tauri::command]` inline in the IPC handler, and
//! WebView2 invokes that handler on the UI thread (tauri-macros
//! `command/wrapper.rs`, tauri `ipc/protocol.rs`). A sync command that touches
//! SQLite, the filesystem, the OS keychain or a child process therefore
//! freezes the whole window — and queues every other IPC call behind it — for
//! as long as it runs. `get_knowledge_gaps` did exactly that for up to 111 s
//! (#873). The fix for the class: such a command is `async` and runs its body
//! through [`off_ui_thread`] on the blocking pool.
//!
//! `ipc_blocking_tests.rs` enforces it: every registered command that is still
//! sync must sit in its allowlist with a reason, and an allowlisted body may
//! not reach a known-blocking API.

use std::future::Future;

/// Run a blocking command body on the blocking pool and await it.
///
/// A panic or cancellation of the blocking task becomes an error of the
/// command's own type (`String` and `FourDaError` both convert from `String`),
/// so the frontend sees a rejected promise instead of a hung one.
pub(crate) fn off_ui_thread<T, E, F>(
    command: &'static str,
    body: F,
) -> impl Future<Output = Result<T, E>>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    let task = tauri::async_runtime::spawn_blocking(body);
    async move {
        task.await
            .unwrap_or_else(|e| Err(E::from(format!("{command}: blocking task failed: {e}"))))
    }
}

/// [`off_ui_thread`] for a body that cannot fail. The command keeps its
/// payload; only a panicked blocking task surfaces, as a `String` error.
pub(crate) async fn off_ui_thread_infallible<T, F>(
    command: &'static str,
    body: F,
) -> Result<T, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    off_ui_thread(command, move || Ok::<T, String>(body())).await
}

/// Run a long synchronous stretch of an `async` task without holding a
/// runtime worker hostage while it runs.
///
/// Tauri's runtime is a multi-thread tokio runtime. A task that computes for
/// tens of seconds inside one `poll` keeps its worker — and whatever that
/// worker had queued in its non-stealable LIFO slot — until it next awaits.
/// The scoring pass is exactly that: ~33 s of `score_item` plus ~11 s of
/// dedup/diversity with no `.await` in between (fresh-profile E2E,
/// 2026-10-09: an `add_interest` whose embedding Ollama answered at 02:04:56
/// only ran its INSERT at 02:05:25.99, 21 ms after the scoring pass reached
/// its first await, and "Enter 4DA" sat frozen for the difference).
///
/// On a multi-thread runtime the body runs under `block_in_place`, which
/// hands this worker's queue to a fresh worker first. Anywhere else (a
/// current-thread test runtime, a plain thread) it simply runs inline —
/// `block_in_place` would panic there.
pub(crate) fn cpu_bound<R>(body: impl FnOnce() -> R) -> R {
    match tokio::runtime::Handle::try_current() {
        Ok(h) if h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(body)
        }
        _ => body(),
    }
}

#[cfg(test)]
#[path = "ipc_blocking_tests.rs"]
mod tests;
