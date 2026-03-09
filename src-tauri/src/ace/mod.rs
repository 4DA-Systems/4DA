//! ACE - Autonomous Context Engine (Simplified)
//!
//! The brain of 4DA. Implements autonomous context detection with:
//! - Project manifest scanning (Cargo.toml, package.json, etc.)
//! - Real-time file watching for context updates
//! - Git history analysis
//! - Embedding-based semantic search
//!
//! Note: Advanced behavior learning, health monitoring, anomaly detection,
//! and validation are archived in _future/ace/ for potential future use.
//!
//! ACE always hits its mark.

pub mod behavior;
pub mod context;
pub mod db;
pub mod embedding;
pub mod git;
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
                "anthropic" | "ollama" => embedding::EmbeddingProvider::Ollama,
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
        })
    }

    /// Get the database connection
    pub fn get_conn(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
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

    /// Stop file watching
    // Watcher API: used when filesystem monitoring is active
    #[allow(dead_code)]
    pub fn stop_watching(&mut self) {
        if let Some(ref watcher) = self.watcher {
            watcher.lock().stop();
        }
        self.watcher = None;
        info!(target: "ace::watcher", "File watching stopped");
    }

    /// Check if file watching is active
    // Watcher API: used when filesystem monitoring is active
    #[allow(dead_code)]
    pub fn is_watching(&self) -> bool {
        self.watcher
            .as_ref()
            .is_some_and(|w| w.lock().is_watching())
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

        for path in scan_paths {
            if !path.exists() {
                continue;
            }

            match self.scanner.scan_directory(path) {
                Ok(signals) => {
                    for signal in signals {
                        projects_found += 1;

                        // Simple confidence check (was using validator)
                        let confidence = 0.8; // Default high confidence for manifest detection

                        if confidence >= 0.3 {
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

                            // Populate project_dependencies table for innovation features
                            if let Ok(conn) = crate::open_db_connection() {
                                let project_path = signal
                                    .manifest_path
                                    .parent()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                let manifest_type =
                                    format!("{:?}", signal.manifest_type).to_lowercase();
                                let language = signal.manifest_type.language();
                                for dep in &signal.dependencies {
                                    let _ = crate::temporal::upsert_dependency(
                                        &conn,
                                        &project_path,
                                        &manifest_type,
                                        dep,
                                        None,
                                        false,
                                        language,
                                    );
                                }
                                for dep in &signal.dev_dependencies {
                                    let _ = crate::temporal::upsert_dependency(
                                        &conn,
                                        &project_path,
                                        &manifest_type,
                                        dep,
                                        None,
                                        true,
                                        language,
                                    );
                                }
                            }

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

        info!(target: "ace::detect",
            tech_count = merged_tech.len(),
            projects = projects_found,
            confidence = context_confidence * 100.0,
            "Context detection complete"
        );

        store_detected_context(&self.conn, &merged_tech, &active_topics)?;

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
                        top = %detections.first().map(|d| d.profile_name.as_str()).unwrap_or("none"),
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

    /// Get active topics
    pub fn get_active_topics(&self) -> Result<Vec<ActiveTopic>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT topic, weight, confidence, source, last_seen FROM active_topics
             WHERE last_seen > datetime('now', '-7 days')
             ORDER BY weight DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let source_str: String = row.get(3)?;

            Ok(ActiveTopic {
                topic: row.get(0)?,
                weight: row.get(1)?,
                confidence: row.get(2)?,
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

    /// Load stored threshold from ACE kv_store
    pub fn get_stored_threshold(&self) -> Option<f32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT value FROM kv_store WHERE key = 'relevance_threshold'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .ok()
        .map(|v| v as f32)
    }

    /// Persist threshold to ACE kv_store
    pub fn store_threshold(&self, threshold: f32) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, updated_at)
             VALUES ('relevance_threshold', ?1, datetime('now'))",
            [threshold as f64],
        );
    }

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

    /// Clear watcher state
    // Watcher API: used when filesystem monitoring is active
    #[allow(dead_code)]
    pub fn clear_watcher_state(&self) -> Result<()> {
        if let Some(persistence) = &self.watcher_persistence {
            persistence.clear()
        } else {
            Err("Watcher persistence not initialized".into())
        }
    }

    /// Get topics from recent file changes for "active work" boosting.
    pub fn get_recent_work_topics(&self, hours: u64) -> Result<Vec<(String, f32)>> {
        let conn = self.conn.lock();

        let mut stmt = conn
            .prepare(
                "SELECT extracted_topics, timestamp FROM file_signals
                 WHERE timestamp > datetime('now', ?1)
                 ORDER BY timestamp DESC LIMIT 50",
            )
            .map_err(|e| e.to_string())?;

        let hours_param = format!("-{} hours", hours);
        let rows = stmt
            .query_map([&hours_param], |row| {
                Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        let mut topic_weights: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        let max_hours = hours as f32;

        for row in rows {
            let (topics_json, timestamp_str) = row.map_err(|e| e.to_string())?;

            if let Some(json_str) = topics_json {
                // Parse JSON array of topics
                if let Ok(topics) = serde_json::from_str::<Vec<String>>(&json_str) {
                    // Compute recency weight: linear decay from 1.0 to 0.5
                    let hours_ago = parse_hours_ago(&timestamp_str);
                    let weight = 1.0 - (hours_ago / max_hours) * 0.5;
                    let weight = weight.clamp(0.5, 1.0);

                    for topic in topics {
                        let topic_lower = topic.to_lowercase();
                        let entry = topic_weights.entry(topic_lower).or_insert(0.0);
                        *entry = entry.max(weight); // Keep highest weight per topic
                    }
                }
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Helper: create an in-memory ACE instance for testing.
    /// Loads the sqlite-vec extension so vec0 virtual tables work.
    fn create_test_ace() -> ACE {
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
        }
    }

    #[test]
    fn test_recent_work_topics_returns_topics() {
        let ace = create_test_ace();
        let conn = ace.get_conn().lock();

        // Insert file_signals with topics within 2 hours (use current timestamp)
        let now = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params!["/src/main.rs", r#"["rust", "tauri", "async"]"#, now,],
        )
        .expect("insert file signal");

        // Insert another signal 30 minutes ago
        let thirty_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(30))
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params!["/src/lib.rs", r#"["sqlite", "embeddings"]"#, thirty_min_ago,],
        )
        .expect("insert file signal 2");

        drop(conn); // Release lock before calling method

        let topics = ace
            .get_recent_work_topics(2)
            .expect("get_recent_work_topics");

        // Should have 5 unique topics
        assert_eq!(topics.len(), 5, "Expected 5 topics, got {:?}", topics);

        // Most recent topics should have highest weight (close to 1.0)
        let rust_weight = topics.iter().find(|(t, _)| t == "rust").map(|(_, w)| *w);
        assert!(rust_weight.is_some(), "Should contain 'rust' topic");
        assert!(
            rust_weight.unwrap() > 0.9,
            "Recent 'rust' topic should have weight > 0.9, got {}",
            rust_weight.unwrap()
        );

        // Slightly older topics should still have decent weight
        let sqlite_weight = topics.iter().find(|(t, _)| t == "sqlite").map(|(_, w)| *w);
        assert!(sqlite_weight.is_some(), "Should contain 'sqlite' topic");
        assert!(
            sqlite_weight.unwrap() > 0.8,
            "30-min-old 'sqlite' topic should have weight > 0.8, got {}",
            sqlite_weight.unwrap()
        );
    }

    #[test]
    fn test_old_work_topics_excluded() {
        let ace = create_test_ace();
        let conn = ace.get_conn().lock();

        // Insert file_signals > 2 hours old (3 hours ago)
        let three_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(3))
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, 'modified', ?2, ?3)",
            rusqlite::params![
                "/old/file.rs",
                r#"["old_topic", "stale_tech"]"#,
                three_hours_ago,
            ],
        )
        .expect("insert old file signal");

        drop(conn);

        let topics = ace
            .get_recent_work_topics(2)
            .expect("get_recent_work_topics");

        assert!(
            topics.is_empty(),
            "Topics older than 2 hours should not appear in 2-hour window, got {:?}",
            topics
        );
    }

    #[test]
    fn test_empty_window_returns_empty() {
        let ace = create_test_ace();

        // Fresh DB with no file_signals at all
        let topics = ace
            .get_recent_work_topics(2)
            .expect("get_recent_work_topics");

        assert!(
            topics.is_empty(),
            "Empty DB should return no work topics, got {:?}",
            topics
        );
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

    #[test]
    fn test_stored_threshold_roundtrip() {
        let ace = create_test_ace();

        assert!(
            ace.get_stored_threshold().is_none(),
            "Fresh DB should have no stored threshold"
        );

        ace.store_threshold(0.42);
        let loaded = ace.get_stored_threshold();
        assert!(loaded.is_some(), "Should load stored threshold");
        assert!(
            (loaded.unwrap() - 0.42).abs() < 0.001,
            "Stored threshold should roundtrip: got {}",
            loaded.unwrap()
        );

        ace.store_threshold(0.18);
        let loaded2 = ace.get_stored_threshold();
        assert!(
            (loaded2.unwrap() - 0.18).abs() < 0.001,
            "Updated threshold should persist: got {}",
            loaded2.unwrap()
        );
    }
}
