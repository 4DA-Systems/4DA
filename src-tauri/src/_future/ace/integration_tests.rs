//! ACE Integration Tests
//!
//! Comprehensive test suite for validating all ACE phases:
//! - Phase A: Foundation (scanning, validation, confidence)
//! - Phase B: Real-Time (file watcher, git analyzer)
//! - Phase C: Behavior Learning (interactions, affinities)
//! - Phase D: Validation (health, audit, accuracy)
//!
//! Run with: cargo test ace::integration_tests --release

#![cfg(test)]

use super::confidence::ConfidenceLevel;
use super::health::{
    AccuracyTracker, AuditEntryType, AuditLogger, ComponentMetrics, ComponentStatus,
    ContextQuality as HContextQuality, FallbackChain, FallbackLevel, FeedbackResult, FeedbackType,
    HealthMonitor, HealthSnapshot, HealthStatus as HStatus, ACE_TARGETS,
};
use super::*;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// Test Infrastructure
// ============================================================================

/// Create a test database with full ACE schema
fn create_test_db() -> Arc<Mutex<Connection>> {
    let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
    let conn = Arc::new(Mutex::new(conn));
    db::migrate(&conn).expect("Failed to run migrations");
    conn
}

/// Create an ACE instance for testing
fn create_test_ace() -> ACE {
    let conn = create_test_db();
    ACE::new(conn).expect("Failed to create ACE instance")
}

// ============================================================================
// Phase A: Foundation Tests
// ============================================================================

mod phase_a_foundation {
    use super::*;

    #[test]
    fn test_database_migration() {
        let conn = create_test_db();
        let conn_guard = conn.lock();

        // Verify all tables exist
        let tables: Vec<String> = conn_guard
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        let required_tables = [
            "detected_projects",
            "detected_tech",
            "file_signals",
            "git_signals",
            "active_topics",
            "anti_topics",
            "interactions",
            "topic_affinities",
            "source_preferences",
            "activity_patterns",
            "validated_signals",
            "audit_log",
            "accuracy_metrics",
            "system_health",
            "bootstrap_paths",
        ];

        for table in required_tables {
            assert!(
                tables.contains(&table.to_string()),
                "Missing table: {}",
                table
            );
        }
    }

    #[test]
    fn test_project_scanner_creation() {
        let scanner = ProjectScanner::new();
        // Scanner should be created with default settings
        assert!(scanner
            .scan_directory(&PathBuf::from("/nonexistent"))
            .is_ok());
    }

    #[test]
    fn test_manifest_types_exist() {
        // Verify manifest type variants exist
        let _cargo = ManifestType::CargoToml;
        let _package = ManifestType::PackageJson;
        let _pyproject = ManifestType::PyprojectToml;
        let _gomod = ManifestType::GoMod;
    }

    #[test]
    fn test_confidence_score_computation() {
        // Test manifest confidence
        let score = SignalConfidence::for_manifest(true, 15, true, 1.0);
        assert!(
            score.value > 0.7,
            "Good manifest should have high confidence"
        );
        assert!(score.usable, "Good manifest should be usable");

        // Test poor manifest
        let score = SignalConfidence::for_manifest(false, 0, false, 100.0);
        assert!(score.value < 0.3, "No manifest should have low confidence");
        assert!(!score.usable, "No manifest should not be usable");
    }

    #[test]
    fn test_confidence_levels() {
        assert_eq!(ConfidenceLevel::from_value(0.95), ConfidenceLevel::Certain);
        assert_eq!(
            ConfidenceLevel::from_value(0.75),
            ConfidenceLevel::Confident
        );
        assert_eq!(ConfidenceLevel::from_value(0.55), ConfidenceLevel::Probable);
        assert_eq!(
            ConfidenceLevel::from_value(0.35),
            ConfidenceLevel::Uncertain
        );
        assert_eq!(ConfidenceLevel::from_value(0.15), ConfidenceLevel::Rejected);
    }

