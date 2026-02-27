//! Context processing — file change handling, freshness decay, and real-time context.

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

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

/// Real-time context data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeContext {
    pub active_topics: Vec<ActiveTopic>,
    pub detected_tech: Vec<DetectedTech>,
    pub context_confidence: f32,
    pub last_updated: String,
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
pub fn process_file_changes(
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

// ============================================================================
// Freshness & Real-time Context
// ============================================================================

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

// ============================================================================
// Detection Storage Helpers
// ============================================================================

/// Store git analysis signals (topics + commit dedup) into ACE tables.
pub fn store_git_signals(
    conn: &Arc<Mutex<Connection>>,
    signals: &[GitSignal],
) -> Result<(), String> {
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
            .map_err(|e| format!("Failed to store git topic: {}", e))?;
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
        .map_err(|e| format!("Failed to store git signal: {}", e))?;
    }

    Ok(())
}

/// Store detected technologies and active topics into ACE tables.
pub fn store_detected_context(
    conn: &Arc<Mutex<Connection>>,
    tech: &[DetectedTech],
    topics: &[ActiveTopic],
) -> Result<(), String> {
    let conn = conn.lock();

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
        assert!(is_notable_dependency("vite"));
        assert!(is_notable_dependency("prisma"));
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
        assert!((hours - 2.0).abs() < 0.1, "Should be ~2 hours ago, got {}", hours);
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
        assert!(hours < 0.1, "Current time should be ~0 hours ago, got {}", hours);
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
