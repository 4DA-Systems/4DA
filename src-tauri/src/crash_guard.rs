// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Crash dump protection — zeroizes sensitive memory before panic handlers run.
//!
//! On Windows, unhandled exceptions create minidumps that capture heap memory.
//! This module installs a panic hook that clears API keys, encryption keys,
//! and other secrets BEFORE the default handler writes the dump.

use crate::settings::types::SensitiveString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

static INSTALL_ONCE: Once = Once::new();

/// Install the crash guard panic hook.
/// Safe to call multiple times — only installs once.
pub(crate) fn install() {
    INSTALL_ONCE.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Record WHAT panicked, synchronously, before anything else. The
            // default hook only prints to stderr (lost for a windowed app) and
            // the log file writer is non-blocking, so a panic that ends the
            // process can drop the record before it is flushed.
            let record = panic_record(info);
            tracing::error!(target: "4da::security", panic = %record, "Panic detected — zeroizing sensitive memory");
            write_panic_record(&record);
            // Zeroize BEFORE the crash dump is written — but only when this
            // panic ends the process. Most panics here are contained (a tokio
            // task, `task_guard::contain`, a worker thread) and the app runs
            // on: wiping the keys then left a live app with blank secrets
            // that the next save() wrote to disk (audit O3).
            if panic_ends_process(
                std::thread::current().name(),
                crate::task_guard::panic_is_contained(),
                is_non_unwinding(info),
            ) {
                zeroize_sensitive_memory();
            }
            // Then run the default panic handler (which may create a crash dump)
            default_hook(info);
        }));
        tracing::info!(target: "4da::security", "Crash guard installed — secrets will be zeroized on panic");
    });
}

/// Set once the secrets in memory were wiped; `SettingsManager::save` refuses
/// to write after that, so blanks never replace the stored keys.
static SECRETS_ZEROIZED: AtomicBool = AtomicBool::new(false);

pub(crate) fn secrets_zeroized() -> bool {
    SECRETS_ZEROIZED.load(Ordering::SeqCst)
}

/// Whether this panic aborts the process whatever thread it is on. The
/// standard library raises these with fixed messages: a panic in a `Drop`
/// while the thread is already unwinding ("panic in a destructor during
/// cleanup") and a panic that reaches a function that cannot unwind.
fn is_non_unwinding(info: &std::panic::PanicHookInfo<'_>) -> bool {
    let payload = info.payload();
    let msg = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("");
    is_non_unwinding_message(msg)
}

fn is_non_unwinding_message(msg: &str) -> bool {
    msg.starts_with("panic in a destructor during cleanup")
        || msg.starts_with("panic in a function that cannot unwind")
}

/// Whether this panic takes the process down.
///
/// - `panic = "abort"`: every panic does.
/// - A panic in a destructor while unwinding, or one that reaches a function
///   that cannot unwind, aborts, on any thread.
/// - Otherwise only the main thread's does (an unwind out of `main` ends the
///   process), and only when it is not inside `task_guard::contain`: the
///   headless engine runs its cycle contained on the main thread
///   (`block_on`), and a contained panic is a skipped cycle, not an exit.
///
/// A panic on any other thread ends that thread or task and the runtime
/// carries on.
fn panic_ends_process(thread_name: Option<&str>, contained: bool, aborting: bool) -> bool {
    cfg!(panic = "abort") || aborting || (thread_name == Some("main") && !contained)
}

/// Longest panic message kept in the record.
const MAX_PANIC_MESSAGE_CHARS: usize = 2000;

/// One-line description of a panic: thread, source location and message.
fn panic_record(info: &std::panic::PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("<unnamed>").to_string();
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown location>".to_string());
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_string());
    let message: String = message.chars().take(MAX_PANIC_MESSAGE_CHARS).collect();
    format!("thread '{thread}' panicked at {location}: {message}")
}