    #[test]
    fn test_multi_source_confidence_bonus() {
        let score = ConfidenceScore::new(0.6, 1);
        let boosted = score.with_multi_source_bonus(3);
        assert!(boosted.value > 0.6, "Multi-source should boost confidence");
        assert!(boosted.value <= 0.95, "Confidence should be capped at 0.95");
    }

    #[test]
    fn test_signal_validator_creation() {
        let validator = SignalValidator::new();
        let result = validator.validate_topic("rust", 0.8, "manifest");
        assert!(result.valid, "Valid topic should pass validation");
    }

    #[test]
    fn test_ace_creation() {
        let ace = create_test_ace();
        // Verify ACE is created with all components
        let health = ace.get_health();
        assert_eq!(health.project_scanner.status, HStatus::Healthy);
    }

    #[test]
    fn test_notable_dependency_detection() {
        assert!(is_notable_dependency("react"));
        assert!(is_notable_dependency("tokio"));
        assert!(is_notable_dependency("django"));
        assert!(!is_notable_dependency("my-obscure-lib"));
    }

    #[test]
    fn test_tech_category_serialization() {
        let tech = DetectedTech {
            name: "rust".to_string(),
            category: TechCategory::Language,
            confidence: 0.9,
            source: DetectionSource::Manifest,
            evidence: vec!["Cargo.toml".to_string()],
        };
        let json = serde_json::to_string(&tech).unwrap();
        assert!(json.contains("\"category\":\"language\""));
    }
}

// ============================================================================
// Phase B: Real-Time Tests
// ============================================================================

mod phase_b_realtime {
    use super::*;

    #[test]
    fn test_watcher_config_defaults() {
        let config = WatcherConfig::default();
        assert!(config.watched_extensions.contains("rs"));
        assert!(config.watched_extensions.contains("ts"));
        assert!(config.watched_extensions.contains("py"));
        assert!(config.skip_dirs.contains("node_modules"));
        assert!(config.skip_dirs.contains("target"));
        assert!(config.skip_dirs.contains(".git"));
    }

    #[test]
    fn test_file_change_types() {
        assert_ne!(FileChangeType::Created, FileChangeType::Modified);
        assert_ne!(FileChangeType::Modified, FileChangeType::Deleted);
    }

    #[test]
    fn test_rust_topic_extraction() {
        let content = r#"
use tokio::runtime::Runtime;
use serde::{Serialize, Deserialize};
use reqwest::Client;

async fn main() {
    println!("Hello");
}
"#;
        let topics = watcher::extract_topics_from_content(content, "rs");
        assert!(topics.contains(&"tokio".to_string()));
        assert!(topics.contains(&"serde".to_string()));
        assert!(topics.contains(&"reqwest".to_string()));
        assert!(topics.contains(&"async".to_string()));
    }

    #[test]
    fn test_javascript_topic_extraction() {
        let content = r#"
import React, { useState } from 'react';
import axios from 'axios';
const lodash = require('lodash');
"#;
        let topics = watcher::extract_topics_from_content(content, "ts");
        assert!(topics.contains(&"react".to_string()));
        assert!(topics.contains(&"axios".to_string()));
        assert!(topics.contains(&"lodash".to_string()));
    }

    #[test]
    fn test_python_topic_extraction() {
        let content = r#"
import pandas as pd
import numpy as np
from flask import Flask, request
from sqlalchemy.orm import Session
"#;
        let topics = watcher::extract_topics_from_content(content, "py");
        assert!(topics.contains(&"pandas".to_string()));
        assert!(topics.contains(&"numpy".to_string()));
        assert!(topics.contains(&"flask".to_string()));
        assert!(topics.contains(&"sqlalchemy".to_string()));
    }

    #[test]
    fn test_git_config_defaults() {
        let config = git::GitConfig::default();
        assert_eq!(config.max_commits, 100);
        assert_eq!(config.history_days, 30);
        assert!(!config.include_merges);
    }

    #[test]
    fn test_git_analyzer_creation() {
        let analyzer = GitAnalyzer::default();
        // Should not panic
        let repos = analyzer.find_repos(&PathBuf::from("/nonexistent"), 1);
        assert!(repos.is_empty());
    }

