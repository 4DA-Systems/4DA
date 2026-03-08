// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

use crate::ace;
use crate::context_engine::ContextEngine;
use crate::db::Database;
use crate::job_queue;
use crate::monitoring;
use crate::settings::SettingsManager;
use crate::source_fetching::{
    load_github_languages_from_settings, load_rss_feeds_from_settings, load_twitter_settings,
    load_youtube_channels_from_settings,
};
use crate::sources::{
    arxiv::ArxivSource, github::GitHubSource, hackernews::HackerNewsSource,
    producthunt::ProductHuntSource, reddit::RedditSource, rss::RssSource, twitter::TwitterSource,
    youtube::YouTubeSource, SourceRegistry,
};
use crate::AnalysisState;

// ============================================================================
// Analysis Abort Flag
// ============================================================================

/// Shared abort flag for analysis cancellation (separate from AnalysisState to avoid mutex)
static ANALYSIS_ABORT: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

pub(crate) fn get_analysis_abort() -> &'static Arc<AtomicBool> {
    &ANALYSIS_ABORT
}

// ============================================================================
// Centralized DB Path & Connection Helpers
// ============================================================================

/// Get the canonical path to the 4da.db database file.
/// Single source of truth — all connection opens should use this.
pub(crate) fn get_db_path() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base.push("4da.db");
    base
}

/// Register the sqlite-vec extension globally (idempotent).
/// Single source of truth — all code needing sqlite-vec calls this.
pub fn register_sqlite_vec_extension() {
    #[allow(clippy::missing_transmute_annotations)]
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
}

/// Open a raw SQLite connection with proper configuration.
/// Registers sqlite-vec auto-extension and sets busy_timeout.
/// Use this for ad-hoc connection needs outside the Database struct.
pub(crate) fn open_db_connection() -> Result<rusqlite::Connection, String> {
    let db_path = get_db_path();

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create data directory {}: {}", parent.display(), e))?;
    }

    register_sqlite_vec_extension();

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Set busy_timeout to prevent "database is locked" errors
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| format!("Failed to set busy_timeout: {}", e))?;

    Ok(conn)
}

// ============================================================================
// Global Database (Lazy Initialized)
// ============================================================================

static DATABASE: OnceCell<Arc<Database>> = OnceCell::new();

pub(crate) fn get_database() -> Result<&'static Arc<Database>, String> {
    DATABASE.get_or_try_init(|| {
        let db_path = get_db_path();

        info!(target: "4da::db", path = ?db_path, "Initializing database");

        let db = match Database::new(&db_path) {
            Ok(db) => db,
            Err(e) => {
                // Database may be corrupted — attempt recovery by renaming and recreating
                tracing::warn!(
                    target: "4da::db",
                    error = %e,
                    "Database open failed, attempting recovery"
                );
                let corrupt_path = db_path.with_extension("db.corrupt");
                if let Err(rename_err) = std::fs::rename(&db_path, &corrupt_path) {
                    return Err(format!(
                        "Database corrupted and recovery failed: {e} (rename: {rename_err})"
                    ));
                }
                // Also move WAL/SHM files if present
                let wal = db_path.with_extension("db-wal");
                let shm = db_path.with_extension("db-shm");
                if wal.exists() {
                    std::fs::remove_file(&wal).ok();
                }
                if shm.exists() {
                    std::fs::remove_file(&shm).ok();
                }
                tracing::info!(
                    target: "4da::db",
                    corrupt = ?corrupt_path,
                    "Corrupt database preserved, creating fresh database"
                );
                Database::new(&db_path)
                    .map_err(|e2| format!("Failed to create fresh database after recovery: {e2}"))?
            }
        };

        // Register all sources at startup (enables source enable/disable enforcement)
        db.register_source("hackernews", "Hacker News").ok();
        db.register_source("arxiv", "arXiv").ok();
        db.register_source("reddit", "Reddit").ok();
        db.register_source("github", "GitHub").ok();
        db.register_source("rss", "RSS").ok();
        db.register_source("youtube", "YouTube").ok();
        db.register_source("twitter", "Twitter").ok();
        db.register_source("lobsters", "Lobsters").ok();
        db.register_source("devto", "Dev.to").ok();
        db.register_source("producthunt", "Product Hunt").ok();

        info!(target: "4da::db", "Database ready");
        Ok(Arc::new(db))
    })
}

// ============================================================================
// Global Context Engine (Lazy Initialized)
// ============================================================================

static CONTEXT_ENGINE: Lazy<parking_lot::RwLock<Option<Arc<ContextEngine>>>> =
    Lazy::new(|| parking_lot::RwLock::new(None));

fn init_context_engine() -> Result<Arc<ContextEngine>, String> {
    let conn = open_db_connection()?;
    let engine = ContextEngine::new(Arc::new(parking_lot::Mutex::new(conn)))
        .map_err(|e| format!("Failed to initialize context engine: {}", e))?;
    info!(target: "4da::context", "Context engine initialized");
    Ok(Arc::new(engine))
}