/// Append the record to `logs/panics.log` with a blocking write, so it
/// survives a panic that takes the process down. Best-effort: a panic hook
/// must never panic itself.
fn write_panic_record(record: &str) {
    use std::io::Write;

    let path = crate::log_retention::log_dir().join("panics.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            file,
            "{} pid={} {}",
            chrono::Utc::now().to_rfc3339(),
            std::process::id(),
            record
        );
        let _ = file.sync_all();
    }
}

/// Clear all known sensitive memory locations.
///
/// Uses `try_lock()` to avoid deadlocking if the panic occurred while
/// the settings lock was held.
fn zeroize_sensitive_memory() {
    use zeroize::Zeroize;

    // Before touching anything: from here on no save may run.
    SECRETS_ZEROIZED.store(true, Ordering::SeqCst);
    let manager = crate::get_settings_manager();
    if let Some(mut guard) = manager.try_lock() {
        let settings = guard.get_mut();

        // LLM API keys
        settings.llm.api_key.zeroize();
        settings.llm.openai_api_key.zeroize();

        // X (Twitter) API key — replace with default, dropping the old
        // SensitiveString which zeroizes its contents on drop.
        settings.x_api_key = SensitiveString::default();

        // License key
        settings.license.license_key.zeroize();

        // Translation provider API key
        settings.translation.api_key.zeroize();

        tracing::info!(target: "4da::security", "Sensitive memory zeroized successfully");
    } else {
        // Settings lock is held by the panicking thread — we can't safely access it.
        // The Drop impls on LLMProvider and TranslationConfig will still fire when
        // the SettingsManager is dropped, providing a second line of defense.
        tracing::warn!(
            target: "4da::security",
            "Could not acquire settings lock for zeroization — relying on Drop impls"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_is_idempotent() {
        // Calling install() multiple times should not panic
        install();
        install();
    }

    /// Only a panic that ends the process wipes the keys. A contained panic
    /// on a worker thread or task used to wipe them too (audit O3).
    #[test]
    fn only_a_process_ending_panic_zeroizes() {
        assert!(panic_ends_process(Some("main"), false, false));
        assert!(
            panic_ends_process(Some("worker"), false, true),
            "a non-unwinding panic aborts"
        );
        assert!(is_non_unwinding_message(
            "panic in a destructor during cleanup"
        ));
        assert!(is_non_unwinding_message(
            "panic in a function that cannot unwind"
        ));
        assert!(!is_non_unwinding_message("index out of bounds"));
        if !cfg!(panic = "abort") {
            assert!(!panic_ends_process(
                Some("tokio-runtime-worker"),
                false,
                false
            ));
            assert!(!panic_ends_process(None, false, false));
            assert!(
                !panic_ends_process(Some("main"), true, false),
                "a contained panic on main (the headless cycle) is not an exit"
            );
        }
    }

    /// Audit follow-up: the headless engine runs its cycle under
    /// `task_guard::contain` on the main thread. A panic there is a skipped
    /// cycle; it used to wipe the keys and let the run continue with blanks.
    #[test]
    fn a_contained_panic_on_the_main_thread_keeps_the_keys() {
        install();
        let joined = std::thread::Builder::new()
            .name("main".into())
            .spawn(|| {
                futures::executor::block_on(crate::task_guard::contain("test-main", async {
                    panic!("contained main-thread panic")
                }))
            })
            .unwrap()
            .join();
        assert_eq!(joined.ok(), Some(None::<()>), "the panic was contained");
        assert!(
            !secrets_zeroized(),
            "a contained panic must not wipe secrets"
        );
    }

    /// End to end through the installed hook: a contained panic on a worker
    /// thread leaves the save gate open.
    #[test]
    fn a_contained_panic_does_not_close_the_save_gate() {
        install();
        let joined = std::thread::Builder::new()
            .name("contained-worker".into())
            .spawn(|| panic!("contained test panic"))
            .unwrap()
            .join();
        assert!(joined.is_err(), "the worker panicked");
        assert!(
            !secrets_zeroized(),
            "a contained panic must not wipe secrets"
        );
    }
}