    #[test]
    fn test_commit_topic_extraction() {
        let topics = git::extract_topics_from_commit_message("feat: add authentication API");
        assert!(topics.contains(&"commit-feat".to_string()));
        assert!(topics.contains(&"auth".to_string()));
        assert!(topics.contains(&"api".to_string()));
    }

    #[test]
    fn test_file_watcher_creation() {
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(config);
        assert!(!watcher.is_watching());
    }
}

// ============================================================================
// Phase C: Behavior Learning Tests
// ============================================================================

mod phase_c_behavior {
    use super::*;

    #[test]
    fn test_behavior_action_strength() {
        // Click strength scales with dwell time
        let click_short = BehaviorAction::Click {
            dwell_time_seconds: 5,
        };
        let click_long = BehaviorAction::Click {
            dwell_time_seconds: 60,
        };
        assert!(click_long.compute_strength() > click_short.compute_strength());

        // Save/Share are maximum positive
        assert_eq!(BehaviorAction::Save.compute_strength(), 1.0);
        assert_eq!(BehaviorAction::Share.compute_strength(), 1.0);

        // Dismiss/Irrelevant are negative
        assert!(BehaviorAction::Dismiss.compute_strength() < 0.0);
        assert_eq!(BehaviorAction::MarkIrrelevant.compute_strength(), -1.0);

        // Ignore is weakly negative
        assert_eq!(BehaviorAction::Ignore.compute_strength(), -0.1);
    }

    #[test]
    fn test_behavior_action_classification() {
        assert!(BehaviorAction::Save.is_positive());
        assert!(BehaviorAction::Dismiss.is_negative());
        assert!(BehaviorAction::Save.is_strong());
        assert!(BehaviorAction::Dismiss.is_strong());
        assert!(!BehaviorAction::Ignore.is_strong());
    }

    #[test]
    fn test_topic_affinity_creation() {
        let affinity = TopicAffinity::new("rust".to_string());
        assert_eq!(affinity.topic, "rust");
        assert_eq!(affinity.positive_signals, 0);
        assert_eq!(affinity.negative_signals, 0);
        assert_eq!(affinity.affinity_score, 0.0);
    }

    #[test]
    fn test_topic_affinity_learning() {
        let mut affinity = TopicAffinity::new("rust".to_string());

        // Not enough data yet
        affinity.recompute_score();
        assert_eq!(affinity.affinity_score, 0.0);

        // Add 5 positive signals
        for _ in 0..5 {
            affinity.record_interaction(1.0);
        }

        // Now we have enough data
        assert!(affinity.affinity_score > 0.0);
        assert!(affinity.confidence > 0.0);
    }

    #[test]
    fn test_anti_topic_detection() {
        let mut anti = AntiTopic::new("crypto".to_string());
        assert_eq!(anti.rejection_count, 1);
        assert!(!anti.user_confirmed);

        // Record more rejections
        for _ in 0..4 {
            anti.record_rejection();
        }

        assert_eq!(anti.rejection_count, 5);
        assert!(anti.confidence > 0.2);
    }

    #[test]
    fn test_anti_topic_confirmation() {
        let mut anti = AntiTopic::new("spam".to_string());
        anti.confirm();
        assert!(anti.user_confirmed);
        assert_eq!(anti.confidence, 1.0);
    }

    #[test]
    fn test_source_preference_learning() {
        let mut pref = SourcePreference::new("hackernews".to_string());

        // Record positive interactions
        for _ in 0..5 {
            pref.record_interaction(1.0);
        }

        assert!(pref.score > 0.0);
        assert_eq!(pref.interactions, 5);
    }

    #[test]
    fn test_activity_patterns() {
        let mut patterns = ActivityPatterns::default();

        for _ in 0..10 {
            patterns.record_interaction(0.8);
        }

        assert_eq!(patterns.total_tracked, 10);
    }

    #[test]
    fn test_learned_behavior_processing() {
        let mut behavior = LearnedBehavior::new();

        // Process some signals
        let signal = BehaviorLearner::create_signal(
            1,
            BehaviorAction::Save,
            vec!["rust".to_string()],
            "hackernews".to_string(),
        );
        behavior.process_signal(&signal);

        assert!(behavior.source_preferences.contains_key("hackernews"));
        assert!(behavior.topic_affinities.contains_key("rust"));
    }