pub(crate) fn get_context_engine() -> Result<Arc<ContextEngine>, String> {
    // Fast path: read lock
    {
        let guard = CONTEXT_ENGINE.read();
        if let Some(ref engine) = *guard {
            return Ok(Arc::clone(engine));
        }
    }
    // Slow path: write lock to initialize
    let mut guard = CONTEXT_ENGINE.write();
    if let Some(ref engine) = *guard {
        return Ok(Arc::clone(engine));
    }
    let engine = init_context_engine()?;
    *guard = Some(Arc::clone(&engine));
    Ok(engine)
}

/// Invalidate the context engine so it reinitializes on next access.
/// Call after settings changes that affect context (interests, exclusions, context dirs).
pub(crate) fn invalidate_context_engine() {
    let mut guard = CONTEXT_ENGINE.write();
    if guard.is_some() {
        *guard = None;
        info!(target: "4da::context", "Context engine invalidated, will reinitialize on next access");
    }
}

// ============================================================================
// Global ACE Instance (Lazy Initialized with RwLock for mutable access)
// ============================================================================

static ACE_ENGINE: OnceCell<Arc<parking_lot::RwLock<ace::ACE>>> = OnceCell::new();

fn init_ace_engine() -> Result<Arc<parking_lot::RwLock<ace::ACE>>, String> {
    let conn = open_db_connection()?;

    let engine = ace::ACE::new(Arc::new(parking_lot::Mutex::new(conn)))
        .map_err(|e| format!("Failed to initialize ACE: {}", e))?;

    info!(target: "4da::ace", "Autonomous Context Engine ready");
    Ok(Arc::new(parking_lot::RwLock::new(engine)))
}

pub(crate) fn get_ace_engine() -> Result<parking_lot::RwLockReadGuard<'static, ace::ACE>, String> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.read())
}

pub(crate) fn get_ace_engine_mut(
) -> Result<parking_lot::RwLockWriteGuard<'static, ace::ACE>, String> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.write())
}

// ============================================================================
// Global Source Registry (Lazy Initialized)
// ============================================================================

static SOURCE_REGISTRY: OnceCell<Mutex<SourceRegistry>> = OnceCell::new();

pub(crate) fn get_source_registry() -> &'static Mutex<SourceRegistry> {
    SOURCE_REGISTRY.get_or_init(|| {
        info!(target: "4da::sources", "Initializing source registry");
        let mut registry = SourceRegistry::new();

        // Register default sources
        registry.register(Box::new(HackerNewsSource::new()));
        registry.register(Box::new(ArxivSource::new()));
        registry.register(Box::new(RedditSource::new()));
        registry.register(Box::new(GitHubSource::with_languages(
            load_github_languages_from_settings(),
        )));
        registry.register(Box::new(ProductHuntSource::new()));

        // Register RSS source (feeds loaded from settings)
        let rss_feeds = load_rss_feeds_from_settings();
        registry.register(Box::new(RssSource::with_feeds(rss_feeds)));

        // Register Twitter/X source (X API v2 with Bearer Token)
        let (twitter_handles, x_api_key) = load_twitter_settings();
        registry.register(Box::new(
            TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key),
        ));

        // Register YouTube source (free RSS feeds, no API key needed)
        let youtube_channels = load_youtube_channels_from_settings();
        registry.register(Box::new(YouTubeSource::with_channels(youtube_channels)));

        info!(target: "4da::sources", count = registry.count(), "Source registry ready");
        Mutex::new(registry)
    })
}

// ============================================================================
// Global Settings Manager
// ============================================================================

static SETTINGS_MANAGER: OnceCell<Mutex<SettingsManager>> = OnceCell::new();

pub(crate) fn get_settings_manager() -> &'static Mutex<SettingsManager> {
    SETTINGS_MANAGER.get_or_init(|| {
        let db_path = get_db_path();
        let data_path = db_path
            .parent()
            .unwrap_or_else(|| {
                // get_db_path() always returns <dir>/data/4da.db, so parent is always Some.
                // If somehow it isn't, fall back to current directory.
                tracing::error!("Database path has no parent directory, falling back to current dir");
                std::path::Path::new(".")
            })
            .to_path_buf();

        info!(target: "4da::settings", "Initializing settings manager");
        let manager = SettingsManager::new(&data_path);
        info!(target: "4da::settings", rerank_enabled = manager.is_rerank_enabled(), "Settings loaded");
        Mutex::new(manager)
    })
}

// ============================================================================
// Global Analysis State
// ============================================================================

static ANALYSIS_STATE: OnceCell<Mutex<AnalysisState>> = OnceCell::new();

pub(crate) fn get_analysis_state() -> &'static Mutex<AnalysisState> {
    ANALYSIS_STATE.get_or_init(|| {
        Mutex::new(AnalysisState {
            running: false,
            completed: false,
            error: None,
            results: None,
            started_at: None,
            last_completed_at: None,
        })
    })
}

// ============================================================================
// Global Monitoring State
// ============================================================================

static MONITORING_STATE: OnceCell<Arc<monitoring::MonitoringState>> = OnceCell::new();

