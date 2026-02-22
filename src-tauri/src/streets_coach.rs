//! STREETS Coach -- AI coaching system for developer autonomy.
//!
//! Provides personalized coaching using the user's configured LLM (BYOK),
//! with access to Sovereign Profile, Developer DNA, Tech Radar, Decisions,
//! playbook progress, and scoring data.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachSession {
    pub id: String,
    pub session_type: String,
    pub title: String,
    pub context_snapshot: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachMessage {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub token_count: u64,
    pub cost_cents: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRecommendation {
    pub primary_engine: EngineChoice,
    pub secondary_engine: EngineChoice,
    pub reasoning: String,
    pub profile_gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineChoice {
    pub engine_number: u8,
    pub engine_name: String,
    pub fit_score: f32,
    pub time_to_first_dollar: String,
    pub revenue_range: String,
    pub reasoning: String,
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchReviewResult {
    pub overall_score: f32,
    pub strengths: Vec<String>,
    pub gaps: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachContext {
    pub sovereign_profile_summary: String,
    pub developer_dna_summary: String,
    pub tech_radar_summary: String,
    pub active_decisions: Vec<String>,
    pub playbook_progress: String,
    pub top_engaged_topics: Vec<String>,
    pub profile_completeness: f32,
}

// ============================================================================
// Helpers
// ============================================================================

fn generate_session_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut h = DefaultHasher::new();
    nanos.hash(&mut h);
    std::thread::current().id().hash(&mut h);
    format!("coach-{:016x}", h.finish())
}

pub fn require_streets_feature(feature: &str) -> Result<(), String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if matches!(license.tier.as_str(), "pro" | "team") {
        return Ok(());
    }
    if let Ok(payload) = crate::settings::verify_license_key(&license.license_key) {
        if payload.features.contains(&feature.to_string()) {
            return Ok(());
        }
    }
    Err(format!(
        "{} requires STREETS Community or Cohort membership",
        feature
    ))
}

fn ensure_coach_tables(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS coach_sessions (
            id TEXT PRIMARY KEY, session_type TEXT NOT NULL, title TEXT NOT NULL,
            context_snapshot TEXT, created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS coach_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
            role TEXT NOT NULL, content TEXT NOT NULL,
            token_count INTEGER NOT NULL DEFAULT 0, cost_cents INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (session_id) REFERENCES coach_sessions(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS coach_documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT, doc_type TEXT NOT NULL,
            content TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .map_err(|e| format!("Failed to create coach tables: {}", e))
}

fn get_llm_client() -> Result<crate::llm::LLMClient, String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let provider = guard.get().llm.clone();
    if provider.api_key.is_empty() && provider.provider != "ollama" {
        return Err("LLM not configured -- set up your API key in Settings".to_string());
    }
    Ok(crate::llm::LLMClient::new(provider))
}

/// Record LLM usage and return (total_tokens, cost_cents).
fn record_llm_usage(
    client: &crate::llm::LLMClient,
    response: &crate::llm::LLMResponse,
) -> (u64, u64) {
    let total = response.input_tokens + response.output_tokens;
    let cost = client.estimate_cost_cents(response.input_tokens, response.output_tokens);
    let mut settings = crate::get_settings_manager().lock();
    settings.record_usage(total, cost);
    (total, cost)
}

fn assemble_coach_context() -> CoachContext {
    let conn = crate::open_db_connection().ok();
    // 1. Sovereign profile
    let sovereign_profile_summary = conn
        .as_ref()
        .and_then(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT category, key, value FROM sovereign_profile ORDER BY category, key",
                )
                .ok()?;
            let rows: Vec<String> = stmt
                .query_map([], |row| {
                    Ok(format!(
                        "{}/{}: {}",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();
            if rows.is_empty() {
                None
            } else {
                Some(rows.join("; "))
            }
        })
        .unwrap_or_default();
    // 2. Developer DNA
    let (developer_dna_summary, top_engaged_topics) = match crate::developer_dna::generate_dna() {
        Ok(dna) => {
            let blind = dna
                .blind_spots
                .iter()
                .take(3)
                .map(|b| b.dependency.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let summary = format!(
                "Identity: {}. Stack: {}. Interests: {}. Blind spots: {}.",
                dna.identity_summary,
                dna.primary_stack.join(", "),
                dna.interests.join(", "),
                blind
            );
            let topics = dna
                .top_engaged_topics
                .iter()
                .take(5)
                .map(|t| t.topic.clone())
                .collect();
            (summary, topics)
        }
        Err(_) => (String::new(), Vec::new()),
    };
    // 3. Tech Radar
    let tech_radar_summary = conn
        .as_ref()
        .and_then(|c| {
            let radar = crate::tech_radar::compute_radar(c).ok()?;
            let by_ring = |ring: &str| {
                radar
                    .entries
                    .iter()
                    .filter(|e| format!("{:?}", e.ring).to_lowercase() == ring)
                    .take(5)
                    .map(|e| e.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            Some(format!(
                "Adopt: [{}]. Trial: [{}]. Assess: [{}]. Hold: [{}].",
                by_ring("adopt"),
                by_ring("trial"),
                by_ring("assess"),
                by_ring("hold")
            ))
        })
        .unwrap_or_default();
    // 4. Active decisions
    let active_decisions = conn.as_ref().and_then(|c| {
        let mut stmt = c.prepare("SELECT subject, decision FROM developer_decisions WHERE status = 'active' LIMIT 10").ok()?;
        let rows: Vec<String> = stmt.query_map([], |row| {
            Ok(format!("{}: {}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).ok()?.filter_map(|r| r.ok()).collect();
        Some(rows)
    }).unwrap_or_default();
    // 5. Playbook progress
    let playbook_progress = conn
        .as_ref()
        .and_then(|c| {
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM playbook_progress", [], |r| r.get(0))
                .unwrap_or(0);
            Some(format!("{} lessons completed", n))
        })
        .unwrap_or_else(|| "0 lessons completed".to_string());
    // 6. Profile completeness
    let profile_completeness = conn
        .as_ref()
        .map(|c| {
            let filled: i64 = c
                .query_row(
                    "SELECT COUNT(DISTINCT category) FROM sovereign_profile",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            (filled as f32 / 9.0) * 100.0
        })
        .unwrap_or(0.0);

    CoachContext {
        sovereign_profile_summary,
        developer_dna_summary,
        tech_radar_summary,
        active_decisions,
        playbook_progress,
        top_engaged_topics,
        profile_completeness,
    }
}

fn format_system_prompt(session_type: &str, ctx: &CoachContext) -> String {
    let or_default = |s: &str, d: &str| {
        if s.is_empty() {
            d.to_string()
        } else {
            s.to_string()
        }
    };
    let decisions_str = if ctx.active_decisions.is_empty() {
        "None recorded".into()
    } else {
        ctx.active_decisions.join("; ")
    };
    let topics_str = if ctx.top_engaged_topics.is_empty() {
        "None yet".into()
    } else {
        ctx.top_engaged_topics.join(", ")
    };
    let context_block = format!(
        "## Developer Profile\nSovereign Profile: {}\nDeveloper DNA: {}\nTech Radar: {}\n\
         Active Decisions: {}\nPlaybook Progress: {}\nTop Topics: {}\nProfile Completeness: {:.0}%",
        or_default(&ctx.sovereign_profile_summary, "Not yet configured"),
        or_default(&ctx.developer_dna_summary, "Not enough data yet"),
        or_default(&ctx.tech_radar_summary, "No radar data yet"),
        decisions_str,
        ctx.playbook_progress,
        topics_str,
        ctx.profile_completeness
    );

    let role_text = match session_type {
        "engine_recommender" => "You are the STREETS Revenue Engine Recommender. Analyze the developer's profile and recommend \
            the best-fit revenue engines from: 1) Open Source + Sponsorship 2) Technical Writing 3) Freelance/Consulting \
            4) SaaS/Micro-SaaS 5) Developer Tools 6) Education/Courses 7) API/Infrastructure 8) Productized Services.\n\n\
            You MUST respond with valid JSON: {\"primary_engine\":{\"engine_number\":N,\"engine_name\":\"...\",\"fit_score\":0.0-1.0,\
            \"time_to_first_dollar\":\"...\",\"revenue_range\":\"...\",\"reasoning\":\"...\",\"prerequisites\":[...]},\
            \"secondary_engine\":{...},\"reasoning\":\"...\",\"profile_gaps\":[...]}",
        "strategy" => "You are the STREETS Strategic Advisor. Generate a comprehensive personalized strategy in markdown. \
            Cover: current position analysis, recommended revenue engines, 90-day roadmap, key risks, specific next actions. \
            Base everything on the developer's actual data. Be specific, not generic.",
        "launch_review" => "You are the STREETS Launch Reviewer. Evaluate project readiness. MUST respond with valid JSON: \
            {\"overall_score\":0.0-10.0,\"strengths\":[...],\"gaps\":[...],\"recommendations\":[...]}. \
            Score on: technical readiness, market positioning, revenue model clarity, operational maturity, differentiation.",
        "progress" => "You are the STREETS Progress Coach. Analyze playbook progress and autonomy journey. Provide: \
            progress summary, bottlenecks, specific next steps, motivational context. Respond in markdown. Be encouraging but honest.",
        _ => "You are the STREETS Coach -- a personalized AI advisor for developer autonomy and revenue generation. \
            You know the STREETS framework (Sovereign Setup, Technical Moats, Revenue Engines, Execution, Evolving Edge, \
            Tactical Automation, Stacking Streams). Give specific, actionable advice referencing their actual stack, decisions, and progress.",
    };
    format!("{}\n\n{}", role_text, context_block)
}

fn extract_json_from_response(response: &str) -> &str {
    if let Some(start) = response.find("```json") {
        let after = &response[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    if let Some(start) = response.find("```") {
        let after = &response[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('{') || inner.starts_with('[') {
                return inner;
            }
        }
    }
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            return &response[start..=end];
        }
    }
    response
}

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
    conn.execute("INSERT INTO coach_sessions (id, session_type, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, session_type, resolved_title, now, now])
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
        let mut stmt = conn.prepare("SELECT role, content FROM coach_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 20")
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
        conn.execute("INSERT INTO coach_messages (session_id, role, content, token_count, cost_cents, created_at) VALUES (?1, 'user', ?2, 0, 0, ?3)",
            params![session_id, content, now]).map_err(|e| format!("Failed to store user message: {}", e))?;
        (system, msgs)
    }; // conn and stmt dropped here
       // Phase 2: Async LLM call (no DB handles held)
    let client = get_llm_client()?;
    let response = client.complete(&system, messages).await?;
    let (total_tokens, cost) = record_llm_usage(&client, &response);
    // Phase 3: Store response in new connection
    let conn = crate::open_db_connection()?;
    let assistant_now = chrono::Utc::now().to_rfc3339();
    conn.execute("INSERT INTO coach_messages (session_id, role, content, token_count, cost_cents, created_at) VALUES (?1, 'assistant', ?2, ?3, ?4, ?5)",
        params![session_id, response.content, total_tokens, cost, assistant_now])
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
    let messages = vec![crate::llm::Message { role: "user".to_string(),
        content: "Give me a progress check-in. Where am I, what should I focus on next, and what am I doing well?".to_string() }];
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

    #[test]
    fn test_generate_session_id_format() {
        let id = generate_session_id();
        assert!(id.starts_with("coach-"));
        assert_eq!(id.len(), 22); // "coach-" (6) + 16 hex chars
    }

    #[test]
    fn test_extract_json_from_fenced_response() {
        let response = "Here is the analysis:\n```json\n{\"score\": 7.5}\n```\nDone.";
        assert_eq!(extract_json_from_response(response), "{\"score\": 7.5}");
    }

    #[test]
    fn test_extract_json_raw_object() {
        let response = "The result is {\"key\": \"value\"} as shown.";
        assert_eq!(extract_json_from_response(response), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_extract_json_no_json() {
        let response = "No JSON here at all.";
        assert_eq!(extract_json_from_response(response), response);
    }

    #[test]
    fn test_format_system_prompt_chat() {
        let ctx = CoachContext {
            sovereign_profile_summary: "cpu/model: AMD Ryzen".to_string(),
            developer_dna_summary: "Rust/TS developer".to_string(),
            tech_radar_summary: "Adopt: [rust, typescript]".to_string(),
            active_decisions: vec!["sqlite: Use SQLite".to_string()],
            playbook_progress: "3 lessons completed".to_string(),
            top_engaged_topics: vec!["rust".to_string(), "wasm".to_string()],
            profile_completeness: 44.0,
        };
        let prompt = format_system_prompt("chat", &ctx);
        assert!(prompt.contains("STREETS Coach"));
        assert!(prompt.contains("AMD Ryzen"));
        assert!(prompt.contains("Rust/TS developer"));
        assert!(prompt.contains("44%"));
    }

    #[test]
    fn test_format_system_prompt_engine_recommender() {
        let ctx = CoachContext {
            sovereign_profile_summary: String::new(),
            developer_dna_summary: String::new(),
            tech_radar_summary: String::new(),
            active_decisions: Vec::new(),
            playbook_progress: "0 lessons completed".to_string(),
            top_engaged_topics: Vec::new(),
            profile_completeness: 0.0,
        };
        let prompt = format_system_prompt("engine_recommender", &ctx);
        assert!(prompt.contains("Revenue Engine Recommender"));
        assert!(prompt.contains("engine_number"));
        assert!(prompt.contains("Not yet configured"));
    }
}
