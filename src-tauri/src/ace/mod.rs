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

pub mod db;
pub mod embedding;
pub mod git;
pub mod scanner;
pub mod watcher;

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub use embedding::{EmbeddingConfig, EmbeddingService};
pub use git::{GitAnalyzer, GitSignal};
pub use scanner::ProjectScanner;
pub use watcher::{
    FileChange, FileChangeType, FileWatcher, InteractionRateLimiter, RateLimitStatus,
    WatcherConfig, WatcherStatePersistence,
};

// ============================================================================
// Behavior Types
// ============================================================================

/// Types of user behavior we track
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BehaviorAction {
    Click { dwell_time_seconds: u64 },
    Save,
    Share,
    Dismiss,
    MarkIrrelevant,
    Scroll { visible_seconds: f32 },
    Ignore,
}

impl BehaviorAction {
    pub fn compute_strength(&self) -> f32 {
        match self {
            BehaviorAction::Click { dwell_time_seconds } => {
                let base = 0.5;
                let dwell_bonus = (*dwell_time_seconds as f32 / 60.0).min(0.5);
                base + dwell_bonus
            }
            BehaviorAction::Save => 1.0,
            BehaviorAction::Share => 1.0,
            BehaviorAction::Dismiss => -0.8,
            BehaviorAction::MarkIrrelevant => -1.0,
            BehaviorAction::Scroll { visible_seconds } => 0.1 * visible_seconds.min(3.0),
            BehaviorAction::Ignore => -0.1,
        }
    }
}

/// Behavior signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSignal {
    pub item_id: i64,
    pub action: BehaviorAction,
    pub timestamp: String,
    pub item_topics: Vec<String>,
    pub item_source: String,
    pub signal_strength: f32,
}

/// Topic affinity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicAffinity {
    pub topic: String,
    pub embedding: Option<Vec<f32>>,
    pub positive_signals: u32,
    pub negative_signals: u32,
    pub total_exposures: u32,
    pub affinity_score: f32,
    pub confidence: f32,
    pub last_interaction: String,
    pub decay_applied: bool,
}

/// Anti-topic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiTopic {
    pub topic: String,
    pub rejection_count: u32,
    pub confidence: f32,
    pub auto_detected: bool,
    pub user_confirmed: bool,
    pub first_rejection: String,
    pub last_rejection: String,
}

/// Source preference (stub for API compatibility)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePreference {
    pub source: String,
    pub score: f32,
    pub interactions: u32,
}

/// Learned behavior (stub for API compatibility)
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearnedBehavior {
    pub interests: Vec<String>,
    pub anti_topics: Vec<String>,
}

/// Activity patterns (stub for API compatibility)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityPatterns {
    pub hourly_engagement: Vec<f32>,
    pub daily_engagement: Vec<f32>,
}

// ============================================================================
// Core ACE Types
// ============================================================================