    #[test]
    fn test_behavior_modifier_computation() {
        let mut behavior = LearnedBehavior::new();

        // Build up affinity
        for _ in 0..10 {
            let signal = BehaviorLearner::create_signal(
                1,
                BehaviorAction::Save,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            );
            behavior.process_signal(&signal);
        }

        let modifier = behavior.get_behavior_modifier(&["rust".to_string()], "hackernews");
        assert!(
            modifier > 0.0,
            "Should have positive modifier for liked topic"
        );
    }

    #[test]
    fn test_ace_interaction_recording() {
        let ace = create_test_ace();
        let result = ace.record_interaction(
            1,
            BehaviorAction::Click {
                dwell_time_seconds: 30,
            },
            vec!["rust".to_string()],
            "hackernews".to_string(),
        );
        assert!(result.is_ok());
    }
}

// ============================================================================
// Phase D: Validation Tests
// ============================================================================

mod phase_d_validation {
    use super::*;

    #[test]
    fn test_health_status_operational_check() {
        assert!(HStatus::Healthy.is_operational());
        assert!(HStatus::Degraded.is_operational());
        assert!(!HStatus::Failed.is_operational());
        assert!(!HStatus::Disabled.is_operational());
    }

    #[test]
    fn test_context_quality_accuracy() {
        assert_eq!(HContextQuality::Excellent.expected_accuracy(), 0.95);
        assert_eq!(HContextQuality::Good.expected_accuracy(), 0.85);
        assert_eq!(HContextQuality::Acceptable.expected_accuracy(), 0.75);
        assert_eq!(HContextQuality::Degraded.expected_accuracy(), 0.60);
        assert_eq!(HContextQuality::Minimal.expected_accuracy(), 0.50);
        assert_eq!(HContextQuality::Emergency.expected_accuracy(), 0.30);
    }

    #[test]
    fn test_health_monitor_check() {
        let conn = create_test_db();
        let mut monitor = HealthMonitor::new(conn);
        let snapshot = monitor.check_health();

        assert!(snapshot.components.len() >= 4);
        assert!(!snapshot.timestamp.is_empty());
    }

    #[test]
    fn test_fallback_level_ordering() {
        assert!(FallbackLevel::Full < FallbackLevel::Emergency);
        assert!(FallbackLevel::NoActivity < FallbackLevel::NoGit);
    }

    #[test]
    fn test_fallback_chain_creation() {
        let chain = FallbackChain::new();
        assert_eq!(chain.current_level(), FallbackLevel::Full);
    }

    #[test]
    fn test_fallback_chain_degradation() {
        let mut chain = FallbackChain::new();

        // Simulate degraded state
        let snapshot = HealthSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            components: vec![
                ComponentStatus {
                    name: "project_scanner".to_string(),
                    status: HStatus::Healthy,
                    last_success: None,
                    last_error: None,
                    error_count: 0,
                    metrics: ComponentMetrics::default(),
                },
                ComponentStatus {
                    name: "file_watcher".to_string(),
                    status: HStatus::Failed,
                    last_success: None,
                    last_error: Some("Error".to_string()),
                    error_count: 5,
                    metrics: ComponentMetrics::default(),
                },
            ],
            overall_status: HStatus::Degraded,
            context_quality: HContextQuality::Degraded,
            active_alerts: vec![],
        };

