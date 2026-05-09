// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};

#[path = "state_llm_limits.rs"]
mod state_llm_limits;
pub(crate) use state_llm_limits::*;

use crate::ace;
use crate::context_engine::ContextEngine;
use crate::db::Database;
use crate::error::{Result, ResultExt};
use crate::monitoring;
use crate::settings::SettingsManager;
use crate::sources::SourceRegistry;
use crate::AnalysisState;

// ============================================================================
// LOCK ORDERING (acquire in this order to prevent deadlocks)
//
// 1. SETTINGS_MANAGER   — lightweight reads, released immediately
// 2. DATABASE            — connection pool, held for queries only
// 3. CONTEXT_ENGINE      — depends on DB reads
// 4. ACE_ENGINE          — depends on settings + DB
// 5. SOURCE_REGISTRY     — depends on settings
// 6. ANALYSIS_STATE      — leaf node, no further locks needed
//
// CRITICAL: Never hold a MutexGuard<T> across an .await point.
//           parking_lot::Mutex is not Send across yield points.
// ============================================================================

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
///
/// Resolution order:
/// 1. FOURDA_DB_PATH env var (explicit override)
/// 2. data/4da.db relative to CARGO_MANIFEST_DIR (development builds)
/// 3. Platform-specific app data directory (deployed builds)
pub(crate) fn get_db_path() -> PathBuf {
    // 1. Explicit override via environment variable
    if let Ok(path) = std::env::var("FOURDA_DB_PATH") {
        return PathBuf::from(path);
    }

    // 2. Development: relative to project root (CARGO_MANIFEST_DIR = src-tauri/)
    let dev_path = {
        let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        base.pop(); // up from src-tauri/ to project root
        base.push("data");
        base.push("4da.db");
        base
    };
    if dev_path.parent().is_some_and(std::path::Path::exists) {
        return dev_path;
    }

    // 3. Deployed: platform-specific app data directory
    let app_data = get_platform_data_dir();
    app_data.join("4da.db")
}

/// Get the platform-specific data directory for 4DA.
/// Mirrors Tauri's app_data_dir resolution for com.4da.app.
fn get_platform_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("com.4da.app").join("data");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home
                .join("Library")
                .join("Application Support")
                .join("com.4da.app")
                .join("data");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Respect XDG Base Directory Specification: $XDG_DATA_HOME (default ~/.local/share)
        if let Some(data_dir) = dirs::data_dir() {
            return data_dir.join("4da").join("data");
        }
    }

    // Ultimate fallback: current directory
    warn!(target: "4da::state", "Could not determine platform data directory, using ./data");
    PathBuf::from("data")
}

/// Whether the sqlite-vec extension loaded successfully.
/// Starts `true` (optimistic) and is set to `false` if loading panics or
/// the post-load verification query fails.
static SQLITE_VEC_AVAILABLE: AtomicBool = AtomicBool::new(true);

/// One-shot guard for sqlite-vec verification logging.
/// The verify query runs once per process at startup; subsequent
/// `open_db_connection()` calls trust the cached result and stay silent.
/// This is what eliminates the cold-boot log spam (224 callsites would
/// otherwise log "sqlite-vec verified" on every connection open).
static SQLITE_VEC_VERIFY_DONE: AtomicBool = AtomicBool::new(false);

/// Check whether vector search (sqlite-vec) is available.
/// Returns `false` if the extension failed to load or verification failed,
/// meaning the app should fall back to keyword-only search.
pub fn is_vector_search_available() -> bool {
    SQLITE_VEC_AVAILABLE.load(Ordering::Relaxed)
}

/// Run the sqlite-vec verification query exactly once per process.
///
/// Called from `initialize_pre_tauri()` at startup. Opens a throwaway
/// connection, runs `SELECT vec_version()`, logs the result a single time.
/// If verification fails, marks the extension unavailable so the rest of
/// the app degrades to keyword-only search.
///
/// Subsequent `open_db_connection()` calls skip the verify+log entirely —
/// this is the fix for the cold-boot log stampede where every background
/// task printed the same "sqlite-vec verified" line on every connection.
pub fn verify_sqlite_vec_once() {
    if SQLITE_VEC_VERIFY_DONE.swap(true, Ordering::SeqCst) {
        return; // already verified earlier in this process
    }

    register_sqlite_vec_extension();

    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match rusqlite::Connection::open(&db_path) {
        Ok(conn) => {
            match conn.query_row("SELECT vec_version()", [], |row| row.get::<_, String>(0)) {
                Ok(version) => {
                    info!(target: "4da::state", version = %version, "sqlite-vec verified (once per process)");
                }
                Err(e) => {
                    warn!(
                        target: "4da::state",
                        error = %e,
                        "sqlite-vec verification query failed — disabling vector search"
                    );
                    SQLITE_VEC_AVAILABLE.store(false, Ordering::Relaxed);
                    crate::capabilities::report_degraded(
                        crate::capabilities::Capability::VectorSearch,
                        "sqlite-vec extension failed to load",
                        "Keyword search only (no vector similarity)",
                    );
                }
            }
        }
        Err(e) => {
            warn!(
                target: "4da::state",
                error = %e,
                "sqlite-vec one-shot verify could not open DB — will retry per-connection on first failure"
            );
            // Reset the flag so a later open_db_connection retries verification
            // (defensive — should not normally happen because pre-Tauri init
            // already ensured the data dir is writable).
            SQLITE_VEC_VERIFY_DONE.store(false, Ordering::SeqCst);
        }
    }
}

