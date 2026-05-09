// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! LLM daily usage rate limiting and configuration helpers.
//!
//! Split from `state.rs` for file-size hygiene. Re-exported via `pub use` so
//! all call-sites continue to use `crate::state::*` unchanged.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use tracing::{info, warn};

use super::get_settings_manager;

// ============================================================================
// LLM Daily Usage Counters (hard cutoff for cost protection)
// ============================================================================

/// Tracks total LLM tokens consumed today (all providers, all callers).
static LLM_DAILY_TOKENS: AtomicU64 = AtomicU64::new(0);

/// Tracks estimated LLM cost in USD cents consumed today.
static LLM_DAILY_COST_CENTS: AtomicU64 = AtomicU64::new(0);

/// Stores the date string (YYYY-MM-DD local time) for daily reset detection.
static LLM_DAILY_RESET_DATE: Lazy<Mutex<String>> =
    Lazy::new(|| Mutex::new(chrono::Local::now().format("%Y-%m-%d").to_string()));

/// Whether the 80% token warning has been emitted today (avoid log spam).
static LLM_TOKEN_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

/// Whether the 80% cost warning has been emitted today (avoid log spam).
static LLM_COST_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

/// Record LLM token usage and check if still under the daily limit.
/// Returns `true` if usage is within the limit, `false` if the limit has been exceeded.
/// Automatically resets the counter at midnight local time.
pub(crate) fn record_llm_tokens(count: u64) -> bool {
    maybe_reset_daily_counter();
    let new_total = LLM_DAILY_TOKENS.fetch_add(count, Ordering::Relaxed) + count;
    let limit = get_llm_daily_token_limit();

    if limit > 0 {
        // Emit warning at 80% usage (once per day)
        let threshold_80 = limit * 4 / 5;
        if new_total >= threshold_80 && !LLM_TOKEN_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
            warn!(
                target: "4da::llm",
                used = new_total,
                limit = limit,
                percent = (new_total as f64 / limit as f64 * 100.0) as u32,
                "LLM daily token usage at 80%+ — approaching limit"
            );
        }

        if new_total > limit {
            warn!(
                target: "4da::llm",
                used = new_total,
                limit = limit,
                "Daily LLM token limit exceeded"
            );
            return false;
        }
    }
    true
}

/// Record LLM cost usage and check if still under the daily cost limit.
/// `cost_cents` is the estimated cost of the call in USD cents.
/// Returns `true` if usage is within the limit, `false` if exceeded.
pub(crate) fn record_llm_cost(cost_cents: u64) -> bool {
    if cost_cents == 0 {
        return true;
    }
    maybe_reset_daily_counter();
    let new_total = LLM_DAILY_COST_CENTS.fetch_add(cost_cents, Ordering::Relaxed) + cost_cents;
    let limit = get_llm_daily_cost_limit();

    if limit > 0 {
        // Emit warning at 80% usage (once per day)
        let threshold_80 = limit * 4 / 5;
        if new_total >= threshold_80 && !LLM_COST_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
            warn!(
                target: "4da::llm",
                used_cents = new_total,
                limit_cents = limit,
                percent = (new_total as f64 / limit as f64 * 100.0) as u32,
                "LLM daily cost at 80%+ — approaching limit (${:.2} / ${:.2})",
                new_total as f64 / 100.0,
                limit as f64 / 100.0,
            );
        }

        if new_total > limit {
            warn!(
                target: "4da::llm",
                used_cents = new_total,
                limit_cents = limit,
                "Daily LLM cost limit exceeded"
            );
            return false;
        }
    }
    true
}

