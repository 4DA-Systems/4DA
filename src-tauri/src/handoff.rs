//! Context Handoff Protocol for 4DA
//!
//! Generates compressed "context packets" capturing current work state,
//! open signals, saved items, and active context for consumption by
//! another session or AI agent.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPacket {
    pub generated_at: String,
    pub version: String,
    pub active_context: ActiveContextSnapshot,
    pub open_signals: Vec<SignalSummary>,
    pub saved_items: Vec<SavedItemSummary>,
    pub recent_briefing: Option<String>,
    pub attention_state: AttentionSnapshot,
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveContextSnapshot {
    pub detected_tech: Vec<String>,
    pub active_topics: Vec<String>,
    pub interests: Vec<String>,
    pub exclusions: Vec<String>,
    pub context_dirs: Vec<String>,
    pub recent_work_topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSummary {
    pub item_id: i64,
    pub title: String,
    pub signal_type: String,
    pub priority: String,
    pub action: Option<String>,
    pub source_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedItemSummary {
    pub item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub saved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionSnapshot {
    pub top_topics: Vec<(String, f32)>,
    pub topic_count: usize,
    pub total_interactions: u64,
}

// ============================================================================
// Implementation
// ============================================================================

/// Generate a context packet from current system state
pub fn generate_packet() -> Result<ContextPacket, String> {
    let conn = crate::open_db_connection()?;

    // Gather active context from ACE
    let active_context = gather_active_context()?;

    // Gather open signals (items with signal classification)
    let open_signals = gather_open_signals(&conn)?;

    // Gather saved items (positive feedback)
    let saved_items = gather_saved_items(&conn)?;

    // Get latest briefing
    let recent_briefing = crate::digest_commands::get_latest_briefing_text();

    // Compute attention snapshot
    let attention_state = compute_attention_snapshot(&conn)?;

    // Generate suggested actions based on signals and state
    let suggested_actions = generate_suggestions(&open_signals, &active_context);

    Ok(ContextPacket {
        generated_at: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        active_context,
        open_signals,
        saved_items,
        recent_briefing,
        attention_state,
        suggested_actions,
    })
}

fn gather_active_context() -> Result<ActiveContextSnapshot, String> {
    let settings = crate::get_settings_manager().lock();
    let s = settings.get();

    let (detected_tech, active_topics, interests, exclusions) = {
        match crate::get_context_engine() {
            Ok(engine) => {
                let interests: Vec<String> = engine
                    .get_interests()
                    .unwrap_or_default()
                    .iter()
                    .map(|i| i.topic.clone())
                    .collect();
                let exclusions = engine.get_exclusions().unwrap_or_default();
                // Try to get ACE data
                let (tech, topics) = match crate::get_ace_engine() {
                    Ok(ace) => {
                        let tech: Vec<String> = ace
                            .get_detected_tech()
                            .unwrap_or_default()
                            .iter()
                            .map(|dt| dt.name.clone())
                            .collect();
                        let topics: Vec<String> = ace
                            .get_active_topics()
                            .unwrap_or_default()
                            .iter()
                            .map(|at| at.topic.clone())
                            .collect();
                        (tech, topics)
                    }
                    Err(_) => (vec![], vec![]),
                };
                (tech, topics, interests, exclusions)
            }
            Err(_) => (vec![], vec![], vec![], vec![]),
        }
    };

    Ok(ActiveContextSnapshot {
        detected_tech,
        active_topics: active_topics.clone(),
        interests,
        exclusions,
        context_dirs: s.context_dirs.clone(),
        recent_work_topics: active_topics,
    })
}

fn gather_open_signals(conn: &rusqlite::Connection) -> Result<Vec<SignalSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.source_type, si.url
             FROM source_items si
             WHERE si.created_at >= datetime('now', '-3 days')
             ORDER BY si.created_at DESC
             LIMIT 50",
        )
        .map_err(|e| e.to_string())?;

    // We don't store signal classifications in the DB yet, so we return recent high-value items
    let items: Vec<SignalSummary> = stmt
        .query_map([], |row| {
            Ok(SignalSummary {
                item_id: row.get(0)?,
                title: row.get(1)?,
                source_type: row.get(2)?,
                signal_type: "recent".to_string(),
                priority: "medium".to_string(),
                action: None,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .take(20)
        .collect();

    Ok(items)
}

fn gather_saved_items(conn: &rusqlite::Connection) -> Result<Vec<SavedItemSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.url, si.source_type, f.created_at
             FROM feedback f
             JOIN source_items si ON si.id = f.source_item_id
             WHERE f.relevant = 1
             ORDER BY f.created_at DESC
             LIMIT 20",
        )
        .map_err(|e| e.to_string())?;

    let items: Vec<SavedItemSummary> = stmt
        .query_map([], |row| {
            Ok(SavedItemSummary {
                item_id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                source_type: row.get(3)?,
                saved_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

fn compute_attention_snapshot(conn: &rusqlite::Connection) -> Result<AttentionSnapshot, String> {
    // Get topic affinities as attention proxy
    let mut stmt = conn
        .prepare(
            "SELECT topic, affinity_score FROM topic_affinities
             WHERE total_exposures >= 3
             ORDER BY ABS(affinity_score) DESC
             LIMIT 15",
        )
        .map_err(|e| {
            // Table might not exist in ACE db - that's OK
            warn!(target: "4da::handoff", "Could not read topic_affinities: {}", e);
            e.to_string()
        });

    let top_topics = match stmt {
        Ok(ref mut s) => s
            .query_map([], |row| {
                let topic: String = row.get(0)?;
                let score: f32 = row.get(1)?;
                Ok((topic, score))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect(),
        Err(_) => vec![],
    };

    let total_interactions: u64 = conn
        .query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(AttentionSnapshot {
        topic_count: top_topics.len(),
        top_topics,
        total_interactions,
    })
}

fn generate_suggestions(signals: &[SignalSummary], context: &ActiveContextSnapshot) -> Vec<String> {
    let mut suggestions = Vec::new();

    if !signals.is_empty() {
        suggestions.push(format!(
            "Review {} recent items from your sources",
            signals.len()
        ));
    }

    if !context.active_topics.is_empty() {
        suggestions.push(format!(
            "Continue working on: {}",
            context
                .active_topics
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if context.detected_tech.len() > 5 {
        suggestions.push("Consider checking dependency health for your projects".to_string());
    }

    suggestions
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn generate_context_packet() -> Result<ContextPacket, String> {
    crate::settings::require_pro_feature("generate_context_packet")?;
    info!(target: "4da::handoff", "Generating context packet");
    generate_packet()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(tech: &[&str], topics: &[&str]) -> ActiveContextSnapshot {
        ActiveContextSnapshot {
            detected_tech: tech.iter().map(|s| s.to_string()).collect(),
            active_topics: topics.iter().map(|s| s.to_string()).collect(),
            interests: vec![],
            exclusions: vec![],
            context_dirs: vec![],
            recent_work_topics: vec![],
        }
    }

    fn make_signal(title: &str) -> SignalSummary {
        SignalSummary {
            item_id: 1,
            title: title.to_string(),
            signal_type: "recent".to_string(),
            priority: "medium".to_string(),
            action: None,
            source_type: "hackernews".to_string(),
        }
    }

    // --- generate_suggestions tests ---

    #[test]
    fn suggestions_empty_when_no_signals_no_context() {
        let suggestions = generate_suggestions(&[], &make_context(&[], &[]));
        assert!(suggestions.is_empty());
    }

    #[test]
    fn suggestions_includes_review_when_signals_present() {
        let signals = vec![make_signal("Test")];
        let suggestions = generate_suggestions(&signals, &make_context(&[], &[]));
        assert!(suggestions
            .iter()
            .any(|s| s.contains("Review") && s.contains("1")));
    }

    #[test]
    fn suggestions_includes_continue_when_topics_present() {
        let context = make_context(&[], &["rust", "tauri"]);
        let suggestions = generate_suggestions(&[], &context);
        assert!(suggestions
            .iter()
            .any(|s| s.contains("Continue working on")));
    }

    #[test]
    fn suggestions_continue_truncates_to_3_topics() {
        let context = make_context(&[], &["rust", "tauri", "react", "python", "go"]);
        let suggestions = generate_suggestions(&[], &context);
        let continue_suggestion = suggestions.iter().find(|s| s.contains("Continue")).unwrap();
        // Should only contain first 3 topics
        assert!(continue_suggestion.contains("rust"));
        assert!(continue_suggestion.contains("tauri"));
        assert!(continue_suggestion.contains("react"));
        assert!(!continue_suggestion.contains("python"));
    }

    #[test]
    fn suggestions_includes_dep_health_when_many_tech() {
        let context = make_context(&["a", "b", "c", "d", "e", "f"], &[]);
        let suggestions = generate_suggestions(&[], &context);
        assert!(suggestions.iter().any(|s| s.contains("dependency health")));
    }

    #[test]
    fn suggestions_no_dep_health_when_few_tech() {
        let context = make_context(&["a", "b", "c"], &[]);
        let suggestions = generate_suggestions(&[], &context);
        assert!(!suggestions.iter().any(|s| s.contains("dependency health")));
    }

    #[test]
    fn suggestions_can_have_all_three() {
        let signals = vec![make_signal("Test")];
        let context = make_context(&["a", "b", "c", "d", "e", "f"], &["rust"]);
        let suggestions = generate_suggestions(&signals, &context);
        assert_eq!(suggestions.len(), 3);
    }

    // --- Serde roundtrip tests ---

    #[test]
    fn context_packet_serde_roundtrip() {
        let packet = ContextPacket {
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            version: "1.0.0".to_string(),
            active_context: make_context(&["rust"], &["tauri"]),
            open_signals: vec![make_signal("Test signal")],
            saved_items: vec![],
            recent_briefing: Some("Latest briefing".to_string()),
            attention_state: AttentionSnapshot {
                top_topics: vec![("rust".to_string(), 0.9)],
                topic_count: 1,
                total_interactions: 42,
            },
            suggested_actions: vec!["Do something".to_string()],
        };
        let json = serde_json::to_string(&packet).unwrap();
        let back: ContextPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, "1.0.0");
        assert_eq!(back.open_signals.len(), 1);
        assert_eq!(back.attention_state.total_interactions, 42);
    }

    #[test]
    fn saved_item_summary_serde_roundtrip() {
        let item = SavedItemSummary {
            item_id: 42,
            title: "Test".to_string(),
            url: Some("https://example.com".to_string()),
            source_type: "hackernews".to_string(),
            saved_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: SavedItemSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.item_id, 42);
        assert_eq!(back.url, Some("https://example.com".to_string()));
    }

    #[test]
    fn attention_snapshot_serde_roundtrip() {
        let snap = AttentionSnapshot {
            top_topics: vec![("rust".to_string(), 0.9), ("go".to_string(), 0.3)],
            topic_count: 2,
            total_interactions: 100,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: AttentionSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.top_topics.len(), 2);
        assert_eq!(back.topic_count, 2);
    }
}
