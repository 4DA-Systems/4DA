// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Crash dump protection — zeroizes sensitive memory before panic handlers run.
//!
//! On Windows, unhandled exceptions create minidumps that capture heap memory.
//! This module installs a panic hook that clears API keys, encryption keys,
//! and other secrets BEFORE the default handler writes the dump.

use std::sync::Once;

static INSTALL_ONCE: Once = Once::new();

/// Install the crash guard panic hook.
/// Safe to call multiple times — only installs once.
pub(crate) fn install() {
    INSTALL_ONCE.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Zeroize all sensitive memory BEFORE the crash dump is written
            tracing::error!(target: "4da::security", "Panic detected — zeroizing sensitive memory");
            zeroize_sensitive_memory();
            // Then run the default panic handler (which may create a crash dump)
            default_hook(info);
        }));
        tracing::info!(target: "4da::security", "Crash guard installed — secrets will be zeroized on panic");
    });
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

        // X (Twitter) API key
        settings.x_api_key.zeroize();

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
