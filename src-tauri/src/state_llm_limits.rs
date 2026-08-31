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

/// Tracks estimated LLM cost in MILLICENTS (1/1000 of a USD cent) consumed
/// today. Millicent granularity because the ledger sums per-call estimates:
/// a cheap judge call costs ~0.4c, which a cents-granular ledger records as 0
/// — the daily cost cap would never see those calls at all.
static LLM_DAILY_COST_MILLICENTS: AtomicU64 = AtomicU64::new(0);

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
/// `cost_millicents` is the estimated cost of the call in millicents
/// (1/1000 of a USD cent). Returns `true` if usage is within the limit,
/// `false` if exceeded.
pub(crate) fn record_llm_cost_millicents(cost_millicents: u64) -> bool {
    if cost_millicents == 0 {
        return true;
    }
    maybe_reset_daily_counter();
    let new_total =
        LLM_DAILY_COST_MILLICENTS.fetch_add(cost_millicents, Ordering::Relaxed) + cost_millicents;
    let limit_millicents = get_llm_daily_cost_limit().saturating_mul(1000);

    if limit_millicents > 0 {
        // Emit warning at 80% usage (once per day)
        let threshold_80 = limit_millicents / 5 * 4;
        if new_total >= threshold_80 && !LLM_COST_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
            warn!(
                target: "4da::llm",
                used_cents = new_total / 1000,
                limit_cents = limit_millicents / 1000,
                percent = (new_total as f64 / limit_millicents as f64 * 100.0) as u32,
                "LLM daily cost at 80%+ — approaching limit (${:.2} / ${:.2})",
                new_total as f64 / 100_000.0,
                limit_millicents as f64 / 100_000.0,
            );
        }

        if new_total > limit_millicents {
            warn!(
                target: "4da::llm",
                used_cents = new_total / 1000,
                limit_cents = limit_millicents / 1000,
                "Daily LLM cost limit exceeded"
            );
            return false;
        }
    }
    true
}

/// Seed today's usage counters from persisted per-call records (the
/// `ai_usage` table), so a process restart does not silently reset the daily
/// budget. Both `fourda` and `fourda-engine` call this at startup: without
/// it, every restart handed the process a fresh daily allowance — a dev day
/// with five restarts could spend 5x the configured cap.
///
/// Uses `fetch_max`, never `store`: if calls were already recorded in this
/// process before seeding ran, the larger value wins and nothing is lost.
pub(crate) fn seed_llm_daily_usage(tokens: u64, cost_millicents: u64) {
    maybe_reset_daily_counter();
    LLM_DAILY_TOKENS.fetch_max(tokens, Ordering::Relaxed);
    LLM_DAILY_COST_MILLICENTS.fetch_max(cost_millicents, Ordering::Relaxed);
    info!(
        target: "4da::llm",
        seeded_tokens = tokens,
        seeded_cost_cents = cost_millicents / 1000,
        "Seeded daily LLM usage counters from persisted usage records"
    );
}