/// Register the sqlite-vec extension globally (idempotent).
/// Single source of truth — all code needing sqlite-vec calls this.
///
/// Wraps the unsafe FFI call in `catch_unwind` so a panic in the extension
/// loader cannot crash the entire application. On failure the extension is
/// marked unavailable and the app degrades to keyword-only search.
#[allow(unsafe_code)]
pub fn register_sqlite_vec_extension() {
    let result = std::panic::catch_unwind(|| {
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });

    if let Err(panic_info) = result {
        let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        error!(
            target: "4da::state",
            error = %msg,
            "sqlite-vec extension failed to load — falling back to keyword-only search"
        );
        SQLITE_VEC_AVAILABLE.store(false, Ordering::Relaxed);
    }
}

/// Open a raw SQLite connection with proper configuration.
/// Registers sqlite-vec auto-extension and sets busy_timeout.
/// Use this for ad-hoc connection needs outside the Database struct.
pub(crate) fn open_db_connection() -> Result<rusqlite::Connection> {
    let db_path = get_db_path();

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create data directory {}: {}", parent.display(), e))?;
    }

    register_sqlite_vec_extension();

    let conn = rusqlite::Connection::open(&db_path).context("Failed to open database")?;

    // Match the PRAGMA configuration from Database::new for consistency
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .context("Failed to set connection PRAGMAs")?;

    // Sqlite-vec verification happens ONCE per process via `verify_sqlite_vec_once()`,
    // called from `initialize_pre_tauri()` at startup. We deliberately do NOT verify
    // here because this function is called from 224 callsites across 83 files —
    // logging on every call produced hundreds of identical "sqlite-vec verified"
    // log lines on every cold boot (fixed in the Sovereign Cold Boot architecture).
    //
    // If the one-shot verify failed, `is_vector_search_available()` already returns
    // false and the rest of the app degrades to keyword-only search.

    Ok(conn)
}

// ============================================================================
// Global Database (Lazy Initialized)
// ============================================================================

static DATABASE: OnceCell<Arc<Database>> = OnceCell::new();

pub(crate) fn try_get_database() -> Option<&'static Arc<Database>> {
    DATABASE.get()
}

