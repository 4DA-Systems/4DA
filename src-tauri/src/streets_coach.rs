//! STREETS Coach -- AI coaching Tauri commands.
//!
//! Uses coach_context for types, context assembly, and helpers.
//! This module contains only the 9 Tauri command handlers.

use rusqlite::params;
use tracing::{debug, info};

pub use crate::coach_context::{
    assemble_coach_context, ensure_coach_tables, extract_json_from_response, format_system_prompt,
    generate_session_id, get_llm_client, record_llm_usage, require_streets_feature, CoachMessage,
    CoachSession, EngineRecommendation, LaunchReviewResult,
};

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn coach_create_session(
    session_type: String,
    title: Option<String>,
) -> Result<CoachSession, String> {
    require_streets_feature("streets_community")?;
    let conn = crate::open_db_connection()?;
    ensure_coach_tables(&conn)?;
    let id = generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let resolved_title = title.unwrap_or_else(|| {
        match session_type.as_str() {
            "engine_recommender" => "Engine Recommendation",
            "strategy" => "Strategy Session",
            "launch_review" => "Launch Review",
            "progress" => "Progress Check-in",
            _ => "Coach Chat",
        }
        .to_string()
    });
    conn.execute(
        "INSERT INTO coach_sessions (id, session_type, title, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, session_type, resolved_title, now, now],
    )
    .map_err(|e| format!("Failed to create session: {}", e))?;
    info!(target: "4da::coach", session_id = %id, session_type = %session_type, "Created coach session");
    Ok(CoachSession {
        id,
        session_type,
        title: resolved_title,
        context_snapshot: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn coach_send_message(
    session_id: String,
    content: String,
) -> Result<CoachMessage, String> {
    require_streets_feature("streets_community")?;
    // Phase 1: All DB reads in a sync block (no holding conn across await)
    let (system, messages) = {
        let conn = crate::open_db_connection()?;
        ensure_coach_tables(&conn)?;
        let session_type: String = conn
            .query_row(
                "SELECT session_type FROM coach_sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|_| format!("Session not found: {}", session_id))?;
        let mut stmt = conn
            .prepare(
                "SELECT role, content FROM coach_messages \
                 WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 20",
            )
            .map_err(|e| format!("Failed to query messages: {}", e))?;
        let mut history: Vec<(String, String)> = stmt
            .query_map(params![session_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Failed to read messages: {}", e))?
            .filter_map(|r| r.ok())
            .collect();
        history.reverse();
        let ctx = assemble_coach_context();
        let system = format_system_prompt(&session_type, &ctx);
        let mut msgs: Vec<crate::llm::Message> = history
            .iter()
            .map(|(role, msg)| crate::llm::Message {
                role: role.clone(),
                content: msg.clone(),
            })
            .collect();
        msgs.push(crate::llm::Message {
            role: "user".to_string(),
            content: content.clone(),
        });
        // Store user message before LLM call
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO coach_messages (session_id, role, content, token_count, cost_cents, created_at) \
             VALUES (?1, 'user', ?2, 0, 0, ?3)",
            params![session_id, content, now],
        )
        .map_err(|e| format!("Failed to store user message: {}", e))?;
        (system, msgs)
    }; // conn and stmt dropped here

    // Phase 2: Async LLM call (no DB handles held)
    let client = get_llm_client()?;
    let response = client.complete(&system, messages).await?;
    let (total_tokens, cost) = record_llm_usage(&client, &response);

    // Phase 3: Store response in new connection
    let conn = crate::open_db_connection()?;
    let assistant_now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO coach_messages (session_id, role, content, token_count, cost_cents, created_at) \
         VALUES (?1, 'assistant', ?2, ?3, ?4, ?5)",
        params![session_id, response.content, total_tokens, cost, assistant_now],
    )
    .map_err(|e| format!("Failed to store assistant message: {}", e))?;
    conn.execute(
        "UPDATE coach_sessions SET updated_at = ?1 WHERE id = ?2",
        params![assistant_now, session_id],
    )
    .map_err(|e| format!("Failed to update session: {}", e))?;
    let msg_id: i64 = conn
        .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
        .unwrap_or(0);
    debug!(target: "4da::coach", session_id = %session_id, tokens = total_tokens, cost_cents = cost, "Coach response delivered");
    Ok(CoachMessage {
        id: msg_id,
        session_id,
        role: "assistant".to_string(),
        content: response.content,
        token_count: total_tokens,
        cost_cents: cost,
        created_at: assistant_now,
    })
}

#[tauri::command]
pub async fn coach_get_history(session_id: String) -> Result<Vec<CoachMessage>, String> {
    let conn = crate::open_db_connection()?;
    ensure_coach_tables(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, role, content, token_count, cost_cents, created_at \
             FROM coach_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| format!("Failed to query history: {}", e))?;
    let messages: Vec<CoachMessage> = stmt
        .query_map(params![session_id], |row| {
            Ok(CoachMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get::<_, i64>(4).unwrap_or(0) as u64,
                cost_cents: row.get::<_, i64>(5).unwrap_or(0) as u64,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Failed to read history: {}", e))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(messages)
}

#[tauri::command]
pub async fn coach_list_sessions() -> Result<Vec<CoachSession>, String> {
    let conn = crate::open_db_connection()?;
    ensure_coach_tables(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, session_type, title, context_snapshot, created_at, updated_at \
             FROM coach_sessions ORDER BY updated_at DESC LIMIT 50",
        )
        .map_err(|e| format!("Failed to query sessions: {}", e))?;
    let sessions: Vec<CoachSession> = stmt
        .query_map([], |row| {
            Ok(CoachSession {
                id: row.get(0)?,
                session_type: row.get(1)?,
                title: row.get(2)?,
                context_snapshot: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("Failed to read sessions: {}", e))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sessions)
}

#[tauri::command]
pub async fn coach_delete_session(session_id: String) -> Result<(), String> {
    let conn = crate::open_db_connection()?;
    ensure_coach_tables(&conn)?;
    conn.execute(
        "DELETE FROM coach_messages WHERE session_id = ?1",
        params![session_id],
    )
    .map_err(|e| format!("Failed to delete messages: {}", e))?;
    conn.execute(
        "DELETE FROM coach_sessions WHERE id = ?1",
        params![session_id],
    )
    .map_err(|e| format!("Failed to delete session: {}", e))?;
    info!(target: "4da::coach", session_id = %session_id, "Deleted coach session");
    Ok(())
}

#[tauri::command]
pub async fn coach_recommend_engines() -> Result<EngineRecommendation, String> {
    require_streets_feature("streets_community")?;
    let ctx = assemble_coach_context();
    let system = format_system_prompt("engine_recommender", &ctx);
    let client = get_llm_client()?;
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content:
            "Analyze my profile and recommend the best revenue engines. Respond with JSON only."
                .to_string(),
    }];
    let response = client.complete(&system, messages).await?;
    record_llm_usage(&client, &response);
    let json_str = extract_json_from_response(&response.content);
    let rec: EngineRecommendation = serde_json::from_str(json_str).map_err(|e| {
        format!(
            "Failed to parse engine recommendation: {}. Raw: {}",
            e,
            &response.content[..response.content.len().min(200)]
        )
    })?;
    info!(target: "4da::coach", primary = %rec.primary_engine.engine_name, secondary = %rec.secondary_engine.engine_name, "Engine recommendation generated");
    Ok(rec)
}

#[tauri::command]
pub async fn coach_generate_strategy() -> Result<String, String> {
    require_streets_feature("streets_community")?;
    let ctx = assemble_coach_context();
    let system = format_system_prompt("strategy", &ctx);
    let client = get_llm_client()?;
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content:
            "Generate my personalized STREETS strategy document. Be comprehensive and specific."
                .to_string(),
    }];
    let response = client.complete(&system, messages).await?;
    record_llm_usage(&client, &response);
    // Store in coach_documents
    if let Ok(conn) = crate::open_db_connection() {
        let _ = ensure_coach_tables(&conn);
        let _ = conn.execute(
            "INSERT INTO coach_documents (doc_type, content) VALUES ('strategy', ?1)",
            params![response.content],
        );
    }
    info!(target: "4da::coach", length = response.content.len(), "Strategy document generated");
    Ok(response.content)
}

#[tauri::command]
pub async fn coach_launch_review(
    project_description: String,
) -> Result<LaunchReviewResult, String> {
    require_streets_feature("streets_community")?;
    let ctx = assemble_coach_context();
    let system = format_system_prompt("launch_review", &ctx);
    let client = get_llm_client()?;
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: format!(
            "Review this project for launch readiness:\n\n{}",
            project_description
        ),
    }];
    let response = client.complete(&system, messages).await?;
    record_llm_usage(&client, &response);
    let json_str = extract_json_from_response(&response.content);
    let result: LaunchReviewResult = serde_json::from_str(json_str).map_err(|e| {
        format!(
            "Failed to parse launch review: {}. Raw: {}",
            e,
            &response.content[..response.content.len().min(200)]
        )
    })?;
    info!(target: "4da::coach", score = result.overall_score, "Launch review completed");
    Ok(result)
}

#[tauri::command]
pub async fn coach_progress_check_in() -> Result<String, String> {
    require_streets_feature("streets_community")?;
    let ctx = assemble_coach_context();
    let system = format_system_prompt("progress", &ctx);
    let client = get_llm_client()?;
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: "Give me a progress check-in. Where am I, what should I focus on next, \
                  and what am I doing well?"
            .to_string(),
    }];
    let response = client.complete(&system, messages).await?;
    record_llm_usage(&client, &response);
    info!(target: "4da::coach", length = response.content.len(), "Progress check-in delivered");
    Ok(response.content)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach_context::EngineChoice;

    /// Helper: resolve default session title the same way coach_create_session does.
    fn resolve_default_title(session_type: &str) -> String {
        match session_type {
            "engine_recommender" => "Engine Recommendation",
            "strategy" => "Strategy Session",
            "launch_review" => "Launch Review",
            "progress" => "Progress Check-in",
            _ => "Coach Chat",
        }
        .to_string()
    }

    // -- Session title resolution --

    #[test]
    fn test_default_title_engine_recommender() {
        assert_eq!(
            resolve_default_title("engine_recommender"),
            "Engine Recommendation"
        );
    }

    #[test]
    fn test_default_title_strategy() {
        assert_eq!(resolve_default_title("strategy"), "Strategy Session");
    }

    #[test]
    fn test_default_title_launch_review() {
        assert_eq!(resolve_default_title("launch_review"), "Launch Review");
    }

    #[test]
    fn test_default_title_progress() {
        assert_eq!(resolve_default_title("progress"), "Progress Check-in");
    }

    #[test]
    fn test_default_title_unknown_type_falls_back() {
        assert_eq!(resolve_default_title("anything_else"), "Coach Chat");
        assert_eq!(resolve_default_title(""), "Coach Chat");
    }

    // -- Struct serialization roundtrips --

    #[test]
    fn test_coach_session_serde_roundtrip() {
        let session = CoachSession {
            id: "coach-abc123".to_string(),
            session_type: "strategy".to_string(),
            title: "My Strategy".to_string(),
            context_snapshot: Some("{\"key\":\"val\"}".to_string()),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T01:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: CoachSession = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "coach-abc123");
        assert_eq!(deserialized.session_type, "strategy");
        assert_eq!(deserialized.title, "My Strategy");
        assert_eq!(
            deserialized.context_snapshot,
            Some("{\"key\":\"val\"}".to_string())
        );
    }

    #[test]
    fn test_coach_session_none_context_snapshot() {
        let session = CoachSession {
            id: "coach-xyz".to_string(),
            session_type: "chat".to_string(),
            title: "Chat".to_string(),
            context_snapshot: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"context_snapshot\":null"));
        let back: CoachSession = serde_json::from_str(&json).unwrap();
        assert!(back.context_snapshot.is_none());
    }

    #[test]
    fn test_coach_message_serde_roundtrip() {
        let msg = CoachMessage {
            id: 42,
            session_id: "coach-sess1".to_string(),
            role: "assistant".to_string(),
            content: "Here is my advice.".to_string(),
            token_count: 150,
            cost_cents: 3,
            created_at: "2025-06-15T10:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: CoachMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 42);
        assert_eq!(back.role, "assistant");
        assert_eq!(back.token_count, 150);
        assert_eq!(back.cost_cents, 3);
    }

    #[test]
    fn test_engine_recommendation_serde_roundtrip() {
        let rec = EngineRecommendation {
            primary_engine: EngineChoice {
                engine_number: 4,
                engine_name: "SaaS/Micro-SaaS".to_string(),
                fit_score: 0.85,
                time_to_first_dollar: "3-6 months".to_string(),
                revenue_range: "$500-$5000/mo".to_string(),
                reasoning: "Strong technical skills".to_string(),
                prerequisites: vec!["Landing page".to_string()],
            },
            secondary_engine: EngineChoice {
                engine_number: 2,
                engine_name: "Technical Writing".to_string(),
                fit_score: 0.7,
                time_to_first_dollar: "1-2 months".to_string(),
                revenue_range: "$200-$2000/mo".to_string(),
                reasoning: "Good communication".to_string(),
                prerequisites: vec![],
            },
            reasoning: "Based on your Rust/TS stack".to_string(),
            profile_gaps: vec!["No portfolio".to_string()],
        };
        let json = serde_json::to_string(&rec).unwrap();
        let back: EngineRecommendation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.primary_engine.engine_number, 4);
        assert_eq!(back.secondary_engine.engine_name, "Technical Writing");
        assert!((back.primary_engine.fit_score - 0.85).abs() < f32::EPSILON);
        assert_eq!(back.profile_gaps.len(), 1);
    }

    #[test]
    fn test_launch_review_result_serde_roundtrip() {
        let result = LaunchReviewResult {
            overall_score: 7.5,
            strengths: vec!["Solid architecture".to_string()],
            gaps: vec!["No pricing page".to_string()],
            recommendations: vec![
                "Add testimonials".to_string(),
                "Set up analytics".to_string(),
            ],
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: LaunchReviewResult = serde_json::from_str(&json).unwrap();
        assert!((back.overall_score - 7.5).abs() < f32::EPSILON);
        assert_eq!(back.strengths.len(), 1);
        assert_eq!(back.gaps[0], "No pricing page");
        assert_eq!(back.recommendations.len(), 2);
    }

    // -- generate_session_id --

    #[test]
    fn test_generate_session_id_uniqueness() {
        let ids: Vec<String> = (0..10).map(|_| generate_session_id()).collect();
        // All should have the correct prefix and length
        for id in &ids {
            assert!(
                id.starts_with("coach-"),
                "id should start with coach-: {}",
                id
            );
            assert_eq!(id.len(), 22, "id should be 22 chars: {}", id);
        }
        // At least some should be distinct (hash includes thread id + nanos)
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        // With nanos granularity on the same thread, there is a small chance of collision,
        // but 10 calls should produce at least 2 distinct values.
        assert!(
            deduped.len() >= 2,
            "Expected at least 2 unique IDs from 10 generations"
        );
    }

    // -- extract_json_from_response edge cases --

    #[test]
    fn test_extract_json_from_response_plain_fenced_block() {
        // Fenced code block without "json" language tag, containing JSON
        let response = "Result:\n```\n{\"a\": 1}\n```\nEnd.";
        assert_eq!(extract_json_from_response(response), "{\"a\": 1}");
    }
}