/// Preflight cost wall (2026-08-31 audit): `true` when spending
/// `estimated_millicents` MORE would cross the daily cost cap.
///
/// [`is_llm_limit_reached`] only answers "are we already over?" — so the call
/// that crossed the line always went through, and the live `sun_alerts` table
/// recorded crossings up to **106% of the daily limit**. The cap has to be a
/// wall, not a tripwire behind the door: callers estimate the next call from
/// its input size and per-model rates BEFORE sending, and skip when the
/// estimate would cross 100%.
///
/// `estimated_millicents == 0` (unknown model, or a zero-rate local model)
/// never blocks — an estimate the registry cannot price is exactly the
/// pre-existing behavior, and refusing calls on fabricated numbers would
/// violate accurate-first. Limit `0` means unlimited, as everywhere else.
pub(crate) fn would_exceed_llm_cost_limit(estimated_millicents: u64) -> bool {
    if estimated_millicents == 0 {
        return false;
    }
    maybe_reset_daily_counter();
    let limit_millicents = get_llm_daily_cost_limit().saturating_mul(1000);
    if limit_millicents == 0 {
        return false;
    }
    LLM_DAILY_COST_MILLICENTS
        .load(Ordering::Relaxed)
        .saturating_add(estimated_millicents)
        > limit_millicents
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
    if cost_limit > 0
        && LLM_DAILY_COST_MILLICENTS.load(Ordering::Relaxed) >= cost_limit.saturating_mul(1000)
    {
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
    let used = LLM_DAILY_COST_MILLICENTS.load(Ordering::Relaxed) / 1000;
    let limit = get_llm_daily_cost_limit();
    (used, limit)
}

/// Reset the counters if the date has changed (new day = fresh budget).
fn maybe_reset_daily_counter() {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut date = LLM_DAILY_RESET_DATE.lock();
    if *date != today {
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
        LLM_DAILY_COST_MILLICENTS.store(0, Ordering::Relaxed);
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

const DEFAULT_RELEVANCE_THRESHOLD: f32 = 0.40;

fn relevance_threshold_from_bits(bits: u32) -> f32 {
    if bits == 0 {
        DEFAULT_RELEVANCE_THRESHOLD
    } else {
        f32::from_bits(bits)
    }
}

fn normalize_relevance_threshold(value: f32) -> f32 {
    value.clamp(0.30, 0.70)
}

/// Get the current relevance threshold (thread-safe).
/// Returns the auto-tuned value, or 0.40 default if not yet initialized.
/// Targets ~5-10% pass rate for genuinely relevant items.
pub(crate) fn get_relevance_threshold() -> f32 {
    relevance_threshold_from_bits(RELEVANCE_THRESHOLD_BITS.load(Ordering::Relaxed))
}

/// Set the relevance threshold (thread-safe, clamped to [0.30, 0.70]).
pub(crate) fn set_relevance_threshold(value: f32) {
    RELEVANCE_THRESHOLD_BITS.store(
        normalize_relevance_threshold(value).to_bits(),
        Ordering::Relaxed,
    );
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
        assert!((relevance_threshold_from_bits(0) - 0.40).abs() < f32::EPSILON);
    }

    #[test]
    fn test_relevance_threshold_bits_roundtrip() {
        let bits = normalize_relevance_threshold(0.50).to_bits();
        assert!((relevance_threshold_from_bits(bits) - 0.50).abs() < f32::EPSILON);
    }

    #[test]
    fn test_relevance_threshold_clamps_to_bounds() {
        assert!((normalize_relevance_threshold(0.10) - 0.30).abs() < f32::EPSILON);
        assert!((normalize_relevance_threshold(0.95) - 0.70).abs() < f32::EPSILON);
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

    /// The preflight wall stops the call that WOULD cross the cap — the exact
    /// call the post-hoc ledger let through at 106% in the 2026-08-31 audit.
    #[test]
    fn test_preflight_wall_blocks_the_crossing_call() {
        let _guard = LLM_TEST_LOCK.lock().unwrap();
        *LLM_DAILY_RESET_DATE.lock() = chrono::Local::now().format("%Y-%m-%d").to_string();
        let limit_millicents = get_llm_daily_cost_limit().saturating_mul(1000);
        assert!(
            limit_millicents > 0,
            "test assumes the default settings carry a nonzero cost cap"
        );

        // 1 millicent of headroom left: a 2-millicent call must be refused,
        // a 1-millicent call must pass (it lands exactly ON the cap).
        LLM_DAILY_COST_MILLICENTS.store(limit_millicents - 1, Ordering::Relaxed);
        assert!(would_exceed_llm_cost_limit(2));
        assert!(!would_exceed_llm_cost_limit(1));

        // Plenty of headroom: nothing blocks.
        LLM_DAILY_COST_MILLICENTS.store(0, Ordering::Relaxed);
        assert!(!would_exceed_llm_cost_limit(limit_millicents));
        assert!(would_exceed_llm_cost_limit(limit_millicents + 1));

        // Unknown-model estimate (0) never blocks, even with the budget gone.
        LLM_DAILY_COST_MILLICENTS.store(limit_millicents, Ordering::Relaxed);
        assert!(!would_exceed_llm_cost_limit(0));

        LLM_DAILY_COST_MILLICENTS.store(0, Ordering::Relaxed);
    }
}