/// The Autonomous Context Engine (simplified)
#[allow(clippy::upper_case_acronyms)]
pub struct ACE {
    conn: Arc<Mutex<Connection>>,
    scanner: ProjectScanner,
    git_analyzer: GitAnalyzer,
    watcher: Option<Mutex<FileWatcher>>,
    watcher_persistence: Option<WatcherStatePersistence>,
    embedding_service: Option<Mutex<EmbeddingService>>,
    rate_limiter: InteractionRateLimiter,
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
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self, String> {
        db::migrate(&conn)?;

        let scanner = ProjectScanner::new();
        let git_analyzer = GitAnalyzer::default();
        let watcher_persistence = WatcherStatePersistence::new(conn.clone()).ok();

        let embedding_config = EmbeddingConfig::default();
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
    pub fn start_watching(&mut self, paths: &[PathBuf]) -> Result<(), String> {
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
    pub fn stop_watching(&mut self) {
        if let Some(ref watcher) = self.watcher {
            watcher.lock().stop();
        }
        self.watcher = None;
        info!(target: "ace::watcher", "File watching stopped");
    }

    /// Check if file watching is active
    pub fn is_watching(&self) -> bool {
        self.watcher
            .as_ref()
            .is_some_and(|w| w.lock().is_watching())
    }

    /// Analyze git repositories in the given paths
    pub fn analyze_git_repos(&self, paths: &[PathBuf]) -> Result<Vec<GitSignal>, String> {
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

        self.store_git_signals(&signals)?;
        Ok(signals)
    }

    fn store_git_signals(&self, signals: &[GitSignal]) -> Result<(), String> {
        let conn = self.conn.lock();

        for signal in signals {
            for topic in &signal.extracted_topics {
                conn.execute(
                    "INSERT INTO active_topics (topic, weight, confidence, source, last_seen)
                     VALUES (?1, ?2, ?3, 'git_commit', datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        weight = MAX(excluded.weight, active_topics.weight),
                        confidence = MAX(excluded.confidence, active_topics.confidence),
                        last_seen = datetime('now')",
                    rusqlite::params![topic, 0.7, signal.confidence],
                )
                .map_err(|e| format!("Failed to store git topic: {}", e))?;
            }

            let topics_json = serde_json::to_string(&signal.extracted_topics).unwrap_or_default();
            conn.execute(
                "INSERT INTO git_signals (repo_path, commit_hash, commit_message, extracted_topics, timestamp)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                rusqlite::params![
                    signal.repo_path.to_string_lossy(),
                    signal.last_commit.clone().unwrap_or_default(),
                    signal.recent_commits.first().map(|c| c.message.clone()).unwrap_or_default(),
                    topics_json
                ],
            ).map_err(|e| format!("Failed to store git signal: {}", e))?;
        }

        Ok(())
    }

    /// Perform autonomous context detection
    pub fn detect_context(&self, scan_paths: &[PathBuf]) -> Result<AutonomousContext, String> {
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

        self.store_detected_context(&merged_tech, &active_topics)?;

        Ok(AutonomousContext {
            detected_tech: merged_tech,
            active_topics,
            projects_scanned: projects_found,
            context_confidence,
            detection_time: chrono::Utc::now().to_rfc3339(),
        })
    }

    fn store_detected_context(
        &self,
        tech: &[DetectedTech],
        topics: &[ActiveTopic],
    ) -> Result<(), String> {
        let conn = self.conn.lock();

        for t in tech {
            let source_str = match t.source {
                DetectionSource::Manifest => "manifest",
                DetectionSource::FileExtension => "file_extension",
                DetectionSource::FileContent => "file_content",
                DetectionSource::GitHistory => "git_history",
                DetectionSource::UserExplicit => "explicit",
            };

            let category_str = match t.category {
                TechCategory::Language => "language",
                TechCategory::Framework => "framework",
                TechCategory::Library => "library",
                TechCategory::Tool => "tool",
                TechCategory::Database => "database",
                TechCategory::Platform => "platform",
            };

            conn.execute(
                "INSERT INTO detected_tech (name, category, confidence, source, evidence)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(name) DO UPDATE SET
                    confidence = MAX(excluded.confidence, detected_tech.confidence),
                    evidence = excluded.evidence,
                    updated_at = datetime('now')",
                rusqlite::params![
                    t.name,
                    category_str,
                    t.confidence,
                    source_str,
                    t.evidence.join("; ")
                ],
            )
            .map_err(|e| format!("Failed to store detected tech: {}", e))?;
        }

        for topic in topics {
            let source_str = match topic.source {
                TopicSource::FileContent => "file_content",
                TopicSource::GitCommit => "git_commit",
                TopicSource::ImportStatement => "import",
                TopicSource::ProjectManifest => "manifest",
                TopicSource::ActivityTracker => "activity",
            };

            conn.execute(
                "INSERT INTO active_topics (topic, weight, confidence, source, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(topic) DO UPDATE SET
                    weight = MAX(excluded.weight, active_topics.weight),
                    confidence = MAX(excluded.confidence, active_topics.confidence),
                    last_seen = excluded.last_seen",
                rusqlite::params![
                    topic.topic,
                    topic.weight,
                    topic.confidence,
                    source_str,
                    topic.last_seen
                ],
            )
            .map_err(|e| format!("Failed to store active topic: {}", e))?;
        }

        Ok(())
    }

    /// Get all detected technologies
    pub fn get_detected_tech(&self) -> Result<Vec<DetectedTech>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT name, category, confidence, source, evidence FROM detected_tech ORDER BY confidence DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
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
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Get active topics
    pub fn get_active_topics(&self) -> Result<Vec<ActiveTopic>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT topic, weight, confidence, source, last_seen FROM active_topics
             WHERE last_seen > datetime('now', '-7 days')
             ORDER BY weight DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
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
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    // ========================================================================
    // Behavior Learning Methods (Simplified)
    // ========================================================================

    /// Record a user interaction
    pub fn record_interaction(
        &self,
        item_id: i64,
        action: BehaviorAction,
        item_topics: Vec<String>,
        item_source: String,
    ) -> Result<(), String> {
        if !self.rate_limiter.check(&item_source) {
            return Err("Rate limited: too many interactions".to_string());
        }

        let signal_strength = action.compute_strength();
        let signal = BehaviorSignal {
            item_id,
            action: action.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            item_topics: item_topics.clone(),
            item_source: item_source.clone(),
            signal_strength,
        };

        self.store_interaction(&signal)?;
        self.update_topic_affinities(&signal)?;

        if signal.signal_strength < -0.5 {
            self.update_anti_topics(&item_topics, signal.signal_strength)?;
        }

        self.update_source_preference(&item_source, signal.signal_strength)?;

        debug!(target: "ace::behavior",
            action = ?action,
            item_id = item_id,
            strength = signal.signal_strength,
            "Recorded behavior signal"
        );

        Ok(())
    }

    /// Get rate limit status
    pub fn get_rate_limit_status(&self, source: &str) -> RateLimitStatus {
        self.rate_limiter.status(source)
    }

    fn store_interaction(&self, signal: &BehaviorSignal) -> Result<(), String> {
        let conn = self.conn.lock();

        let action_type = match &signal.action {
            BehaviorAction::Click { .. } => "click",
            BehaviorAction::Save => "save",
            BehaviorAction::Share => "share",
            BehaviorAction::Dismiss => "dismiss",
            BehaviorAction::MarkIrrelevant => "mark_irrelevant",
            BehaviorAction::Scroll { .. } => "scroll",
            BehaviorAction::Ignore => "ignore",
        };

        let action_data = serde_json::to_string(&signal.action).unwrap_or_default();
        let topics_json = serde_json::to_string(&signal.item_topics).unwrap_or_default();

        conn.execute(
            "INSERT INTO interactions (item_id, action_type, action_data, item_topics, item_source, signal_strength)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                signal.item_id,
                action_type,
                action_data,
                topics_json,
                signal.item_source,
                signal.signal_strength
            ],
        )
        .map_err(|e| format!("Failed to store interaction: {}", e))?;

        Ok(())
    }

    fn update_topic_affinities(&self, signal: &BehaviorSignal) -> Result<(), String> {
        let conn = self.conn.lock();

        for topic in &signal.item_topics {
            if signal.signal_strength > 0.0 {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, positive_signals, total_exposures, last_interaction)
                     VALUES (?1, 1, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        positive_signals = topic_affinities.positive_signals + 1,
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        decay_applied = 0,
                        last_decay_at = NULL,
                        updated_at = datetime('now')",
                    rusqlite::params![topic],
                )
            } else if signal.signal_strength < 0.0 {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, negative_signals, total_exposures, last_interaction)
                     VALUES (?1, 1, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        negative_signals = topic_affinities.negative_signals + 1,
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        decay_applied = 0,
                        last_decay_at = NULL,
                        updated_at = datetime('now')",
                    rusqlite::params![topic],
                )
            } else {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, total_exposures, last_interaction)
                     VALUES (?1, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        updated_at = datetime('now')",
                    rusqlite::params![topic],
                )
            }
            .map_err(|e| format!("Failed to update topic affinity: {}", e))?;