/// Check if either the daily token limit or cost limit has been reached (pre-call gate).
/// Returns `true` if we are over any limit.
pub(crate) fn is_llm_limit_reached() -> bool {
    maybe_reset_daily_counter();

    let token_limit = get_llm_daily_token_limit();
    if token_limit > 0 && LLM_DAILY_TOKENS.load(Ordering::Relaxed) >= token_limit {
        return true;
    }

    let cost_limit = get_llm_daily_cost_limit();
    if cost_limit > 0 && LLM_DAILY_COST_CENTS.load(Ordering::Relaxed) >= cost_limit {
        return true;
    }

    false
}

/// Get current daily LLM token usage and the configured limit.
/// Returns `(used, limit)` where limit=0 means unlimited.
pub(crate) fn get_llm_token_usage() -> (u64, u64) {
    maybe_reset_daily_counter();
    let used = LLM_DAILY_TOKENS.load(Ordering::Relaxed);
    let limit = get_llm_daily_token_limit();
    (used, limit)
}

/// Get current daily LLM cost usage and the configured limit.
/// Returns `(used_cents, limit_cents)` where limit=0 means unlimited.
pub(crate) fn get_llm_cost_usage() -> (u64, u64) {
    maybe_reset_daily_counter();
    let used = LLM_DAILY_COST_CENTS.load(Ordering::Relaxed);
    let limit = get_llm_daily_cost_limit();
    (used, limit)
}

/// Reset the counters if the date has changed (new day = fresh budget).
fn maybe_reset_daily_counter() {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut date = LLM_DAILY_RESET_DATE.lock();
    if *date != today {
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
        LLM_DAILY_COST_CENTS.store(0, Ordering::Relaxed);
        LLM_TOKEN_WARNING_EMITTED.store(false, Ordering::Relaxed);
        LLM_COST_WARNING_EMITTED.store(false, Ordering::Relaxed);
        info!(target: "4da::llm", old_date = %*date, new_date = %today, "Daily LLM usage counters reset");
        *date = today;
    }
}

/// Read the LLM daily_token_limit from settings.
fn get_llm_daily_token_limit() -> u64 {
    let settings = get_settings_manager().lock();
    settings.get().llm_limits.daily_token_limit
}

/// Read the LLM daily_cost_limit_cents from settings.
fn get_llm_daily_cost_limit() -> u64 {
    let settings = get_settings_manager().lock();
    settings.get().llm_limits.daily_cost_limit_cents
}

// ============================================================================
// Configuration
// ============================================================================

/// Get context directories from settings (no fallback - empty means no context)
pub(crate) fn get_context_dirs() -> Vec<PathBuf> {
    let settings = get_settings_manager().lock();
    let dirs = settings.get().context_dirs.clone();
    drop(settings);

    dirs.into_iter()
        .map(|d| normalize_context_path(&d))
        .collect()
}

/// Convert WSL-style paths (/mnt/c/...) to Windows paths (C:\...) when running on Windows.
/// This handles the common case where paths are stored in settings using WSL conventions
/// but the app runs as a native Windows process.
fn normalize_context_path(path: &str) -> PathBuf {
    if cfg!(windows) && path.starts_with("/mnt/") {
        let rest = &path[5..]; // strip "/mnt/"
        let mut chars = rest.chars();
        if let Some(drive_letter) = chars.next() {
            if drive_letter.is_ascii_lowercase() {
                let remainder = chars.as_str();
                let win_remainder = remainder
                    .strip_prefix('/')
                    .unwrap_or(remainder)
                    .replace('/', "\\");
                return PathBuf::from(format!(
                    "{}:\\{}",
                    drive_letter.to_ascii_uppercase(),
                    win_remainder
                ));
            }
        }
    }
    PathBuf::from(path)
}

/// Legacy function for single directory (uses first configured dir)
pub(crate) fn get_context_dir() -> Option<PathBuf> {
    get_context_dirs().into_iter().next()
}

/// File extensions we care about for Phase 0
pub(crate) const SUPPORTED_EXTENSIONS: &[&str] = &["md", "txt", "rs", "ts", "js", "py"];

