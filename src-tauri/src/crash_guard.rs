// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Crash dump protection — zeroizes sensitive memory before panic handlers run.
//!
//! On Windows, unhandled exceptions create minidumps that capture heap memory.
//! This module installs a panic hook that clears API keys, encryption keys,
//! and other secrets BEFORE the default handler writes the dump.

use crate::settings::types::SensitiveString;
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
            // Zeroize all sensitive memory BEFORE the crash dump is written
            zeroize_sensitive_memory();
            // Then run the default panic handler (which may create a crash dump)
            default_hook(info);
        }));
        tracing::info!(target: "4da::security", "Crash guard installed — secrets will be zeroized on panic");
    });
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
}