            // For strong negative signals (MarkIrrelevant = -1.0, Dismiss = -0.8),
            // activate affinity immediately — don't wait for 5 exposures.
            // Users expect instant feedback when they explicitly reject content.
            conn.execute(
                "UPDATE topic_affinities SET
                    affinity_score = CASE
                        WHEN negative_signals > 0 AND positive_signals = 0 THEN
                            -1.0 * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
                        WHEN total_exposures >= 3 THEN
                            (CAST(positive_signals AS REAL) - CAST(negative_signals AS REAL)) / CAST(total_exposures AS REAL)
                            * MIN(CAST(total_exposures AS REAL) / 20.0, 1.0)
                        ELSE 0.0
                    END,
                    confidence = CASE
                        WHEN negative_signals > 0 AND positive_signals = 0 THEN
                            MAX(0.3, MIN(CAST(total_exposures AS REAL) / 10.0, 1.0))
                        ELSE MIN(CAST(total_exposures AS REAL) / 20.0, 1.0)
                    END
                 WHERE topic = ?1",
                rusqlite::params![topic],
            )
            .map_err(|e| format!("Failed to recompute affinity: {}", e))?;
        }

        Ok(())
    }

    fn update_anti_topics(&self, topics: &[String], signal_strength: f32) -> Result<(), String> {
        if signal_strength >= -0.5 {
            return Ok(());
        }

        let conn = self.conn.lock();

        for topic in topics {
            conn.execute(
                "INSERT INTO anti_topics (topic, rejection_count, confidence, last_rejection)
                 VALUES (?1, 1, 0.2, datetime('now'))
                 ON CONFLICT(topic) DO UPDATE SET
                    rejection_count = anti_topics.rejection_count + 1,
                    confidence = MIN(CAST(anti_topics.rejection_count + 1 AS REAL) / 10.0, 0.9),
                    last_rejection = datetime('now'),
                    updated_at = datetime('now')",
                rusqlite::params![topic],
            )
            .map_err(|e| format!("Failed to update anti-topic: {}", e))?;
        }

        Ok(())
    }

    fn update_source_preference(&self, source: &str, signal_strength: f32) -> Result<(), String> {
        let conn = self.conn.lock();
        let alpha = 0.1;

        conn.execute(
            "INSERT INTO source_preferences (source, score, interactions, last_interaction)
             VALUES (?1, ?2, 1, datetime('now'))
             ON CONFLICT(source) DO UPDATE SET
                score = source_preferences.score * (1.0 - ?3) + ?2 * ?3,
                interactions = source_preferences.interactions + 1,
                last_interaction = datetime('now'),
                updated_at = datetime('now')",
            rusqlite::params![source, signal_strength, alpha],
        )
        .map_err(|e| format!("Failed to update source preference: {}", e))?;

        Ok(())
    }

    /// Get topic affinities
    pub fn get_topic_affinities(&self) -> Result<Vec<TopicAffinity>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT topic, positive_signals, negative_signals, total_exposures,
                    affinity_score, confidence, last_interaction
             FROM topic_affinities
             WHERE total_exposures >= 5
             ORDER BY ABS(affinity_score) DESC
             LIMIT 100",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(TopicAffinity {
                    topic: row.get(0)?,
                    embedding: None,
                    positive_signals: row.get(1)?,
                    negative_signals: row.get(2)?,
                    total_exposures: row.get(3)?,
                    affinity_score: row.get(4)?,
                    confidence: row.get(5)?,
                    last_interaction: row.get(6)?,
                    decay_applied: false,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Get anti-topics
    pub fn get_anti_topics(&self, min_rejections: u32) -> Result<Vec<AntiTopic>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT topic, rejection_count, confidence, auto_detected, user_confirmed,
                    first_rejection, last_rejection
             FROM anti_topics
             WHERE rejection_count >= ?1
             ORDER BY rejection_count DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([min_rejections], |row| {
                Ok(AntiTopic {
                    topic: row.get(0)?,
                    rejection_count: row.get(1)?,
                    confidence: row.get(2)?,
                    auto_detected: row.get::<_, i32>(3)? != 0,
                    user_confirmed: row.get::<_, i32>(4)? != 0,
                    first_rejection: row.get(5)?,
                    last_rejection: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Get behavior modifier for an item
    pub fn get_behavior_modifier(&self, topics: &[String], source: &str) -> Result<f32, String> {
        let conn = self.conn.lock();
        let mut modifier = 0.0;
        let mut count = 0;

        for topic in topics {
            let result: Result<(f32, f32), _> = conn.query_row(
                "SELECT affinity_score, confidence FROM topic_affinities WHERE topic = ?1",
                [topic],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );
            if let Ok((score, confidence)) = result {
                modifier += score * confidence;
                count += 1;
            }
        }

        if count > 0 {
            modifier /= count as f32;
        }

        let source_score: f32 = conn
            .query_row(
                "SELECT score FROM source_preferences WHERE source = ?1",
                [source],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        modifier += source_score * 0.3;

        Ok(modifier.clamp(-1.0, 1.0))
    }

    /// Get learned behavior summary
    pub fn get_learned_behavior(&self) -> Result<LearnedBehaviorSummary, String> {
        let affinities = self.get_topic_affinities()?;
        let anti_topics = self.get_anti_topics(5)?;

        let conn = self.conn.lock();

        let total_interactions: u32 = conn
            .query_row("SELECT COUNT(*) FROM interactions", [], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = conn
            .prepare(
                "SELECT source, score, interactions FROM source_preferences ORDER BY score DESC",
            )
            .map_err(|e| e.to_string())?;

        let source_prefs: Vec<SourcePreferenceSummary> = stmt
            .query_map([], |row| {
                Ok(SourcePreferenceSummary {
                    source: row.get(0)?,
                    score: row.get(1)?,
                    interactions: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let learning_confidence = (total_interactions as f32 / 100.0).min(0.95);

        let interests: Vec<String> = affinities
            .iter()
            .filter(|a| a.affinity_score > 0.3 && a.confidence > 0.5)
            .map(|a| a.topic.clone())
            .collect();

        Ok(LearnedBehaviorSummary {
            total_interactions,
            learning_confidence,
            interests,
            anti_topics: anti_topics.iter().map(|a| a.topic.clone()).collect(),
            source_preferences: source_prefs,
            top_affinities: affinities.into_iter().take(10).collect(),
        })
    }

    /// Get source preferences for scoring
    pub fn get_source_preferences(&self) -> Result<Vec<(String, f32)>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT source, score FROM source_preferences WHERE interactions >= 5 ORDER BY source",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Confirm an anti-topic
    pub fn confirm_anti_topic(&self, topic: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE anti_topics SET user_confirmed = 1, confidence = 1.0, updated_at = datetime('now')
             WHERE topic = ?1",
            [topic],
        )
        .map_err(|e| format!("Failed to confirm anti-topic: {}", e))?;
        Ok(())
    }

    /// Apply temporal decay to topic affinities
    /// Uses 30-day half-life: after 30 days of no interaction, scores halve.
    /// Runs continuously based on time since last decay (not a one-shot boolean).
    /// Deletes fully-decayed affinities (|score| < 0.05).
    pub fn apply_behavior_decay(&self) -> Result<usize, String> {
        let conn = self.conn.lock();

        // Fetch all affinities that haven't been interacted with in >1 day
        // Use last_decay_at to compute incremental decay (not decay from epoch)
        let mut stmt = conn
            .prepare(
                "SELECT topic, affinity_score, confidence, last_interaction,
                        COALESCE(last_decay_at, last_interaction) as decay_baseline
                 FROM topic_affinities
                 WHERE julianday('now') - julianday(last_interaction) > 1",
            )
            .map_err(|e| format!("Failed to prepare decay query: {}", e))?;

        let rows: Vec<(String, f32, f32, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, f32>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| format!("Failed to query topics for decay: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect decay rows: {}", e))?;

        let mut updated = 0;
        let now = chrono::Utc::now().to_rfc3339();

        for (topic, affinity_score, confidence, _last_interaction, decay_baseline) in &rows {
            // Parse the decay baseline timestamp
            let baseline = chrono::DateTime::parse_from_rfc3339(decay_baseline)
                .or_else(|_| {
                    // Try SQLite datetime format: "YYYY-MM-DD HH:MM:SS"
                    chrono::NaiveDateTime::parse_from_str(decay_baseline, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| dt.and_utc().fixed_offset())
                })
                .unwrap_or_else(|_| chrono::Utc::now().fixed_offset());

            let days_since = (chrono::Utc::now() - baseline.with_timezone(&chrono::Utc)).num_hours()
                as f32
                / 24.0;
            if days_since < 1.0 {
                continue; // Already decayed recently
            }

            // 30-day half-life decay
            let decay_factor = 0.5_f32.powf(days_since / 30.0);
            let new_affinity = affinity_score * decay_factor;
            let new_confidence = confidence.min(1.0) * decay_factor;

            // Delete fully-decayed affinities
            if new_affinity.abs() < 0.05 {
                conn.execute(
                    "DELETE FROM topic_affinities WHERE topic = ?1",
                    rusqlite::params![topic],
                )
                .map_err(|e| format!("Failed to delete decayed topic: {}", e))?;
                updated += 1;
                continue;
            }

            // Update with decayed values and record decay timestamp
            conn.execute(
                "UPDATE topic_affinities SET
                    affinity_score = ?1,
                    confidence = ?2,
                    last_decay_at = ?3,
                    decay_applied = 1
                 WHERE topic = ?4",
                rusqlite::params![new_affinity, new_confidence, now, topic],
            )
            .map_err(|e| format!("Failed to update topic decay: {}", e))?;

            updated += 1;
        }

        if updated > 0 {
            info!(target: "ace::behavior", updated = updated, "Applied temporal decay to topic affinities");
        }

        Ok(updated)
    }

    // ========================================================================
    // Embedding Methods
    // ========================================================================

    /// Generate embedding for a topic
    pub fn embed_topic(&self, topic: &str) -> Result<Vec<f32>, String> {
        match &self.embedding_service {
            Some(service) => service.lock().embed(topic),
            None => Err("Embedding service not initialized".to_string()),
        }
    }

    /// Find similar topics
    pub fn find_similar_topics(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        let topics = self.get_active_topics()?;
        let topic_strings: Vec<String> = topics.iter().map(|t| t.topic.clone()).collect();

        match &self.embedding_service {
            Some(service) => service.lock().find_similar(query, &topic_strings, top_k),
            None => Err("Embedding service not initialized".to_string()),
        }
    }

    /// Check if embedding service is operational
    pub fn is_embedding_operational(&self) -> bool {
        self.embedding_service
            .as_ref()
            .map(|s| s.lock().is_operational())
            .unwrap_or(false)
    }

    // ========================================================================
    // Threshold Auto-Tuning Methods
    // ========================================================================

    /// Compute threshold adjustment based on user engagement rate over the last 7 days.
    /// Returns `Some(new_threshold)` if adjustment is warranted, `None` if data is
    /// insufficient or the current threshold is already appropriate.
    ///
    /// Logic:
    /// - High engagement (>50% positive): threshold too strict, lower by 0.02
    /// - Low engagement (<15% positive): threshold too loose, raise by 0.02
    /// - Always clamped to [0.30, 0.50]
    pub fn compute_threshold_adjustment(&self, current_threshold: f32) -> Option<f32> {
        let conn = self.conn.lock();

        // Count total interactions from the last 7 days
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
    pub fn save_watcher_state(&self) -> Result<(), String> {
        if let (Some(persistence), Some(watcher)) = (&self.watcher_persistence, &self.watcher) {
            let watcher_guard = watcher.lock();
            persistence.save(&watcher_guard)
        } else {
            Err("Watcher or persistence not initialized".to_string())
        }
    }

    /// Clear watcher state
    pub fn clear_watcher_state(&self) -> Result<(), String> {
        if let Some(persistence) = &self.watcher_persistence {
            persistence.clear()
        } else {
            Err("Watcher persistence not initialized".to_string())
        }
    }

    /// Get topics from recent file changes for "active work" boosting.
    /// Returns topics with recency-weighted scores (higher = more recent).
    /// Used by the scoring pipeline to boost content matching what the user
    /// is actively working on RIGHT NOW.
    pub fn get_recent_work_topics(&self, hours: u64) -> Result<Vec<(String, f32)>, String> {
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

// ============================================================================
// Additional Types
// ============================================================================

/// Summary of learned behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedBehaviorSummary {
    pub total_interactions: u32,
    pub learning_confidence: f32,
    pub interests: Vec<String>,
    pub anti_topics: Vec<String>,
    pub source_preferences: Vec<SourcePreferenceSummary>,
    pub top_affinities: Vec<TopicAffinity>,
}

/// Source preference summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePreferenceSummary {
    pub source: String,
    pub score: f32,
    pub interactions: u32,
}

/// Result of autonomous context detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousContext {
    pub detected_tech: Vec<DetectedTech>,
    pub active_topics: Vec<ActiveTopic>,
    pub projects_scanned: usize,
    pub context_confidence: f32,
    pub detection_time: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse an ISO timestamp string and return how many hours ago it was.
/// Falls back to `24.0` if parsing fails (treats unparseable timestamps as old).
fn parse_hours_ago(timestamp_str: &str) -> f32 {
    if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S") {
        let now = chrono::Utc::now().naive_utc();
        let duration = now - ts;
        (duration.num_minutes() as f32 / 60.0).max(0.0)
    } else {
        // Fallback: treat as old
        24.0
    }
}

/// Check if a dependency is notable
fn is_notable_dependency(name: &str) -> bool {
    let notable = [
        // Rust
        "tokio",
        "async-std",
        "serde",
        "reqwest",
        "sqlx",
        "diesel",
        "actix",
        "axum",
        "rocket",
        "tauri",
        "warp",
        "hyper",
        "tonic",
        "prost",
        // JavaScript/TypeScript
        "react",
        "vue",
        "angular",
        "svelte",
        "next",
        "nuxt",
        "express",
        "fastify",
        "nest",
        "prisma",
        "drizzle",
        "typeorm",
        "mongoose",
        "vite",
        "webpack",
        "esbuild",
        // Python
        "django",
        "flask",
        "fastapi",
        "numpy",
        "pandas",
        "tensorflow",
        "pytorch",
        "scikit-learn",
        "sqlalchemy",
        "celery",
        "redis",
        // Go
        "gin",
        "echo",
        "fiber",
        "gorm",
        "cobra",
        "viper",
        // Databases
        "postgresql",
        "mysql",
        "sqlite",
        "mongodb",
        "redis",
        "elasticsearch",
    ];

    notable.iter().any(|n| name.to_lowercase().contains(n))
}

/// Merge duplicate detected technologies
fn merge_detected_tech(tech: Vec<DetectedTech>) -> Vec<DetectedTech> {
    let mut map: HashMap<String, DetectedTech> = HashMap::new();

    for t in tech {
        let key = t.name.to_lowercase();
        if let Some(existing) = map.get_mut(&key) {
            if t.confidence > existing.confidence {
                existing.confidence = t.confidence;
            }
            existing.evidence.extend(t.evidence);
        } else {
            map.insert(key, t);
        }
    }

    let mut result: Vec<_> = map.into_values().collect();
    result.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    result
}

/// Process file changes from the watcher
fn process_file_changes(
    conn: &Arc<Mutex<Connection>>,
    changes: &[FileChange],
) -> Result<(), String> {
    use crate::extractors::ExtractorRegistry;

    let conn = conn.lock();
    let extractor_registry = ExtractorRegistry::new();

    for change in changes {
        let change_type_str = match change.change_type {
            FileChangeType::Created => "created",
            FileChangeType::Modified => "modified",
            FileChangeType::Deleted => "deleted",
        };

        // Extract content based on file type
        let (topics, extracted_text, source_type) = if change.change_type != FileChangeType::Deleted
        {
            // Check if this is a document file that needs extraction
            let ext = change
                .path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            if matches!(
                ext.as_str(),
                "pdf"
                    | "docx"
                    | "xlsx"
                    | "zip"
                    | "tar"
                    | "gz"
                    | "tgz"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "tiff"
                    | "tif"
                    | "bmp"
                    | "gif"
                    | "webp"
            ) {
                // Use document extractor
                match extractor_registry.extract(&change.path) {
                    Ok(doc) => {
                        // Extract topics from the extracted text
                        let topics = watcher::extract_topics_from_content(&doc.text, &ext);
                        info!(target: "ace::watcher",
                            path = %change.path.display(),
                            source_type = %doc.source_type,
                            word_count = doc.word_count(),
                            confidence = doc.confidence,
                            "Extracted document content"
                        );
                        (topics, Some(doc.text), doc.source_type)
                    }
                    Err(e) => {
                        warn!(target: "ace::watcher",
                            path = %change.path.display(),
                            error = %e,
                            "Failed to extract document"
                        );
                        (Vec::new(), None, "unknown".to_string())
                    }
                }
            } else {
                // Plain text file - use existing logic
                let topics = watcher::extract_topics_from_file(&change.path).unwrap_or_default();
                (topics, None, "text".to_string())
            }
        } else {
            (Vec::new(), None, "deleted".to_string())
        };

        let topics_json = serde_json::to_string(&topics).unwrap_or_default();

        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![change.path.to_string_lossy(), change_type_str, topics_json],
        )
        .map_err(|e| format!("Failed to store file signal: {}", e))?;

        for topic in &topics {
            conn.execute(
                "INSERT INTO active_topics (topic, weight, confidence, source, last_seen)
                 VALUES (?1, 0.6, 0.7, 'file_content', datetime('now'))
                 ON CONFLICT(topic) DO UPDATE SET
                    weight = MAX(excluded.weight, active_topics.weight),
                    last_seen = datetime('now')",
                rusqlite::params![topic],
            )
            .map_err(|e| format!("Failed to update active topic: {}", e))?;
        }

        // Store extracted document content in indexed_documents and document_chunks
        if let Some(text) = extracted_text {
            if !text.is_empty() {
                let file_path_str = change.path.to_string_lossy().to_string();
                let file_name = change
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let file_size = std::fs::metadata(&change.path)
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                let word_count = text.split_whitespace().count() as i64;

                // Insert or update indexed_documents
                conn.execute(
                    "INSERT INTO indexed_documents (file_path, file_name, file_type, file_size, word_count, extraction_confidence, extracted_topics, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
                     ON CONFLICT(file_path) DO UPDATE SET
                        file_name = excluded.file_name,
                        file_type = excluded.file_type,
                        file_size = excluded.file_size,
                        word_count = excluded.word_count,
                        extraction_confidence = excluded.extraction_confidence,
                        extracted_topics = excluded.extracted_topics,
                        updated_at = datetime('now')",
                    rusqlite::params![
                        file_path_str,
                        file_name,
                        source_type,
                        file_size,
                        word_count,
                        0.8_f64, // Default confidence for successful extraction
                        topics_json
                    ],
                ).ok();

                // Get the document ID
                if let Ok(doc_id) = conn.query_row(
                    "SELECT id FROM indexed_documents WHERE file_path = ?1",
                    rusqlite::params![file_path_str],
                    |row| row.get::<_, i64>(0),
                ) {
                    // Delete old chunks for this document
                    conn.execute(
                        "DELETE FROM document_chunks WHERE document_id = ?1",
                        rusqlite::params![doc_id],
                    )
                    .ok();

                    // Split text into chunks (max 1000 words per chunk)
                    let words: Vec<&str> = text.split_whitespace().collect();
                    let chunk_size = 1000;
                    for (i, chunk_words) in words.chunks(chunk_size).enumerate() {
                        let chunk_text = chunk_words.join(" ");
                        let chunk_word_count = chunk_words.len() as i64;
                        conn.execute(
                            "INSERT INTO document_chunks (document_id, chunk_index, content, word_count)
                             VALUES (?1, ?2, ?3, ?4)",
                            rusqlite::params![doc_id, i as i64, chunk_text, chunk_word_count],
                        ).ok();
                    }

                    debug!(target: "ace::watcher",
                        path = %change.path.display(),
                        doc_id = doc_id,
                        chunks = (words.len() / chunk_size) + 1,
                        "Stored document content"
                    );
                }
            }
        }

        debug!(target: "ace::watcher",
            path = %change.path.display(),
            change_type = change_type_str,
            topic_count = topics.len(),
            source_type = source_type,
            "Processed file change"
        );
    }

    Ok(())
}

/// Apply freshness decay to active topics
pub fn apply_freshness_decay(conn: &Arc<Mutex<Connection>>) -> Result<usize, String> {
    let conn = conn.lock();

    let updated = conn
        .execute(
            "UPDATE active_topics SET
            weight = weight * (0.5 * (1.0 + (julianday('now') - julianday(last_seen)) / 7.0)),
            decay_applied = 1
         WHERE decay_applied = 0
           AND julianday('now') - julianday(last_seen) > 1",
            [],
        )
        .map_err(|e| format!("Failed to apply decay: {}", e))?;

    let removed = conn
        .execute("DELETE FROM active_topics WHERE weight < 0.1", [])
        .map_err(|e| format!("Failed to clean up topics: {}", e))?;

    if updated > 0 || removed > 0 {
        debug!(target: "ace::decay", updated = updated, removed = removed, "Applied freshness decay");
    }

    Ok(updated)
}

/// Get real-time context for relevance scoring
pub fn get_realtime_context(conn: &Arc<Mutex<Connection>>) -> Result<RealtimeContext, String> {
    let conn = conn.lock();

    let mut stmt = conn
        .prepare(
            "SELECT topic, weight, confidence, source, last_seen
         FROM active_topics
         WHERE julianday('now') - julianday(last_seen) <= 7
         ORDER BY weight DESC
         LIMIT 50",
        )
        .map_err(|e| e.to_string())?;

    let topics: Vec<ActiveTopic> = stmt
        .query_map([], |row| {
            Ok(ActiveTopic {
                topic: row.get(0)?,
                weight: row.get(1)?,
                confidence: row.get(2)?,
                source: match row.get::<_, String>(3)?.as_str() {
                    "file_content" => TopicSource::FileContent,
                    "git_commit" => TopicSource::GitCommit,
                    "import" => TopicSource::ImportStatement,
                    "manifest" => TopicSource::ProjectManifest,
                    _ => TopicSource::ActivityTracker,
                },
                last_seen: row.get(4)?,
                embedding: None,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT name, category, confidence, source
         FROM detected_tech
         ORDER BY confidence DESC
         LIMIT 20",
        )
        .map_err(|e| e.to_string())?;

    let tech: Vec<DetectedTech> = stmt
        .query_map([], |row| {
            let category_str: String = row.get(1)?;
            let source_str: String = row.get(3)?;

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
                evidence: Vec::new(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let context_confidence = if topics.is_empty() && tech.is_empty() {
        0.3
    } else {
        let topic_conf = if topics.is_empty() {
            0.0
        } else {
            topics.iter().map(|t| t.confidence).sum::<f32>() / topics.len() as f32
        };
        let tech_conf = if tech.is_empty() {
            0.0
        } else {
            tech.iter().map(|t| t.confidence).sum::<f32>() / tech.len() as f32
        };
        (topic_conf * 0.5 + tech_conf * 0.5).min(0.95)
    };

    Ok(RealtimeContext {
        active_topics: topics,
        detected_tech: tech,
        context_confidence,
        last_updated: chrono::Utc::now().to_rfc3339(),
    })
}

/// Real-time context data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeContext {
    pub active_topics: Vec<ActiveTopic>,
    pub detected_tech: Vec<DetectedTech>,
    pub context_confidence: f32,
    pub last_updated: String,
}

// ============================================================================
// Topic Embedding Functions (semantic matching via sqlite-vec)
// ============================================================================

/// Convert embedding vector to blob for SQLite storage
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob from SQLite to embedding vector
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Store a topic embedding in the database and vec0 index
pub fn store_topic_embedding(
    conn: &Arc<Mutex<Connection>>,
    topic: &str,
    embedding: &[f32],
) -> Result<(), String> {
    let conn = conn.lock();
    let embedding_blob = embedding_to_blob(embedding);

    // Get the topic's rowid
    let topic_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM active_topics WHERE topic = ?1",
            rusqlite::params![topic],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = topic_id {
        // Update the embedding in active_topics
        conn.execute(
            "UPDATE active_topics SET embedding = ?1 WHERE id = ?2",
            rusqlite::params![embedding_blob, id],
        )
        .map_err(|e| format!("Failed to update topic embedding: {}", e))?;

        // Update or insert into vec0 index
        // First try to update existing, then insert if not found
        let updated = conn
            .execute(
                "UPDATE topic_vec SET embedding = ?1 WHERE rowid = ?2",
                rusqlite::params![embedding_blob, id],
            )
            .unwrap_or(0);

        if updated == 0 {
            // Insert with explicit rowid matching the topic id
            conn.execute(
                "INSERT OR REPLACE INTO topic_vec (rowid, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, embedding_blob],
            )
            .map_err(|e| format!("Failed to insert topic into vec0: {}", e))?;
        }
    }

    Ok(())
}

/// Load all topic embeddings from the database
pub fn load_topic_embeddings(
    conn: &Arc<Mutex<Connection>>,
) -> Result<std::collections::HashMap<String, Vec<f32>>, String> {
    let conn = conn.lock();
    let mut result = std::collections::HashMap::new();

    let mut stmt = conn
        .prepare(
            "SELECT topic, embedding FROM active_topics
             WHERE embedding IS NOT NULL
             AND julianday('now') - julianday(last_seen) <= 7",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let topic: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((topic, blob))
        })
        .map_err(|e| e.to_string())?;

    for (topic, blob) in rows.flatten() {
        let embedding = blob_to_embedding(&blob);
        result.insert(topic, embedding);
    }

    debug!(
        target: "ace::embedding",
        count = result.len(),
        "Loaded topic embeddings from database"
    );

    Ok(result)
}

/// Generate embeddings for topics that don't have them
/// Returns count of topics updated
#[allow(dead_code)] // Future: batch embedding generation on startup
pub async fn generate_missing_topic_embeddings(
    conn: &Arc<Mutex<Connection>>,
) -> Result<usize, String> {
    // Find topics without embeddings
    let topics_without_embeddings: Vec<(i64, String)> = {
        let conn_guard = conn.lock();
        let mut stmt = conn_guard
            .prepare(
                "SELECT id, topic FROM active_topics
                 WHERE embedding IS NULL
                 AND julianday('now') - julianday(last_seen) <= 7
                 LIMIT 50",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    if topics_without_embeddings.is_empty() {
        return Ok(0);
    }

    info!(
        target: "ace::embedding",
        count = topics_without_embeddings.len(),
        "Generating embeddings for topics without embeddings"
    );

    // Generate embeddings using the main embed_texts function
    let topic_texts: Vec<String> = topics_without_embeddings
        .iter()
        .map(|(_, t)| t.clone())
        .collect();

    let embeddings = crate::embed_texts(&topic_texts).await?;

    // Store embeddings
    let mut updated = 0;
    for ((id, topic), embedding) in topics_without_embeddings.iter().zip(embeddings.iter()) {
        let embedding_blob = embedding_to_blob(embedding);

        let conn_guard = conn.lock();

        // Update active_topics
        if conn_guard
            .execute(
                "UPDATE active_topics SET embedding = ?1 WHERE id = ?2",
                rusqlite::params![embedding_blob, id],
            )
            .is_ok()
        {
            // Insert into vec0 index
            let _ = conn_guard.execute(
                "INSERT OR REPLACE INTO topic_vec (rowid, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, embedding_blob],
            );
            updated += 1;
            debug!(target: "ace::embedding", topic = %topic, "Generated embedding for topic");
        }
    }

    info!(target: "ace::embedding", updated = updated, "Generated topic embeddings");
    Ok(updated)
}

/// KNN search for topics similar to a given embedding
/// Returns (topic, similarity_score) pairs sorted by similarity
#[allow(dead_code)] // Future: semantic topic matching via KNN
pub fn find_similar_topics(
    conn: &Arc<Mutex<Connection>>,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<(String, f32)>, String> {
    let conn = conn.lock();
    let embedding_blob = embedding_to_blob(query_embedding);

    let mut stmt = conn
        .prepare(
            "SELECT at.topic, tv.distance
             FROM topic_vec tv
             JOIN active_topics at ON at.id = tv.rowid
             WHERE tv.embedding MATCH ?1
             AND k = ?2
             ORDER BY tv.distance",
        )
        .map_err(|e| format!("Failed to prepare KNN query: {}", e))?;

    let rows = stmt
        .query_map(rusqlite::params![embedding_blob, limit as i32], |row| {
            let topic: String = row.get(0)?;
            let distance: f32 = row.get(1)?;
            // Convert L2 distance to similarity (1 / (1 + distance))
            let similarity = 1.0 / (1.0 + distance);
            Ok((topic, similarity))
        })
        .map_err(|e| format!("KNN query failed: {}", e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notable_dependency() {
        assert!(is_notable_dependency("react"));
        assert!(is_notable_dependency("tokio"));
        assert!(is_notable_dependency("@prisma/client"));
        assert!(!is_notable_dependency("my-random-lib"));
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

    /// Helper: create an in-memory ACE instance for testing.
    /// Loads the sqlite-vec extension so vec0 virtual tables work.
    fn create_test_ace() -> ACE {
        // Load sqlite-vec extension for vec0 virtual tables
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

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
    /// Uses recent timestamps so they fall within the 7-day window.
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
        // 15 clicks + 5 saves = 20 positive out of 25 total => 80% engagement
        insert_interactions(&ace, "click", 15);
        insert_interactions(&ace, "save", 5);
        insert_interactions(&ace, "dismiss", 5);

        // Start above the floor so lowering is possible
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
        // 2 clicks out of 25 total => 8% engagement
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
        // High engagement to trigger lowering
        insert_interactions(&ace, "click", 25);

        // Start at minimum bound - should not go below 0.30
        let result = ace.compute_threshold_adjustment(0.30);
        assert!(
            result.is_none(),
            "Threshold at minimum (0.30) should not decrease further"
        );

        // Low engagement to trigger raising
        let ace2 = create_test_ace();
        insert_interactions(&ace2, "dismiss", 25);

        // Start at maximum bound - should not go above 0.50
        let result2 = ace2.compute_threshold_adjustment(0.50);
        assert!(
            result2.is_none(),
            "Threshold at maximum (0.50) should not increase further"
        );
    }

    #[test]
    fn test_insufficient_data_no_change() {
        let ace = create_test_ace();
        // Only 5 interactions - below the 20 minimum
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

        // Initially no stored threshold
        assert!(
            ace.get_stored_threshold().is_none(),
            "Fresh DB should have no stored threshold"
        );

        // Store a threshold
        ace.store_threshold(0.42);
        let loaded = ace.get_stored_threshold();
        assert!(loaded.is_some(), "Should load stored threshold");
        assert!(
            (loaded.unwrap() - 0.42).abs() < 0.001,
            "Stored threshold should roundtrip: got {}",
            loaded.unwrap()
        );

        // Overwrite
        ace.store_threshold(0.18);
        let loaded2 = ace.get_stored_threshold();
        assert!(
            (loaded2.unwrap() - 0.18).abs() < 0.001,
            "Updated threshold should persist: got {}",
            loaded2.unwrap()
        );
    }
}
