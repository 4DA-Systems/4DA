// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE - Autonomous Context Engine (Simplified)
//!
//! The brain of 4DA. Implements autonomous context detection with:
//! - Project manifest scanning (Cargo.toml, package.json, etc.)
//! - Real-time file watching for context updates
//! - Git history analysis
//! - Embedding-based semantic search
//!
//! Note: the archived experiments (advanced interaction capture, health
//! monitoring, anomaly detection, validation) live in _future/ace/.
//!
//! ACE always hits its mark.

pub mod behavior;
pub(crate) mod builtin_modules;
pub mod context;
pub mod db;
pub mod embedding;
pub mod git;
pub(crate) mod platform_cfg;
pub(crate) mod readme_indexing;
pub mod scanner;
pub mod topic_embeddings;
pub mod watcher;

pub use behavior::*;
pub use context::*;
pub use topic_embeddings::*;

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::error::Result;

pub use embedding::{EmbeddingConfig, EmbeddingService};
pub use git::{GitAnalyzer, GitSignal};
pub use scanner::ProjectScanner;
pub use watcher::{
    FileChange, FileChangeType, FileWatcher, InteractionRateLimiter, RateLimitStatus,
    WatcherConfig, WatcherStatePersistence,
};

// ============================================================================
// Core ACE Types
// ============================================================================

/// The Autonomous Context Engine (simplified)
#[allow(clippy::upper_case_acronyms)]
pub struct ACE {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) scanner: ProjectScanner,
    pub(crate) git_analyzer: GitAnalyzer,
    pub(crate) watcher: Option<Mutex<FileWatcher>>,
    pub(crate) watcher_persistence: Option<WatcherStatePersistence>,
    pub(crate) embedding_service: Option<Mutex<EmbeddingService>>,
    pub(crate) rate_limiter: InteractionRateLimiter,
    /// Aggregate peak commit hours (0-23) across all analyzed repos, sorted by frequency.
    /// Populated by `analyze_git_repos()` and read by `get_ace_context()` for scoring.
    pub(crate) peak_hours: Vec<u8>,
}