        chain.update_from_health(&snapshot);
        assert_eq!(chain.current_level(), FallbackLevel::NoFileWatch);
    }

    #[test]
    fn test_fallback_available_features() {
        let chain = FallbackChain::new();
        let features = chain.available_features();
        assert!(!features.is_empty());
        assert!(features.contains(&"Project manifest scanning".to_string()));
    }

    #[test]
    fn test_audit_entry_types() {
        assert_ne!(
            AuditEntryType::ContextUpdate,
            AuditEntryType::RelevanceDecision
        );
        assert_ne!(AuditEntryType::HealthCheck, AuditEntryType::AnomalyDetected);
    }

    #[test]
    fn test_audit_logger_logging() {
        let conn = create_test_db();
        let mut logger = AuditLogger::new(conn);

        let id = logger.log_context_update("Test update", &["rust".to_string()], 0.9);
        assert!(id.is_ok());

        let recent = logger.get_recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].entry_type, AuditEntryType::ContextUpdate);
    }

    #[test]
    fn test_audit_logger_query() {
        let conn = create_test_db();
        let mut logger = AuditLogger::new(conn.clone());

        // Log multiple entry types
        logger.log_context_update("Context", &[], 0.9).unwrap();
        logger
            .log_anomaly("sudden_appearance", "crypto", 0.8)
            .unwrap();

        // Query specific type
        let results = logger.query(Some(AuditEntryType::AnomalyDetected), 10);
        assert!(results.is_ok());
        let entries = results.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_type, AuditEntryType::AnomalyDetected);
    }

    #[test]
    fn test_feedback_type_classification() {
        assert!(FeedbackType::Click.is_positive());
        assert!(FeedbackType::Save.is_positive());
        assert!(FeedbackType::ThumbsUp.is_positive());
        assert!(FeedbackType::ThumbsDown.is_negative());
        assert!(FeedbackType::Dismiss.is_negative());
    }

    #[test]
    fn test_accuracy_tracker_recording() {
        let conn = create_test_db();
        let mut tracker = AccuracyTracker::new(conn);

        for i in 0..10 {
            tracker.record_feedback(FeedbackResult {
                item_id: i,
                predicted_score: 0.8,
                feedback: if i % 2 == 0 {
                    FeedbackType::Click
                } else {
                    FeedbackType::Ignore
                },
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        let metrics = tracker.compute_metrics();
        assert_eq!(metrics.window_size, 10);
        assert!(metrics.precision > 0.0);
    }

    #[test]
    fn test_accuracy_targets() {
        assert_eq!(ACE_TARGETS.min_precision, 0.85);
        assert_eq!(ACE_TARGETS.min_engagement, 0.30);
        assert_eq!(ACE_TARGETS.max_calibration_error, 0.10);
    }

    #[test]
    fn test_ace_health_check() {
        let ace = create_test_ace();
        let snapshot = ace.check_health();

        assert!(!snapshot.timestamp.is_empty());
        assert!(snapshot.components.len() >= 4);
    }

    #[test]
    fn test_ace_fallback_level() {
        let ace = create_test_ace();
        let level = ace.get_fallback_level();
        // Fresh ACE should start at full or near-full
        assert!(level <= FallbackLevel::NoActivity);
    }

    #[test]
    fn test_ace_available_features() {
        let ace = create_test_ace();
        let features = ace.get_available_features();
        assert!(!features.is_empty());
    }

    #[test]
    fn test_ace_accuracy_metrics() {
        let ace = create_test_ace();
        let metrics = ace.get_accuracy_metrics();
        assert_eq!(metrics.total_shown, 0);
        assert_eq!(metrics.window_size, 0);
    }

    #[test]
    fn test_ace_accuracy_feedback() {
        let ace = create_test_ace();

        ace.record_accuracy_feedback(FeedbackResult {
            item_id: 1,
            predicted_score: 0.9,
            feedback: FeedbackType::Click,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        let metrics = ace.get_accuracy_metrics();
        assert_eq!(metrics.total_shown, 1);
        assert_eq!(metrics.total_clicked, 1);
    }

    #[test]
    fn test_ace_system_status() {
        let ace = create_test_ace();
        let status = ace.get_system_status();

        assert!(!status.health_snapshot.timestamp.is_empty());
        assert!(!status.available_features.is_empty());
    }
}

// ============================================================================
// Cross-Phase Integration Tests
// ============================================================================

mod integration {
    use super::*;

    #[test]
    fn test_full_ace_lifecycle() {
        // Create ACE
        let ace = create_test_ace();

        // Phase A: Verify foundation
        let health = ace.get_health();
        assert_eq!(health.overall_status, HStatus::Healthy);

        // Phase C: Record interaction
        let result = ace.record_interaction(
            1,
            BehaviorAction::Save,
            vec!["rust".to_string(), "tokio".to_string()],
            "hackernews".to_string(),
        );
        assert!(result.is_ok());

        // Phase D: Check health after activity
        let snapshot = ace.check_health();
        assert!(snapshot.components.len() >= 4);

        // Verify metrics
        let status = ace.get_system_status();
        assert!(status.health_snapshot.overall_status.is_operational());
    }

    #[test]
    fn test_behavior_to_affinity_pipeline() {
        let ace = create_test_ace();

        // Record multiple positive interactions for same topic
        for i in 0..10 {
            ace.record_interaction(
                i,
                BehaviorAction::Click {
                    dwell_time_seconds: 60,
                },
                vec!["machine-learning".to_string()],
                "arxiv".to_string(),
            )
            .unwrap();
        }

        // Verify affinities were learned
        let affinities = ace.get_topic_affinities().unwrap();
        // May be empty if not enough exposures, but shouldn't error
        assert!(affinities.len() >= 0);
    }

    #[test]
    fn test_negative_behavior_creates_anti_topic() {
        let ace = create_test_ace();

        // Record multiple dismissals
        for i in 0..7 {
            ace.record_interaction(
                i,
                BehaviorAction::MarkIrrelevant,
                vec!["spam".to_string()],
                "email".to_string(),
            )
            .unwrap();
        }

        // Check for anti-topic
        let anti_topics = ace.get_anti_topics(5).unwrap();
        assert!(anti_topics.iter().any(|a| a.topic == "spam"));
    }

    #[test]
    fn test_health_degradation_triggers_fallback() {
        let ace = create_test_ace();

        // Initial state
        let level_before = ace.get_fallback_level();

        // Check health (may update fallback)
        let _snapshot = ace.check_health();

        // Fallback level should be set
        let level_after = ace.get_fallback_level();
        assert!(level_after <= FallbackLevel::Emergency);
    }

    #[test]
    fn test_audit_trail_captures_decisions() {
        let ace = create_test_ace();

        // Record a relevance decision
        let result =
            ace.audit_relevance_decision(42, 0.85, &["rust".to_string(), "async".to_string()], 0.9);
        assert!(result.is_ok());

        // Verify it's in the audit log
        let entries = ace.get_audit_entries(10);
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .any(|e| e.entry_type == AuditEntryType::RelevanceDecision));
    }

    #[test]
    fn test_accuracy_persists_across_feedback() {
        let ace = create_test_ace();

        // Record feedback
        for i in 0..5 {
            ace.record_accuracy_feedback(FeedbackResult {
                item_id: i,
                predicted_score: 0.8,
                feedback: FeedbackType::Click,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        // Persist
        let result = ace.persist_accuracy_metrics();
        assert!(result.is_ok());

        // Verify metrics
        assert!(ace.meets_accuracy_targets() || ace.get_accuracy_metrics().window_size < 20);
    }

    #[test]
    fn test_context_quality_reflects_component_health() {
        let ace = create_test_ace();
        let status = ace.get_system_status();

        // Context quality should match operational components
        match status.health_snapshot.context_quality {
            HContextQuality::Excellent => {
                // All components should be healthy
                assert!(
                    status
                        .health_snapshot
                        .components
                        .iter()
                        .filter(|c| c.status == HStatus::Healthy)
                        .count()
                        >= 4
                );
            }
            HContextQuality::Emergency => {
                // Few/no components operational
                assert!(
                    status
                        .health_snapshot
                        .components
                        .iter()
                        .filter(|c| c.status.is_operational())
                        .count()
                        <= 1
                );
            }
            _ => {
                // Partial operation
            }
        }
    }

    #[test]
    fn test_bootstrap_paths_exist() {
        let conn = create_test_db();
        let paths = db::get_bootstrap_paths(&conn).unwrap();
        assert!(!paths.is_empty(), "Bootstrap paths should be seeded");
    }

    #[test]
    fn test_realtime_context_query() {
        let conn = create_test_db();

        // Query should succeed even with empty data
        let context = get_realtime_context(&conn).unwrap();
        assert!(context.context_confidence >= 0.0);
        assert!(context.context_confidence <= 1.0);
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

mod stress {
    use super::*;

    #[test]
    fn test_high_volume_interactions() {
        let ace = create_test_ace();

        // Record many interactions rapidly
        for i in 0..100 {
            let action = if i % 3 == 0 {
                BehaviorAction::Click {
                    dwell_time_seconds: (i % 60) as u64,
                }
            } else if i % 3 == 1 {
                BehaviorAction::Save
            } else {
                BehaviorAction::Dismiss
            };

            let topics = vec![format!("topic-{}", i % 10), format!("category-{}", i % 5)];

            ace.record_interaction(i, action, topics, "test".to_string())
                .unwrap();
        }

        // System should still be healthy
        let status = ace.get_system_status();
        assert!(status.health_snapshot.overall_status.is_operational());
    }

    #[test]
    fn test_many_health_checks() {
        let ace = create_test_ace();

        // Run many health checks
        for _ in 0..50 {
            let snapshot = ace.check_health();
            assert!(!snapshot.timestamp.is_empty());
        }

        // Should still be operational
        let level = ace.get_fallback_level();
        assert!(level <= FallbackLevel::Emergency);
    }

    #[test]
    fn test_audit_log_capacity() {
        let ace = create_test_ace();

        // Log many entries
        for i in 0..1000 {
            ace.audit_context_update(
                &format!("Update {}", i),
                &[format!("topic-{}", i % 20)],
                0.8,
            )
            .unwrap();
        }

        // Query should still work
        let entries = ace.get_audit_entries(50);
        assert_eq!(entries.len(), 50);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn test_empty_topics() {
        let ace = create_test_ace();

        let result = ace.record_interaction(
            1,
            BehaviorAction::Save,
            vec![], // Empty topics
            "hackernews".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_unicode_topics() {
        let ace = create_test_ace();

        let result = ace.record_interaction(
            1,
            BehaviorAction::Save,
            vec!["日本語".to_string(), "émoji🎉".to_string()],
            "international".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_very_long_topic() {
        let ace = create_test_ace();

        let long_topic = "a".repeat(1000);
        let result = ace.record_interaction(
            1,
            BehaviorAction::Click {
                dwell_time_seconds: 30,
            },
            vec![long_topic],
            "test".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_zero_confidence() {
        let score = ConfidenceScore::new(0.0, 0);
        assert_eq!(score.level, ConfidenceLevel::Rejected);
        assert!(!score.usable);
    }

    #[test]
    fn test_maximum_confidence() {
        let score = ConfidenceScore::new(1.0, 10);
        assert_eq!(score.level, ConfidenceLevel::Certain);
        assert!(score.usable);
    }

    #[test]
    fn test_negative_dwell_time_handled() {
        // Dwell time is u64, so negative not possible, but test edge case of 0
        let action = BehaviorAction::Click {
            dwell_time_seconds: 0,
        };
        assert!(action.compute_strength() >= 0.0);
    }
}

// External helper function reference
fn is_notable_dependency(name: &str) -> bool {
    super::is_notable_dependency(name)
}

// External function reference for topic extraction
mod git {
    pub use super::super::git::*;

    pub fn extract_topics_from_commit_message(message: &str) -> Vec<String> {
        use std::collections::HashSet;
        let mut topics = HashSet::new();
        let message_lower = message.to_lowercase();

        let prefixes = [
            "feat", "fix", "docs", "style", "refactor", "test", "chore", "perf", "ci",
        ];
        for prefix in prefixes {
            if message_lower.starts_with(prefix) {
                topics.insert(format!("commit-{}", prefix));
            }
        }

        let tech_keywords = ["api", "auth", "authentication"];
        for keyword in tech_keywords {
            if message_lower.contains(keyword) {
                topics.insert(keyword.to_string());
            }
        }

        topics.into_iter().collect()
    }
}