pub(crate) fn get_database() -> Result<&'static Arc<Database>> {
    DATABASE.get_or_try_init(|| {
        let db_path = get_db_path();

        info!(target: "4da::db", path = ?db_path, "Initializing database");

        // PRE-FLIGHT: try preemptive corruption recovery before opening the
        // connection. This catches subtle corruption (the kind that opens
        // cleanly but fails on read) via PRAGMA quick_check, and restores
        // from a `*.db.backup.vN` sibling if one exists. The result is
        // recorded so `startup_health::check_database` can surface it as a
        // `HealthIssue` on the next frontend poll. The fallback `Database::new`
        // primitive recovery below remains as a belt-and-suspenders safety net
        // for the residual case where preemptive recovery itself failed.
        let recovery = crate::db::migrations::recover_corrupt_db_if_needed(&db_path);
        match &recovery {
            crate::db::migrations::CorruptionRecovery::Healthy
            | crate::db::migrations::CorruptionRecovery::NoExistingDb => {
                // Common path — log nothing, no notice.
            }
            crate::db::migrations::CorruptionRecovery::RestoredFromBackup { restored_from } => {
                tracing::warn!(
                    target: "4da::db",
                    from = %restored_from.display(),
                    "DB was corrupt — restored from backup before open"
                );
            }
            crate::db::migrations::CorruptionRecovery::QuarantinedNoBackup { quarantined_to } => {
                tracing::error!(
                    target: "4da::db",
                    quarantined = %quarantined_to.display(),
                    "DB was corrupt and no backup existed — starting fresh"
                );
            }
            crate::db::migrations::CorruptionRecovery::RecoveryFailed { reason } => {
                tracing::error!(target: "4da::db", %reason, "Preemptive DB recovery failed — falling through to Database::new");
            }
        }
        crate::db::migrations::set_db_recovery_notice(recovery);

        let db = match Database::new(&db_path) {
            Ok(db) => db,
            Err(e) => {
                // Last-resort recovery — Database::new() still failed even after
                // the preemptive pass. Rename the offending file to the legacy
                // single-slot `.db.corrupt` name and create a fresh DB. This
                // path only fires for edge cases (rusqlite open errors that
                // PRAGMA quick_check didn't catch) and intentionally uses a
                // different filename pattern from the new recovery so both
                // artifacts can coexist for support investigation.
                tracing::warn!(
                    target: "4da::db",
                    error = %e,
                    "Database open failed after preemptive recovery — last-resort fallback"
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

        info!(target: "4da::db", "Database ready");
        Ok(Arc::new(db))
    })?;

    // Register all sources at startup (enables source enable/disable enforcement).
    // MUST be outside get_or_try_init: build_all_sources() → load_*_from_settings()
    // calls get_database() internally (for circuit-breaker checks), which would
    // re-enter the OnceCell and deadlock.
    static SOURCES_REGISTERED: std::sync::Once = std::sync::Once::new();
    let db = DATABASE.get().expect("database just initialized");
    SOURCES_REGISTERED.call_once(|| {
        for source in crate::sources::build_all_sources() {
            db.register_source(source.source_type(), source.name()).ok();
        }
    });

    Ok(db)
}

// ============================================================================
// Global Context Engine (Lazy Initialized)
// ============================================================================

static CONTEXT_ENGINE: Lazy<parking_lot::RwLock<Option<Arc<ContextEngine>>>> =
    Lazy::new(|| parking_lot::RwLock::new(None));

fn init_context_engine() -> Result<Arc<ContextEngine>> {
    let conn = open_db_connection()?;
    let engine = ContextEngine::new(Arc::new(parking_lot::Mutex::new(conn)))
        .context("Failed to initialize context engine")?;
    info!(target: "4da::context", "Context engine initialized");
    Ok(Arc::new(engine))
}

pub(crate) fn get_context_engine() -> Result<Arc<ContextEngine>> {
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

fn init_ace_engine() -> Result<Arc<parking_lot::RwLock<ace::ACE>>> {
    let conn = open_db_connection()?;

    let engine = ace::ACE::new(Arc::new(parking_lot::Mutex::new(conn)))
        .context("Failed to initialize ACE")?;

    info!(target: "4da::ace", "Autonomous Context Engine ready");
    Ok(Arc::new(parking_lot::RwLock::new(engine)))
}

pub(crate) fn get_ace_engine() -> Result<parking_lot::RwLockReadGuard<'static, ace::ACE>> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.read())
}

pub(crate) fn get_ace_engine_mut() -> Result<parking_lot::RwLockWriteGuard<'static, ace::ACE>> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.write())
}

/// Get detected languages from ACE engine for curated feed suggestions.
/// Returns empty vec if ACE is not initialized or has no data.
pub(crate) fn get_ace_detected_languages() -> Vec<String> {
    let engine = match get_ace_engine() {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    match engine.get_detected_tech() {
        Ok(techs) => techs
            .into_iter()
            .filter(|t| matches!(t.category, crate::ace::TechCategory::Language))
            .map(|t| t.name)
            .collect(),
        Err(_) => vec![],
    }
}

// ============================================================================
// Global Source Registry (Lazy Initialized)
// ============================================================================

static SOURCE_REGISTRY: OnceCell<Mutex<SourceRegistry>> = OnceCell::new();

pub(crate) fn get_source_registry() -> &'static Mutex<SourceRegistry> {
    SOURCE_REGISTRY.get_or_init(|| {
        info!(target: "4da::sources", "Initializing source registry");
        let mut registry = SourceRegistry::new();

        // Single source of truth: build_all_sources() is THE ONLY place
        // sources are instantiated. Adding a new source = one line there.
        for source in crate::sources::build_all_sources() {
            registry.register(source);
        }

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
            near_misses: None,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        // After successful registration, the extension should be marked available
        assert!(is_vector_search_available());
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result: String = conn
            .query_row("SELECT vec_version()", [], |row| row.get(0))
            .unwrap();
        assert!(!result.is_empty());
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
}
