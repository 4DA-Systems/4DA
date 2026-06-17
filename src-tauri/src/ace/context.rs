// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Context processing — file change handling, freshness decay, and real-time context.

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

use super::{
    ActiveTopic, DetectedTech, DetectionSource, FileChange, FileChangeType, GitSignal,
    TechCategory, TopicSource,
};

// ============================================================================
// Types
// ============================================================================

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
pub fn parse_hours_ago(timestamp_str: &str) -> f32 {
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
pub fn is_notable_dependency(name: &str) -> bool {
    // Only frameworks, runtimes, and core libraries that define a developer's stack.
    // ORMs (drizzle, prisma, typeorm, mongoose) and build tools (vite, webpack)
    // excluded — they are companion dependencies, not identity-defining.
    let notable = [
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
        "react",
        "react-dom",
        "react-native",
        "vue",
        "angular",
        "svelte",
        "next",
        "nuxt",
        "express",
        "fastify",
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
        "gin",
        "echo",
        "fiber",
        "gorm",
        "cobra",
        "viper",
        "postgresql",
        "mysql",
        "sqlite",
        "mongodb",
        "redis",
        "elasticsearch",
    ];

    let notable_scopes = [
        "tauri-apps",
        "nestjs",
        "prisma",
        "angular",
        "vue",
        "nuxt",
        "svelte",
        "tensorflow",
        "pytorch",
    ];

    let lower = name.to_lowercase();

    if let Some(rest) = lower.strip_prefix('@') {
        // Scoped package: match scope root only
        let scope = rest.split('/').next().unwrap_or("");
        return notable_scopes.contains(&scope);
    }

    // Unscoped: exact match only
    notable.contains(&lower.as_str())
}

/// Merge duplicate detected technologies
pub fn merge_detected_tech(tech: Vec<DetectedTech>) -> Vec<DetectedTech> {
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

// ============================================================================
// File Change Processing
// ============================================================================

/// Process file changes from the watcher
pub fn process_file_changes(conn: &Arc<Mutex<Connection>>, changes: &[FileChange]) -> Result<()> {
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
        let (topics, extracted_text, source_type) = if change.change_type == FileChangeType::Deleted
        {
            (Vec::new(), None, "deleted".to_string())
        } else {
            // Check if this is a document file that needs extraction
            let ext = change
                .path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
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
                        let topics = super::watcher::extract_topics_from_content(&doc.text, &ext);
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
                let topics =
                    super::watcher::extract_topics_from_file(&change.path).unwrap_or_default();
                (topics, None, "text".to_string())
            }
        };

        let topics_json = serde_json::to_string(&topics).unwrap_or_default();

        conn.execute(
            "INSERT INTO file_signals (path, change_type, extracted_topics, timestamp)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![change.path.to_string_lossy(), change_type_str, topics_json],
        )
        .context("Failed to store file signal")?;

        for topic in &topics {
            conn.execute(
                "INSERT INTO active_topics (topic, weight, confidence, source, last_seen)
                 VALUES (?1, 0.6, 0.7, 'file_content', datetime('now'))
                 ON CONFLICT(topic) DO UPDATE SET
                    weight = MAX(excluded.weight, active_topics.weight),
                    last_seen = datetime('now')",
                rusqlite::params![topic],
            )
            .context("Failed to update active topic")?;
        }

        // Rich topic extraction — deeper semantic signals from file content
        let ext = change
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if let Ok(meta) = std::fs::metadata(&change.path) {
            if meta.len() > 10_000_000 {
                continue; // Skip files > 10MB for topic extraction
            }
        }
        if let Ok(content) = std::fs::read_to_string(&change.path) {
            let rich_topics = super::watcher::extract_rich_topics(&content, ext);
            for (topic, confidence) in &rich_topics {
                conn.execute(
                    "INSERT INTO active_topics (topic, weight, confidence, source, last_seen)
                     VALUES (?1, ?2, ?3, 'file_content', datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        weight = MAX(excluded.weight, active_topics.weight),
                        confidence = MAX(excluded.confidence, active_topics.confidence),
                        last_seen = datetime('now')",
                    rusqlite::params![topic, confidence, confidence],
                )
                .unwrap_or_else(|e| {
                    warn!(target: "4da::ace", error = %e, topic = %topic, "Failed to upsert rich topic");
                    0
                }); // Non-critical — don't fail the whole batch
            }
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
                ).unwrap_or_else(|e| {
                    warn!(target: "4da::ace", error = %e, path = %file_path_str, "Failed to upsert indexed_documents");
                    0
                });

                // Get the document ID
                if let Ok(doc_id) = conn.query_row(
                    "SELECT id FROM indexed_documents WHERE file_path = ?1",
                    rusqlite::params![file_path_str],
                    |row| row.get::<_, i64>(0),
                ) {
                    // Delete old chunks for this document
                    if let Err(e) = conn.execute(
                        "DELETE FROM document_chunks WHERE document_id = ?1",
                        rusqlite::params![doc_id],
                    ) {
                        warn!(target: "4da::ace", error = %e, doc_id, "Failed to delete old document chunks");
                    }

                    // Split text into chunks (max 1000 words per chunk)
                    let words: Vec<&str> = text.split_whitespace().collect();
                    let chunk_size = 1000;
                    for (i, chunk_words) in words.chunks(chunk_size).enumerate() {
                        let chunk_text = chunk_words.join(" ");
                        let chunk_word_count = chunk_words.len() as i64;
                        if let Err(e) = conn.execute(
                            "INSERT INTO document_chunks (document_id, chunk_index, content, word_count)
                             VALUES (?1, ?2, ?3, ?4)",
                            rusqlite::params![doc_id, i as i64, chunk_text, chunk_word_count],
                        ) {
                            warn!(target: "4da::ace", error = %e, doc_id, chunk_index = i, "Failed to insert document chunk");
                        }
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

// ============================================================================
// Detection Storage Helpers
// ============================================================================

/// Store git analysis signals (topics + commit dedup) into ACE tables.
pub fn store_git_signals(conn: &Arc<Mutex<Connection>>, signals: &[GitSignal]) -> Result<()> {
    let conn = conn.lock();

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
            .context("Failed to store git topic")?;
        }

        let topics_json = serde_json::to_string(&signal.extracted_topics).unwrap_or_default();
        let messages: String = signal
            .recent_commits
            .iter()
            .take(5)
            .map(|c| c.message.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        let hash = match &signal.last_commit {
            Some(h) if !h.is_empty() => h.clone(),
            _ => continue,
        };
        let already_stored: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM git_signals WHERE repo_path = ?1 AND commit_hash = ?2)",
                rusqlite::params![signal.repo_path.to_string_lossy(), &hash],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if already_stored {
            continue;
        }
        conn.execute(
            "INSERT INTO git_signals (repo_path, commit_hash, commit_message, extracted_topics, timestamp)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![
                signal.repo_path.to_string_lossy(),
                &hash,
                &messages,
                topics_json
            ],
        )
        .context("Failed to store git signal")?;
    }

    Ok(())
}

/// Store detected technologies and active topics into ACE tables.
///
/// Clears previous manifest-sourced entries before inserting so that
/// activity-weighted confidence reflects current project state, not
/// stale maximums from previous scans. Non-manifest entries (explicit,
/// git_history) are preserved.
pub fn store_detected_context(
    conn: &Arc<Mutex<Connection>>,
    tech: &[DetectedTech],
    topics: &[ActiveTopic],
) -> Result<()> {
    let conn = conn.lock();

    conn.execute("DELETE FROM detected_tech WHERE source = 'manifest'", [])
        .ok();

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
                evidence = CASE WHEN excluded.confidence > detected_tech.confidence
                    THEN excluded.evidence ELSE detected_tech.evidence END,
                updated_at = datetime('now'),
                -- Re-detection re-baselines decay: clear last_decay_at so the next
                -- decay run measures from the fresh updated_at, not a stale baseline.
                last_decay_at = NULL",
            rusqlite::params![
                t.name,
                category_str,
                t.confidence,
                source_str,
                t.evidence.join("; ")
            ],
        )
        .context("Failed to store detected tech")?;
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
        .context("Failed to store active topic")?;
    }

    Ok(())
}

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

    // ====================================================================
    // is_notable_dependency comprehensive tests
    // ====================================================================

    #[test]
    fn test_notable_dependency_rust_ecosystem() {
        assert!(is_notable_dependency("tokio"));
        assert!(is_notable_dependency("serde"));
        assert!(is_notable_dependency("reqwest"));
        assert!(is_notable_dependency("axum"));
        assert!(is_notable_dependency("tauri"));
        assert!(is_notable_dependency("hyper"));
    }

    #[test]
    fn test_notable_dependency_js_ecosystem() {
        assert!(is_notable_dependency("react"));
        assert!(is_notable_dependency("vue"));
        assert!(is_notable_dependency("angular"));
        assert!(is_notable_dependency("next"));
        assert!(is_notable_dependency("express"));
    }

    #[test]
    fn test_notable_dependency_python_ecosystem() {
        assert!(is_notable_dependency("django"));
        assert!(is_notable_dependency("flask"));
        assert!(is_notable_dependency("fastapi"));
        assert!(is_notable_dependency("numpy"));
        assert!(is_notable_dependency("pandas"));
        assert!(is_notable_dependency("tensorflow"));
    }

    #[test]
    fn test_notable_dependency_databases() {
        assert!(is_notable_dependency("postgresql"));
        assert!(is_notable_dependency("mysql"));
        assert!(is_notable_dependency("sqlite"));
        assert!(is_notable_dependency("mongodb"));
        assert!(is_notable_dependency("redis"));
    }

    #[test]
    fn test_notable_dependency_case_insensitive() {
        assert!(is_notable_dependency("React"));
        assert!(is_notable_dependency("TOKIO"));
        assert!(is_notable_dependency("Django"));
    }

    #[test]
    fn test_not_notable_dependency() {
        assert!(!is_notable_dependency("my-lib"));
        assert!(!is_notable_dependency("custom-tool"));
        assert!(!is_notable_dependency("utils"));
        assert!(!is_notable_dependency("helpers"));
        assert!(!is_notable_dependency("vite"));
        assert!(!is_notable_dependency("webpack"));
        assert!(!is_notable_dependency("prisma"));
        assert!(is_notable_dependency("@prisma/client"));
    }

    // ====================================================================
    // parse_hours_ago tests
    // ====================================================================

    #[test]
    fn test_parse_hours_ago_valid_timestamp() {
        let now = chrono::Utc::now().naive_utc();
        let two_hours_ago = now - chrono::Duration::hours(2);
        let ts_str = two_hours_ago.format("%Y-%m-%d %H:%M:%S").to_string();
        let hours = parse_hours_ago(&ts_str);
        assert!(
            (hours - 2.0).abs() < 0.1,
            "Should be ~2 hours ago, got {}",
            hours
        );
    }

    #[test]
    fn test_parse_hours_ago_invalid_timestamp() {
        let hours = parse_hours_ago("not-a-timestamp");
        assert_eq!(hours, 24.0, "Invalid timestamps should return 24.0");
    }

    #[test]
    fn test_parse_hours_ago_empty_string() {
        let hours = parse_hours_ago("");
        assert_eq!(hours, 24.0, "Empty string should return 24.0");
    }

    #[test]
    fn test_parse_hours_ago_recent() {
        let now = chrono::Utc::now().naive_utc();
        let ts_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let hours = parse_hours_ago(&ts_str);
        assert!(
            hours < 0.1,
            "Current time should be ~0 hours ago, got {}",
            hours
        );
    }

    // ====================================================================
    // merge_detected_tech tests
    // ====================================================================

    #[test]
    fn test_merge_detected_tech_empty() {
        let result = merge_detected_tech(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_merge_detected_tech_no_duplicates() {
        let tech = vec![
            DetectedTech {
                name: "Rust".to_string(),
                category: TechCategory::Language,
                confidence: 0.9,
                source: DetectionSource::Manifest,
                evidence: vec!["Cargo.toml".to_string()],
            },
            DetectedTech {
                name: "TypeScript".to_string(),
                category: TechCategory::Language,
                confidence: 0.8,
                source: DetectionSource::FileExtension,
                evidence: vec![".ts files".to_string()],
            },
        ];
        let result = merge_detected_tech(tech);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_merge_detected_tech_deduplicates() {
        let tech = vec![
            DetectedTech {
                name: "Rust".to_string(),
                category: TechCategory::Language,
                confidence: 0.7,
                source: DetectionSource::FileExtension,
                evidence: vec![".rs files".to_string()],
            },
            DetectedTech {
                name: "rust".to_string(),
                category: TechCategory::Language,
                confidence: 0.9,
                source: DetectionSource::Manifest,
                evidence: vec!["Cargo.toml".to_string()],
            },
        ];
        let result = merge_detected_tech(tech);
        assert_eq!(result.len(), 1, "Duplicate names should be merged");
        assert_eq!(result[0].confidence, 0.9, "Should keep higher confidence");
        assert_eq!(result[0].evidence.len(), 2, "Evidence should be merged");
    }

    #[test]
    fn test_merge_detected_tech_sorted_by_confidence() {
        let tech = vec![
            DetectedTech {
                name: "Python".to_string(),
                category: TechCategory::Language,
                confidence: 0.5,
                source: DetectionSource::FileExtension,
                evidence: vec![],
            },
            DetectedTech {
                name: "Rust".to_string(),
                category: TechCategory::Language,
                confidence: 0.9,
                source: DetectionSource::Manifest,
                evidence: vec![],
            },
            DetectedTech {
                name: "Go".to_string(),
                category: TechCategory::Language,
                confidence: 0.7,
                source: DetectionSource::FileExtension,
                evidence: vec![],
            },
        ];
        let result = merge_detected_tech(tech);
        assert_eq!(result.len(), 3);
        assert!(result[0].confidence >= result[1].confidence);
        assert!(result[1].confidence >= result[2].confidence);
    }
}