pub(crate) fn get_monitoring_state() -> &'static Arc<monitoring::MonitoringState> {
    MONITORING_STATE.get_or_init(|| Arc::new(monitoring::MonitoringState::new()))
}

// ============================================================================
// Global Job Queue (Background Extraction Processing)
// ============================================================================

// Planned: async job queue for background task management
#[allow(dead_code)]
static JOB_QUEUE: OnceCell<Arc<parking_lot::RwLock<job_queue::JobQueue>>> = OnceCell::new();

// Planned: async job queue for background task management
#[allow(dead_code)]
fn init_job_queue() -> Result<Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    let conn = open_db_connection()?;

    let queue = job_queue::JobQueue::new(Arc::new(parking_lot::Mutex::new(conn)));
    info!(target: "4da::job_queue", "Job queue initialized");
    Ok(Arc::new(parking_lot::RwLock::new(queue)))
}

// Planned: async job queue for background task management
#[allow(dead_code)]
pub(crate) fn get_job_queue(
) -> Result<&'static Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    JOB_QUEUE.get_or_try_init(init_job_queue)
}

// ============================================================================
// LLM Daily Token Usage Counter (hard cutoff for cost protection)
// ============================================================================

/// Tracks total LLM tokens consumed today (all providers, all callers).
static LLM_DAILY_TOKENS: AtomicU64 = AtomicU64::new(0);

/// Stores the date string (YYYY-MM-DD local time) for daily reset detection.
static LLM_DAILY_RESET_DATE: Lazy<Mutex<String>> =
    Lazy::new(|| Mutex::new(chrono::Local::now().format("%Y-%m-%d").to_string()));

/// Record LLM token usage and check if still under the daily limit.
/// Returns `true` if usage is within the limit, `false` if the limit has been exceeded.
/// Automatically resets the counter at midnight local time.
pub(crate) fn record_llm_tokens(count: u64) -> bool {
    maybe_reset_daily_counter();
    let new_total = LLM_DAILY_TOKENS.fetch_add(count, Ordering::Relaxed) + count;
    let limit = get_daily_token_limit();
    if limit > 0 && new_total > limit {
        warn!(
            target: "4da::llm",
            used = new_total,
            limit = limit,
            "Daily LLM token limit exceeded"
        );
        return false;
    }
    true
}

/// Check if the daily token limit has already been reached (pre-call gate).
/// Returns `true` if we are over the limit.
pub(crate) fn is_llm_limit_reached() -> bool {
    maybe_reset_daily_counter();
    let limit = get_daily_token_limit();
    if limit == 0 {
        return false; // 0 = unlimited
    }
    LLM_DAILY_TOKENS.load(Ordering::Relaxed) >= limit
}

/// Get current daily LLM token usage and the configured limit.
/// Returns `(used, limit)` where limit=0 means unlimited.
pub(crate) fn get_llm_token_usage() -> (u64, u64) {
    maybe_reset_daily_counter();
    let used = LLM_DAILY_TOKENS.load(Ordering::Relaxed);
    let limit = get_daily_token_limit();
    (used, limit)
}

/// Reset the counter if the date has changed (new day = fresh budget).
fn maybe_reset_daily_counter() {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut date = LLM_DAILY_RESET_DATE.lock();
    if *date != today {
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
        info!(target: "4da::llm", old_date = %*date, new_date = %today, "Daily LLM token counter reset");
        *date = today;
    }
}

/// Read the daily_token_limit from settings (cached per call; settings rarely change).
fn get_daily_token_limit() -> u64 {
    let settings = get_settings_manager().lock();
    settings.get().rerank.daily_token_limit
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
        0.35 // Default: accounts for multiplicative compression from quality layers
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
    fn test_get_db_path_points_to_data_dir() {
        let path = get_db_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("data") && path_str.ends_with("4da.db"));
    }

    #[test]
    fn test_register_sqlite_vec_extension_is_idempotent() {
        register_sqlite_vec_extension();
        register_sqlite_vec_extension();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result: String = conn
            .query_row("SELECT vec_version()", [], |row| row.get(0))
            .unwrap();
        assert!(!result.is_empty());
    }

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
        assert!((get_relevance_threshold() - 0.35).abs() < f32::EPSILON);
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
    fn test_analysis_abort_flag_toggle() {
        let abort = get_analysis_abort();
        abort.store(false, Ordering::Relaxed);
        assert!(!abort.load(Ordering::Relaxed));
        abort.store(true, Ordering::Relaxed);
        assert!(abort.load(Ordering::Relaxed));
        abort.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_supported_extensions_contains_expected_types() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"rs"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"ts"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"py"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"md"));
        assert_eq!(SUPPORTED_EXTENSIONS.len(), 6);
    }

    #[test]
    fn test_llm_daily_tokens_tracks_usage() {
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
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
        // With default limit > 0 and zero usage, should not be reached
        // (depends on settings default being > 0, which it is: 500_000)
        assert!(!is_llm_limit_reached());
        LLM_DAILY_TOKENS.store(0, Ordering::Relaxed);
    }
}
