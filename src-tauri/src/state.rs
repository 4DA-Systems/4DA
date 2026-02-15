// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1). See LICENSE file.

use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tracing::info;

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

/// Open a raw SQLite connection with proper configuration.
/// Registers sqlite-vec auto-extension and sets busy_timeout.
/// Use this for ad-hoc connection needs outside the Database struct.
pub(crate) fn open_db_connection() -> Result<rusqlite::Connection, String> {
    let db_path = get_db_path();

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Register sqlite-vec extension globally (idempotent)
    // One of two registration sites. See also: db.rs:Database::new()
    #[allow(clippy::missing_transmute_annotations)]
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

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

        let db =
            Database::new(&db_path).map_err(|e| format!("Failed to initialize database: {}", e))?;

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
        let data_path = get_db_path()
            .parent()
            .expect("Database path must have a parent directory")
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

static JOB_QUEUE: OnceCell<Arc<parking_lot::RwLock<job_queue::JobQueue>>> = OnceCell::new();

fn init_job_queue() -> Result<Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    let conn = open_db_connection()?;

    let queue = job_queue::JobQueue::new(Arc::new(parking_lot::Mutex::new(conn)));
    info!(target: "4da::job_queue", "Job queue initialized");
    Ok(Arc::new(parking_lot::RwLock::new(queue)))
}

pub(crate) fn get_job_queue(
) -> Result<&'static Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    JOB_QUEUE.get_or_try_init(init_job_queue)
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