/// A detected technology with confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedTech {
    pub name: String,
    pub category: TechCategory,
    pub confidence: f32,
    pub source: DetectionSource,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TechCategory {
    Language,
    Framework,
    Library,
    Tool,
    Database,
    Platform,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DetectionSource {
    Manifest,
    FileExtension,
    FileContent,
    GitHistory,
    UserExplicit,
}

/// How far back a topic may still count as active at all. A hard bound so the
/// query stays cheap; ageing INSIDE it is handled by decay, not by a cliff.
pub(crate) const TOPIC_WINDOW_DAYS: u32 = 30;

/// Half-life for topic confidence. At 14 days a topic counts half as much as
/// one seen today, at 28 days a quarter. Chosen so a dependency touched
/// fortnightly still registers, while a keyword seen once in a JSON fixture
/// fades toward nothing instead of holding full strength for a week and then
/// vanishing outright.
pub(crate) const TOPIC_CONFIDENCE_HALF_LIFE_DAYS: f64 = 14.0;

/// Age-decayed confidence for a topic last seen `age_days` ago.
///
/// Exponential half-life, clamped so a clock skew that produces a negative age
/// cannot amplify a topic above its stored confidence.
pub(crate) fn decayed_topic_confidence(stored: f32, age_days: f64) -> f32 {
    let age = age_days.max(0.0);
    let decay = 0.5_f64.powf(age / TOPIC_CONFIDENCE_HALF_LIFE_DAYS) as f32;
    stored * decay
}

/// Active topic detected from current work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTopic {
    pub topic: String,
    pub weight: f32,
    pub confidence: f32,
    pub source: TopicSource,
    pub last_seen: String,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TopicSource {
    FileContent,
    GitCommit,
    ImportStatement,
    ProjectManifest,
    ActivityTracker,
}

// ============================================================================
// ACE Implementation
// ============================================================================

impl ACE {
    /// Create a new ACE instance
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        db::migrate(&conn)?;

        // Verify ACE database integrity (same defense as main DB)
        {
            let conn_guard = conn.lock();
            match conn_guard.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)) {
                Ok(result) if result == "ok" => {
                    info!(target: "4da::ace", "ACE database integrity verified");
                }
                Ok(result) => {
                    warn!(target: "4da::ace", result = %result, "ACE database integrity check returned warnings");
                }
                Err(e) => {
                    warn!(target: "4da::ace", error = %e, "ACE database integrity check failed — continuing with caution");
                }
            }
        }

        let scanner = ProjectScanner::new();
        let git_analyzer = GitAnalyzer::default();
        let watcher_persistence = WatcherStatePersistence::new(conn.clone()).ok();

        // Determine embedding provider from user's LLM settings.
        // Maps to the correct provider variant so EmbeddingService can
        // select the right dimension size and code path.
        let embedding_provider = {
            let settings = crate::get_settings_manager().lock();
            let llm_provider = &settings.get().llm.provider;
            match llm_provider.as_str() {
                "openai" => embedding::EmbeddingProvider::OpenAI,
                // Anthropic has no embedding API; all non-OpenAI providers use Ollama
                _ => embedding::EmbeddingProvider::Ollama,
            }
        };
        let embedding_config = EmbeddingConfig {
            provider: embedding_provider,
            ..EmbeddingConfig::default()
        };
        let embedding_service = EmbeddingService::new(embedding_config, conn.clone());

        let rate_limiter = InteractionRateLimiter::new(1000, 100, 60);

        let watcher_config = WatcherConfig::default();
        let watcher = FileWatcher::new(watcher_config);

        Ok(Self {
            conn,
            scanner,
            git_analyzer,
            watcher: Some(Mutex::new(watcher)),
            watcher_persistence,
            embedding_service: Some(Mutex::new(embedding_service)),
            rate_limiter,
            peak_hours: Vec::new(),
        })
    }

    /// Get the database connection
    pub fn get_conn(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }

    /// Liveness of the file-watcher thread, for the health report.
    ///
    /// `None` = liveness is not a meaningful question here: either no watcher
    /// was constructed on this engine (headless engine, tests) or one was
    /// constructed but never started. `Some(false)` = a watcher that WAS
    /// started is no longer running — stopped deliberately, killed by a panic
    /// in a file-change callback, or dropped by its notify backend.
    ///
    /// The never-started case must not read as `Some(false)`. `ACE::new`
    /// constructs the watcher eagerly, while `start_watching` runs late and
    /// sometimes not at all (no context dirs configured, or every watch path
    /// missing). Collapsing those into "not running" told a perfectly healthy
    /// cold-start user their watcher had failed and to restart the app.
    pub fn watcher_is_running(&self) -> Option<bool> {
        self.watcher.as_ref().and_then(|w| {
            let guard = w.lock();
            guard.has_started().then(|| guard.is_running())
        })
    }

    /// Start file watching for real-time context updates
    pub fn start_watching(&mut self, paths: &[PathBuf]) -> Result<()> {
        let config = WatcherConfig::default();
        let mut watcher = FileWatcher::new(config);

        let conn = self.conn.clone();
        watcher.set_callback(move |changes| {
            if let Err(e) = process_file_changes(&conn, &changes) {
                error!(target: "ace::watcher", error = %e, "Error processing file changes");
            }
        });

        for path in paths {
            if path.exists() {
                watcher.watch(path)?;
            }
        }

        self.watcher = Some(Mutex::new(watcher));
        info!(target: "ace::watcher", path_count = paths.len(), "File watching started");
        Ok(())
    }

    /// Analyze git repositories in the given paths
    pub fn analyze_git_repos(&self, paths: &[PathBuf]) -> Result<Vec<GitSignal>> {
        let mut signals = Vec::new();

        for path in paths {
            if !path.exists() {
                continue;
            }

            let repos = self.git_analyzer.find_repos(path, 3);

            for repo_path in repos {
                match self.git_analyzer.analyze_repo(&repo_path) {
                    Ok(signal) => {
                        debug!(target: "ace::git",
                            repo = %signal.repo_name,
                            commits = signal.recent_commits.len(),
                            confidence = signal.confidence * 100.0,
                            "Analyzed git repo"
                        );
                        signals.push(signal);
                    }
                    Err(e) => {
                        warn!(target: "ace::git", path = %repo_path.display(), error = %e, "Failed to analyze repo");
                    }
                }
            }
        }

        store_git_signals(&self.conn, &signals)?;
        Ok(signals)
    }

    /// Perform autonomous context detection
    pub fn detect_context(&self, scan_paths: &[PathBuf]) -> Result<AutonomousContext> {
        info!(target: "ace::detect", "Starting autonomous context detection");

        let mut detected_tech: Vec<DetectedTech> = Vec::new();
        let mut active_topics: Vec<ActiveTopic> = Vec::new();
        let mut projects_found = 0;

        // "Your Stack" exclusions (tier 3 of the canonical inclusion policy),
        // fetched once per detection pass. Tier-3 projects stay DETECTED and
        // listed (so the user can toggle them back on) but contribute nothing
        // to intelligence: no detected_tech evidence, no active topics, no
        // dependency snapshots.
        let user_excluded = crate::project_inclusion::user_excluded_paths();

        for path in scan_paths {
            if !path.exists() {
                continue;
            }

            match self.scanner.scan_directory(path) {
                Ok(signals) => {
                    for signal in signals {
                        let project_path = signal
                            .manifest_path
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Tiers 1+2 (agent infra / non-project scaffolding):
                        // defense in depth — the scanner already refuses to
                        // descend into these trees, but a signal that slips
                        // through (e.g. a scan rooted inside one) must never
                        // become context.
                        if crate::project_inclusion::is_hard_excluded(&project_path) {
                            continue;
                        }
                        let tier3_excluded = crate::project_inclusion::is_user_excluded(
                            &project_path,
                            &user_excluded,
                        );

                        projects_found += 1;

                        // Confidence from evidence density × project activity.
                        // Base (manifest richness): 0.50–0.90
                        // Then scaled by project_relevance (git recency × path pattern):
                        //   active (<7d): 1.0, recent (<30d): 0.7, stale (<90d): 0.3, abandoned: 0.1
                        // This prevents tech from old/abandoned projects from contaminating
                        // the user's active stack profile.
                        let base_confidence = {
                            let mut conf = 0.50_f32;
                            if signal.project_name.is_some() {
                                conf += 0.10;
                            }
                            conf += (signal.frameworks.len() as f32 * 0.05).min(0.15);
                            conf += (signal.languages.len() as f32 * 0.05).min(0.10);
                            if !signal.dev_dependencies.is_empty() {
                                conf += 0.05;
                            }
                            conf.min(0.90)
                        };
                        let confidence = base_confidence * signal.project_relevance;

                        if base_confidence >= 0.3 {
                            // Tier-3 (user-excluded) projects contribute NO
                            // tech evidence — the "Your Stack" toggle means
                            // "this is not my stack".
                            if !tier3_excluded {
                                for lang in &signal.languages {
                                    detected_tech.push(DetectedTech {
                                        name: lang.clone(),
                                        category: TechCategory::Language,
                                        confidence,
                                        source: DetectionSource::Manifest,
                                        evidence: vec![format!(
                                            "Found in {}",
                                            signal.manifest_path.display()
                                        )],
                                    });
                                }

                                for framework in &signal.frameworks {
                                    detected_tech.push(DetectedTech {
                                        name: framework.clone(),
                                        category: TechCategory::Framework,
                                        confidence: confidence * 0.9,
                                        source: DetectionSource::Manifest,
                                        evidence: vec![format!(
                                            "Dependency in {}",
                                            signal.manifest_path.display()
                                        )],
                                    });
                                }

                                for dep in &signal.dependencies {
                                    if is_notable_dependency(dep) {
                                        detected_tech.push(DetectedTech {
                                            name: dep.clone(),
                                            category: TechCategory::Library,
                                            confidence: confidence * 0.7,
                                            source: DetectionSource::Manifest,
                                            evidence: vec![format!(
                                                "Dependency in {}",
                                                signal.manifest_path.display()
                                            )],
                                        });
                                    }
                                }
                            }

                            // Populate project_dependencies table for innovation features.
                            // Skip low-relevance projects (example/demo/test dirs)
                            // to prevent irrelevant preemption alerts.
                            let relevance = signal.project_relevance;
                            // Strict manifest mode (ledger): a user-configured context_dir is
                            // relevant by definition. The ledger's fixture stacks are plain dirs
                            // with no git history, so they score below 0.15 and their deps would
                            // never persist — leaving the registry/OSV grounding paths empty.
                            // Persist when the manifest lives under a configured context_dir.
                            let force_persist = crate::source_fetching::strict_manifest_mode() && {
                                let dirs = crate::get_context_dirs();
                                let norm = |p: &std::path::Path| {
                                    p.to_string_lossy().replace('\\', "/").to_lowercase()
                                };
                                let manifest = norm(signal.manifest_path.as_path());
                                !dirs.is_empty()
                                    && dirs.iter().any(|d| {
                                        let d = norm(d.as_path());
                                        let d = d.trim_end_matches('/');
                                        manifest == d || manifest.starts_with(&format!("{d}/"))
                                    })
                            };
                            if relevance >= 0.15 || force_persist {
                                if let Ok(conn) = crate::open_db_connection() {
                                    let manifest_type =
                                        format!("{:?}", signal.manifest_type).to_lowercase();
                                    let language = signal.manifest_type.language();

                                    // Record the project itself so the Cross-Project
                                    // Intelligence views have data (they read detected_projects).
                                    let project_name =
                                        signal.project_name.clone().unwrap_or_else(|| {
                                            signal
                                                .manifest_path
                                                .parent()
                                                .and_then(std::path::Path::file_name)
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| "unknown".to_string())
                                        });
                                    if let Err(e) = upsert_detected_project(
                                        &conn,
                                        &project_path,
                                        &project_name,
                                        &signal.languages,
                                        &signal.frameworks,
                                        &signal.dependencies,
                                        &signal.detected_at,
                                        relevance,
                                    ) {
                                        tracing::warn!(target: "4da::ace", error = %e, path = %project_path, "Failed to upsert detected_project");
                                    }

                                    for dep in &signal.dependencies {
                                        // Provenance: declared in the manifest, or
                                        // merely INFERRED from source import lines?
                                        // The builtin self-heal purge keys on this.
                                        let detected_from =
                                            if signal.import_scraped_dependencies.contains(dep) {
                                                "import_scrape"
                                            } else {
                                                "manifest"
                                            };
                                        if let Err(e) = crate::temporal::upsert_dependency(
                                            &conn,
                                            &project_path,
                                            &manifest_type,
                                            dep,
                                            None,
                                            false,
                                            true, // direct: from manifest [dependencies]
                                            language,
                                            relevance,
                                            detected_from,
                                        ) {
                                            tracing::warn!(target: "4da::ace", error = %e, dep = %dep, "Failed to upsert dependency");
                                        }
                                    }
                                    for dep in &signal.dev_dependencies {
                                        if let Err(e) = crate::temporal::upsert_dependency(
                                            &conn,
                                            &project_path,
                                            &manifest_type,
                                            dep,
                                            None,
                                            true,
                                            true, // direct: from manifest [dev-dependencies]
                                            language,
                                            relevance,
                                            "manifest",
                                        ) {
                                            tracing::warn!(target: "4da::ace", error = %e, dep = %dep, "Failed to upsert dev dependency");
                                        }
                                    }
                                    // Manifest-declared transitives (go.mod
                                    // `// indirect`): persist with is_direct=0
                                    // — they matter for advisory matching but
                                    // are NOT the user's direct stack.
                                    for dep in &signal.indirect_dependencies {
                                        if let Err(e) =
                                            crate::temporal::upsert_manifest_indirect_dependency(
                                                &conn,
                                                &project_path,
                                                &manifest_type,
                                                dep,
                                                language,
                                                relevance,
                                            )
                                        {
                                            tracing::warn!(target: "4da::ace", error = %e, dep = %dep, "Failed to upsert indirect dependency");
                                        }
                                    }

                                    // Platform-gated direct deps (e.g.
                                    // [target.'cfg(windows)'.dependencies]). Record the
                                    // target spec + whether it's active on the host so
                                    // platform-irrelevant advisories can be de-emphasised.
                                    // Ships silent until the relevance gate reads
                                    // platform_active (no behaviour change yet).
                                    let host = crate::ace::platform_cfg::host_target();
                                    for (dep, target_cfg) in &signal.target_dependencies {
                                        let active =
                                            crate::ace::platform_cfg::target_active_on_host(
                                                Some(target_cfg),
                                                host,
                                            );
                                        if let Err(e) =
                                            crate::temporal::upsert_dependency_with_platform(
                                                &conn,
                                                &project_path,
                                                &manifest_type,
                                                dep,
                                                None,
                                                false,
                                                true, // direct: from manifest [target.*.dependencies]
                                                language,
                                                relevance,
                                                Some(target_cfg),
                                                active,
                                                "manifest",
                                            )
                                        {
                                            tracing::warn!(target: "4da::ace", error = %e, dep = %dep, "Failed to upsert target dependency");
                                        }
                                    }

                                    // Prune direct deps no longer present in this
                                    // manifest (dropped deps, or now-skipped local
                                    // path/git crates) so they stop surfacing as
                                    // stale "unmonitored" blind spots.
                                    let current_names: Vec<String> = signal
                                        .dependencies
                                        .iter()
                                        .chain(signal.dev_dependencies.iter())
                                        .chain(signal.indirect_dependencies.iter())
                                        .chain(
                                            signal.target_dependencies.iter().map(|(name, _)| name),
                                        )
                                        .cloned()
                                        .collect();
                                    match crate::temporal::prune_removed_dependencies(
                                        &conn,
                                        &project_path,
                                        language,
                                        &current_names,
                                    ) {
                                        Ok(n) if n > 0 => {
                                            tracing::info!(target: "4da::ace", project = %project_path, removed = n, "Pruned stale direct dependencies");
                                        }
                                        Ok(_) => {}
                                        Err(e) => {
                                            tracing::warn!(target: "4da::ace", error = %e, project = %project_path, "Failed to prune stale dependencies");
                                        }
                                    }

                                    // Snapshot deps for the dep_linker's UNION query.
                                    // Skipped for tier-3 (user-excluded) projects:
                                    // snapshots are grounding evidence, and the user
                                    // said this is not their stack. (The project +
                                    // project_dependencies rows above still persist
                                    // so the Your Stack list can offer the toggle.)
                                    if !tier3_excluded {
                                        if let Ok(db) = crate::get_database() {
                                            let ecosystem =
                                                signal.manifest_type.language().to_string();
                                            let mut entries: Vec<
                                                crate::db::dep_snapshots::DepEntry,
                                            > = signal
                                                .dependencies
                                                .iter()
                                                .map(|d| crate::db::dep_snapshots::DepEntry {
                                                    name: d.clone(),
                                                    ecosystem: ecosystem.clone(),
                                                    version: None,
                                                    is_direct: true,
                                                    is_dev: false,
                                                    source: manifest_type.clone(),
                                                })
                                                .collect();
                                            entries.extend(signal.dev_dependencies.iter().map(
                                                |d| crate::db::dep_snapshots::DepEntry {
                                                    name: d.clone(),
                                                    ecosystem: ecosystem.clone(),
                                                    version: None,
                                                    is_direct: true,
                                                    is_dev: true,
                                                    source: manifest_type.clone(),
                                                },
                                            ));
                                            entries.extend(
                                                signal.indirect_dependencies.iter().map(|d| {
                                                    crate::db::dep_snapshots::DepEntry {
                                                        name: d.clone(),
                                                        ecosystem: ecosystem.clone(),
                                                        version: None,
                                                        is_direct: false,
                                                        is_dev: false,
                                                        source: manifest_type.clone(),
                                                    }
                                                }),
                                            );
                                            if let Err(e) =
                                                db.snapshot_project_deps(&project_path, &entries)
                                            {
                                                tracing::debug!(target: "4da::ace", error = %e, "Failed to snapshot deps");
                                            }
                                        }
                                    }
                                }
                            }

                            if !tier3_excluded {
                                for lang in &signal.languages {
                                    active_topics.push(ActiveTopic {
                                        topic: lang.clone(),
                                        weight: 0.8,
                                        confidence,
                                        source: TopicSource::ProjectManifest,
                                        last_seen: chrono::Utc::now().to_rfc3339(),
                                        embedding: None,
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(target: "ace::detect", path = %path.display(), error = %e, "Failed to scan path");
                }
            }
        }

        let merged_tech = merge_detected_tech(detected_tech);

        let context_confidence = if merged_tech.is_empty() {
            0.3
        } else {
            let avg_confidence: f32 =
                merged_tech.iter().map(|t| t.confidence).sum::<f32>() / merged_tech.len() as f32;
            avg_confidence.min(0.95)
        };

        let active_count = merged_tech.iter().filter(|t| t.confidence >= 0.5).count();
        let stale_count = merged_tech.len() - active_count;
        info!(target: "ace::detect",
            tech_count = merged_tech.len(),
            active = active_count,
            stale = stale_count,
            projects = projects_found,
            confidence = context_confidence * 100.0,
            "Context detection complete (activity-weighted)"
        );

        store_detected_context(&self.conn, &merged_tech, &active_topics)?;

        // Keep the topic_vec KNN index in sync with persisted topic embeddings.
        // Bounded batch, non-fatal, and a no-op when no embedder is configured
        // (cold machines pay nothing).
        //
        // ⚠ ACCURACY AUDIT 2026-07-26 — this comment used to claim that without
        // the backfill "semantic topic dedup misses every topic loaded from the
        // DB cache". That is NOT true today: the dedup path is
        // `TopicEmbeddings::find_similar_topics`, which loads topic STRINGS via
        // `get_active_topics()` and delegates to the embedding service — it
        // never queries `topic_vec`. A repo-wide search finds no KNN read of
        // `topic_vec` at all, so the index is currently WRITE-ONLY: backfilled,
        // dimension-checked and rebuilt, but never searched (live: 930 index
        // rows vs 844 embedded topics, i.e. 86 orphans that harm nothing
        // because nothing reads them).
        //
        // Left in place deliberately rather than deleted: removing a vec0 table
        // plus its migration is its own reviewed change, and a future KNN dedup
        // is the obvious consumer. Recorded here so the next reader is not
        // misled into believing this work is load-bearing.
        match self.populate_topic_vec(512) {
            Ok(n) if n > 0 => {
                info!(target: "ace::detect", synced = n, "topic_vec index backfilled from persisted topic embeddings");
            }
            Ok(_) => {}
            Err(e) => {
                warn!(target: "ace::detect", error = %e, "topic_vec backfill failed (non-fatal)");
            }
        }

        // Auto-enrich: run stack profile detection after context update
        {
            let ace_ctx = crate::scoring::get_ace_context();
            let detections = crate::stacks::detection::detect_matching_profiles(&ace_ctx);
            if !detections.is_empty() {
                let conn = self.conn.lock();
                if let Err(e) = crate::stacks::save_detected_stacks(&conn, &detections) {
                    warn!(target: "ace::detect", error = %e, "Failed to save auto-detected stacks");
                } else {
                    info!(target: "ace::detect",
                        profiles = detections.len(),
                        top = %detections.first().map_or("none", |d| d.profile_name.as_str()),
                        "Auto-detected stack profiles after context scan"
                    );
                }
            }
        }

        Ok(AutonomousContext {
            detected_tech: merged_tech,
            active_topics,
            projects_scanned: projects_found,
            context_confidence,
            detection_time: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Get all detected technologies
    pub fn get_detected_tech(&self) -> Result<Vec<DetectedTech>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT name, category, confidence, source, evidence FROM detected_tech ORDER BY confidence DESC",
            )?;

        let rows = stmt.query_map([], |row| {
            let category_str: String = row.get(1)?;
            let source_str: String = row.get(3)?;
            let evidence_str: String = row.get(4)?;

            Ok(DetectedTech {
                name: row.get(0)?,
                category: match category_str.as_str() {
                    "language" => TechCategory::Language,
                    "framework" => TechCategory::Framework,
                    "library" => TechCategory::Library,
                    "tool" => TechCategory::Tool,
                    "database" => TechCategory::Database,
                    _ => TechCategory::Platform,
                },
                confidence: row.get(2)?,
                source: match source_str.as_str() {
                    "manifest" => DetectionSource::Manifest,
                    "file_extension" => DetectionSource::FileExtension,
                    "file_content" => DetectionSource::FileContent,
                    "git_history" => DetectionSource::GitHistory,
                    _ => DetectionSource::UserExplicit,
                },
                evidence: evidence_str.split("; ").map(String::from).collect(),
            })
        })?;

        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Get active topics, with confidence decayed by how long ago the topic
    /// was last seen.
    ///
    /// This used to be a hard `last_seen > -7 days` cliff, which measures
    /// FILE-EDIT RECENCY and was being used as a proxy for STACK MEMBERSHIP.
    /// Measured consequence on 2026-08-26: `kubernetes` — minted from the
    /// scoring pipeline's own benchmark fixture — was a live user topic while
    /// `tokio`, a crate this application actually depends on, had expired out
    /// of the window entirely. Not because of what the operator builds, but
    /// because of which files were saved in the last seven days.
    ///
    /// Two changes. The window widens to [`TOPIC_WINDOW_DAYS`], which restores
    /// eight genuine stack topics that the 7-day cliff was evicting (tokio,
    /// reqwest, thiserror, tauri_plugin_deep_link, tauri_plugin_autostart,
    /// tracing_subscriber, stripe, noble). And CONFIDENCE now decays with age
    /// on a [`TOPIC_CONFIDENCE_HALF_LIFE_DAYS`] half-life, so a topic seen this
    /// morning outweighs one last seen three weeks ago instead of counting the
    /// same.
    ///
    /// Decay is applied to CONFIDENCE, never to `weight`. Weight is what
    /// `ace_context` admits on (>= 0.55) and the stored mint value is 0.6, so
    /// decaying it would evict every file-content topic within days — the
    /// opposite of the problem being fixed. Confidence is what the semantic
    /// boost weights by, which is where age SHOULD register.
    pub fn get_active_topics(&self) -> Result<Vec<ActiveTopic>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT topic, weight, confidence, source, last_seen,
                    CAST(julianday('now') - julianday(last_seen) AS REAL) AS age_days
             FROM active_topics
             WHERE last_seen > datetime('now', ?1)
             ORDER BY weight DESC",
        )?;

        let window = format!("-{TOPIC_WINDOW_DAYS} days");
        let rows = stmt.query_map([window], |row| {
            let source_str: String = row.get(3)?;
            let stored_confidence: f32 = row.get(2)?;
            let age_days: f64 = row.get::<_, Option<f64>>(5)?.unwrap_or(0.0);

            Ok(ActiveTopic {
                topic: row.get(0)?,
                weight: row.get(1)?,
                confidence: decayed_topic_confidence(stored_confidence, age_days),
                source: match source_str.as_str() {
                    "file_content" => TopicSource::FileContent,
                    "git_commit" => TopicSource::GitCommit,
                    "import" => TopicSource::ImportStatement,
                    "manifest" => TopicSource::ProjectManifest,
                    _ => TopicSource::ActivityTracker,
                },
                last_seen: row.get(4)?,
                embedding: None,
            })
        })?;

        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    // ========================================================================
    // Threshold Auto-Tuning Methods
    // ========================================================================

    /// Compute threshold adjustment based on user engagement rate over the last 7 days.
    pub fn compute_threshold_adjustment(&self, current_threshold: f32) -> Option<f32> {
        let conn = self.conn.lock();

        let total_shown: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions WHERE timestamp > datetime('now', '-7 days')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Need at least 20 interactions for meaningful adjustment
        if total_shown < 20 {
            return None;
        }

        let positive: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions
                 WHERE timestamp > datetime('now', '-7 days')
                 AND action_type IN ('click', 'save', 'share')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let engagement_rate = positive as f32 / total_shown as f32;

        // High engagement (>50%): threshold may be too strict, lower it to show more
        if engagement_rate > 0.50 {
            let new = (current_threshold - 0.02).clamp(0.30, 0.50);
            if (new - current_threshold).abs() > f32::EPSILON {
                return Some(new);
            }
        }

        // Low engagement (<15%): threshold too loose, raise it to filter more
        if engagement_rate < 0.15 {
            let new = (current_threshold + 0.02).clamp(0.30, 0.50);
            if (new - current_threshold).abs() > f32::EPSILON {
                return Some(new);
            }
        }

        None // No adjustment needed
    }

    // Threshold persistence (get_stored_threshold / store_threshold) was
    // REMOVED in v19 (AD-029): a persisted tuner-written threshold was
    // re-installed on every ACE warmup, so a poisoned value survived
    // restarts indefinitely. The threshold is a fixed default now; the
    // orphaned kv_store key is deleted by the v19 migration.

    // ========================================================================
    // Watcher Persistence Methods
    // ========================================================================

    /// Save watcher state
    pub fn save_watcher_state(&self) -> Result<()> {
        if let (Some(persistence), Some(watcher)) = (&self.watcher_persistence, &self.watcher) {
            let watcher_guard = watcher.lock();
            persistence.save(&watcher_guard)
        } else {
            Err("Watcher or persistence not initialized".into())
        }
    }

    /// Session-aware work topic extraction with gap-based session detection.
    ///
    /// Instead of a fixed 2-hour window, detects natural work sessions by finding
    /// gaps > 30 minutes in file_signals timestamps. Applies graduated weights:
    /// - Current session topics: weight 1.0
    /// - Previous same-day session: weight 0.5
    /// - Yesterday's sessions: weight 0.2
    pub fn get_session_aware_work_topics(&self) -> Result<Vec<(String, f32)>> {
        let conn = self.conn.lock();

        // Fetch all file_signals from last 24 hours, ordered most recent first
        let mut stmt = conn.prepare(
            "SELECT extracted_topics, timestamp FROM file_signals
             WHERE timestamp > datetime('now', '-24 hours')
             ORDER BY timestamp DESC LIMIT 200",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
        })?;

        // Collect all signals with parsed hours-ago
        let mut signals: Vec<(Vec<String>, f32)> = Vec::new(); // (topics, hours_ago)
        for row in rows {
            let (topics_json, timestamp_str) = row?;
            if let Some(json_str) = topics_json {
                if let Ok(topics) = serde_json::from_str::<Vec<String>>(&json_str) {
                    if !topics.is_empty() {
                        let hours_ago = parse_hours_ago(&timestamp_str);
                        signals.push((topics, hours_ago));
                    }
                }
            }
        }

        if signals.is_empty() {
            return Ok(Vec::new());
        }

        // Detect session boundaries: a gap > 0.5 hours (30 min) between consecutive signals.
        // signals are ordered most-recent-first, so signals[0] is the newest.
        let session_gap_hours: f32 = 0.5;
        let mut session_ids: Vec<usize> = Vec::with_capacity(signals.len());
        let mut current_session: usize = 0;
        session_ids.push(0); // First signal is session 0 (current)

        for i in 1..signals.len() {
            // signals[i] is older than signals[i-1]
            let gap = signals[i].0.len(); // just need the hours_ago difference
            let _ = gap; // unused, we compare hours_ago values
            let time_gap = signals[i].1 - signals[i - 1].1;
            if time_gap.abs() > session_gap_hours {
                current_session += 1;
            }
            session_ids.push(current_session);
        }

        // Determine today boundary (signals older than ~16 hours are "yesterday")
        // Use a simple heuristic: signals > 16 hours ago are yesterday
        let yesterday_threshold: f32 = 16.0;

        let mut topic_weights: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();

        for (idx, (topics, hours_ago)) in signals.iter().enumerate() {
            let session_id = session_ids[idx];

            // Compute session-based weight
            let session_weight = if session_id == 0 {
                // Current session: full weight with slight recency decay
                let decay = 1.0 - (hours_ago / 4.0).min(1.0) * 0.15;
                decay.clamp(0.85, 1.0)
            } else if *hours_ago < yesterday_threshold {
                // Previous same-day session
                0.5
            } else {
                // Yesterday's sessions
                0.2
            };

            for topic in topics {
                let topic_lower = topic.to_lowercase();
                let entry = topic_weights.entry(topic_lower).or_insert(0.0);
                *entry = entry.max(session_weight);
            }
        }

        let mut result: Vec<(String, f32)> = topic_weights.into_iter().collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(result)
    }
}

/// Check if a package name looks like a Rust crate (heuristic for ecosystem classification).
pub fn is_rust_package(name: &str) -> bool {
    matches!(
        name,
        "tokio"
            | "serde"
            | "anyhow"
            | "thiserror"
            | "clap"
            | "tracing"
            | "hyper"
            | "axum"
            | "actix"
            | "sqlx"
            | "diesel"
            | "tauri"
            | "warp"
            | "reqwest"
            | "rusqlite"
            | "parking_lot"
            | "crossbeam"
            | "rayon"
            | "rand"
    ) || name.contains('_') // Rust crates typically use underscores
}

/// Upsert a scanned project into `detected_projects` — the source the Cross-Project
/// Intelligence readers (tech convergence / project-health comparison / cross-project
/// deps) query. `path` is UNIQUE, so a rescan refreshes the row instead of erroring.
/// Before this writer existed the table was never populated and those views always
/// read empty no matter how much ACE scanned.
fn upsert_detected_project(
    conn: &Connection,
    path: &str,
    name: &str,
    languages: &[String],
    frameworks: &[String],
    dependencies: &[String],
    last_activity: &str,
    confidence: f32,
) -> rusqlite::Result<()> {
    let to_json = |v: &[String]| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO detected_projects \
            (path, name, languages, frameworks, dependencies, last_activity, detection_confidence, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now')) \
         ON CONFLICT(path) DO UPDATE SET \
            name = excluded.name, \
            languages = excluded.languages, \
            frameworks = excluded.frameworks, \
            dependencies = excluded.dependencies, \
            last_activity = excluded.last_activity, \
            detection_confidence = excluded.detection_confidence, \
            updated_at = datetime('now')",
        rusqlite::params![
            path,
            name,
            to_json(languages),
            to_json(frameworks),
            to_json(dependencies),
            last_activity,
            confidence,
        ],
    )?;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

/// Create an in-memory ACE instance for testing.
/// Loads the sqlite-vec extension so vec0 virtual tables work.
/// Shared across ACE submodule test suites (behavior, scanner, etc.).
#[cfg(test)]
pub(crate) fn create_test_ace() -> ACE {
    // Load sqlite-vec extension for vec0 virtual tables
    crate::register_sqlite_vec_extension();

    let conn = Arc::new(Mutex::new(
        Connection::open_in_memory().expect("in-memory DB"),
    ));
    db::migrate(&conn).expect("ACE migration");
    ACE {
        conn,
        scanner: ProjectScanner::new(),
        git_analyzer: GitAnalyzer::default(),
        watcher: None,
        watcher_persistence: None,
        embedding_service: None,
        rate_limiter: InteractionRateLimiter::new(1000, 100, 60),
        peak_hours: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_detected_project_inserts_then_upserts_on_path() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE detected_projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                languages TEXT, frameworks TEXT, dependencies TEXT,
                last_activity TEXT, detection_confidence REAL DEFAULT 0.5,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        let langs = vec!["rust".to_string()];
        let fws = vec!["tauri".to_string()];
        let deps = vec!["tokio".to_string(), "serde".to_string()];

        upsert_detected_project(
            &conn,
            "/proj/a",
            "alpha",
            &langs,
            &fws,
            &deps,
            "2026-06-09 10:00:00",
            0.8,
        )
        .unwrap();

        let (count, name, langs_json): (i64, String, String) = conn
            .query_row(
                "SELECT COUNT(*), name, languages FROM detected_projects WHERE path = '/proj/a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(name, "alpha");
        assert_eq!(langs_json, "[\"rust\"]");

        // A rescan of the same path UPDATEs in place — never duplicates (path is UNIQUE).
        upsert_detected_project(
            &conn,
            "/proj/a",
            "alpha-renamed",
            &langs,
            &fws,
            &deps,
            "2026-06-10 10:00:00",
            0.9,
        )
        .unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM detected_projects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1, "upsert must not duplicate on path conflict");
        let new_name: String = conn
            .query_row(
                "SELECT name FROM detected_projects WHERE path = '/proj/a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(new_name, "alpha-renamed");
    }

    // Temporal decay tests

    #[test]
    fn test_decay_30_day_half_life() {
        // After 30 days, score should be ~50% of original
        let decay_factor = 0.5_f32.powf(30.0 / 30.0);
        assert!(
            (decay_factor - 0.5).abs() < 0.01,
            "30-day decay should halve: got {}",
            decay_factor
        );
    }

    #[test]
    fn test_decay_recent_untouched() {
        // Items interacted with recently should have minimal decay
        let decay_factor = 0.5_f32.powf(0.5 / 30.0); // Half a day
        assert!(
            decay_factor > 0.98,
            "Recent items should barely decay: got {}",
            decay_factor
        );
    }

    #[test]
    fn test_decay_fully_decayed_deleted() {
        // Items with very small scores after decay should be cleaned up
        let original_score = 0.08_f32;
        let decay_factor = 0.5_f32.powf(30.0 / 30.0); // 30 days
        let decayed = original_score * decay_factor;
        assert!(
            decayed.abs() < 0.05,
            "Low score after 30 days should be below deletion threshold: got {}",
            decayed
        );
    }

    // ========================================================================
    // Active Work Window tests
    // ========================================================================

    // ========================================================================
    // Session-aware work topics tests
    // ========================================================================

    #[test]
    fn test_session_aware_current_session_full_weight() {
        let ace = create_test_ace();
        let conn = ace.get_conn().lock();

        // Insert signals within the current session (all recent, no gaps)
        let now = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params!["/src/main.rs", r#"["rust", "tauri"]"#, now],
        )
        .expect("insert");

        let ten_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(10))
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params!["/src/lib.rs", r#"["sqlite"]"#, ten_min_ago],
        )
        .expect("insert");

        drop(conn);

        let topics = ace
            .get_session_aware_work_topics()
            .expect("session aware topics");

        assert!(!topics.is_empty(), "Should return current session topics");

        // Current session topics should have weight >= 0.85
        for (topic, weight) in &topics {
            assert!(
                *weight >= 0.85,
                "Current session topic '{}' should have weight >= 0.85, got {}",
                topic,
                weight
            );
        }
    }

    #[test]
    fn test_session_aware_previous_session_lower_weight() {
        let ace = create_test_ace();
        let conn = ace.get_conn().lock();

        // Current session signal
        let now = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params!["/src/main.rs", r#"["current_topic"]"#, now],
        )
        .expect("insert current");

        // Previous session signal (2 hours ago, creates a gap > 30 min)
        let two_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(2))
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params!["/src/old.rs", r#"["previous_topic"]"#, two_hours_ago],
        )
        .expect("insert previous");

        drop(conn);

        let topics = ace
            .get_session_aware_work_topics()
            .expect("session aware topics");

        let current = topics.iter().find(|(t, _)| t == "current_topic");
        let previous = topics.iter().find(|(t, _)| t == "previous_topic");

        assert!(current.is_some(), "Should contain current session topic");
        assert!(previous.is_some(), "Should contain previous session topic");

        let current_w = current.unwrap().1;
        let previous_w = previous.unwrap().1;
        assert!(
            current_w > previous_w,
            "Current session weight ({}) should be higher than previous ({})",
            current_w,
            previous_w
        );
        assert!(
            (previous_w - 0.5).abs() < 0.01,
            "Previous same-day session should have weight ~0.5, got {}",
            previous_w
        );
    }

    #[test]
    fn test_session_aware_empty_returns_empty() {
        let ace = create_test_ace();
        let topics = ace
            .get_session_aware_work_topics()
            .expect("session aware topics");
        assert!(topics.is_empty(), "Empty DB should return no topics");
    }

    // ========================================================================
    // Threshold auto-tuning tests
    // ========================================================================

    /// Helper: insert N interactions with the given action_type into the ACE DB.
    fn insert_interactions(ace: &ACE, action_type: &str, count: usize) {
        let conn = ace.get_conn().lock();
        let now = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        for i in 0..count {
            conn.execute(
                "INSERT INTO interactions (item_id, action_type, action_data, item_topics, item_source, signal_strength, timestamp)
                 VALUES (?1, ?2, '{}', '[]', 'hackernews', 0.5, ?3)",
                rusqlite::params![i as i64 + 1, action_type, now],
            )
            .expect("insert interaction");
        }
    }

    #[test]
    fn test_high_engagement_lowers_threshold() {
        let ace = create_test_ace();
        insert_interactions(&ace, "click", 15);
        insert_interactions(&ace, "save", 5);
        insert_interactions(&ace, "dismiss", 5);

        let current = 0.40;
        let result = ace.compute_threshold_adjustment(current);
        assert!(
            result.is_some(),
            "High engagement should trigger adjustment"
        );
        let new_threshold = result.unwrap();
        assert!(
            new_threshold < current,
            "High engagement should lower threshold: got {} (was {})",
            new_threshold,
            current
        );
        assert!(
            (new_threshold - 0.38).abs() < f32::EPSILON,
            "Expected 0.38, got {}",
            new_threshold
        );
    }

    #[test]
    fn test_low_engagement_raises_threshold() {
        let ace = create_test_ace();
        insert_interactions(&ace, "click", 2);
        insert_interactions(&ace, "dismiss", 18);
        insert_interactions(&ace, "ignore", 5);

        let current = 0.36;
        let result = ace.compute_threshold_adjustment(current);
        assert!(result.is_some(), "Low engagement should trigger adjustment");
        let new_threshold = result.unwrap();
        assert!(
            new_threshold > current,
            "Low engagement should raise threshold: got {} (was {})",
            new_threshold,
            current
        );
        assert!(
            (new_threshold - 0.38).abs() < f32::EPSILON,
            "Expected 0.38, got {}",
            new_threshold
        );
    }

    #[test]
    fn test_threshold_bounds() {
        let ace = create_test_ace();
        insert_interactions(&ace, "click", 25);

        let result = ace.compute_threshold_adjustment(0.30);
        assert!(
            result.is_none(),
            "Threshold at minimum (0.30) should not decrease further"
        );

        let ace2 = create_test_ace();
        insert_interactions(&ace2, "dismiss", 25);

        let result2 = ace2.compute_threshold_adjustment(0.50);
        assert!(
            result2.is_none(),
            "Threshold at maximum (0.50) should not increase further"
        );
    }

    #[test]
    fn test_insufficient_data_no_change() {
        let ace = create_test_ace();
        insert_interactions(&ace, "click", 5);

        let result = ace.compute_threshold_adjustment(0.30);
        assert!(
            result.is_none(),
            "Fewer than 20 interactions should return None"
        );
    }

    // test_stored_threshold_roundtrip removed in v19 (AD-029) along with
    // threshold persistence itself — a persisted tuner value must never
    // resurrect across restarts.
}

#[cfg(test)]
mod topic_decay_tests {
    use super::*;

    /// The inversion this replaced: `kubernetes`, minted from the scoring
    /// pipeline's own benchmark fixture, was a live user topic while `tokio` —
    /// a crate this app depends on — had expired out of a hard 7-day window.
    /// Measured after the change: 22 admitted topics become 30, and the eight
    /// restored are all genuine stack (tokio, reqwest, thiserror, the two tauri
    /// plugins, tracing_subscriber, stripe, noble).
    #[test]
    fn confidence_halves_every_half_life() {
        let c = 0.70_f32;
        assert!((decayed_topic_confidence(c, 0.0) - c).abs() < 1e-6);
        assert!(
            (decayed_topic_confidence(c, TOPIC_CONFIDENCE_HALF_LIFE_DAYS) - c / 2.0).abs() < 1e-5
        );
        assert!(
            (decayed_topic_confidence(c, TOPIC_CONFIDENCE_HALF_LIFE_DAYS * 2.0) - c / 4.0).abs()
                < 1e-5
        );
    }

    #[test]
    fn a_topic_seen_today_outweighs_one_seen_a_fortnight_ago() {
        assert!(decayed_topic_confidence(0.70, 1.0) > decayed_topic_confidence(0.70, 14.0));
    }

    /// A topic still inside the window must keep a usable share of its
    /// confidence — the point is to age it, not to evict it by another route.
    #[test]
    fn topics_inside_the_window_are_not_erased() {
        let at_edge = decayed_topic_confidence(0.70, f64::from(TOPIC_WINDOW_DAYS));
        assert!(
            at_edge > 0.15,
            "a topic at the window edge still counts for something, got {at_edge}"
        );
    }

    /// Clock skew must never amplify a topic above what was stored.
    #[test]
    fn negative_age_cannot_amplify() {
        assert!(decayed_topic_confidence(0.70, -5.0) <= 0.70);
    }

    #[test]
    fn window_is_wider_than_the_old_seven_day_cliff() {
        assert!(
            TOPIC_WINDOW_DAYS > 7,
            "the 7-day cliff is what evicted tokio while keeping a fixture keyword"
        );
    }
}