/// Relevance threshold stored as atomic u32 bits for thread-safe auto-tuning.
/// Adjusted daily based on user engagement rate (see `compute_threshold_adjustment`).
static RELEVANCE_THRESHOLD_BITS: AtomicU32 = AtomicU32::new(0);

/// Get the current relevance threshold (thread-safe).
/// Returns the auto-tuned value, or 0.35 default if not yet initialized.
/// Targets ~5-10% pass rate for genuinely relevant items.
pub(crate) fn get_relevance_threshold() -> f32 {
    let bits = RELEVANCE_THRESHOLD_BITS.load(Ordering::Relaxed);
    if bits == 0 {
        0.40 // Default: tightened from 0.35 to cut borderline items and produce a more curated stream
    } else {
        f32::from_bits(bits)
    }
}

/// Set the relevance threshold (thread-safe, clamped to [0.30, 0.70]).
pub(crate) fn set_relevance_threshold(value: f32) {
    let clamped = value.clamp(0.30, 0.70);
    RELEVANCE_THRESHOLD_BITS.store(clamped.to_bits(), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_normalize_context_path_wsl_to_windows() {
        let result = normalize_context_path("/mnt/c/Users/foo");
        if cfg!(windows) {
            assert_eq!(result, PathBuf::from("C:\\Users\\foo"));
        }
    }

    #[test]
    fn test_normalize_context_path_preserves_native_paths() {
        let native = if cfg!(windows) {
            "D:\\Projects\\myapp"
        } else {
            "/home/user/projects"
        };
        assert_eq!(normalize_context_path(native), PathBuf::from(native));
    }

    #[test]
    fn test_normalize_context_path_wsl_drive_letters() {
        if cfg!(windows) {
            assert_eq!(
                normalize_context_path("/mnt/d/code"),
                PathBuf::from("D:\\code")
            );
        }
    }

    #[test]
    fn test_relevance_threshold_default() {
        RELEVANCE_THRESHOLD_BITS.store(0, Ordering::Relaxed);
        assert!((get_relevance_threshold() - 0.40).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_and_get_relevance_threshold() {
        set_relevance_threshold(0.50);
        assert!((get_relevance_threshold() - 0.50).abs() < f32::EPSILON);
        RELEVANCE_THRESHOLD_BITS.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_relevance_threshold_clamps_to_bounds() {
        set_relevance_threshold(0.10);
        assert!((get_relevance_threshold() - 0.30).abs() < f32::EPSILON);
        set_relevance_threshold(0.95);
        assert!((get_relevance_threshold() - 0.70).abs() < f32::EPSILON);
        RELEVANCE_THRESHOLD_BITS.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_supported_extensions_contains_expected_types() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"rs"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"ts"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"py"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"md"));
        assert_eq!(SUPPORTED_EXTENSIONS.len(), 6);
    }

    // Tests that share global LLM_DAILY_TOKENS must not run in parallel.
    static LLM_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_llm_daily_tokens_tracks_usage() {
        let _guard = LLM_TEST_LOCK.lock().unwrap();
        // Ensure date matches so maybe_reset_daily_counter() won't clear our values
        *LLM_DAILY_RESET_DATE.lock() = chrono::Local::now().format("%Y-%m-%d").to_string();

        // Reset to known state
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
        let (used, _) = get_llm_token_usage();
        assert_eq!(used, 0);

        // Record some tokens
        record_llm_tokens(1000);
        let (used, _) = get_llm_token_usage();
        assert_eq!(used, 1000);

        // Record more
        record_llm_tokens(500);
        let (used, _) = get_llm_token_usage();
        assert_eq!(used, 1500);

        // Cleanup
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_llm_limit_not_reached_when_zero() {
        let _guard = LLM_TEST_LOCK.lock().unwrap();
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
        // With default limit > 0 and zero usage, should not be reached
        // (depends on settings default being > 0, which it is: 500_000)
        assert!(!is_llm_limit_reached());
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
    }
}
